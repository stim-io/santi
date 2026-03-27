use std::sync::Arc;
use std::time::Duration;

use santi_core::port::{
    lock::Lock,
    provider::Provider,
    session_ledger::SessionLedgerPort,
    soul::SoulPort,
    soul_runtime::SoulRuntimePort,
};
use santi_db::{
    adapter::{session_ledger::DbSessionLedger, soul::DbSoul, soul_runtime::DbSoulRuntime},
    db::init_postgres,
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

        let default_soul_id = "soul_default".to_string();
        let provider = Arc::new(provider);
        let lock: Arc<dyn Lock> = lock_client;
        let provider: Arc<dyn Provider> = provider;
        let session_ledger: Arc<dyn SessionLedgerPort> = Arc::new(DbSessionLedger::new(pool.clone()));
        let soul_port: Arc<dyn SoulPort> = Arc::new(DbSoul::new(pool.clone()));
        let soul_runtime: Arc<dyn SoulRuntimePort> = Arc::new(DbSoulRuntime::new(pool));

        let session_memory = Arc::new(SessionMemoryService::new(
            soul_runtime.clone(),
            soul_port.clone(),
            default_soul_id.clone(),
        ));
        let session_query = Arc::new(SessionQueryService::new(
            session_ledger.clone(),
            soul_port.clone(),
            default_soul_id.clone(),
        ));
        let session_send = Arc::new(SessionSendService::new(
            config.openai_model.clone(),
            default_soul_id,
            lock,
            session_ledger,
            soul_runtime,
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
