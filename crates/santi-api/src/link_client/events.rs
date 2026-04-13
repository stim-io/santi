use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

use santi_core::{
    error::{Error, Result},
    port::provider::{ProviderEvent, ProviderFunctionCall},
};

use super::{ObservedGatewayEvent, OpenAiResponsesClient};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum VerboseGatewayEvent {
    ResponseCreated {
        response_id: String,
    },
    ResponseInProgress {
        response_id: String,
    },
    OutputItemAdded {
        output_index: usize,
        item_id: Option<String>,
        item_type: Option<String>,
        item: Option<Value>,
    },
    ContentPartAdded {
        output_index: usize,
        content_index: usize,
        part_type: Option<String>,
    },
    OutputTextDelta {
        output_index: Option<usize>,
        content_index: Option<usize>,
        delta: String,
    },
    OutputTextDone {
        output_index: Option<usize>,
        content_index: Option<usize>,
        text: String,
    },
    OutputItemDone {
        output_index: usize,
        item_id: Option<String>,
        item_type: Option<String>,
        item: Option<Value>,
    },
    Completed {
        response_id: Option<String>,
    },
    OpaqueUpstreamEvent(ObservedGatewayEvent),
}

pub(super) fn parse_sse_frame(
    frame: &str,
    sequence: &mut usize,
) -> Result<Vec<VerboseGatewayEvent>> {
    let mut events = Vec::new();

    for line in frame.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some(data) = parse_sse_data_line(line) else {
            continue;
        };

        let payload = parse_sse_payload(data)?;
        if let Some(event) = parse_verbose_gateway_event(payload, data, sequence) {
            events.push(event);
        }
    }

    Ok(events)
}

pub(super) fn handle_verbose_gateway_event(
    provider: &OpenAiResponsesClient,
    event: VerboseGatewayEvent,
    current_response_id: &mut Option<String>,
    completed_items: &mut BTreeMap<usize, Value>,
    streamed_text_parts: &mut HashSet<(Option<usize>, Option<usize>)>,
) -> Result<Vec<ProviderEvent>> {
    match event {
        VerboseGatewayEvent::ResponseCreated { response_id }
        | VerboseGatewayEvent::ResponseInProgress { response_id } => {
            *current_response_id = Some(response_id);
            Ok(Vec::new())
        }
        VerboseGatewayEvent::OutputTextDelta {
            output_index,
            content_index,
            delta,
        } => {
            streamed_text_parts.insert((output_index, content_index));
            Ok(vec![ProviderEvent::OutputTextDelta(delta)])
        }
        VerboseGatewayEvent::OutputTextDone {
            output_index,
            content_index,
            text,
        } => maybe_emit_output_text_done(output_index, content_index, text, streamed_text_parts),
        VerboseGatewayEvent::OutputItemDone {
            output_index,
            item_id,
            item_type,
            item,
        } => handle_output_item_done(
            current_response_id,
            completed_items,
            output_index,
            item_id,
            item_type,
            item,
        ),
        VerboseGatewayEvent::Completed { response_id } => {
            cache_completed_response(provider, current_response_id, completed_items, &response_id);
            Ok(vec![ProviderEvent::Completed { response_id }])
        }
        _ => Ok(Vec::new()),
    }
}

fn maybe_emit_output_text_done(
    output_index: Option<usize>,
    content_index: Option<usize>,
    text: String,
    streamed_text_parts: &mut HashSet<(Option<usize>, Option<usize>)>,
) -> Result<Vec<ProviderEvent>> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    if streamed_text_parts.insert((output_index, content_index)) {
        return Ok(vec![ProviderEvent::OutputTextDelta(text)]);
    }

    Ok(Vec::new())
}

fn handle_output_item_done(
    current_response_id: &Option<String>,
    completed_items: &mut BTreeMap<usize, Value>,
    output_index: usize,
    item_id: Option<String>,
    item_type: Option<String>,
    item: Option<Value>,
) -> Result<Vec<ProviderEvent>> {
    if let Some(item) = item.clone() {
        completed_items.insert(output_index, item);
    }

    if item_type.as_deref() != Some("function_call") {
        return Ok(Vec::new());
    }

    Ok(vec![build_function_call_requested(
        current_response_id.clone(),
        item_id,
        item,
    )?])
}

fn build_function_call_requested(
    response_id: Option<String>,
    item_id: Option<String>,
    item: Option<Value>,
) -> Result<ProviderEvent> {
    let response_id = response_id.ok_or_else(|| Error::Upstream {
        message: "missing response_id for function_call event".to_string(),
    })?;
    let item = item.ok_or_else(|| Error::Upstream {
        message: "missing function_call item payload".to_string(),
    })?;
    let call_id = required_item_string(&item, "call_id", "missing function_call call_id")?;
    let name = required_item_string(&item, "name", "missing function_call name")?;
    let arguments_raw = item
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}")
        .to_string();
    let arguments =
        serde_json::from_str::<Value>(&arguments_raw).map_err(|err| Error::Upstream {
            message: format!("invalid function_call arguments: {err}"),
        })?;

    Ok(ProviderEvent::FunctionCallRequested(ProviderFunctionCall {
        response_id,
        item_id,
        call_id,
        name,
        arguments_raw,
        arguments,
    }))
}

fn required_item_string(item: &Value, key: &str, message: &str) -> Result<String> {
    item.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::Upstream {
            message: message.to_string(),
        })
}

fn cache_completed_response(
    provider: &OpenAiResponsesClient,
    current_response_id: &mut Option<String>,
    completed_items: &BTreeMap<usize, Value>,
    response_id: &Option<String>,
) {
    if let Some(response_id) = response_id.clone() {
        let cached_output = completed_items.values().cloned().collect::<Vec<_>>();
        if !cached_output.is_empty() {
            provider.cache_response_output(response_id.clone(), cached_output);
        }
        *current_response_id = Some(response_id);
    }
}

fn parse_sse_data_line(line: &str) -> Option<&str> {
    if !line.starts_with("data:") {
        return None;
    }

    let data = line.trim_start_matches("data:").trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }

    Some(data)
}

fn parse_sse_payload(data: &str) -> Result<Value> {
    serde_json::from_str(data).map_err(|err| {
        tracing::warn!(error = %err, "invalid gateway SSE payload");
        Error::Upstream {
            message: format!("invalid gateway SSE payload: {err}"),
        }
    })
}

fn parse_verbose_gateway_event(
    payload: Value,
    raw_data: &str,
    sequence: &mut usize,
) -> Option<VerboseGatewayEvent> {
    let event_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    match event_type.as_str() {
        "response.created" => response_id_from_payload(&payload)
            .map(|response_id| VerboseGatewayEvent::ResponseCreated { response_id }),
        "response.in_progress" => response_id_from_payload(&payload)
            .map(|response_id| VerboseGatewayEvent::ResponseInProgress { response_id }),
        "response.output_item.added" => Some(build_output_item_added_event(&payload)),
        "response.content_part.added" => Some(VerboseGatewayEvent::ContentPartAdded {
            output_index: payload_usize(&payload, "output_index"),
            content_index: payload_usize(&payload, "content_index"),
            part_type: nested_string(&payload, "part", "type"),
        }),
        "response.output_text.delta" => payload.get("delta").and_then(Value::as_str).map(|delta| {
            VerboseGatewayEvent::OutputTextDelta {
                output_index: payload_optional_usize(&payload, "output_index"),
                content_index: payload_optional_usize(&payload, "content_index"),
                delta: delta.to_string(),
            }
        }),
        "response.output_text.done" => Some(VerboseGatewayEvent::OutputTextDone {
            output_index: payload_optional_usize(&payload, "output_index"),
            content_index: payload_optional_usize(&payload, "content_index"),
            text: payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }),
        "response.output_item.done" => Some(build_output_item_done_event(&payload)),
        "response.completed" => Some(VerboseGatewayEvent::Completed {
            response_id: response_id_from_payload(&payload),
        }),
        _ => Some(build_opaque_upstream_event(
            &event_type,
            raw_data,
            payload,
            sequence,
        )),
    }
}

fn response_id_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("response")
        .and_then(|response| response.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn build_output_item_added_event(payload: &Value) -> VerboseGatewayEvent {
    VerboseGatewayEvent::OutputItemAdded {
        output_index: payload_usize(payload, "output_index"),
        item_id: nested_string(payload, "item", "id"),
        item_type: nested_string(payload, "item", "type"),
        item: payload.get("item").cloned(),
    }
}

fn build_output_item_done_event(payload: &Value) -> VerboseGatewayEvent {
    VerboseGatewayEvent::OutputItemDone {
        output_index: payload_usize(payload, "output_index"),
        item_id: nested_string(payload, "item", "id"),
        item_type: nested_string(payload, "item", "type"),
        item: payload.get("item").cloned(),
    }
}

fn build_opaque_upstream_event(
    event_type: &str,
    raw_data: &str,
    payload: Value,
    sequence: &mut usize,
) -> VerboseGatewayEvent {
    *sequence += 1;
    tracing::debug!(event_type = %event_type, sequence = *sequence, raw_data = %raw_data, "observed opaque gateway event");
    VerboseGatewayEvent::OpaqueUpstreamEvent(ObservedGatewayEvent {
        sequence: *sequence,
        event_type: event_type.to_string(),
        raw_data: raw_data.to_string(),
        json_payload: Some(payload),
    })
}

fn payload_usize(payload: &Value, key: &str) -> usize {
    payload.get(key).and_then(Value::as_u64).unwrap_or(0) as usize
}

fn payload_optional_usize(payload: &Value, key: &str) -> Option<usize> {
    payload
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

fn nested_string(payload: &Value, object_key: &str, field_key: &str) -> Option<String> {
    payload
        .get(object_key)
        .and_then(|value| value.get(field_key))
        .and_then(Value::as_str)
        .map(str::to_string)
}
