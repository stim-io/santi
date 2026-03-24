use serde::Serialize;
use utoipa::ToSchema;

use crate::model::{message::Message, session::Session, soul::Soul};

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

impl From<Session> for SessionMemoryResponse {
    fn from(value: Session) -> Self {
        Self {
            id: value.id,
            memory: value.memory,
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
    pub r#type: String,
    pub role: Option<String>,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone, Debug, serde::Deserialize, ToSchema)]
pub struct SessionSendRequest {
    pub content: Vec<SessionSendContentPart>,
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

impl From<Message> for SessionMessageResponse {
    fn from(value: Message) -> Self {
        Self {
            id: value.id,
            r#type: value.r#type,
            role: value.role,
            content: value.content,
            created_at: value.created_at,
        }
    }
}

impl SessionMessagesResponse {
    pub fn from_messages(messages: Vec<Message>) -> Self {
        Self {
            messages: messages
                .into_iter()
                .map(SessionMessageResponse::from)
                .collect(),
        }
    }
}
