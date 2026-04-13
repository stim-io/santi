use serde_json::{json, Value};
use sqlx::{postgres::PgRow, Row};

use santi_core::{
    error::{Error, Result},
    model::runtime::{Compact, ProviderState, SoulSession, Turn, TurnStatus, TurnTriggerType},
};

pub(super) fn map_soul_session_row(row: &PgRow) -> Result<SoulSession> {
    Ok(SoulSession {
        id: row.get("id"),
        soul_id: row.get("soul_id"),
        session_id: row.get("session_id"),
        session_memory: row.get("session_memory"),
        provider_state: row
            .try_get::<Option<serde_json::Value>, _>("provider_state")
            .map_err(|err| Error::Internal {
                message: format!("decode provider_state failed: {err}"),
            })?
            .map(decode_provider_state)
            .transpose()?,
        next_seq: row.get("next_seq"),
        last_seen_session_seq: row.get("last_seen_session_seq"),
        parent_soul_session_id: row.try_get("parent_soul_session_id").ok(),
        fork_point: row.try_get("fork_point").ok(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
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

pub(super) fn map_turn_row(row: &PgRow) -> Result<Turn> {
    Ok(Turn {
        id: row.get("id"),
        soul_session_id: row.get("soul_session_id"),
        trigger_type: match row.get::<String, _>("trigger_type").as_str() {
            "session_send" => TurnTriggerType::SessionSend,
            _ => TurnTriggerType::System,
        },
        trigger_ref: row.get("trigger_ref"),
        input_through_session_seq: row.get("input_through_session_seq"),
        base_soul_session_seq: row.get("base_soul_session_seq"),
        end_soul_session_seq: row.get("end_soul_session_seq"),
        status: match row.get::<String, _>("status").as_str() {
            "running" => TurnStatus::Running,
            "completed" => TurnStatus::Completed,
            _ => TurnStatus::Failed,
        },
        error_text: row.get("error_text"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        finished_at: row.try_get("finished_at").ok(),
    })
}

pub(super) fn map_compact_row(row: PgRow) -> Compact {
    Compact {
        id: row.get("id"),
        turn_id: row.get("turn_id"),
        summary: row.get("summary"),
        start_session_seq: row.get("start_session_seq"),
        end_session_seq: row.get("end_session_seq"),
        created_at: row.get("created_at"),
    }
}
