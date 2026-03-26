use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    Account,
    Soul,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageState {
    Pending,
    Fixed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageContent {
    pub parts: Vec<MessagePart>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text {
        text: String,
    },
    Image {
        mime_type: String,
        data_base64: String,
    },
}

#[derive(Clone, Debug)]
pub struct Message {
    pub id: String,
    pub actor_type: ActorType,
    pub actor_id: String,
    pub content: MessageContent,
    pub state: MessageState,
    pub version: i64,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageEventAction {
    Patch,
    Insert,
    Remove,
    Fix,
    Delete,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MessagePartPatch {
    pub index: i64,
    pub merge: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageInsertItem {
    pub index: i64,
    pub part: MessagePart,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageEventPayload {
    Patch { patches: Vec<MessagePartPatch> },
    Insert { items: Vec<MessageInsertItem> },
    Remove { indexes: Vec<i64> },
    Fix,
    Delete { reason: Option<String> },
}

#[derive(Clone, Debug)]
pub struct MessageEvent {
    pub id: String,
    pub message_id: String,
    pub action: MessageEventAction,
    pub actor_type: ActorType,
    pub actor_id: String,
    pub base_version: i64,
    pub payload: MessageEventPayload,
    pub created_at: String,
}
