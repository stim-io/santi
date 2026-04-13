use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use santi_core::port::provider::{FunctionCallOutput, ProviderRequest, ProviderTool};

#[derive(Debug, Serialize)]
pub(super) struct UpstreamResponsesRequest {
    pub(super) model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) instructions: Option<String>,
    pub(super) input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tools: Option<Vec<Value>>,
    pub(super) stream: bool,
    pub(super) store: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) previous_response_id: Option<String>,
    pub(super) prompt_cache_key: String,
}

pub(super) fn build_prompt_cache_key(input: &ProviderRequest) -> String {
    let phase = if input.previous_response_id.is_some() {
        "tool_followup"
    } else {
        "initial"
    };

    let instructions = input
        .instructions
        .as_deref()
        .map(normalize_cacheable_instructions)
        .unwrap_or_default();

    let tools = stable_json_string(&map_tools(input.tools.clone()).unwrap_or_default());

    let payload = serde_json::json!({
        "v": 1,
        "model": input.model,
        "phase": phase,
        "instructions": instructions,
        "tools": tools,
    });

    let mut hasher = Sha256::new();
    hasher.update(payload.to_string().as_bytes());
    let digest = hasher.finalize();
    let prompt_cache_key = format!(
        "santi:oai:v1:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );

    let tool_names = input
        .tools
        .as_ref()
        .map(|tools| {
            tools
                .iter()
                .map(|tool| match tool {
                    ProviderTool::Function(tool) => tool.name.clone(),
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();

    tracing::debug!(
        model = %input.model,
        phase,
        has_previous_response_id = input.previous_response_id.is_some(),
        instructions_present = input.instructions.is_some(),
        instructions_normalized_len = instructions.len(),
        tools_count = input.tools.as_ref().map(|tools| tools.len()).unwrap_or(0),
        tool_names,
        prompt_cache_key = %prompt_cache_key,
        "built gateway prompt cache key"
    );

    prompt_cache_key
}

fn normalize_cacheable_instructions(input: &str) -> String {
    let stripped = strip_tag_block(input, "santi-meta");
    let stripped = strip_tag_block(&stripped, "santi-runtime");
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_tag_block(input: &str, tag: &str) -> String {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");

    let mut remaining = input;
    let mut result = String::new();

    loop {
        let Some(start) = remaining.find(&start_tag) else {
            result.push_str(remaining);
            break;
        };

        result.push_str(&remaining[..start]);
        let after_start = &remaining[start + start_tag.len()..];

        let Some(end) = after_start.find(&end_tag) else {
            break;
        };

        remaining = &after_start[end + end_tag.len()..];
    }

    result
}

fn stable_json_string<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("stable json serialization failed")
}

pub(super) fn map_tools(tools: Option<Vec<ProviderTool>>) -> Option<Vec<Value>> {
    tools.map(|tools| {
        tools
            .into_iter()
            .map(|tool| match tool {
                ProviderTool::Function(tool) => serde_json::json!({
                    "type": "function",
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }),
            })
            .collect()
    })
}

pub(super) fn map_function_call_outputs(outputs: Vec<FunctionCallOutput>) -> Vec<Value> {
    outputs
        .into_iter()
        .map(|output| {
            serde_json::json!({
                "type": "function_call_output",
                "call_id": output.call_id,
                "output": output.output,
            })
        })
        .collect()
}
