use std::{pin::Pin, sync::Arc};

use async_stream::try_stream;
use futures::{Stream, StreamExt};
use santi_core::{
    error::{Error, LockError},
    hook::HookSpec,
    model::{
        message::{ActorType, MessageContent, MessagePart, MessageState},
        runtime::{AssemblyItem, AssemblyTarget, ProviderState, Turn, TurnTriggerType},
        session::SessionMessage,
    },
    port::{
        ebus::SubscriberSetPort,
        effect_ledger::EffectLedgerPort,
        lock::{Lock, LockGuard},
        provider::{Provider, ProviderEvent, ProviderFunctionCall, ProviderRequest},
        session_ledger::{AppendSessionMessage, SessionLedgerPort},
        soul_runtime::{
            AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn, FailTurn,
            SoulRuntimePort, StartTurn,
        },
    },
    provider::ProviderInputMessage,
    service::session::kernel::{
        runtime_prompt::{build_runtime_prompt, RuntimePromptSource},
        transcript,
    },
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    hooks::{compile_hook_specs, HookEvaluator, TurnCompletedHookInput},
    runtime::{
        context::ToolRuntimeContext,
        prompt::render_runtime_instructions,
        tools::{ToolExecutor, ToolExecutorConfig},
    },
    session::{
        compact::SessionCompactService, effect::SessionEffectService, fork::SessionForkService,
        hook_runtime::HookRuntime, memory::SessionMemoryService,
    },
};

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

struct TurnRunOutput {
    turn: Turn,
    session: santi_core::model::session::Session,
    soul_session_id: String,
    assistant_message: SessionMessage,
}

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

#[derive(Clone)]
struct StartupContext {
    session: santi_core::model::session::Session,
    provider_input: Vec<ProviderInputMessage>,
    instructions: Option<String>,
    soul_session_id: String,
    trigger_type: TurnTriggerType,
    input_through_session_seq: i64,
    trigger_message_id: String,
    runtime_context: ToolRuntimeContext,
}

impl SessionSendService {
    pub fn new(
        model: String,
        default_soul_id: String,
        lock: Arc<dyn Lock>,
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_runtime: Arc<dyn SoulRuntimePort>,
        effect_ledger: Arc<dyn EffectLedgerPort>,
        fork_service: Arc<SessionForkService>,
        provider: Arc<dyn Provider>,
        session_memory: SessionMemoryService,
        tool_config: ToolExecutorConfig,
        ebus: Arc<dyn SubscriberSetPort<Arc<dyn HookEvaluator>>>,
    ) -> Self {
        let tools = Arc::new(ToolExecutor::new(session_memory, tool_config));
        let compact_service = Arc::new(SessionCompactService::new(
            lock.clone(),
            session_ledger.clone(),
            soul_runtime.clone(),
            default_soul_id.clone(),
        ));
        let turn_service = Arc::new(SessionTurnService {
            model: model.clone(),
            default_soul_id: default_soul_id.clone(),
            lock: lock.clone(),
            session_ledger: session_ledger.clone(),
            soul_runtime: soul_runtime.clone(),
            provider: provider.clone(),
            tools: tools.clone(),
        });
        let effect_service = Arc::new(SessionEffectService::new(
            effect_ledger,
            fork_service,
            turn_service.clone(),
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
        };
        let hooks = self.hooks.clone();
        let request = TurnExecutionRequest {
            session_id: cmd.session_id,
            input: TurnInput::UserText {
                text: cmd.user_content,
            },
            emit_events: true,
            run_hooks: true,
        };

        tokio::spawn(async move {
            let result = turn_service.execute(request, Some(hooks), Some(tx)).await;

            if let Err(err) = result {
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

        let startup = run_turn_startup(
            &self.default_soul_id,
            &request,
            self.session_ledger.clone(),
            self.soul_runtime.clone(),
            self.tools.clone(),
        )
        .await;

        let (startup, guard) = match startup {
            Ok(startup) => (startup, guard),
            Err(err) => {
                let release_result = guard.release().await.map_err(map_lock_error);
                return Err(match release_result {
                    Ok(()) => err,
                    Err(release_err) => release_err,
                });
            }
        };

        run_turn_worker(
            self.default_soul_id.clone(),
            request,
            self.model.clone(),
            self.provider.clone(),
            self.session_ledger.clone(),
            self.soul_runtime.clone(),
            self.tools.clone(),
            hooks,
            startup,
            tx,
            guard,
        )
        .await
    }
}

async fn run_turn_startup(
    default_soul_id: &str,
    request: &TurnExecutionRequest,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    tools: Arc<ToolExecutor>,
) -> Result<StartupContext, SendSessionError> {
    let session = session_ledger
        .get_session(&request.session_id)
        .await
        .map_err(map_core_error)?
        .ok_or(SendSessionError::NotFound)?;

    let soul_session = soul_runtime
        .get_or_create_soul_session(default_soul_id, &request.session_id)
        .await
        .map_err(map_core_error)?;

    let turn_context = soul_runtime
        .load_turn_context(default_soul_id, &request.session_id)
        .await
        .map_err(map_core_error)?
        .ok_or(SendSessionError::NotFound)?;

    let trigger_message = session_ledger
        .append_message(AppendSessionMessage {
            session_id: session.id.clone(),
            message_id: format!("msg_{}", Uuid::new_v4().simple()),
            actor_type: match &request.input {
                TurnInput::UserText { .. } => ActorType::Account,
                TurnInput::SystemSeed { .. } => ActorType::System,
            },
            actor_id: match &request.input {
                TurnInput::UserText { .. } => "account_local".to_string(),
                TurnInput::SystemSeed { actor_id, .. } => actor_id.clone(),
            },
            content: text_content(match &request.input {
                TurnInput::UserText { text } | TurnInput::SystemSeed { text, .. } => text,
            }),
            state: MessageState::Fixed,
        })
        .await
        .map_err(map_core_error)?;

    soul_runtime
        .append_message_ref(AppendMessageRef {
            soul_session_id: soul_session.id.clone(),
            message_id: trigger_message.message.id.clone(),
        })
        .await
        .map_err(map_core_error)?;

    let assembly = soul_runtime
        .list_assembly_items(&soul_session.id, None)
        .await
        .map_err(map_core_error)?;

    let provider_input = assembly_to_provider_input(&assembly);
    let runtime_context = tools.build_context(&request.session_id, &turn_context.soul.id);
    let core_prompt = build_runtime_prompt(RuntimePromptSource {
        session_id: Some(request.session_id.clone()),
        soul_id: Some(turn_context.soul.id.clone()),
        soul_memory: Some(turn_context.soul.memory.clone()),
        session_memory: Some(turn_context.soul_session.session_memory.clone()),
        request_instructions: None,
    });
    let instructions = render_runtime_instructions(&core_prompt, &runtime_context, &tools);

    Ok(StartupContext {
        session,
        provider_input,
        instructions,
        soul_session_id: soul_session.id,
        trigger_type: match &request.input {
            TurnInput::UserText { .. } => TurnTriggerType::SessionSend,
            TurnInput::SystemSeed { .. } => TurnTriggerType::System,
        },
        input_through_session_seq: trigger_message.relation.session_seq,
        trigger_message_id: trigger_message.message.id,
        runtime_context,
    })
}

async fn run_turn_worker(
    default_soul_id: String,
    request: TurnExecutionRequest,
    model: String,
    provider: Arc<dyn Provider>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    tools: Arc<ToolExecutor>,
    hooks: Option<Arc<HookRuntime>>,
    startup: StartupContext,
    tx: Option<mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>>,
    guard: Box<dyn LockGuard + Send>,
) -> Result<Turn, SendSessionError> {
    let turn_id = format!("turn_{}", Uuid::new_v4().simple());
    let started_turn = soul_runtime
        .start_turn(StartTurn {
            turn_id: turn_id.clone(),
            soul_session_id: startup.soul_session_id.clone(),
            trigger_type: startup.trigger_type.clone(),
            trigger_ref: Some(startup.trigger_message_id.clone()),
            input_through_session_seq: startup.input_through_session_seq,
        })
        .await
        .map_err(map_core_error)?;

    let run_result = run_turn_body(
        default_soul_id,
        request.clone(),
        model,
        provider,
        session_ledger,
        soul_runtime.clone(),
        tools,
        startup,
        started_turn,
        tx.clone(),
    )
    .await;

    if let Err(err) = &run_result {
        let _ = soul_runtime
            .fail_turn(FailTurn {
                turn_id,
                error_text: render_send_error(err),
            })
            .await;
    }

    let release_result = guard.release().await.map_err(map_lock_error);
    match (run_result, release_result) {
        (Err(err), _) => Err(err),
        (Ok(_turn), Err(err)) => Err(err),
        (Ok(output), Ok(())) => {
            if request.run_hooks {
                if let Some(hooks) = hooks {
                    if let Some(soul_session) = soul_runtime
                        .get_soul_session(&output.soul_session_id)
                        .await
                        .map_err(map_core_error)?
                    {
                        let assembly = soul_runtime
                            .list_assembly_items(&output.soul_session_id, None)
                            .await
                            .map_err(map_core_error)?;

                        let _ = hooks
                            .run_turn_completed(TurnCompletedHookInput {
                                turn: &output.turn,
                                session: &output.session,
                                soul_session: &soul_session,
                                assistant_message: Some(&output.assistant_message),
                                assembly_tail: &assembly,
                            })
                            .await;
                    }
                }
            }

            if request.emit_events {
                if let Some(tx) = &tx {
                    let _ = tx.send(Ok(SendSessionEvent::Completed));
                }
            }

            Ok(output.turn)
        }
    }
}

async fn run_turn_body(
    default_soul_id: String,
    request: TurnExecutionRequest,
    model: String,
    provider: Arc<dyn Provider>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    tools: Arc<ToolExecutor>,
    startup: StartupContext,
    turn: Turn,
    tx: Option<mpsc::UnboundedSender<Result<SendSessionEvent, SendSessionError>>>,
) -> Result<TurnRunOutput, SendSessionError> {
    let mut assistant_text = String::new();
    let mut previous_response_id: Option<String> = None;
    let mut function_call_outputs = None;

    loop {
        let request = ProviderRequest {
            model: model.clone(),
            instructions: startup.instructions.clone(),
            input: if previous_response_id.is_some() {
                Vec::new()
            } else {
                startup.provider_input.clone()
            },
            tools: Some(tools.provider_tools()),
            previous_response_id: previous_response_id.clone(),
            function_call_outputs: function_call_outputs.take(),
        };

        let stream = provider.stream(request);
        futures::pin_mut!(stream);

        let mut calls = Vec::new();
        let mut completed_response_id: Option<String> = None;

        while let Some(event) = stream.next().await {
            match event.map_err(map_core_error)? {
                ProviderEvent::OutputTextDelta(delta) => {
                    assistant_text.push_str(&delta);
                    if let Some(tx) = &tx {
                        let _ = tx.send(Ok(SendSessionEvent::OutputTextDelta(delta)));
                    }
                }
                ProviderEvent::FunctionCallRequested(call) => {
                    previous_response_id = Some(call.response_id.clone());
                    calls.push(call);
                }
                ProviderEvent::Completed { response_id } => {
                    if response_id.is_some() {
                        completed_response_id = response_id.clone();
                    }
                    break;
                }
            }
        }

        if calls.is_empty() {
            if previous_response_id.is_none() {
                previous_response_id = completed_response_id;
            }
            break;
        }

        let mut outputs = Vec::new();
        for call in calls {
            outputs.push(
                handle_tool_call(&turn, &startup.runtime_context, &soul_runtime, &tools, call)
                    .await?,
            );
        }

        function_call_outputs = Some(outputs);
    }

    if assistant_text.trim().is_empty() {
        return Err(SendSessionError::Internal(
            "provider completed without assistant output".to_string(),
        ));
    }

    let assistant_message = session_ledger
        .append_message(AppendSessionMessage {
            session_id: request.session_id.clone(),
            message_id: format!("msg_{}", Uuid::new_v4().simple()),
            actor_type: ActorType::Soul,
            actor_id: default_soul_id,
            content: text_content(&assistant_text),
            state: MessageState::Fixed,
        })
        .await
        .map_err(map_core_error)?;

    let assistant_entry = soul_runtime
        .append_message_ref(AppendMessageRef {
            soul_session_id: startup.soul_session_id.clone(),
            message_id: assistant_message.message.id.clone(),
        })
        .await
        .map_err(map_core_error)?;

    soul_runtime
        .complete_turn(CompleteTurn {
            turn_id: turn.id.clone(),
            last_seen_session_seq: assistant_message.relation.session_seq,
            provider_state: previous_response_id.map(|response_id| {
                let basis_soul_session_seq = assistant_entry.entry.soul_session_seq;

                ProviderState {
                    provider: "openai_compatible".to_string(),
                    basis_soul_session_seq,
                    opaque: serde_json::json!({ "response_id": response_id }),
                    schema_version: Some("phase2".to_string()),
                }
            }),
        })
        .await
        .map_err(map_core_error)?;

    Ok(TurnRunOutput {
        turn,
        session: startup.session,
        soul_session_id: startup.soul_session_id,
        assistant_message,
    })
}

async fn handle_tool_call(
    turn: &Turn,
    runtime_context: &ToolRuntimeContext,
    soul_runtime: &Arc<dyn SoulRuntimePort>,
    tools: &Arc<ToolExecutor>,
    call: ProviderFunctionCall,
) -> Result<santi_core::port::provider::FunctionCallOutput, SendSessionError> {
    soul_runtime
        .append_tool_call(AppendToolCall {
            tool_call_id: call.call_id.clone(),
            turn_id: turn.id.clone(),
            tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
        })
        .await
        .map_err(map_core_error)?;

    let dispatch_result = tools
        .dispatch(runtime_context, &call)
        .await
        .map_err(SendSessionError::Internal)?;

    soul_runtime
        .append_tool_result(AppendToolResult {
            tool_result_id: format!("tool_result_{}", Uuid::new_v4().simple()),
            tool_call_id: call.call_id.clone(),
            output: Some(dispatch_result.tool_output.clone()),
            error_text: None,
        })
        .await
        .map_err(map_core_error)?;

    Ok(dispatch_result.function_call_output)
}

fn text_content(text: &str) -> MessageContent {
    MessageContent {
        parts: vec![MessagePart::Text {
            text: text.to_string(),
        }],
    }
}

fn render_send_error(err: &SendSessionError) -> String {
    match err {
        SendSessionError::Busy => "session send busy".to_string(),
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

fn assembly_item_to_input_message(
    item: &AssemblyItem,
) -> Option<santi_core::provider::ProviderInputMessage> {
    match &item.target {
        AssemblyTarget::Message(message) => transcript::to_input_message(message),
        AssemblyTarget::Compact(compact) => transcript::compact_to_input_message(compact),
        AssemblyTarget::ToolCall(_) | AssemblyTarget::ToolResult(_) => None,
    }
}

fn assembly_to_provider_input(
    items: &[AssemblyItem],
) -> Vec<santi_core::provider::ProviderInputMessage> {
    let effective_compact_indexes = effective_compact_indexes(items);

    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match &item.target {
            AssemblyTarget::Message(message) => {
                if message_is_compacted(
                    message.relation.session_seq,
                    items,
                    &effective_compact_indexes,
                ) {
                    None
                } else {
                    transcript::to_input_message(message)
                }
            }
            AssemblyTarget::Compact(_) if effective_compact_indexes.contains(&index) => {
                assembly_item_to_input_message(item)
            }
            AssemblyTarget::Compact(_)
            | AssemblyTarget::ToolCall(_)
            | AssemblyTarget::ToolResult(_) => None,
        })
        .collect()
}

fn effective_compact_indexes(items: &[AssemblyItem]) -> std::collections::BTreeSet<usize> {
    let compact_ranges = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match &item.target {
            AssemblyTarget::Compact(compact) => {
                Some((index, compact.start_session_seq, compact.end_session_seq))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    compact_ranges
        .iter()
        .filter(|(index, start, end)| {
            !compact_ranges
                .iter()
                .any(|(other_index, other_start, other_end)| {
                    other_index > index && other_start <= start && other_end >= end
                })
        })
        .map(|(index, _, _)| *index)
        .collect()
}

fn message_is_compacted(
    session_seq: i64,
    items: &[AssemblyItem],
    effective_compact_indexes: &std::collections::BTreeSet<usize>,
) -> bool {
    items.iter().enumerate().any(|(index, item)| {
        if !effective_compact_indexes.contains(&index) {
            return false;
        }

        match &item.target {
            AssemblyTarget::Compact(compact) => {
                compact.start_session_seq <= session_seq && session_seq <= compact.end_session_seq
            }
            _ => false,
        }
    })
}

#[allow(dead_code)]
fn _message_id(message: &SessionMessage) -> &str {
    &message.message.id
}

#[cfg(test)]
mod tests {
    use santi_core::model::{
        message::{ActorType, Message, MessageContent, MessagePart, MessageState},
        runtime::{AssemblyItem, AssemblyTarget, Compact, SoulSessionEntry, SoulSessionTargetType},
        session::{SessionMessage, SessionMessageRef},
    };

    use super::assembly_to_provider_input;

    #[test]
    fn compact_replaces_covered_messages_in_provider_input() {
        let items = vec![
            message_item(1, "first"),
            message_item(2, "second"),
            compact_item(3, 1, 2, "summary one"),
            message_item(4, "third"),
        ];

        let input = assembly_to_provider_input(&items);
        let contents = input
            .into_iter()
            .map(|m| (m.role, m.content))
            .collect::<Vec<_>>();

        assert_eq!(
            contents,
            vec![
                (
                    "system".to_string(),
                    "[compact 1-2]\nsummary one".to_string()
                ),
                ("user".to_string(), "third".to_string()),
            ]
        );
    }

    #[test]
    fn later_wider_compact_supersedes_earlier_compact() {
        let items = vec![
            message_item(1, "first"),
            compact_item(2, 1, 1, "summary one"),
            message_item(2, "second"),
            compact_item(4, 1, 2, "summary two"),
        ];

        let input = assembly_to_provider_input(&items);
        let contents = input.into_iter().map(|m| m.content).collect::<Vec<_>>();

        assert_eq!(contents, vec!["[compact 1-2]\nsummary two".to_string()]);
    }

    fn message_item(session_seq: i64, text: &str) -> AssemblyItem {
        AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: "ss_1".to_string(),
                target_type: SoulSessionTargetType::Message,
                target_id: format!("msg_{session_seq}"),
                soul_session_seq: session_seq,
                created_at: "now".to_string(),
            },
            target: AssemblyTarget::Message(SessionMessage {
                relation: SessionMessageRef {
                    session_id: "sess_1".to_string(),
                    message_id: format!("msg_{session_seq}"),
                    session_seq,
                    created_at: "now".to_string(),
                },
                message: Message {
                    id: format!("msg_{session_seq}"),
                    actor_type: ActorType::Account,
                    actor_id: "acct_1".to_string(),
                    content: MessageContent {
                        parts: vec![MessagePart::Text {
                            text: text.to_string(),
                        }],
                    },
                    state: MessageState::Fixed,
                    version: 1,
                    deleted_at: None,
                    created_at: "now".to_string(),
                    updated_at: "now".to_string(),
                },
            }),
        }
    }

    fn compact_item(
        soul_session_seq: i64,
        start_session_seq: i64,
        end_session_seq: i64,
        summary: &str,
    ) -> AssemblyItem {
        AssemblyItem {
            entry: SoulSessionEntry {
                soul_session_id: "ss_1".to_string(),
                target_type: SoulSessionTargetType::Compact,
                target_id: format!("compact_{soul_session_seq}"),
                soul_session_seq,
                created_at: "now".to_string(),
            },
            target: AssemblyTarget::Compact(Compact {
                id: format!("compact_{soul_session_seq}"),
                turn_id: format!("turn_{soul_session_seq}"),
                summary: summary.to_string(),
                start_session_seq,
                end_session_seq,
                created_at: "now".to_string(),
            }),
        }
    }
}
