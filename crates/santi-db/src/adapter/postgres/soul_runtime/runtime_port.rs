use sqlx::Row;
use uuid::Uuid;

use santi_core::{
    error::{Error, Result},
    model::{
        message::{ActorType, Message, MessageContent, MessageState},
        runtime::{
            AssemblyItem, AssemblyTarget, ProviderState, SoulSession, SoulSessionEntry,
            SoulSessionTargetType, ToolCall, ToolResult, Turn, TurnTriggerType,
        },
        session::{SessionMessage, SessionMessageRef},
    },
    port::{
        soul_runtime::{
            AcquireSoulSession, AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn,
            FailTurn, SoulRuntimePort, StartTurn,
        },
        soul_session_query::SoulSessionQueryPort,
    },
};

use super::{
    helpers::{encode_provider_state, map_soul_session_row, map_turn_row},
    DbSoulRuntime,
};

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
        .map_err(|err| Error::Internal {
            message: format!("soul_session get failed: {err}"),
        })?;
        row.map(|row| map_soul_session_row(&row)).transpose()
    }

    async fn write_session_memory(
        &self,
        soul_session_id: &str,
        text: &str,
    ) -> Result<Option<SoulSession>> {
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
        .map_err(|err| Error::Internal {
            message: format!("session memory update failed: {err}"),
        })?;
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
        .bind(match input.trigger_type {
            TurnTriggerType::SessionSend => "session_send",
            TurnTriggerType::System => "system",
        })
        .bind(&input.trigger_ref)
        .bind(input.input_through_session_seq)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("turn start failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "soul_session",
        })?;
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
        .map_err(|err| Error::Internal {
            message: format!("append message ref failed: {err}"),
        })?;

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

    async fn append_tool_call(&self, input: AppendToolCall) -> Result<AssemblyItem> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("append tool call tx begin failed: {err}"),
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
        .map_err(|err| Error::Internal {
            message: format!("insert tool call failed: {err}"),
        })?;

        let soul_session_id: String =
            sqlx::query_scalar(r#"SELECT soul_session_id FROM turns WHERE id = $1 LIMIT 1"#)
                .bind(&input.turn_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|err| Error::Internal {
                    message: format!("load tool call soul session failed: {err}"),
                })?
                .ok_or(Error::NotFound { resource: "turn" })?;

        let allocated_seq = Self::allocate_seq(&mut tx, &soul_session_id).await?;

        let entry_row = sqlx::query(
            r#"
            INSERT INTO r_soul_session_messages (soul_session_id, target_type, target_id, soul_session_seq)
            VALUES ($1, 'tool_call', $2, $3)
            RETURNING soul_session_id, target_id, soul_session_seq,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
            "#,
        )
        .bind(&soul_session_id)
        .bind(&input.tool_call_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert tool call assembly entry failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("append tool call tx commit failed: {err}"),
        })?;

        Ok(AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: entry_row.get("soul_session_id"),
                target_type: SoulSessionTargetType::ToolCall,
                target_id: entry_row.get("target_id"),
                soul_session_seq: entry_row.get("soul_session_seq"),
                created_at: entry_row.get("created_at"),
            },
            target: AssemblyTarget::ToolCall(ToolCall {
                id: input.tool_call_id,
                turn_id: input.turn_id,
                tool_name: input.tool_name,
                arguments: input.arguments,
                created_at: entry_row.get("created_at"),
            }),
        })
    }

    async fn append_tool_result(&self, input: AppendToolResult) -> Result<AssemblyItem> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("append tool result tx begin failed: {err}"),
        })?;

        sqlx::query(
            r#"
            INSERT INTO tool_results (id, tool_call_id, output, error_text)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(&input.tool_result_id)
        .bind(&input.tool_call_id)
        .bind(input.output.clone().map(sqlx::types::Json))
        .bind(&input.error_text)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert tool result failed: {err}"),
        })?;

        let soul_session_id: String = sqlx::query_scalar(
            r#"
            SELECT t.soul_session_id
            FROM tool_calls tc
            JOIN turns t ON t.id = tc.turn_id
            WHERE tc.id = $1
            LIMIT 1
            "#,
        )
        .bind(&input.tool_call_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("load tool result soul session failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "tool_call",
        })?;

        let allocated_seq = Self::allocate_seq(&mut tx, &soul_session_id).await?;

        let entry_row = sqlx::query(
            r#"
            INSERT INTO r_soul_session_messages (soul_session_id, target_type, target_id, soul_session_seq)
            VALUES ($1, 'tool_result', $2, $3)
            RETURNING soul_session_id, target_id, soul_session_seq,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at
            "#,
        )
        .bind(&soul_session_id)
        .bind(&input.tool_result_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("insert tool result assembly entry failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("append tool result tx commit failed: {err}"),
        })?;

        Ok(AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: entry_row.get("soul_session_id"),
                target_type: SoulSessionTargetType::ToolResult,
                target_id: entry_row.get("target_id"),
                soul_session_seq: entry_row.get("soul_session_seq"),
                created_at: entry_row.get("created_at"),
            },
            target: AssemblyTarget::ToolResult(ToolResult {
                id: input.tool_result_id,
                tool_call_id: input.tool_call_id,
                output: input.output,
                error_text: input.error_text,
                created_at: entry_row.get("created_at"),
            }),
        })
    }

    async fn complete_turn(&self, input: CompleteTurn) -> Result<Turn> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("complete turn tx begin failed: {err}"),
        })?;

        let row = sqlx::query(
            r#"
            UPDATE turns
            SET status = 'completed',
                end_soul_session_seq = (
                    SELECT next_seq - 1 FROM soul_sessions WHERE id = turns.soul_session_id
                ),
                updated_at = NOW(),
                finished_at = NOW()
            WHERE id = $1
            RETURNING id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
                      base_soul_session_seq, end_soul_session_seq, status, error_text,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                      to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at,
                      to_char(finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS finished_at
            "#,
        )
        .bind(&input.turn_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("complete turn failed: {err}"),
        })?
        .ok_or(Error::NotFound { resource: "turn" })?;

        let provider_state = input
            .provider_state
            .map(|state: ProviderState| sqlx::types::Json(encode_provider_state(&state)));

        sqlx::query(
            r#"
            UPDATE soul_sessions
            SET last_seen_session_seq = $2,
                provider_state = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(row.get::<String, _>("soul_session_id"))
        .bind(input.last_seen_session_seq)
        .bind(provider_state)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("update soul session after complete failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("complete turn tx commit failed: {err}"),
        })?;

        map_turn_row(&row)
    }

    async fn fail_turn(&self, input: FailTurn) -> Result<Turn> {
        let row = sqlx::query(
            r#"
            UPDATE turns
            SET status = 'failed',
                end_soul_session_seq = (
                    SELECT next_seq - 1 FROM soul_sessions WHERE id = turns.soul_session_id
                ),
                error_text = $2,
                updated_at = NOW(),
                finished_at = NOW()
            WHERE id = $1
            RETURNING id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
                      base_soul_session_seq, end_soul_session_seq, status, error_text,
                      to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS created_at,
                      to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS updated_at,
                      to_char(finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS finished_at
            "#,
        )
        .bind(&input.turn_id)
        .bind(&input.error_text)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("fail turn failed: {err}"),
        })?
        .ok_or(Error::NotFound { resource: "turn" })?;

        map_turn_row(&row)
    }
}

#[async_trait::async_trait]
impl SoulSessionQueryPort for DbSoulRuntime {
    async fn get_soul_session_by_session_id(
        &self,
        session_id: &str,
    ) -> Result<Option<SoulSession>> {
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
        .map_err(|err| Error::Internal {
            message: format!("get_soul_session_by_session_id failed: {err}"),
        })?;
        row.map(|row| map_soul_session_row(&row)).transpose()
    }
}
