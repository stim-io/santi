use sqlx::PgPool;

mod compact;
mod fork;
mod helpers;
mod runtime_port;

#[derive(Clone)]
pub struct DbSoulRuntime {
    pub(super) pool: PgPool,
}

impl DbSoulRuntime {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn allocate_seq(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        soul_session_id: &str,
    ) -> santi_core::error::Result<i64> {
        let row = sqlx::query(
            r#"
            UPDATE soul_sessions
            SET next_seq = next_seq + 1, updated_at = NOW()
            WHERE id = $1
            RETURNING next_seq - 1 AS allocated_seq
            "#,
        )
        .bind(soul_session_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|err| santi_core::error::Error::Internal {
            message: format!("allocate soul session seq failed: {err}"),
        })?
        .ok_or(santi_core::error::Error::NotFound {
            resource: "soul_session",
        })?;

        Ok(sqlx::Row::get(&row, "allocated_seq"))
    }
}
