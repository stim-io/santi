use serde::Serialize;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MetaCapabilities {
    pub health: bool,
    pub sessions: bool,
    pub soul: bool,
    pub admin_hooks: bool,
    pub streaming: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MetaResponse {
    pub api_version: String,
    pub service_name: String,
    pub service_version: String,
    pub compatible_cli_xy: String,
    pub mode: String,
    pub capabilities: MetaCapabilities,
}
