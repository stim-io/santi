use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Json, State},
    http::Uri,
    response::IntoResponse,
    routing::{any, post},
    Router,
};
use futures::StreamExt;
use santi_api::{
    chat_client::ChatCompletionsClient,
    config::{Config, Mode, ProviderApi},
    link_client::OpenAiResponsesClient,
};
use santi_core::{
    port::provider::{
        FunctionCallOutput, Provider, ProviderEvent, ProviderFunctionTool, ProviderRequest,
        ProviderTool,
    },
    provider::ProviderInputMessage,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;

#[tokio::test]
async fn chat_stream_maps_request() {
    let server = MockProvider::start(
        "/chat/completions",
        vec![chat_sse(
            r#"{"id":"chatcmpl-1","choices":[{"delta":{"content":"hi"},"finish_reason":"stop"}]}"#,
        )],
    )
    .await;
    let client = ChatCompletionsClient::new("key".into(), server.base_url.clone());

    let events = collect_events(&client, chat_request()).await;

    assert_eq!(events[0], ProviderEvent::OutputTextDelta("hi".into()));
    assert_eq!(
        events[1],
        ProviderEvent::Completed {
            response_id: Some("chatcmpl-1".into())
        }
    );

    let requests = server.requests();
    assert_eq!(requests[0]["model"], "deepseek-chat");
    assert_eq!(requests[0]["messages"][0]["role"], "system");
    assert_eq!(requests[0]["messages"][1]["role"], "user");
    assert_eq!(requests[0]["tools"][0]["function"]["name"], "bash");
    assert_eq!(requests[0]["stream"], true);
}

#[tokio::test]
async fn chat_reuses_tool_cache() {
    let server = MockProvider::start(
        "/chat/completions",
        vec![
            chat_sse(
                r#"{"id":"chatcmpl-1","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"bash","arguments":"{\"command\":\"pwd\"}"}}]},"finish_reason":"tool_calls"}]}"#,
            ),
            chat_sse(
                r#"{"id":"chatcmpl-2","choices":[{"delta":{"content":"done"},"finish_reason":"stop"}]}"#,
            ),
        ],
    )
    .await;
    let client = ChatCompletionsClient::new("key".into(), server.base_url.clone());

    let first_events = collect_events(&client, chat_request()).await;
    assert!(matches!(
        &first_events[0],
        ProviderEvent::FunctionCallRequested(call) if call.call_id == "call-1"
    ));

    let second_events = collect_events(
        &client,
        ProviderRequest {
            model: "deepseek-chat".into(),
            instructions: None,
            input: Vec::new(),
            tools: None,
            previous_response_id: Some("chatcmpl-1".into()),
            function_call_outputs: Some(vec![FunctionCallOutput {
                call_id: "call-1".into(),
                output: "ok".into(),
            }]),
        },
    )
    .await;
    assert_eq!(
        second_events[0],
        ProviderEvent::OutputTextDelta("done".into())
    );

    let requests = server.requests();
    let followup = &requests[1]["messages"];
    assert_eq!(followup[0]["role"], "system");
    assert_eq!(followup[1]["role"], "user");
    assert_eq!(followup[2]["role"], "assistant");
    assert_eq!(followup[2]["tool_calls"][0]["function"]["name"], "bash");
    assert_eq!(followup[3]["role"], "tool");
    assert_eq!(followup[3]["tool_call_id"], "call-1");
}

#[tokio::test]
async fn chat_preserves_full_base() {
    let server = MockProvider::start_any(vec![chat_sse(
        r#"{"id":"chatcmpl-1","choices":[{"delta":{"content":"hi"},"finish_reason":"stop"}]}"#,
    )])
    .await;
    let client = ChatCompletionsClient::new(
        "key".into(),
        format!("{}/chat/completions", server.base_url),
    );

    let _ = collect_events(&client, chat_request()).await;

    assert_eq!(server.paths(), vec!["/chat/completions"]);
}

#[tokio::test]
async fn responses_stream_maps_transcript() {
    let server = MockProvider::start(
        "/responses",
        vec![responses_sse(
            r#"{"type":"response.completed","response":{"id":"resp-1"}}"#,
        )],
    )
    .await;
    let client = OpenAiResponsesClient::new("key".into(), server.base_url.clone());

    let _ = collect_events(
        &client,
        ProviderRequest {
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
        },
    )
    .await;

    assert_eq!(
        server.requests()[0]["input"],
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

#[test]
fn config_redacts_probe_urls() {
    let config = config_with_base_url("https://user:secret@example.test/openai/v1?token=abc#frag");

    assert_eq!(
        config.provider_probe_url(),
        "https://user:secret@example.test/openai/v1/health"
    );
    assert_eq!(
        config.provider_probe_display_url(),
        "https://example.test/openai/v1/health"
    );
}

#[test]
fn config_keeps_gateway_url() {
    let config = config_with_base_url("http://127.0.0.1:18082/openai/v1");

    assert_eq!(
        config.meta_provider().gateway_base_url,
        Some("http://127.0.0.1:18082/openai/v1".into())
    );
}

#[test]
fn provider_api_accepts_alias() {
    assert_eq!(
        ProviderApi::from_env_value("deepseek".to_string()).unwrap(),
        ProviderApi::ChatCompletions
    );
    assert_eq!(
        ProviderApi::from_env_value("responses".to_string()).unwrap(),
        ProviderApi::Responses
    );
}

fn chat_request() -> ProviderRequest {
    ProviderRequest {
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
    }
}

async fn collect_events(provider: &dyn Provider, request: ProviderRequest) -> Vec<ProviderEvent> {
    let mut stream = provider.stream(request);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event.unwrap());
    }
    events
}

fn chat_sse(payload: &str) -> String {
    format!("data: {payload}\n\n")
}

fn responses_sse(payload: &str) -> String {
    format!("data: {payload}\n\n")
}

fn config_with_base_url(openai_base_url: &str) -> Config {
    Config {
        mode: Mode::Standalone,
        bind_addr: "127.0.0.1:8080".parse().unwrap(),
        launch_profile: None,
        provider_api: ProviderApi::Responses,
        openai_api_key: "key".into(),
        openai_base_url: openai_base_url.into(),
        openai_model: "test-model".into(),
        database_url: String::new(),
        redis_url: String::new(),
        standalone_sqlite_path: "standalone.sqlite".into(),
        execution_root: ".".into(),
        runtime_root: ".runtime".into(),
        bash_timeout_secs: 30,
        bash_output_truncate_chars: 100,
        bash_output_hard_bytes: 1000,
        hook_source: None,
    }
}

#[derive(Clone)]
struct MockProvider {
    base_url: String,
    requests: Arc<Mutex<Vec<Value>>>,
    paths: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone)]
struct MockState {
    requests: Arc<Mutex<Vec<Value>>>,
    paths: Arc<Mutex<Vec<String>>>,
    responses: Arc<Mutex<VecDeque<String>>>,
}

impl MockProvider {
    async fn start(route: &str, responses: Vec<String>) -> Self {
        Self::start_with_router(Router::new().route(route, post(mock_handler)), responses).await
    }

    async fn start_any(responses: Vec<String>) -> Self {
        Self::start_with_router(Router::new().fallback(any(mock_handler)), responses).await
    }

    async fn start_with_router(router: Router<MockState>, responses: Vec<String>) -> Self {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let paths = Arc::new(Mutex::new(Vec::new()));
        let state = MockState {
            requests: requests.clone(),
            paths: paths.clone(),
            responses: Arc::new(Mutex::new(responses.into())),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());

        tokio::spawn(async move {
            axum::serve(listener, router.with_state(state))
                .await
                .unwrap();
        });

        Self {
            base_url,
            requests,
            paths,
        }
    }

    fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }

    fn paths(&self) -> Vec<String> {
        self.paths.lock().unwrap().clone()
    }
}

async fn mock_handler(
    State(state): State<MockState>,
    uri: Uri,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    state.paths.lock().unwrap().push(uri.path().to_string());
    state.requests.lock().unwrap().push(payload);
    let body = state
        .responses
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or_else(|| {
            responses_sse(r#"{"type":"response.completed","response":{"id":"resp-default"}}"#)
        });

    ([("content-type", "text/event-stream")], body).into_response()
}
