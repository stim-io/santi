use std::sync::Arc;
use std::time::Duration;

use santi_core::port::{
    lock::Lock, provider::Provider, session_ledger::SessionLedgerPort, soul::SoulPort,
    soul_runtime::SoulRuntimePort,
};
use santi_db::{
    adapter::{session_ledger::DbSessionLedger, soul::DbSoul, soul_runtime::DbSoulRuntime},
    db::init_postgres,
};
use santi_ebus::InMemorySubscriberSet;
use santi_lock::{RedisLockClient, RedisLockConfig};
use santi_provider::openai_compatible::OpenAiCompatibleProvider;
use santi_runtime::{
    hooks::{compile_hook_specs, load_hook_specs, HookEvaluator},
    runtime::tools::ToolExecutorConfig,
    session::{
        compact::SessionCompactService, fork::SessionForkService, memory::SessionMemoryService,
        query::SessionQueryService, send::SessionSendService,
    },
};
use santi_core::hook::{HookSpec, HookSpecSource};

#[derive(Clone)]
pub struct AppState {
    session_memory: Arc<SessionMemoryService>,
    session_compact: Arc<SessionCompactService>,
    session_query: Arc<SessionQueryService>,
    session_send: Arc<SessionSendService>,
    session_fork: Arc<SessionForkService>,
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
        let session_ledger: Arc<dyn SessionLedgerPort> =
            Arc::new(DbSessionLedger::new(pool.clone()));
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
        let session_compact = Arc::new(SessionCompactService::new(
            lock.clone(),
            session_ledger.clone(),
            soul_runtime.clone(),
            default_soul_id.clone(),
        ));
        let hook_specs = load_startup_hook_specs(config.hook_source.as_ref()).await?;
        let ebus: Arc<dyn santi_core::port::ebus::SubscriberSetPort<Arc<dyn HookEvaluator>>> =
            Arc::new(InMemorySubscriberSet::<Arc<dyn HookEvaluator>>::new());
        ebus.replace_all(compile_hook_specs(&hook_specs));
        let session_fork = Arc::new(SessionForkService::new(lock.clone(), soul_runtime.clone()));

        let session_send = Arc::new(SessionSendService::new(
            config.openai_model.clone(),
            default_soul_id,
            lock.clone(),
            session_ledger.clone(),
            soul_runtime.clone(),
            provider,
            session_memory.as_ref().clone(),
            ToolExecutorConfig {
                runtime_root: config.runtime_root.clone(),
                execution_root: config.execution_root.clone(),
            },
            ebus,
        ));

        Ok(Self {
            session_memory,
            session_compact,
            session_query,
            session_send,
            session_fork,
        })
    }

    pub fn session_send(&self) -> Arc<SessionSendService> {
        self.session_send.clone()
    }

    pub fn session_memory(&self) -> Arc<SessionMemoryService> {
        self.session_memory.clone()
    }

    pub fn session_compact(&self) -> Arc<SessionCompactService> {
        self.session_compact.clone()
    }

    pub fn session_query(&self) -> Arc<SessionQueryService> {
        self.session_query.clone()
    }

    pub fn session_fork(&self) -> Arc<SessionForkService> {
        self.session_fork.clone()
    }

    pub fn reload_hooks(&self, specs: &[HookSpec]) -> usize {
        self.session_send.replace_hooks(specs)
    }

    pub async fn reload_hooks_from_source(&self, source: HookSpecSource) -> Result<usize, String> {
        let specs = load_hook_specs(&source).await?;
        Ok(self.reload_hooks(&specs))
    }
}

async fn load_startup_hook_specs(source: Option<&HookSpecSource>) -> Result<Vec<HookSpec>, String> {
    match source {
        Some(source) => load_hook_specs(source).await,
        None => Ok(Vec::new()),
    }
}
