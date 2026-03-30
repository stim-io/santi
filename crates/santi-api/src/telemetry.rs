use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use std::time::Instant;
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_tracing(service_name: &'static str) {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{},tower_http=info", service_name).into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
}

pub async fn trace_http_request(mut request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let request_id = uuid::Uuid::new_v4().simple().to_string();

    let span = tracing::info_span!(
        "http_request",
        method = %method,
        path = %path,
        request_id = %request_id,
        status = tracing::field::Empty,
        latency_ms = tracing::field::Empty,
    );

    request.extensions_mut().insert(request_id.clone());

    let mut response = next.run(request).instrument(span.clone()).await;
    let latency_ms = start.elapsed().as_millis() as u64;
    let status = response.status().as_u16();

    span.record("status", tracing::field::display(status));
    span.record("latency_ms", tracing::field::display(latency_ms));

    if let Ok(value) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-santi-trace-id", value);
    }

    tracing::info!(status, latency_ms, "http request completed");

    response
}
