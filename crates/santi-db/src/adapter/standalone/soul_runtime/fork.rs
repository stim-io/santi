use serde_json::Value;
use sqlx::Row;

use santi_core::{
    error::{Error, Result},
    port::soul_session_fork::SoulSessionForkPort,
};

use super::{
    helpers::{decode_provider_state, encode_provider_state, map_soul_session_row},
    StandaloneSoulRuntime,
};

#[async_trait::async_trait]
impl SoulSessionForkPort for StandaloneSoulRuntime {
    async fn fork_soul_session(
        &self,
        parent_soul_session_id: &str,
        fork_point: i64,
        new_soul_session_id: &str,
        new_session_id: &str,
    ) -> Result<santi_core::model::runtime::SoulSession> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("standalone fork soul session tx begin failed: {err}"),
        })?;

        let parent_row = sqlx::query(
            r#"SELECT soul_id, session_id, session_memory, provider_state, next_seq, last_seen_session_seq
               FROM standalone_soul_sessions
               WHERE id = ?1
               LIMIT 1"#,
        )
        .bind(parent_soul_session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("load standalone parent soul session failed: {err}"),
        })?
        .ok_or(Error::NotFound { resource: "soul_session" })?;

        let parent_session_id: String = parent_row.get("session_id");

        sqlx::query(
            r#"INSERT INTO sessions (id, parent_session_id, fork_point)
               VALUES (?1, ?2, ?3)
               ON CONFLICT(id) DO NOTHING"#,
        )
        .bind(new_session_id)
        .bind(&parent_session_id)
        .bind(fork_point)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert standalone forked session failed: {err}"),
        })?;

        let parent_messages = sqlx::query(
            r#"SELECT actor_type, actor_id, content_text, state, session_seq, created_at
               FROM session_messages
               WHERE session_id = ?1 AND session_seq <= ?2
               ORDER BY session_seq ASC"#,
        )
        .bind(&parent_session_id)
        .bind(fork_point)
        .fetch_all(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("load standalone forked session messages failed: {err}"),
        })?;

        for row in parent_messages {
            sqlx::query(
                r#"INSERT INTO session_messages
                   (id, session_id, session_seq, actor_type, actor_id, content_text, state, created_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            )
            .bind(format!("msg_{}", uuid::Uuid::new_v4().simple()))
            .bind(new_session_id)
            .bind(row.get::<i64, _>("session_seq"))
            .bind(row.get::<String, _>("actor_type"))
            .bind(row.get::<String, _>("actor_id"))
            .bind(row.get::<String, _>("content_text"))
            .bind(row.get::<String, _>("state"))
            .bind(row.get::<String, _>("created_at"))
            .execute(&mut *tx)
            .await
            .map_err(|err| Error::Internal {
                message: format!("copy standalone forked session message failed: {err}"),
            })?;
        }

        let provider_state: Option<String> = parent_row
            .try_get::<Option<String>, _>("provider_state")
            .map_err(|err| Error::Internal {
                message: format!("decode parent provider_state failed: {err}"),
            })?
            .map(|raw| serde_json::from_str::<Value>(&raw))
            .transpose()
            .map_err(|err| Error::Internal {
                message: format!("parse parent provider_state failed: {err}"),
            })?
            .map(decode_provider_state)
            .transpose()?
            .filter(|state| state.basis_soul_session_seq <= fork_point)
            .map(|state| serde_json::to_string(&encode_provider_state(&state)))
            .transpose()
            .map_err(|err| Error::Internal {
                message: format!("encode forked provider_state failed: {err}"),
            })?;

        let row = sqlx::query(
            r#"INSERT INTO standalone_soul_sessions (
                   id, soul_id, session_id, session_memory, provider_state,
                   next_seq, last_seen_session_seq, parent_soul_session_id, fork_point
               )
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
               ON CONFLICT(id) DO UPDATE SET updated_at = standalone_soul_sessions.updated_at
               RETURNING id, soul_id, session_id, session_memory, provider_state, next_seq,
                         last_seen_session_seq, parent_soul_session_id, fork_point,
                         created_at, updated_at"#,
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
            message: format!("insert standalone forked soul session failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("standalone fork soul session tx commit failed: {err}"),
        })?;

        map_soul_session_row(row)
    }
}
