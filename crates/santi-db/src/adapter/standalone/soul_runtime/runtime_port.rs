use santi_core::{
    error::{Error, Result},
    model::runtime::{AssemblyItem, AssemblyTarget, SoulSessionEntry, SoulSessionTargetType, Turn},
    port::{
        soul_runtime::{
            AcquireSoulSession, AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn,
            FailTurn, SoulRuntimePort, StartTurn,
        },
        soul_session_query::SoulSessionQueryPort,
    },
};
use sqlx::Row;

use super::{
    helpers::{
        encode_provider_state, map_session_message_row, map_turn_row, tool_call_target,
        tool_result_target,
    },
    StandaloneSoulRuntime,
};

#[async_trait::async_trait]
impl SoulRuntimePort for StandaloneSoulRuntime {
    async fn acquire_soul_session(&self, input: AcquireSoulSession) -> Result<santi_core::model::runtime::SoulSession> {
        self.ensure_acquired_soul_session(&input).await
    }

    async fn get_soul_session(
        &self,
        soul_session_id: &str,
    ) -> Result<Option<santi_core::model::runtime::SoulSession>> {
        self.fetch_soul_session_by_id(soul_session_id).await
    }

    async fn write_session_memory(
        &self,
        soul_session_id: &str,
        text: &str,
    ) -> Result<Option<santi_core::model::runtime::SoulSession>> {
        let row = sqlx::query(
            r#"UPDATE standalone_soul_sessions
               SET session_memory = ?2,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING id, soul_id, session_id, session_memory, provider_state, next_seq,
                         last_seen_session_seq, parent_soul_session_id, fork_point,
                         created_at, updated_at"#,
        )
        .bind(soul_session_id)
        .bind(text)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone session memory update failed: {err}"),
        })?;

        row.map(super::helpers::map_soul_session_row).transpose()
    }

    async fn start_turn(&self, input: StartTurn) -> Result<Turn> {
        let row = sqlx::query(
            r#"INSERT INTO standalone_turns (
                   id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq, base_soul_session_seq, status
               )
               SELECT ?1, ?2, ?3, ?4, ?5, next_seq - 1, 'running'
               FROM standalone_soul_sessions
               WHERE id = ?2
               RETURNING id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
                         base_soul_session_seq, end_soul_session_seq, status, error_text,
                         created_at, updated_at, finished_at"#,
        )
        .bind(&input.turn_id)
        .bind(&input.soul_session_id)
        .bind(match input.trigger_type {
            santi_core::model::runtime::TurnTriggerType::SessionSend => "session_send",
            santi_core::model::runtime::TurnTriggerType::System => "system",
        })
        .bind(&input.trigger_ref)
        .bind(input.input_through_session_seq)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone turn start failed: {err}"),
        })?;

        let row = row.ok_or(Error::NotFound {
            resource: "standalone_soul_session",
        })?;

        map_turn_row(row)
    }

    async fn append_message_ref(&self, input: AppendMessageRef) -> Result<AssemblyItem> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("standalone append message ref tx begin failed: {err}"),
        })?;

        let seq_row = sqlx::query(
            r#"UPDATE standalone_soul_sessions
               SET next_seq = next_seq + 1,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING next_seq - 1 AS allocated_seq"#,
        )
        .bind(&input.soul_session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone append message ref seq advance failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_soul_session",
        })?;

        let allocated_seq: i64 = seq_row.get("allocated_seq");

        let entry_row = sqlx::query(
            r#"INSERT INTO standalone_soul_session_items (soul_session_id, target_type, target_id, soul_session_seq)
               VALUES (?1, 'message', ?2, ?3)
               RETURNING soul_session_id, target_id, soul_session_seq, created_at"#,
        )
        .bind(&input.soul_session_id)
        .bind(&input.message_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone append message ref insert failed: {err}"),
        })?;

        let message_row = sqlx::query(
            r#"SELECT id AS message_id, session_id, session_seq, actor_type, actor_id, content_text, state, created_at AS message_created_at
               FROM session_messages
               WHERE id = ?1
               LIMIT 1"#,
        )
        .bind(&input.message_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone append message ref message lookup failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "session_message",
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("standalone append message ref tx commit failed: {err}"),
        })?;

        Ok(AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: entry_row.get("soul_session_id"),
                target_type: SoulSessionTargetType::Message,
                target_id: entry_row.get("target_id"),
                soul_session_seq: entry_row.get("soul_session_seq"),
                created_at: entry_row.get("created_at"),
            },
            target: AssemblyTarget::Message(map_session_message_row(message_row)?),
        })
    }

    async fn append_tool_call(&self, input: AppendToolCall) -> Result<AssemblyItem> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("standalone append tool call tx begin failed: {err}"),
        })?;

        let soul_session_id: String = sqlx::query_scalar(
            r#"SELECT soul_session_id FROM standalone_turns WHERE id = ?1 LIMIT 1"#,
        )
        .bind(&input.turn_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone load tool call soul session failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_turn",
        })?;

        let seq_row = sqlx::query(
            r#"UPDATE standalone_soul_sessions
               SET next_seq = next_seq + 1,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING next_seq - 1 AS allocated_seq"#,
        )
        .bind(&soul_session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone append tool call seq advance failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_soul_session",
        })?;

        let allocated_seq: i64 = seq_row.get("allocated_seq");

        let entry_row = sqlx::query(
            r#"INSERT INTO standalone_soul_session_items (soul_session_id, target_type, target_id, soul_session_seq)
               VALUES (?1, 'tool_call', ?2, ?3)
               RETURNING soul_session_id, target_id, soul_session_seq, created_at"#,
        )
        .bind(&soul_session_id)
        .bind(&input.tool_call_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone append tool call insert failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("standalone append tool call tx commit failed: {err}"),
        })?;

        let created_at: String = entry_row.get("created_at");

        Ok(AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: entry_row.get("soul_session_id"),
                target_type: SoulSessionTargetType::ToolCall,
                target_id: entry_row.get("target_id"),
                soul_session_seq: entry_row.get("soul_session_seq"),
                created_at: created_at.clone(),
            },
            target: AssemblyTarget::ToolCall(tool_call_target(input, created_at)),
        })
    }

    async fn append_tool_result(&self, input: AppendToolResult) -> Result<AssemblyItem> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("standalone append tool result tx begin failed: {err}"),
        })?;

        let soul_session_id: String = sqlx::query_scalar(
            r#"SELECT soul_session_id
               FROM standalone_soul_session_items
               WHERE target_type = 'tool_call' AND target_id = ?1
               LIMIT 1"#,
        )
        .bind(&input.tool_call_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone load tool result soul session failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "tool_call",
        })?;

        let seq_row = sqlx::query(
            r#"UPDATE standalone_soul_sessions
               SET next_seq = next_seq + 1,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING next_seq - 1 AS allocated_seq"#,
        )
        .bind(&soul_session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone append tool result seq advance failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_soul_session",
        })?;

        let allocated_seq: i64 = seq_row.get("allocated_seq");

        let entry_row = sqlx::query(
            r#"INSERT INTO standalone_soul_session_items (soul_session_id, target_type, target_id, soul_session_seq)
               VALUES (?1, 'tool_result', ?2, ?3)
               RETURNING soul_session_id, target_id, soul_session_seq, created_at"#,
        )
        .bind(&soul_session_id)
        .bind(&input.tool_result_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone append tool result insert failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("standalone append tool result tx commit failed: {err}"),
        })?;

        let created_at: String = entry_row.get("created_at");

        Ok(AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: entry_row.get("soul_session_id"),
                target_type: SoulSessionTargetType::ToolResult,
                target_id: entry_row.get("target_id"),
                soul_session_seq: entry_row.get("soul_session_seq"),
                created_at: created_at.clone(),
            },
            target: AssemblyTarget::ToolResult(tool_result_target(input, created_at)),
        })
    }

    async fn complete_turn(&self, input: CompleteTurn) -> Result<Turn> {
        let provider_state = input
            .provider_state
            .as_ref()
            .map(encode_provider_state)
            .map(|value| serde_json::to_string(&value))
            .transpose()
            .map_err(|err| Error::Internal {
                message: format!("encode standalone provider_state failed: {err}"),
            })?;

        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("standalone complete turn tx begin failed: {err}"),
        })?;

        let row = sqlx::query(
            r#"UPDATE standalone_turns
               SET status = 'completed',
                   end_soul_session_seq = (
                       SELECT next_seq - 1
                       FROM standalone_soul_sessions
                       WHERE id = (SELECT soul_session_id FROM standalone_turns WHERE id = ?1)
                   ),
                   updated_at = CURRENT_TIMESTAMP,
                   finished_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
                         base_soul_session_seq, end_soul_session_seq, status, error_text,
                         created_at, updated_at, finished_at"#,
        )
        .bind(&input.turn_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone turn complete failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_turn",
        })?;

        sqlx::query(
            r#"UPDATE standalone_soul_sessions
               SET last_seen_session_seq = ?2,
                   provider_state = ?3,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = (SELECT soul_session_id FROM standalone_turns WHERE id = ?1)"#,
        )
        .bind(&input.turn_id)
        .bind(input.last_seen_session_seq)
        .bind(provider_state)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone soul_session complete failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("standalone complete turn tx commit failed: {err}"),
        })?;

        map_turn_row(row)
    }

    async fn fail_turn(&self, input: FailTurn) -> Result<Turn> {
        let row = sqlx::query(
            r#"UPDATE standalone_turns
               SET status = 'failed',
                   end_soul_session_seq = (
                       SELECT next_seq - 1
                       FROM standalone_soul_sessions
                       WHERE id = (SELECT soul_session_id FROM standalone_turns WHERE id = ?1)
                   ),
                   error_text = ?2,
                   updated_at = CURRENT_TIMESTAMP,
                   finished_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
                         base_soul_session_seq, end_soul_session_seq, status, error_text,
                         created_at, updated_at, finished_at"#,
        )
        .bind(&input.turn_id)
        .bind(&input.error_text)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone turn fail failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "standalone_turn",
        })?;

        map_turn_row(row)
    }
}

#[async_trait::async_trait]
impl SoulSessionQueryPort for StandaloneSoulRuntime {
    async fn get_soul_session_by_session_id(
        &self,
        session_id: &str,
    ) -> Result<Option<santi_core::model::runtime::SoulSession>> {
        self.fetch_soul_session_by_session_id(session_id).await
    }
}
