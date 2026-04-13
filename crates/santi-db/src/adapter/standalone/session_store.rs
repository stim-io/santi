use std::path::Path;

use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use santi_core::{
    error::{Error, Result},
    model::{
        message::{ActorType, Message, MessageContent, MessagePart, MessageState},
        session::{Session, SessionMessage, SessionMessageRef},
    },
    port::session_ledger::{AppendSessionMessage, ApplyMessageEvent, SessionLedgerPort},
};

#[derive(Clone)]
pub struct StandaloneSessionLedger {
    pool: SqlitePool,
}

impl StandaloneSessionLedger {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| Error::Internal {
                    message: format!("create sqlite parent dir failed: {err}"),
                })?;
        }

        let database_url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .map_err(|err| Error::Internal {
                message: format!("connect sqlite failed: {err}"),
            })?;

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, parent_session_id TEXT NULL, fork_point INTEGER NULL, session_memory TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)"#,
        )
        .execute(&pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("migrate sqlite sessions failed: {err}"),
        })?;

        sqlx::query(r#"ALTER TABLE sessions ADD COLUMN session_memory TEXT NOT NULL DEFAULT ''"#)
            .execute(&pool)
            .await
            .ok();

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS session_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                session_seq INTEGER NOT NULL,
                actor_type TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                content_text TEXT NOT NULL,
                state TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(session_id, session_seq)
            )"#,
        )
        .execute(&pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("migrate sqlite session_messages failed: {err}"),
        })?;

        Ok(Self { pool })
    }

    pub async fn create_session(&self, session_id: &str) -> Result<Session> {
        sqlx::query(r#"INSERT INTO sessions (id, created_at, updated_at) VALUES (?1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#)
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|err| Error::Internal { message: format!("session create failed: {err}") })?;
        self.get_session(session_id)
            .await
            .map(|opt| opt.expect("just inserted session"))
    }

    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let row = sqlx::query(r#"SELECT id, parent_session_id, fork_point, created_at, updated_at FROM sessions WHERE id = ?1 LIMIT 1"#)
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| Error::Internal { message: format!("session get failed: {err}") })?;

        Ok(row.map(|row| Session {
            id: row.get("id"),
            parent_session_id: row.try_get("parent_session_id").ok(),
            fork_point: row.try_get("fork_point").ok(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
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

    pub async fn set_session_memory(
        &self,
        session_id: &str,
        memory: &str,
    ) -> Result<Option<(String, String, String)>> {
        let result = sqlx::query(r#"UPDATE sessions SET session_memory = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1"#)
            .bind(session_id)
            .bind(memory)
            .execute(&self.pool)
            .await
            .map_err(|err| Error::Internal { message: format!("session memory update failed: {err}") })?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_session_memory(session_id).await
    }

    pub async fn append_user_message(
        &self,
        session_id: &str,
        content_text: &str,
    ) -> Result<Option<SessionMessage>> {
        self.append_text_message(
            session_id,
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

        let next_seq = sqlx::query(r#"SELECT COALESCE(MAX(session_seq), 0) + 1 AS next_seq FROM session_messages WHERE session_id = ?1"#)
            .bind(session_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|err| Error::Internal { message: format!("session message seq lookup failed: {err}") })?
            .get::<i64, _>("next_seq");

        let id = uuid::Uuid::new_v4().to_string();
        let actor_type_db = match actor_type {
            ActorType::Account => "account",
            ActorType::Soul => "soul",
            ActorType::System => "system",
        };
        let state_db = match state {
            MessageState::Pending => "pending",
            MessageState::Fixed => "fixed",
        };

        let result = sqlx::query(r#"INSERT INTO session_messages (id, session_id, session_seq, actor_type, actor_id, content_text, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)"#)
            .bind(&id)
            .bind(session_id)
            .bind(next_seq)
            .bind(actor_type_db)
            .bind(actor_id)
            .bind(content_text)
            .bind(state_db)
            .execute(&self.pool)
            .await
            .map_err(|err| Error::Internal { message: format!("session message append failed: {err}") })?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.list_messages(session_id)
            .await
            .map(|mut messages| messages.pop())
    }

    pub async fn list_messages(&self, session_id: &str) -> Result<Vec<SessionMessage>> {
        let rows = sqlx::query(r#"SELECT id, session_id, session_seq, actor_type, actor_id, content_text, state, created_at FROM session_messages WHERE session_id = ?1 ORDER BY session_seq ASC, created_at ASC"#)
            .bind(session_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| Error::Internal { message: format!("session messages list failed: {err}") })?;

        Ok(rows
            .into_iter()
            .map(|row| SessionMessage {
                message: Message {
                    id: row.get("id"),
                    actor_type: match row.get::<String, _>("actor_type").as_str() {
                        "soul" => ActorType::Soul,
                        "system" => ActorType::System,
                        _ => ActorType::Account,
                    },
                    actor_id: row.get("actor_id"),
                    content: MessageContent {
                        parts: vec![MessagePart::Text {
                            text: row.get("content_text"),
                        }],
                    },
                    state: match row.get::<String, _>("state").as_str() {
                        "fixed" => MessageState::Fixed,
                        _ => MessageState::Pending,
                    },
                    created_at: row.get("created_at"),
                    updated_at: row.get("created_at"),
                    deleted_at: None,
                    version: 1,
                },
                relation: SessionMessageRef {
                    message_id: row.get("id"),
                    session_id: row.get("session_id"),
                    session_seq: row.get("session_seq"),
                    created_at: row.get("created_at"),
                },
            })
            .collect())
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
            r#"SELECT id, session_id, session_seq, actor_type, actor_id, content_text, state, created_at FROM session_messages WHERE id = ?1 LIMIT 1"#,
        )
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("session message get failed: {err}") })?;

        Ok(row.map(|row| SessionMessage {
            message: Message {
                id: row.get("id"),
                actor_type: match row.get::<String, _>("actor_type").as_str() {
                    "soul" => ActorType::Soul,
                    "system" => ActorType::System,
                    _ => ActorType::Account,
                },
                actor_id: row.get("actor_id"),
                content: MessageContent {
                    parts: vec![MessagePart::Text {
                        text: row.get("content_text"),
                    }],
                },
                state: match row.get::<String, _>("state").as_str() {
                    "fixed" => MessageState::Fixed,
                    _ => MessageState::Pending,
                },
                created_at: row.get("created_at"),
                updated_at: row.get("created_at"),
                deleted_at: None,
                version: 1,
            },
            relation: SessionMessageRef {
                message_id: row.get("id"),
                session_id: row.get("session_id"),
                session_seq: row.get("session_seq"),
                created_at: row.get("created_at"),
            },
        }))
    }

    async fn list_messages(
        &self,
        session_id: &str,
        _after_session_seq: Option<i64>,
    ) -> Result<Vec<SessionMessage>> {
        StandaloneSessionLedger::list_messages(self, session_id).await
    }

    async fn append_message(&self, input: AppendSessionMessage) -> Result<SessionMessage> {
        let content_text = input
            .content
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text } => Some(text.as_str()),
                MessagePart::Image { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        self.append_text_message(
            &input.session_id,
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

    async fn apply_message_event(&self, _input: ApplyMessageEvent) -> Result<SessionMessage> {
        Err(Error::Internal {
            message: "message events are unavailable in standalone mode".to_string(),
        })
    }
}
