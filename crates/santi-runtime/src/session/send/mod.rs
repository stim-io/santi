use std::{pin::Pin, sync::Arc};

use async_stream::try_stream;
use futures::Stream;
use santi_core::{
    error::{Error, LockError},
    hook::HookSpec,
    model::runtime::Turn,
    port::{
        compact_runtime::CompactRuntimePort,
        ebus::SubscriberSetPort,
        effect_ledger::EffectLedgerPort,
        lock::{Lock, LockGuard},
        provider::Provider,
        session_ledger::SessionLedgerPort,
        soul_runtime::SoulRuntimePort,
    },
};
use tokio::sync::mpsc;

use crate::{
    hooks::{compile_hook_specs, HookEvaluator},
    runtime::tools::{ToolExecutor, ToolExecutorConfig},
    session::{
        compact::SessionCompactService,
        effect::SessionEffectService,
        fork::SessionForkService,
        hook_runtime::HookRuntime,
        memory::SessionMemoryService,
        watch::{
            SessionWatchActivityChanged, SessionWatchActivityKind, SessionWatchActivityState,
            SessionWatchEvent, SessionWatchHub, SessionWatchState, SessionWatchStateChanged,
        },
    },
};

mod assembly;
mod turn;

use self::turn::{run_turn_startup, run_turn_worker};

#[derive(Clone)]
pub struct SessionSendService {
    model: String,
    default_soul_id: String,
    lock: Arc<dyn Lock>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    provider: Arc<dyn Provider>,
    tools: Arc<ToolExecutor>,
    hooks: Arc<HookRuntime>,
    watch: Arc<SessionWatchHub>,
}

#[derive(Clone)]
pub struct SessionTurnService {
    model: String,
    default_soul_id: String,
    lock: Arc<dyn Lock>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    provider: Arc<dyn Provider>,
    tools: Arc<ToolExecutor>,
    watch: Arc<SessionWatchHub>,
}

pub struct SendSessionCommand {
    pub session_id: String,
    pub user_content: String,
}

#[derive(Clone, Debug)]
pub enum SendSessionError {
    Busy,
    NotFound,
    Internal(String),
}

pub enum SendSessionEvent {
    OutputTextDelta(String),
    Completed,
}

pub type SendSessionStream =
    Pin<Box<dyn Stream<Item = Result<SendSessionEvent, SendSessionError>> + Send>>;

#[derive(Clone)]
pub enum TurnInput {
    UserText { text: String },
    SystemSeed { actor_id: String, text: String },
}

#[derive(Clone)]
pub struct TurnExecutionRequest {
    pub session_id: String,
    pub input: TurnInput,
    pub emit_events: bool,
    pub run_hooks: bool,
}

impl SessionSendService {
    pub fn new(
        model: String,
        default_soul_id: String,
        lock: Arc<dyn Lock>,
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_runtime: Arc<dyn SoulRuntimePort>,
        compact_runtime: Arc<dyn CompactRuntimePort>,
        effect_ledger: Arc<dyn EffectLedgerPort>,
        fork_service: Arc<SessionForkService>,
        provider: Arc<dyn Provider>,
        session_memory: SessionMemoryService,
        tool_config: ToolExecutorConfig,
        ebus: Arc<dyn SubscriberSetPort<Arc<dyn HookEvaluator>>>,
        watch: Arc<SessionWatchHub>,
    ) -> Self {
        let tools = Arc::new(ToolExecutor::new(session_memory, tool_config));
        let compact_service = Arc::new(SessionCompactService::new(
            lock.clone(),
            session_ledger.clone(),
            soul_runtime.clone(),
            compact_runtime,
            default_soul_id.clone(),
            watch.clone(),
        ));
        let turn_service = Arc::new(SessionTurnService {
            model: model.clone(),
            default_soul_id: default_soul_id.clone(),
            lock: lock.clone(),
            session_ledger: session_ledger.clone(),
            soul_runtime: soul_runtime.clone(),
            provider: provider.clone(),
            tools: tools.clone(),
            watch: watch.clone(),
        });
        let effect_service = Arc::new(SessionEffectService::new(
            effect_ledger,
            fork_service,
            turn_service.clone(),
            watch.clone(),
        ));
        Self {
            model,
            default_soul_id,
            lock,
            session_ledger,
            soul_runtime,
            provider,
            tools,
            hooks: Arc::new(HookRuntime::new(ebus, compact_service, effect_service)),
            watch,
        }
    }

    pub fn replace_hooks(&self, specs: &[HookSpec]) -> usize {
        let subscribers = compile_hook_specs(specs);
        let count = subscribers.len();
        self.hooks.replace_subscribers(subscribers);
        count
    }

    pub async fn start(
        &self,
        cmd: SendSessionCommand,
    ) -> Result<SendSessionStream, SendSessionError> {
        let guard = self
            .lock
            .acquire(&format!("lock:session_send:{}", cmd.session_id))
            .await
            .map_err(map_lock_error)?;

        let (tx, mut rx) = mpsc::unbounded_channel::<Result<SendSessionEvent, SendSessionError>>();
        let error_tx = tx.clone();

        let turn_service = SessionTurnService {
            model: self.model.clone(),
            default_soul_id: self.default_soul_id.clone(),
            lock: self.lock.clone(),
            session_ledger: self.session_ledger.clone(),
            soul_runtime: self.soul_runtime.clone(),
            provider: self.provider.clone(),
            tools: self.tools.clone(),
            watch: self.watch.clone(),
        };
        let hooks = self.hooks.clone();
        let session_id = cmd.session_id.clone();
        let request = TurnExecutionRequest {
            session_id: session_id.clone(),
            input: TurnInput::UserText {
                text: cmd.user_content,
            },
            emit_events: true,
            run_hooks: true,
        };
        let watch = self.watch.clone();

        tokio::spawn(async move {
            let result = turn_service
                .execute_with_guard(request, Some(hooks), Some(tx), guard)
                .await;

            if let Err(err) = result {
                watch.publish(
                    &session_id,
                    SessionWatchEvent::ActivityChanged(SessionWatchActivityChanged {
                        session_id: session_id.clone(),
                        activity: SessionWatchActivityKind::Send,
                        state: SessionWatchActivityState::Failed,
                        label: Some(render_send_error(&err)),
                    }),
                );
                watch.publish(
                    &session_id,
                    SessionWatchEvent::StateChanged(SessionWatchStateChanged {
                        session_id: session_id.clone(),
                        state: SessionWatchState::Failed,
                    }),
                );
                let _ = error_tx.send(Err(err));
            }
        });

        Ok(Box::pin(try_stream! {
            while let Some(event) = rx.recv().await {
                match event {
                    Ok(event) => yield event,
                    Err(err) => Err(err)?,
                }
            }
        }))
    }
}

impl SessionTurnService {
    pub async fn execute(
        &self,
        request: TurnExecutionRequest,
        hooks: Option<Arc<HookRuntime>>,
        tx: Option<mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>>,
    ) -> Result<Turn, SendSessionError> {
        let guard = self
            .lock
            .acquire(&format!("lock:session_send:{}", request.session_id))
            .await
            .map_err(map_lock_error)?;

        self.execute_with_guard(request, hooks, tx, guard).await
    }

    async fn execute_with_guard(
        &self,
        request: TurnExecutionRequest,
        hooks: Option<Arc<HookRuntime>>,
        tx: Option<mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>>,
        guard: Box<dyn LockGuard + Send>,
    ) -> Result<Turn, SendSessionError> {
        let startup = match run_turn_startup(
            &self.default_soul_id,
            &request,
            self.session_ledger.clone(),
            self.soul_runtime.clone(),
            self.tools.clone(),
        )
        .await
        {
            Ok(startup) => startup,
            Err(err) => return turn::release_guard_on_error(guard, err).await,
        };

        turn::publish_turn_started(&self.watch, &request.session_id);

        run_turn_worker(
            self.default_soul_id.clone(),
            request,
            self.model.clone(),
            self.provider.clone(),
            self.session_ledger.clone(),
            self.soul_runtime.clone(),
            self.tools.clone(),
            self.watch.clone(),
            hooks,
            startup,
            tx,
            guard,
        )
        .await
    }
}

fn render_send_error(err: &SendSessionError) -> String {
    match err {
        SendSessionError::Busy => "session send already in progress".to_string(),
        SendSessionError::NotFound => "session not found".to_string(),
        SendSessionError::Internal(message) => message.clone(),
    }
}

fn map_core_error(err: Error) -> SendSessionError {
    match err {
        Error::NotFound { resource } if resource == "session" => SendSessionError::NotFound,
        Error::Busy { .. } => SendSessionError::Busy,
        Error::NotFound { resource } => SendSessionError::Internal(format!("{resource} not found")),
        Error::InvalidInput { message }
        | Error::Upstream { message }
        | Error::Internal { message } => SendSessionError::Internal(message),
    }
}

fn map_lock_error(err: LockError) -> SendSessionError {
    match err {
        LockError::Busy => SendSessionError::Busy,
        LockError::Lost => SendSessionError::Internal("session send lock lost".to_string()),
        LockError::Backend { message } => SendSessionError::Internal(message),
    }
}
