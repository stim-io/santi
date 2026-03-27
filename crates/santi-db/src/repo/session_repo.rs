use sqlx::{PgPool, Postgres, Row, Transaction};



#[derive(Clone)]
pub struct SessionRepo {
    pool: PgPool,
}

#[derive(Clone, Debug)]\npub struct RepoSession {\n    pub id: String,\n    pub soul_id: String,\n    pub memory: String,\n    pub created_at: String,\n    pub updated_at: String,\n}\n\nimpl SessionRepo {\n    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn begin_tx(&self) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
        self.pool.begin().await
    }

    pub async fn create(
        &self,
        session_id: &str,
        soul_id: &str,
    ) -> Result<Session, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO sessions (id, soul_id)
            VALUES ($1, $2)
            RETURNING
                id,
                soul_id,
                memory,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(session_id)
        .bind(soul_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(RepoSession {\n            id: row.get(\"id\"),\n            soul_id: row.get(\"soul_id\"),\n            memory: row.get(\"memory\"),\n            created_at: row.get(\"created_at\"),\n            updated_at: row.get(\"updated_at\"),\n        })
    }
}

impl SessionRepo {
    pub async fn get(&self, session_id: &str) -> Result<Option<Session>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                soul_id,
                memory,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM sessions
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| RepoSession {\n            id: row.get(\"id\"),\n            soul_id: row.get(\"soul_id\"),\n            memory: row.get(\"memory\"),\n            created_at: row.get(\"created_at\"),\n            updated_at: row.get(\"updated_at\"),\n        }))
    }

    pub async fn exists(&self, session_id: &str) -> Result<bool, sqlx::Error> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM sessions
                WHERE id = $1
            )
            "#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(exists)
    }

    pub async fn allocate_next_session_seq(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        session_id: &str,
    ) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            r#"
            UPDATE sessions
            SET next_session_seq = next_session_seq + 1,
                updated_at = NOW()
            WHERE id = $1
            RETURNING next_session_seq - 1 AS allocated_seq
            "#,
        )
        .bind(session_id)
        .fetch_one(&mut **tx)
        .await?;

        Ok(row.get("allocated_seq"))
    }

    pub async fn update_memory(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        session_id: &str,
        memory: &str,
    ) -> Result<Option<Session>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            UPDATE sessions
            SET memory = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                soul_id,
                memory,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(session_id)
        .bind(memory)
        .fetch_optional(&mut **tx)
        .await?;

        Ok(row.map(|row| RepoSession {\n            id: row.get(\"id\"),\n            soul_id: row.get(\"soul_id\"),\n            memory: row.get(\"memory\"),\n            created_at: row.get(\"created_at\"),\n            updated_at: row.get(\"updated_at\"),\n        }))
    }
}
