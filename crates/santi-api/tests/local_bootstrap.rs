use santi_api::{
    bootstrap_local::bootstrap_local,
    config::{Config, Mode},
};
use santi_core::port::lock::Lock;
use santi_db::adapter::local::{
    session_fork_compact::LocalSessionForkCompactStore, session_store::LocalSessionStore,
    soul_runtime::LocalSoulRuntime, soul_store::LocalSoulStore,
};
use santi_lock::InProcessLock;
use santi_runtime::session::query::SessionQueryService;
use std::sync::Arc;

#[tokio::test]
async fn local_bootstrap_injects_session_store() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        mode: Mode::Local,
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        openai_api_key: String::new(),
        openai_base_url: String::new(),
        openai_model: String::new(),
        database_url: String::new(),
        redis_url: String::new(),
        local_sqlite_path: dir.path().join("local.sqlite").display().to_string(),
        execution_root: String::new(),
        runtime_root: String::new(),
        hook_source: None,
    };

    let state = bootstrap_local(&config).await.unwrap();
    assert_eq!(state.mode(), Mode::Local);
    assert!(state.local_bootstrap_lock().is_some());
    assert!(state.capabilities().admin_hooks);
}

#[tokio::test]
async fn local_bootstrap_fails_when_lock_is_held() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config {
        mode: Mode::Local,
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        openai_api_key: String::new(),
        openai_base_url: String::new(),
        openai_model: String::new(),
        database_url: String::new(),
        redis_url: String::new(),
        local_sqlite_path: dir.path().join("local.sqlite").display().to_string(),
        execution_root: String::new(),
        runtime_root: String::new(),
        hook_source: None,
    };

    let first = bootstrap_local(&config).await.unwrap();
    let err = match bootstrap_local(&config).await {
        Ok(_) => panic!("second bootstrap should fail"),
        Err(err) => err,
    };

    assert_eq!(first.mode(), Mode::Local);
    match err {
        santi_core::error::Error::Internal { message } => {
            assert!(message.contains("local bootstrap lock already held"));
        }
        other => panic!("unexpected error: {:?}", other),
    }
}

#[tokio::test]
async fn local_query_service_lists_compacts_via_fork_compact_store() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("local.sqlite");

    let store = Arc::new(LocalSessionStore::new(&db_path).await.unwrap());
    let soul_store = Arc::new(LocalSoulStore::new(&db_path).await.unwrap());
    let soul_runtime = Arc::new(LocalSoulRuntime::new(&db_path).await.unwrap());
    let send_lock: Arc<dyn Lock> = Arc::new(InProcessLock::default());
    let fork_compact = Arc::new(LocalSessionForkCompactStore::new(&db_path, send_lock).await.unwrap());

    store.create_session("sess_1").await.unwrap();
    store.append_user_message("sess_1", "hello local").await.unwrap();
    fork_compact.compact_session("sess_1", "local compact").await.unwrap();

    let query = SessionQueryService::new(
        store,
        soul_store,
        soul_runtime,
        fork_compact,
        "soul_default".to_string(),
    );

    let compacts = query.list_session_compacts("sess_1").await.unwrap();
    assert_eq!(compacts.len(), 1);
    assert_eq!(compacts[0].summary, "local compact");
}
