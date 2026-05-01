use std::{pin::Pin, sync::Arc};

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use santi_core::{
    model::{effect::SessionEffect, runtime::Compact, session::Session, session::SessionMessage},
    port::effect_ledger::EffectLedgerPort,
};
use santi_runtime::session::{
    compact::{CompactSessionError, SessionCompactService},
    fork::{ForkError, ForkResult, SessionForkService},
    memory::SessionMemoryService,
    query::SessionQueryService,
    send::{
        ReplyToSessionMessageCommand, SendSessionCommand, SendSessionError, SendSessionEvent,
        SessionSendService,
    },
    watch::{SessionWatchError, SessionWatchEvent, SessionWatchService, SessionWatchSnapshot},
};

use crate::schema::{session::SessionMemoryResponse, session_events::SessionStreamEvent};

use super::error::ApiError;

pub type SessionEventStream =
    Pin<Box<dyn Stream<Item = Result<SessionStreamEvent, ApiError>> + Send>>;
pub type SessionWatchEventStream =
    Pin<Box<dyn Stream<Item = Result<SessionWatchEvent, ApiError>> + Send>>;

#[async_trait]
pub trait SessionApi: Send + Sync {
    async fn create_session(&self) -> Result<Session, ApiError>;
    async fn create_session_with_id(&self, session_id: &str) -> Result<Session, ApiError>;
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
    async fn reply_to_session_message(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<SessionEventStream, ApiError>;
    async fn fork_session(
        &self,
        session_id: &str,
        fork_point: i64,
        request_id: String,
    ) -> Result<ForkResult, ApiError>;
    async fn compact_session(&self, session_id: &str, summary: &str) -> Result<Compact, ApiError>;
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

trait SessionApiDeps {
    fn query(&self) -> &Arc<SessionQueryService>;
    fn watch(&self) -> &Arc<SessionWatchService>;
    fn memory(&self) -> &Arc<SessionMemoryService>;
    fn compact(&self) -> &Arc<SessionCompactService>;
    fn send(&self) -> &Arc<SessionSendService>;
    fn fork(&self) -> &Arc<SessionForkService>;
    fn effect_ledger(&self) -> &Arc<dyn EffectLedgerPort>;
}

impl SessionApiDeps for DistributedSessionApi {
    fn query(&self) -> &Arc<SessionQueryService> {
        &self.query
    }

    fn watch(&self) -> &Arc<SessionWatchService> {
        &self.watch
    }

    fn memory(&self) -> &Arc<SessionMemoryService> {
        &self.memory
    }

    fn compact(&self) -> &Arc<SessionCompactService> {
        &self.compact
    }

    fn send(&self) -> &Arc<SessionSendService> {
        &self.send
    }

    fn fork(&self) -> &Arc<SessionForkService> {
        &self.fork
    }

    fn effect_ledger(&self) -> &Arc<dyn EffectLedgerPort> {
        &self.effect_ledger
    }
}

impl SessionApiDeps for StandaloneSessionApi {
    fn query(&self) -> &Arc<SessionQueryService> {
        &self.query
    }

    fn watch(&self) -> &Arc<SessionWatchService> {
        &self.watch
    }

    fn memory(&self) -> &Arc<SessionMemoryService> {
        &self.memory
    }

    fn compact(&self) -> &Arc<SessionCompactService> {
        &self.compact
    }

    fn send(&self) -> &Arc<SessionSendService> {
        &self.send
    }

    fn fork(&self) -> &Arc<SessionForkService> {
        &self.fork
    }

    fn effect_ledger(&self) -> &Arc<dyn EffectLedgerPort> {
        &self.effect_ledger
    }
}

#[async_trait]
impl<T> SessionApi for T
where
    T: SessionApiDeps + Send + Sync,
{
    async fn create_session(&self) -> Result<Session, ApiError> {
        self.query()
            .create_session()
            .await
            .map_err(ApiError::Internal)
    }

    async fn create_session_with_id(&self, session_id: &str) -> Result<Session, ApiError> {
        self.query()
            .create_session_with_id(session_id)
            .await
            .map_err(ApiError::Internal)
    }

    async fn get_session(&self, session_id: &str) -> Result<Session, ApiError> {
        self.query()
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
        self.query()
            .list_session_messages(session_id)
            .await
            .map_err(ApiError::Internal)
    }

    async fn get_session_watch_snapshot(
        &self,
        session_id: &str,
    ) -> Result<SessionWatchSnapshot, ApiError> {
        self.watch()
            .get_session_watch_snapshot(session_id)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::NotFound("session not found".to_string()))
    }

    async fn watch_session(&self, session_id: &str) -> Result<SessionWatchEventStream, ApiError> {
        let stream = self
            .watch()
            .watch_session(session_id)
            .await
            .map_err(map_watch_error)?;
        Ok(Box::pin(
            stream.map(|result| result.map_err(map_watch_error)),
        ))
    }

    async fn list_session_effects(&self, session_id: &str) -> Result<Vec<SessionEffect>, ApiError> {
        self.get_session(session_id).await?;
        self.effect_ledger()
            .list_effects(session_id)
            .await
            .map_err(|err| ApiError::Internal(format!("{err:?}")))
    }

    async fn list_session_compacts(&self, session_id: &str) -> Result<Vec<Compact>, ApiError> {
        self.get_session(session_id).await?;
        self.query()
            .list_session_compacts(session_id)
            .await
            .map_err(ApiError::Internal)
    }

    async fn get_session_memory(
        &self,
        session_id: &str,
    ) -> Result<SessionMemoryResponse, ApiError> {
        self.memory()
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
        self.memory()
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
            .send()
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

    async fn reply_to_session_message(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<SessionEventStream, ApiError> {
        self.get_session(session_id).await?;
        let stream = self
            .send()
            .reply_to_session_message(ReplyToSessionMessageCommand {
                session_id: session_id.to_string(),
                message_id: message_id.to_string(),
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
        self.fork()
            .fork_session(session_id.to_string(), fork_point, request_id)
            .await
            .map_err(map_fork_error)
    }

    async fn compact_session(&self, session_id: &str, summary: &str) -> Result<Compact, ApiError> {
        self.get_session(session_id).await?;
        self.compact()
            .compact_session(session_id, summary)
            .await
            .map_err(map_compact_error)
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
