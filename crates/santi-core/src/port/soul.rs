use crate::{error::Result, model::soul::Soul};

#[async_trait::async_trait]
pub trait SoulPort: Send + Sync {
    async fn get_soul(&self, soul_id: &str) -> Result<Option<Soul>>;
    async fn write_soul_memory(&self, soul_id: &str, text: &str) -> Result<Option<Soul>>;
}
