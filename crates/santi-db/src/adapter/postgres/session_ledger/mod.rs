use sqlx::{PgPool, Row};

use santi_core::{
    error::{Error, Result},
    model::session::{Session, SessionMessage, SessionMessageRef},
    port::session_ledger::{AppendSessionMessage, ApplyMessageEvent, SessionLedgerPort},
};

mod mapping;
mod mutation;

use mapping::{
    actor_type_str, map_message_row, map_session_message_row, message_state_str, payload_action_str,
};
use mutation::apply_message_event_to_message;

#[derive(Clone)]
pub struct DbSessionLedger {
    pool: PgPool,
}

impl DbSessionLedger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SessionLedgerPort for DbSessionLedger {
    async fn create_session(&self, session_id: &str) -> Result<Session> {
        let row = sqlx::query(
            r#"
            INSERT INTO sessions (id)
            VALUES ($1)
            RETURNING
                id,
                parent_session_id,
                fork_point,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session create failed: {err}"),
        })?;

        Ok(Session {
            id: row.get("id"),
            parent_session_id: row.try_get("parent_session_id").ok(),
            fork_point: row.try_get("fork_point").ok(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<Session>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                parent_session_id,
                fork_point,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM sessions
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session get failed: {err}"),
        })?;

        Ok(row.map(|row| Session {
            id: row.get("id"),
            parent_session_id: row.try_get("parent_session_id").ok(),
            fork_point: row.try_get("fork_point").ok(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn get_message(&self, message_id: &str) -> Result<Option<SessionMessage>> {
        let row = sqlx::query(
            r#"
            SELECT
                m.id,
                m.actor_type,
                m.actor_id,
                m.content,
                m.state,
                m.version,
                to_char(m.deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS deleted_at,
                to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(m.updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at,
                rsm.session_id,
                rsm.message_id,
                rsm.session_seq,
                to_char(rsm.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS relation_created_at
            FROM messages m
            JOIN r_session_messages rsm ON rsm.message_id = m.id
            WHERE m.id = $1
            LIMIT 1
            "#,
        )
        .bind(message_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("message get failed: {err}"),
        })?;

        row.map(map_session_message_row).transpose()
    }

    async fn list_messages(
        &self,
        session_id: &str,
        after_session_seq: Option<i64>,
    ) -> Result<Vec<SessionMessage>> {
        let rows = sqlx::query(
            r#"
            SELECT
                rsm.session_id,
                rsm.message_id,
                rsm.session_seq,
                to_char(rsm.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS relation_created_at,
                m.id,
                m.actor_type,
                m.actor_id,
                m.content,
                m.state,
                m.version,
                to_char(m.deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS deleted_at,
                to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(m.updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM r_session_messages rsm
            JOIN messages m ON m.id = rsm.message_id
            WHERE rsm.session_id = $1
              AND ($2::BIGINT IS NULL OR rsm.session_seq > $2)
            ORDER BY rsm.session_seq ASC
            "#,
        )
        .bind(session_id)
        .bind(after_session_seq)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session messages query failed: {err}"),
        })?;

        rows.into_iter().map(map_session_message_row).collect()
    }

    async fn append_message(&self, input: AppendSessionMessage) -> Result<SessionMessage> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("transaction begin failed: {err}"),
        })?;

        let exists =
            sqlx::query_scalar::<_, bool>(r#"SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1)"#)
                .bind(&input.session_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|err| Error::Internal {
                    message: format!("session exists query failed: {err}"),
                })?;

        if !exists {
            return Err(Error::NotFound {
                resource: "session",
            });
        }

        sqlx::query(
            r#"
            INSERT INTO messages (id, actor_type, actor_id, content, state, version)
            VALUES ($1, $2, $3, $4, $5, 1)
            "#,
        )
        .bind(&input.message_id)
        .bind(actor_type_str(&input.actor_type))
        .bind(&input.actor_id)
        .bind(sqlx::types::Json(&input.content))
        .bind(message_state_str(&input.state))
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("message insert failed: {err}"),
        })?;

        let session_seq = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COALESCE(MAX(session_seq), 0) + 1
            FROM r_session_messages
            WHERE session_id = $1
            "#,
        )
        .bind(&input.session_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session seq allocation failed: {err}"),
        })?;

        let relation_row = sqlx::query(
            r#"
            INSERT INTO r_session_messages (session_id, message_id, session_seq)
            VALUES ($1, $2, $3)
            RETURNING
                session_id,
                message_id,
                session_seq,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS relation_created_at
            "#,
        )
        .bind(&input.session_id)
        .bind(&input.message_id)
        .bind(session_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("session relation insert failed: {err}"),
        })?;

        let message_row = sqlx::query(
            r#"
            SELECT
                id,
                actor_type,
                actor_id,
                content,
                state,
                version,
                to_char(deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS deleted_at,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM messages
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(&input.message_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("message reload failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("transaction commit failed: {err}"),
        })?;

        Ok(SessionMessage {
            relation: SessionMessageRef {
                session_id: relation_row.get("session_id"),
                message_id: relation_row.get("message_id"),
                session_seq: relation_row.get("session_seq"),
                created_at: relation_row.get("relation_created_at"),
            },
            message: map_message_row(&message_row)?,
        })
    }

    async fn apply_message_event(&self, input: ApplyMessageEvent) -> Result<SessionMessage> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("transaction begin failed: {err}"),
        })?;

        let row = sqlx::query(
            r#"
            SELECT
                rsm.session_id,
                rsm.message_id,
                rsm.session_seq,
                to_char(rsm.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS relation_created_at,
                m.id,
                m.actor_type,
                m.actor_id,
                m.content,
                m.state,
                m.version,
                to_char(m.deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS deleted_at,
                to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(m.updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM r_session_messages rsm
            JOIN messages m ON m.id = rsm.message_id
            WHERE rsm.session_id = $1
              AND rsm.message_id = $2
            LIMIT 1
            "#,
        )
        .bind(&input.session_id)
        .bind(&input.message_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("message load failed: {err}"),
        })?
        .ok_or(Error::NotFound { resource: "message" })?;

        let session_message = map_session_message_row(row)?;
        let is_delete = matches!(
            input.payload,
            santi_core::model::message::MessageEventPayload::Delete { .. }
        );
        let updated_message = apply_message_event_to_message(
            session_message.message,
            &input.actor_type,
            &input.actor_id,
            input.base_version,
            &input.payload,
        )?;

        sqlx::query(
            r#"
            INSERT INTO message_events (id, message_id, action, actor_type, actor_id, base_version, payload)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&input.event_id)
        .bind(&input.message_id)
        .bind(payload_action_str(&input.payload))
        .bind(actor_type_str(&input.actor_type))
        .bind(&input.actor_id)
        .bind(input.base_version)
        .bind(sqlx::types::Json(&input.payload))
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("message event insert failed: {err}"),
        })?;

        sqlx::query(
            r#"
            UPDATE messages
            SET content = $2,
                state = $3,
                version = $4,
                deleted_at = CASE
                    WHEN $5 THEN NOW()
                    ELSE deleted_at
                END,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(&updated_message.id)
        .bind(sqlx::types::Json(&updated_message.content))
        .bind(message_state_str(&updated_message.state))
        .bind(updated_message.version)
        .bind(is_delete)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("message update failed: {err}"),
        })?;

        let row = sqlx::query(
            r#"
            SELECT
                rsm.session_id,
                rsm.message_id,
                rsm.session_seq,
                to_char(rsm.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS relation_created_at,
                m.id,
                m.actor_type,
                m.actor_id,
                m.content,
                m.state,
                m.version,
                to_char(m.deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS deleted_at,
                to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(m.updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM r_session_messages rsm
            JOIN messages m ON m.id = rsm.message_id
            WHERE rsm.session_id = $1
              AND rsm.message_id = $2
            LIMIT 1
            "#,
        )
        .bind(&input.session_id)
        .bind(&input.message_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("message reload failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("transaction commit failed: {err}"),
        })?;

        map_session_message_row(row)
    }
}
