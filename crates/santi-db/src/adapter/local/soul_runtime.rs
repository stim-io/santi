use std::path::Path;

use serde_json::{json, Value};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use santi_core::{
    error::{Error, Result},
    model::{
        message::{ActorType, Message, MessageContent, MessagePart, MessageState},
        runtime::{
            AssemblyItem, AssemblyTarget, Compact, ProviderState, SoulSession,
            SoulSessionEntry, SoulSessionTargetType, ToolCall, ToolResult, Turn, TurnStatus,
            TurnTriggerType,
        },
        session::{SessionMessage, SessionMessageRef},
    },
    port::{compact_ledger::CompactLedgerPort, soul_runtime::{AcquireSoulSession, AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn, FailTurn, SoulRuntimePort, StartTurn}},
};

#[derive(Clone)]
pub struct LocalSoulRuntime {
    pool: SqlitePool,
}

impl LocalSoulRuntime {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| Error::Internal {
                    message: format!("create sqlite parent dir failed: {err}"),
                })?;
        }

        let database_url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .map_err(|err| Error::Internal {
                message: format!("connect sqlite failed: {err}"),
            })?;

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS local_soul_sessions (
                id TEXT PRIMARY KEY,
                soul_id TEXT NOT NULL,
                session_id TEXT NOT NULL UNIQUE,
                session_memory TEXT NOT NULL DEFAULT '',
                provider_state TEXT NULL,
                next_seq INTEGER NOT NULL DEFAULT 1,
                last_seen_session_seq INTEGER NOT NULL DEFAULT 0,
                parent_soul_session_id TEXT NULL,
                fork_point INTEGER NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"#,
        )
        .execute(&pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("migrate sqlite local_soul_sessions failed: {err}"),
        })?;

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS local_turns (
                id TEXT PRIMARY KEY,
                soul_session_id TEXT NOT NULL,
                trigger_type TEXT NOT NULL,
                trigger_ref TEXT NULL,
                input_through_session_seq INTEGER NOT NULL,
                base_soul_session_seq INTEGER NOT NULL,
                end_soul_session_seq INTEGER NULL,
                status TEXT NOT NULL,
                error_text TEXT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                finished_at TEXT NULL
            )"#,
        )
        .execute(&pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("migrate sqlite local_turns failed: {err}"),
        })?;

        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS local_soul_session_items (
                soul_session_id TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_id TEXT NOT NULL,
                soul_session_seq INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (soul_session_id, soul_session_seq)
            )"#,
        )
        .execute(&pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("migrate sqlite local_soul_session_items failed: {err}"),
        })?;

        Ok(Self { pool })
    }

    async fn ensure_soul_session(&self, soul_id: &str, session_id: &str) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO local_soul_sessions (id, soul_id, session_id)
               VALUES (?1, ?2, ?3)
               ON CONFLICT(session_id) DO UPDATE SET updated_at = local_soul_sessions.updated_at"#,
        )
        .bind(Self::local_soul_session_id(session_id))
        .bind(soul_id)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("ensure local soul_session failed: {err}"),
        })?;

        Ok(())
    }

    async fn fetch_soul_session_by_id(&self, soul_session_id: &str) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"SELECT id, soul_id, session_id, session_memory, provider_state, next_seq,
                      last_seen_session_seq, parent_soul_session_id, fork_point,
                      created_at, updated_at
               FROM local_soul_sessions
               WHERE id = ?1
               LIMIT 1"#,
        )
        .bind(soul_session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local soul_session get failed: {err}"),
        })?;

        row.map(map_soul_session_row).transpose()
    }

    async fn fetch_soul_session_by_session_id(
        &self,
        session_id: &str,
    ) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"SELECT id, soul_id, session_id, session_memory, provider_state, next_seq,
                      last_seen_session_seq, parent_soul_session_id, fork_point,
                      created_at, updated_at
               FROM local_soul_sessions
               WHERE session_id = ?1
               LIMIT 1"#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local soul_session by session failed: {err}"),
        })?;

        row.map(map_soul_session_row).transpose()
    }

    fn local_soul_session_id(session_id: &str) -> String {
        format!("ss_local_{session_id}")
    }

    fn unsupported(feature: &str) -> Error {
        Error::InvalidInput {
            message: format!("{feature} not implemented in local mode"),
        }
    }
}

#[async_trait::async_trait]
impl SoulRuntimePort for LocalSoulRuntime {
    async fn acquire_soul_session(&self, input: AcquireSoulSession) -> Result<SoulSession> {
        self.ensure_soul_session(&input.soul_id, &input.session_id).await?;
        self.fetch_soul_session_by_session_id(&input.session_id)
            .await?
            .ok_or(Error::NotFound {
                resource: "local_soul_session",
            })
    }

    async fn get_soul_session(&self, soul_session_id: &str) -> Result<Option<SoulSession>> {
        self.fetch_soul_session_by_id(soul_session_id).await
    }

    async fn write_session_memory(&self, soul_session_id: &str, text: &str) -> Result<Option<SoulSession>> {
        let row = sqlx::query(
            r#"UPDATE local_soul_sessions
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
            message: format!("local session memory update failed: {err}"),
        })?;

        row.map(map_soul_session_row).transpose()
    }

    async fn start_turn(&self, input: StartTurn) -> Result<Turn> {
        let row = sqlx::query(
            r#"INSERT INTO local_turns (
                   id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq, base_soul_session_seq, status
               )
               SELECT ?1, ?2, ?3, ?4, ?5, next_seq - 1, 'running'
               FROM local_soul_sessions
               WHERE id = ?2
               RETURNING id, soul_session_id, trigger_type, trigger_ref, input_through_session_seq,
                         base_soul_session_seq, end_soul_session_seq, status, error_text,
                         created_at, updated_at, finished_at"#,
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
            message: format!("local turn start failed: {err}"),
        })?;

        let row = row.ok_or(Error::NotFound {
            resource: "local_soul_session",
        })?;

        map_turn_row(row)
    }

    async fn append_message_ref(&self, input: AppendMessageRef) -> Result<AssemblyItem> {
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("local append message ref tx begin failed: {err}"),
        })?;

        let seq_row = sqlx::query(
            r#"UPDATE local_soul_sessions
               SET next_seq = next_seq + 1,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING next_seq - 1 AS allocated_seq"#,
        )
        .bind(&input.soul_session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local append message ref seq advance failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "local_soul_session",
        })?;

        let allocated_seq: i64 = seq_row.get("allocated_seq");

        let entry_row = sqlx::query(
            r#"INSERT INTO local_soul_session_items (soul_session_id, target_type, target_id, soul_session_seq)
               VALUES (?1, 'message', ?2, ?3)
               RETURNING soul_session_id, target_id, soul_session_seq, created_at"#,
        )
        .bind(&input.soul_session_id)
        .bind(&input.message_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local append message ref insert failed: {err}"),
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
            message: format!("local append message ref message lookup failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "session_message",
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("local append message ref tx commit failed: {err}"),
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

    async fn append_tool_call(&self, _input: AppendToolCall) -> Result<AssemblyItem> {
        let input = _input;
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("local append tool call tx begin failed: {err}"),
        })?;

        let soul_session_id: String = sqlx::query_scalar(
            r#"SELECT soul_session_id FROM local_turns WHERE id = ?1 LIMIT 1"#,
        )
        .bind(&input.turn_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local load tool call soul session failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "local_turn",
        })?;

        let seq_row = sqlx::query(
            r#"UPDATE local_soul_sessions
               SET next_seq = next_seq + 1,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING next_seq - 1 AS allocated_seq"#,
        )
        .bind(&soul_session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local append tool call seq advance failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "local_soul_session",
        })?;

        let allocated_seq: i64 = seq_row.get("allocated_seq");

        let entry_row = sqlx::query(
            r#"INSERT INTO local_soul_session_items (soul_session_id, target_type, target_id, soul_session_seq)
               VALUES (?1, 'tool_call', ?2, ?3)
               RETURNING soul_session_id, target_id, soul_session_seq, created_at"#,
        )
        .bind(&soul_session_id)
        .bind(&input.tool_call_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local append tool call insert failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("local append tool call tx commit failed: {err}"),
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

    async fn append_tool_result(&self, _input: AppendToolResult) -> Result<AssemblyItem> {
        let input = _input;
        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("local append tool result tx begin failed: {err}"),
        })?;

        let soul_session_id: String = sqlx::query_scalar(
            r#"SELECT soul_session_id
               FROM local_soul_session_items
               WHERE target_type = 'tool_call' AND target_id = ?1
               LIMIT 1"#,
        )
        .bind(&input.tool_call_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local load tool result soul session failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "tool_call",
        })?;

        let seq_row = sqlx::query(
            r#"UPDATE local_soul_sessions
               SET next_seq = next_seq + 1,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = ?1
               RETURNING next_seq - 1 AS allocated_seq"#,
        )
        .bind(&soul_session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local append tool result seq advance failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "local_soul_session",
        })?;

        let allocated_seq: i64 = seq_row.get("allocated_seq");

        let entry_row = sqlx::query(
            r#"INSERT INTO local_soul_session_items (soul_session_id, target_type, target_id, soul_session_seq)
               VALUES (?1, 'tool_result', ?2, ?3)
               RETURNING soul_session_id, target_id, soul_session_seq, created_at"#,
        )
        .bind(&soul_session_id)
        .bind(&input.tool_result_id)
        .bind(allocated_seq)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local append tool result insert failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("local append tool result tx commit failed: {err}"),
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
        let provider_state = input
            .provider_state
            .as_ref()
            .map(encode_provider_state)
            .map(|value| serde_json::to_string(&value))
            .transpose()
            .map_err(|err| Error::Internal {
                message: format!("encode local provider_state failed: {err}"),
            })?;

        let mut tx = self.pool.begin().await.map_err(|err| Error::Internal {
            message: format!("local complete turn tx begin failed: {err}"),
        })?;

        let row = sqlx::query(
            r#"UPDATE local_turns
               SET status = 'completed',
                   end_soul_session_seq = (
                       SELECT next_seq - 1
                       FROM local_soul_sessions
                       WHERE id = (SELECT soul_session_id FROM local_turns WHERE id = ?1)
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
            message: format!("local turn complete failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "local_turn",
        })?;

        sqlx::query(
            r#"UPDATE local_soul_sessions
               SET last_seen_session_seq = ?2,
                   provider_state = ?3,
                   updated_at = CURRENT_TIMESTAMP
               WHERE id = (SELECT soul_session_id FROM local_turns WHERE id = ?1)"#,
        )
        .bind(&input.turn_id)
        .bind(input.last_seen_session_seq)
        .bind(provider_state)
        .execute(&mut *tx)
        .await
        .map_err(|err| Error::Internal {
            message: format!("local soul_session complete failed: {err}"),
        })?;

        tx.commit().await.map_err(|err| Error::Internal {
            message: format!("local complete turn tx commit failed: {err}"),
        })?;

        map_turn_row(row)
    }

    async fn fail_turn(&self, input: FailTurn) -> Result<Turn> {
        let row = sqlx::query(
            r#"UPDATE local_turns
               SET status = 'failed',
                   end_soul_session_seq = (
                       SELECT next_seq - 1
                       FROM local_soul_sessions
                       WHERE id = (SELECT soul_session_id FROM local_turns WHERE id = ?1)
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
            message: format!("local turn fail failed: {err}"),
        })?
        .ok_or(Error::NotFound {
            resource: "local_turn",
        })?;

        map_turn_row(row)
    }

    async fn get_soul_session_by_session_id(&self, session_id: &str) -> Result<Option<SoulSession>> {
        self.fetch_soul_session_by_session_id(session_id).await
    }

}

#[async_trait::async_trait]
impl CompactLedgerPort for LocalSoulRuntime {
    async fn list_compacts(&self, _soul_session_id: &str) -> Result<Vec<Compact>> {
        Err(Self::unsupported("list_compacts"))
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn local_tool_call_and_result_append_allocate_entries() {
        let dir = tempdir().expect("tempdir");
        let runtime = LocalSoulRuntime::new(dir.path().join("local.sqlite"))
            .await
            .expect("runtime");

        let soul_session = runtime
            .acquire_soul_session(AcquireSoulSession {
                soul_id: "soul_default".to_string(),
                session_id: "sess_1".to_string(),
            })
            .await
            .expect("soul session");

        let turn = runtime
            .start_turn(StartTurn {
                turn_id: "turn_1".to_string(),
                soul_session_id: soul_session.id.clone(),
                trigger_type: TurnTriggerType::System,
                trigger_ref: None,
                input_through_session_seq: 0,
            })
            .await
            .expect("turn");

        let tool_call = runtime
            .append_tool_call(AppendToolCall {
                tool_call_id: "call_1".to_string(),
                turn_id: turn.id.clone(),
                tool_name: "bash".to_string(),
                arguments: serde_json::json!({"command": "pwd"}),
            })
            .await
            .expect("tool call");

        assert_eq!(tool_call.entry.soul_session_id, soul_session.id);
        assert_eq!(tool_call.entry.target_type, SoulSessionTargetType::ToolCall);
        assert_eq!(tool_call.entry.soul_session_seq, 1);
        match tool_call.target {
            AssemblyTarget::ToolCall(call) => {
                assert_eq!(call.id, "call_1");
                assert_eq!(call.turn_id, "turn_1");
                assert_eq!(call.tool_name, "bash");
            }
            _ => panic!("expected tool call target"),
        }

        let tool_result = runtime
            .append_tool_result(AppendToolResult {
                tool_result_id: "result_1".to_string(),
                tool_call_id: "call_1".to_string(),
                output: Some(serde_json::json!({"ok": true})),
                error_text: None,
            })
            .await
            .expect("tool result");

        assert_eq!(tool_result.entry.target_type, SoulSessionTargetType::ToolResult);
        assert_eq!(tool_result.entry.soul_session_seq, 2);
        match tool_result.target {
            AssemblyTarget::ToolResult(result) => {
                assert_eq!(result.id, "result_1");
                assert_eq!(result.tool_call_id, "call_1");
                assert_eq!(result.output, Some(serde_json::json!({"ok": true})));
                assert_eq!(result.error_text, None);
            }
            _ => panic!("expected tool result target"),
        }
    }
}

fn map_soul_session_row(row: sqlx::sqlite::SqliteRow) -> Result<SoulSession> {
    let provider_state = row
        .try_get::<Option<String>, _>("provider_state")
        .map_err(|err| Error::Internal {
            message: format!("local provider_state decode failed: {err}"),
        })?
        .map(|raw| serde_json::from_str::<Value>(&raw))
        .transpose()
        .map_err(|err| Error::Internal {
            message: format!("parse local provider_state failed: {err}"),
        })?
        .map(decode_provider_state)
        .transpose()?;

    Ok(SoulSession {
        id: row.get("id"),
        soul_id: row.get("soul_id"),
        session_id: row.get("session_id"),
        session_memory: row.get("session_memory"),
        provider_state,
        next_seq: row.get("next_seq"),
        last_seen_session_seq: row.get("last_seen_session_seq"),
        parent_soul_session_id: row.try_get("parent_soul_session_id").ok(),
        fork_point: row.try_get("fork_point").ok(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_turn_row(row: sqlx::sqlite::SqliteRow) -> Result<Turn> {
    Ok(Turn {
        id: row.get("id"),
        soul_session_id: row.get("soul_session_id"),
        trigger_type: match row.get::<String, _>("trigger_type").as_str() {
            "session_send" => TurnTriggerType::SessionSend,
            _ => TurnTriggerType::System,
        },
        trigger_ref: row.try_get("trigger_ref").ok(),
        input_through_session_seq: row.get("input_through_session_seq"),
        base_soul_session_seq: row.get("base_soul_session_seq"),
        end_soul_session_seq: row.try_get("end_soul_session_seq").ok(),
        status: match row.get::<String, _>("status").as_str() {
            "running" => TurnStatus::Running,
            "completed" => TurnStatus::Completed,
            _ => TurnStatus::Failed,
        },
        error_text: row.try_get("error_text").ok(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        finished_at: row.try_get("finished_at").ok(),
    })
}

fn map_session_message_row(row: sqlx::sqlite::SqliteRow) -> Result<SessionMessage> {
    Ok(SessionMessage {
        relation: SessionMessageRef {
            message_id: row.get("message_id"),
            session_id: row.get("session_id"),
            session_seq: row.get("session_seq"),
            created_at: row.get("message_created_at"),
        },
        message: Message {
            id: row.get("message_id"),
            actor_type: match row.get::<String, _>("actor_type").as_str() {
                "soul" => ActorType::Soul,
                "system" => ActorType::System,
                _ => ActorType::Account,
            },
            actor_id: row.get("actor_id"),
            content: MessageContent {
                parts: vec![MessagePart::Text {
                    text: row.get("content_text"),
                }],
            },
            state: match row.get::<String, _>("state").as_str() {
                "fixed" => MessageState::Fixed,
                _ => MessageState::Pending,
            },
            created_at: row.get("message_created_at"),
            updated_at: row.get("message_created_at"),
            deleted_at: None,
            version: 1,
        },
    })
}

fn encode_provider_state(state: &ProviderState) -> Value {
    json!({
        "provider": state.provider,
        "basis_soul_session_seq": state.basis_soul_session_seq,
        "opaque": state.opaque,
        "schema_version": state.schema_version,
    })
}

fn decode_provider_state(value: Value) -> Result<ProviderState> {
    let obj = value.as_object().ok_or(Error::Internal {
        message: "provider_state must be an object".to_string(),
    })?;

    let provider = obj
        .get("provider")
        .and_then(Value::as_str)
        .ok_or(Error::Internal {
            message: "provider_state.provider missing".to_string(),
        })?
        .to_string();
    let basis_soul_session_seq = obj
        .get("basis_soul_session_seq")
        .and_then(Value::as_i64)
        .ok_or(Error::Internal {
            message: "provider_state.basis_soul_session_seq missing".to_string(),
        })?;
    let opaque = obj.get("opaque").cloned().ok_or(Error::Internal {
        message: "provider_state.opaque missing".to_string(),
    })?;
    let schema_version = obj
        .get("schema_version")
        .and_then(|value| value.as_str().map(ToString::to_string));

    Ok(ProviderState {
        provider,
        basis_soul_session_seq,
        opaque,
        schema_version,
    })
}
