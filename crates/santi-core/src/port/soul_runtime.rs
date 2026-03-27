use serde_json::Value;

use crate::{
    error::Result,
    model::runtime::{
        AssemblyItem, ProviderState, SoulSession, Turn, TurnContext, TurnTriggerType,
    },
};

#[derive(Clone, Debug)]
pub struct StartTurn {
    pub turn_id: String,
    pub soul_session_id: String,
    pub trigger_type: TurnTriggerType,
    pub trigger_ref: Option<String>,
    pub input_through_session_seq: i64,
}

#[derive(Clone, Debug)]
pub struct AppendMessageRef {
    pub soul_session_id: String,
    pub message_id: String,
}

#[derive(Clone, Debug)]
pub struct AppendToolCall {
    pub tool_call_id: String,
    pub turn_id: String,
    pub tool_name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug)]
pub struct AppendToolResult {
    pub tool_result_id: String,
    pub tool_call_id: String,
    pub output: Option<Value>,
    pub error_text: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AppendCompact {
    pub compact_id: String,
    pub turn_id: String,
    pub summary: String,
    pub start_session_seq: i64,
    pub end_session_seq: i64,
}

#[derive(Clone, Debug)]
pub struct CompleteTurn {
    pub turn_id: String,
    pub last_seen_session_seq: i64,
    pub provider_state: Option<ProviderState>,
}

#[derive(Clone, Debug)]
pub struct FailTurn {
    pub turn_id: String,
    pub error_text: String,
}

#[async_trait::async_trait]
pub trait SoulRuntimePort: Send + Sync {
    async fn get_or_create_soul_session(&self, soul_id: &str, session_id: &str)
        -> Result<SoulSession>;
    async fn get_soul_session(&self, soul_session_id: &str) -> Result<Option<SoulSession>>;
    async fn load_turn_context(&self, soul_id: &str, session_id: &str) -> Result<Option<TurnContext>>;
    async fn write_session_memory(
        &self,
        soul_session_id: &str,
        text: &str,
    ) -> Result<Option<SoulSession>>;
    async fn start_turn(&self, input: StartTurn) -> Result<Turn>;
    async fn append_message_ref(&self, input: AppendMessageRef) -> Result<AssemblyItem>;
    async fn append_tool_call(&self, input: AppendToolCall) -> Result<AssemblyItem>;
    async fn append_tool_result(&self, input: AppendToolResult) -> Result<AssemblyItem>;
    async fn append_compact(&self, input: AppendCompact) -> Result<AssemblyItem>;
    async fn complete_turn(&self, input: CompleteTurn) -> Result<Turn>;
    async fn fail_turn(&self, input: FailTurn) -> Result<Turn>;
    async fn list_assembly_items(
        &self,
        soul_session_id: &str,
        after_soul_session_seq: Option<i64>,
    ) -> Result<Vec<AssemblyItem>>;
}
