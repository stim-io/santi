use axum::{body::Body, http::Request, http::StatusCode};
use serde_json::Value;

use crate::common::{bootstrap_test_app, request_json, start_mock_gateway};

#[tokio::test]
async fn standalone_http_meta_admin_and_soul_routes_work() {
    let (_dir, app) = bootstrap_test_app(start_mock_gateway().await).await;

    let (status, meta) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/meta")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(meta.get("mode").and_then(Value::as_str), Some("standalone"));
    assert_eq!(
        meta.pointer("/capabilities/health")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        meta.pointer("/capabilities/sessions")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        meta.pointer("/capabilities/soul").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        meta.pointer("/capabilities/streaming")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        meta.pointer("/capabilities/admin_hooks")
            .and_then(Value::as_bool),
        Some(true)
    );

    let (status, reload_hooks) = request_json(
        &app,
        Request::builder()
            .method("PUT")
            .uri("/api/v1/admin/hooks")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"hooks":[{"id":"compact-standalone","enabled":true,"hook_point":"turn_completed","kind":"compact_threshold","params":{"min_messages_since_last_compact":2}}]}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        reload_hooks.get("hook_count").and_then(Value::as_u64),
        Some(1)
    );

    let (status, soul) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/soul")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(soul.get("id").and_then(Value::as_str), Some("soul_default"));

    let (status, updated) = request_json(
        &app,
        Request::builder()
            .method("PUT")
            .uri("/api/v1/soul/memory")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"text":"standalone soul memory"}"#))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        updated.get("memory").and_then(Value::as_str),
        Some("standalone soul memory")
    );

    let (status, soul) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/soul")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        soul.get("memory").and_then(Value::as_str),
        Some("standalone soul memory")
    );
}
