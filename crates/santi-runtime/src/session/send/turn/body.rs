use std::sync::Arc;

use futures::StreamExt;
use santi_core::{
    model::{
        message::{ActorType, MessageState},
        runtime::{ProviderState, Turn},
    },
    port::{
        provider::{FunctionCallOutput, ProviderEvent, ProviderFunctionCall, ProviderRequest},
        session_ledger::AppendSessionMessage,
        soul_runtime::{
            AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn, SoulRuntimePort,
        },
    },
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    runtime::{context::ToolRuntimeContext, tools::ToolExecutor},
    session::watch::{
        SessionWatchActivityChanged, SessionWatchActivityKind, SessionWatchActivityState,
        SessionWatchEvent, SessionWatchMessageChange, SessionWatchMessageChanged,
        SessionWatchState, SessionWatchStateChanged,
    },
};

use super::super::{map_core_error, SendSessionError, SendSessionEvent};
use super::{text_content, StartupContext, TurnRunDeps, TurnRunOutput};

pub(super) async fn run_turn_body(
    deps: TurnRunDeps,
    request: super::super::TurnExecutionRequest,
    startup: StartupContext,
    turn: Turn,
    tx: Option<mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>>,
) -> Result<TurnRunOutput, SendSessionError> {
    let mut assistant_text = String::new();
    let mut previous_response_id: Option<String> = None;
    let mut function_call_outputs = None;

    loop {
        let request = ProviderRequest {
            model: deps.model.clone(),
            instructions: startup.instructions.clone(),
            input: if previous_response_id.is_some() {
                Vec::new()
            } else {
                startup.provider_input.clone()
            },
            tools: Some(deps.tools.provider_tools()),
            previous_response_id: previous_response_id.clone(),
            function_call_outputs: function_call_outputs.take(),
        };

        let stream = deps.provider.stream(request);
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
                handle_tool_call(
                    &turn,
                    &startup.runtime_context,
                    &deps.soul_runtime,
                    &deps.tools,
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

    let assistant_message = deps
        .session_ledger
        .append_message(AppendSessionMessage {
            session_id: request.session_id.clone(),
            message_id: format!("msg_{}", Uuid::new_v4().simple()),
            actor_type: ActorType::Soul,
            actor_id: deps.default_soul_id,
            content: text_content(&assistant_text),
            state: MessageState::Fixed,
        })
        .await
        .map_err(map_core_error)?;

    deps.watch.publish(
        &request.session_id,
        SessionWatchEvent::MessageChanged(SessionWatchMessageChanged {
            session_id: request.session_id.clone(),
            message_id: assistant_message.message.id.clone(),
            session_seq: assistant_message.relation.session_seq,
            change: SessionWatchMessageChange::Finalized,
            actor_type: format!("{:?}", assistant_message.message.actor_type).to_lowercase(),
        }),
    );

    let assistant_entry = deps
        .soul_runtime
        .append_message_ref(AppendMessageRef {
            soul_session_id: startup.soul_session_id.clone(),
            message_id: assistant_message.message.id.clone(),
        })
        .await
        .map_err(map_core_error)?;

    let completed_turn = deps
        .soul_runtime
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

    deps.watch.publish(
        &request.session_id,
        SessionWatchEvent::ActivityChanged(SessionWatchActivityChanged {
            session_id: request.session_id.clone(),
            activity: SessionWatchActivityKind::Send,
            state: SessionWatchActivityState::Completed,
            label: None,
        }),
    );
    deps.watch.publish(
        &request.session_id,
        SessionWatchEvent::StateChanged(SessionWatchStateChanged {
            session_id: request.session_id.clone(),
            state: SessionWatchState::Completed,
        }),
    );

    Ok(TurnRunOutput {
        turn: completed_turn,
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
