use std::path::Path;

use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use santi_core::{
    error::{Error, Result},
    model::soul::Soul,
    port::soul::SoulPort,
};

#[derive(Clone)]
pub struct LocalSoulStore {
    pool: SqlitePool,
}

impl LocalSoulStore {
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
            r#"CREATE TABLE IF NOT EXISTS souls (id TEXT PRIMARY KEY, memory TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)"#,
        )
        .execute(&pool)
        .await
        .map_err(|err| Error::Internal { message: format!("migrate sqlite souls failed: {err}") })?;

        Ok(Self { pool })
    }

    pub async fn get_default_soul(&self) -> Result<Soul> {
        let soul_id = "soul_default";
        let row = sqlx::query(
            r#"SELECT id, memory, created_at, updated_at FROM souls WHERE id = ?1 LIMIT 1"#,
        )
        .bind(soul_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("soul get failed: {err}"),
        })?;

        if let Some(row) = row {
            return Ok(Soul {
                id: row.get("id"),
                memory: row.get("memory"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        sqlx::query(
            r#"INSERT INTO souls (id, memory, created_at, updated_at) VALUES (?1, '', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"#,
        )
        .bind(soul_id)
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("soul default init failed: {err}") })?;

        let row = sqlx::query(
            r#"SELECT id, memory, created_at, updated_at FROM souls WHERE id = ?1 LIMIT 1"#,
        )
        .bind(soul_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("soul reload failed: {err}"),
        })?;

        Ok(Soul {
            id: row.get("id"),
            memory: row.get("memory"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    pub async fn set_default_soul_memory(&self, memory: &str) -> Result<Soul> {
        let soul_id = "soul_default";
        sqlx::query(
            r#"INSERT INTO souls (id, memory, created_at, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) ON CONFLICT(id) DO UPDATE SET memory = excluded.memory, updated_at = CURRENT_TIMESTAMP"#,
        )
        .bind(soul_id)
        .bind(memory)
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("soul memory update failed: {err}") })?;

        self.get_default_soul().await
    }
}

#[async_trait::async_trait]
impl SoulPort for LocalSoulStore {
    async fn get_soul(&self, soul_id: &str) -> Result<Option<Soul>> {
        if soul_id != "soul_default" {
            return Ok(None);
        }

        self.get_default_soul().await.map(Some)
    }

    async fn write_soul_memory(&self, soul_id: &str, text: &str) -> Result<Option<Soul>> {
        if soul_id != "soul_default" {
            return Ok(None);
        }

        self.set_default_soul_memory(text).await.map(Some)
    }
}
