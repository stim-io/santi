use serde::Serialize;
use utoipa::ToSchema;

use crate::model::{
    effect::SessionEffect,
    message::MessagePart,
    runtime::{Compact, SoulSession, ToolActivity},
    session::{Session, SessionMessage},
    soul::Soul,
};
use santi_runtime::session::watch::SessionWatchSnapshot;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionResponse {
    pub id: String,
    pub parent_session_id: Option<String>,
    pub fork_point: Option<i64>,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Deserialize, ToSchema)]
pub struct SessionMemoryRequest {
    pub text: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionMemoryResponse {
    pub id: String,
    pub memory: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SoulResponse {
    pub id: String,
    pub memory: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, serde::Deserialize, ToSchema)]
pub struct SoulMemoryRequest {
    pub text: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SoulMemoryResponse {
    pub id: String,
    pub memory: String,
    pub updated_at: String,
}

impl From<Session> for SessionResponse {
    fn from(value: Session) -> Self {
        Self {
            id: value.id,
            parent_session_id: value.parent_session_id,
            fork_point: value.fork_point,
            created_at: value.created_at,
        }
    }
}

impl From<SoulSession> for SessionMemoryResponse {
    fn from(value: SoulSession) -> Self {
        Self {
            id: value.id,
            memory: value.session_memory,
            updated_at: value.updated_at,
        }
    }
}

impl SessionMemoryResponse {
    pub fn new(id: String, memory: String, updated_at: String) -> Self {
        Self {
            id,
            memory,
            updated_at,
        }
    }
}

impl From<Soul> for SoulResponse {
    fn from(value: Soul) -> Self {
        Self {
            id: value.id,
            memory: value.memory,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<Soul> for SoulMemoryResponse {
    fn from(value: Soul) -> Self {
        Self {
            id: value.id,
            memory: value.memory,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionMessageResponse {
    pub id: String,
    pub actor_type: String,
    pub actor_id: String,
    pub session_seq: i64,
    pub content_text: String,
    pub state: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Deserialize, ToSchema)]
pub struct SessionSendRequest {
    pub content: Vec<SessionSendContentPart>,
}

#[derive(Clone, Debug, serde::Deserialize, ToSchema)]
pub struct SessionCompactRequest {
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionCompactResponse {
    pub id: String,
    pub turn_id: String,
    pub summary: String,
    pub start_session_seq: i64,
    pub end_session_seq: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionCompactsResponse {
    pub compacts: Vec<SessionCompactResponse>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionToolActivitiesResponse {
    pub tool_activities: Vec<SessionToolActivityResponse>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionToolActivityResponse {
    pub tool_call_id: String,
    pub tool_name: String,
    pub tool_call_seq: i64,
    pub tool_call_created_at: String,
    pub tool_result_id: Option<String>,
    pub tool_result_seq: Option<i64>,
    pub tool_result_created_at: Option<String>,
    pub result_state: String,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<u64>,
    pub stdout_chars: Option<u64>,
    pub stderr_chars: Option<u64>,
    pub output_summary: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum SessionSendContentPart {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionMessagesResponse {
    pub messages: Vec<SessionMessageResponse>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionEffectResponse {
    pub id: String,
    pub session_id: String,
    pub effect_type: String,
    pub idempotency_key: String,
    pub status: String,
    pub source_hook_id: String,
    pub source_turn_id: String,
    pub result_ref: Option<String>,
    pub error_text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionEffectsResponse {
    pub effects: Vec<SessionEffectResponse>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionWatchMessageSummaryResponse {
    pub id: String,
    pub actor_type: String,
    pub actor_id: String,
    pub session_seq: i64,
    pub content_text: String,
    pub state: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionWatchEffectSummaryResponse {
    pub id: String,
    pub effect_type: String,
    pub status: String,
    pub source_hook_id: String,
    pub result_ref: Option<String>,
    pub error_text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionWatchSnapshotResponse {
    pub session_id: String,
    pub latest_seq: i64,
    pub messages: Vec<SessionWatchMessageSummaryResponse>,
    pub effects: Vec<SessionWatchEffectSummaryResponse>,
}

impl From<SessionMessage> for SessionMessageResponse {
    fn from(value: SessionMessage) -> Self {
        Self {
            id: value.message.id,
            actor_type: format!("{:?}", value.message.actor_type).to_lowercase(),
            actor_id: value.message.actor_id,
            session_seq: value.relation.session_seq,
            content_text: value
                .message
                .content
                .parts
                .iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text } => Some(text.as_str()),
                    MessagePart::Image { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            state: format!("{:?}", value.message.state).to_lowercase(),
            created_at: value.message.created_at,
        }
    }
}

impl SessionMessagesResponse {
    pub fn from_messages(messages: Vec<SessionMessage>) -> Self {
        Self {
            messages: messages
                .into_iter()
                .map(SessionMessageResponse::from)
                .collect(),
        }
    }
}

impl From<SessionEffect> for SessionEffectResponse {
    fn from(value: SessionEffect) -> Self {
        Self {
            id: value.id,
            session_id: value.session_id,
            effect_type: value.effect_type,
            idempotency_key: value.idempotency_key,
            status: value.status,
            source_hook_id: value.source_hook_id,
            source_turn_id: value.source_turn_id,
            result_ref: value.result_ref,
            error_text: value.error_text,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl SessionEffectsResponse {
    pub fn from_effects(effects: Vec<SessionEffect>) -> Self {
        Self {
            effects: effects.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<SessionWatchSnapshot> for SessionWatchSnapshotResponse {
    fn from(value: SessionWatchSnapshot) -> Self {
        Self {
            session_id: value.session_id,
            latest_seq: value.latest_seq,
            messages: value
                .messages
                .into_iter()
                .map(|message| SessionWatchMessageSummaryResponse {
                    id: message.id,
                    actor_type: message.actor_type,
                    actor_id: message.actor_id,
                    session_seq: message.session_seq,
                    content_text: message.content_text,
                    state: message.state,
                    created_at: message.created_at,
                })
                .collect(),
            effects: value
                .effects
                .into_iter()
                .map(|effect| SessionWatchEffectSummaryResponse {
                    id: effect.id,
                    effect_type: effect.effect_type,
                    status: effect.status,
                    source_hook_id: effect.source_hook_id,
                    result_ref: effect.result_ref,
                    error_text: effect.error_text,
                    created_at: effect.created_at,
                    updated_at: effect.updated_at,
                })
                .collect(),
        }
    }
}

impl From<Compact> for SessionCompactResponse {
    fn from(value: Compact) -> Self {
        Self {
            id: value.id,
            turn_id: value.turn_id,
            summary: value.summary,
            start_session_seq: value.start_session_seq,
            end_session_seq: value.end_session_seq,
            created_at: value.created_at,
        }
    }
}

impl SessionCompactsResponse {
    pub fn from_compacts(compacts: Vec<Compact>) -> Self {
        Self {
            compacts: compacts.into_iter().map(Into::into).collect(),
        }
    }
}

impl SessionToolActivitiesResponse {
    pub fn from_tool_activities(tool_activities: Vec<ToolActivity>) -> Self {
        Self {
            tool_activities: tool_activities
                .into_iter()
                .map(SessionToolActivityResponse::from)
                .collect(),
        }
    }
}

impl From<ToolActivity> for SessionToolActivityResponse {
    fn from(value: ToolActivity) -> Self {
        let result = value.tool_result;
        let output = result.as_ref().and_then(|result| result.output.as_ref());

        Self {
            tool_call_id: value.tool_call.id,
            tool_name: value.tool_call.tool_name,
            tool_call_seq: value.tool_call_seq,
            tool_call_created_at: value.tool_call.created_at,
            tool_result_id: result.as_ref().map(|result| result.id.clone()),
            tool_result_seq: value.tool_result_seq,
            tool_result_created_at: result.as_ref().map(|result| result.created_at.clone()),
            result_state: result_state(result.as_ref(), output),
            exit_code: bash_exit_code(output),
            duration_ms: output
                .and_then(|output| output.get("duration_ms"))
                .and_then(serde_json::Value::as_u64),
            stdout_chars: bash_stream_chars(output, "stdout"),
            stderr_chars: bash_stream_chars(output, "stderr"),
            output_summary: output.map(output_summary),
        }
    }
}

fn result_state(
    result: Option<&santi_core::model::runtime::ToolResult>,
    output: Option<&serde_json::Value>,
) -> String {
    if result.is_none() {
        return "pending".to_string();
    }
    if result
        .and_then(|result| result.error_text.as_ref())
        .is_some()
        || output
            .and_then(|output| output.get("ok"))
            .and_then(serde_json::Value::as_bool)
            == Some(false)
    {
        return "tool-error".to_string();
    }

    "completed".to_string()
}

fn bash_exit_code(output: Option<&serde_json::Value>) -> Option<i64> {
    output
        .and_then(|output| output.pointer("/bash_result/exit_code"))
        .and_then(serde_json::Value::as_i64)
}

fn bash_stream_chars(output: Option<&serde_json::Value>, stream: &str) -> Option<u64> {
    output
        .and_then(|output| output.pointer(&format!("/bash_result/{stream}")))
        .and_then(serde_json::Value::as_str)
        .map(|value| value.chars().count() as u64)
}

fn output_summary(output: &serde_json::Value) -> String {
    if let Some(exit_code) = bash_exit_code(Some(output)) {
        return format!(
            "bash exit {exit_code}; stdout {} chars; stderr {} chars",
            bash_stream_chars(Some(output), "stdout").unwrap_or(0),
            bash_stream_chars(Some(output), "stderr").unwrap_or(0)
        );
    }

    if let Some(ok) = output.get("ok").and_then(serde_json::Value::as_bool) {
        return format!("ok {ok}");
    }

    match output {
        serde_json::Value::Object(map) => {
            let keys = map.keys().take(4).cloned().collect::<Vec<_>>().join(",");
            format!("json object keys: {keys}")
        }
        serde_json::Value::Array(items) => format!("json array items: {}", items.len()),
        _ => "json scalar".to_string(),
    }
}

#[derive(Clone, Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct ForkRequest {
    pub fork_point: i64,
    pub request_id: String,
}

#[derive(Clone, Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ForkResponse {
    pub new_session_id: String,
    pub parent_session_id: String,
    pub fork_point: i64,
}
