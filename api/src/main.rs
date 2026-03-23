use axum::middleware;
use santi_api::{app, config::Config, state::AppState, telemetry};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    telemetry::init_tracing("santi-api");
    let config = Config::from_env().expect("config load failed");
    let state = AppState::new(config.clone()).await;
    let router = app::build_router(state).layer(middleware::from_fn(telemetry::trace_http_request));

    let listener = TcpListener::bind(config.bind_addr)
        .await
        .expect("bind failed");

    axum::serve(listener, router).await.expect("serve failed");
}
