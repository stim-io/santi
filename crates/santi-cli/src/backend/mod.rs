pub mod api;
pub mod local;

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use santi_core::hook::HookSpecSource;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliSession {
    pub id: String,
    pub parent_session_id: Option<String>,
    pub fork_point: Option<i64>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForkedCliSession {
    pub id: String,
    pub parent_session_id: String,
    pub fork_point: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliHealth {
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliSoul {
    pub id: String,
    pub memory: String,
    pub created_at: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliMemoryRecord {
    pub id: String,
    pub memory: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliCompact {
    pub id: String,
    pub turn_id: String,
    pub summary: String,
    pub start_session_seq: i64,
    pub end_session_seq: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliSessionEffects {
    pub effects: Vec<CliSessionEffect>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliSessionEffect {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliHookReload {
    pub hook_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliMessage {
    pub id: String,
    pub actor_type: String,
    pub actor_id: String,
    pub session_seq: i64,
    pub content_text: String,
    pub state: String,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub enum SendEvent {
    OutputTextDelta(String),
    Completed,
}

pub type SendStream = Pin<Box<dyn Stream<Item = Result<SendEvent, BackendError>> + Send>>;

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("session not found")]
    NotFound,

    #[error("session send already in progress")]
    Busy,

    #[error("backend error: {0}")]
    Other(String),
}

#[async_trait]
pub trait CliBackend: Send + Sync {
    async fn health(&self) -> Result<CliHealth, BackendError>;
    async fn create_session(&self) -> Result<CliSession, BackendError>;
    async fn get_session(&self, session_id: String) -> Result<CliSession, BackendError>;
    async fn fork_session(
        &self,
        session_id: String,
        fork_point: i64,
    ) -> Result<ForkedCliSession, BackendError>;
    async fn send_session(
        &self,
        session_id: String,
        content: String,
        wait: bool,
    ) -> Result<SendStream, BackendError>;
    async fn list_messages(&self, session_id: String) -> Result<Vec<CliMessage>, BackendError>;
    async fn get_default_soul(&self) -> Result<CliSoul, BackendError>;
    async fn set_default_soul_memory(&self, text: String) -> Result<CliMemoryRecord, BackendError>;
    async fn set_session_memory(
        &self,
        session_id: String,
        text: String,
    ) -> Result<CliMemoryRecord, BackendError>;
    async fn get_session_memory(&self, session_id: String)
        -> Result<CliMemoryRecord, BackendError>;
    async fn compact_session(
        &self,
        session_id: String,
        summary: String,
    ) -> Result<CliCompact, BackendError>;
    async fn list_compacts(&self, session_id: String) -> Result<Vec<CliCompact>, BackendError>;
    async fn list_session_effects(
        &self,
        session_id: String,
    ) -> Result<CliSessionEffects, BackendError>;
    async fn reload_hooks(&self, source: HookSpecSource) -> Result<CliHookReload, BackendError>;
}
