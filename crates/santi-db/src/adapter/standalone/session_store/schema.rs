use std::path::Path;

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use santi_core::error::{Error, Result};

pub(super) async fn setup_sqlite_pool(path: &Path) -> Result<SqlitePool> {
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

    Ok(pool)
}
