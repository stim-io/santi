use async_stream::try_stream;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    pin::Pin,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use santi_core::{
    error::{Error, Result},
    port::provider::{
        FunctionCallOutput, Provider, ProviderEvent, ProviderFunctionCall, ProviderRequest,
        ProviderTool,
    },
    provider::ProviderInputMessage,
};

#[derive(Clone)]
pub struct ChatCompletionsClient {
    client: Client,
    api_key: String,
    base_url: String,
    response_cache: Arc<Mutex<HashMap<String, Vec<Value>>>>,
}

#[derive(Debug, Serialize)]
struct ChatCompletionsRequest {
    model: String,
    messages: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    stream: bool,
}

#[derive(Clone, Debug, Default)]
struct ToolCallDraft {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

struct ChatStreamState {
    response_id: Option<String>,
    request_messages: Vec<Value>,
    assistant_text: String,
    tool_calls: BTreeMap<usize, ToolCallDraft>,
    completed: bool,
}

impl ChatCompletionsClient {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            response_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn endpoint(&self) -> String {
        if self.base_url.ends_with("/chat/completions") {
            self.base_url.clone()
        } else {
            format!("{}/chat/completions", self.base_url)
        }
    }

    fn stream_verbose(
        &self,
        input: ProviderRequest,
    ) -> impl Stream<Item = Result<ProviderEvent>> + '_ {
        try_stream! {
            tracing::info!(model = %input.model, gateway_base_url = %self.base_url, "chat completions request started");
            let request = self.map_request(input)?;
            let mut state = ChatStreamState::new(request.messages.clone());
            let response = self
                .client
                .post(self.endpoint())
                .bearer_auth(&self.api_key)
                .json(&request)
                .send()
                .await
                .map_err(|err| Error::Upstream { message: err.to_string() })?;

            let status = response.status();
            let response = if status.is_success() {
                tracing::info!(http_status = %status, "chat completions request finished");
                response
            } else {
                let text = response.text().await.unwrap_or_default();
                tracing::warn!(http_status = %status, error_body = %text, "chat completions request failed");
                Err(Error::Upstream { message: format!("chat completions provider error: {status} {text}") })?
            };

            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|err| Error::Upstream { message: err.to_string() })?;
                let text = String::from_utf8_lossy(&chunk);
                buffer.push_str(&text);

                while let Some(index) = buffer.find("\n\n") {
                    let frame = buffer[..index].to_string();
                    buffer = buffer[index + 2..].to_string();

                    for payload in parse_sse_frame(&frame)? {
                        for event in state.handle_payload(self, payload)? {
                            yield event;
                        }
                    }
                }
            }

            if !buffer.trim().is_empty() {
                for payload in parse_sse_frame(buffer.trim())? {
                    for event in state.handle_payload(self, payload)? {
                        yield event;
                    }
                }
            }

            if !state.completed {
                yield ProviderEvent::Completed {
                    response_id: state.response_id.clone(),
                };
            }
        }
    }

    fn map_request(&self, input: ProviderRequest) -> Result<ChatCompletionsRequest> {
        Ok(ChatCompletionsRequest {
            model: input.model,
            messages: self.map_messages(
                input.instructions,
                input.input,
                input.previous_response_id.as_deref(),
                input.function_call_outputs,
            )?,
            tools: map_chat_tools(input.tools),
            stream: true,
        })
    }

    fn map_messages(
        &self,
        instructions: Option<String>,
        input: Vec<ProviderInputMessage>,
        previous_response_id: Option<&str>,
        function_call_outputs: Option<Vec<FunctionCallOutput>>,
    ) -> Result<Vec<Value>> {
        if let Some(previous_response_id) = previous_response_id {
            let mut messages = self.cached_messages(previous_response_id)?;
            messages.extend(map_tool_outputs(function_call_outputs.unwrap_or_default()));
            return Ok(messages);
        }

        let mut messages = Vec::new();
        if let Some(instructions) = instructions.filter(|value| !value.trim().is_empty()) {
            messages.push(json!({
                "role": "system",
                "content": instructions,
            }));
        }

        messages.extend(map_provider_input_messages(input));
        if let Some(outputs) = function_call_outputs {
            messages.extend(map_tool_outputs(outputs));
        }

        Ok(messages)
    }

    fn cached_messages(&self, previous_response_id: &str) -> Result<Vec<Value>> {
        let cache = self.response_cache.lock().expect("response cache poisoned");
        cache
            .get(previous_response_id)
            .cloned()
            .ok_or_else(|| Error::Upstream {
                message: format!(
                    "missing cached chat completion response for {previous_response_id}"
                ),
            })
    }

    fn cache_response_messages(&self, response_id: String, messages: Vec<Value>) {
        let mut cache = self.response_cache.lock().expect("response cache poisoned");
        cache.insert(response_id, messages);
    }
}

impl Provider for ChatCompletionsClient {
    fn stream(
        &self,
        request: ProviderRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<ProviderEvent>> + Send>> {
        let provider = self.clone();

        Box::pin(try_stream! {
            let stream = provider.stream_verbose(request);
            futures::pin_mut!(stream);

            while let Some(event) = stream.next().await {
                yield event?;
            }
        })
    }
}

impl ChatStreamState {
    fn new(request_messages: Vec<Value>) -> Self {
        Self {
            response_id: None,
            request_messages,
            assistant_text: String::new(),
            tool_calls: BTreeMap::new(),
            completed: false,
        }
    }

    fn handle_payload(
        &mut self,
        provider: &ChatCompletionsClient,
        payload: Value,
    ) -> Result<Vec<ProviderEvent>> {
        if let Some(response_id) = payload.get("id").and_then(Value::as_str) {
            self.response_id
                .get_or_insert_with(|| response_id.to_string());
        }

        let mut events = Vec::new();
        let Some(choices) = payload.get("choices").and_then(Value::as_array) else {
            return Ok(events);
        };

        for choice in choices {
            if let Some(delta) = choice.get("delta") {
                events.extend(self.handle_delta(delta)?);
            }

            if choice
                .get("finish_reason")
                .and_then(Value::as_str)
                .is_some()
            {
                events.extend(self.complete_choice(provider)?);
            }
        }

        Ok(events)
    }

    fn handle_delta(&mut self, delta: &Value) -> Result<Vec<ProviderEvent>> {
        let mut events = Vec::new();

        if let Some(content) = delta.get("content").and_then(Value::as_str) {
            if !content.is_empty() {
                self.assistant_text.push_str(content);
                events.push(ProviderEvent::OutputTextDelta(content.to_string()));
            }
        }

        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                self.merge_tool_call_delta(tool_call)?;
            }
        }

        Ok(events)
    }

    fn merge_tool_call_delta(&mut self, tool_call: &Value) -> Result<()> {
        let index = tool_call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let draft = self.tool_calls.entry(index).or_default();

        if let Some(id) = tool_call.get("id").and_then(Value::as_str) {
            draft.id.get_or_insert_with(|| id.to_string());
        }

        if let Some(function) = tool_call.get("function") {
            if let Some(name) = function.get("name").and_then(Value::as_str) {
                draft.name.get_or_insert_with(|| name.to_string());
            }
            if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
                draft.arguments.push_str(arguments);
            }
        }

        Ok(())
    }

    fn complete_choice(&mut self, provider: &ChatCompletionsClient) -> Result<Vec<ProviderEvent>> {
        if self.completed {
            return Ok(Vec::new());
        }

        self.completed = true;
        let response_id = self.ensure_response_id();
        let mut events = Vec::new();

        if !self.tool_calls.is_empty() {
            let calls = self.provider_tool_calls(&response_id)?;
            provider.cache_response_messages(
                response_id.clone(),
                self.cached_messages_for_tool_followup(&calls),
            );
            events.extend(calls.into_iter().map(ProviderEvent::FunctionCallRequested));
        }

        events.push(ProviderEvent::Completed {
            response_id: Some(response_id),
        });
        Ok(events)
    }

    fn ensure_response_id(&mut self) -> String {
        self.response_id
            .get_or_insert_with(|| format!("chatcmpl_{}", Uuid::new_v4().simple()))
            .clone()
    }

    fn provider_tool_calls(&self, response_id: &str) -> Result<Vec<ProviderFunctionCall>> {
        self.tool_calls
            .iter()
            .map(|(index, draft)| draft.to_provider_call(response_id, *index))
            .collect()
    }

    fn cached_messages_for_tool_followup(&self, calls: &[ProviderFunctionCall]) -> Vec<Value> {
        let mut messages = self.request_messages.clone();
        let tool_calls = calls
            .iter()
            .map(|call| {
                json!({
                    "id": call.call_id,
                    "type": "function",
                    "function": {
                        "name": call.name,
                        "arguments": call.arguments_raw,
                    },
                })
            })
            .collect::<Vec<_>>();

        messages.push(json!({
            "role": "assistant",
            "content": if self.assistant_text.is_empty() {
                Value::Null
            } else {
                Value::String(self.assistant_text.clone())
            },
            "tool_calls": tool_calls,
        }));
        messages
    }
}

impl ToolCallDraft {
    fn to_provider_call(&self, response_id: &str, index: usize) -> Result<ProviderFunctionCall> {
        let call_id = self
            .id
            .clone()
            .unwrap_or_else(|| format!("call_{}", Uuid::new_v4().simple()));
        let name = self.name.clone().ok_or_else(|| Error::Upstream {
            message: format!("missing chat completion tool call name at index {index}"),
        })?;
        let arguments_raw = if self.arguments.trim().is_empty() {
            "{}".to_string()
        } else {
            self.arguments.clone()
        };
        let arguments =
            serde_json::from_str::<Value>(&arguments_raw).map_err(|err| Error::Upstream {
                message: format!("invalid chat completion tool call arguments: {err}"),
            })?;

        Ok(ProviderFunctionCall {
            response_id: response_id.to_string(),
            item_id: Some(call_id.clone()),
            call_id,
            name,
            arguments_raw,
            arguments,
        })
    }
}

fn parse_sse_frame(frame: &str) -> Result<Vec<Value>> {
    let mut payloads = Vec::new();

    for line in frame.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data.is_empty() || data == "[DONE]" {
            continue;
        }

        payloads.push(serde_json::from_str(data).map_err(|err| Error::Upstream {
            message: format!("invalid chat completion SSE payload: {err}"),
        })?);
    }

    Ok(payloads)
}

fn map_provider_input_messages(input: Vec<ProviderInputMessage>) -> Vec<Value> {
    input
        .into_iter()
        .map(|message| {
            json!({
                "role": message.role,
                "content": message.content,
            })
        })
        .collect()
}

fn map_tool_outputs(outputs: Vec<FunctionCallOutput>) -> Vec<Value> {
    outputs
        .into_iter()
        .map(|output| {
            json!({
                "role": "tool",
                "tool_call_id": output.call_id,
                "content": output.output,
            })
        })
        .collect()
}

fn map_chat_tools(tools: Option<Vec<ProviderTool>>) -> Option<Vec<Value>> {
    tools.map(|tools| {
        tools
            .into_iter()
            .map(|tool| match tool {
                ProviderTool::Function(tool) => json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                    },
                }),
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;
    use santi_core::{
        port::provider::{ProviderFunctionTool, ProviderTool},
        provider::ProviderInputMessage,
    };

    #[test]
    fn maps_initial_request_to_chat_messages_and_tools() {
        let client = ChatCompletionsClient::new("key".into(), "https://api.deepseek.com".into());
        let request = client
            .map_request(ProviderRequest {
                model: "deepseek-chat".into(),
                instructions: Some("system guidance".into()),
                input: vec![ProviderInputMessage {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                tools: Some(vec![ProviderTool::Function(ProviderFunctionTool {
                    name: "bash".into(),
                    description: "run bash".into(),
                    parameters: json!({ "type": "object" }),
                })]),
                previous_response_id: None,
                function_call_outputs: None,
            })
            .unwrap();

        assert_eq!(request.model, "deepseek-chat");
        assert_eq!(request.messages[0]["role"], "system");
        assert_eq!(request.messages[1]["role"], "user");
        assert_eq!(request.tools.unwrap()[0]["function"]["name"], "bash");
        assert!(request.stream);
    }

    #[test]
    fn parses_chat_completion_content_delta() {
        let frame = r#"data: {"id":"chatcmpl-1","choices":[{"delta":{"content":"hi"},"finish_reason":null}]}"#;
        let payloads = parse_sse_frame(frame).unwrap();
        let client = ChatCompletionsClient::new("key".into(), "https://api.deepseek.com".into());
        let mut state = ChatStreamState::new(Vec::new());
        let events = state.handle_payload(&client, payloads[0].clone()).unwrap();

        assert_eq!(events, vec![ProviderEvent::OutputTextDelta("hi".into())]);
    }

    #[test]
    fn caches_tool_call_context_for_followup_outputs() {
        let client = ChatCompletionsClient::new("key".into(), "https://api.deepseek.com".into());
        let mut state = ChatStreamState::new(vec![json!({"role": "user", "content": "run pwd"})]);
        let payload = json!({
            "id": "chatcmpl-1",
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call-1",
                        "type": "function",
                        "function": {"name": "bash", "arguments": "{\"command\":\"pwd\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let events = state.handle_payload(&client, payload).unwrap();
        assert!(matches!(events[0], ProviderEvent::FunctionCallRequested(_)));
        assert!(matches!(events[1], ProviderEvent::Completed { .. }));

        let followup = client
            .map_messages(
                None,
                Vec::new(),
                Some("chatcmpl-1"),
                Some(vec![FunctionCallOutput {
                    call_id: "call-1".into(),
                    output: "ok".into(),
                }]),
            )
            .unwrap();

        assert_eq!(followup[1]["role"], "assistant");
        assert_eq!(followup[1]["tool_calls"][0]["function"]["name"], "bash");
        assert_eq!(followup[2]["role"], "tool");
        assert_eq!(followup[2]["tool_call_id"], "call-1");
    }

    #[tokio::test]
    async fn endpoint_appends_chat_completions_when_base_url_is_root() {
        let client = ChatCompletionsClient::new("key".into(), "https://api.deepseek.com".into());
        assert_eq!(
            client.endpoint(),
            "https://api.deepseek.com/chat/completions"
        );

        let client = ChatCompletionsClient::new(
            "key".into(),
            "https://api.deepseek.com/chat/completions".into(),
        );
        assert_eq!(
            client.endpoint(),
            "https://api.deepseek.com/chat/completions"
        );

        let mut stream = client.stream(ProviderRequest {
            model: "deepseek-chat".into(),
            instructions: None,
            input: Vec::new(),
            tools: None,
            previous_response_id: Some("missing".into()),
            function_call_outputs: None,
        });
        assert!(stream.next().await.unwrap().is_err());
    }
}
