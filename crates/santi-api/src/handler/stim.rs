use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response, Sse},
    Json,
};
use futures::StreamExt;
use santi_core::{
    error::Error as CoreError,
    model::message::{
        ActorType, MessageContent as CoreMessageContent, MessageEventPayload, MessageInsertItem,
        MessagePart, MessagePartPatch, MessageState as CoreMessageState,
    },
    port::session_ledger::{AppendSessionMessage, ApplyMessageEvent},
};
use stim_proto::{
    AcknowledgementResult, ContentPart, MessageEnvelope, MessageOperation, MessageState,
    MutationPayload, ProtocolAcknowledgement, ProtocolSubmission, ReplySnapshot,
};
use std::convert::Infallible;

use crate::{handler::session_events::done_event, schema::common::ErrorResponse, state::AppState, surface::ApiError};

#[utoipa::path(
    post,
    path = "/api/v1/stim/envelopes",
    tag = "stim",
    request_body(content = MessageEnvelope),
    responses(
        (status = 200, description = "Stim protocol submission result", body = ProtocolSubmission)
    )
)]
pub async fn accept_envelope(
    State(state): State<AppState>,
    Json(envelope): Json<MessageEnvelope>,
) -> impl IntoResponse {
    match apply_stim_envelope(&state, envelope).await {
        Ok(submission) => (StatusCode::OK, Json(submission)).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/stim/replies/{reply_id}",
    tag = "stim",
    params(
        ("reply_id" = String, Path, description = "Reply id")
    ),
    responses(
        (status = 200, description = "Stim protocol reply snapshot", body = ReplySnapshot),
        (status = 404, description = "Reply not found", body = ErrorResponse)
    )
)]
pub async fn get_reply_snapshot(
    State(state): State<AppState>,
    Path(reply_id): Path<String>,
) -> impl IntoResponse {
    match state.protocol_replies().snapshot(&reply_id) {
        Some(snapshot) => (StatusCode::OK, Json(snapshot)).into_response(),
        None => ApiError::NotFound("reply not found".into())
            .into_error_response()
            .into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/stim/replies/{reply_id}/events",
    tag = "stim",
    params(
        ("reply_id" = String, Path, description = "Reply id")
    ),
    responses(
        (status = 200, description = "Stim protocol reply event stream"),
        (status = 404, description = "Reply not found", body = ErrorResponse)
    )
)]
pub async fn stream_reply_events(
    State(state): State<AppState>,
    Path(reply_id): Path<String>,
) -> Response {
    let Some(subscription) = state.protocol_replies().subscribe(&reply_id) else {
        return ApiError::NotFound("reply not found".into())
            .into_error_response()
            .into_response();
    };

    let stream = async_stream::stream! {
        for event in subscription.history {
            yield Ok::<_, Infallible>(encode_reply_event(event));
        }

        if subscription.terminal {
            yield Ok::<_, Infallible>(done_event());
            return;
        }

        let mut receiver = subscription.receiver;
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let terminal = matches!(event.event, stim_proto::ReplyEventKind::Completed | stim_proto::ReplyEventKind::Failed { .. });
                    yield Ok::<_, Infallible>(encode_reply_event(event));
                    if terminal {
                        yield Ok::<_, Infallible>(done_event());
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    yield Ok::<_, Infallible>(done_event());
                    break;
                }
            }
        }
    };

    Sse::new(stream).into_response()
}

async fn apply_stim_envelope(
    state: &AppState,
    envelope: MessageEnvelope,
) -> Result<ProtocolSubmission, ApiError> {
    if envelope.state != MessageState::Pending && envelope.state != MessageState::Fixed {
        return Ok(ProtocolSubmission {
            acknowledgement: rejected_ack(
                &envelope,
                AcknowledgementResult::InvalidStateTransition,
                Some("unsupported message state".into()),
            ),
            reply: None,
        });
    }

    match state.session_api().get_session(&envelope.conversation_id).await {
        Ok(_) => {}
        Err(ApiError::NotFound(_)) => {
            if envelope.session_bootstrap.is_none() {
                return Ok(ProtocolSubmission {
                    acknowledgement: rejected_ack(
                        &envelope,
                        AcknowledgementResult::UnknownConversation,
                        Some("session bootstrap required for unknown conversation".into()),
                    ),
                    reply: None,
                });
            }

            state
                .session_api()
                .create_session_with_id(&envelope.conversation_id)
                .await?;
        }
        Err(err) => return Err(err),
    }

    let ack = match envelope.operation {
        MessageOperation::Create => apply_create(state, &envelope).await?,
        MessageOperation::Patch => apply_patch(state, &envelope).await?,
        MessageOperation::Insert => apply_insert(state, &envelope).await?,
        MessageOperation::Remove => apply_remove(state, &envelope).await?,
        MessageOperation::Fix => apply_fix(state, &envelope).await?,
    };

    let reply = if matches!(envelope.operation, MessageOperation::Fix)
        && matches!(ack.ack_result, AcknowledgementResult::Applied)
    {
        start_protocol_reply(state, &envelope).await?
    } else {
        None
    };

    Ok(ProtocolSubmission {
        acknowledgement: ack,
        reply,
    })
}

async fn apply_create(
    state: &AppState,
    envelope: &MessageEnvelope,
) -> Result<ProtocolAcknowledgement, ApiError> {
    let content = map_content(&envelope.payload)?;

    let message = state
        .session_ledger()
        .append_message(AppendSessionMessage {
            session_id: envelope.conversation_id.clone(),
            message_id: envelope.message_id.clone(),
            actor_type: ActorType::Account,
            actor_id: envelope.sender_endpoint_id.clone(),
            content,
            state: map_state(&envelope.state),
        })
        .await
        .map_err(map_core_error)?;

    Ok(ProtocolAcknowledgement {
        ack_envelope_id: format!("ack-{}", envelope.envelope_id),
        ack_message_id: envelope.message_id.clone(),
        ack_version: message.message.version as u64,
        ack_result: AcknowledgementResult::Applied,
        detail: Some(format!(
            "santi stored create for session {} message {} version {}",
            envelope.conversation_id, envelope.message_id, message.message.version
        )),
    })
}

async fn apply_patch(
    state: &AppState,
    envelope: &MessageEnvelope,
) -> Result<ProtocolAcknowledgement, ApiError> {
    let patches = match &envelope.payload {
        MutationPayload::Patch { patches } => patches
            .iter()
            .map(|patch| MessagePartPatch {
                index: patch.index as i64,
                merge: patch.merge.clone(),
            })
            .collect(),
        _ => return Err(ApiError::BadRequest("patch payload required".into())),
    };

    apply_event(
        state,
        envelope,
        MessageEventPayload::Patch { patches },
        "patch",
    )
    .await
}

async fn apply_insert(
    state: &AppState,
    envelope: &MessageEnvelope,
) -> Result<ProtocolAcknowledgement, ApiError> {
    let items = match &envelope.payload {
        MutationPayload::Insert { items } => items
            .iter()
            .map(|item| {
                Ok(MessageInsertItem {
                    index: item.index as i64,
                    part: map_part(&item.part)?,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?,
        _ => return Err(ApiError::BadRequest("insert payload required".into())),
    };

    apply_event(
        state,
        envelope,
        MessageEventPayload::Insert { items },
        "insert",
    )
    .await
}

async fn apply_remove(
    state: &AppState,
    envelope: &MessageEnvelope,
) -> Result<ProtocolAcknowledgement, ApiError> {
    let indexes = match &envelope.payload {
        MutationPayload::Remove { indexes } => indexes.iter().map(|index| *index as i64).collect(),
        _ => return Err(ApiError::BadRequest("remove payload required".into())),
    };

    apply_event(
        state,
        envelope,
        MessageEventPayload::Remove { indexes },
        "remove",
    )
    .await
}

async fn apply_fix(
    state: &AppState,
    envelope: &MessageEnvelope,
) -> Result<ProtocolAcknowledgement, ApiError> {
    apply_event(state, envelope, MessageEventPayload::Fix, "fix").await
}

async fn apply_event(
    state: &AppState,
    envelope: &MessageEnvelope,
    payload: MessageEventPayload,
    action: &str,
) -> Result<ProtocolAcknowledgement, ApiError> {
    let message = state
        .session_ledger()
        .apply_message_event(ApplyMessageEvent {
            session_id: envelope.conversation_id.clone(),
            message_id: envelope.message_id.clone(),
            event_id: envelope.envelope_id.clone(),
            actor_type: ActorType::Account,
            actor_id: envelope.sender_endpoint_id.clone(),
            base_version: envelope.base_version.unwrap_or_default() as i64,
            payload,
        })
        .await
        .map_err(map_core_error)?;

    Ok(ProtocolAcknowledgement {
        ack_envelope_id: format!("ack-{}", envelope.envelope_id),
        ack_message_id: envelope.message_id.clone(),
        ack_version: message.message.version as u64,
        ack_result: AcknowledgementResult::Applied,
        detail: Some(format!(
            "santi applied {action} for session {} message {} version {} state {:?}",
            envelope.conversation_id,
            envelope.message_id,
            message.message.version,
            message.message.state
        )),
    })
}

fn map_state(state: &MessageState) -> CoreMessageState {
    match state {
        MessageState::Pending => CoreMessageState::Pending,
        MessageState::Fixed => CoreMessageState::Fixed,
    }
}

fn map_content(payload: &MutationPayload) -> Result<CoreMessageContent, ApiError> {
    match payload {
        MutationPayload::Create { content } => Ok(CoreMessageContent {
            parts: content
                .parts
                .iter()
                .map(map_part)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        _ => Err(ApiError::BadRequest("create payload required".into())),
    }
}

fn map_part(part: &ContentPart) -> Result<MessagePart, ApiError> {
    match part {
        ContentPart::Text(text) => Ok(MessagePart::Text {
            text: text.text.clone(),
        }),
        _ => Err(ApiError::Unsupported(
            "santi stim-proto participation currently supports text content only".into(),
        )),
    }
}

fn map_core_error(error: CoreError) -> ApiError {
    match error {
        CoreError::NotFound { resource } => ApiError::NotFound(resource.into()),
        CoreError::Busy { resource } => ApiError::Conflict(resource.into()),
        CoreError::InvalidInput { message } => ApiError::Validation(message),
        CoreError::Upstream { message } | CoreError::Internal { message } => ApiError::Internal(message),
    }
}

fn rejected_ack(
    envelope: &MessageEnvelope,
    ack_result: AcknowledgementResult,
    detail: Option<String>,
) -> ProtocolAcknowledgement {
    ProtocolAcknowledgement {
        ack_envelope_id: format!("ack-{}", envelope.envelope_id),
        ack_message_id: envelope.message_id.clone(),
        ack_version: envelope.new_version,
        ack_result,
        detail,
    }
}

async fn start_protocol_reply(
    state: &AppState,
    envelope: &MessageEnvelope,
) -> Result<Option<stim_proto::ReplyHandle>, ApiError> {
    let user_content = message_text_for_reply(state, envelope).await?;
    let handle = state
        .protocol_replies()
        .create_reply(envelope.conversation_id.clone(), envelope.message_id.clone());

    let reply_id = handle.reply_id.clone();
    let session_id = envelope.conversation_id.clone();
    let session_api = state.session_api();
    let protocol_replies = state.protocol_replies();

    tokio::spawn(async move {
        match session_api.send_session(&session_id, user_content).await {
            Ok(mut stream) => {
                while let Some(event) = stream.next().await {
                    match event {
                        Ok(crate::schema::session_events::SessionStreamEvent::OutputTextDelta(delta)) => {
                            protocol_replies.emit_text_delta(&reply_id, delta);
                        }
                        Ok(crate::schema::session_events::SessionStreamEvent::Completed) => {
                            protocol_replies.complete(&reply_id);
                            return;
                        }
                        Err(err) => {
                            protocol_replies.fail(&reply_id, "reply_runtime_failed", format!("{err:?}"));
                            return;
                        }
                    }
                }

                protocol_replies.fail(
                    &reply_id,
                    "reply_stream_incomplete",
                    "reply stream ended without completion event".into(),
                );
            }
            Err(err) => {
                protocol_replies.fail(&reply_id, "reply_start_failed", format!("{err:?}"));
            }
        }
    });

    Ok(Some(handle))
}

async fn message_text_for_reply(state: &AppState, envelope: &MessageEnvelope) -> Result<String, ApiError> {
    let messages = state
        .session_api()
        .list_session_messages(&envelope.conversation_id)
        .await?;

    let text = messages
        .into_iter()
        .find(|message| message.message.id == envelope.message_id)
        .map(|message| {
            message
                .message
                .content
                .parts
                .into_iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| ApiError::Internal("protocol reply text not found for fixed message".into()))?;

    Ok(text)
}

fn encode_reply_event(event: stim_proto::ReplyEvent) -> axum::response::sse::Event {
    axum::response::sse::Event::default().data(
        serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string()),
    )
}
