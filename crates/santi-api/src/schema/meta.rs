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
    pub launch_profile: Option<String>,
    pub bind_addr: Option<String>,
    pub provider: MetaProvider,
    pub runtime: MetaRuntime,
    pub capabilities: MetaCapabilities,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MetaProvider {
    pub api: String,
    pub model: String,
    pub gateway_base_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MetaRuntime {
    pub execution_root: String,
    pub runtime_root: String,
    pub standalone_sqlite_path: Option<String>,
}
