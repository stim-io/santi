use std::{pin::Pin, sync::Arc};

use async_stream::try_stream;
use futures::{Stream, StreamExt};
use santi_core::{
    error::{Error, LockError},
    model::{
        message::{ActorType, MessageContent, MessagePart, MessageState},
        runtime::{ProviderState, Turn, TurnTriggerType},
        session::SessionMessage,
    },
    port::{
        lock::{Lock, LockGuard},
        provider::{Provider, ProviderEvent, ProviderFunctionCall, ProviderRequest},
        session_ledger::{AppendSessionMessage, SessionLedgerPort},
        soul_runtime::{AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn, FailTurn, SoulRuntimePort, StartTurn},
    },
    provider::ProviderInputMessage,
    service::session::kernel::{
        runtime_prompt::{build_runtime_prompt, RuntimePromptSource},
        transcript,
    },
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    runtime::{context::ToolRuntimeContext, prompt::render_runtime_instructions, tools::{ToolExecutor, ToolExecutorConfig}},
    session::memory::SessionMemoryService,
};

#[derive(Clone)]
pub struct SessionSendService {
    model: String,
    default_soul_id: String,
    lock: Arc<dyn Lock>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
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

pub type SendSessionStream =
    Pin<Box<dyn Stream<Item = Result<SendSessionEvent, SendSessionError>> + Send>>;

#[derive(Clone)]
struct StartupContext {
    provider_input: Vec<ProviderInputMessage>,
    instructions: Option<String>,
    soul_session_id: String,
    input_through_session_seq: i64,
    trigger_message_id: String,
    runtime_context: ToolRuntimeContext,
}

impl SessionSendService {
    pub fn new(
        model: String,
        default_soul_id: String,
        lock: Arc<dyn Lock>,
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_runtime: Arc<dyn SoulRuntimePort>,
        provider: Arc<dyn Provider>,
        session_memory: SessionMemoryService,
        tool_config: ToolExecutorConfig,
    ) -> Self {
        Self {
            model,
            default_soul_id,
            lock,
            session_ledger,
            soul_runtime,
            provider,
            tools: Arc::new(ToolExecutor::new(session_memory, tool_config)),
        }
    }

    pub async fn start(
        &self,
        cmd: SendSessionCommand,
    ) -> Result<SendSessionStream, SendSessionError> {
        let guard = self
            .lock
            .acquire(&format!("lock:session_send:{}", cmd.session_id))
            .await
            .map_err(map_lock_error)?;

        let startup = run_startup(
            &self.default_soul_id,
            &cmd,
            self.session_ledger.clone(),
            self.soul_runtime.clone(),
            self.tools.clone(),
        )
        .await;

        let (startup, guard) = match startup {
            Ok(startup) => (startup, guard),
            Err(err) => {
                let release_result = guard.release().await.map_err(map_lock_error);
                return Err(match release_result {
                    Ok(()) => err,
                    Err(release_err) => release_err,
                });
            }
        };

        let model = self.model.clone();
        let default_soul_id = self.default_soul_id.clone();
        let provider = self.provider.clone();
        let session_ledger = self.session_ledger.clone();
        let soul_runtime = self.soul_runtime.clone();
        let tools = self.tools.clone();
        let (tx, mut rx) = mpsc::unbounded_channel::<Result<SendSessionEvent, SendSessionError>>();
        let error_tx = tx.clone();

        tokio::spawn(async move {
            let result = run_session_send_worker(
                default_soul_id,
                cmd,
                model,
                provider,
                session_ledger,
                soul_runtime,
                tools,
                startup,
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
    default_soul_id: &str,
    cmd: &SendSessionCommand,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    tools: Arc<ToolExecutor>,
) -> Result<StartupContext, SendSessionError> {
    let session = session_ledger
        .get_session(&cmd.session_id)
        .await
        .map_err(map_core_error)?
        .ok_or(SendSessionError::NotFound)?;

    let soul_session = soul_runtime
        .get_or_create_soul_session(default_soul_id, &cmd.session_id)
        .await
        .map_err(map_core_error)?;

    let turn_context = soul_runtime
        .load_turn_context(default_soul_id, &cmd.session_id)
        .await
        .map_err(map_core_error)?
        .ok_or(SendSessionError::NotFound)?;

    let user_message = session_ledger
        .append_message(AppendSessionMessage {
            session_id: session.id,
            message_id: format!("msg_{}", Uuid::new_v4().simple()),
            actor_type: ActorType::Account,
            actor_id: "account_local".to_string(),
            content: text_content(&cmd.user_content),
            state: MessageState::Fixed,
        })
        .await
        .map_err(map_core_error)?;

    let history = session_ledger
        .list_messages(&cmd.session_id, None)
        .await
        .map_err(map_core_error)?;

    let provider_input = history
        .iter()
        .filter_map(transcript::to_input_message)
        .collect::<Vec<_>>();
    let runtime_context = tools.build_context(&cmd.session_id, &turn_context.soul.id);
    let core_prompt = build_runtime_prompt(RuntimePromptSource {
        session_id: Some(cmd.session_id.clone()),
        soul_id: Some(turn_context.soul.id.clone()),
        soul_memory: Some(turn_context.soul.memory.clone()),
        session_memory: Some(turn_context.soul_session.session_memory.clone()),
        request_instructions: None,
    });
    let instructions = render_runtime_instructions(&core_prompt, &runtime_context, &tools);

    Ok(StartupContext {
        provider_input,
        instructions,
        soul_session_id: soul_session.id,
        input_through_session_seq: user_message.relation.session_seq,
        trigger_message_id: user_message.message.id,
        runtime_context,
    })
}

async fn run_session_send_worker(
    default_soul_id: String,
    cmd: SendSessionCommand,
    model: String,
    provider: Arc<dyn Provider>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    tools: Arc<ToolExecutor>,
    startup: StartupContext,
    tx: mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>,
    guard: Box<dyn LockGuard + Send>,
) -> Result<(), SendSessionError> {
    let turn_id = format!("turn_{}", Uuid::new_v4().simple());
    let started_turn = soul_runtime
        .start_turn(StartTurn {
            turn_id: turn_id.clone(),
            soul_session_id: startup.soul_session_id.clone(),
            trigger_type: TurnTriggerType::SessionSend,
            trigger_ref: Some(startup.trigger_message_id.clone()),
            input_through_session_seq: startup.input_through_session_seq,
        })
        .await
        .map_err(map_core_error)?;

    let run_result = run_turn_body(
        default_soul_id,
        cmd,
        model,
        provider,
        session_ledger,
        soul_runtime.clone(),
        tools,
        startup,
        started_turn,
        tx,
    )
    .await;

    if let Err(err) = &run_result {
        let _ = soul_runtime
            .fail_turn(FailTurn {
                turn_id,
                error_text: render_send_error(err),
            })
            .await;
    }

    let release_result = guard.release().await.map_err(map_lock_error);
    match (run_result, release_result) {
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Ok(()), Ok(())) => Ok(()),
    }
}

async fn run_turn_body(
    default_soul_id: String,
    cmd: SendSessionCommand,
    model: String,
    provider: Arc<dyn Provider>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    tools: Arc<ToolExecutor>,
    startup: StartupContext,
    turn: Turn,
    tx: mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>,
) -> Result<(), SendSessionError> {
    let mut assistant_text = String::new();
    let mut previous_response_id: Option<String> = None;
    let mut function_call_outputs = None;

    loop {
        let request = ProviderRequest {
            model: model.clone(),
            instructions: startup.instructions.clone(),
            input: if previous_response_id.is_some() {
                Vec::new()
            } else {
                startup.provider_input.clone()
            },
            tools: Some(tools.provider_tools()),
            previous_response_id: previous_response_id.clone(),
            function_call_outputs: function_call_outputs.take(),
        };

        let stream = provider.stream(request);
        futures::pin_mut!(stream);

        let mut calls = Vec::new();
        let mut completed_response_id: Option<String> = None;

        while let Some(event) = stream.next().await {
            match event.map_err(map_core_error)? {
                ProviderEvent::OutputTextDelta(delta) => {
                    assistant_text.push_str(&delta);
                    let _ = tx.send(Ok(SendSessionEvent::OutputTextDelta(delta)));
                }
                ProviderEvent::FunctionCallRequested(call) => {
                    previous_response_id = Some(call.response_id.clone());
                    calls.push(call);
                }
                ProviderEvent::Completed { response_id } => {
                    if response_id.is_some() {
                        completed_response_id = response_id.clone();
                    }
                    break;
                }
            }
        }

        if calls.is_empty() {
            if previous_response_id.is_none() {
                previous_response_id = completed_response_id;
            }
            break;
        }

        let mut outputs = Vec::new();
        for call in calls {
            outputs.push(
                handle_tool_call(
                    &turn,
                    &startup.runtime_context,
                    &soul_runtime,
                    &tools,
                    call,
                )
                .await?,
            );
        }

        function_call_outputs = Some(outputs);
    }

    if assistant_text.trim().is_empty() {
        return Err(SendSessionError::Internal(
            "provider completed without assistant output".to_string(),
        ));
    }

    let assistant_message = session_ledger
        .append_message(AppendSessionMessage {
            session_id: cmd.session_id,
            message_id: format!("msg_{}", Uuid::new_v4().simple()),
            actor_type: ActorType::Soul,
            actor_id: default_soul_id,
            content: text_content(&assistant_text),
            state: MessageState::Fixed,
        })
        .await
        .map_err(map_core_error)?;

    soul_runtime
        .append_message_ref(AppendMessageRef {
            soul_session_id: startup.soul_session_id,
            message_id: assistant_message.message.id.clone(),
        })
        .await
        .map_err(map_core_error)?;

    soul_runtime
        .complete_turn(CompleteTurn {
            turn_id: turn.id,
            last_seen_session_seq: assistant_message.relation.session_seq,
            provider_state: previous_response_id.map(|response_id| ProviderState {
                provider: "openai_compatible".to_string(),
                basis_soul_session_seq: assistant_message.relation.session_seq,
                opaque: serde_json::json!({ "response_id": response_id }),
                schema_version: Some("phase2".to_string()),
            }),
        })
        .await
        .map_err(map_core_error)?;

    let _ = tx.send(Ok(SendSessionEvent::Completed));
    Ok(())
}

async fn handle_tool_call(
    turn: &Turn,
    runtime_context: &ToolRuntimeContext,
    soul_runtime: &Arc<dyn SoulRuntimePort>,
    tools: &Arc<ToolExecutor>,
    call: ProviderFunctionCall,
) -> Result<santi_core::port::provider::FunctionCallOutput, SendSessionError> {
    soul_runtime
        .append_tool_call(AppendToolCall {
            tool_call_id: call.call_id.clone(),
            turn_id: turn.id.clone(),
            tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
        })
        .await
        .map_err(map_core_error)?;

    let dispatch_result = tools
        .dispatch(runtime_context, &call)
        .await
        .map_err(SendSessionError::Internal)?;

    soul_runtime
        .append_tool_result(AppendToolResult {
            tool_result_id: format!("tool_result_{}", Uuid::new_v4().simple()),
            tool_call_id: call.call_id.clone(),
            output: Some(dispatch_result.tool_output.clone()),
            error_text: None,
        })
        .await
        .map_err(map_core_error)?;

    Ok(dispatch_result.function_call_output)
}

fn text_content(text: &str) -> MessageContent {
    MessageContent {
        parts: vec![MessagePart::Text {
            text: text.to_string(),
        }],
    }
}

fn render_send_error(err: &SendSessionError) -> String {
    match err {
        SendSessionError::Busy => "session send busy".to_string(),
        SendSessionError::NotFound => "session not found".to_string(),
        SendSessionError::Internal(message) => message.clone(),
    }
}

fn map_core_error(err: Error) -> SendSessionError {
    match err {
        Error::NotFound { resource } if resource == "session" => SendSessionError::NotFound,
        Error::Busy { .. } => SendSessionError::Busy,
        Error::NotFound { resource } => SendSessionError::Internal(format!("{resource} not found")),
        Error::InvalidInput { message }
        | Error::Upstream { message }
        | Error::Internal { message } => SendSessionError::Internal(message),
    }
}

fn map_lock_error(err: LockError) -> SendSessionError {
    match err {
        LockError::Busy => SendSessionError::Busy,
        LockError::Lost => SendSessionError::Internal("session send lock lost".to_string()),
        LockError::Backend { message } => SendSessionError::Internal(message),
    }
}

#[allow(dead_code)]
fn _message_id(message: &SessionMessage) -> &str {
    &message.message.id
}
