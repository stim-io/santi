use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::StreamExt;
use santi_core::{
    hook::{HookSpec, HookSpecSource},
    model::{message::MessagePart, runtime::Compact, session::SessionMessage, soul::Soul},
    port::{
        ebus::SubscriberSetPort, effect_ledger::EffectLedgerPort,
        lock::Lock, provider::Provider, session_ledger::SessionLedgerPort, soul::SoulPort,
        soul_runtime::SoulRuntimePort,
    },
};
use santi_db::{
    adapter::{
        effect_ledger::DbEffectLedger, session_ledger::DbSessionLedger, soul::DbSoul,
        soul_runtime::DbSoulRuntime,
    },
    db::init_postgres,
};
use santi_lock::{RedisLockClient, RedisLockConfig};
use santi_ebus::InMemorySubscriberSet;
use santi_provider::openai_compatible::OpenAiCompatibleProvider;
use santi_runtime::{
    hooks::{compile_hook_specs, load_hook_specs, HookEvaluator},
    runtime::tools::ToolExecutorConfig,
    session::{
        compact::{CompactSessionError, SessionCompactService},
        memory::SessionMemoryService,
        query::SessionQueryService,
        send::{SendSessionCommand, SendSessionError, SendSessionEvent, SessionSendService},
    },
};
use tokio::time::sleep;

use crate::{
    backend::{
        BackendError, CliBackend, CliCompact, CliHealth, CliHookReload, CliMemoryRecord,
        CliMessage, CliSession, CliSoul, SendEvent, SendStream,
    },
    config::Config,
};

#[derive(Clone)]
pub struct LocalBackend {
    default_soul_id: String,
    session_memory: Arc<SessionMemoryService>,
    session_compact: Arc<SessionCompactService>,
    session_query: Arc<SessionQueryService>,
    session_send: Arc<SessionSendService>,
}

impl LocalBackend {
    pub async fn new(config: Config) -> Result<Self, String> {
        let openai_api_key = config
            .openai_api_key
            .clone()
            .ok_or_else(|| "missing OPENAI API key for local backend".to_string())?;
        let provider =
            OpenAiCompatibleProvider::new(openai_api_key, config.openai_base_url.clone());
        let pool = init_postgres(&config.database_url)
            .await
            .map_err(|err| format!("init postgres failed: {err}"))?;
        let lock_client = Arc::new(
            RedisLockClient::new(
                &config.redis_url,
                RedisLockConfig {
                    ttl: Duration::from_secs(120),
                    renew_interval: Duration::from_secs(40),
                    acquire_timeout: Duration::from_millis(500),
                    key_prefix: None,
                },
            )
            .await
            .map_err(|err| format!("init redis lock failed: {err}"))?,
        );

        let default_soul_id = "soul_default".to_string();
        let provider: Arc<dyn Provider> = Arc::new(provider);
        let lock: Arc<dyn Lock> = lock_client;
        let session_ledger: Arc<dyn SessionLedgerPort> =
            Arc::new(DbSessionLedger::new(pool.clone()));
        let effect_ledger: Arc<dyn EffectLedgerPort> = Arc::new(DbEffectLedger::new(pool.clone()));
        let soul_port: Arc<dyn SoulPort> = Arc::new(DbSoul::new(pool.clone()));
        let soul_runtime: Arc<dyn SoulRuntimePort> = Arc::new(DbSoulRuntime::new(pool));

        let session_memory = Arc::new(SessionMemoryService::new(
            soul_runtime.clone(),
            soul_port.clone(),
            default_soul_id.clone(),
        ));
        let session_query = Arc::new(SessionQueryService::new(
            session_ledger.clone(),
            soul_port,
            default_soul_id.clone(),
        ));
        let session_compact = Arc::new(SessionCompactService::new(
            lock.clone(),
            session_ledger.clone(),
            soul_runtime.clone(),
            default_soul_id.clone(),
        ));
        let hook_specs = load_startup_hook_specs(config.hook_source.as_ref()).await?;
        let ebus: Arc<dyn SubscriberSetPort<Arc<dyn HookEvaluator>>> =
            Arc::new(InMemorySubscriberSet::<Arc<dyn HookEvaluator>>::new());
        ebus.replace_all(compile_hook_specs(&hook_specs));
        let session_fork = Arc::new(santi_runtime::session::fork::SessionForkService::new(
            lock.clone(),
            soul_runtime.clone(),
        ));
        let session_send = Arc::new(SessionSendService::new(
            config.openai_model.clone(),
            default_soul_id.clone(),
            lock,
            session_ledger,
            soul_runtime,
            effect_ledger,
            session_fork,
            provider,
            session_memory.as_ref().clone(),
            ToolExecutorConfig {
                runtime_root: config.runtime_root,
                execution_root: config.execution_root,
            },
            ebus,
        ));

        Ok(Self {
            default_soul_id,
            session_memory,
            session_compact,
            session_query,
            session_send,
        })
    }

    async fn reload_hook_source(&self, source: &HookSpecSource) -> Result<usize, String> {
        let specs = load_hook_specs(source).await?;
        Ok(self.session_send.replace_hooks(&specs))
    }
}

async fn load_startup_hook_specs(
    source: Option<&HookSpecSource>,
) -> Result<Vec<HookSpec>, String> {
    match source {
        Some(source) => load_hook_specs(source).await,
        None => Ok(Vec::new()),
    }
}

#[async_trait]
impl CliBackend for LocalBackend {
    async fn health(&self) -> Result<CliHealth, BackendError> {
        Ok(CliHealth {
            status: "ok".to_string(),
        })
    }

    async fn create_session(&self) -> Result<CliSession, BackendError> {
        let session = self
            .session_query
            .create_session()
            .await
            .map_err(BackendError::Other)?;
        Ok(CliSession {
            id: session.id,
            parent_session_id: session.parent_session_id,
            fork_point: session.fork_point,
            created_at: session.created_at,
        })
    }

    async fn get_session(&self, session_id: String) -> Result<CliSession, BackendError> {
        let session = self
            .session_query
            .get_session(&session_id)
            .await
            .map_err(BackendError::Other)?
            .ok_or(BackendError::NotFound)?;
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
        let stream = loop {
            match self
                .session_send
                .start(SendSessionCommand {
                    session_id: session_id.clone(),
                    user_content: content.clone(),
                })
                .await
            {
                Ok(stream) => break stream,
                Err(SendSessionError::Busy) if wait => sleep(Duration::from_millis(350)).await,
                Err(err) => return Err(map_send_error(err)),
            }
        };

        Ok(Box::pin(stream.map(|event| {
            event.map(map_send_event).map_err(map_send_error)
        })))
    }

    async fn list_messages(&self, session_id: String) -> Result<Vec<CliMessage>, BackendError> {
        self.session_query
            .list_session_messages(&session_id)
            .await
            .map_err(BackendError::Other)
            .map(|messages| messages.into_iter().map(map_session_message).collect())
    }

    async fn get_default_soul(&self) -> Result<CliSoul, BackendError> {
        let soul = self
            .session_query
            .get_default_soul()
            .await
            .map_err(BackendError::Other)?
            .ok_or(BackendError::NotFound)?;
        Ok(map_soul(soul))
    }

    async fn set_default_soul_memory(&self, text: String) -> Result<CliMemoryRecord, BackendError> {
        let soul = self
            .session_memory
            .write_soul_memory(&self.default_soul_id, &text)
            .await
            .map_err(BackendError::Other)?
            .ok_or(BackendError::NotFound)?;
        Ok(CliMemoryRecord {
            id: soul.id,
            memory: soul.memory,
            updated_at: soul.updated_at,
        })
    }

    async fn set_session_memory(
        &self,
        session_id: String,
        text: String,
    ) -> Result<CliMemoryRecord, BackendError> {
        let soul_session = self
            .session_memory
            .write_session_memory(&session_id, &text)
            .await
            .map_err(BackendError::Other)?
            .ok_or(BackendError::NotFound)?;
        Ok(CliMemoryRecord {
            id: soul_session.id,
            memory: soul_session.session_memory,
            updated_at: soul_session.updated_at,
        })
    }

    async fn compact_session(
        &self,
        session_id: String,
        summary: String,
    ) -> Result<CliCompact, BackendError> {
        self.session_compact
            .compact_session(&session_id, &summary)
            .await
            .map(map_compact)
            .map_err(map_compact_error)
    }

    async fn reload_hooks(&self, source: HookSpecSource) -> Result<CliHookReload, BackendError> {
        Ok(CliHookReload {
            hook_count: self
                .reload_hook_source(&source)
                .await
                .map_err(BackendError::Other)?,
        })
    }
}

fn map_send_event(event: SendSessionEvent) -> SendEvent {
    match event {
        SendSessionEvent::OutputTextDelta(delta) => SendEvent::OutputTextDelta(delta),
        SendSessionEvent::Completed => SendEvent::Completed,
    }
}

fn map_send_error(err: SendSessionError) -> BackendError {
    match err {
        SendSessionError::Busy => BackendError::Busy,
        SendSessionError::NotFound => BackendError::NotFound,
        SendSessionError::Internal(message) => BackendError::Other(message),
    }
}

fn map_compact_error(err: CompactSessionError) -> BackendError {
    match err {
        CompactSessionError::Busy => BackendError::Busy,
        CompactSessionError::NotFound => BackendError::NotFound,
        CompactSessionError::Invalid(message) | CompactSessionError::Internal(message) => {
            BackendError::Other(message)
        }
    }
}

fn map_session_message(message: SessionMessage) -> CliMessage {
    let content_text = first_text_part(&message);

    CliMessage {
        id: message.message.id,
        actor_type: serde_json::to_string(&message.message.actor_type)
            .unwrap_or_else(|_| "\"system\"".to_string())
            .trim_matches('"')
            .to_string(),
        actor_id: message.message.actor_id,
        session_seq: message.relation.session_seq,
        content_text,
        state: serde_json::to_string(&message.message.state)
            .unwrap_or_else(|_| "\"fixed\"".to_string())
            .trim_matches('"')
            .to_string(),
        created_at: message.message.created_at,
    }
}

fn first_text_part(message: &SessionMessage) -> String {
    message
        .message
        .content
        .parts
        .iter()
        .find_map(|part| match part {
            MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn map_soul(soul: Soul) -> CliSoul {
    CliSoul {
        id: soul.id,
        memory: soul.memory,
        created_at: Some(soul.created_at),
        updated_at: soul.updated_at,
    }
}

fn map_compact(compact: Compact) -> CliCompact {
    CliCompact {
        id: compact.id,
        turn_id: compact.turn_id,
        summary: compact.summary,
        start_session_seq: compact.start_session_seq,
        end_session_seq: compact.end_session_seq,
        created_at: compact.created_at,
    }
}
