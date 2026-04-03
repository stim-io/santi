use std::sync::Arc;

use santi_core::hook::{HookSpec, HookSpecSource};
use santi_db::{
    adapter::postgres::{
        effect_ledger::DbEffectLedger, session_ledger::DbSessionLedger, soul::DbSoul,
        soul_runtime::DbSoulRuntime,
    },
    db::init_postgres,
};
use santi_ebus::adapter::local::InMemorySubscriberSet;
use santi_lock::adapter::redis::{RedisLockClient, RedisLockConfig};
use santi_runtime::{
    hooks::{compile_hook_specs, load_hook_specs, HookEvaluator},
    runtime::tools::ToolExecutorConfig,
    session::{
        compact::SessionCompactService, fork::SessionForkService, memory::SessionMemoryService,
        query::SessionQueryService, send::SessionSendService,
    },
};

use crate::{
    config::{Config, Mode},
    link_client::OpenAiResponsesClient,
    state::AppState,
    surface::{default_capabilities, HostedAdminApi, HostedSessionApi, HostedSoulApi},
};

pub async fn bootstrap(
    config: &Config,
) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    match config.mode {
        Mode::Hosted => hosted_bootstrap(config).await,
        Mode::Local => crate::bootstrap_local::bootstrap_local(config)
            .await
            .map_err(|err| Box::<dyn std::error::Error + Send + Sync>::from(format!("{err:?}"))),
    }
}

async fn hosted_bootstrap(
    config: &Config,
) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let provider = OpenAiResponsesClient::new(
        config.openai_api_key.clone(),
        config.openai_base_url.clone(),
    );
    let pool = init_postgres(&config.database_url).await?;
    let lock_client = Arc::new(
        RedisLockClient::new(
            &config.redis_url,
            RedisLockConfig {
                ttl: std::time::Duration::from_secs(120),
                renew_interval: std::time::Duration::from_secs(40),
                acquire_timeout: std::time::Duration::from_millis(500),
                key_prefix: None,
            },
        )
        .await?,
    );

    let default_soul_id = "soul_default".to_string();
    let provider = Arc::new(provider);
    let lock: Arc<dyn santi_core::port::lock::Lock> = lock_client;
    let provider: Arc<dyn santi_core::port::provider::Provider> = provider;
    let session_ledger: Arc<dyn santi_core::port::session_ledger::SessionLedgerPort> =
        Arc::new(DbSessionLedger::new(pool.clone()));
    let effect_ledger: Arc<dyn santi_core::port::effect_ledger::EffectLedgerPort> =
        Arc::new(DbEffectLedger::new(pool.clone()));
    let soul_port: Arc<dyn santi_core::port::soul::SoulPort> = Arc::new(DbSoul::new(pool.clone()));
    let soul_runtime_impl = Arc::new(DbSoulRuntime::new(pool));
    let soul_runtime: Arc<dyn santi_core::port::soul_runtime::SoulRuntimePort> =
        soul_runtime_impl.clone();
    let soul_session_query: Arc<dyn santi_core::port::soul_session_query::SoulSessionQueryPort> =
        soul_runtime_impl.clone();
    let compact_ledger: Arc<dyn santi_core::port::compact_ledger::CompactLedgerPort> =
        soul_runtime_impl.clone();
    let compact_runtime: Arc<dyn santi_core::port::compact_runtime::CompactRuntimePort> =
        soul_runtime_impl.clone();
    let soul_session_fork: Arc<dyn santi_core::port::soul_session_fork::SoulSessionForkPort> =
        soul_runtime_impl.clone();

    let session_memory = Arc::new(SessionMemoryService::new(
        soul_runtime.clone(),
        soul_session_query.clone(),
        soul_port.clone(),
        default_soul_id.clone(),
    ));
    let session_query = Arc::new(SessionQueryService::new(
        session_ledger.clone(),
        soul_port.clone(),
        soul_session_query.clone(),
        compact_ledger,
        default_soul_id.clone(),
    ));
    let session_compact = Arc::new(SessionCompactService::new(
        lock.clone(),
        session_ledger.clone(),
        soul_runtime.clone(),
        compact_runtime,
        default_soul_id.clone(),
    ));
    let hook_specs = load_startup_hook_specs(config.hook_source.as_ref()).await?;
    let ebus: Arc<dyn santi_core::port::ebus::SubscriberSetPort<Arc<dyn HookEvaluator>>> =
        Arc::new(InMemorySubscriberSet::<Arc<dyn HookEvaluator>>::new());
    ebus.replace_all(compile_hook_specs(&hook_specs));
    let session_fork = Arc::new(SessionForkService::new(
        lock.clone(),
        soul_session_query,
        soul_session_fork,
    ));

    let session_send = Arc::new(SessionSendService::new(
        config.openai_model.clone(),
        default_soul_id,
        lock.clone(),
        session_ledger.clone(),
        soul_runtime.clone(),
        soul_runtime_impl.clone(),
        effect_ledger.clone(),
        session_fork.clone(),
        provider,
        session_memory.as_ref().clone(),
        ToolExecutorConfig {
            runtime_root: config.runtime_root.clone(),
            execution_root: config.execution_root.clone(),
        },
        ebus,
    ));

    Ok(AppState::new(
        config.mode.clone(),
        default_capabilities(&config.mode),
        Arc::new(HostedSessionApi {
            query: session_query.clone(),
            memory: session_memory.clone(),
            compact: session_compact,
            send: session_send.clone(),
            fork: session_fork,
            effect_ledger,
        }),
        Arc::new(HostedSoulApi {
            query: session_query,
            memory: session_memory,
        }),
        Arc::new(HostedAdminApi { send: session_send }),
        None,
    ))
}

async fn load_startup_hook_specs(source: Option<&HookSpecSource>) -> Result<Vec<HookSpec>, String> {
    match source {
        Some(source) => load_hook_specs(source).await,
        None => Ok(Vec::new()),
    }
}
