use santi_core::{
    error::{Error, Result},
    model::runtime::{SoulSession, ToolActivity, ToolCall, ToolResult},
    port::soul_session_query::SoulSessionQueryPort,
};
use serde_json::Value;
use sqlx::Row;

use super::StandaloneSoulRuntime;

#[async_trait::async_trait]
impl SoulSessionQueryPort for StandaloneSoulRuntime {
    async fn get_session_soul(&self, session_id: &str) -> Result<Option<SoulSession>> {
        self.fetch_session_soul(session_id).await
    }

    async fn list_tool_activities(&self, soul_session_id: &str) -> Result<Vec<ToolActivity>> {
        let rows = sqlx::query(
            r#"SELECT
                   call_item.soul_session_seq AS tool_call_seq,
                   call.id AS tool_call_id,
                   call.turn_id AS tool_call_turn_id,
                   call.tool_name AS tool_call_name,
                   call.arguments AS tool_call_arguments,
                   call.created_at AS tool_call_created_at,
                   result.id AS tool_result_id,
                   result.output AS tool_result_output,
                   result.error_text AS tool_result_error_text,
                   result.created_at AS tool_result_created_at,
                   result_item.soul_session_seq AS tool_result_seq
               FROM standalone_soul_session_items call_item
               JOIN standalone_tool_calls call
                 ON call.id = call_item.target_id
               LEFT JOIN standalone_tool_results result
                 ON result.tool_call_id = call.id
               LEFT JOIN standalone_soul_session_items result_item
                 ON result_item.soul_session_id = call_item.soul_session_id
                AND result_item.target_type = 'tool_result'
                AND result_item.target_id = result.id
               WHERE call_item.soul_session_id = ?1
                 AND call_item.target_type = 'tool_call'
               ORDER BY call_item.soul_session_seq"#,
        )
        .bind(soul_session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("standalone list tool activities failed: {err}"),
        })?;

        rows.into_iter().map(map_tool_activity_row).collect()
    }
}

fn map_tool_activity_row(row: sqlx::sqlite::SqliteRow) -> Result<ToolActivity> {
    let arguments = parse_json_text(row.get("tool_call_arguments"), "tool call arguments")?;
    let tool_result_id = row
        .try_get::<Option<String>, _>("tool_result_id")
        .ok()
        .flatten();
    let tool_result = tool_result_id
        .map(|id| {
            let output = row
                .try_get::<Option<String>, _>("tool_result_output")
                .ok()
                .flatten()
                .map(|raw| parse_json_text(raw, "tool result output"))
                .transpose()?;

            Ok(ToolResult {
                id,
                tool_call_id: row.get("tool_call_id"),
                output,
                error_text: row
                    .try_get::<Option<String>, _>("tool_result_error_text")
                    .ok()
                    .flatten(),
                created_at: row.get("tool_result_created_at"),
            })
        })
        .transpose()?;

    Ok(ToolActivity {
        tool_call: ToolCall {
            id: row.get("tool_call_id"),
            turn_id: row.get("tool_call_turn_id"),
            tool_name: row.get("tool_call_name"),
            arguments,
            created_at: row.get("tool_call_created_at"),
        },
        tool_call_seq: row.get("tool_call_seq"),
        tool_result,
        tool_result_seq: row
            .try_get::<Option<i64>, _>("tool_result_seq")
            .ok()
            .flatten(),
    })
}

fn parse_json_text(raw: String, label: &str) -> Result<Value> {
    serde_json::from_str(&raw).map_err(|err| Error::Internal {
        message: format!("standalone parse {label} failed: {err}"),
    })
}
