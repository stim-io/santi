use axum::{http::StatusCode, response::IntoResponse, Json};

use crate::schema::health::HealthResponse;

#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "health",
    responses(
        (status = 200, description = "Health check response", body = HealthResponse)
    )
)]
pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(HealthResponse::ok()))
}
