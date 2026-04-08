use axum::{http::StatusCode, response::IntoResponse, Json};

use crate::schema::meta::{MetaCapabilities, MetaResponse};

#[utoipa::path(
    get,
    path = "/api/v1/meta",
    tag = "meta",
    responses(
        (status = 200, description = "Service metadata", body = MetaResponse)
    )
)]
pub async fn meta(
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
) -> impl IntoResponse {
    let version = env!("CARGO_PKG_VERSION");
    let compatible_cli_xy = version.split('.').take(2).collect::<Vec<_>>().join(".");

    (
        StatusCode::OK,
        Json(MetaResponse {
            api_version: "v1".to_string(),
            service_name: env!("CARGO_PKG_NAME").to_string(),
            service_version: version.to_string(),
            compatible_cli_xy,
            mode: match state.mode() {
                crate::config::Mode::Distributed => "distributed".to_string(),
                crate::config::Mode::Standalone => "standalone".to_string(),
            },
            capabilities: MetaCapabilities {
                health: state.capabilities().health,
                sessions: state.capabilities().sessions,
                soul: state.capabilities().soul,
                admin_hooks: state.capabilities().admin_hooks,
                streaming: state.capabilities().streaming,
            },
        }),
    )
}
