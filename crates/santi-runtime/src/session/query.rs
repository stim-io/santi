use std::sync::Arc;

use santi_core::{
    error::Error,
    model::{runtime::Compact, session::Session, session::SessionMessage, soul::Soul},
    port::{session_ledger::SessionLedgerPort, soul::SoulPort, soul_runtime::SoulRuntimePort},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionQueryService {
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_port: Arc<dyn SoulPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    default_soul_id: String,
}

impl SessionQueryService {
    pub fn new(
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_port: Arc<dyn SoulPort>,
        soul_runtime: Arc<dyn SoulRuntimePort>,
        default_soul_id: String,
    ) -> Self {
        Self {
            session_ledger,
            soul_port,
            soul_runtime,
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

    pub async fn list_session_compacts(&self, session_id: &str) -> Result<Vec<Compact>, String> {
        let soul_session = self
            .soul_runtime
            .get_soul_session_by_session_id(session_id)
            .await
            .map_err(render_error)?;
        let Some(soul_session) = soul_session else {
            return Ok(vec![]);
        };

        let items = self
            .soul_runtime
            .list_assembly_items(&soul_session.id, None)
            .await
            .map_err(render_error)?;
        Ok(items
            .into_iter()
            .filter_map(|item| match item.target {
                santi_core::model::runtime::AssemblyTarget::Compact(compact) => Some(compact),
                _ => None,
            })
            .collect())
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
