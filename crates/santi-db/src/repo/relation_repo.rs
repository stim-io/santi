use sqlx::{Postgres, Transaction};

#[derive(Clone)]
pub struct RelationRepo;

impl RelationRepo {
    pub fn new() -> Self {
        Self
    }
}

impl RelationRepo {
    pub async fn attach_message_to_session(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        session_id: &str,
        message_id: &str,
        session_seq: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO r_session_messages (session_id, message_id, session_seq)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(session_id)
        .bind(message_id)
        .bind(session_seq)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}
