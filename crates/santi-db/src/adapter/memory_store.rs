use std::sync::Arc;

use santi_core::{
    error::Error,
    model::{session::Session, soul::Soul},
    port::memory_store::MemoryStore,
};

use crate::repo::{session_repo::SessionRepo, soul_repo::SoulRepo};

#[derive(Clone)]
pub struct RepoBackedMemoryStore {
    session_repo: Arc<SessionRepo>,
    soul_repo: Arc<SoulRepo>,
}

impl RepoBackedMemoryStore {
    pub fn new(session_repo: Arc<SessionRepo>, soul_repo: Arc<SoulRepo>) -> Self {
        Self {
            session_repo,
            soul_repo,
        }
    }
}

#[async_trait::async_trait]
impl MemoryStore for RepoBackedMemoryStore {
    async fn write_session_memory(&self, session_id: &str, text: &str) -> santi_core::error::Result<Option<Session>> {
        let mut tx = self
            .session_repo
            .begin_tx()
            .await
            .map_err(|err| Error::Internal {
                message: format!("transaction begin failed: {err}"),
            })?;

        let updated = self
            .session_repo
            .update_memory(&mut tx, session_id, text)
            .await
            .map_err(|err| Error::Internal {
                message: format!("session memory update failed: {err}"),
            })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("transaction commit failed: {err}"),
        })?;

        Ok(updated)
    }

    async fn write_soul_memory(&self, soul_id: &str, text: &str) -> santi_core::error::Result<Option<Soul>> {
        let mut tx = self
            .session_repo
            .begin_tx()
            .await
            .map_err(|err| Error::Internal {
                message: format!("transaction begin failed: {err}"),
            })?;

        let updated = self
            .soul_repo
            .update_memory(&mut tx, soul_id, text)
            .await
            .map_err(|err| Error::Internal {
                message: format!("soul memory update failed: {err}"),
            })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("transaction commit failed: {err}"),
        })?;

        Ok(updated)
    }
}
