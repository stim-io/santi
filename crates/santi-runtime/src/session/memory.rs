use std::sync::Arc;

use santi_core::model::{session::Session, soul::Soul};
use santi_db::repo::{session_repo::SessionRepo, soul_repo::SoulRepo};

#[derive(Clone)]
pub struct SessionMemoryService {
    session_repo: Arc<SessionRepo>,
    soul_repo: Arc<SoulRepo>,
}

impl SessionMemoryService {
    pub fn new(session_repo: Arc<SessionRepo>, soul_repo: Arc<SoulRepo>) -> Self {
        Self { session_repo, soul_repo }
    }

    pub async fn write_session_memory(
        &self,
        session_id: &str,
        text: &str,
    ) -> Result<Option<Session>, String> {
        let mut tx = self
            .session_repo
            .begin_tx()
            .await
            .map_err(|err| format!("transaction begin failed: {err}"))?;

        let updated = self
            .session_repo
            .update_memory(&mut tx, session_id, text)
            .await
            .map_err(|err| format!("session memory update failed: {err}"))?;

        tx.commit()
            .await
            .map_err(|err| format!("transaction commit failed: {err}"))?;

        Ok(updated)
    }

    pub async fn write_soul_memory(&self, soul_id: &str, text: &str) -> Result<Option<Soul>, String> {
        let mut tx = self
            .session_repo
            .begin_tx()
            .await
            .map_err(|err| format!("transaction begin failed: {err}"))?;

        let updated = self
            .soul_repo
            .update_memory(&mut tx, soul_id, text)
            .await
            .map_err(|err| format!("soul memory update failed: {err}"))?;

        tx.commit()
            .await
            .map_err(|err| format!("transaction commit failed: {err}"))?;

        Ok(updated)
    }
}
