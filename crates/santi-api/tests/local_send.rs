use std::sync::Arc;

use santi_core::port::{lock::Lock, session_ledger::SessionLedgerPort};
use santi_db::adapter::local_session_store::LocalSessionStore;
use santi_db::adapter::local_soul_runtime::LocalSoulRuntime;
use santi_lock::InProcessLock;
use santi_runtime::session::local_send::{LocalSendError, LocalSessionSendService};

#[tokio::test]
async fn local_send_returns_busy_when_same_session_lock_is_held() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("local.sqlite");
    let store = Arc::new(LocalSessionStore::new(&path).await.unwrap());
    let soul_runtime = Arc::new(LocalSoulRuntime::new(&path).await.unwrap());
    store.create_session("session_busy").await.unwrap();

    let lock = Arc::new(InProcessLock::default());
    let held = lock
        .acquire("lock:session_send:session_busy")
        .await
        .unwrap();
    let session_ledger: Arc<dyn SessionLedgerPort> = store;
    let service = LocalSessionSendService::new(lock, session_ledger, soul_runtime);

    let result = service.send_text("session_busy", "hello").await;
    assert!(matches!(result, Err(LocalSendError::Busy)));

    held.release().await.unwrap();
}
