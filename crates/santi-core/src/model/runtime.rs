use serde_json::Value;

use crate::model::session::SessionMessage;

#[derive(Clone, Debug)]
pub struct ProviderState {
    pub provider: String,
    pub basis_soul_session_seq: i64,
    pub opaque: Value,
    pub schema_version: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SoulSession {
    pub id: String,
    pub soul_id: String,
    pub session_id: String,
    pub session_memory: String,
    pub provider_state: Option<ProviderState>,
    pub next_seq: i64,
    pub last_seen_session_seq: i64,
    pub parent_soul_session_id: Option<String>,
    pub fork_point: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnTriggerType {
    SessionSend,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug)]
pub struct Turn {
    pub id: String,
    pub soul_session_id: String,
    pub trigger_type: TurnTriggerType,
    pub trigger_ref: Option<String>,
    pub input_through_session_seq: i64,
    pub base_soul_session_seq: i64,
    pub end_soul_session_seq: Option<i64>,
    pub status: TurnStatus,
    pub error_text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub turn_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct ToolResult {
    pub id: String,
    pub tool_call_id: String,
    pub output: Option<Value>,
    pub error_text: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct ToolActivity {
    pub tool_call: ToolCall,
    pub tool_call_seq: i64,
    pub tool_result: Option<ToolResult>,
    pub tool_result_seq: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct Compact {
    pub id: String,
    pub turn_id: String,
    pub summary: String,
    pub start_session_seq: i64,
    pub end_session_seq: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoulSessionTargetType {
    Message,
    Compact,
    ToolCall,
    ToolResult,
}

#[derive(Clone, Debug)]
pub struct SoulSessionEntry {
    pub soul_session_id: String,
    pub target_type: SoulSessionTargetType,
    pub target_id: String,
    pub soul_session_seq: i64,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub enum AssemblyTarget {
    Message(SessionMessage),
    Compact(Compact),
    ToolCall(ToolCall),
    ToolResult(ToolResult),
}

#[derive(Clone, Debug)]
pub struct AssemblyItem {
    pub entry: SoulSessionEntry,
    pub target: AssemblyTarget,
}

#[derive(Clone, Debug)]
pub struct TurnContext {
    pub session: crate::model::session::Session,
    pub soul_session: SoulSession,
    pub soul: crate::model::soul::Soul,
}
