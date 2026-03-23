use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::model::message::Message;

#[derive(Clone)]
pub struct MessageRepo {
    pool: PgPool,
}

impl MessageRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub struct NewMessage<'a> {
    pub id: &'a str,
    pub r#type: &'a str,
    pub role: Option<&'a str>,
    pub content: &'a str,
}

impl MessageRepo {
    pub async fn insert(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        input: NewMessage<'_>,
    ) -> Result<Message, sqlx::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO messages (id, type, role, content)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id,
                type,
                role,
                content,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
            "#,
        )
        .bind(input.id)
        .bind(input.r#type)
        .bind(input.role)
        .bind(input.content)
        .fetch_one(&mut **tx)
        .await?;

        Ok(Message {
            id: row.get("id"),
            r#type: row.get("type"),
            role: row.get("role"),
            content: row.get("content"),
            created_at: row.get("created_at"),
        })
    }

    pub async fn list_for_session(&self, session_id: &str) -> Result<Vec<Message>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                m.id,
                m.type,
                m.role,
                m.content,
                to_char(m.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
            FROM r_session_messages rsm
            JOIN messages m ON m.id = rsm.message_id
            WHERE rsm.session_id = $1
            ORDER BY rsm.session_seq ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| Message {
                id: row.get("id"),
                r#type: row.get("type"),
                role: row.get("role"),
                content: row.get("content"),
                created_at: row.get("created_at"),
            })
            .collect())
    }
}
