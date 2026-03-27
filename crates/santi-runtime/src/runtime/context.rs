use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ToolRuntimeContext {
    pub session_id: String,
    pub soul_id: String,
    pub soul_memory_dir: PathBuf,
    pub session_memory_dir: PathBuf,
    pub fallback_cwd: PathBuf,
}
