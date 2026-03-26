use std::sync::Arc;

use santi_core::{
    error::Error,
    model::{session::Session, soul::Soul},
    port::memory_store::MemoryStore,
};

#[derive(Clone)]
pub struct SessionMemoryService {
    memory_store: Arc<dyn MemoryStore>,
}

impl SessionMemoryService {
    pub fn new(memory_store: Arc<dyn MemoryStore>) -> Self {
        Self { memory_store }
    }

    pub async fn write_session_memory(
        &self,
        session_id: &str,
        text: &str,
    ) -> Result<Option<Session>, String> {
        self.memory_store
            .write_session_memory(session_id, text)
            .await
            .map_err(render_error)
    }

    pub async fn write_soul_memory(&self, soul_id: &str, text: &str) -> Result<Option<Soul>, String> {
        self.memory_store
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
