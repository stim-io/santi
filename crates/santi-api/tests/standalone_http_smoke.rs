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
use tower::ServiceExt;

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

#[tokio::test]
async fn standalone_http_session_create_get_and_meta_smoke() {
    let dir = tempfile::tempdir().unwrap();
    let gateway_base_url = start_mock_gateway().await;
    let config = standalone_config(
        dir.path().join("standalone.sqlite").display().to_string(),
        gateway_base_url,
    );
    let state = bootstrap_standalone(&config).await.unwrap();
    let app = build_router(state);

    let create_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_res.status(), StatusCode::CREATED);

    let create_body = axum::body::to_bytes(create_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = created.get("id").and_then(Value::as_str).unwrap();
    assert!(!session_id.is_empty());

    let get_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{session_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_res.status(), StatusCode::OK);

    let get_body = axum::body::to_bytes(get_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let fetched: Value = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(fetched.get("id").and_then(Value::as_str), Some(session_id));

    let messages_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{session_id}/messages"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(messages_res.status(), StatusCode::OK);

    let messages_body = axum::body::to_bytes(messages_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let messages: Value = serde_json::from_slice(&messages_body).unwrap();
    assert_eq!(
        messages
            .get("messages")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let send_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/send"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"content":[{"type":"text","text":"hello standalone"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(send_res.status(), StatusCode::OK);
    let send_body = axum::body::to_bytes(send_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let send_text = String::from_utf8(send_body.to_vec()).unwrap();
    assert!(send_text.contains("response.output_text.delta"));
    assert!(send_text.contains("hello from gateway"));
    assert!(send_text.contains("completed"));
    assert!(send_text.contains("[DONE]"));

    let effects_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{session_id}/effects"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(effects_res.status(), StatusCode::OK);

    let effects_body = axum::body::to_bytes(effects_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let effects: Value = serde_json::from_slice(&effects_body).unwrap();
    assert_eq!(
        effects
            .get("effects")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let messages_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{session_id}/messages"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(messages_res.status(), StatusCode::OK);
    let messages_body = axum::body::to_bytes(messages_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let messages: Value = serde_json::from_slice(&messages_body).unwrap();
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

    let compacts_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{session_id}/compacts"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compacts_res.status(), StatusCode::OK);

    let compacts_body = axum::body::to_bytes(compacts_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let compacts: Value = serde_json::from_slice(&compacts_body).unwrap();
    assert_eq!(
        compacts
            .get("compacts")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let effects_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{session_id}/effects"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(effects_res.status(), StatusCode::OK);

    let effects_body = axum::body::to_bytes(effects_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let effects: Value = serde_json::from_slice(&effects_body).unwrap();
    assert_eq!(
        effects
            .get("effects")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );

    let fork_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/fork"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"fork_point":1,"request_id":"standalone-fork"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fork_res.status(), StatusCode::CREATED);
    let fork_body = axum::body::to_bytes(fork_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let fork: Value = serde_json::from_slice(&fork_body).unwrap();
    assert_eq!(
        fork.get("parent_session_id").and_then(Value::as_str),
        Some(session_id)
    );
    let forked_session_id = fork.get("new_session_id").and_then(Value::as_str).unwrap();
    assert!(!forked_session_id.is_empty());

    let forked_messages_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{forked_session_id}/messages"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forked_messages_res.status(), StatusCode::OK);
    let forked_messages_body = axum::body::to_bytes(forked_messages_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let forked_messages: Value = serde_json::from_slice(&forked_messages_body).unwrap();
    let forked_items = forked_messages
        .get("messages")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(forked_items.len(), 1);
    assert_eq!(
        forked_items[0].get("content_text").and_then(Value::as_str),
        Some("hello standalone")
    );

    let compact_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/compact"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"summary":"standalone compact"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compact_res.status(), StatusCode::OK);
    let compact_body = axum::body::to_bytes(compact_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let compact: Value = serde_json::from_slice(&compact_body).unwrap();
    assert_eq!(
        compact.get("summary").and_then(Value::as_str),
        Some("standalone compact")
    );

    let compacts_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/sessions/{session_id}/compacts"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compacts_res.status(), StatusCode::OK);

    let compacts_body = axum::body::to_bytes(compacts_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let compacts: Value = serde_json::from_slice(&compacts_body).unwrap();
    let compacts_items = compacts.get("compacts").and_then(Value::as_array).unwrap();
    assert_eq!(compacts_items.len(), 1);
    assert_eq!(
        compacts_items[0].get("summary").and_then(Value::as_str),
        Some("standalone compact")
    );

    let missing_messages_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/sessions/missing-session/messages")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_messages_res.status(), StatusCode::NOT_FOUND);

    let missing_send_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions/missing-session/send")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"content":[{"type":"text","text":"missing"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_send_res.status(), StatusCode::NOT_FOUND);

    let missing_compacts_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/sessions/missing-session/compacts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_compacts_res.status(), StatusCode::NOT_FOUND);

    let missing_fork_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions/missing-session/fork")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"fork_point":1,"request_id":"missing-fork"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_fork_res.status(), StatusCode::NOT_FOUND);

    let missing_compact_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions/missing-session/compact")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"summary":"missing compact"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_compact_res.status(), StatusCode::NOT_FOUND);

    let meta_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/meta")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(meta_res.status(), StatusCode::OK);

    let meta_body = axum::body::to_bytes(meta_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let meta: Value = serde_json::from_slice(&meta_body).unwrap();
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

    let reload_hooks_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/admin/hooks")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"hooks":[{"id":"compact-standalone","enabled":true,"hook_point":"turn_completed","kind":"compact_threshold","params":{"min_messages_since_last_compact":2}}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reload_hooks_res.status(), StatusCode::OK);

    let reload_hooks_body = axum::body::to_bytes(reload_hooks_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let reload_hooks: Value = serde_json::from_slice(&reload_hooks_body).unwrap();
    assert_eq!(
        reload_hooks.get("hook_count").and_then(Value::as_u64),
        Some(1)
    );

    let get_soul_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/soul")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_soul_res.status(), StatusCode::OK);

    let get_soul_body = axum::body::to_bytes(get_soul_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let soul: Value = serde_json::from_slice(&get_soul_body).unwrap();
    assert_eq!(soul.get("id").and_then(Value::as_str), Some("soul_default"));

    let set_soul_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/soul/memory")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"text":"standalone soul memory"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(set_soul_res.status(), StatusCode::OK);

    let set_soul_body = axum::body::to_bytes(set_soul_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: Value = serde_json::from_slice(&set_soul_body).unwrap();
    assert_eq!(
        updated.get("memory").and_then(Value::as_str),
        Some("standalone soul memory")
    );

    let get_soul_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/soul")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_soul_res.status(), StatusCode::OK);
    let get_soul_body = axum::body::to_bytes(get_soul_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let soul: Value = serde_json::from_slice(&get_soul_body).unwrap();
    assert_eq!(
        soul.get("memory").and_then(Value::as_str),
        Some("standalone soul memory")
    );
}
