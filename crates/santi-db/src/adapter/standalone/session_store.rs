use std::path::Path;

use serde_json::{Map, Value};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use santi_core::{
    error::{Error, Result},
    model::{
        message::{
            ActorType, Message, MessageContent, MessageEventPayload, MessagePart, MessageState,
        },
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
                content_json TEXT NULL,
                state TEXT NOT NULL,
                version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(session_id, session_seq)
            )"#,
        )
        .execute(&pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("migrate sqlite session_messages failed: {err}"),
        })?;

        sqlx::query(r#"ALTER TABLE session_messages ADD COLUMN version INTEGER NOT NULL DEFAULT 1"#)
            .execute(&pool)
            .await
            .ok();
        sqlx::query(
            r#"ALTER TABLE session_messages ADD COLUMN updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP"#,
        )
        .execute(&pool)
        .await
        .ok();
        sqlx::query(r#"ALTER TABLE session_messages ADD COLUMN content_json TEXT NULL"#)
            .execute(&pool)
            .await
            .ok();

        Ok(Self { pool })
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

fn map_session_message_row(row: sqlx::sqlite::SqliteRow) -> SessionMessage {
    SessionMessage {
        message: Message {
            id: row.get("id"),
            actor_type: actor_type(&row.get::<String, _>("actor_type")),
            actor_id: row.get("actor_id"),
            content: content_from_row(&row),
            state: message_state(&row.get::<String, _>("state")),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            deleted_at: None,
            version: row.get("version"),
        },
        relation: SessionMessageRef {
            message_id: row.get("id"),
            session_id: row.get("session_id"),
            session_seq: row.get("session_seq"),
            created_at: row.get("created_at"),
        },
    }
}

fn content_from_row(row: &sqlx::sqlite::SqliteRow) -> MessageContent {
    row.try_get::<String, _>("content_json")
        .ok()
        .and_then(|raw| serde_json::from_str::<MessageContent>(&raw).ok())
        .unwrap_or_else(|| MessageContent {
            parts: vec![MessagePart::Text {
                text: row.get("content_text"),
            }],
        })
}

fn actor_type(raw: &str) -> ActorType {
    match raw {
        "soul" => ActorType::Soul,
        "system" => ActorType::System,
        _ => ActorType::Account,
    }
}

fn actor_type_db(actor_type: &ActorType) -> &'static str {
    match actor_type {
        ActorType::Account => "account",
        ActorType::Soul => "soul",
        ActorType::System => "system",
    }
}

fn message_state(raw: &str) -> MessageState {
    match raw {
        "fixed" => MessageState::Fixed,
        _ => MessageState::Pending,
    }
}

fn state_db(state: &MessageState) -> &'static str {
    match state {
        MessageState::Pending => "pending",
        MessageState::Fixed => "fixed",
    }
}

fn content_to_text(content: &MessageContent) -> Result<String> {
    let parts = content
        .parts
        .iter()
        .map(|part| match part {
            MessagePart::Text { text } => Ok(text.as_str()),
            MessagePart::Image { .. } => Err(Error::InvalidInput {
                message: "standalone stim message lifecycle supports text parts only".to_string(),
            }),
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(parts.join("\n\n"))
}

fn content_to_json(content: &MessageContent) -> Result<String> {
    serde_json::to_string(content).map_err(|err| Error::Internal {
        message: format!("message content serialize failed: {err}"),
    })
}

fn apply_message_event_to_message(
    mut message: Message,
    actor_type: &ActorType,
    actor_id: &str,
    base_version: i64,
    payload: &MessageEventPayload,
) -> Result<Message> {
    if &message.actor_type != actor_type || message.actor_id != actor_id {
        return Err(Error::InvalidInput {
            message: "only the original actor may mutate a message".to_string(),
        });
    }

    if message.version != base_version {
        return Err(Error::InvalidInput {
            message: format!(
                "message version mismatch: expected {}, got {}",
                message.version, base_version
            ),
        });
    }

    if message.deleted_at.is_some() {
        return Err(Error::InvalidInput {
            message: "deleted messages cannot be mutated".to_string(),
        });
    }

    if message.state == MessageState::Fixed {
        return Err(Error::InvalidInput {
            message: "fixed messages cannot be mutated".to_string(),
        });
    }

    match payload {
        MessageEventPayload::Patch { patches } => {
            let mut parts = message.content.parts.clone();
            for patch in patches {
                let index = valid_index(parts.len(), patch.index, "patch")?;
                parts[index] = merge_message_part(&parts[index], &patch.merge)?;
            }
            message.content = MessageContent { parts };
        }
        MessageEventPayload::Insert { items } => {
            let mut parts = message.content.parts.clone();
            let mut sorted_items = items.clone();
            sorted_items.sort_by_key(|item| item.index);
            for item in sorted_items {
                let index = valid_insert_index(parts.len(), item.index)?;
                parts.insert(index, item.part);
            }
            message.content = MessageContent { parts };
        }
        MessageEventPayload::Remove { indexes } => {
            let mut unique_indexes = indexes.clone();
            unique_indexes.sort_unstable();
            unique_indexes.dedup();
            let parts_len = message.content.parts.len();
            for index in &unique_indexes {
                let _ = valid_index(parts_len, *index, "remove")?;
            }

            let mut parts = message.content.parts.clone();
            for index in unique_indexes.into_iter().rev() {
                parts.remove(index as usize);
            }
            message.content = MessageContent { parts };
        }
        MessageEventPayload::Fix => {
            message.state = MessageState::Fixed;
        }
        MessageEventPayload::Delete { .. } => {
            message.deleted_at = Some(String::new());
        }
    }

    message.version += 1;
    Ok(message)
}

fn valid_index(len: usize, raw: i64, action: &str) -> Result<usize> {
    if raw < 0 || raw as usize >= len {
        return Err(Error::InvalidInput {
            message: format!("{action} index out of bounds: {raw}"),
        });
    }
    Ok(raw as usize)
}

fn valid_insert_index(len: usize, raw: i64) -> Result<usize> {
    if raw < 0 || raw as usize > len {
        return Err(Error::InvalidInput {
            message: format!("insert index out of bounds: {raw}"),
        });
    }
    Ok(raw as usize)
}

fn merge_message_part(part: &MessagePart, merge: &Value) -> Result<MessagePart> {
    let mut base = serde_json::to_value(part).map_err(|err| Error::Internal {
        message: format!("message part serialize failed: {err}"),
    })?;

    let merge_object = merge.as_object().ok_or(Error::InvalidInput {
        message: "patch merge must be an object".to_string(),
    })?;

    let base_object = base.as_object_mut().ok_or(Error::Internal {
        message: "message part must serialize to an object".to_string(),
    })?;

    merge_json_object(base_object, merge_object);

    serde_json::from_value(base).map_err(|err| Error::InvalidInput {
        message: format!("patch produced invalid message part: {err}"),
    })
}

fn merge_json_object(base: &mut Map<String, Value>, merge: &Map<String, Value>) {
    for (key, value) in merge {
        base.insert(key.clone(), value.clone());
    }
}
