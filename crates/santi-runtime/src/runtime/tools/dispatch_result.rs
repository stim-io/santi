use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use santi_core::port::provider::{FunctionCallOutput, ProviderFunctionCall};

use super::{BashToolResultEnvelope, ToolCallFeedbackMsg, ToolDispatchResult};

const BASH_MODEL_OUTPUT_PREVIEW_CHARS: usize = 2_000;

pub(super) fn parse_tool_args<T: DeserializeOwned>(
    call: &ProviderFunctionCall,
    tool_name: &str,
) -> Result<T, ToolDispatchResult> {
    serde_json::from_value(call.arguments.clone()).map_err(|err| {
        build_failed_dispatch_result(
            &call.name,
            &call.call_id,
            format!("invalid {tool_name} arguments: {err}"),
        )
    })
}

pub(super) fn build_bash_dispatch_result(
    tool_name: &str,
    call_id: &str,
    result: &BashToolResultEnvelope,
) -> Result<ToolDispatchResult, String> {
    let tool_output = serde_json::to_value(result)
        .map_err(|err| format!("serialize bash tool output failed: {err}"))?;
    let model_output = bash_model_tool_output(result)?;

    Ok(ToolDispatchResult {
        tool_name: tool_name.to_string(),
        ok: matches!(result.feedback_msg, ToolCallFeedbackMsg::NormalToolCall),
        function_call_output: FunctionCallOutput {
            call_id: call_id.to_string(),
            output: model_output.to_string(),
        },
        tool_output,
    })
}

pub fn bash_model_tool_output(result: &BashToolResultEnvelope) -> Result<Value, String> {
    let bash = &result.bash_result;
    let needs_model_projection =
        !matches!(result.feedback_msg, ToolCallFeedbackMsg::NormalToolCall)
            || bash.stdout_truncated
            || bash.stderr_truncated;

    if !needs_model_projection {
        return serde_json::to_value(result)
            .map_err(|err| format!("serialize bash model output failed: {err}"));
    }

    Ok(serde_json::json!({
        "feedback_msg": &result.feedback_msg,
        "duration_ms": result.duration_ms,
        "model_projection": {
            "kind": "bash_output_preview",
            "note": "Large or incomplete bash output is summarized for the model. Use artifact paths with bash if exact content is needed."
        },
        "bash_result": {
            "exit_code": bash.exit_code,
            "stdout_preview": preview_for_model(&bash.stdout),
            "stderr_preview": preview_for_model(&bash.stderr),
            "stdout_chars": bash.stdout_chars,
            "stderr_chars": bash.stderr_chars,
            "stdout_truncated": bash.stdout_truncated,
            "stderr_truncated": bash.stderr_truncated,
            "stdout_artifact_path": bash.stdout_artifact_path.clone(),
            "stderr_artifact_path": bash.stderr_artifact_path.clone(),
        }
    }))
}

pub(super) fn build_success_dispatch_result<T: Serialize>(
    tool_name: &str,
    call_id: &str,
    result: &T,
) -> Result<ToolDispatchResult, String> {
    let tool_output = serde_json::to_value(result)
        .map_err(|err| format!("serialize tool output failed: {err}"))?;

    Ok(ToolDispatchResult {
        tool_name: tool_name.to_string(),
        ok: true,
        function_call_output: FunctionCallOutput {
            call_id: call_id.to_string(),
            output: tool_output.to_string(),
        },
        tool_output,
    })
}

pub(super) fn build_failed_dispatch_result(
    tool_name: &str,
    call_id: &str,
    message: String,
) -> ToolDispatchResult {
    let tool_output = serde_json::json!({
        "ok": false,
        "error": {
            "type": "tool_error",
            "message": message,
        }
    });

    ToolDispatchResult {
        tool_name: tool_name.to_string(),
        ok: false,
        function_call_output: FunctionCallOutput {
            call_id: call_id.to_string(),
            output: tool_output.to_string(),
        },
        tool_output,
    }
}

fn preview_for_model(text: &str) -> String {
    if text.chars().count() <= BASH_MODEL_OUTPUT_PREVIEW_CHARS {
        return text.to_string();
    }

    let preview = text
        .chars()
        .take(BASH_MODEL_OUTPUT_PREVIEW_CHARS)
        .collect::<String>();
    format!("{preview}\n[model-facing preview truncated]")
}
