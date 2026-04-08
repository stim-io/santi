use std::{fs::OpenOptions, sync::Arc};

use santi_db::adapter::standalone::{
    effect_ledger::StandaloneEffectLedger, session_compact::StandaloneSessionCompactStore,
    session_fork::StandaloneSessionForkStore, session_store::StandaloneSessionStore,
    soul_runtime::StandaloneSoulRuntime, soul_store::StandaloneSoulStore,
};
use santi_ebus::adapter::standalone::InMemorySubscriberSet;
use santi_lock::adapter::standalone::InProcessLock;
use santi_runtime::hooks::{compile_hook_specs, load_hook_specs, HookEvaluator};
use santi_runtime::session::{
    memory::SessionMemoryService, query::SessionQueryService,
    standalone_send::StandaloneSessionSendService,
};

use crate::{
    config::Config,
    state::AppState,
    surface::{default_capabilities, StandaloneAdminApi, StandaloneSessionApi, StandaloneSoulApi},
};

pub async fn bootstrap_standalone(config: &Config) -> santi_core::error::Result<AppState> {
    let lock = acquire_standalone_bootstrap_lock(&config.standalone_sqlite_path)?;
    let send_lock: Arc<dyn santi_core::port::lock::Lock> = Arc::new(InProcessLock::default());
    let store = Arc::new(StandaloneSessionStore::new(&config.standalone_sqlite_path).await?);
    let soul_store = Arc::new(StandaloneSoulStore::new(&config.standalone_sqlite_path).await?);
    let soul_runtime = Arc::new(StandaloneSoulRuntime::new(&config.standalone_sqlite_path).await?);
    let effect_ledger: Arc<dyn santi_core::port::effect_ledger::EffectLedgerPort> =
        Arc::new(StandaloneEffectLedger::new(&config.standalone_sqlite_path).await?);
    let soul_runtime_port: Arc<dyn santi_core::port::soul_runtime::SoulRuntimePort> =
        soul_runtime.clone();
    let soul_session_query: Arc<dyn santi_core::port::soul_session_query::SoulSessionQueryPort> =
        soul_runtime.clone();
    let fork = Arc::new(
        StandaloneSessionForkStore::new(&config.standalone_sqlite_path, send_lock.clone()).await?,
    );
    let compact = Arc::new(
        StandaloneSessionCompactStore::new(&config.standalone_sqlite_path, send_lock.clone())
            .await?,
    );
    let compact_ledger: Arc<dyn santi_core::port::compact_ledger::CompactLedgerPort> =
        compact.clone();
    let session_ledger: Arc<dyn santi_core::port::session_ledger::SessionLedgerPort> =
        store.clone();
    let soul_port: Arc<dyn santi_core::port::soul::SoulPort> = soul_store;
    let soul_runtime: Arc<dyn santi_core::port::soul_runtime::SoulRuntimePort> = soul_runtime;
    let hook_specs = load_startup_hook_specs(config.hook_source.as_ref()).await?;
    let ebus: Arc<dyn santi_core::port::ebus::SubscriberSetPort<Arc<dyn HookEvaluator>>> =
        Arc::new(InMemorySubscriberSet::<Arc<dyn HookEvaluator>>::new());
    ebus.replace_all(compile_hook_specs(&hook_specs));
    let send = Arc::new(StandaloneSessionSendService::new(
        send_lock,
        session_ledger,
        soul_runtime_port,
    ));
    let memory = Arc::new(SessionMemoryService::new(
        soul_runtime.clone(),
        soul_session_query.clone(),
        soul_port.clone(),
        "soul_default".to_string(),
    ));
    let query = Arc::new(SessionQueryService::new(
        store.clone(),
        soul_port,
        soul_session_query,
        compact_ledger,
        "soul_default".to_string(),
    ));

    Ok(AppState::new(
        config.mode.clone(),
        default_capabilities(&config.mode),
        Arc::new(StandaloneSessionApi {
            query: query.clone(),
            memory: memory.clone(),
            fork,
            compact,
            effect_ledger,
            send,
        }),
        Arc::new(StandaloneSoulApi {
            session_query: query,
            memory,
        }),
        Arc::new(StandaloneAdminApi { ebus }),
        Some(lock),
    ))
}

async fn load_startup_hook_specs(
    source: Option<&santi_core::hook::HookSpecSource>,
) -> santi_core::error::Result<Vec<santi_core::hook::HookSpec>> {
    match source {
        Some(source) => load_hook_specs(source)
            .await
            .map_err(|message| santi_core::error::Error::InvalidInput { message }),
        None => Ok(Vec::new()),
    }
}

fn acquire_standalone_bootstrap_lock(
    sqlite_path: &str,
) -> santi_core::error::Result<Arc<std::fs::File>> {
    let lock_path = std::path::Path::new(sqlite_path).with_extension("lock");
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| santi_core::error::Error::Internal {
            message: format!("create standalone lock parent dir failed: {err}"),
        })?;
    }

    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|err| santi_core::error::Error::Internal {
            message: format!("open standalone bootstrap lock failed: {err}"),
        })?;

    fs2::FileExt::try_lock_exclusive(&lock_file).map_err(|err| {
        santi_core::error::Error::Internal {
            message: format!(
                "standalone bootstrap lock already held for {}: {err}",
                lock_path.display()
            ),
        }
    })?;

    Ok(Arc::new(lock_file))
}
