use santi_api::{
    bootstrap_local::bootstrap_local,
    config::{Config, Mode},
};

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
