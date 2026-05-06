use crate::{
    error::Result,
    model::runtime::{SoulSession, ToolActivity},
};

#[async_trait::async_trait]
pub trait SoulSessionQueryPort: Send + Sync {
    async fn get_soul_session_by_session_id(&self, session_id: &str)
        -> Result<Option<SoulSession>>;

    async fn list_tool_activities(&self, soul_session_id: &str) -> Result<Vec<ToolActivity>>;
}
