use async_stream::try_stream;
use futures::Stream;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    pin::Pin,
    sync::{Arc, Mutex},
};

use santi_core::{
    error::{Error, Result},
    port::provider::{
        FunctionCallOutput, Provider, ProviderEvent, ProviderFunctionCall, ProviderRequest,
        ProviderTool,
    },
};

#[derive(Clone, Debug, PartialEq)]
enum VerboseProviderEvent {
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
    OutputTextDelta(String),
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
    OpaqueUpstreamEvent(ObservedUpstreamEvent),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObservedUpstreamEvent {
    pub sequence: usize,
    pub event_type: String,
    pub raw_data: String,
    pub json_payload: Option<Value>,
}

#[derive(Clone)]
pub struct OpenAiCompatibleProvider {
    client: Client,
    api_key: String,
    base_url: String,
    response_cache: Arc<Mutex<HashMap<String, Vec<Value>>>>,
}

#[derive(Debug, Serialize)]
struct UpstreamResponsesRequest {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    stream: bool,
    store: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_response_id: Option<String>,
    prompt_cache_key: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            response_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn stream_verbose(
        &self,
        input: ProviderRequest,
    ) -> impl Stream<Item = Result<VerboseProviderEvent>> + '_ {
        try_stream! {
            tracing::info!(model = %input.model, "upstream response request started");
            let request = self.map_request(input)?;
            let response = self
                .client
                .post(format!("{}/responses", self.base_url))
                .bearer_auth(&self.api_key)
                .json(&request)
                .send()
                .await
                .map_err(|err| Error::Upstream { message: err.to_string() })?;

            let status = response.status();
            let response = if status.is_success() {
                tracing::info!(http_status = %status, "upstream response request finished");
                response
            } else {
                let text = response.text().await.unwrap_or_default();
                tracing::warn!(http_status = %status, error_body = %text, "upstream response request failed");
                Err(Error::Upstream { message: format!("upstream provider error: {} {}", status, text) })?
            };

            let mut stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut sequence = 0usize;

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|err| Error::Upstream { message: err.to_string() })?;
                let text = String::from_utf8_lossy(&chunk);
                buffer.push_str(&text);

                while let Some(index) = buffer.find("\n\n") {
                    let frame = buffer[..index].to_string();
                    buffer = buffer[index + 2..].to_string();

                    for event in parse_sse_frame(&frame, &mut sequence)? {
                        yield event;
                    }
                }
            }

            if !buffer.trim().is_empty() {
                for event in parse_sse_frame(buffer.trim(), &mut sequence)? {
                    yield event;
                }
            }
        }
    }
}

impl Provider for OpenAiCompatibleProvider {
    fn stream(
        &self,
        request: ProviderRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<ProviderEvent>> + Send>> {
        let provider = self.clone();

        Box::pin(try_stream! {
            let stream = provider.stream_verbose(request);
            futures::pin_mut!(stream);
            let mut current_response_id: Option<String> = None;
            let mut completed_items = BTreeMap::<usize, Value>::new();

            while let Some(event) = stream.next().await {
                match event? {
                    VerboseProviderEvent::ResponseCreated { response_id }
                    | VerboseProviderEvent::ResponseInProgress { response_id } => {
                        current_response_id = Some(response_id);
                    }
                    VerboseProviderEvent::OutputTextDelta(delta) => {
                        yield ProviderEvent::OutputTextDelta(delta);
                    }
                    VerboseProviderEvent::OutputItemDone {
                        output_index,
                        item_id,
                        item_type,
                        item,
                    } => {
                        if let Some(item) = item.clone() {
                            completed_items.insert(output_index, item);
                        }

                        if item_type.as_deref() != Some("function_call") {
                            continue;
                        }

                        let response_id = current_response_id.clone().ok_or_else(|| Error::Upstream {
                            message: "missing response_id for function_call event".to_string(),
                        })?;
                        let item = item.ok_or_else(|| Error::Upstream {
                            message: "missing function_call item payload".to_string(),
                        })?;
                        let call_id = item
                            .get("call_id")
                            .and_then(Value::as_str)
                            .ok_or_else(|| Error::Upstream {
                                message: "missing function_call call_id".to_string(),
                            })?
                            .to_string();
                        let name = item
                            .get("name")
                            .and_then(Value::as_str)
                            .ok_or_else(|| Error::Upstream {
                                message: "missing function_call name".to_string(),
                            })?
                            .to_string();
                        let arguments_raw = item
                            .get("arguments")
                            .and_then(Value::as_str)
                            .unwrap_or("{}")
                            .to_string();
                        let arguments = serde_json::from_str::<Value>(&arguments_raw).map_err(|err| {
                            Error::Upstream {
                                message: format!("invalid function_call arguments: {err}"),
                            }
                        })?;

                        yield ProviderEvent::FunctionCallRequested(ProviderFunctionCall {
                            response_id,
                            item_id,
                            call_id,
                            name,
                            arguments_raw,
                            arguments,
                        });
                    }
                    VerboseProviderEvent::Completed { response_id } => {
                        if let Some(response_id) = response_id.clone() {
                            let cached_output = completed_items.values().cloned().collect::<Vec<_>>();
                            if !cached_output.is_empty() {
                                provider.cache_response_output(response_id.clone(), cached_output);
                            }
                            current_response_id = Some(response_id);
                        }
                        yield ProviderEvent::Completed { response_id };
                    }
                    _ => {}
                }
            }
        })
    }
}

impl OpenAiCompatibleProvider {
    fn map_request(&self, input: ProviderRequest) -> Result<UpstreamResponsesRequest> {
        let prompt_cache_key = build_prompt_cache_key(&input);
        let previous_response_id = input.previous_response_id.clone();
        let function_call_outputs_count = input
            .function_call_outputs
            .as_ref()
            .map(|outputs| outputs.len())
            .unwrap_or(0);
        let request_input = self.map_input(
            input.input,
            previous_response_id.as_deref(),
            input.function_call_outputs,
        )?;

        tracing::debug!(
            has_previous_response_id = previous_response_id.is_some(),
            function_call_outputs_count,
            input_items = request_input.as_array().map(|items| items.len()).unwrap_or(0),
            prompt_cache_key = %prompt_cache_key,
            "mapped openai responses request"
        );

        Ok(UpstreamResponsesRequest {
            model: input.model,
            instructions: input.instructions,
            input: request_input,
            tools: map_tools(input.tools),
            stream: true,
            store: false,
            previous_response_id: None,
            prompt_cache_key,
        })
    }

    fn map_input(
        &self,
        input: Vec<santi_core::provider::ProviderInputMessage>,
        previous_response_id: Option<&str>,
        function_call_outputs: Option<Vec<FunctionCallOutput>>,
    ) -> Result<Value> {
        if let Some(previous_response_id) = previous_response_id {
            let function_call_outputs_count = function_call_outputs
                .as_ref()
                .map(|outputs| outputs.len())
                .unwrap_or(0);
            let cache = self.response_cache.lock().expect("response cache poisoned");
            let response_cache_size = cache.len();
            let cached_output = match cache.get(previous_response_id).cloned() {
                Some(cached_output) => {
                    tracing::debug!(
                        previous_response_id,
                        cache_hit = true,
                        cached_output_items = cached_output.len(),
                        function_call_outputs_count,
                        response_cache_size,
                        flattening_applied = true,
                        "flattening provider continuation input"
                    );
                    cached_output
                }
                None => {
                    let known_cached_response_ids_sample =
                        cache.keys().take(3).cloned().collect::<Vec<_>>().join(",");
                    tracing::warn!(
                        previous_response_id,
                        cache_hit = false,
                        function_call_outputs_count,
                        response_cache_size,
                        known_cached_response_ids_sample,
                        "missing cached response output for provider continuation"
                    );
                    return Err(Error::Upstream {
                        message: format!(
                            "missing cached response output for {previous_response_id}"
                        ),
                    });
                }
            };
            drop(cache);

            let mut merged = cached_output;
            if let Some(outputs) = function_call_outputs {
                merged.extend(map_function_call_outputs(outputs));
            }

            tracing::debug!(
                previous_response_id,
                merged_items_count = merged.len(),
                "provider continuation input prepared"
            );
            return Ok(Value::Array(merged));
        }

        if let Some(outputs) = function_call_outputs {
            let mapped = map_function_call_outputs(outputs);
            tracing::debug!(
                function_call_outputs_count = mapped.len(),
                "mapped function call outputs without cached continuation context"
            );
            return Ok(Value::Array(mapped));
        }

        Ok(serde_json::to_value(input).expect("responses input serialize failed"))
    }

    fn cache_response_output(&self, response_id: String, output: Vec<Value>) {
        let item_types = output
            .iter()
            .filter_map(|item| item.get("type").and_then(Value::as_str))
            .map(str::to_string)
            .collect::<Vec<_>>();
        let function_call_items_count = item_types
            .iter()
            .filter(|kind| **kind == "function_call")
            .count();
        let text_items_count = item_types.iter().filter(|kind| **kind == "message").count();

        let mut cache = self.response_cache.lock().expect("response cache poisoned");
        cache.insert(response_id.clone(), output);
        let cache_size_after_insert = cache.len();

        tracing::debug!(
            response_id,
            output_items_count = item_types.len(),
            item_types = %item_types.join(","),
            function_call_items_count,
            text_items_count,
            cache_size_after_insert,
            "cached provider response output"
        );
    }
}

fn parse_sse_frame(frame: &str, sequence: &mut usize) -> Result<Vec<VerboseProviderEvent>> {
    let mut events = Vec::new();

    for line in frame.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if !line.starts_with("data:") {
            continue;
        }

        let data = line.trim_start_matches("data:").trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }

        let payload: serde_json::Value = serde_json::from_str(data).map_err(|err| {
            tracing::warn!(error = %err, "invalid upstream SSE payload");
            Error::Upstream {
                message: format!("invalid upstream SSE payload: {err}"),
            }
        })?;

        let event_type = payload
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();

        if event_type == "response.created" {
            if let Some(response_id) = payload
                .get("response")
                .and_then(|response| response.get("id"))
                .and_then(Value::as_str)
            {
                events.push(VerboseProviderEvent::ResponseCreated {
                    response_id: response_id.to_string(),
                });
                continue;
            }
        }

        if event_type == "response.in_progress" {
            if let Some(response_id) = payload
                .get("response")
                .and_then(|response| response.get("id"))
                .and_then(Value::as_str)
            {
                events.push(VerboseProviderEvent::ResponseInProgress {
                    response_id: response_id.to_string(),
                });
                continue;
            }
        }

        if event_type == "response.output_item.added" {
            let output_index = payload
                .get("output_index")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let item_id = payload
                .get("item")
                .and_then(|item| item.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let item_type = payload
                .get("item")
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let item = payload.get("item").cloned();

            events.push(VerboseProviderEvent::OutputItemAdded {
                output_index,
                item_id,
                item_type,
                item,
            });
            continue;
        }

        if event_type == "response.content_part.added" {
            let output_index = payload
                .get("output_index")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let content_index = payload
                .get("content_index")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let part_type = payload
                .get("part")
                .and_then(|part| part.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string);

            events.push(VerboseProviderEvent::ContentPartAdded {
                output_index,
                content_index,
                part_type,
            });
            continue;
        }

        if event_type == "response.output_text.delta" {
            if let Some(content) = payload.get("delta").and_then(Value::as_str) {
                events.push(VerboseProviderEvent::OutputTextDelta(content.to_string()));
            }
            continue;
        }

        if event_type == "response.output_text.done" {
            let output_index = payload
                .get("output_index")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            let content_index = payload
                .get("content_index")
                .and_then(Value::as_u64)
                .map(|v| v as usize);
            let text = payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();

            events.push(VerboseProviderEvent::OutputTextDone {
                output_index,
                content_index,
                text,
            });
            continue;
        }

        if event_type == "response.output_item.done" {
            let output_index = payload
                .get("output_index")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let item_id = payload
                .get("item")
                .and_then(|item| item.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let item_type = payload
                .get("item")
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let item = payload.get("item").cloned();

            events.push(VerboseProviderEvent::OutputItemDone {
                output_index,
                item_id,
                item_type,
                item,
            });
            continue;
        }

        if event_type == "response.completed" {
            let response_id = payload
                .get("response")
                .and_then(|response| response.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            events.push(VerboseProviderEvent::Completed { response_id });
            continue;
        }

        *sequence += 1;
        tracing::debug!(event_type = %event_type, sequence = *sequence, raw_data = %data, "observed opaque upstream event");
        events.push(VerboseProviderEvent::OpaqueUpstreamEvent(
            ObservedUpstreamEvent {
                sequence: *sequence,
                event_type: event_type.to_string(),
                raw_data: data.to_string(),
                json_payload: Some(payload),
            },
        ));
    }

    Ok(events)
}

fn build_prompt_cache_key(input: &ProviderRequest) -> String {
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
    let prompt_cache_key = format!("santi:oai:v1:{:x}", hasher.finalize());

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
        "built openai prompt cache key"
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

fn map_tools(tools: Option<Vec<ProviderTool>>) -> Option<Vec<Value>> {
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

fn map_function_call_outputs(outputs: Vec<FunctionCallOutput>) -> Vec<Value> {
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

use futures::StreamExt;
