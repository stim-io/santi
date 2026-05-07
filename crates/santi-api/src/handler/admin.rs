use std::time::Duration;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::{
    schema::{
        admin::{
            ConfigApplyRequest, ConfigApplyResponse, ConfigCurrentResponse, HookReloadRequest,
            HookReloadResponse, ProviderProbeResponse, ProviderProbeState,
        },
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
    match state
        .admin_api()
        .reload_hooks_from_source(request.source.into())
        .await
    {
        Ok(hook_count) => (StatusCode::OK, Json(HookReloadResponse { hook_count })).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/config",
    tag = "admin",
    responses(
        (status = 200, description = "Current effective runtime config projection", body = ConfigCurrentResponse)
    )
)]
pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    (StatusCode::OK, Json(state.current_config()))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/config/apply",
    tag = "admin",
    request_body(content = ConfigApplyRequest),
    responses(
        (status = 200, description = "Runtime config applied", body = ConfigApplyResponse),
        (status = 400, description = "Invalid config payload", body = ErrorResponse)
    )
)]
pub async fn apply_config(
    State(state): State<AppState>,
    Json(request): Json<ConfigApplyRequest>,
) -> impl IntoResponse {
    match state.apply_config(request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => err.into_error_response().into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/provider/probe",
    tag = "admin",
    responses(
        (status = 200, description = "Configured provider gateway probe result", body = ProviderProbeResponse),
        (status = 500, description = "Probe client setup failed", body = ErrorResponse)
    )
)]
pub async fn probe_provider(State(state): State<AppState>) -> impl IntoResponse {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "provider_probe_client_failed",
                    format!("failed to build provider probe client: {error}"),
                )),
            )
                .into_response()
        }
    };

    let checked_url = state.provider_probe_display_url();
    match client.get(state.provider_probe_url()).send().await {
        Ok(response) if response.status().is_success() => (
            StatusCode::OK,
            Json(ProviderProbeResponse {
                state: ProviderProbeState::Ready,
                checked_url,
                http_status: Some(response.status().as_u16()),
                detail: Some("provider gateway health probe returned success".into()),
            }),
        )
            .into_response(),
        Ok(response) => (
            StatusCode::OK,
            Json(ProviderProbeResponse {
                state: ProviderProbeState::Degraded,
                checked_url,
                http_status: Some(response.status().as_u16()),
                detail: Some(format!(
                    "provider gateway health probe returned HTTP {}",
                    response.status()
                )),
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::OK,
            Json(ProviderProbeResponse {
                state: ProviderProbeState::Unreachable,
                checked_url,
                http_status: None,
                detail: Some("provider gateway health probe request failed".into()),
            }),
        )
            .into_response(),
    }
}
