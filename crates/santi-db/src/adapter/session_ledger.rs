use serde_json::{Map, Value};
use sqlx::{postgres::PgRow, PgPool, Row};

use santi_core::{
    error::{Error, Result},
    model::{
        message::{
            ActorType, Message, MessageContent, MessageEventPayload, MessagePart, MessageState,
        },
        session::{Session, SessionMessage, SessionMessageRef},
    },
    port::session_ledger::{AppendSessionMessage, ApplyMessageEvent, SessionLedgerPort},
};

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
        let is_delete = matches!(input.payload, MessageEventPayload::Delete { .. });
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

fn apply_message_event_to_message(
    mut message: Message,
    actor_type: &ActorType,
    actor_id: &str,
    base_version: i64,
    payload: &MessageEventPayload,
) -> Result<Message> {
    if &message.actor_type != actor_type || message.actor_id != actor_id {
        return Err(Error::InvalidInput {
            message: "only the original actor may mutate a message".to_string(),
        });
    }

    if message.version != base_version {
        return Err(Error::InvalidInput {
            message: format!(
                "message version mismatch: expected {}, got {}",
                message.version, base_version
            ),
        });
    }

    if message.deleted_at.is_some() {
        return Err(Error::InvalidInput {
            message: "deleted messages cannot be mutated".to_string(),
        });
    }

    if message.state == MessageState::Fixed {
        return Err(Error::InvalidInput {
            message: "fixed messages cannot be mutated".to_string(),
        });
    }

    match payload {
        MessageEventPayload::Patch { patches } => {
            let mut parts = message.content.parts.clone();
            for patch in patches {
                let index = valid_index(parts.len(), patch.index, "patch")?;
                parts[index] = merge_message_part(&parts[index], &patch.merge)?;
            }
            message.content = MessageContent { parts };
        }
        MessageEventPayload::Insert { items } => {
            let mut parts = message.content.parts.clone();
            let mut sorted_items = items.clone();
            sorted_items.sort_by_key(|item| item.index);
            for item in sorted_items {
                let index = valid_insert_index(parts.len(), item.index)?;
                parts.insert(index, item.part);
            }
            message.content = MessageContent { parts };
        }
        MessageEventPayload::Remove { indexes } => {
            let mut unique_indexes = indexes.clone();
            unique_indexes.sort_unstable();
            unique_indexes.dedup();
            let parts_len = message.content.parts.len();
            for index in &unique_indexes {
                let _ = valid_index(parts_len, *index, "remove")?;
            }

            let mut parts = message.content.parts.clone();
            for index in unique_indexes.into_iter().rev() {
                parts.remove(index as usize);
            }
            message.content = MessageContent { parts };
        }
        MessageEventPayload::Fix => {
            message.state = MessageState::Fixed;
        }
        MessageEventPayload::Delete { .. } => {
            message.deleted_at = Some(String::new());
        }
    }

    message.version += 1;
    Ok(message)
}

fn valid_index(len: usize, raw: i64, action: &str) -> Result<usize> {
    if raw < 0 || raw as usize >= len {
        return Err(Error::InvalidInput {
            message: format!("{action} index out of bounds: {raw}"),
        });
    }
    Ok(raw as usize)
}

fn valid_insert_index(len: usize, raw: i64) -> Result<usize> {
    if raw < 0 || raw as usize > len {
        return Err(Error::InvalidInput {
            message: format!("insert index out of bounds: {raw}"),
        });
    }
    Ok(raw as usize)
}

fn merge_message_part(part: &MessagePart, merge: &Value) -> Result<MessagePart> {
    let mut base = serde_json::to_value(part).map_err(|err| Error::Internal {
        message: format!("message part serialize failed: {err}"),
    })?;

    let merge_object = merge.as_object().ok_or(Error::InvalidInput {
        message: "patch merge must be an object".to_string(),
    })?;

    let base_object = base.as_object_mut().ok_or(Error::Internal {
        message: "message part must serialize to an object".to_string(),
    })?;

    merge_json_object(base_object, merge_object);

    serde_json::from_value(base).map_err(|err| Error::InvalidInput {
        message: format!("patch produced invalid message part: {err}"),
    })
}

fn merge_json_object(base: &mut Map<String, Value>, merge: &Map<String, Value>) {
    for (key, value) in merge {
        base.insert(key.clone(), value.clone());
    }
}

fn map_session_message_row(row: PgRow) -> Result<SessionMessage> {
    Ok(SessionMessage {
        relation: SessionMessageRef {
            session_id: row.get("session_id"),
            message_id: row.get("message_id"),
            session_seq: row.get("session_seq"),
            created_at: row.get("relation_created_at"),
        },
        message: map_message_row(&row)?,
    })
}

fn map_message_row(row: &PgRow) -> Result<Message> {
    Ok(Message {
        id: row.get("id"),
        actor_type: parse_actor_type(row.get::<String, _>("actor_type").as_str())?,
        actor_id: row.get("actor_id"),
        content: row.get::<sqlx::types::Json<MessageContent>, _>("content").0,
        state: parse_message_state(row.get::<String, _>("state").as_str())?,
        version: row.get("version"),
        deleted_at: row.try_get("deleted_at").ok(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn parse_actor_type(value: &str) -> Result<ActorType> {
    match value {
        "account" => Ok(ActorType::Account),
        "soul" => Ok(ActorType::Soul),
        "system" => Ok(ActorType::System),
        _ => Err(Error::Internal {
            message: format!("unknown actor_type: {value}"),
        }),
    }
}

fn parse_message_state(value: &str) -> Result<MessageState> {
    match value {
        "pending" => Ok(MessageState::Pending),
        "fixed" => Ok(MessageState::Fixed),
        _ => Err(Error::Internal {
            message: format!("unknown message state: {value}"),
        }),
    }
}

fn actor_type_str(value: &ActorType) -> &'static str {
    match value {
        ActorType::Account => "account",
        ActorType::Soul => "soul",
        ActorType::System => "system",
    }
}

fn message_state_str(value: &MessageState) -> &'static str {
    match value {
        MessageState::Pending => "pending",
        MessageState::Fixed => "fixed",
    }
}

fn payload_action_str(value: &MessageEventPayload) -> &'static str {
    match value {
        MessageEventPayload::Patch { .. } => "patch",
        MessageEventPayload::Insert { .. } => "insert",
        MessageEventPayload::Remove { .. } => "remove",
        MessageEventPayload::Fix => "fix",
        MessageEventPayload::Delete { .. } => "delete",
    }
}

#[cfg(test)]
mod tests {
    use santi_core::{
        error::Error,
        model::message::{
            ActorType, Message, MessageContent, MessageEventPayload, MessageInsertItem,
            MessagePart, MessagePartPatch, MessageState,
        },
    };

    use super::apply_message_event_to_message;

    fn pending_message(parts: Vec<MessagePart>) -> Message {
        Message {
            id: "msg_1".to_string(),
            actor_type: ActorType::Account,
            actor_id: "acct_1".to_string(),
            content: MessageContent { parts },
            state: MessageState::Pending,
            version: 1,
            deleted_at: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[test]
    fn patch_updates_part_and_increments_version() {
        let message = pending_message(vec![MessagePart::Text {
            text: "hello".to_string(),
        }]);

        let updated = apply_message_event_to_message(
            message,
            &ActorType::Account,
            "acct_1",
            1,
            &MessageEventPayload::Patch {
                patches: vec![MessagePartPatch {
                    index: 0,
                    merge: serde_json::json!({ "text": "world" }),
                }],
            },
        )
        .unwrap();

        assert_eq!(updated.version, 2);
        assert_eq!(
            updated.content.parts,
            vec![MessagePart::Text {
                text: "world".to_string()
            }]
        );
    }

    #[test]
    fn remove_rejects_out_of_bounds_index() {
        let message = pending_message(vec![MessagePart::Text {
            text: "hello".to_string(),
        }]);

        let err = apply_message_event_to_message(
            message,
            &ActorType::Account,
            "acct_1",
            1,
            &MessageEventPayload::Remove { indexes: vec![1] },
        )
        .unwrap_err();

        assert_eq!(
            err,
            Error::InvalidInput {
                message: "remove index out of bounds: 1".to_string(),
            }
        );
    }

    #[test]
    fn fixed_message_rejects_further_mutation() {
        let mut message = pending_message(vec![MessagePart::Text {
            text: "hello".to_string(),
        }]);
        message.state = MessageState::Fixed;

        let err = apply_message_event_to_message(
            message,
            &ActorType::Account,
            "acct_1",
            1,
            &MessageEventPayload::Delete { reason: None },
        )
        .unwrap_err();

        assert_eq!(
            err,
            Error::InvalidInput {
                message: "fixed messages cannot be mutated".to_string(),
            }
        );
    }

    #[test]
    fn insert_uses_exact_indexes_without_gap_creation() {
        let message = pending_message(vec![MessagePart::Text {
            text: "a".to_string(),
        }]);

        let updated = apply_message_event_to_message(
            message,
            &ActorType::Account,
            "acct_1",
            1,
            &MessageEventPayload::Insert {
                items: vec![MessageInsertItem {
                    index: 1,
                    part: MessagePart::Text {
                        text: "b".to_string(),
                    },
                }],
            },
        )
        .unwrap();

        assert_eq!(
            updated.content.parts,
            vec![
                MessagePart::Text {
                    text: "a".to_string()
                },
                MessagePart::Text {
                    text: "b".to_string()
                }
            ]
        );
    }
}
