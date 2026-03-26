use std::sync::Arc;

use santi_core::{
    error::Error,
    model::{message::Message, session::Session, soul::Soul},
    port::session_query::SessionQueryPort,
};

use crate::repo::{
    message_repo::MessageRepo,
    session_repo::SessionRepo,
    soul_repo::SoulRepo,
};

#[derive(Clone)]
pub struct RepoBackedSessionQuery {
    session_repo: Arc<SessionRepo>,
    soul_repo: Arc<SoulRepo>,
    message_repo: Arc<MessageRepo>,
}

impl RepoBackedSessionQuery {
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
}

#[async_trait::async_trait]
impl SessionQueryPort for RepoBackedSessionQuery {
    async fn create_session(&self, session_id: &str, soul_id: &str) -> santi_core::error::Result<Session> {
        self.session_repo
            .create(session_id, soul_id)
            .await
            .map_err(|err| Error::Internal {
                message: format!("session create failed: {err}"),
            })
    }

    async fn get_session(&self, session_id: &str) -> santi_core::error::Result<Option<Session>> {
        self.session_repo
            .get(session_id)
            .await
            .map_err(|err| Error::Internal {
                message: format!("session get failed: {err}"),
            })
    }

    async fn list_session_messages(&self, session_id: &str) -> santi_core::error::Result<Vec<Message>> {
        self.message_repo
            .list_for_session(session_id)
            .await
            .map_err(|err| Error::Internal {
                message: format!("session messages query failed: {err}"),
            })
    }

    async fn get_soul(&self, soul_id: &str) -> santi_core::error::Result<Option<Soul>> {
        self.soul_repo
            .get(soul_id)
            .await
            .map_err(|err| Error::Internal {
                message: format!("soul get failed: {err}"),
            })
    }
}
