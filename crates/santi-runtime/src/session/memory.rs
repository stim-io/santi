use std::sync::Arc;

use santi_core::{
    error::Error,
    model::{runtime::SoulSession, soul::Soul},
    port::{
        soul::SoulPort, soul_runtime::SoulRuntimePort, soul_session_query::SoulSessionQueryPort,
    },
};

#[derive(Clone)]
pub struct SessionMemoryService {
    soul_session_query: Arc<dyn SoulSessionQueryPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    soul_port: Arc<dyn SoulPort>,
    default_soul_id: String,
}

impl SessionMemoryService {
    pub fn new(
        soul_runtime: Arc<dyn SoulRuntimePort>,
        soul_session_query: Arc<dyn SoulSessionQueryPort>,
        soul_port: Arc<dyn SoulPort>,
        default_soul_id: String,
    ) -> Self {
        Self {
            soul_session_query,
            soul_runtime,
            soul_port,
            default_soul_id,
        }
    }

    pub async fn write_session_memory(
        &self,
        session_id: &str,
        text: &str,
    ) -> Result<Option<SoulSession>, String> {
        let soul_session = self
            .soul_runtime
            .acquire_soul_session(santi_core::port::soul_runtime::AcquireSoulSession {
                soul_id: self.default_soul_id.clone(),
                session_id: session_id.to_string(),
            })
            .await
            .map_err(render_error)?;

        self.soul_runtime
            .write_session_memory(&soul_session.id, text)
            .await
            .map_err(render_error)
    }

    pub async fn get_session_memory(
        &self,
        session_id: &str,
    ) -> Result<Option<SoulSession>, String> {
        self.soul_session_query
            .get_soul_session_by_session_id(session_id)
            .await
            .map_err(render_error)
    }

    pub async fn write_soul_memory(
        &self,
        soul_id: &str,
        text: &str,
    ) -> Result<Option<Soul>, String> {
        self.soul_port
            .write_soul_memory(soul_id, text)
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
