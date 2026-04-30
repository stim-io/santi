use sqlx::Row;

use santi_core::{
    error::{Error, Result},
    model::{
        message::{ActorType, Message, MessageContent, MessagePart, MessageState},
        session::{SessionMessage, SessionMessageRef},
    },
};

pub(super) fn map_session_message_row(row: sqlx::sqlite::SqliteRow) -> SessionMessage {
    SessionMessage {
        message: Message {
            id: row.get("id"),
            actor_type: actor_type(&row.get::<String, _>("actor_type")),
            actor_id: row.get("actor_id"),
            content: content_from_row(&row),
            state: message_state(&row.get::<String, _>("state")),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            deleted_at: None,
            version: row.get("version"),
        },
        relation: SessionMessageRef {
            message_id: row.get("id"),
            session_id: row.get("session_id"),
            session_seq: row.get("session_seq"),
            created_at: row.get("created_at"),
        },
    }
}

fn content_from_row(row: &sqlx::sqlite::SqliteRow) -> MessageContent {
    row.try_get::<String, _>("content_json")
        .ok()
        .and_then(|raw| serde_json::from_str::<MessageContent>(&raw).ok())
        .unwrap_or_else(|| MessageContent {
            parts: vec![MessagePart::Text {
                text: row.get("content_text"),
            }],
        })
}

fn actor_type(raw: &str) -> ActorType {
    match raw {
        "soul" => ActorType::Soul,
        "system" => ActorType::System,
        _ => ActorType::Account,
    }
}

pub(super) fn actor_type_db(actor_type: &ActorType) -> &'static str {
    match actor_type {
        ActorType::Account => "account",
        ActorType::Soul => "soul",
        ActorType::System => "system",
    }
}

fn message_state(raw: &str) -> MessageState {
    match raw {
        "fixed" => MessageState::Fixed,
        _ => MessageState::Pending,
    }
}

pub(super) fn state_db(state: &MessageState) -> &'static str {
    match state {
        MessageState::Pending => "pending",
        MessageState::Fixed => "fixed",
    }
}

pub(super) fn content_to_text(content: &MessageContent) -> Result<String> {
    let parts = content
        .parts
        .iter()
        .map(|part| match part {
            MessagePart::Text { text } => Ok(text.as_str()),
            MessagePart::Image { .. } => Err(Error::InvalidInput {
                message: "standalone stim message lifecycle supports text parts only".to_string(),
            }),
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(parts.join("\n\n"))
}

pub(super) fn content_to_json(content: &MessageContent) -> Result<String> {
    serde_json::to_string(content).map_err(|err| Error::Internal {
        message: format!("message content serialize failed: {err}"),
    })
}
