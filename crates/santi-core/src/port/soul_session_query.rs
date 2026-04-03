use crate::{error::Result, model::runtime::SoulSession};

#[async_trait::async_trait]
pub trait SoulSessionQueryPort: Send + Sync {
    async fn get_soul_session_by_session_id(&self, session_id: &str)
        -> Result<Option<SoulSession>>;
}
