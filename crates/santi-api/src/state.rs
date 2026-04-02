use std::sync::Arc;

use crate::{
    config::Mode,
    surface::{AdminApi, ApiCapabilities, SessionApi, SoulApi},
};

#[derive(Clone)]
pub struct AppState {
    mode: Mode,
    capabilities: ApiCapabilities,
    session_api: Arc<dyn SessionApi>,
    soul_api: Arc<dyn SoulApi>,
    admin_api: Arc<dyn AdminApi>,
    local_bootstrap_lock: Option<Arc<std::fs::File>>,
}

impl AppState {
    pub fn new(
        mode: Mode,
        capabilities: ApiCapabilities,
        session_api: Arc<dyn SessionApi>,
        soul_api: Arc<dyn SoulApi>,
        admin_api: Arc<dyn AdminApi>,
        local_bootstrap_lock: Option<Arc<std::fs::File>>,
    ) -> Self {
        Self {
            mode,
            capabilities,
            session_api,
            soul_api,
            admin_api,
            local_bootstrap_lock,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode.clone()
    }

    pub fn capabilities(&self) -> &ApiCapabilities {
        &self.capabilities
    }

    pub fn session_api(&self) -> Arc<dyn SessionApi> {
        self.session_api.clone()
    }

    pub fn soul_api(&self) -> Arc<dyn SoulApi> {
        self.soul_api.clone()
    }

    pub fn admin_api(&self) -> Arc<dyn AdminApi> {
        self.admin_api.clone()
    }

    pub fn local_bootstrap_lock(&self) -> Option<Arc<std::fs::File>> {
        self.local_bootstrap_lock.clone()
    }
}
