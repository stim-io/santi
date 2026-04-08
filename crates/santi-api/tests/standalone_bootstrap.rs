use santi_api::{
    bootstrap_standalone::bootstrap_standalone,
    config::{Config, Mode},
};
use santi_core::port::lock::Lock;
use santi_db::adapter::standalone::{
    session_compact::StandaloneSessionCompactStore, session_store::StandaloneSessionStore,
    soul_runtime::StandaloneSoulRuntime, soul_store::StandaloneSoulStore,
};
use santi_lock::adapter::standalone::InProcessLock;
use santi_runtime::session::query::SessionQueryService;
use std::sync::Arc;

#[tokio::test]
async fn standalone_bootstrap_injects_session_store() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        mode: Mode::Standalone,
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        openai_api_key: String::new(),
        openai_base_url: String::new(),
        openai_model: String::new(),
        database_url: String::new(),
        redis_url: String::new(),
        standalone_sqlite_path: dir.path().join("standalone.sqlite").display().to_string(),
        execution_root: String::new(),
        runtime_root: String::new(),
        hook_source: None,
    };

    let state = bootstrap_standalone(&config).await.unwrap();
    assert_eq!(state.mode(), Mode::Standalone);
    assert!(state.standalone_bootstrap_lock().is_some());
    assert!(state.capabilities().admin_hooks);
}

#[tokio::test]
async fn standalone_bootstrap_fails_when_lock_is_held() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        mode: Mode::Standalone,
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        openai_api_key: String::new(),
        openai_base_url: String::new(),
        openai_model: String::new(),
        database_url: String::new(),
        redis_url: String::new(),
        standalone_sqlite_path: dir.path().join("standalone.sqlite").display().to_string(),
        execution_root: String::new(),
        runtime_root: String::new(),
        hook_source: None,
    };

    let first = bootstrap_standalone(&config).await.unwrap();
    let err = match bootstrap_standalone(&config).await {
        Ok(_) => panic!("second bootstrap should fail"),
        Err(err) => err,
    };

    assert_eq!(first.mode(), Mode::Standalone);
    match err {
        santi_core::error::Error::Internal { message } => {
            assert!(message.contains("standalone bootstrap lock already held"));
        }
        other => panic!("unexpected error: {:?}", other),
    }
}

#[tokio::test]
async fn standalone_query_service_lists_compacts_via_fork_compact_store() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("standalone.sqlite");

    let store = Arc::new(StandaloneSessionStore::new(&db_path).await.unwrap());
    let soul_store = Arc::new(StandaloneSoulStore::new(&db_path).await.unwrap());
    let soul_runtime = Arc::new(StandaloneSoulRuntime::new(&db_path).await.unwrap());
    let send_lock: Arc<dyn Lock> = Arc::new(InProcessLock::default());
    let compact_ledger = Arc::new(
        StandaloneSessionCompactStore::new(&db_path, send_lock)
            .await
            .unwrap(),
    );

    store.create_session("sess_1").await.unwrap();
    store
        .append_user_message("sess_1", "hello standalone")
        .await
        .unwrap();
    compact_ledger
        .compact_session("sess_1", "standalone compact")
        .await
        .unwrap();

    let query = SessionQueryService::new(
        store,
        soul_store,
        soul_runtime,
        compact_ledger,
        "soul_default".to_string(),
    );

    let compacts = query.list_session_compacts("sess_1").await.unwrap();
    assert_eq!(compacts.len(), 1);
    assert_eq!(compacts[0].summary, "standalone compact");
}
