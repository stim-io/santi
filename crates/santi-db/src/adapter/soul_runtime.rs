use sqlx::{postgres::PgRow, PgPool, Row};
use uuid::Uuid;

use santi_core::{
    error::{Error, Result},
    model::{
        message::{ActorType, Message, MessageContent, MessageState},
        runtime::{AssemblyItem, AssemblyTarget, ProviderState, SoulSession, SoulSessionEntry, SoulSessionTargetType, ToolCall, ToolResult, Turn, TurnContext, TurnStatus, TurnTriggerType},
        session::{Session, SessionMessage, SessionMessageRef},
        soul::Soul,
    },
    port::soul_runtime::{AppendCompact, AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn, FailTurn, SoulRuntimePort, StartTurn},
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
    async fn get_or_create_soul_session(&self, soul_id: &str, session_id: &str) -> Result<SoulSession> {
        let row = sqlx::query(
            r#"
            INSERT INTO soul_sessions (id, soul_id, session_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (soul_id, session_id)
            DO UPDATE SET updated_at = soul_sessions.updated_at
            RETURNING
                id,
                soul_id,
                session_id,
                session_memory,
                provider_state,
                next_seq,
                last_seen_session_seq,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            "#,
        )
        .bind(format!("ss_{}", Uuid::new_v4().simple()))
        .bind(soul_id)
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("get_or_create soul_session failed: {err}") })?;

        map_soul_session_row(&row)
    }

    async fn get_soul_session(&self, soul_session_id: &str) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                soul_id,
                session_id,
                session_memory,
                provider_state,
                next_seq,
                last_seen_session_seq,
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

    async fn load_turn_context(&self, soul_id: &str, session_id: &str) -> Result<Option<TurnContext>> {
        let session_row = sqlx::query(
            r#"
            SELECT id,
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
        .map_err(|err| Error::Internal { message: format!("session load failed: {err}") })?;

        let Some(session_row) = session_row else { return Ok(None) };

        let soul_row = sqlx::query(
            r#"
            SELECT id, memory,
                   to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                   to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at
            FROM souls
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(soul_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("soul load failed: {err}") })?
        .ok_or(Error::NotFound { resource: "soul" })?;

        let soul_session = self.get_or_create_soul_session(soul_id, session_id).await?;

        Ok(Some(TurnContext {
            session: Session {
                id: session_row.get("id"),
                created_at: session_row.get("created_at"),
                updated_at: session_row.get("updated_at"),
            },
            soul_session,
            soul: Soul {
                id: soul_row.get("id"),
                memory: soul_row.get("memory"),
                created_at: soul_row.get("created_at"),
                updated_at: soul_row.get("updated_at"),
            },
        }))
    }

    async fn write_session_memory(&self, soul_session_id: &str, text: &str) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"
            UPDATE soul_sessions
            SET session_memory = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                soul_id,
                session_id,
                session_memory,
                provider_state,
                next_seq,
                last_seen_session_seq,
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
            VALUES ($1, $2, $3, $4, $5, 0, 'running')
            RETURNING
                id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
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
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("turn start failed: {err}") })?;

        map_turn_row(&row)
    }

    async fn append_message_ref(&self, input: AppendMessageRef) -> Result<AssemblyItem> {
        let row = sqlx::query(
            r#"
            WITH advanced AS (
                UPDATE soul_sessions
                SET next_seq = next_seq + 1,
                    updated_at = NOW()
                WHERE id = $1
                RETURNING next_seq - 1 AS allocated_seq
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

    async fn append_tool_call(&self, _input: AppendToolCall) -> Result<AssemblyItem> {
        let input = _input;
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("transaction begin failed: {err}"),
        })?;

        sqlx::query(
            r#"
            INSERT INTO tool_calls (id, turn_id, tool_name, arguments)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(&input.tool_call_id)
        .bind(&input.turn_id)
        .bind(&input.tool_name)
        .bind(sqlx::types::Json(&input.arguments))
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal { message: format!("tool call insert failed: {err}") })?;

        let entry = append_runtime_entry_tx(&mut tx, &input.turn_id, "tool_call", &input.tool_call_id).await?;

        let row = sqlx::query(
            r#"
            SELECT id, turn_id, tool_name, arguments,
                   to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
            FROM tool_calls
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(&input.tool_call_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal { message: format!("tool call reload failed: {err}") })?;

        tx.commit().await.map_err(|err| Error::Internal { message: format!("transaction commit failed: {err}") })?;

        Ok(AssemblyItem {
            entry,
            target: AssemblyTarget::ToolCall(ToolCall {
                id: row.get("id"),
                turn_id: row.get("turn_id"),
                tool_name: row.get("tool_name"),
                arguments: row.get::<sqlx::types::Json<serde_json::Value>, _>("arguments").0,
                created_at: row.get("created_at"),
            }),
        })
    }

    async fn append_tool_result(&self, _input: AppendToolResult) -> Result<AssemblyItem> {
        let input = _input;
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("transaction begin failed: {err}"),
        })?;

        sqlx::query(
            r#"
            INSERT INTO tool_results (id, tool_call_id, output, error_text)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(&input.tool_result_id)
        .bind(&input.tool_call_id)
        .bind(input.output.as_ref().map(sqlx::types::Json))
        .bind(&input.error_text)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal { message: format!("tool result insert failed: {err}") })?;

        let turn_row = sqlx::query(
            r#"
            SELECT tc.turn_id
            FROM tool_calls tc
            WHERE tc.id = $1
            LIMIT 1
            "#,
        )
        .bind(&input.tool_call_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal { message: format!("tool result turn lookup failed: {err}") })?;
        let turn_id: String = turn_row.get("turn_id");

        let entry = append_runtime_entry_tx(&mut tx, &turn_id, "tool_result", &input.tool_result_id).await?;

        let row = sqlx::query(
            r#"
            SELECT id, tool_call_id, output, error_text,
                   to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
            FROM tool_results
            WHERE id = $1
            LIMIT 1
            "#,
        )
        .bind(&input.tool_result_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal { message: format!("tool result reload failed: {err}") })?;

        tx.commit().await.map_err(|err| Error::Internal { message: format!("transaction commit failed: {err}") })?;

        Ok(AssemblyItem {
            entry,
            target: AssemblyTarget::ToolResult(ToolResult {
                id: row.get("id"),
                tool_call_id: row.get("tool_call_id"),
                output: row.try_get::<sqlx::types::Json<serde_json::Value>, _>("output").ok().map(|json| json.0),
                error_text: row.get("error_text"),
                created_at: row.get("created_at"),
            }),
        })
    }

    async fn append_compact(&self, _input: AppendCompact) -> Result<AssemblyItem> {
        Err(Error::Internal { message: "append_compact not implemented in phase 1".to_string() })
    }

    async fn complete_turn(&self, input: CompleteTurn) -> Result<Turn> {
        let end_seq = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT next_seq - 1
            FROM soul_sessions
            WHERE id = (SELECT soul_session_id FROM turns WHERE id = $1)
            LIMIT 1
            "#,
        )
        .bind(&input.turn_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("turn end seq lookup failed: {err}") })?;

        let row = sqlx::query(
            r#"
            UPDATE turns
            SET status = 'completed',
                end_soul_session_seq = $2,
                updated_at = NOW(),
                finished_at = NOW()
            WHERE id = $1
            RETURNING
                id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
                base_soul_session_seq, end_soul_session_seq, status, error_text,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at,
                to_char(finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS finished_at
            "#,
        )
        .bind(&input.turn_id)
        .bind(end_seq)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("turn complete failed: {err}") })?;

        if let Some(provider_state) = input.provider_state {
            sqlx::query(
                r#"
                UPDATE soul_sessions
                SET last_seen_session_seq = $2,
                    provider_state = $3,
                    updated_at = NOW()
                WHERE id = (SELECT soul_session_id FROM turns WHERE id = $1)
                "#,
            )
            .bind(&input.turn_id)
            .bind(input.last_seen_session_seq)
            .bind(serde_json::json!({
                "provider": provider_state.provider,
                "basis_soul_session_seq": provider_state.basis_soul_session_seq,
                "opaque": provider_state.opaque,
                "schema_version": provider_state.schema_version,
            }))
            .execute(&self.pool)
            .await
            .map_err(|err| Error::Internal { message: format!("soul_session complete failed: {err}") })?;
        }

        map_turn_row(&row)
    }

    async fn fail_turn(&self, input: FailTurn) -> Result<Turn> {
        let end_seq = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT next_seq - 1
            FROM soul_sessions
            WHERE id = (SELECT soul_session_id FROM turns WHERE id = $1)
            LIMIT 1
            "#,
        )
        .bind(&input.turn_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("turn end seq lookup failed: {err}") })?;

        let row = sqlx::query(
            r#"
            UPDATE turns
            SET status = 'failed',
                end_soul_session_seq = $2,
                error_text = $3,
                updated_at = NOW(),
                finished_at = NOW()
            WHERE id = $1
            RETURNING
                id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
                base_soul_session_seq, end_soul_session_seq, status, error_text,
                to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at,
                to_char(finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS finished_at
            "#,
        )
        .bind(&input.turn_id)
        .bind(end_seq)
        .bind(&input.error_text)
        .fetch_one(&self.pool)
        .await
        .map_err(|err| Error::Internal { message: format!("turn fail failed: {err}") })?;

        map_turn_row(&row)
    }

    async fn list_assembly_items(&self, _soul_session_id: &str, _after_soul_session_seq: Option<i64>) -> Result<Vec<AssemblyItem>> {
        Ok(Vec::new())
    }
}

async fn append_runtime_entry_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    turn_id: &str,
    target_type: &str,
    target_id: &str,
) -> Result<SoulSessionEntry> {
    let soul_session_row = sqlx::query(
        r#"
        SELECT soul_session_id
        FROM turns
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(turn_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| Error::Internal { message: format!("turn soul_session lookup failed: {err}") })?;
    let soul_session_id: String = soul_session_row.get("soul_session_id");

    let row = sqlx::query(
        r#"
        WITH advanced AS (
            UPDATE soul_sessions
            SET next_seq = next_seq + 1,
                updated_at = NOW()
            WHERE id = $1
            RETURNING next_seq - 1 AS allocated_seq
        )
        INSERT INTO r_soul_session_messages (soul_session_id, target_type, target_id, soul_session_seq)
        SELECT $1, $2, $3, allocated_seq FROM advanced
        RETURNING soul_session_id, target_type, target_id, soul_session_seq,
                  to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
        "#,
    )
    .bind(&soul_session_id)
    .bind(target_type)
    .bind(target_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|err| Error::Internal { message: format!("runtime entry insert failed: {err}") })?;

    Ok(SoulSessionEntry {
        soul_session_id: row.get("soul_session_id"),
        target_type: match row.get::<String, _>("target_type").as_str() {
            "message" => SoulSessionTargetType::Message,
            "tool_call" => SoulSessionTargetType::ToolCall,
            "tool_result" => SoulSessionTargetType::ToolResult,
            _ => SoulSessionTargetType::Compact,
        },
        target_id: row.get("target_id"),
        soul_session_seq: row.get("soul_session_seq"),
        created_at: row.get("created_at"),
    })
}

fn map_soul_session_row(row: &PgRow) -> Result<SoulSession> {
    Ok(SoulSession {
        id: row.get("id"),
        soul_id: row.get("soul_id"),
        session_id: row.get("session_id"),
        session_memory: row.get("session_memory"),
        provider_state: row
            .try_get::<serde_json::Value, _>("provider_state")
            .ok()
            .and_then(parse_provider_state),
        next_seq: row.get("next_seq"),
        last_seen_session_seq: row.get("last_seen_session_seq"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn parse_provider_state(value: serde_json::Value) -> Option<ProviderState> {
    Some(ProviderState {
        provider: value.get("provider")?.as_str()?.to_string(),
        basis_soul_session_seq: value.get("basis_soul_session_seq")?.as_i64()?,
        opaque: value.get("opaque")?.clone(),
        schema_version: value
            .get("schema_version")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
    })
}

fn map_turn_row(row: &PgRow) -> Result<Turn> {
    Ok(Turn {
        id: row.get("id"),
        soul_session_id: row.get("soul_session_id"),
        trigger_type: match row.get::<String, _>("trigger_type").as_str() {
            "session_send" => TurnTriggerType::SessionSend,
            _ => TurnTriggerType::System,
        },
        trigger_ref: row.get("trigger_ref"),
        input_through_session_seq: row.get("input_through_session_seq"),
        base_soul_session_seq: row.get("base_soul_session_seq"),
        end_soul_session_seq: row.get("end_soul_session_seq"),
        status: match row.get::<String, _>("status").as_str() {
            "running" => TurnStatus::Running,
            "completed" => TurnStatus::Completed,
            _ => TurnStatus::Failed,
        },
        error_text: row.get("error_text"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        finished_at: row.try_get("finished_at").ok(),
    })
}
