use sqlx::{PgPool, Row};

use santi_core::{
    error::{Error, Result},
    model::effect::SessionEffect,
    port::effect_ledger::{CreateSessionEffect, EffectLedgerPort, UpdateSessionEffect},
};

#[derive(Clone)]
pub struct DbEffectLedger {
    pool: PgPool,
}

impl DbEffectLedger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl EffectLedgerPort for DbEffectLedger {
    async fn list_effects(&self, session_id: &str) -> Result<Vec<SessionEffect>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                session_id,
                effect_type,
                idempotency_key,
                status,
                source_hook_id,
                source_turn_id,
                result_ref,
                error_text,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM session_effects
            WHERE session_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("effect list failed: {err}"),
        })?;

        Ok(rows.into_iter().map(map_session_effect_row).collect())
    }

    async fn get_effect(
        &self,
        session_id: &str,
        effect_type: &str,
        idempotency_key: &str,
    ) -> Result<Option<SessionEffect>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                session_id,
                effect_type,
                idempotency_key,
                status,
                source_hook_id,
                source_turn_id,
                result_ref,
                error_text,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM session_effects
            WHERE session_id = $1 AND effect_type = $2 AND idempotency_key = $3
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .bind(effect_type)
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("effect get failed: {err}"),
        })?;

        Ok(row.map(map_session_effect_row))
    }

    async fn create_effect(&self, input: CreateSessionEffect) -> Result<SessionEffect> {
        let row = sqlx::query(
            r#"
            INSERT INTO session_effects (
                id,
                session_id,
                effect_type,
                idempotency_key,
                status,
                source_hook_id,
                source_turn_id,
                result_ref,
                error_text
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (session_id, effect_type, idempotency_key)
            DO UPDATE SET updated_at = session_effects.updated_at
            RETURNING
                id,
                session_id,
                effect_type,
                idempotency_key,
                status,
                source_hook_id,
                source_turn_id,
                result_ref,
                error_text,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(input.effect_id)
        .bind(input.session_id)
        .bind(input.effect_type)
        .bind(input.idempotency_key)
        .bind(input.status)
        .bind(input.source_hook_id)
        .bind(input.source_turn_id)
        .bind(input.result_ref)
        .bind(input.error_text)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("effect create failed: {err}"),
        })?;

        Ok(map_session_effect_row(row))
    }

    async fn update_effect(&self, input: UpdateSessionEffect) -> Result<Option<SessionEffect>> {
        let row = sqlx::query(
            r#"
            UPDATE session_effects
            SET status = $2,
                result_ref = $3,
                error_text = $4,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                session_id,
                effect_type,
                idempotency_key,
                status,
                source_hook_id,
                source_turn_id,
                result_ref,
                error_text,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(input.effect_id)
        .bind(input.status)
        .bind(input.result_ref)
        .bind(input.error_text)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("effect update failed: {err}"),
        })?;

        Ok(row.map(map_session_effect_row))
    }
}

fn map_session_effect_row(row: sqlx::postgres::PgRow) -> SessionEffect {
    SessionEffect {
        id: row.get("id"),
        session_id: row.get("session_id"),
        effect_type: row.get("effect_type"),
        idempotency_key: row.get("idempotency_key"),
        status: row.get("status"),
        source_hook_id: row.get("source_hook_id"),
        source_turn_id: row.get("source_turn_id"),
        result_ref: row.try_get("result_ref").ok(),
        error_text: row.try_get("error_text").ok(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
