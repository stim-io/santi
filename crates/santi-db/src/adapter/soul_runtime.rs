use sqlx::{postgres::PgRow, PgPool, Row};
use uuid::Uuid;

use santi_core::{
    error::{Error, Result},
    model::{
        message::{ActorType, Message, MessageContent, MessageState},
        runtime::{
            AssemblyItem, AssemblyTarget, SoulSession, SoulSessionEntry, SoulSessionTargetType,
            Turn, TurnStatus, TurnTriggerType,
        },
        session::{SessionMessage, SessionMessageRef},
    },
    port::soul_runtime::{
        AcquireSoulSession, AppendCompact, AppendMessageRef, AppendToolCall, AppendToolResult,
        CompleteTurn, FailTurn, SoulRuntimePort, StartTurn,
    },
};

#[derive(Clone)]
pub struct DbSoulRuntime {
    pool: PgPool,
}

impl DbSoulRuntime {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SoulRuntimePort for DbSoulRuntime {
    async fn acquire_soul_session(&self, input: AcquireSoulSession) -> Result<SoulSession> {
        let row = sqlx::query(
            r#"
            INSERT INTO soul_sessions (id, soul_id, session_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (soul_id, session_id)
            DO UPDATE SET updated_at = soul_sessions.updated_at
            RETURNING
                id, soul_id, session_id, session_memory, provider_state, next_seq,
                last_seen_session_seq, parent_soul_session_id, fork_point,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(format!("ss_{}", Uuid::new_v4().simple()))
        .bind(input.soul_id)
        .bind(input.session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("acquire soul_session failed: {err}"),
        })?;
        map_soul_session_row(&row)
    }

    async fn get_soul_session(&self, soul_session_id: &str) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"
            SELECT id, soul_id, session_id, session_memory, provider_state, next_seq,
                   last_seen_session_seq, parent_soul_session_id, fork_point,
                   to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                   to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM soul_sessions
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(soul_session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("soul_session get failed: {err}") })?;
        row.map(|row| map_soul_session_row(&row)).transpose()
    }

    async fn write_session_memory(&self, soul_session_id: &str, text: &str) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"
            UPDATE soul_sessions SET session_memory = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, soul_id, session_id, session_memory, provider_state, next_seq,
                      last_seen_session_seq, parent_soul_session_id, fork_point,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                      to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(soul_session_id)
        .bind(text)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("session memory update failed: {err}") })?;
        row.map(|row| map_soul_session_row(&row)).transpose()
    }

    async fn start_turn(&self, input: StartTurn) -> Result<Turn> {
        let row = sqlx::query(
            r#"
            INSERT INTO turns (id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq, base_soul_session_seq, status)
            SELECT $1, $2, $3, $4, $5, next_seq - 1, 'running' FROM soul_sessions WHERE id = $2
            RETURNING id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
                      base_soul_session_seq, end_soul_session_seq, status, error_text,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                      to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at,
                      to_char(finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS finished_at
            "#,
        )
        .bind(&input.turn_id)
        .bind(&input.soul_session_id)
        .bind(match input.trigger_type { TurnTriggerType::SessionSend => "session_send", TurnTriggerType::System => "system" })
        .bind(&input.trigger_ref)
        .bind(input.input_through_session_seq)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("turn start failed: {err}") })?
        .ok_or(Error::NotFound { resource: "soul_session" })?;
        map_turn_row(&row)
    }

    async fn append_message_ref(&self, input: AppendMessageRef) -> Result<AssemblyItem> {
        let row = sqlx::query(
            r#"
            WITH advanced AS (
                UPDATE soul_sessions SET next_seq = next_seq + 1, updated_at = NOW()
                WHERE id = $1 RETURNING next_seq - 1 AS allocated_seq
            )
            INSERT INTO r_soul_session_messages (soul_session_id, target_type, target_id, soul_session_seq)
            SELECT $1, 'message', $2, allocated_seq FROM advanced
            RETURNING soul_session_id, target_id, soul_session_seq,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
            "#,
        )
        .bind(&input.soul_session_id)
        .bind(&input.message_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("append message ref failed: {err}") })?;

        Ok(AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: row.get("soul_session_id"),
                target_type: SoulSessionTargetType::Message,
                target_id: row.get("target_id"),
                soul_session_seq: row.get("soul_session_seq"),
                created_at: row.get("created_at"),
            },
            target: AssemblyTarget::Message(SessionMessage {
                relation: SessionMessageRef {
                    session_id: String::new(),
                    message_id: input.message_id,
                    session_seq: 0,
                    created_at: row.get("created_at"),
                },
                message: Message {
                    id: row.get("target_id"),
                    actor_type: ActorType::System,
                    actor_id: "runtime_stub".to_string(),
                    content: MessageContent { parts: Vec::new() },
                    state: MessageState::Fixed,
                    version: 1,
                    deleted_at: None,
                    created_at: row.get("created_at"),
                    updated_at: row.get("created_at"),
                },
            }),
        })
    }

    async fn append_tool_call(&self, input: AppendToolCall) -> Result<AssemblyItem> { Err(Error::InvalidInput { message: format!("unsupported tool call {}", input.tool_call_id) }) }
    async fn append_tool_result(&self, input: AppendToolResult) -> Result<AssemblyItem> { Err(Error::InvalidInput { message: format!("unsupported tool result {}", input.tool_result_id) }) }
    async fn append_compact(&self, input: AppendCompact) -> Result<AssemblyItem> { Err(Error::InvalidInput { message: format!("unsupported compact {}", input.compact_id) }) }
    async fn complete_turn(&self, _input: CompleteTurn) -> Result<Turn> { Err(Error::InvalidInput { message: "unsupported complete_turn".to_string() }) }
    async fn fail_turn(&self, _input: FailTurn) -> Result<Turn> { Err(Error::InvalidInput { message: "unsupported fail_turn".to_string() }) }
    async fn get_soul_session_by_session_id(&self, session_id: &str) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"
            SELECT id, soul_id, session_id, session_memory, provider_state, next_seq,
                   last_seen_session_seq, parent_soul_session_id, fork_point,
                   to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                   to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM soul_sessions WHERE session_id = $1 LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("get_soul_session_by_session_id failed: {err}") })?;
        row.map(|row| map_soul_session_row(&row)).transpose()
    }
    async fn fork_soul_session(&self, _parent_soul_session_id: &str, _fork_point: i64, _new_soul_session_id: &str, _new_session_id: &str) -> Result<SoulSession> { Err(Error::InvalidInput { message: "unsupported fork_soul_session".to_string() }) }
}

fn map_soul_session_row(row: &PgRow) -> Result<SoulSession> {
    Ok(SoulSession {
        id: row.get("id"), soul_id: row.get("soul_id"), session_id: row.get("session_id"), session_memory: row.get("session_memory"),
        provider_state: row.try_get::<serde_json::Value, _>("provider_state").ok().and_then(|_| None), next_seq: row.get("next_seq"),
        last_seen_session_seq: row.get("last_seen_session_seq"), parent_soul_session_id: row.try_get("parent_soul_session_id").ok(), fork_point: row.try_get("fork_point").ok(),
        created_at: row.get("created_at"), updated_at: row.get("updated_at"),
    })
}

fn map_turn_row(row: &PgRow) -> Result<Turn> {
    Ok(Turn {
        id: row.get("id"), soul_session_id: row.get("soul_session_id"), trigger_type: match row.get::<String, _>("trigger_type").as_str() { "session_send" => TurnTriggerType::SessionSend, _ => TurnTriggerType::System },
        trigger_ref: row.get("trigger_ref"), input_through_session_seq: row.get("input_through_session_seq"), base_soul_session_seq: row.get("base_soul_session_seq"), end_soul_session_seq: row.get("end_soul_session_seq"),
        status: match row.get::<String, _>("status").as_str() { "running" => TurnStatus::Running, "completed" => TurnStatus::Completed, _ => TurnStatus::Failed }, error_text: row.get("error_text"),
        created_at: row.get("created_at"), updated_at: row.get("updated_at"), finished_at: row.try_get("finished_at").ok(),
    })
}
