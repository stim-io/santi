use std::sync::Arc;
use std::time::Duration;

use santi_core::{port::{lock::Lock, provider::Provider, turn_store::TurnStore}};
use santi_db::adapter::{
    memory_store::RepoBackedMemoryStore,
    session_query::RepoBackedSessionQuery,
    turn_store::RepoBackedTurnStore,
};
use santi_db::{
    db::init_postgres,
    repo::{
        message_repo::MessageRepo,
        relation_repo::RelationRepo,
        session_repo::SessionRepo,
        soul_repo::SoulRepo,
    },
};
use santi_lock::{RedisLockClient, RedisLockConfig};
use santi_provider::openai_compatible::OpenAiCompatibleProvider;
use santi_runtime::{
    runtime::tools::ToolExecutorConfig,
    session::{memory::SessionMemoryService, query::SessionQueryService, send::SessionSendService},
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
        let pool = init_postgres(&config.database_url)
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
        let session_query_port = Arc::new(RepoBackedSessionQuery::new(
            session_repo.clone(),
            soul_repo.clone(),
            message_repo.clone(),
        ));
        let memory_store = Arc::new(RepoBackedMemoryStore::new(
            session_repo.clone(),
            soul_repo.clone(),
        ));
        let provider = Arc::new(provider);
        let lock: Arc<dyn Lock> = lock_client;
        let turn_store: Arc<dyn TurnStore> = turn_store;
        let provider: Arc<dyn Provider> = provider;
        let session_memory = Arc::new(SessionMemoryService::new(memory_store));
        let session_query = Arc::new(SessionQueryService::new(
            session_query_port,
            "soul_default".to_string(),
        ));
        let session_send = Arc::new(SessionSendService::new(
            config.openai_model.clone(),
            lock,
            turn_store,
            provider,
            session_memory.as_ref().clone(),
            ToolExecutorConfig {
                runtime_root: config.runtime_root.clone(),
                execution_root: config.execution_root.clone(),
            },
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
