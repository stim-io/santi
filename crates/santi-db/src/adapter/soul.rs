use sqlx::{PgPool, Row};

use santi_core::{error::{Error, Result}, model::soul::Soul, port::soul::SoulPort};

#[derive(Clone)]
pub struct DbSoul {
    pool: PgPool,
}

impl DbSoul {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SoulPort for DbSoul {
    async fn get_soul(&self, soul_id: &str) -> Result<Option<Soul>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                memory,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM souls
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(soul_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("soul get failed: {err}") })?;

        Ok(row.map(|row| Soul {
            id: row.get("id"),
            memory: row.get("memory"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn write_soul_memory(&self, soul_id: &str, text: &str) -> Result<Option<Soul>> {
        let row = sqlx::query(
            r#"
            UPDATE souls
            SET memory = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                memory,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(soul_id)
        .bind(text)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("soul memory update failed: {err}") })?;

        Ok(row.map(|row| Soul {
            id: row.get("id"),
            memory: row.get("memory"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }
}
