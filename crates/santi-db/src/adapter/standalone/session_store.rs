use std::path::Path;

use sqlx::{Row, SqlitePool};

mod mapping;
mod message_events;
mod schema;

use mapping::{actor_type_db, content_to_json, content_to_text, map_session_message_row, state_db};
use message_events::apply_message_event_to_message;
use schema::setup_sqlite_pool;

use santi_core::{
    error::{Error, Result},
    model::{
        message::{ActorType, MessageContent, MessagePart, MessageState},
        session::{Session, SessionMessage},
    },
    port::session_ledger::{AppendSessionMessage, ApplyMessageEvent, SessionLedgerPort},
};

#[derive(Clone)]
pub struct StandaloneSessionLedger {
    pool: SqlitePool,
}

impl StandaloneSessionLedger {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            pool: setup_sqlite_pool(path.as_ref()).await?,
        })
    }

    pub async fn create_session(&self, session_id: &str) -> Result<Session> {
        sqlx::query(
            r#"INSERT INTO sessions (id, created_at, updated_at) VALUES (?1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
        )
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session create failed: {err}"),
        })?;
        self.get_session(session_id)
            .await
            .map(|opt| opt.expect("just inserted session"))
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let row = sqlx::query(
            r#"SELECT id, parent_session_id, fork_point, created_at, updated_at FROM sessions WHERE id = ?1 LIMIT 1"#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session get failed: {err}"),
        })?;

        Ok(row.map(|row| Session {
            id: row.get("id"),
            parent_session_id: row.try_get("parent_session_id").ok(),
            fork_point: row.try_get("fork_point").ok(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    pub async fn set_session_memory(
        &self,
        session_id: &str,
        memory: &str,
    ) -> Result<Option<(String, String, String)>> {
        let result = sqlx::query(
            r#"UPDATE sessions SET session_memory = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1"#,
        )
        .bind(session_id)
        .bind(memory)
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session memory update failed: {err}"),
        })?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_session_memory(session_id).await
    }

    pub async fn get_session_memory(
        &self,
        session_id: &str,
    ) -> Result<Option<(String, String, String)>> {
        let row = sqlx::query(
            r#"SELECT id, session_memory, updated_at FROM sessions WHERE id = ?1 LIMIT 1"#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session memory get failed: {err}"),
        })?;

        Ok(row.map(|row| {
            (
                row.get("id"),
                row.get("session_memory"),
                row.get("updated_at"),
            )
        }))
    }

    pub async fn append_user_message(
        &self,
        session_id: &str,
        content_text: &str,
    ) -> Result<Option<SessionMessage>> {
        self.append_text_message(
            session_id,
            &uuid::Uuid::new_v4().to_string(),
            ActorType::Account,
            "user",
            content_text,
            MessageState::Fixed,
        )
        .await
    }

    async fn append_text_message(
        &self,
        session_id: &str,
        message_id: &str,
        actor_type: ActorType,
        actor_id: &str,
        content_text: &str,
        state: MessageState,
    ) -> Result<Option<SessionMessage>> {
        let session_exists = sqlx::query(r#"SELECT 1 FROM sessions WHERE id = ?1 LIMIT 1"#)
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| Error::Internal {
                message: format!("session existence check failed: {err}"),
            })?;

        if session_exists.is_none() {
            return Ok(None);
        }

        let next_seq = sqlx::query(
            r#"SELECT COALESCE(MAX(session_seq), 0) + 1 AS next_seq FROM session_messages WHERE session_id = ?1"#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session message seq lookup failed: {err}"),
        })?
        .get::<i64, _>("next_seq");

        let result = sqlx::query(
            r#"INSERT INTO session_messages (id, session_id, session_seq, actor_type, actor_id, content_text, content_json, state, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
        )
        .bind(message_id)
        .bind(session_id)
        .bind(next_seq)
        .bind(actor_type_db(&actor_type))
        .bind(actor_id)
        .bind(content_text)
        .bind(content_to_json(&MessageContent {
            parts: vec![MessagePart::Text {
                text: content_text.to_string(),
            }],
        })?)
        .bind(state_db(&state))
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session message append failed: {err}"),
        })?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.list_messages(session_id)
            .await
            .map(|mut messages| messages.pop())
    }

    pub async fn list_messages(&self, session_id: &str) -> Result<Vec<SessionMessage>> {
        let rows = sqlx::query(
            r#"SELECT id, session_id, session_seq, actor_type, actor_id, content_text, content_json, state, version, created_at, updated_at FROM session_messages WHERE session_id = ?1 ORDER BY session_seq ASC, created_at ASC"#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session messages list failed: {err}"),
        })?;

        Ok(rows.into_iter().map(map_session_message_row).collect())
    }
}

#[async_trait::async_trait]
impl SessionLedgerPort for StandaloneSessionLedger {
    async fn create_session(&self, session_id: &str) -> Result<Session> {
        StandaloneSessionLedger::create_session(self, session_id).await
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        StandaloneSessionLedger::get_session(self, session_id).await
    }

    async fn get_message(&self, message_id: &str) -> Result<Option<SessionMessage>> {
        let row = sqlx::query(
            r#"SELECT id, session_id, session_seq, actor_type, actor_id, content_text, content_json, state, version, created_at, updated_at FROM session_messages WHERE id = ?1 LIMIT 1"#,
        )
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session message get failed: {err}"),
        })?;

        Ok(row.map(map_session_message_row))
    }

    async fn list_messages(
        &self,
        session_id: &str,
        _after_session_seq: Option<i64>,
    ) -> Result<Vec<SessionMessage>> {
        StandaloneSessionLedger::list_messages(self, session_id).await
    }

    async fn append_message(&self, input: AppendSessionMessage) -> Result<SessionMessage> {
        let content_text = content_to_text(&input.content)?;

        self.append_text_message(
            &input.session_id,
            &input.message_id,
            input.actor_type,
            &input.actor_id,
            &content_text,
            input.state,
        )
        .await?
        .ok_or(Error::NotFound {
            resource: "session",
        })
    }

    async fn apply_message_event(&self, input: ApplyMessageEvent) -> Result<SessionMessage> {
        let current = self
            .get_message(&input.message_id)
            .await?
            .ok_or(Error::NotFound {
                resource: "message",
            })?;

        if current.relation.session_id != input.session_id {
            return Err(Error::NotFound {
                resource: "message",
            });
        }

        let updated_message = apply_message_event_to_message(
            current.message,
            &input.actor_type,
            &input.actor_id,
            input.base_version,
            &input.payload,
        )?;

        let content_text = content_to_text(&updated_message.content)?;

        sqlx::query(
            r#"UPDATE session_messages SET content_text = ?2, content_json = ?3, state = ?4, version = ?5, updated_at = CURRENT_TIMESTAMP WHERE id = ?1"#,
        )
        .bind(&input.message_id)
        .bind(content_text)
        .bind(content_to_json(&updated_message.content)?)
        .bind(state_db(&updated_message.state))
        .bind(updated_message.version)
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session message event update failed: {err}"),
        })?;

        self.get_message(&input.message_id)
            .await?
            .ok_or(Error::NotFound {
                resource: "message",
            })
    }
}
