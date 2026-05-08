use axum::{body::Body, http::Request, http::StatusCode};
use serde_json::Value;

use crate::common::{
    bootstrap_test_app, create_session, request_json, request_text, start_mock_gateway,
    start_text_mock_gateway,
};

#[tokio::test]
async fn provider_probe_reports_health() {
    let gateway = start_mock_gateway().await;
    let (_dir, app) = bootstrap_test_app(gateway.clone()).await;

    let (status, json) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/admin/provider/probe")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(json.get("state").and_then(Value::as_str), Some("ready"));
    assert_eq!(json.get("http_status").and_then(Value::as_u64), Some(200));
    let expected_checked_url = format!("{gateway}/health");
    assert_eq!(
        json.get("checked_url").and_then(Value::as_str),
        Some(expected_checked_url.as_str())
    );
}

#[tokio::test]
async fn config_apply_reloads_provider() {
    let first_gateway = start_text_mock_gateway("before reload").await;
    let second_gateway = start_text_mock_gateway("after reload").await;
    let (_dir, app) = bootstrap_test_app(first_gateway).await;
    let session_id = create_session(&app).await;

    let (status, current) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/admin/config")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        current.get("config_version").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        current.get("source").and_then(Value::as_str),
        Some("startup")
    );
    assert_eq!(
        current.pointer("/provider/model").and_then(Value::as_str),
        Some("gpt-5.4")
    );

    let (status, response) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/admin/config/apply")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{
                    "launch_profile":"test-hot-reload",
                    "provider":{{
                        "api":"responses",
                        "model":"gpt-reloaded",
                        "gateway_base_url":{gateway},
                        "api_key":"reloaded-test-key"
                    }}
                }}"#,
                gateway = serde_json::to_string(&second_gateway).unwrap()
            )))
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        response.get("status").and_then(Value::as_str),
        Some("applied")
    );
    assert_eq!(
        response.get("config_version").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        response.get("source").and_then(Value::as_str),
        Some("admin-apply")
    );
    assert_eq!(
        response.pointer("/provider/model").and_then(Value::as_str),
        Some("gpt-reloaded")
    );
    assert!(response.get("event_id").and_then(Value::as_str).is_some());

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
    assert_eq!(
        meta.get("launch_profile").and_then(Value::as_str),
        Some("test-hot-reload")
    );
    assert_eq!(
        meta.pointer("/provider/model").and_then(Value::as_str),
        Some("gpt-reloaded")
    );

    let (status, current) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/admin/config")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        current.get("config_version").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        current.get("last_event_id").and_then(Value::as_str),
        response.get("event_id").and_then(Value::as_str)
    );
    assert_eq!(
        current.get("source").and_then(Value::as_str),
        Some("admin-apply")
    );
    assert_eq!(
        current.pointer("/provider/model").and_then(Value::as_str),
        Some("gpt-reloaded")
    );
    assert!(!serde_json::to_string(&current)
        .unwrap()
        .contains("reloaded-test-key"));

    let (status, body) = request_text(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/sessions/{session_id}/send"))
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"content":[{"type":"text","text":"use reloaded provider"}]}"#,
            ))
            .unwrap(),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("after reload"));
}
