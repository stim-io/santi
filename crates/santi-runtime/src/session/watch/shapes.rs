use futures::Stream;
use serde::{Deserialize, Serialize};

pub type SessionWatchStream =
    std::pin::Pin<Box<dyn Stream<Item = Result<SessionWatchEvent, SessionWatchError>> + Send>>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionWatchState {
    Idle,
    Running,
    Completed,
    Failed,
    Conflicted,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionWatchMessageChange {
    Created,
    Delta,
    Finalized,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionWatchActivityKind {
    Send,
    Tool,
    Hook,
    Compact,
    Fork,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionWatchActivityState {
    Started,
    Progress,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionWatchConnected {
    pub session_id: String,
    pub latest_seq: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionWatchStateChanged {
    pub session_id: String,
    pub state: SessionWatchState,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionWatchMessageChanged {
    pub session_id: String,
    pub message_id: String,
    pub session_seq: i64,
    pub change: SessionWatchMessageChange,
    pub actor_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionWatchActivityChanged {
    pub session_id: String,
    pub activity: SessionWatchActivityKind,
    pub state: SessionWatchActivityState,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionWatchEvent {
    Connected(SessionWatchConnected),
    StateChanged(SessionWatchStateChanged),
    MessageChanged(SessionWatchMessageChanged),
    ActivityChanged(SessionWatchActivityChanged),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionWatchMessageSummary {
    pub id: String,
    pub actor_type: String,
    pub actor_id: String,
    pub session_seq: i64,
    pub content_text: String,
    pub state: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionWatchEffectSummary {
    pub id: String,
    pub effect_type: String,
    pub status: String,
    pub source_hook_id: String,
    pub result_ref: Option<String>,
    pub error_text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionWatchSnapshot {
    pub session_id: String,
    pub latest_seq: i64,
    pub messages: Vec<SessionWatchMessageSummary>,
    pub effects: Vec<SessionWatchEffectSummary>,
}

#[derive(Clone, Debug)]
pub enum SessionWatchError {
    NotFound,
    Internal(String),
}
