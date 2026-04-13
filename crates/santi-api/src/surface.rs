use std::{pin::Pin, sync::Arc};

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use santi_core::port::ebus::SubscriberSetPort;
use santi_core::{
    hook::HookSpecSource,
    model::{
        effect::SessionEffect, runtime::Compact, session::Session, session::SessionMessage,
        soul::Soul,
    },
    port::effect_ledger::EffectLedgerPort,
};
use santi_runtime::hooks::{compile_hook_specs, load_hook_specs, HookEvaluator};
use santi_runtime::session::{
    compact::{CompactSessionError, SessionCompactService},
    fork::{ForkError, ForkResult, SessionForkService},
    memory::SessionMemoryService,
    query::SessionQueryService,
    send::{SendSessionCommand, SendSessionError, SendSessionEvent, SessionSendService},
    watch::{SessionWatchError, SessionWatchEvent, SessionWatchService, SessionWatchSnapshot},
};

use crate::{
    config::Mode,
    schema::{
        common::ErrorResponse, session::SessionMemoryResponse, session_events::SessionStreamEvent,
    },
};

pub type SessionEventStream =
    Pin<Box<dyn Stream<Item = Result<SessionStreamEvent, ApiError>> + Send>>;
pub type SessionWatchEventStream =
    Pin<Box<dyn Stream<Item = Result<SessionWatchEvent, ApiError>> + Send>>;

#[derive(Clone)]
pub struct ApiCapabilities {
    pub health: bool,
    pub sessions: bool,
    pub soul: bool,
    pub admin_hooks: bool,
    pub streaming: bool,
}

#[derive(Clone, Debug)]
pub enum ApiError {
    NotFound(String),
    Conflict(String),
    Validation(String),
    Unsupported(String),
    BadRequest(String),
    Internal(String),
}

impl ApiError {
    pub fn into_error_response(self) -> (axum::http::StatusCode, axum::Json<ErrorResponse>) {
        match self {
            Self::NotFound(message) => (
                axum::http::StatusCode::NOT_FOUND,
                axum::Json(ErrorResponse::new("not_found", message)),
            ),
            Self::Conflict(message) => (
                axum::http::StatusCode::CONFLICT,
                axum::Json(ErrorResponse::new("conflict", message)),
            ),
            Self::Validation(message) => (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse::new("validation_error", message)),
            ),
            Self::Unsupported(message) => (
                axum::http::StatusCode::NOT_IMPLEMENTED,
                axum::Json(ErrorResponse::new("not_implemented", message)),
            ),
            Self::BadRequest(message) => (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse::new("bad_request", message)),
            ),
            Self::Internal(message) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse::new("internal_error", message)),
            ),
        }
    }
}

#[async_trait]
pub trait SessionApi: Send + Sync {
    async fn create_session(&self) -> Result<Session, ApiError>;
    async fn get_session(&self, session_id: &str) -> Result<Session, ApiError>;
    async fn list_session_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionMessage>, ApiError>;
    async fn get_session_watch_snapshot(
        &self,
        session_id: &str,
    ) -> Result<SessionWatchSnapshot, ApiError>;
    async fn watch_session(&self, session_id: &str) -> Result<SessionWatchEventStream, ApiError>;
    async fn list_session_effects(&self, session_id: &str) -> Result<Vec<SessionEffect>, ApiError>;
    async fn list_session_compacts(&self, session_id: &str) -> Result<Vec<Compact>, ApiError>;
    async fn get_session_memory(&self, session_id: &str)
        -> Result<SessionMemoryResponse, ApiError>;
    async fn set_session_memory(
        &self,
        session_id: &str,
        text: &str,
    ) -> Result<SessionMemoryResponse, ApiError>;
    async fn send_session(
        &self,
        session_id: &str,
        user_content: String,
    ) -> Result<SessionEventStream, ApiError>;
    async fn fork_session(
        &self,
        session_id: &str,
        fork_point: i64,
        request_id: String,
    ) -> Result<ForkResult, ApiError>;
    async fn compact_session(&self, session_id: &str, summary: &str) -> Result<Compact, ApiError>;
}

#[async_trait]
pub trait SoulApi: Send + Sync {
    async fn get_default_soul(&self) -> Result<Soul, ApiError>;
    async fn set_default_soul_memory(&self, text: &str) -> Result<Soul, ApiError>;
}

#[async_trait]
pub trait AdminApi: Send + Sync {
    async fn reload_hooks_from_source(&self, source: HookSpecSource) -> Result<usize, ApiError>;
}

#[derive(Clone)]
pub struct DistributedSessionApi {
    pub query: Arc<SessionQueryService>,
    pub watch: Arc<SessionWatchService>,
    pub memory: Arc<SessionMemoryService>,
    pub compact: Arc<SessionCompactService>,
    pub send: Arc<SessionSendService>,
    pub fork: Arc<SessionForkService>,
    pub effect_ledger: Arc<dyn EffectLedgerPort>,
}

#[derive(Clone)]
pub struct StandaloneSessionApi {
    pub query: Arc<SessionQueryService>,
    pub watch: Arc<SessionWatchService>,
    pub memory: Arc<SessionMemoryService>,
    pub compact: Arc<SessionCompactService>,
    pub send: Arc<SessionSendService>,
    pub fork: Arc<SessionForkService>,
    pub effect_ledger: Arc<dyn EffectLedgerPort>,
}

#[derive(Clone)]
pub struct DistributedSoulApi {
    pub query: Arc<SessionQueryService>,
    pub memory: Arc<SessionMemoryService>,
}

#[derive(Clone)]
pub struct StandaloneSoulApi {
    pub session_query: Arc<SessionQueryService>,
    pub memory: Arc<SessionMemoryService>,
}

#[derive(Clone)]
pub struct DistributedAdminApi {
    pub send: Arc<SessionSendService>,
}

#[derive(Clone)]
pub struct StandaloneAdminApi {
    pub ebus: Arc<dyn SubscriberSetPort<Arc<dyn HookEvaluator>>>,
}

#[async_trait]
impl SessionApi for DistributedSessionApi {
    async fn create_session(&self) -> Result<Session, ApiError> {
        self.query
            .create_session()
            .await
            .map_err(ApiError::Internal)
    }

    async fn get_session(&self, session_id: &str) -> Result<Session, ApiError> {
        self.query
            .get_session(session_id)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("session not found".to_string()))
    }

    async fn list_session_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionMessage>, ApiError> {
        self.get_session(session_id).await?;
        self.query
            .list_session_messages(session_id)
            .await
            .map_err(ApiError::Internal)
    }

    async fn get_session_watch_snapshot(
        &self,
        session_id: &str,
    ) -> Result<SessionWatchSnapshot, ApiError> {
        self.watch
            .get_session_watch_snapshot(session_id)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("session not found".to_string()))
    }

    async fn watch_session(&self, session_id: &str) -> Result<SessionWatchEventStream, ApiError> {
        let stream = self
            .watch
            .watch_session(session_id)
            .await
            .map_err(map_watch_error)?;
        Ok(Box::pin(
            stream.map(|result| result.map_err(map_watch_error)),
        ))
    }

    async fn list_session_effects(&self, session_id: &str) -> Result<Vec<SessionEffect>, ApiError> {
        self.get_session(session_id).await?;
        self.effect_ledger
            .list_effects(session_id)
            .await
            .map_err(|err| ApiError::Internal(format!("{err:?}")))
    }

    async fn list_session_compacts(&self, session_id: &str) -> Result<Vec<Compact>, ApiError> {
        self.get_session(session_id).await?;
        self.query
            .list_session_compacts(session_id)
            .await
            .map_err(ApiError::Internal)
    }

    async fn get_session_memory(
        &self,
        session_id: &str,
    ) -> Result<SessionMemoryResponse, ApiError> {
        self.memory
            .get_session_memory(session_id)
            .await
            .map_err(ApiError::Internal)?
            .map(SessionMemoryResponse::from)
            .ok_or_else(|| ApiError::NotFound("session not found".to_string()))
    }

    async fn set_session_memory(
        &self,
        session_id: &str,
        text: &str,
    ) -> Result<SessionMemoryResponse, ApiError> {
        self.memory
            .write_session_memory(session_id, text)
            .await
            .map_err(ApiError::Internal)?
            .map(SessionMemoryResponse::from)
            .ok_or_else(|| ApiError::NotFound("session not found".to_string()))
    }

    async fn send_session(
        &self,
        session_id: &str,
        user_content: String,
    ) -> Result<SessionEventStream, ApiError> {
        self.get_session(session_id).await?;
        let stream = self
            .send
            .start(SendSessionCommand {
                session_id: session_id.to_string(),
                user_content,
            })
            .await
            .map_err(map_send_error)?;

        Ok(Box::pin(stream.map(|result| match result {
            Ok(SendSessionEvent::OutputTextDelta(text)) => {
                Ok(SessionStreamEvent::OutputTextDelta(text))
            }
            Ok(SendSessionEvent::Completed) => Ok(SessionStreamEvent::Completed),
            Err(err) => Err(map_send_error(err)),
        })))
    }

    async fn fork_session(
        &self,
        session_id: &str,
        fork_point: i64,
        request_id: String,
    ) -> Result<ForkResult, ApiError> {
        self.get_session(session_id).await?;
        self.fork
            .fork_session(session_id.to_string(), fork_point, request_id)
            .await
            .map_err(map_fork_error)
    }

    async fn compact_session(&self, session_id: &str, summary: &str) -> Result<Compact, ApiError> {
        self.get_session(session_id).await?;
        self.compact
            .compact_session(session_id, summary)
            .await
            .map_err(map_compact_error)
    }
}

#[async_trait]
impl SessionApi for StandaloneSessionApi {
    async fn create_session(&self) -> Result<Session, ApiError> {
        self.query
            .create_session()
            .await
            .map_err(ApiError::Internal)
    }

    async fn get_session(&self, session_id: &str) -> Result<Session, ApiError> {
        self.query
            .get_session(session_id)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("session not found".to_string()))
    }

    async fn list_session_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionMessage>, ApiError> {
        self.get_session(session_id).await?;
        self.query
            .list_session_messages(session_id)
            .await
            .map_err(ApiError::Internal)
    }

    async fn get_session_watch_snapshot(
        &self,
        session_id: &str,
    ) -> Result<SessionWatchSnapshot, ApiError> {
        self.watch
            .get_session_watch_snapshot(session_id)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("session not found".to_string()))
    }

    async fn watch_session(&self, session_id: &str) -> Result<SessionWatchEventStream, ApiError> {
        let stream = self
            .watch
            .watch_session(session_id)
            .await
            .map_err(map_watch_error)?;
        Ok(Box::pin(
            stream.map(|result| result.map_err(map_watch_error)),
        ))
    }

    async fn list_session_effects(&self, session_id: &str) -> Result<Vec<SessionEffect>, ApiError> {
        self.get_session(session_id).await?;
        self.effect_ledger
            .list_effects(session_id)
            .await
            .map_err(|err| ApiError::Internal(format!("{err:?}")))
    }

    async fn list_session_compacts(&self, session_id: &str) -> Result<Vec<Compact>, ApiError> {
        self.get_session(session_id).await?;
        self.query
            .list_session_compacts(session_id)
            .await
            .map_err(ApiError::Internal)
    }

    async fn get_session_memory(
        &self,
        session_id: &str,
    ) -> Result<SessionMemoryResponse, ApiError> {
        self.memory
            .get_session_memory(session_id)
            .await
            .map_err(ApiError::Internal)?
            .map(SessionMemoryResponse::from)
            .ok_or_else(|| ApiError::NotFound("session not found".to_string()))
    }

    async fn set_session_memory(
        &self,
        session_id: &str,
        text: &str,
    ) -> Result<SessionMemoryResponse, ApiError> {
        self.memory
            .write_session_memory(session_id, text)
            .await
            .map_err(ApiError::Internal)?
            .map(SessionMemoryResponse::from)
            .ok_or_else(|| ApiError::NotFound("session not found".to_string()))
    }

    async fn send_session(
        &self,
        session_id: &str,
        user_content: String,
    ) -> Result<SessionEventStream, ApiError> {
        self.get_session(session_id).await?;
        let stream = self
            .send
            .start(SendSessionCommand {
                session_id: session_id.to_string(),
                user_content,
            })
            .await
            .map_err(map_send_error)?;

        Ok(Box::pin(stream.map(|result| match result {
            Ok(SendSessionEvent::OutputTextDelta(text)) => {
                Ok(SessionStreamEvent::OutputTextDelta(text))
            }
            Ok(SendSessionEvent::Completed) => Ok(SessionStreamEvent::Completed),
            Err(err) => Err(map_send_error(err)),
        })))
    }

    async fn fork_session(
        &self,
        session_id: &str,
        fork_point: i64,
        request_id: String,
    ) -> Result<ForkResult, ApiError> {
        self.get_session(session_id).await?;
        self.fork
            .fork_session(session_id.to_string(), fork_point, request_id)
            .await
            .map_err(map_fork_error)
    }

    async fn compact_session(&self, session_id: &str, summary: &str) -> Result<Compact, ApiError> {
        self.get_session(session_id).await?;
        self.compact
            .compact_session(session_id, summary)
            .await
            .map_err(map_compact_error)
    }
}

#[async_trait]
impl SoulApi for DistributedSoulApi {
    async fn get_default_soul(&self) -> Result<Soul, ApiError> {
        self.query
            .get_default_soul()
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("soul not found".to_string()))
    }

    async fn set_default_soul_memory(&self, text: &str) -> Result<Soul, ApiError> {
        self.memory
            .write_soul_memory("soul_default", text)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("soul not found".to_string()))
    }
}

#[async_trait]
impl SoulApi for StandaloneSoulApi {
    async fn get_default_soul(&self) -> Result<Soul, ApiError> {
        self.session_query
            .get_default_soul()
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("soul not found".to_string()))
    }

    async fn set_default_soul_memory(&self, text: &str) -> Result<Soul, ApiError> {
        self.memory
            .write_soul_memory("soul_default", text)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("soul not found".to_string()))
    }
}

#[async_trait]
impl AdminApi for DistributedAdminApi {
    async fn reload_hooks_from_source(&self, source: HookSpecSource) -> Result<usize, ApiError> {
        let specs = santi_runtime::hooks::load_hook_specs(&source)
            .await
            .map_err(ApiError::BadRequest)?;
        Ok(self.send.replace_hooks(&specs))
    }
}

#[async_trait]
impl AdminApi for StandaloneAdminApi {
    async fn reload_hooks_from_source(&self, source: HookSpecSource) -> Result<usize, ApiError> {
        let specs = load_hook_specs(&source)
            .await
            .map_err(ApiError::BadRequest)?;
        let subscribers = compile_hook_specs(&specs);
        let count = subscribers.len();
        self.ebus.replace_all(subscribers);
        Ok(count)
    }
}

pub fn default_capabilities(mode: &Mode) -> ApiCapabilities {
    match mode {
        Mode::Distributed => ApiCapabilities {
            health: true,
            sessions: true,
            soul: true,
            admin_hooks: true,
            streaming: true,
        },
        Mode::Standalone => ApiCapabilities {
            health: true,
            sessions: true,
            soul: true,
            admin_hooks: true,
            streaming: false,
        },
    }
}

fn map_send_error(err: SendSessionError) -> ApiError {
    match err {
        SendSessionError::Busy => {
            ApiError::Conflict("session send already in progress".to_string())
        }
        SendSessionError::NotFound => ApiError::NotFound("session not found".to_string()),
        SendSessionError::Internal(message) => ApiError::Internal(message),
    }
}

fn map_fork_error(err: ForkError) -> ApiError {
    match err {
        ForkError::Busy => ApiError::Conflict("session fork already in progress".to_string()),
        ForkError::ParentNotFound => ApiError::NotFound("parent session not found".to_string()),
        ForkError::InvalidForkPoint(message) => ApiError::Validation(message),
        ForkError::Internal(message) => ApiError::Internal(message),
    }
}

fn map_compact_error(err: CompactSessionError) -> ApiError {
    match err {
        CompactSessionError::Busy => {
            ApiError::Conflict("session compact already in progress".to_string())
        }
        CompactSessionError::NotFound => ApiError::NotFound("session not found".to_string()),
        CompactSessionError::Invalid(message) => ApiError::Validation(message),
        CompactSessionError::Internal(message) => ApiError::Internal(message),
    }
}

fn map_watch_error(err: SessionWatchError) -> ApiError {
    match err {
        SessionWatchError::NotFound => ApiError::NotFound("session not found".to_string()),
        SessionWatchError::Internal(message) => ApiError::Internal(message),
    }
}
