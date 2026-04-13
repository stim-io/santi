use sqlx::{postgres::PgRow, Row};

use santi_core::{
    error::{Error, Result},
    model::{
        message::{ActorType, Message, MessageContent, MessageEventPayload, MessageState},
        session::{SessionMessage, SessionMessageRef},
    },
};

pub(super) fn map_session_message_row(row: PgRow) -> Result<SessionMessage> {
    Ok(SessionMessage {
        relation: SessionMessageRef {
            session_id: row.get("session_id"),
            message_id: row.get("message_id"),
            session_seq: row.get("session_seq"),
            created_at: row.get("relation_created_at"),
        },
        message: map_message_row(&row)?,
    })
}

pub(super) fn map_message_row(row: &PgRow) -> Result<Message> {
    Ok(Message {
        id: row.get("id"),
        actor_type: parse_actor_type(row.get::<String, _>("actor_type").as_str())?,
        actor_id: row.get("actor_id"),
        content: row.get::<sqlx::types::Json<MessageContent>, _>("content").0,
        state: parse_message_state(row.get::<String, _>("state").as_str())?,
        version: row.get("version"),
        deleted_at: row.try_get("deleted_at").ok(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn parse_actor_type(value: &str) -> Result<ActorType> {
    match value {
        "account" => Ok(ActorType::Account),
        "soul" => Ok(ActorType::Soul),
        "system" => Ok(ActorType::System),
        _ => Err(Error::Internal {
            message: format!("unknown actor_type: {value}"),
        }),
    }
}

fn parse_message_state(value: &str) -> Result<MessageState> {
    match value {
        "pending" => Ok(MessageState::Pending),
        "fixed" => Ok(MessageState::Fixed),
        _ => Err(Error::Internal {
            message: format!("unknown message state: {value}"),
        }),
    }
}

pub(super) fn actor_type_str(value: &ActorType) -> &'static str {
    match value {
        ActorType::Account => "account",
        ActorType::Soul => "soul",
        ActorType::System => "system",
    }
}

pub(super) fn message_state_str(value: &MessageState) -> &'static str {
    match value {
        MessageState::Pending => "pending",
        MessageState::Fixed => "fixed",
    }
}

pub(super) fn payload_action_str(value: &MessageEventPayload) -> &'static str {
    match value {
        MessageEventPayload::Patch { .. } => "patch",
        MessageEventPayload::Insert { .. } => "insert",
        MessageEventPayload::Remove { .. } => "remove",
        MessageEventPayload::Fix => "fix",
        MessageEventPayload::Delete { .. } => "delete",
    }
}
