use std::path::Path;

use santi_core::{
    error::{Error, Result},
    model::runtime::SoulSession,
};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use super::{helpers::map_soul_session_row, StandaloneSoulRuntime};

pub(super) async fn create_pool(path: &Path) -> Result<SqlitePool> {
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
        r#"CREATE TABLE IF NOT EXISTS standalone_turns (
            id TEXT PRIMARY KEY,
            soul_session_id TEXT NOT NULL,
            trigger_type TEXT NOT NULL,
            trigger_ref TEXT NULL,
            input_through_session_seq INTEGER NOT NULL,
            base_soul_session_seq INTEGER NOT NULL,
            end_soul_session_seq INTEGER NULL,
            status TEXT NOT NULL,
            error_text TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            finished_at TEXT NULL
        )"#,
    )
    .execute(&pool)
    .await
    .map_err(|err| Error::Internal {
        message: format!("migrate sqlite standalone_turns failed: {err}"),
    })?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS standalone_soul_session_items (
            soul_session_id TEXT NOT NULL,
            target_type TEXT NOT NULL,
            target_id TEXT NOT NULL,
            soul_session_seq INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (soul_session_id, soul_session_seq)
        )"#,
    )
    .execute(&pool)
    .await
    .map_err(|err| Error::Internal {
        message: format!("migrate sqlite standalone_soul_session_items failed: {err}"),
    })?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            parent_session_id TEXT NULL,
            fork_point INTEGER NULL,
            session_memory TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )"#,
    )
    .execute(&pool)
    .await
    .map_err(|err| Error::Internal {
        message: format!("migrate sqlite sessions failed: {err}"),
    })?;

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

    Ok(pool)
}

impl StandaloneSoulRuntime {
    pub(super) async fn ensure_soul_session(&self, soul_id: &str, session_id: &str) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO standalone_soul_sessions (id, soul_id, session_id)
               VALUES (?1, ?2, ?3)
               ON CONFLICT(session_id) DO UPDATE SET updated_at = standalone_soul_sessions.updated_at"#,
        )
        .bind(Self::standalone_soul_session_id(session_id))
        .bind(soul_id)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("ensure standalone soul_session failed: {err}"),
        })?;

        Ok(())
    }

    pub(super) async fn fetch_soul_session_by_id(
        &self,
        soul_session_id: &str,
    ) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"SELECT id, soul_id, session_id, session_memory, provider_state, next_seq,
                      last_seen_session_seq, parent_soul_session_id, fork_point,
                      created_at, updated_at
               FROM standalone_soul_sessions
               WHERE id = ?1
               LIMIT 1"#,
        )
        .bind(soul_session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone soul_session get failed: {err}"),
        })?;

        row.map(map_soul_session_row).transpose()
    }

    pub(super) async fn fetch_soul_session_by_session_id(
        &self,
        session_id: &str,
    ) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"SELECT id, soul_id, session_id, session_memory, provider_state, next_seq,
                      last_seen_session_seq, parent_soul_session_id, fork_point,
                      created_at, updated_at
               FROM standalone_soul_sessions
               WHERE session_id = ?1
               LIMIT 1"#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone soul_session by session failed: {err}"),
        })?;

        row.map(map_soul_session_row).transpose()
    }

    pub(super) fn standalone_soul_session_id(session_id: &str) -> String {
        format!("ss_standalone_{session_id}")
    }
}
