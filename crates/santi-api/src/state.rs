use std::sync::Arc;

use santi_core::port::session_ledger::SessionLedgerPort;

use crate::{
    config::Mode,
    protocol_reply::ProtocolReplyStore,
    surface::{AdminApi, ApiCapabilities, SessionApi, SoulApi},
};

#[derive(Clone)]
pub struct AppState {
    mode: Mode,
    capabilities: ApiCapabilities,
    session_api: Arc<dyn SessionApi>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_api: Arc<dyn SoulApi>,
    admin_api: Arc<dyn AdminApi>,
    protocol_replies: Arc<ProtocolReplyStore>,
    standalone_bootstrap_lock: Option<Arc<std::fs::File>>,
}

impl AppState {
    pub fn new(
        mode: Mode,
        capabilities: ApiCapabilities,
        session_api: Arc<dyn SessionApi>,
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_api: Arc<dyn SoulApi>,
        admin_api: Arc<dyn AdminApi>,
        standalone_bootstrap_lock: Option<Arc<std::fs::File>>,
    ) -> Self {
        Self {
            mode,
            capabilities,
            session_api,
            session_ledger,
            soul_api,
            admin_api,
            protocol_replies: Arc::new(ProtocolReplyStore::default()),
            standalone_bootstrap_lock,
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

    pub fn session_ledger(&self) -> Arc<dyn SessionLedgerPort> {
        self.session_ledger.clone()
    }

    pub fn soul_api(&self) -> Arc<dyn SoulApi> {
        self.soul_api.clone()
    }

    pub fn admin_api(&self) -> Arc<dyn AdminApi> {
        self.admin_api.clone()
    }

    pub fn protocol_replies(&self) -> Arc<ProtocolReplyStore> {
        self.protocol_replies.clone()
    }

    pub fn standalone_bootstrap_lock(&self) -> Option<Arc<std::fs::File>> {
        self.standalone_bootstrap_lock.clone()
    }
}
