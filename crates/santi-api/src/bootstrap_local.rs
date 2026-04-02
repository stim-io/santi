use std::{fs::OpenOptions, sync::Arc};

use santi_db::adapter::{
    local_effect_ledger::LocalEffectLedger,
    local_session_fork_compact::LocalSessionForkCompactStore,
    local_session_store::LocalSessionStore, local_soul_runtime::LocalSoulRuntime,
    local_soul_store::LocalSoulStore,
};
use santi_ebus::InMemorySubscriberSet;
use santi_lock::InProcessLock;
use santi_runtime::hooks::{compile_hook_specs, load_hook_specs, HookEvaluator};
use santi_runtime::session::{
    local_send::LocalSessionSendService, memory::SessionMemoryService, query::SessionQueryService,
};

use crate::{
    config::Config,
    state::AppState,
    surface::{default_capabilities, LocalAdminApi, LocalSessionApi, LocalSoulApi},
};

pub async fn bootstrap_local(config: &Config) -> santi_core::error::Result<AppState> {
    let lock = acquire_local_bootstrap_lock(&config.local_sqlite_path)?;
    let send_lock: Arc<dyn santi_core::port::lock::Lock> = Arc::new(InProcessLock::default());
    let store = Arc::new(LocalSessionStore::new(&config.local_sqlite_path).await?);
    let soul_store = Arc::new(LocalSoulStore::new(&config.local_sqlite_path).await?);
    let soul_runtime = Arc::new(LocalSoulRuntime::new(&config.local_sqlite_path).await?);
    let effect_ledger: Arc<dyn santi_core::port::effect_ledger::EffectLedgerPort> =
        Arc::new(LocalEffectLedger::new(&config.local_sqlite_path).await?);
    let soul_runtime_port: Arc<dyn santi_core::port::soul_runtime::SoulRuntimePort> =
        soul_runtime.clone();
    let fork_compact = Arc::new(
        LocalSessionForkCompactStore::new(&config.local_sqlite_path, send_lock.clone()).await?,
    );
    let session_ledger: Arc<dyn santi_core::port::session_ledger::SessionLedgerPort> =
        store.clone();
    let soul_port: Arc<dyn santi_core::port::soul::SoulPort> = soul_store;
    let soul_runtime: Arc<dyn santi_core::port::soul_runtime::SoulRuntimePort> = soul_runtime;
    let hook_specs = load_startup_hook_specs(config.hook_source.as_ref()).await?;
    let ebus: Arc<dyn santi_core::port::ebus::SubscriberSetPort<Arc<dyn HookEvaluator>>> =
        Arc::new(InMemorySubscriberSet::<Arc<dyn HookEvaluator>>::new());
    ebus.replace_all(compile_hook_specs(&hook_specs));
    let send = Arc::new(LocalSessionSendService::new(
        send_lock,
        session_ledger,
        soul_runtime_port,
    ));
    let memory = Arc::new(SessionMemoryService::new(
        soul_runtime.clone(),
        soul_port.clone(),
        "soul_default".to_string(),
    ));
    let query = Arc::new(SessionQueryService::new(
        store.clone(),
        soul_port,
        soul_runtime,
        "soul_default".to_string(),
    ));

    Ok(AppState::new(
        config.mode.clone(),
        default_capabilities(&config.mode),
        Arc::new(LocalSessionApi {
            query: query.clone(),
            memory: memory.clone(),
            fork_compact,
            effect_ledger,
            send,
        }),
        Arc::new(LocalSoulApi {
            session_query: query,
            memory,
        }),
        Arc::new(LocalAdminApi { ebus }),
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

fn acquire_local_bootstrap_lock(
    sqlite_path: &str,
) -> santi_core::error::Result<Arc<std::fs::File>> {
    let lock_path = std::path::Path::new(sqlite_path).with_extension("lock");
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| santi_core::error::Error::Internal {
            message: format!("create local lock parent dir failed: {err}"),
        })?;
    }

    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|err| santi_core::error::Error::Internal {
            message: format!("open local bootstrap lock failed: {err}"),
        })?;

    fs2::FileExt::try_lock_exclusive(&lock_file).map_err(|err| {
        santi_core::error::Error::Internal {
            message: format!(
                "local bootstrap lock already held for {}: {err}",
                lock_path.display()
            ),
        }
    })?;

    Ok(Arc::new(lock_file))
}
