use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::model::soul::Soul;

#[derive(Clone)]
pub struct SoulRepo {
    pool: PgPool,
}

impl SoulRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl SoulRepo {
    pub async fn get(&self, soul_id: &str) -> Result<Option<Soul>, sqlx::Error> {
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
        .await?;

        Ok(row.map(|row| Soul {
            id: row.get("id"),
            memory: row.get("memory"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    pub async fn update_memory(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        soul_id: &str,
        memory: &str,
    ) -> Result<Option<Soul>, sqlx::Error> {
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
        .bind(memory)
        .fetch_optional(&mut **tx)
        .await?;

        Ok(row.map(|row| Soul {
            id: row.get("id"),
            memory: row.get("memory"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }
}
