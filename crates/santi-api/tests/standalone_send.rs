use std::sync::Arc;

use santi_core::port::{lock::Lock, session_ledger::SessionLedgerPort};
use santi_db::adapter::standalone::{
    session_store::StandaloneSessionStore, soul_runtime::StandaloneSoulRuntime,
};
use santi_lock::adapter::standalone::InProcessLock;
use santi_runtime::session::standalone_send::{StandaloneSendError, StandaloneSessionSendService};

#[tokio::test]
async fn standalone_send_returns_busy_when_same_session_lock_is_held() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("standalone.sqlite");
    let store = Arc::new(StandaloneSessionStore::new(&path).await.unwrap());
    let soul_runtime = Arc::new(StandaloneSoulRuntime::new(&path).await.unwrap());
    store.create_session("session_busy").await.unwrap();

    let lock = Arc::new(InProcessLock::default());
    let held = lock
        .acquire("lock:session_send:session_busy")
        .await
        .unwrap();
    let session_ledger: Arc<dyn SessionLedgerPort> = store;
    let service = StandaloneSessionSendService::new(lock, session_ledger, soul_runtime);

    let result = service.send_text("session_busy", "hello").await;
    assert!(matches!(result, Err(StandaloneSendError::Busy)));

    held.release().await.unwrap();
}
