use crate::{error::Result, model::runtime::AssemblyItem};

#[derive(Clone, Debug)]
pub struct AppendCompact {
    pub compact_id: String,
    pub turn_id: String,
    pub summary: String,
    pub start_session_seq: i64,
    pub end_session_seq: i64,
}

#[async_trait::async_trait]
pub trait CompactRuntimePort: Send + Sync {
    async fn append_compact(&self, input: AppendCompact) -> Result<AssemblyItem>;
}
