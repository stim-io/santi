use std::{path::Path, sync::Arc};

use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use santi_core::{
    error::{Error, LockError, Result},
    model::runtime::Compact,
    port::{compact_ledger::CompactLedgerPort, lock::Lock, lock::LockGuard},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StandaloneCompactError {
    Busy,
    NotFound,
    Invalid(String),
    Internal(String),
}

#[derive(Clone)]
pub struct StandaloneSessionCompact {
    pool: SqlitePool,
    lock: Arc<dyn Lock>,
}

impl StandaloneSessionCompact {
    pub async fn new(path: impl AsRef<Path>, lock: Arc<dyn Lock>) -> Result<Self> {
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
            r#"CREATE TABLE IF NOT EXISTS standalone_soul_sessions (
                id TEXT PRIMARY KEY,
                soul_id TEXT NOT NULL,
                session_id TEXT NOT NULL UNIQUE,
                session_memory TEXT NOT NULL DEFAULT '',
                provider_state TEXT NULL,
                next_seq INTEGER NOT NULL DEFAULT 1,
                last_seen_session_seq INTEGER NOT NULL DEFAULT 0,
                parent_soul_session_id TEXT NULL,
                fork_point INTEGER NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"#,
        )
        .execute(&pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("migrate sqlite standalone_soul_sessions failed: {err}"),
        })?;

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS standalone_session_compacts (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                turn_id TEXT NOT NULL,
                summary TEXT NOT NULL,
                start_session_seq INTEGER NOT NULL,
                end_session_seq INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"#,
        )
        .execute(&pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("migrate sqlite standalone_session_compacts failed: {err}"),
        })?;

        Ok(Self { pool, lock })
    }

    pub async fn compact_session(
        &self,
        session_id: &str,
        summary: &str,
    ) -> std::result::Result<Compact, StandaloneCompactError> {
        let guard = self
            .lock
            .acquire(&format!("lock:session_send:{session_id}"))
            .await
            .map_err(map_lock_error)?;
        if summary.trim().is_empty() {
            release_compact_guard(guard).await?;
            return Err(StandaloneCompactError::Invalid(
                "expected non-empty compact summary".to_string(),
            ));
        }
        let session_exists = sqlx::query(r#"SELECT 1 FROM sessions WHERE id = ?1 LIMIT 1"#)
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_compact_sql_error)?;
        if session_exists.is_none() {
            release_compact_guard(guard).await?;
            return Err(StandaloneCompactError::NotFound);
        }
        let end_session_seq = sqlx::query(
            r#"SELECT MAX(session_seq) AS end_session_seq FROM session_messages WHERE session_id = ?1"#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_compact_sql_error)?
        .try_get::<Option<i64>, _>("end_session_seq")
        .map_err(|err| {
            StandaloneCompactError::Internal(format!(
                "decode standalone compact range failed: {err}"
            ))
        })?
        .ok_or_else(|| StandaloneCompactError::Invalid("cannot compact empty session".to_string()))?;
        let start_session_seq = sqlx::query(
            r#"SELECT MAX(end_session_seq) AS max_end_session_seq FROM standalone_session_compacts WHERE session_id = ?1"#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_compact_sql_error)?
        .try_get::<Option<i64>, _>("max_end_session_seq")
        .map_err(|err| {
            StandaloneCompactError::Internal(format!(
                "decode standalone compact head failed: {err}"
            ))
        })?
        .map(|seq| seq + 1)
        .unwrap_or(1);
        if start_session_seq > end_session_seq {
            release_compact_guard(guard).await?;
            return Err(StandaloneCompactError::Invalid(
                "no uncompacted session range".to_string(),
            ));
        }
        let compact = Compact {
            id: format!("compact_{}", Uuid::new_v4().simple()),
            turn_id: format!("turn_standalone_compact_{}", Uuid::new_v4().simple()),
            summary: summary.trim().to_string(),
            start_session_seq,
            end_session_seq,
            created_at: current_timestamp_string(&self.pool)
                .await
                .map_err(map_compact_core_error)?,
        };
        sqlx::query(r#"INSERT OR REPLACE INTO standalone_session_compacts (id, session_id, turn_id, summary, start_session_seq, end_session_seq) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#)
            .bind(&compact.id)
            .bind(session_id)
            .bind(&compact.turn_id)
            .bind(&compact.summary)
            .bind(compact.start_session_seq)
            .bind(compact.end_session_seq)
            .execute(&self.pool)
            .await
            .map_err(map_compact_sql_error)?;
        self.ensure_soul_session(session_id, end_session_seq + 1)
            .await
            .map_err(map_compact_core_error)?;
        release_compact_guard(guard).await?;
        Ok(compact)
    }

    pub async fn list_compacts(&self, session_id: &str) -> Result<Vec<Compact>> {
        let rows = sqlx::query(r#"SELECT id, turn_id, summary, start_session_seq, end_session_seq, created_at FROM standalone_session_compacts WHERE session_id = ?1 ORDER BY created_at ASC"#)
            .bind(session_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|err| Error::Internal { message: format!("list standalone compacts failed: {err}") })?;
        Ok(rows
            .into_iter()
            .map(|row| Compact {
                id: row.get("id"),
                turn_id: row.get("turn_id"),
                summary: row.get("summary"),
                start_session_seq: row.get("start_session_seq"),
                end_session_seq: row.get("end_session_seq"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    async fn ensure_soul_session(&self, session_id: &str, next_seq: i64) -> Result<()> {
        sqlx::query(r#"INSERT INTO standalone_soul_sessions (id, soul_id, session_id, session_memory, next_seq, last_seen_session_seq, parent_soul_session_id, fork_point) VALUES (?1, 'soul_default', ?2, '', ?3, 0, NULL, NULL) ON CONFLICT(session_id) DO UPDATE SET updated_at = standalone_soul_sessions.updated_at"#)
            .bind(format!("ss_standalone_{session_id}"))
            .bind(session_id)
            .bind(next_seq)
            .execute(&self.pool)
            .await
            .map_err(|err| Error::Internal { message: format!("ensure standalone soul_session failed: {err}") })?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl CompactLedgerPort for StandaloneSessionCompact {
    async fn list_compacts(&self, soul_session_id: &str) -> Result<Vec<Compact>> {
        let session_id: String = sqlx::query_scalar(
            r#"SELECT session_id FROM standalone_soul_sessions WHERE id = ?1 LIMIT 1"#,
        )
        .bind(soul_session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("load standalone compact session id failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_soul_session",
        })?;
        StandaloneSessionCompact::list_compacts(self, &session_id).await
    }
}

async fn current_timestamp_string(pool: &SqlitePool) -> Result<String> {
    sqlx::query(r#"SELECT CURRENT_TIMESTAMP AS now"#)
        .fetch_one(pool)
        .await
        .map(|row| row.get("now"))
        .map_err(|err| Error::Internal {
            message: format!("load current timestamp failed: {err}"),
        })
}

async fn release_compact_guard(
    guard: Box<dyn LockGuard + Send>,
) -> std::result::Result<(), StandaloneCompactError> {
    guard.release().await.map_err(map_lock_error)
}

fn map_compact_sql_error(err: sqlx::Error) -> StandaloneCompactError {
    StandaloneCompactError::Internal(format!("standalone compact sqlite failed: {err}"))
}

fn map_compact_core_error(err: Error) -> StandaloneCompactError {
    match err {
        Error::NotFound { .. } => StandaloneCompactError::NotFound,
        Error::Busy { resource } => StandaloneCompactError::Internal(format!("{resource} busy")),
        Error::InvalidInput { message } => StandaloneCompactError::Invalid(message),
        Error::Upstream { message } | Error::Internal { message } => {
            StandaloneCompactError::Internal(message)
        }
    }
}

fn map_lock_error(err: LockError) -> StandaloneCompactError {
    match err {
        LockError::Busy => StandaloneCompactError::Busy,
        LockError::Lost => {
            StandaloneCompactError::Internal("session compact lock lost".to_string())
        }
        LockError::Backend { message } => StandaloneCompactError::Internal(message),
    }
}
