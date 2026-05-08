use crate::{error::Result, model::runtime::SoulSession};

#[async_trait::async_trait]
pub trait SoulSessionForkPort: Send + Sync {
    async fn fork_soul_session(
        &self,
        parent_soul_session_id: &str,
        fork_point: i64,
        new_soul_session_id: &str,
        new_session_id: &str,
    ) -> Result<SoulSession>;
}
