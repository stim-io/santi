use crate::{
    error::Result,
    model::{
        message::{ActorType, MessageContent, MessageEventPayload, MessageState},
        session::{Session, SessionMessage},
    },
};

#[derive(Clone, Debug)]
pub struct AppendSessionMessage {
    pub session_id: String,
    pub message_id: String,
    pub actor_type: ActorType,
    pub actor_id: String,
    pub content: MessageContent,
    pub state: MessageState,
}

#[derive(Clone, Debug)]
pub struct ApplyMessageEvent {
    pub session_id: String,
    pub message_id: String,
    pub event_id: String,
    pub actor_type: ActorType,
    pub actor_id: String,
    pub base_version: i64,
    pub payload: MessageEventPayload,
}

#[async_trait::async_trait]
pub trait SessionLedgerPort: Send + Sync {
    async fn create_session(&self, session_id: &str) -> Result<Session>;
    async fn get_session(&self, session_id: &str) -> Result<Option<Session>>;
    async fn get_message(&self, message_id: &str) -> Result<Option<SessionMessage>>;
    async fn list_messages(
        &self,
        session_id: &str,
        after_session_seq: Option<i64>,
    ) -> Result<Vec<SessionMessage>>;
    async fn append_message(&self, input: AppendSessionMessage) -> Result<SessionMessage>;
    async fn apply_message_event(&self, input: ApplyMessageEvent) -> Result<SessionMessage>;
}
