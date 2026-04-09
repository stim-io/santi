use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use santi_api::{
    app::build_router,
    bootstrap_standalone::bootstrap_standalone,
    config::{Config, Mode},
};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::time::{sleep, Duration};
use tower::ServiceExt;

async fn bootstrap_test_app(gateway_base_url: String) -> (tempfile::TempDir, Router) {
    let dir = tempfile::tempdir().unwrap();
    let config = standalone_config(
        dir.path().join("standalone.sqlite").display().to_string(),
        gateway_base_url,
    );
    let state = bootstrap_standalone(&config).await.unwrap();
    (dir, build_router(state))
}

async fn request_json(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json = serde_json::from_slice(&body).unwrap();
    (status, json)
}

async fn request_text(app: &Router, request: Request<Body>) -> (StatusCode, String) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn create_session(app: &Router) -> String {
    let (status, created) = request_json(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/sessions")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    created
        .get("id")
        .and_then(Value::as_str)
        .unwrap()
        .to_string()
}

fn standalone_config(path: String, gateway_base_url: String) -> Config {
    Config {
        mode: Mode::Standalone,
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        openai_api_key: "test-key".to_string(),
        openai_base_url: gateway_base_url,
        openai_model: "gpt-5.4".to_string(),
        database_url: String::new(),
        redis_url: String::new(),
        standalone_sqlite_path: path,
        execution_root: String::new(),
        runtime_root: String::new(),
        hook_source: None,
    }
}

async fn start_mock_gateway() -> String {
    async fn responses() -> impl IntoResponse {
        let body = concat!(
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_test_1\"}}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hello from gateway\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test_1\"}}\n\n",
            "data: [DONE]\n\n"
        );

        ([("content-type", "text/event-stream")], body)
    }

    let app = Router::new().route("/openai/v1/responses", post(responses));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{addr}/openai/v1")
}

async fn start_delayed_mock_gateway(delay_ms: u64) -> String {
    async fn responses(delay_ms: u64) -> impl IntoResponse {
        sleep(Duration::from_millis(delay_ms)).await;
        let body = concat!(
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_test_1\"}}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"delayed gateway\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_test_1\"}}\n\n",
            "data: [DONE]\n\n"
        );

        ([("content-type", "text/event-stream")], body)
    }

    let app = Router::new().route("/openai/v1/responses", post(move || responses(delay_ms)));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{addr}/openai/v1")
}

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
