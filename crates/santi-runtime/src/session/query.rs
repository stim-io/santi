use std::sync::Arc;

use santi_core::{
    error::Error,
    model::{message::Message, session::Session, soul::Soul},
    port::session_query::SessionQueryPort,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionQueryService {
    query_port: Arc<dyn SessionQueryPort>,
    default_soul_id: String,
}

impl SessionQueryService {
    pub fn new(query_port: Arc<dyn SessionQueryPort>, default_soul_id: String) -> Self {
        Self {
            query_port,
            default_soul_id,
        }
    }

    pub async fn create_session(&self) -> Result<Session, String> {
        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        self.query_port
            .create_session(&session_id, &self.default_soul_id)
            .await
            .map_err(render_error)
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>, String> {
        self.query_port
            .get_session(session_id)
            .await
            .map_err(render_error)
    }

    pub async fn list_session_messages(&self, session_id: &str) -> Result<Vec<Message>, String> {
        self.query_port
            .list_session_messages(session_id)
            .await
            .map_err(render_error)
    }

    pub async fn get_default_soul(&self) -> Result<Option<Soul>, String> {
        self.query_port
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
