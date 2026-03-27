use std::sync::Arc;

use santi_core::{
    error::Error,
    model::{session::Session, session::SessionMessage, soul::Soul},
    port::{session_ledger::SessionLedgerPort, soul::SoulPort},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionQueryService {
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_port: Arc<dyn SoulPort>,
    default_soul_id: String,
}

impl SessionQueryService {
    pub fn new(
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_port: Arc<dyn SoulPort>,
        default_soul_id: String,
    ) -> Self {
        Self {
            session_ledger,
            soul_port,
            default_soul_id,
        }
    }

    pub async fn create_session(&self) -> Result<Session, String> {
        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        self.session_ledger
            .create_session(&session_id)
            .await
            .map_err(render_error)
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>, String> {
        self.session_ledger
            .get_session(session_id)
            .await
            .map_err(render_error)
    }

    pub async fn list_session_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionMessage>, String> {
        self.session_ledger
            .list_messages(session_id, None)
            .await
            .map_err(render_error)
    }

    pub async fn get_default_soul(&self) -> Result<Option<Soul>, String> {
        self.soul_port
            .get_soul(&self.default_soul_id)
            .await
            .map_err(render_error)
    }
}

fn render_error(err: Error) -> String {
    match err {
        Error::NotFound { resource } => format!("{resource} not found"),
        Error::Busy { resource } => format!("{resource} busy"),
        Error::InvalidInput { message } => message,
        Error::Upstream { message } => message,
        Error::Internal { message } => message,
    }
}
