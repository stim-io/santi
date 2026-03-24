use serde_json::Value;
use uuid::Uuid;

use crate::model::message::{Message, ToolCallArtifact, ToolResultArtifact};

pub const TOOL_CALL_MESSAGE_TYPE: &str = "tool_call";
pub const TOOL_RESULT_MESSAGE_TYPE: &str = "tool_result";

pub fn build_tool_call_message(tool_call_id: String, name: String, arguments: Value) -> Message {
    let content = serde_json::to_string(&ToolCallArtifact {
        v: 1,
        tool_call_id,
        name,
        arguments,
    })
    .expect("tool call artifact serialize failed");

    Message {
        id: format!("msg_{}", Uuid::new_v4().simple()),
        r#type: TOOL_CALL_MESSAGE_TYPE.to_string(),
        role: None,
        content,
        created_at: now_seconds_string(),
    }
}

pub fn build_tool_result_message(
    tool_call_id: String,
    name: String,
    ok: bool,
    output: Value,
) -> Message {
    let content = serde_json::to_string(&ToolResultArtifact {
        v: 1,
        tool_call_id,
        name,
        ok,
        output,
    })
    .expect("tool result artifact serialize failed");

    Message {
        id: format!("msg_{}", Uuid::new_v4().simple()),
        r#type: TOOL_RESULT_MESSAGE_TYPE.to_string(),
        role: None,
        content,
        created_at: now_seconds_string(),
    }
}

pub fn parse_tool_call_artifact(message: &Message) -> Option<ToolCallArtifact> {
    if message.r#type != TOOL_CALL_MESSAGE_TYPE {
        return None;
    }
    serde_json::from_str(&message.content).ok()
}

pub fn parse_tool_result_artifact(message: &Message) -> Option<ToolResultArtifact> {
    if message.r#type != TOOL_RESULT_MESSAGE_TYPE {
        return None;
    }
    serde_json::from_str(&message.content).ok()
}

fn now_seconds_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
