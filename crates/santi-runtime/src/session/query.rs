use std::sync::Arc;

use santi_core::{
    error::Error,
    model::{
        runtime::{Compact, ToolActivity},
        session::Session,
        session::SessionMessage,
        soul::Soul,
    },
    port::{
        compact_ledger::CompactLedgerPort, session_ledger::SessionLedgerPort, soul::SoulPort,
        soul_session_query::SoulSessionQueryPort,
    },
};
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionQueryService {
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_port: Arc<dyn SoulPort>,
    soul_session_query: Arc<dyn SoulSessionQueryPort>,
    compact_ledger: Arc<dyn CompactLedgerPort>,
    default_soul_id: String,
}

impl SessionQueryService {
    pub fn new(
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_port: Arc<dyn SoulPort>,
        soul_session_query: Arc<dyn SoulSessionQueryPort>,
        compact_ledger: Arc<dyn CompactLedgerPort>,
        default_soul_id: String,
    ) -> Self {
        Self {
            session_ledger,
            soul_port,
            soul_session_query,
            compact_ledger,
            default_soul_id,
        }
    }

    pub async fn create_session(&self) -> Result<Session, String> {
        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        self.create_session_with_id(&session_id).await
    }

    pub async fn create_session_with_id(&self, session_id: &str) -> Result<Session, String> {
        self.session_ledger
            .create_session(session_id)
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
        let Some(soul_session) = self
            .soul_session_query
            .get_session_soul(session_id)
            .await
            .map_err(render_error)?
        else {
            return Ok(vec![]);
        };

        self.compact_ledger
            .list_compacts(&soul_session.id)
            .await
            .map_err(render_error)
    }

    pub async fn list_session_tool_activities(
        &self,
        session_id: &str,
    ) -> Result<Vec<ToolActivity>, String> {
        let Some(soul_session) = self
            .soul_session_query
            .get_session_soul(session_id)
            .await
            .map_err(render_error)?
        else {
            return Ok(vec![]);
        };

        self.soul_session_query
            .list_tool_activities(&soul_session.id)
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
