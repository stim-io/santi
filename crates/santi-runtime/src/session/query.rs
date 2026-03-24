use std::sync::Arc;

use santi_core::model::{message::Message, session::Session, soul::Soul};
use santi_db::repo::{message_repo::MessageRepo, session_repo::SessionRepo, soul_repo::SoulRepo};
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionQueryService {
    session_repo: Arc<SessionRepo>,
    soul_repo: Arc<SoulRepo>,
    message_repo: Arc<MessageRepo>,
}

impl SessionQueryService {
    pub fn new(
        session_repo: Arc<SessionRepo>,
        soul_repo: Arc<SoulRepo>,
        message_repo: Arc<MessageRepo>,
    ) -> Self {
        Self {
            session_repo,
            soul_repo,
            message_repo,
        }
    }

    pub async fn create_session(&self) -> Result<Session, String> {
        let session_id = format!("sess_{}", Uuid::new_v4().simple());
        self.session_repo
            .create(&session_id, "soul_default")
            .await
            .map_err(|err| format!("session create failed: {err}"))
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>, String> {
        self.session_repo
            .get(session_id)
            .await
            .map_err(|err| format!("session get failed: {err}"))
    }

    pub async fn list_session_messages(&self, session_id: &str) -> Result<Vec<Message>, String> {
        self.message_repo
            .list_for_session(session_id)
            .await
            .map_err(|err| format!("session messages query failed: {err}"))
    }

    pub async fn get_default_soul(&self) -> Result<Option<Soul>, String> {
        self.soul_repo
            .get("soul_default")
            .await
            .map_err(|err| format!("soul get failed: {err}"))
    }
}
