use async_stream::try_stream;
use futures::Stream;
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::pin::Pin;

use santi_core::{
    error::{Error, Result},
    port::provider::{FunctionCallOutput, Provider, ProviderEvent, ProviderRequest},
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
}

impl OpenAiCompatibleProvider {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn stream_verbose(
        &self,
        input: ProviderRequest,
    ) -> impl Stream<Item = Result<VerboseProviderEvent>> + '_ {
        try_stream! {
            tracing::info!(model = %input.model, "upstream response request started");
            let request = map_request(input);
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
    type EventStream = Pin<Box<dyn Stream<Item = Result<ProviderEvent>> + Send>>;

    fn stream(&self, request: ProviderRequest) -> Self::EventStream {
        let provider = self.clone();

        Box::pin(try_stream! {
            let stream = provider.stream_verbose(request);
            futures::pin_mut!(stream);

            while let Some(event) = stream.next().await {
                match event? {
                    VerboseProviderEvent::OutputTextDelta(delta) => {
                        yield ProviderEvent::OutputTextDelta(delta);
                    }
                    VerboseProviderEvent::Completed { response_id } => {
                        yield ProviderEvent::Completed { response_id };
                    }
                    _ => {}
                }
            }
        })
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

fn map_request(input: ProviderRequest) -> UpstreamResponsesRequest {
    UpstreamResponsesRequest {
        model: input.model,
        instructions: input.instructions,
        input: map_input(input.input, input.function_call_output),
        tools: input.tools,
        stream: true,
        store: false,
        previous_response_id: input.previous_response_id,
    }
}

fn map_input(
    input: Vec<santi_core::provider::ProviderInputMessage>,
    function_call_output: Option<FunctionCallOutput>,
) -> Value {
    if let Some(output) = function_call_output {
        serde_json::json!([
            {
                "type": "function_call_output",
                "call_id": output.call_id,
                "output": output.output,
            }
        ])
    } else {
        serde_json::to_value(input).expect("responses input serialize failed")
    }
}

use futures::StreamExt;
