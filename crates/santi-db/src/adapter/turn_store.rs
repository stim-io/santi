use std::sync::Arc;

use santi_core::{
    error::Error,
    model::message::Message,
    port::turn_store::{NewTurnMessage, TurnContext, TurnStore},
};
use uuid::Uuid;

use crate::repo::{
    message_repo::{MessageRepo, NewMessage},
    relation_repo::RelationRepo,
    session_repo::SessionRepo,
    soul_repo::SoulRepo,
};

#[derive(Clone)]
pub struct RepoBackedTurnStore {
    session_repo: Arc<SessionRepo>,
    soul_repo: Arc<SoulRepo>,
    message_repo: Arc<MessageRepo>,
    relation_repo: Arc<RelationRepo>,
}

impl RepoBackedTurnStore {
    pub fn new(
        session_repo: Arc<SessionRepo>,
        soul_repo: Arc<SoulRepo>,
        message_repo: Arc<MessageRepo>,
        relation_repo: Arc<RelationRepo>,
    ) -> Self {
        Self {
            session_repo,
            soul_repo,
            message_repo,
            relation_repo,
        }
    }
}

#[async_trait::async_trait]
impl TurnStore for RepoBackedTurnStore {
    async fn load_turn_context(
        &self,
        session_id: &str,
    ) -> santi_core::error::Result<Option<TurnContext>> {
        let session = self
            .session_repo
            .get(session_id)
            .await
            .map_err(|err| Error::Internal {
                message: format!("session load failed: {err}"),
            })?;

        let Some(session) = session else {
            return Ok(None);
        };

        let soul = self
            .soul_repo
            .get(&session.soul_id)
            .await
            .map_err(|err| Error::Internal {
                message: format!("soul load failed: {err}"),
            })?
            .ok_or_else(|| Error::NotFound { resource: "soul" })?;

        Ok(Some(TurnContext {
            session,
            soul_memory: soul.memory,
        }))
    }

    async fn list_messages(&self, session_id: &str) -> santi_core::error::Result<Vec<Message>> {
        self.message_repo
            .list_for_session(session_id)
            .await
            .map_err(|err| Error::Internal {
                message: format!("message history load failed: {err}"),
            })
    }

    async fn append_message(
        &self,
        session_id: &str,
        message: NewTurnMessage,
    ) -> santi_core::error::Result<Message> {
        if !self
            .session_repo
            .exists(session_id)
            .await
            .map_err(|err| Error::Internal {
                message: format!("session exists query failed: {err}"),
            })?
        {
            return Err(Error::NotFound { resource: "session" });
        }

        let mut tx = self
            .session_repo
            .begin_tx()
            .await
            .map_err(|err| Error::Internal {
                message: format!("transaction begin failed: {err}"),
            })?;

        let session_seq = self
            .session_repo
            .allocate_next_session_seq(&mut tx, session_id)
            .await
            .map_err(|err| Error::Internal {
                message: format!("session seq allocation failed: {err}"),
            })?;

        let message_id = format!("msg_{}", Uuid::new_v4().simple());
        let persisted = self
            .message_repo
            .insert(
                &mut tx,
                NewMessage {
                    id: &message_id,
                    r#type: &message.r#type,
                    role: message.role.as_deref(),
                    content: &message.content,
                },
            )
            .await
            .map_err(|err| Error::Internal {
                message: format!("message insert failed: {err}"),
            })?;

        self.relation_repo
            .attach_message_to_session(&mut tx, session_id, &persisted.id, session_seq)
            .await
            .map_err(|err| Error::Internal {
                message: format!("message relation insert failed: {err}"),
            })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("transaction commit failed: {err}"),
        })?;

        Ok(persisted)
    }
}
