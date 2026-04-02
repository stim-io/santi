use std::path::Path;

use std::sync::Arc;

use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use santi_core::{
    error::{Error, LockError, Result},
    model::runtime::Compact,
    port::{lock::Lock, lock::LockGuard},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalForkResult {
    pub new_session_id: String,
    pub parent_session_id: String,
    pub fork_point: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalForkError {
    Busy,
    ParentNotFound,
    InvalidForkPoint(String),
    Internal(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalCompactError {
    Busy,
    NotFound,
    Invalid(String),
    Internal(String),
}

#[derive(Clone)]
pub struct LocalSessionForkCompactStore {
    pool: SqlitePool,
    lock: Arc<dyn Lock>,
}

impl LocalSessionForkCompactStore {
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
            r#"CREATE TABLE IF NOT EXISTS local_soul_sessions (
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
            message: format!("migrate sqlite local_soul_sessions failed: {err}"),
        })?;

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS local_session_compacts (
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
            message: format!("migrate sqlite local_session_compacts failed: {err}"),
        })?;

        Ok(Self { pool, lock })
    }

    pub async fn fork_session(
        &self,
        parent_session_id: &str,
        fork_point: i64,
        request_id: &str,
    ) -> std::result::Result<LocalForkResult, LocalForkError> {
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
            return Err(LocalForkError::ParentNotFound);
        };

        let parent_messages = sqlx::query(
            r#"SELECT actor_type, actor_id, content_text, state, session_seq
               FROM session_messages
               WHERE session_id = ?1
               ORDER BY session_seq ASC"#,
        )
        .bind(parent_session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_fork_sql_error)?;

        let parent_next_seq = parent_messages
            .last()
            .map(|row| row.get::<i64, _>("session_seq") + 1)
            .unwrap_or(1);

        if fork_point < 1 || fork_point >= parent_next_seq {
            release_fork_guard(guard).await?;
            return Err(LocalForkError::InvalidForkPoint(format!(
                "illegal fork_point {}: must be 1 <= fp < {}",
                fork_point, parent_next_seq
            )));
        }

        let namespace = Uuid::NAMESPACE_OID;
        let hash_input = format!(
            "santi_fork:{}:{}:{}",
            parent_session_id, fork_point, request_id
        );
        let new_session_id = format!(
            "sess_{}",
            Uuid::new_v5(&namespace, hash_input.as_bytes()).simple()
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
                return Ok(LocalForkResult {
                    new_session_id,
                    parent_session_id: parent_session_id.to_string(),
                    fork_point,
                });
            }
            return Err(LocalForkError::Internal(
                "existing fork session id collided with incompatible lineage".to_string(),
            ));
        }

        let copied_messages: Vec<_> = parent_messages
            .into_iter()
            .filter(|row| row.get::<i64, _>("session_seq") <= fork_point)
            .collect();

        sqlx::query(
            r#"INSERT INTO sessions (id, parent_session_id, fork_point, session_memory, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
        )
        .bind(&new_session_id)
        .bind(parent_session_id)
        .bind(fork_point)
        .bind(parent_session.get::<String, _>("session_memory"))
        .execute(&self.pool)
        .await
        .map_err(map_fork_sql_error)?;

        for (index, row) in copied_messages.into_iter().enumerate() {
            sqlx::query(
                r#"INSERT INTO session_messages
                   (id, session_id, session_seq, actor_type, actor_id, content_text, state, created_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)"#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&new_session_id)
            .bind((index as i64) + 1)
            .bind(row.get::<String, _>("actor_type"))
            .bind(row.get::<String, _>("actor_id"))
            .bind(row.get::<String, _>("content_text"))
            .bind(row.get::<String, _>("state"))
            .execute(&self.pool)
            .await
            .map_err(map_fork_sql_error)?;
        }

        let parent_soul_session = self
            .ensure_soul_session(parent_session_id, None, None, parent_next_seq)
            .await
            .map_err(map_fork_core_error)?;
        self.ensure_soul_session(
            &new_session_id,
            Some(parent_soul_session.id),
            Some(fork_point),
            fork_point + 1,
        )
        .await
        .map_err(map_fork_core_error)?;

        release_fork_guard(guard).await?;

        Ok(LocalForkResult {
            new_session_id,
            parent_session_id: parent_session_id.to_string(),
            fork_point,
        })
    }

    pub async fn compact_session(
        &self,
        session_id: &str,
        summary: &str,
    ) -> std::result::Result<Compact, LocalCompactError> {
        let guard = self
            .lock
            .acquire(&format!("lock:session_send:{session_id}"))
            .await
            .map_err(map_compact_lock_error)?;

        if summary.trim().is_empty() {
            release_compact_guard(guard).await?;
            return Err(LocalCompactError::Invalid(
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
            return Err(LocalCompactError::NotFound);
        }

        let end_session_seq = sqlx::query(
            r#"SELECT MAX(session_seq) AS end_session_seq FROM session_messages WHERE session_id = ?1"#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_compact_sql_error)?
        .try_get::<Option<i64>, _>("end_session_seq")
        .map_err(|err| LocalCompactError::Internal(format!("decode local compact range failed: {err}")))?
        .ok_or_else(|| LocalCompactError::Invalid("cannot compact empty session".to_string()))?;

        let start_session_seq = sqlx::query(
            r#"SELECT MAX(end_session_seq) AS max_end_session_seq FROM local_session_compacts WHERE session_id = ?1"#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_compact_sql_error)?
        .try_get::<Option<i64>, _>("max_end_session_seq")
        .map_err(|err| LocalCompactError::Internal(format!("decode local compact head failed: {err}")))?
        .map(|seq| seq + 1)
        .unwrap_or(1);

        if start_session_seq > end_session_seq {
            release_compact_guard(guard).await?;
            return Err(LocalCompactError::Invalid(
                "no uncompacted session range".to_string(),
            ));
        }

        let compact = Compact {
            id: format!("compact_{}", Uuid::new_v4().simple()),
            turn_id: format!("turn_local_compact_{}", Uuid::new_v4().simple()),
            summary: summary.trim().to_string(),
            start_session_seq,
            end_session_seq,
            created_at: current_timestamp_string(&self.pool)
                .await
                .map_err(map_compact_core_error)?,
        };

        sqlx::query(
            r#"INSERT OR REPLACE INTO local_session_compacts
               (id, session_id, turn_id, summary, start_session_seq, end_session_seq)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
        )
        .bind(&compact.id)
        .bind(session_id)
        .bind(&compact.turn_id)
        .bind(&compact.summary)
        .bind(compact.start_session_seq)
        .bind(compact.end_session_seq)
        .execute(&self.pool)
        .await
        .map_err(map_compact_sql_error)?;

        self.ensure_soul_session(session_id, None, None, end_session_seq + 1)
            .await
            .map_err(map_compact_core_error)?;

        release_compact_guard(guard).await?;

        Ok(compact)
    }

    pub async fn list_compacts(&self, session_id: &str) -> Result<Vec<Compact>> {
        let rows = sqlx::query(
            r#"SELECT id, turn_id, summary, start_session_seq, end_session_seq, created_at
               FROM local_session_compacts
               WHERE session_id = ?1
               ORDER BY created_at ASC"#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("list local compacts failed: {err}"),
        })?;

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

    async fn ensure_soul_session(
        &self,
        session_id: &str,
        parent_soul_session_id: Option<String>,
        fork_point: Option<i64>,
        next_seq: i64,
    ) -> Result<LocalSoulSessionRow> {
        sqlx::query(
            r#"INSERT INTO local_soul_sessions
               (id, soul_id, session_id, session_memory, next_seq, last_seen_session_seq, parent_soul_session_id, fork_point)
               VALUES (?1, 'soul_default', ?2, '', ?3, 0, ?4, ?5)
               ON CONFLICT(session_id) DO UPDATE SET updated_at = local_soul_sessions.updated_at"#,
        )
        .bind(format!("ss_local_{session_id}"))
        .bind(session_id)
        .bind(next_seq)
        .bind(parent_soul_session_id)
        .bind(fork_point)
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("ensure local soul_session failed: {err}"),
        })?;

        let row = sqlx::query(
            r#"SELECT id, session_id, next_seq FROM local_soul_sessions WHERE session_id = ?1 LIMIT 1"#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("load local soul_session failed: {err}"),
        })?;

        Ok(LocalSoulSessionRow { id: row.get("id") })
    }
}

#[derive(Clone, Debug)]
struct LocalSoulSessionRow {
    id: String,
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

async fn release_fork_guard(
    guard: Box<dyn LockGuard + Send>,
) -> std::result::Result<(), LocalForkError> {
    guard.release().await.map_err(map_lock_error)
}

async fn release_compact_guard(
    guard: Box<dyn LockGuard + Send>,
) -> std::result::Result<(), LocalCompactError> {
    guard.release().await.map_err(map_compact_lock_error)
}

fn map_fork_sql_error(err: sqlx::Error) -> LocalForkError {
    LocalForkError::Internal(format!("local fork sqlite failed: {err}"))
}

fn map_fork_core_error(err: Error) -> LocalForkError {
    match err {
        Error::NotFound { .. } => LocalForkError::ParentNotFound,
        Error::Busy { resource } => LocalForkError::Internal(format!("{resource} busy")),
        Error::InvalidInput { message } => LocalForkError::InvalidForkPoint(message),
        Error::Upstream { message } | Error::Internal { message } => {
            LocalForkError::Internal(message)
        }
    }
}

fn map_lock_error(err: LockError) -> LocalForkError {
    match err {
        LockError::Busy => LocalForkError::Busy,
        LockError::Lost => LocalForkError::Internal("fork session lock lost".to_string()),
        LockError::Backend { message } => LocalForkError::Internal(message),
    }
}

fn map_compact_sql_error(err: sqlx::Error) -> LocalCompactError {
    LocalCompactError::Internal(format!("local compact sqlite failed: {err}"))
}

fn map_compact_core_error(err: Error) -> LocalCompactError {
    match err {
        Error::NotFound { .. } => LocalCompactError::NotFound,
        Error::Busy { resource } => LocalCompactError::Internal(format!("{resource} busy")),
        Error::InvalidInput { message } => LocalCompactError::Invalid(message),
        Error::Upstream { message } | Error::Internal { message } => {
            LocalCompactError::Internal(message)
        }
    }
}

fn map_compact_lock_error(err: LockError) -> LocalCompactError {
    match err {
        LockError::Busy => LocalCompactError::Busy,
        LockError::Lost => LocalCompactError::Internal("session compact lock lost".to_string()),
        LockError::Backend { message } => LocalCompactError::Internal(message),
    }
}
