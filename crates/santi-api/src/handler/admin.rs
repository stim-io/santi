use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::{
    schema::{
        admin::{HookReloadRequest, HookReloadResponse},
        common::ErrorResponse,
    },
    state::AppState,
};

#[utoipa::path(
    put,
    path = "/api/v1/admin/hooks",
    tag = "admin",
    request_body(content = HookReloadRequest),
    responses(
        (status = 200, description = "Hook registry replaced", body = HookReloadResponse),
        (status = 400, description = "Invalid hook payload", body = ErrorResponse)
    )
)]
pub async fn reload_hooks(
    State(state): State<AppState>,
    Json(request): Json<HookReloadRequest>,
) -> impl IntoResponse {
    match state.reload_hooks_from_source(request.source.into()).await {
        Ok(hook_count) => (StatusCode::OK, Json(HookReloadResponse { hook_count })).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::new(err))).into_response(),
    }
}
