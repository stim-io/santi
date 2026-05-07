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
    let capabilities = state.capabilities();

    (
        StatusCode::OK,
        Json(MetaResponse {
            api_version: "v1".to_string(),
            service_name: env!("CARGO_PKG_NAME").to_string(),
            service_version: version.to_string(),
            compatible_cli_xy,
            mode: state.mode().as_str().to_string(),
            launch_profile: state.launch_profile(),
            bind_addr: Some(state.bind_addr()),
            provider: state.provider(),
            runtime: state.runtime(),
            capabilities: MetaCapabilities {
                health: capabilities.health,
                sessions: capabilities.sessions,
                soul: capabilities.soul,
                admin_hooks: capabilities.admin_hooks,
                streaming: capabilities.streaming,
            },
        }),
    )
}
