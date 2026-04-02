use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

use santi_core::{
    error::{Error, Result},
    model::session::{Session, SessionMessage},
    port::session_ledger::{AppendSessionMessage, ApplyMessageEvent, SessionLedgerPort},
};

#[derive(Clone)]
pub struct SqliteSessionLedger {
    pool: SqlitePool,
}

impl SqliteSessionLedger {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SessionLedgerPort for SqliteSessionLedger {
    async fn create_session(&self, session_id: &str) -> Result<Session> {
        sqlx::query(
            r#"INSERT INTO sessions (id, created_at, updated_at) VALUES (?1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
        )
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("session create failed: {err}") })?;

        self.get_session(session_id).await.map(|opt| opt.expect("just inserted session"))
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let row = sqlx::query(
            r#"SELECT id, parent_session_id, fork_point, created_at, updated_at FROM sessions WHERE id = ?1 LIMIT 1"#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("session get failed: {err}") })?;

        Ok(row.map(|row: SqliteRow| Session {
            id: row.get("id"),
            parent_session_id: row.try_get("parent_session_id").ok(),
            fork_point: row.try_get("fork_point").ok(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn list_messages(&self, _session_id: &str, _after_session_seq: Option<i64>) -> Result<Vec<SessionMessage>> {
        Ok(vec![])
    }

    async fn append_message(&self, _input: AppendSessionMessage) -> Result<SessionMessage> {
        Err(Error::Internal { message: "session messages are unavailable in local mode".to_string() })
    }

    async fn apply_message_event(&self, _input: ApplyMessageEvent) -> Result<SessionMessage> {
        Err(Error::Internal { message: "session messages are unavailable in local mode".to_string() })
    }
}
