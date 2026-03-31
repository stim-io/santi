use std::time::Duration;

use async_stream::try_stream;
use async_trait::async_trait;
use futures::TryStreamExt;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use santi_core::hook::HookSpecSource;
use tokio::time::sleep;

use crate::{
    backend::{
        BackendError, CliBackend, CliCompact, CliHealth, CliHookReload, CliMemoryRecord,
        CliMessage, CliSession, CliSoul, SendEvent, SendStream,
    },
    config::Config,
};

#[derive(Clone)]
pub struct ApiBackend {
    client: Client,
    config: Config,
}

#[derive(Debug, Deserialize)]
struct SessionResponse {
    id: String,
    parent_session_id: Option<String>,
    fork_point: Option<i64>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct SessionMessagesResponse {
    messages: Vec<CliMessage>,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
}

#[derive(Debug, Deserialize)]
struct SoulResponse {
    id: String,
    memory: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct MemoryResponse {
    id: String,
    memory: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    message: String,
}

#[derive(Debug, Serialize)]
struct SessionSendRequest {
    content: Vec<SessionSendContentPart>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum SessionSendContentPart {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Serialize)]
struct MemoryRequest {
    text: String,
}

#[derive(Debug, Serialize)]
struct SessionCompactRequest {
    summary: String,
}

#[derive(Debug, Serialize)]
struct HookReloadRequest {
    #[serde(flatten)]
    source: HookSpecSource,
}

#[derive(Debug, Deserialize)]
struct SessionOutputTextDeltaEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: String,
}

#[derive(Debug, Deserialize)]
struct SessionCompletedEvent {
    #[serde(rename = "type")]
    event_type: String,
}

impl ApiBackend {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url.trim_end_matches('/'), path)
    }
}

#[async_trait]
impl CliBackend for ApiBackend {
    async fn health(&self) -> Result<CliHealth, BackendError> {
        let response = self
            .client
            .get(self.endpoint("/api/v1/health"))
            .send()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        response
            .json::<HealthResponse>()
            .await
            .map(|body| CliHealth {
                status: body.status,
            })
            .map_err(|err| BackendError::Other(err.to_string()))
    }

    async fn create_session(&self) -> Result<CliSession, BackendError> {
        let response = self
            .client
            .post(self.endpoint("/api/v1/sessions"))
            .send()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        let session = response
            .json::<SessionResponse>()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        Ok(CliSession {
            id: session.id,
            parent_session_id: session.parent_session_id,
            fork_point: session.fork_point,
            created_at: session.created_at,
        })
    }

    async fn get_session(&self, session_id: String) -> Result<CliSession, BackendError> {
        let response = self
            .client
            .get(self.endpoint(&format!("/api/v1/sessions/{session_id}")))
            .send()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        let session = response
            .json::<SessionResponse>()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        Ok(CliSession {
            id: session.id,
            parent_session_id: session.parent_session_id,
            fork_point: session.fork_point,
            created_at: session.created_at,
        })
    }

    async fn send_session(
        &self,
        session_id: String,
        content: String,
        wait: bool,
    ) -> Result<SendStream, BackendError> {
        let url = self.endpoint(&format!("/api/v1/sessions/{session_id}/send"));
        let request_body = SessionSendRequest {
            content: vec![SessionSendContentPart::Text { text: content }],
        };

        let response = loop {
            let response = self
                .client
                .post(&url)
                .json(&request_body)
                .send()
                .await
                .map_err(|err| BackendError::Other(err.to_string()))?;

            if response.status() != StatusCode::CONFLICT || !wait {
                break response;
            }

            sleep(Duration::from_millis(350)).await;
        };

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        let bytes_stream = response.bytes_stream();

        Ok(Box::pin(try_stream! {
            let mut buffer = String::new();
            futures::pin_mut!(bytes_stream);

            while let Some(chunk) = bytes_stream.try_next().await.map_err(|err| BackendError::Other(err.to_string()))? {
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(idx) = buffer.find('\n') {
                    let line = buffer[..idx].trim().to_string();
                    buffer = buffer[idx + 1..].to_string();

                    if !line.starts_with("data: ") {
                        continue;
                    }

                    let payload = &line[6..];
                    if payload == "[DONE]" || payload.is_empty() {
                        continue;
                    }

                    if let Ok(error) = serde_json::from_str::<ErrorResponse>(payload) {
                        Err(BackendError::Other(error.error.message))?;
                    }

                    if let Ok(event) = serde_json::from_str::<SessionOutputTextDeltaEvent>(payload) {
                        if event.event_type == "response.output_text.delta" {
                            yield SendEvent::OutputTextDelta(event.delta);
                            continue;
                        }
                    }

                    if let Ok(event) = serde_json::from_str::<SessionCompletedEvent>(payload) {
                        if event.event_type == "response.completed" {
                            yield SendEvent::Completed;
                            continue;
                        }
                    }
                }
            }
        }))
    }

    async fn list_messages(&self, session_id: String) -> Result<Vec<CliMessage>, BackendError> {
        let response = self
            .client
            .get(self.endpoint(&format!("/api/v1/sessions/{session_id}/messages")))
            .send()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        let body = response
            .json::<SessionMessagesResponse>()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;
        Ok(body.messages)
    }

    async fn get_default_soul(&self) -> Result<CliSoul, BackendError> {
        let response = self
            .client
            .get(self.endpoint("/api/v1/soul"))
            .send()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        let soul = response
            .json::<SoulResponse>()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        Ok(CliSoul {
            id: soul.id,
            memory: soul.memory,
            created_at: Some(soul.created_at),
            updated_at: soul.updated_at,
        })
    }

    async fn set_default_soul_memory(&self, text: String) -> Result<CliMemoryRecord, BackendError> {
        let response = self
            .client
            .put(self.endpoint("/api/v1/soul/memory"))
            .json(&MemoryRequest { text })
            .send()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        let body = response
            .json::<MemoryResponse>()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;
        Ok(CliMemoryRecord {
            id: body.id,
            memory: body.memory,
            updated_at: body.updated_at,
        })
    }

    async fn set_session_memory(
        &self,
        session_id: String,
        text: String,
    ) -> Result<CliMemoryRecord, BackendError> {
        let response = self
            .client
            .put(self.endpoint(&format!("/api/v1/sessions/{session_id}/memory")))
            .json(&MemoryRequest { text })
            .send()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        let body = response
            .json::<MemoryResponse>()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;
        Ok(CliMemoryRecord {
            id: body.id,
            memory: body.memory,
            updated_at: body.updated_at,
        })
    }

    async fn compact_session(
        &self,
        session_id: String,
        summary: String,
    ) -> Result<CliCompact, BackendError> {
        let response = self
            .client
            .post(self.endpoint(&format!("/api/v1/sessions/{session_id}/compact")))
            .json(&SessionCompactRequest { summary })
            .send()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        response
            .json::<CliCompact>()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))
    }

    async fn reload_hooks(&self, source: HookSpecSource) -> Result<CliHookReload, BackendError> {
        let response = self
            .client
            .put(self.endpoint("/api/v1/admin/hooks"))
            .json(&HookReloadRequest { source })
            .send()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))?;

        if !response.status().is_success() {
            return Err(map_error_response(response).await);
        }

        response
            .json::<CliHookReload>()
            .await
            .map_err(|err| BackendError::Other(err.to_string()))
    }
}

async fn map_error_response(response: reqwest::Response) -> BackendError {
    let status = response.status();
    let fallback = format!("http {}", status.as_u16());
    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<ErrorResponse>(&body)
        .map(|error| error.error.message)
        .unwrap_or(if body.trim().is_empty() {
            fallback
        } else {
            body
        });

    match status {
        StatusCode::NOT_FOUND => BackendError::NotFound,
        StatusCode::CONFLICT => BackendError::Busy,
        _ => BackendError::Other(message),
    }
}
