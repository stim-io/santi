use crate::{
    error::Result,
    model::runtime::{SoulSession, ToolActivity},
};

#[async_trait::async_trait]
pub trait SoulSessionQueryPort: Send + Sync {
    async fn get_session_soul(&self, session_id: &str) -> Result<Option<SoulSession>>;

    async fn list_tool_activities(&self, soul_session_id: &str) -> Result<Vec<ToolActivity>>;
}
