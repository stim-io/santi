mod events;
mod request;

use async_stream::try_stream;
use events::{handle_verbose_gateway_event, parse_sse_frame, VerboseGatewayEvent};
use futures::{Stream, StreamExt};
use request::{
    build_prompt_cache_key, map_function_call_outputs, map_tools, UpstreamResponsesRequest,
};
use reqwest::Client;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    pin::Pin,
    sync::{Arc, Mutex},
};

use santi_core::{
    error::{Error, Result},
    port::provider::{FunctionCallOutput, Provider, ProviderEvent, ProviderRequest},
    provider::ProviderInputMessage,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ObservedGatewayEvent {
    pub sequence: usize,
    pub event_type: String,
    pub raw_data: String,
    pub json_payload: Option<Value>,
}

#[derive(Clone)]
pub struct OpenAiResponsesClient {
    client: Client,
    api_key: String,
    base_url: String,
    response_cache: Arc<Mutex<HashMap<String, Vec<Value>>>>,
}

impl OpenAiResponsesClient {
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
    ) -> impl Stream<Item = Result<VerboseGatewayEvent>> + '_ {
        try_stream! {
            tracing::info!(model = %input.model, gateway_base_url = %self.base_url, "gateway responses request started");
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
                tracing::info!(http_status = %status, "gateway responses request finished");
                response
            } else {
                let text = response.text().await.unwrap_or_default();
                tracing::warn!(http_status = %status, error_body = %text, "gateway responses request failed");
                Err(Error::Upstream { message: format!("gateway provider error: {} {}", status, text) })?
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
            "mapped gateway responses request"
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
        input: Vec<ProviderInputMessage>,
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
                        "flattening gateway continuation input"
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
                        "missing cached response output for gateway continuation"
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
                "gateway continuation input prepared"
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

        Ok(map_provider_input_messages(input))
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
            "cached gateway response output"
        );
    }
}

fn map_provider_input_messages(input: Vec<ProviderInputMessage>) -> Value {
    Value::Array(input.into_iter().map(map_provider_input_message).collect())
}

fn map_provider_input_message(message: ProviderInputMessage) -> Value {
    let content_type = if message.role == "assistant" {
        "output_text"
    } else {
        "input_text"
    };

    json!({
        "role": message.role,
        "content": [
            {
                "type": content_type,
                "text": message.content,
            }
        ],
    })
}

impl Provider for OpenAiResponsesClient {
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
            let mut streamed_text_parts = HashSet::<(Option<usize>, Option<usize>)>::new();

            while let Some(event) = stream.next().await {
                for provider_event in handle_verbose_gateway_event(
                    &provider,
                    event?,
                    &mut current_response_id,
                    &mut completed_items,
                    &mut streamed_text_parts,
                )? {
                    yield provider_event;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use santi_core::{port::provider::ProviderRequest, provider::ProviderInputMessage};

    use super::OpenAiResponsesClient;

    #[test]
    fn maps_transcript_messages_to_responses_content_parts() {
        let client = OpenAiResponsesClient::new("test-key".into(), "http://gateway.test".into());

        let request = client
            .map_request(ProviderRequest {
                model: "test-model".into(),
                instructions: None,
                input: vec![
                    ProviderInputMessage {
                        role: "user".into(),
                        content: "first marker".into(),
                    },
                    ProviderInputMessage {
                        role: "assistant".into(),
                        content: "first marker".into(),
                    },
                    ProviderInputMessage {
                        role: "user".into(),
                        content: "what did I send before?".into(),
                    },
                ],
                tools: None,
                previous_response_id: None,
                function_call_outputs: None,
            })
            .expect("request should map");

        assert_eq!(
            request.input,
            json!([
                {
                    "role": "user",
                    "content": [{ "type": "input_text", "text": "first marker" }],
                },
                {
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "first marker" }],
                },
                {
                    "role": "user",
                    "content": [{ "type": "input_text", "text": "what did I send before?" }],
                },
            ])
        );
    }
}
