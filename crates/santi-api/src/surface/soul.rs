use std::sync::Arc;

use async_trait::async_trait;
use santi_core::model::soul::Soul;
use santi_runtime::session::{memory::SessionMemoryService, query::SessionQueryService};

use super::error::ApiError;

#[async_trait]
pub trait SoulApi: Send + Sync {
    async fn get_default_soul(&self) -> Result<Soul, ApiError>;
    async fn set_default_soul_memory(&self, text: &str) -> Result<Soul, ApiError>;
}

#[derive(Clone)]
pub struct DistributedSoulApi {
    pub query: Arc<SessionQueryService>,
    pub memory: Arc<SessionMemoryService>,
}

#[derive(Clone)]
pub struct StandaloneSoulApi {
    pub session_query: Arc<SessionQueryService>,
    pub memory: Arc<SessionMemoryService>,
}

trait SoulApiDeps {
    fn soul_query(&self) -> &Arc<SessionQueryService>;
    fn memory(&self) -> &Arc<SessionMemoryService>;
}

impl SoulApiDeps for DistributedSoulApi {
    fn soul_query(&self) -> &Arc<SessionQueryService> {
        &self.query
    }

    fn memory(&self) -> &Arc<SessionMemoryService> {
        &self.memory
    }
}

impl SoulApiDeps for StandaloneSoulApi {
    fn soul_query(&self) -> &Arc<SessionQueryService> {
        &self.session_query
    }

    fn memory(&self) -> &Arc<SessionMemoryService> {
        &self.memory
    }
}

#[async_trait]
impl<T> SoulApi for T
where
    T: SoulApiDeps + Send + Sync,
{
    async fn get_default_soul(&self) -> Result<Soul, ApiError> {
        self.soul_query()
            .get_default_soul()
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("soul not found".to_string()))
    }

    async fn set_default_soul_memory(&self, text: &str) -> Result<Soul, ApiError> {
        self.memory()
            .write_soul_memory("soul_default", text)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("soul not found".to_string()))
    }
}
