use axum::{body::Body, http::Request, http::StatusCode};
use serde_json::Value;
use tokio::time::{sleep, Duration};
use tower::ServiceExt;

use crate::common::{
    bootstrap_test_app, create_session, request_json, request_text, start_delayed_mock_gateway,
    start_mock_gateway, start_tool_call_mock_gateway,
};

#[tokio::test]
async fn standalone_http_fork_and_compact_flows_work() {
    let (_dir, app) = bootstrap_test_app(start_mock_gateway().await).await;
    let session_id = create_session(&app).await;

    let (status, _) = request_text(
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

    let (status, fork) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/sessions/{session_id}/fork"))
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"fork_point":1,"request_id":"standalone-fork"}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        fork.get("parent_session_id").and_then(Value::as_str),
        Some(session_id.as_str())
    );
    let forked_session_id = fork.get("new_session_id").and_then(Value::as_str).unwrap();
    assert!(!forked_session_id.is_empty());

    let (status, forked_messages) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/sessions/{forked_session_id}/messages"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let forked_items = forked_messages
        .get("messages")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(forked_items.len(), 1);
    assert_eq!(
        forked_items[0].get("content_text").and_then(Value::as_str),
        Some("hello standalone")
    );

    let (status, compact) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/sessions/{session_id}/compact"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"summary":"standalone compact"}"#))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        compact.get("summary").and_then(Value::as_str),
        Some("standalone compact")
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
    let compacts_items = compacts.get("compacts").and_then(Value::as_array).unwrap();
    assert_eq!(compacts_items.len(), 1);
    assert_eq!(
        compacts_items[0].get("summary").and_then(Value::as_str),
        Some("standalone compact")
    );
}

#[tokio::test]
async fn standalone_http_exposes_session_tool_activity_summaries() {
    let (_dir, app) = bootstrap_test_app(start_tool_call_mock_gateway().await).await;
    let session_id = create_session(&app).await;

    let (status, _) = request_text(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/sessions/{session_id}/send"))
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"content":[{"type":"text","text":"please use a tool"}]}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, tool_activities) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/sessions/{session_id}/tool-activities"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = tool_activities
        .get("tool_activities")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].get("tool_name").and_then(Value::as_str),
        Some("bash")
    );

    assert_bash_tool_activity_summary(&items[0]);
}

#[cfg(not(windows))]
fn assert_bash_tool_activity_summary(item: &Value) {
    assert_eq!(
        item.get("result_state").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(item.get("exit_code").and_then(Value::as_i64), Some(0));
    assert_eq!(
        item.get("output_summary").and_then(Value::as_str),
        Some("bash exit 0; stdout 12 chars; stderr 0 chars")
    );
}

#[cfg(windows)]
fn assert_bash_tool_activity_summary(item: &Value) {
    assert_eq!(
        item.get("result_state").and_then(Value::as_str),
        Some("tool-error")
    );
    assert_eq!(item.get("exit_code").and_then(Value::as_i64), None);
    assert_eq!(
        item.get("output_summary").and_then(Value::as_str),
        Some("ok false")
    );
}

#[tokio::test]
async fn standalone_http_missing_session_routes_return_not_found() {
    let (_dir, app) = bootstrap_test_app(start_mock_gateway().await).await;

    let (status, _) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/sessions/missing-session/messages")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/sessions/missing-session/tool-activities")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/sessions/missing-session/watch-snapshot")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/sessions/missing-session/watch")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/sessions/missing-session/send")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"content":[{"type":"text","text":"missing"}]}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/sessions/missing-session/compacts")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/sessions/missing-session/fork")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"fork_point":1,"request_id":"missing-fork"}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/sessions/missing-session/compact")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"summary":"missing compact"}"#))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn standalone_http_concurrent_send_returns_conflict() {
    let (_dir, app) = bootstrap_test_app(start_delayed_mock_gateway(200).await).await;
    let session_id = create_session(&app).await;

    let app_for_first = app.clone();
    let first_session_id = session_id.clone();
    let first = tokio::spawn(async move {
        app_for_first
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{first_session_id}/send"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"content":[{"type":"text","text":"first concurrent send"}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    });

    sleep(Duration::from_millis(25)).await;

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/send"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"content":[{"type":"text","text":"second concurrent send"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let first = first.await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::CONFLICT);

    let second_body = axum::body::to_bytes(second.into_body(), usize::MAX)
        .await
        .unwrap();
    let second_json: Value = serde_json::from_slice(&second_body).unwrap();
    assert_eq!(
        second_json.pointer("/error/code").and_then(Value::as_str),
        Some("conflict")
    );
    assert_eq!(
        second_json
            .pointer("/error/message")
            .and_then(Value::as_str),
        Some("session send already in progress")
    );
}
