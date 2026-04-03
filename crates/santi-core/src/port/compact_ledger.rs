use crate::{error::Result, model::runtime::Compact};

#[async_trait::async_trait]
pub trait CompactLedgerPort: Send + Sync {
    async fn list_compacts(&self, soul_session_id: &str) -> Result<Vec<Compact>>;
}
