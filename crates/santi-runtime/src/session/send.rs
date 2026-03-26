use std::sync::Arc;
use std::pin::Pin;

use async_stream::try_stream;
use futures::{Stream, StreamExt};
use santi_core::{
    error::{Error, LockError},
    port::{
        lock::{Lock, LockGuard},
        provider::{Provider, ProviderEvent, ProviderFunctionCall, ProviderRequest},
        turn_store::{NewTurnMessage, TurnStore},
    },
    provider::ProviderInputMessage,
    service::session::kernel::{
        runtime_prompt::{build_runtime_prompt, RuntimePromptSource},
        tool_artifact::{build_tool_call_message, build_tool_result_message},
        transcript,
    },
};
use tokio::sync::mpsc;

use crate::{
    runtime::{
        context::ToolRuntimeContext,
        prompt::render_runtime_instructions,
        tools::{ToolExecutor, ToolExecutorConfig},
    },
    session::memory::SessionMemoryService,
};

#[derive(Clone)]
pub struct SessionSendService {
    model: String,
    lock: Arc<dyn Lock>,
    turn_store: Arc<dyn TurnStore>,
    provider: Arc<dyn Provider>,
    tools: Arc<ToolExecutor>,
}

pub struct SendSessionCommand {
    pub session_id: String,
    pub user_content: String,
}

#[derive(Clone, Debug)]
pub enum SendSessionError {
    Busy,
    NotFound,
    Internal(String),
}

pub enum SendSessionEvent {
    OutputTextDelta(String),
    Completed,
}

pub type SendSessionStream = Pin<Box<dyn Stream<Item = Result<SendSessionEvent, SendSessionError>> + Send>>;

impl SessionSendService {
    pub fn new(
        model: String,
        lock: Arc<dyn Lock>,
        turn_store: Arc<dyn TurnStore>,
        provider: Arc<dyn Provider>,
        session_memory: SessionMemoryService,
        tool_config: ToolExecutorConfig,
    ) -> Self {
        Self {
            model,
            lock,
            turn_store,
            provider,
            tools: Arc::new(ToolExecutor::new(session_memory, tool_config)),
        }
    }

    pub async fn start(
        &self,
        cmd: SendSessionCommand,
    ) -> Result<SendSessionStream, SendSessionError> {
        let model = self.model.clone();
        let lock = self.lock.clone();
        let turn_store = self.turn_store.clone();
        let provider = self.provider.clone();
        let tools = self.tools.clone();
        let session_id = cmd.session_id.clone();
        let guard = lock
            .acquire(&format!("lock:session_send:{}", session_id))
            .await
            .map_err(map_lock_error)?;

        let startup = run_startup(&cmd, turn_store.clone(), tools.clone()).await;
        let (provider_input, instructions, runtime_context, guard) = match startup {
            Ok((provider_input, instructions, runtime_context)) => {
                (provider_input, instructions, runtime_context, guard)
            }
            Err(err) => {
                let release_result = guard.release().await.map_err(map_lock_error);
                return Err(match release_result {
                    Ok(()) => err,
                    Err(release_err) => release_err,
                });
            }
        };

        tracing::info!(session_id = %cmd.session_id, input_messages = provider_input.len(), model = %model, "provider request dispatched");

        let (tx, mut rx) = mpsc::unbounded_channel::<Result<SendSessionEvent, SendSessionError>>();
        let error_tx = tx.clone();

        tokio::spawn(async move {
            let tx = tx.clone();
            let result = run_session_send_worker(
                cmd,
                model,
                provider,
                turn_store,
                provider_input,
                instructions,
                runtime_context,
                tools,
                tx,
                guard,
            )
            .await;

            if let Err(err) = result {
                let _ = error_tx.send(Err(err));
            }
        });

        Ok(Box::pin(try_stream! {
            while let Some(event) = rx.recv().await {
                match event {
                    Ok(event) => yield event,
                    Err(err) => Err(err)?,
                }
            }
        }))
    }
}

async fn run_startup(
    cmd: &SendSessionCommand,
    turn_store: Arc<dyn TurnStore>,
    tools: Arc<ToolExecutor>,
) -> Result<(Vec<ProviderInputMessage>, Option<String>, ToolRuntimeContext), SendSessionError> {
    tracing::info!(session_id = %cmd.session_id, "session send started");

    let turn_context = turn_store
        .load_turn_context(&cmd.session_id)
        .await
        .map_err(map_core_error)?
        .ok_or(SendSessionError::NotFound)?;

    let message = turn_store
        .append_message(
            &cmd.session_id,
            NewTurnMessage {
                r#type: "user".to_string(),
                role: Some("user".to_string()),
                content: cmd.user_content.clone(),
            },
        )
        .await
        .map_err(map_core_error)?;

    tracing::info!(session_id = %cmd.session_id, message_id = %message.id, "user message persisted");

    let history = turn_store
        .list_messages(&cmd.session_id)
        .await
        .map_err(map_core_error)?;

    let runtime_context = tools.build_context(&cmd.session_id, &turn_context.session.soul_id);
    let prompt = build_runtime_prompt(RuntimePromptSource {
        session_id: Some(cmd.session_id.clone()),
        soul_id: Some(turn_context.session.soul_id.clone()),
        soul_memory: Some(turn_context.soul_memory.clone()),
        session_memory: Some(turn_context.session.memory.clone()),
        request_instructions: None,
    });
    let provider_input: Vec<ProviderInputMessage> = history
        .iter()
        .filter_map(transcript::to_input_message)
        .collect();

    let instructions = render_runtime_instructions(&prompt, &runtime_context, &tools);
    Ok((provider_input, instructions, runtime_context))
}

async fn run_session_send_worker(
    cmd: SendSessionCommand,
    model: String,
    provider: Arc<dyn Provider>,
    turn_store: Arc<dyn TurnStore>,
    provider_input: Vec<ProviderInputMessage>,
    instructions: Option<String>,
    runtime_context: ToolRuntimeContext,
    tools: Arc<ToolExecutor>,
    tx: mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>,
    guard: Box<dyn LockGuard + Send>,
) -> Result<(), SendSessionError> {
    let run_result = async {
        let provider_model = model;
        let runtime_instructions = instructions.clone();
        let mut assistant_text = String::new();
        let mut next_request = ProviderRequest {
            model: provider_model.clone(),
            instructions,
            input: provider_input,
            tools: Some(tools.provider_tools()),
            previous_response_id: None,
            function_call_outputs: None,
        };
        let mut should_stream_deltas = false;

        loop {
            let pass = run_provider_pass(provider.clone(), next_request, should_stream_deltas.then_some(&tx)).await?;

            match pass {
                ProviderPassOutcome::Completed { assistant_text: pass_text } => {
                    if !should_stream_deltas && !pass_text.is_empty() {
                        let _ = tx.send(Ok(SendSessionEvent::OutputTextDelta(pass_text.clone())));
                    }
                    assistant_text.push_str(&pass_text);
                    persist_assistant_message(turn_store.clone(), &cmd.session_id, assistant_text).await?;
                    let _ = tx.send(Ok(SendSessionEvent::Completed));
                    break Ok(());
                }
                ProviderPassOutcome::ToolCalls {
                    calls,
                    assistant_prefix,
                    response_id,
                } => {
                    tracing::info!(
                        session_id = %cmd.session_id,
                        tool_calls = calls.len(),
                        prefix_chars = assistant_prefix.len(),
                        "provider pass requested tool calls"
                    );

                    let mut function_call_outputs = Vec::new();

                    for call in calls {
                        append_artifact_message(
                            turn_store.clone(),
                            &cmd.session_id,
                            build_tool_call_message(call.call_id.clone(), call.name.clone(), call.arguments.clone()),
                        )
                        .await?;

                        let dispatch_result = tools
                            .dispatch(&runtime_context, &call)
                            .await
                            .map_err(SendSessionError::Internal)?;

                        append_artifact_message(
                            turn_store.clone(),
                            &cmd.session_id,
                            build_tool_result_message(
                                call.call_id.clone(),
                                dispatch_result.tool_name.clone(),
                                dispatch_result.ok,
                                dispatch_result.tool_output.clone(),
                            ),
                        )
                        .await?;

                        function_call_outputs.push(dispatch_result.function_call_output);
                    }

                    next_request = ProviderRequest {
                        model: provider_model.clone(),
                        instructions: runtime_instructions.clone(),
                        input: Vec::new(),
                        tools: Some(tools.provider_tools()),
                        previous_response_id: Some(response_id),
                        function_call_outputs: Some(function_call_outputs),
                    };
                    should_stream_deltas = true;
                }
            }
        }
    }
    .await;

    let release_result = guard.release().await.map_err(map_lock_error);
    match (run_result, release_result) {
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Ok(()), Ok(())) => Ok(()),
    }
}

enum ProviderPassOutcome {
    Completed { assistant_text: String },
    ToolCalls {
        calls: Vec<ProviderFunctionCall>,
        assistant_prefix: String,
        response_id: String,
    },
}

async fn run_provider_pass(
    provider: Arc<dyn Provider>,
    request: ProviderRequest,
    tx: Option<&mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>>,
) -> Result<ProviderPassOutcome, SendSessionError> {
    let stream = provider.stream(request);
    futures::pin_mut!(stream);

    let mut assistant_text = String::new();
    let mut calls = Vec::new();
    let mut response_id: Option<String> = None;

    while let Some(event) = stream.next().await {
        match event.map_err(map_core_error)? {
            ProviderEvent::OutputTextDelta(delta) => {
                assistant_text.push_str(&delta);
                if let Some(tx) = tx {
                    let _ = tx.send(Ok(SendSessionEvent::OutputTextDelta(delta)));
                }
            }
            ProviderEvent::FunctionCallRequested(call) => {
                if let Some(existing_response_id) = response_id.as_ref() {
                    if existing_response_id != &call.response_id {
                        return Err(SendSessionError::Internal(
                            "provider returned inconsistent response_id across tool calls".to_string(),
                        ));
                    }
                } else {
                    response_id = Some(call.response_id.clone());
                }
                calls.push(call);
            }
            ProviderEvent::Completed { response_id: completed_response_id } => {
                if calls.is_empty() {
                    return Ok(ProviderPassOutcome::Completed { assistant_text });
                }

                let response_id = completed_response_id
                    .or(response_id)
                    .ok_or_else(|| {
                        SendSessionError::Internal(
                            "provider completed tool-call pass without response_id".to_string(),
                        )
                    })?;

                return Ok(ProviderPassOutcome::ToolCalls {
                    calls,
                    assistant_prefix: assistant_text,
                    response_id,
                });
            }
        }
    }

    Err(SendSessionError::Internal(
        "provider stream ended before completion".to_string(),
    ))
}

async fn persist_assistant_message(
    turn_store: Arc<dyn TurnStore>,
    session_id: &str,
    assistant_text: String,
) -> Result<(), SendSessionError> {
    let assistant_message = turn_store
        .append_message(
            session_id,
            NewTurnMessage {
                r#type: "assistant".to_string(),
                role: Some("assistant".to_string()),
                content: assistant_text.clone(),
            },
        )
        .await
        .map_err(map_core_error)?;

    tracing::info!(session_id = %session_id, message_id = %assistant_message.id, output_chars = assistant_text.len(), "session send completed");
    Ok(())
}

async fn append_artifact_message(
    turn_store: Arc<dyn TurnStore>,
    session_id: &str,
    artifact: santi_core::model::message::Message,
) -> Result<(), SendSessionError> {
    turn_store
        .append_message(
            session_id,
            NewTurnMessage {
                r#type: artifact.r#type,
                role: artifact.role,
                content: artifact.content,
            },
        )
        .await
        .map_err(map_core_error)?;
    Ok(())
}

fn map_lock_error(err: LockError) -> SendSessionError {
    match err {
        LockError::Busy => SendSessionError::Busy,
        LockError::Lost => SendSessionError::Internal("session send lock lost".to_string()),
        LockError::Backend { message } => SendSessionError::Internal(message),
    }
}

fn map_core_error(err: Error) -> SendSessionError {
    match err {
        Error::NotFound { resource } if resource == "session" => SendSessionError::NotFound,
        Error::NotFound { resource } => SendSessionError::Internal(format!("{resource} not found")),
        Error::Busy { resource } => SendSessionError::Internal(format!("{resource} busy")),
        Error::InvalidInput { message } => SendSessionError::Internal(message),
        Error::Upstream { message } => SendSessionError::Internal(message),
        Error::Internal { message } => SendSessionError::Internal(message),
    }
}
