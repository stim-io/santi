use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RuntimeSelfFacts {
    pub service_name: String,
    pub assembly_mode: String,
    pub launch_profile: Option<String>,
    pub bind_addr: Option<String>,
    pub provider_model: String,
    pub provider_api: String,
    pub provider_gateway_base_url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ToolRuntimeContext {
    pub session_id: String,
    pub soul_id: String,
    pub soul_memory_dir: PathBuf,
    pub session_memory_dir: PathBuf,
    pub fallback_cwd: PathBuf,
}
