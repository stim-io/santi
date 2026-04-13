use serde_json::Value;
use sqlx::Row;

use santi_core::{
    error::{Error, Result},
    port::soul_session_fork::SoulSessionForkPort,
};

use super::{
    helpers::{decode_provider_state, encode_provider_state, map_soul_session_row},
    DbSoulRuntime,
};

#[async_trait::async_trait]
impl SoulSessionForkPort for DbSoulRuntime {
    async fn fork_soul_session(
        &self,
        parent_soul_session_id: &str,
        fork_point: i64,
        new_soul_session_id: &str,
        new_session_id: &str,
    ) -> Result<santi_core::model::runtime::SoulSession> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("fork soul session tx begin failed: {err}"),
        })?;

        let parent_row = sqlx::query(
            r#"
            SELECT soul_id, session_id, session_memory, provider_state, next_seq, last_seen_session_seq
            FROM soul_sessions
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(parent_soul_session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("load parent soul session failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "soul_session",
        })?;

        sqlx::query(
            r#"
            INSERT INTO sessions (id, parent_session_id, fork_point)
            VALUES ($1, $2, $3)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(new_session_id)
        .bind(parent_row.get::<String, _>("session_id"))
        .bind(fork_point)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert forked session failed: {err}"),
        })?;

        sqlx::query(
            r#"
            INSERT INTO r_session_messages (session_id, message_id, session_seq)
            SELECT $1, message_id, session_seq
            FROM r_session_messages
            WHERE session_id = $2
              AND session_seq <= $3
            ORDER BY session_seq ASC
            ON CONFLICT (session_id, message_id) DO NOTHING
            "#,
        )
        .bind(new_session_id)
        .bind(parent_row.get::<String, _>("session_id"))
        .bind(fork_point)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("copy forked session messages failed: {err}"),
        })?;

        let provider_state: Option<sqlx::types::Json<Value>> = parent_row
            .try_get::<Option<serde_json::Value>, _>("provider_state")
            .map_err(|err| Error::Internal {
                message: format!("decode parent provider_state failed: {err}"),
            })?
            .map(decode_provider_state)
            .transpose()?
            .filter(|state| state.basis_soul_session_seq <= fork_point)
            .map(|state| sqlx::types::Json(encode_provider_state(&state)));

        let row = sqlx::query(
            r#"
            INSERT INTO soul_sessions (
                id, soul_id, session_id, session_memory, provider_state,
                next_seq, last_seen_session_seq, parent_soul_session_id, fork_point
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO UPDATE SET updated_at = soul_sessions.updated_at
            RETURNING id, soul_id, session_id, session_memory, provider_state, next_seq,
                      last_seen_session_seq, parent_soul_session_id, fork_point,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                      to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(new_soul_session_id)
        .bind(parent_row.get::<String, _>("soul_id"))
        .bind(new_session_id)
        .bind(parent_row.get::<String, _>("session_memory"))
        .bind(provider_state)
        .bind(fork_point + 1)
        .bind(std::cmp::min(
            parent_row.get::<i64, _>("last_seen_session_seq"),
            fork_point,
        ))
        .bind(parent_soul_session_id)
        .bind(fork_point)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert forked soul session failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("fork soul session tx commit failed: {err}"),
        })?;

        map_soul_session_row(&row)
    }
}
