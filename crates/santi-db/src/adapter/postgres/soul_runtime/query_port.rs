use santi_core::{
    error::{Error, Result},
    model::runtime::{SoulSession, ToolActivity, ToolCall, ToolResult},
    port::soul_session_query::SoulSessionQueryPort,
};
use sqlx::Row;

use super::{helpers::map_soul_session_row, DbSoulRuntime};

#[async_trait::async_trait]
impl SoulSessionQueryPort for DbSoulRuntime {
    async fn get_session_soul(&self, session_id: &str) -> Result<Option<SoulSession>> {
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
            message: format!("get_session_soul failed: {err}"),
        })?;
        row.map(|row| map_soul_session_row(&row)).transpose()
    }

    async fn list_tool_activities(&self, soul_session_id: &str) -> Result<Vec<ToolActivity>> {
        let rows = sqlx::query(
            r#"
            SELECT
                call_item.soul_session_seq AS tool_call_seq,
                tc.id AS tool_call_id,
                tc.turn_id AS tool_call_turn_id,
                tc.tool_name AS tool_call_name,
                tc.arguments AS tool_call_arguments,
                to_char(tc.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS tool_call_created_at,
                tr.id AS tool_result_id,
                tr.output AS tool_result_output,
                tr.error_text AS tool_result_error_text,
                to_char(tr.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SSOF') AS tool_result_created_at,
                result_item.soul_session_seq AS tool_result_seq
            FROM r_soul_session_messages call_item
            JOIN tool_calls tc
              ON tc.id = call_item.target_id
            LEFT JOIN tool_results tr
              ON tr.tool_call_id = tc.id
            LEFT JOIN r_soul_session_messages result_item
              ON result_item.soul_session_id = call_item.soul_session_id
             AND result_item.target_type = 'tool_result'
             AND result_item.target_id = tr.id
            WHERE call_item.soul_session_id = $1
              AND call_item.target_type = 'tool_call'
            ORDER BY call_item.soul_session_seq
            "#,
        )
        .bind(soul_session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| Error::Internal {
            message: format!("list tool activities failed: {err}"),
        })?;

        rows.into_iter().map(map_tool_activity_row).collect()
    }
}

fn map_tool_activity_row(row: sqlx::postgres::PgRow) -> Result<ToolActivity> {
    let arguments: sqlx::types::Json<serde_json::Value> = row.get("tool_call_arguments");
    let tool_result_id = row
        .try_get::<Option<String>, _>("tool_result_id")
        .ok()
        .flatten();
    let tool_result = tool_result_id
        .map(|id| {
            let output = row
                .try_get::<Option<sqlx::types::Json<serde_json::Value>>, _>("tool_result_output")
                .ok()
                .flatten()
                .map(|value| value.0);

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
            arguments: arguments.0,
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
