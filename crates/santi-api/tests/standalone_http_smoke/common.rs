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

pub async fn bootstrap_test_app(gateway_base_url: String) -> (tempfile::TempDir, Router) {
    let dir = tempfile::tempdir().unwrap();
    let config = standalone_config(
        dir.path().join("standalone.sqlite").display().to_string(),
        gateway_base_url,
    );
    let state = bootstrap_standalone(&config).await.unwrap();
    (dir, build_router(state))
}

pub async fn request_json(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json = serde_json::from_slice(&body).unwrap();
    (status, json)
}

pub async fn request_text(app: &Router, request: Request<Body>) -> (StatusCode, String) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

pub async fn wait_for_reply_completion(app: &Router, reply_id: &str) -> Value {
    for _ in 0..50 {
        let (status, snapshot) = request_json(
            app,
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/stim/replies/{reply_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        if snapshot.get("status").and_then(Value::as_str) == Some("completed") {
            return snapshot;
        }

        sleep(Duration::from_millis(10)).await;
    }

    panic!("reply did not complete in time");
}

pub async fn create_session(app: &Router) -> String {
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

pub async fn start_mock_gateway() -> String {
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

pub async fn start_delayed_mock_gateway(delay_ms: u64) -> String {
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
