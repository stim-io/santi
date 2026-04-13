use std::sync::Arc;

use futures::StreamExt;
use santi_core::{
    model::{
        message::{ActorType, MessageContent, MessagePart, MessageState},
        runtime::{ProviderState, Turn, TurnTriggerType},
        session::{Session, SessionMessage},
    },
    port::{
        lock::LockGuard,
        provider::{FunctionCallOutput, Provider, ProviderEvent, ProviderFunctionCall, ProviderRequest},
        session_ledger::{AppendSessionMessage, SessionLedgerPort},
        soul_runtime::{
            AcquireSoulSession, AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn,
            FailTurn, SoulRuntimePort, StartTurn,
        },
    },
    provider::ProviderInputMessage,
    service::session::kernel::runtime_prompt::{build_runtime_prompt, RuntimePromptSource},
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    hooks::TurnCompletedHookInput,
    runtime::{context::ToolRuntimeContext, prompt::render_runtime_instructions, tools::ToolExecutor},
    session::{
        hook_runtime::HookRuntime,
        watch::{
            SessionWatchActivityChanged, SessionWatchActivityKind, SessionWatchActivityState,
            SessionWatchEvent, SessionWatchHub, SessionWatchMessageChange,
            SessionWatchMessageChanged, SessionWatchState, SessionWatchStateChanged,
        },
    },
};

use super::{
    assembly::{assembly_to_provider_input, build_assembly_items}, map_core_error, map_lock_error,
    render_send_error, SendSessionError, SendSessionEvent, TurnExecutionRequest, TurnInput,
};

pub(super) struct TurnRunOutput {
    pub(super) turn: Turn,
    pub(super) session: Session,
    pub(super) soul_session_id: String,
    pub(super) assistant_message: SessionMessage,
}

#[derive(Clone)]
pub(super) struct StartupContext {
    pub(super) session: Session,
    pub(super) provider_input: Vec<ProviderInputMessage>,
    pub(super) instructions: Option<String>,
    pub(super) soul_session_id: String,
    pub(super) trigger_type: TurnTriggerType,
    pub(super) input_through_session_seq: i64,
    pub(super) trigger_message_id: String,
    pub(super) runtime_context: ToolRuntimeContext,
}

pub(super) async fn run_turn_startup(
    default_soul_id: &str,
    request: &TurnExecutionRequest,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    tools: Arc<ToolExecutor>,
) -> Result<StartupContext, SendSessionError> {
    let soul_session = soul_runtime
        .acquire_soul_session(AcquireSoulSession {
            soul_id: default_soul_id.to_string(),
            session_id: request.session_id.clone(),
        })
        .await
        .map_err(map_core_error)?;

    let session = session_ledger
        .get_session(&request.session_id)
        .await
        .map_err(map_core_error)?
        .ok_or(SendSessionError::NotFound)?;

    let trigger_message = session_ledger
        .append_message(AppendSessionMessage {
            session_id: session.id.clone(),
            message_id: format!("msg_{}", Uuid::new_v4().simple()),
            actor_type: match &request.input {
                TurnInput::UserText { .. } => ActorType::Account,
                TurnInput::SystemSeed { .. } => ActorType::System,
            },
            actor_id: match &request.input {
                TurnInput::UserText { .. } => "account_local".to_string(),
                TurnInput::SystemSeed { actor_id, .. } => actor_id.clone(),
            },
            content: text_content(match &request.input {
                TurnInput::UserText { text } | TurnInput::SystemSeed { text, .. } => text,
            }),
            state: MessageState::Fixed,
        })
        .await
        .map_err(map_core_error)?;

    soul_runtime
        .append_message_ref(AppendMessageRef {
            soul_session_id: soul_session.id.clone(),
            message_id: trigger_message.message.id.clone(),
        })
        .await
        .map_err(map_core_error)?;

    let assembly =
        build_assembly_items(session_ledger.clone(), &session.id, &soul_session.id).await?;
    let provider_input = assembly_to_provider_input(&assembly);
    let runtime_context = tools.build_context(&request.session_id, &soul_session.soul_id);
    let core_prompt = build_runtime_prompt(RuntimePromptSource {
        session_id: Some(request.session_id.clone()),
        soul_id: Some(soul_session.soul_id.clone()),
        soul_memory: None,
        session_memory: Some(soul_session.session_memory.clone()),
        request_instructions: None,
    });
    let instructions = render_runtime_instructions(&core_prompt, &runtime_context, &tools);

    Ok(StartupContext {
        session,
        provider_input,
        instructions,
        soul_session_id: soul_session.id,
        trigger_type: match &request.input {
            TurnInput::UserText { .. } => TurnTriggerType::SessionSend,
            TurnInput::SystemSeed { .. } => TurnTriggerType::System,
        },
        input_through_session_seq: trigger_message.relation.session_seq,
        trigger_message_id: trigger_message.message.id,
        runtime_context,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_turn_worker(
    default_soul_id: String,
    request: TurnExecutionRequest,
    model: String,
    provider: Arc<dyn Provider>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    tools: Arc<ToolExecutor>,
    watch: Arc<SessionWatchHub>,
    hooks: Option<Arc<HookRuntime>>,
    startup: StartupContext,
    tx: Option<mpsc::UnboundedSender<Result<super::SendSessionEvent, SendSessionError>>>,
    guard: Box<dyn LockGuard + Send>,
) -> Result<Turn, SendSessionError> {
    let turn_id = format!("turn_{}", Uuid::new_v4().simple());
    let started_turn = soul_runtime
        .start_turn(StartTurn {
            turn_id: turn_id.clone(),
            soul_session_id: startup.soul_session_id.clone(),
            trigger_type: startup.trigger_type.clone(),
            trigger_ref: Some(startup.trigger_message_id.clone()),
            input_through_session_seq: startup.input_through_session_seq,
        })
        .await
        .map_err(map_core_error)?;

    let run_result = run_turn_body(
        default_soul_id,
        request.clone(),
        model,
        provider,
        session_ledger.clone(),
        soul_runtime.clone(),
        tools,
        startup,
        started_turn,
        tx.clone(),
        watch,
    )
    .await;

    fail_turn_on_error(&soul_runtime, &turn_id, &run_result).await;

    let output = finish_turn_run(run_result, guard).await?;

    run_turn_completed_hooks(&request, hooks, &output, session_ledger, soul_runtime).await?;

    emit_turn_completed_event(&request, tx.as_ref());

    Ok(output.turn)
}

pub(super) fn publish_turn_started(watch: &SessionWatchHub, session_id: &str) {
    watch.publish(
        session_id,
        SessionWatchEvent::StateChanged(SessionWatchStateChanged {
            session_id: session_id.to_string(),
            state: SessionWatchState::Running,
        }),
    );
    watch.publish(
        session_id,
        SessionWatchEvent::ActivityChanged(SessionWatchActivityChanged {
            session_id: session_id.to_string(),
            activity: SessionWatchActivityKind::Send,
            state: SessionWatchActivityState::Started,
            label: None,
        }),
    );
}

pub(super) async fn release_guard_on_error<T>(
    guard: Box<dyn LockGuard + Send>,
    err: SendSessionError,
) -> Result<T, SendSessionError> {
    match guard.release().await.map_err(map_lock_error) {
        Ok(()) => Err(err),
        Err(release_err) => Err(release_err),
    }
}

async fn fail_turn_on_error(
    soul_runtime: &Arc<dyn SoulRuntimePort>,
    turn_id: &str,
    run_result: &Result<TurnRunOutput, SendSessionError>,
) {
    if let Err(err) = run_result {
        let _ = soul_runtime
            .fail_turn(FailTurn {
                turn_id: turn_id.to_string(),
                error_text: render_send_error(err),
            })
            .await;
    }
}

async fn finish_turn_run(
    run_result: Result<TurnRunOutput, SendSessionError>,
    guard: Box<dyn LockGuard + Send>,
) -> Result<TurnRunOutput, SendSessionError> {
    let release_result = guard.release().await.map_err(map_lock_error);
    let output = match run_result {
        Ok(output) => output,
        Err(err) => return Err(err),
    };

    release_result?;
    Ok(output)
}

async fn run_turn_completed_hooks(
    request: &TurnExecutionRequest,
    hooks: Option<Arc<HookRuntime>>,
    output: &TurnRunOutput,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
) -> Result<(), SendSessionError> {
    if !request.run_hooks {
        return Ok(());
    }

    let Some(hooks) = hooks else {
        return Ok(());
    };

    let Some(soul_session) = soul_runtime
        .get_soul_session(&output.soul_session_id)
        .await
        .map_err(map_core_error)?
    else {
        return Ok(());
    };

    let assembly =
        build_assembly_items(session_ledger, &output.session.id, &output.soul_session_id).await?;

    let _ = hooks
        .run_turn_completed(TurnCompletedHookInput {
            turn: &output.turn,
            session: &output.session,
            soul_session: &soul_session,
            assistant_message: Some(&output.assistant_message),
            assembly_tail: &assembly,
        })
        .await;

    Ok(())
}

fn emit_turn_completed_event(
    request: &TurnExecutionRequest,
    tx: Option<&mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>>,
) {
    if !request.emit_events {
        return;
    }

    if let Some(tx) = tx {
        let _ = tx.send(Ok(SendSessionEvent::Completed));
    }
}

async fn run_turn_body(
    default_soul_id: String,
    request: TurnExecutionRequest,
    model: String,
    provider: Arc<dyn Provider>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    tools: Arc<ToolExecutor>,
    startup: StartupContext,
    turn: Turn,
    tx: Option<mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>>,
    watch: Arc<SessionWatchHub>,
) -> Result<TurnRunOutput, SendSessionError> {
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
                    if let Some(tx) = &tx {
                        let _ = tx.send(Ok(SendSessionEvent::OutputTextDelta(delta)));
                    }
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
                handle_tool_call(&turn, &startup.runtime_context, &soul_runtime, &tools, call)
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
            session_id: request.session_id.clone(),
            message_id: format!("msg_{}", Uuid::new_v4().simple()),
            actor_type: ActorType::Soul,
            actor_id: default_soul_id,
            content: text_content(&assistant_text),
            state: MessageState::Fixed,
        })
        .await
        .map_err(map_core_error)?;

    watch.publish(
        &request.session_id,
        SessionWatchEvent::MessageChanged(SessionWatchMessageChanged {
            session_id: request.session_id.clone(),
            message_id: assistant_message.message.id.clone(),
            session_seq: assistant_message.relation.session_seq,
            change: SessionWatchMessageChange::Finalized,
            actor_type: format!("{:?}", assistant_message.message.actor_type).to_lowercase(),
        }),
    );

    let assistant_entry = soul_runtime
        .append_message_ref(AppendMessageRef {
            soul_session_id: startup.soul_session_id.clone(),
            message_id: assistant_message.message.id.clone(),
        })
        .await
        .map_err(map_core_error)?;

    soul_runtime
        .complete_turn(CompleteTurn {
            turn_id: turn.id.clone(),
            last_seen_session_seq: assistant_message.relation.session_seq,
            provider_state: previous_response_id.map(|response_id| {
                let basis_soul_session_seq = assistant_entry.entry.soul_session_seq;

                ProviderState {
                    provider: "openai_compatible".to_string(),
                    basis_soul_session_seq,
                    opaque: serde_json::json!({ "response_id": response_id }),
                    schema_version: Some("phase2".to_string()),
                }
            }),
        })
        .await
        .map_err(map_core_error)?;

    watch.publish(
        &request.session_id,
        SessionWatchEvent::ActivityChanged(SessionWatchActivityChanged {
            session_id: request.session_id.clone(),
            activity: SessionWatchActivityKind::Send,
            state: SessionWatchActivityState::Completed,
            label: None,
        }),
    );
    watch.publish(
        &request.session_id,
        SessionWatchEvent::StateChanged(SessionWatchStateChanged {
            session_id: request.session_id.clone(),
            state: SessionWatchState::Completed,
        }),
    );

    Ok(TurnRunOutput {
        turn,
        session: startup.session,
        soul_session_id: startup.soul_session_id,
        assistant_message,
    })
}

async fn handle_tool_call(
    turn: &Turn,
    runtime_context: &ToolRuntimeContext,
    soul_runtime: &Arc<dyn SoulRuntimePort>,
    tools: &Arc<ToolExecutor>,
    call: ProviderFunctionCall,
) -> Result<FunctionCallOutput, SendSessionError> {
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
