use serde::Serialize;
use utoipa::ToSchema;

use crate::model::{
    message::MessagePart,
    runtime::{Compact, SoulSession},
    session::{Session, SessionMessage},
    soul::Soul,
};

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SessionResponse {
    pub id: String,
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
