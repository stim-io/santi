use santi_api::schema::session::SessionResponse;
use santi_db::adapter::standalone::{
    session_store::StandaloneSessionLedger, soul_store::StandaloneSoul,
};

#[tokio::test]
async fn standalone_session_create_and_get_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("standalone.sqlite");
    let store = std::sync::Arc::new(StandaloneSessionLedger::new(&path).await.unwrap());
    let session = store.create_session("session_1").await.unwrap();
    assert_eq!(session.id, "session_1");

    let loaded = store.get_session("session_1").await.unwrap().unwrap();
    assert_eq!(loaded.id, "session_1");
}

#[tokio::test]
async fn standalone_response_mapping_keeps_session_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("standalone.sqlite");
    let store = StandaloneSessionLedger::new(&path).await.unwrap();
    let session = store.create_session("session_2").await.unwrap();
    let response = SessionResponse::from(session);
    assert_eq!(response.id, "session_2");
}

#[tokio::test]
async fn standalone_session_message_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("standalone.sqlite");
    let store = StandaloneSessionLedger::new(&path).await.unwrap();
    store.create_session("session_3").await.unwrap();

    let appended = store
        .append_user_message("session_3", "hello")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(appended.message.actor_id, "user");

    let messages = store.list_messages("session_3").await.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message.actor_id, "user");
}

#[tokio::test]
async fn standalone_default_soul_is_persisted_on_first_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("standalone.sqlite");
    let store = StandaloneSoul::new(&path).await.unwrap();

    let soul = store.get_default_soul().await.unwrap();
    assert_eq!(soul.id, "soul_default");
    assert!(soul.memory.is_empty());
    assert!(!soul.created_at.is_empty());
    assert!(!soul.updated_at.is_empty());

    let reloaded = store.get_default_soul().await.unwrap();
    assert_eq!(reloaded.id, "soul_default");
    assert!(!reloaded.created_at.is_empty());
    assert!(!reloaded.updated_at.is_empty());
}
