use serde_json::{json, Value};
use sqlx::Row;

use santi_core::{
    error::{Error, Result},
    model::{
        message::{ActorType, Message, MessageContent, MessagePart, MessageState},
        runtime::{
            Compact, ProviderState, SoulSession, ToolCall, ToolResult, Turn, TurnStatus,
            TurnTriggerType,
        },
        session::{SessionMessage, SessionMessageRef},
    },
};

pub(super) fn map_soul_session_row(row: sqlx::sqlite::SqliteRow) -> Result<SoulSession> {
    let provider_state = row
        .try_get::<Option<String>, _>("provider_state")
        .map_err(|err| Error::Internal {
            message: format!("standalone provider_state decode failed: {err}"),
        })?
        .map(|raw| serde_json::from_str::<Value>(&raw))
        .transpose()
        .map_err(|err| Error::Internal {
            message: format!("parse standalone provider_state failed: {err}"),
        })?
        .map(decode_provider_state)
        .transpose()?;

    Ok(SoulSession {
        id: row.get("id"),
        soul_id: row.get("soul_id"),
        session_id: row.get("session_id"),
        session_memory: row.get("session_memory"),
        provider_state,
        next_seq: row.get("next_seq"),
        last_seen_session_seq: row.get("last_seen_session_seq"),
        parent_soul_session_id: row.try_get("parent_soul_session_id").ok(),
        fork_point: row.try_get("fork_point").ok(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(super) fn map_turn_row(row: sqlx::sqlite::SqliteRow) -> Result<Turn> {
    Ok(Turn {
        id: row.get("id"),
        soul_session_id: row.get("soul_session_id"),
        trigger_type: match row.get::<String, _>("trigger_type").as_str() {
            "session_send" => TurnTriggerType::SessionSend,
            _ => TurnTriggerType::System,
        },
        trigger_ref: row.try_get("trigger_ref").ok(),
        input_through_session_seq: row.get("input_through_session_seq"),
        base_soul_session_seq: row.get("base_soul_session_seq"),
        end_soul_session_seq: row.try_get("end_soul_session_seq").ok(),
        status: match row.get::<String, _>("status").as_str() {
            "running" => TurnStatus::Running,
            "completed" => TurnStatus::Completed,
            _ => TurnStatus::Failed,
        },
        error_text: row.try_get("error_text").ok(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        finished_at: row.try_get("finished_at").ok(),
    })
}

pub(super) fn map_session_message_row(row: sqlx::sqlite::SqliteRow) -> Result<SessionMessage> {
    Ok(SessionMessage {
        relation: SessionMessageRef {
            message_id: row.get("message_id"),
            session_id: row.get("session_id"),
            session_seq: row.get("session_seq"),
            created_at: row.get("message_created_at"),
        },
        message: Message {
            id: row.get("message_id"),
            actor_type: match row.get::<String, _>("actor_type").as_str() {
                "soul" => ActorType::Soul,
                "system" => ActorType::System,
                _ => ActorType::Account,
            },
            actor_id: row.get("actor_id"),
            content: MessageContent {
                parts: vec![MessagePart::Text {
                    text: row.get("content_text"),
                }],
            },
            state: match row.get::<String, _>("state").as_str() {
                "fixed" => MessageState::Fixed,
                _ => MessageState::Pending,
            },
            created_at: row.get("message_created_at"),
            updated_at: row.get("message_created_at"),
            deleted_at: None,
            version: 1,
        },
    })
}

pub(super) fn encode_provider_state(state: &ProviderState) -> Value {
    json!({
        "provider": state.provider,
        "basis_soul_session_seq": state.basis_soul_session_seq,
        "opaque": state.opaque,
        "schema_version": state.schema_version,
    })
}

pub(super) fn decode_provider_state(value: Value) -> Result<ProviderState> {
    let obj = value.as_object().ok_or(Error::Internal {
        message: "provider_state must be an object".to_string(),
    })?;

    let provider = obj
        .get("provider")
        .and_then(Value::as_str)
        .ok_or(Error::Internal {
            message: "provider_state.provider missing".to_string(),
        })?
        .to_string();
    let basis_soul_session_seq = obj
        .get("basis_soul_session_seq")
        .and_then(Value::as_i64)
        .ok_or(Error::Internal {
            message: "provider_state.basis_soul_session_seq missing".to_string(),
        })?;
    let opaque = obj.get("opaque").cloned().ok_or(Error::Internal {
        message: "provider_state.opaque missing".to_string(),
    })?;
    let schema_version = obj
        .get("schema_version")
        .and_then(|value| value.as_str().map(ToString::to_string));

    Ok(ProviderState {
        provider,
        basis_soul_session_seq,
        opaque,
        schema_version,
    })
}

pub(super) fn map_compact_row(row: sqlx::sqlite::SqliteRow) -> Compact {
    Compact {
        id: row.get("id"),
        turn_id: row.get("turn_id"),
        summary: row.get("summary"),
        start_session_seq: row.get("start_session_seq"),
        end_session_seq: row.get("end_session_seq"),
        created_at: row.get("created_at"),
    }
}

pub(super) fn tool_call_target(
    input: santi_core::port::soul_runtime::AppendToolCall,
    created_at: String,
) -> ToolCall {
    ToolCall {
        id: input.tool_call_id,
        turn_id: input.turn_id,
        tool_name: input.tool_name,
        arguments: input.arguments,
        created_at,
    }
}

pub(super) fn tool_result_target(
    input: santi_core::port::soul_runtime::AppendToolResult,
    created_at: String,
) -> ToolResult {
    ToolResult {
        id: input.tool_result_id,
        tool_call_id: input.tool_call_id,
        output: input.output,
        error_text: input.error_text,
        created_at,
    }
}
