use std::{path::Path, sync::Arc};

use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use santi_core::{
    error::{Error, LockError, Result},
    port::lock::{Lock, LockGuard},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandaloneForkResult {
    pub new_session_id: String,
    pub parent_session_id: String,
    pub fork_point: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StandaloneForkError {
    Busy,
    ParentNotFound,
    InvalidForkPoint(String),
    Internal(String),
}

#[derive(Clone)]
pub struct StandaloneSessionForkStore {
    pool: SqlitePool,
    lock: Arc<dyn Lock>,
}

impl StandaloneSessionForkStore {
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
        sqlx::query(r#"CREATE TABLE IF NOT EXISTS standalone_soul_sessions (id TEXT PRIMARY KEY, soul_id TEXT NOT NULL, session_id TEXT NOT NULL UNIQUE, session_memory TEXT NOT NULL DEFAULT '', provider_state TEXT NULL, next_seq INTEGER NOT NULL DEFAULT 1, last_seen_session_seq INTEGER NOT NULL DEFAULT 0, parent_soul_session_id TEXT NULL, fork_point INTEGER NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)"#).execute(&pool).await.map_err(|err| Error::Internal { message: format!("migrate sqlite standalone_soul_sessions failed: {err}") })?;
        sqlx::query(r#"CREATE TABLE IF NOT EXISTS standalone_session_compacts (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, turn_id TEXT NOT NULL, summary TEXT NOT NULL, start_session_seq INTEGER NOT NULL, end_session_seq INTEGER NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)"#).execute(&pool).await.map_err(|err| Error::Internal { message: format!("migrate sqlite standalone_session_compacts failed: {err}") })?;
        let _ = lock;
        Ok(Self { pool, lock })
    }

    pub async fn fork_session(
        &self,
        parent_session_id: &str,
        fork_point: i64,
        request_id: &str,
    ) -> std::result::Result<StandaloneForkResult, StandaloneForkError> {
        let guard = self
            .lock
            .acquire(&format!("lock:session_send:{parent_session_id}"))
            .await
            .map_err(map_lock_error)?;
        let parent_session =
            sqlx::query(r#"SELECT id, session_memory FROM sessions WHERE id = ?1 LIMIT 1"#)
                .bind(parent_session_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_fork_sql_error)?;
        let Some(parent_session) = parent_session else {
            release_fork_guard(guard).await?;
            return Err(StandaloneForkError::ParentNotFound);
        };
        let parent_messages = sqlx::query(r#"SELECT actor_type, actor_id, content_text, state, session_seq FROM session_messages WHERE session_id = ?1 ORDER BY session_seq ASC"#).bind(parent_session_id).fetch_all(&self.pool).await.map_err(map_fork_sql_error)?;
        let parent_next_seq = parent_messages
            .last()
            .map(|row| row.get::<i64, _>("session_seq") + 1)
            .unwrap_or(1);
        if fork_point < 1 || fork_point >= parent_next_seq {
            release_fork_guard(guard).await?;
            return Err(StandaloneForkError::InvalidForkPoint(format!(
                "illegal fork_point {}: must be 1 <= fp < {}",
                fork_point, parent_next_seq
            )));
        }
        let new_session_id = format!(
            "sess_{}",
            Uuid::new_v5(
                &Uuid::NAMESPACE_OID,
                format!(
                    "santi_fork:{}:{}:{}",
                    parent_session_id, fork_point, request_id
                )
                .as_bytes()
            )
            .simple()
        );
        let existing_child = sqlx::query(
            r#"SELECT id, parent_session_id, fork_point FROM sessions WHERE id = ?1 LIMIT 1"#,
        )
        .bind(&new_session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_fork_sql_error)?;
        if let Some(existing_child) = existing_child {
            let same_lineage = existing_child
                .try_get::<Option<String>, _>("parent_session_id")
                .ok()
                .flatten()
                == Some(parent_session_id.to_string())
                && existing_child
                    .try_get::<Option<i64>, _>("fork_point")
                    .ok()
                    .flatten()
                    == Some(fork_point);
            release_fork_guard(guard).await?;
            if same_lineage {
                return Ok(StandaloneForkResult {
                    new_session_id,
                    parent_session_id: parent_session_id.to_string(),
                    fork_point,
                });
            }
            return Err(StandaloneForkError::Internal(
                "existing fork session id collided with incompatible lineage".to_string(),
            ));
        }
        let copied_messages: Vec<_> = parent_messages
            .into_iter()
            .filter(|row| row.get::<i64, _>("session_seq") <= fork_point)
            .collect();
        sqlx::query(r#"INSERT INTO sessions (id, parent_session_id, fork_point, session_memory, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#).bind(&new_session_id).bind(parent_session_id).bind(fork_point).bind(parent_session.get::<String, _>("session_memory")).execute(&self.pool).await.map_err(map_fork_sql_error)?;
        for (index, row) in copied_messages.into_iter().enumerate() {
            sqlx::query(r#"INSERT INTO session_messages (id, session_id, session_seq, actor_type, actor_id, content_text, state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)"#).bind(Uuid::new_v4().to_string()).bind(&new_session_id).bind((index as i64) + 1).bind(row.get::<String, _>("actor_type")).bind(row.get::<String, _>("actor_id")).bind(row.get::<String, _>("content_text")).bind(row.get::<String, _>("state")).execute(&self.pool).await.map_err(map_fork_sql_error)?;
        }
        self.ensure_soul_session(parent_session_id, parent_next_seq)
            .await
            .map_err(map_fork_core_error)?;
        self.ensure_soul_session(&new_session_id, fork_point + 1)
            .await
            .map_err(map_fork_core_error)?;
        release_fork_guard(guard).await?;
        Ok(StandaloneForkResult {
            new_session_id,
            parent_session_id: parent_session_id.to_string(),
            fork_point,
        })
    }

    async fn ensure_soul_session(
        &self,
        session_id: &str,
        next_seq: i64,
    ) -> Result<StandaloneSoulSessionRow> {
        sqlx::query(r#"INSERT INTO standalone_soul_sessions (id, soul_id, session_id, session_memory, next_seq, last_seen_session_seq, parent_soul_session_id, fork_point) VALUES (?1, 'soul_default', ?2, '', ?3, 0, NULL, NULL) ON CONFLICT(session_id) DO UPDATE SET updated_at = standalone_soul_sessions.updated_at"#).bind(format!("ss_standalone_{session_id}")).bind(session_id).bind(next_seq).execute(&self.pool).await.map_err(|err| Error::Internal { message: format!("ensure standalone soul_session failed: {err}") })?;
        let row =
            sqlx::query(r#"SELECT id FROM standalone_soul_sessions WHERE session_id = ?1 LIMIT 1"#)
                .bind(session_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|err| Error::Internal {
                    message: format!("load standalone soul_session failed: {err}"),
                })?;
        let _ = row.get::<String, _>("id");
        Ok(StandaloneSoulSessionRow)
    }
}

#[derive(Clone, Debug)]
struct StandaloneSoulSessionRow;

async fn release_fork_guard(
    guard: Box<dyn LockGuard + Send>,
) -> std::result::Result<(), StandaloneForkError> {
    guard.release().await.map_err(map_lock_error)
}
fn map_fork_sql_error(err: sqlx::Error) -> StandaloneForkError {
    StandaloneForkError::Internal(format!("standalone fork sqlite failed: {err}"))
}
fn map_fork_core_error(err: Error) -> StandaloneForkError {
    match err {
        Error::NotFound { .. } => StandaloneForkError::ParentNotFound,
        Error::Busy { resource } => StandaloneForkError::Internal(format!("{resource} busy")),
        Error::InvalidInput { message } => StandaloneForkError::InvalidForkPoint(message),
        Error::Upstream { message } | Error::Internal { message } => {
            StandaloneForkError::Internal(message)
        }
    }
}
fn map_lock_error(err: LockError) -> StandaloneForkError {
    match err {
        LockError::Busy => StandaloneForkError::Busy,
        LockError::Lost => StandaloneForkError::Internal("fork session lock lost".to_string()),
        LockError::Backend { message } => StandaloneForkError::Internal(message),
    }
}
