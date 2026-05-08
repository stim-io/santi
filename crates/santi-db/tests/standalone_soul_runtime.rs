use santi_core::{
    model::runtime::{AssemblyTarget, SoulSessionTargetType, TurnTriggerType},
    port::{
        soul_runtime::{
            AcquireSoulSession, AppendToolCall, AppendToolResult, SoulRuntimePort, StartTurn,
        },
        soul_session_query::SoulSessionQueryPort,
    },
};
use santi_db::adapter::standalone::soul_runtime::StandaloneSoulRuntime;
use tempfile::tempdir;

#[tokio::test]
async fn appends_tool_entries() {
    let dir = tempdir().expect("tempdir");
    let runtime = StandaloneSoulRuntime::new(dir.path().join("standalone.sqlite"))
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

    assert_eq!(
        tool_result.entry.target_type,
        SoulSessionTargetType::ToolResult
    );
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

    let activities = runtime
        .list_tool_activities(&soul_session.id)
        .await
        .expect("tool activities");
    assert_eq!(activities.len(), 1);
    assert_eq!(activities[0].tool_call.id, "call_1");
    assert_eq!(activities[0].tool_call.tool_name, "bash");
    assert_eq!(activities[0].tool_call_seq, 1);
    assert_eq!(
        activities[0]
            .tool_result
            .as_ref()
            .map(|result| result.id.as_str()),
        Some("result_1")
    );
    assert_eq!(activities[0].tool_result_seq, Some(2));
}
