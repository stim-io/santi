use axum::{body::Body, http::Request, http::StatusCode};
use serde_json::Value;

use crate::common::{
    bootstrap_test_app, create_session, request_json, request_text, start_mock_gateway,
};

#[tokio::test]
async fn standalone_http_main_path_smoke() {
    let (_dir, app) = bootstrap_test_app(start_mock_gateway().await).await;
    let session_id = create_session(&app).await;

    let (status, fetched) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/sessions/{session_id}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        fetched.get("id").and_then(Value::as_str),
        Some(session_id.as_str())
    );

    let (status, messages) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/sessions/{session_id}/messages"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        messages
            .get("messages")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let (status, watch_snapshot) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/sessions/{session_id}/watch-snapshot"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        watch_snapshot.get("session_id").and_then(Value::as_str),
        Some(session_id.as_str())
    );
    assert_eq!(
        watch_snapshot.get("latest_seq").and_then(Value::as_i64),
        Some(0)
    );
    assert_eq!(
        watch_snapshot
            .get("messages")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        watch_snapshot
            .get("effects")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let (status, send_text) = request_text(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/sessions/{session_id}/send"))
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"content":[{"type":"text","text":"hello standalone"}]}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(send_text.contains("response.output_text.delta"));
    assert!(send_text.contains("hello from gateway"));
    assert!(send_text.contains("completed"));
    assert!(send_text.contains("[DONE]"));

    let (status, messages) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/sessions/{session_id}/messages"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = messages.get("messages").and_then(Value::as_array).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].get("content_text").and_then(Value::as_str),
        Some("hello standalone")
    );
    assert_eq!(
        items[1].get("actor_type").and_then(Value::as_str),
        Some("soul")
    );
    assert_eq!(
        items[1].get("content_text").and_then(Value::as_str),
        Some("hello from gateway")
    );

    let (status, watch_snapshot) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/sessions/{session_id}/watch-snapshot"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        watch_snapshot.get("latest_seq").and_then(Value::as_i64),
        Some(2)
    );
    let watch_messages = watch_snapshot
        .get("messages")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(watch_messages.len(), 2);
    assert_eq!(
        watch_messages[0]
            .get("content_text")
            .and_then(Value::as_str),
        Some("hello standalone")
    );
    assert_eq!(
        watch_messages[1]
            .get("content_text")
            .and_then(Value::as_str),
        Some("hello from gateway")
    );

    let (status, effects) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/sessions/{session_id}/effects"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        effects
            .get("effects")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let (status, compacts) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/sessions/{session_id}/compacts"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        compacts
            .get("compacts")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}
