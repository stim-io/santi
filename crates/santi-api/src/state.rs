use std::sync::Arc;
use std::time::Duration;

use santi_lock::{RedisLockClient, RedisLockConfig};
use santi_runtime::{
    runtime::tools::ToolExecutor,
    session::{memory::SessionMemoryService, query::SessionQueryService, send::SessionSendService},
};

use crate::{
    adapter::turn_store::RepoBackedTurnStore,
    db,
    repo::{
        message_repo::MessageRepo,
        relation_repo::RelationRepo,
        session_repo::SessionRepo,
        soul_repo::SoulRepo,
    },
    service::openai_compatible::OpenAiCompatibleProvider,
};

#[derive(Clone)]
pub struct AppState {
    session_memory: Arc<SessionMemoryService>,
    session_query: Arc<SessionQueryService>,
    session_send: Arc<SessionSendService>,
}

impl AppState {
    pub async fn new(
        config: crate::config::Config,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let provider = OpenAiCompatibleProvider::new(
            config.openai_api_key.clone(),
            config.openai_base_url.clone(),
        );
        let pool = db::init_postgres(&config.database_url)
            .await
            .inspect_err(|err| {
                tracing::error!(component = "postgres", error = %err, "app state init failed");
            })?;
        let lock_client = Arc::new(
            RedisLockClient::new(
                &config.redis_url,
                RedisLockConfig {
                    ttl: Duration::from_secs(120),
                    renew_interval: Duration::from_secs(40),
                    acquire_timeout: Duration::from_millis(500),
                    key_prefix: None,
                },
            )
            .await
            .inspect_err(|err| {
                tracing::error!(component = "redis_lock", error = %err, "app state init failed");
            })?,
        );
        let session_repo = Arc::new(SessionRepo::new(pool.clone()));
        let soul_repo = Arc::new(SoulRepo::new(pool.clone()));
        let message_repo = Arc::new(MessageRepo::new(pool.clone()));
        let relation_repo = Arc::new(RelationRepo::new());
        let turn_store = Arc::new(RepoBackedTurnStore::new(
            session_repo.clone(),
            soul_repo.clone(),
            message_repo.clone(),
            relation_repo.clone(),
        ));
        let provider = Arc::new(provider);
        let session_memory = Arc::new(SessionMemoryService::new(session_repo.clone(), soul_repo.clone()));
        let session_query = Arc::new(SessionQueryService::new(
            session_repo.clone(),
            soul_repo.clone(),
            message_repo.clone(),
        ));
        let tools = Arc::new(ToolExecutor::new(
            session_memory.as_ref().clone(),
            config.runtime_root.clone(),
            config.execution_root.clone(),
        ));
        let session_send = Arc::new(SessionSendService::new(
            config.openai_model.clone(),
            lock_client,
            turn_store,
            provider.clone(),
            tools.clone(),
        ));

        Ok(Self {
            session_memory,
            session_query,
            session_send,
        })
    }

    pub fn session_send(&self) -> Arc<SessionSendService> {
        self.session_send.clone()
    }

    pub fn session_memory(&self) -> Arc<SessionMemoryService> {
        self.session_memory.clone()
    }

    pub fn session_query(&self) -> Arc<SessionQueryService> {
        self.session_query.clone()
    }
}
