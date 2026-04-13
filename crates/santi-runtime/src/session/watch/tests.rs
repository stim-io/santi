use std::sync::{Arc, Mutex};

use futures::StreamExt;
use santi_core::{
    error::Result,
    model::{
        effect::SessionEffect,
        message::{ActorType, Message, MessageContent, MessagePart, MessageState},
        runtime::{AssemblyItem, Compact, SoulSession, Turn},
        session::{Session, SessionMessage, SessionMessageRef},
        soul::Soul,
    },
    port::{
        compact_ledger::CompactLedgerPort,
        effect_ledger::EffectLedgerPort,
        session_ledger::{AppendSessionMessage, ApplyMessageEvent, SessionLedgerPort},
        soul::SoulPort,
        soul_runtime::{
            AcquireSoulSession, AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn,
            FailTurn, SoulRuntimePort, StartTurn,
        },
        soul_session_query::SoulSessionQueryPort,
    },
};

use crate::session::{
    query::SessionQueryService,
    watch::{
        SessionWatchConnected, SessionWatchEvent, SessionWatchHub, SessionWatchService,
        SessionWatchState, SessionWatchStateChanged,
    },
};

struct FakeSessionLedger {
    session: Option<Session>,
    messages: Vec<SessionMessage>,
}

#[async_trait::async_trait]
impl SessionLedgerPort for FakeSessionLedger {
    async fn create_session(&self, _session_id: &str) -> Result<Session> {
        unreachable!()
    }

    async fn get_session(&self, _session_id: &str) -> Result<Option<Session>> {
        Ok(self.session.clone())
    }

    async fn get_message(&self, message_id: &str) -> Result<Option<SessionMessage>> {
        Ok(self
            .messages
            .iter()
            .find(|message| message.message.id == message_id)
            .cloned())
    }

    async fn list_messages(
        &self,
        _session_id: &str,
        _after_session_seq: Option<i64>,
    ) -> Result<Vec<SessionMessage>> {
        Ok(self.messages.clone())
    }

    async fn append_message(&self, _input: AppendSessionMessage) -> Result<SessionMessage> {
        unreachable!()
    }

    async fn apply_message_event(&self, _input: ApplyMessageEvent) -> Result<SessionMessage> {
        unreachable!()
    }
}

struct FakeSoulPort;

#[async_trait::async_trait]
impl SoulPort for FakeSoulPort {
    async fn get_soul(&self, _soul_id: &str) -> Result<Option<Soul>> {
        Ok(None)
    }

    async fn write_soul_memory(&self, _soul_id: &str, _text: &str) -> Result<Option<Soul>> {
        Ok(None)
    }
}

struct FakeSoulRuntime;

#[async_trait::async_trait]
impl SoulRuntimePort for FakeSoulRuntime {
    async fn acquire_soul_session(&self, _input: AcquireSoulSession) -> Result<SoulSession> {
        unreachable!()
    }
    async fn get_soul_session(&self, _soul_session_id: &str) -> Result<Option<SoulSession>> {
        unreachable!()
    }
    async fn write_session_memory(
        &self,
        _soul_session_id: &str,
        _text: &str,
    ) -> Result<Option<SoulSession>> {
        unreachable!()
    }
    async fn start_turn(&self, _input: StartTurn) -> Result<Turn> {
        unreachable!()
    }
    async fn append_message_ref(&self, _input: AppendMessageRef) -> Result<AssemblyItem> {
        unreachable!()
    }
    async fn append_tool_call(&self, _input: AppendToolCall) -> Result<AssemblyItem> {
        unreachable!()
    }
    async fn append_tool_result(&self, _input: AppendToolResult) -> Result<AssemblyItem> {
        unreachable!()
    }
    async fn complete_turn(&self, _input: CompleteTurn) -> Result<Turn> {
        unreachable!()
    }
    async fn fail_turn(&self, _input: FailTurn) -> Result<Turn> {
        unreachable!()
    }
}

#[async_trait::async_trait]
impl SoulSessionQueryPort for FakeSoulRuntime {
    async fn get_soul_session_by_session_id(&self, _session_id: &str) -> Result<Option<SoulSession>> {
        Ok(None)
    }
}

struct FakeCompactLedger;

#[async_trait::async_trait]
impl CompactLedgerPort for FakeCompactLedger {
    async fn list_compacts(&self, _soul_session_id: &str) -> Result<Vec<Compact>> {
        Ok(vec![])
    }
}

struct FakeEffectLedger {
    effects: Arc<Mutex<Vec<SessionEffect>>>,
}

#[async_trait::async_trait]
impl EffectLedgerPort for FakeEffectLedger {
    async fn get_effect(
        &self,
        _session_id: &str,
        _effect_type: &str,
        _idempotency_key: &str,
    ) -> Result<Option<SessionEffect>> {
        unreachable!()
    }

    async fn list_effects(&self, _session_id: &str) -> Result<Vec<SessionEffect>> {
        Ok(self.effects.lock().unwrap().clone())
    }

    async fn create_effect(
        &self,
        _input: santi_core::port::effect_ledger::CreateSessionEffect,
    ) -> Result<SessionEffect> {
        unreachable!()
    }

    async fn update_effect(
        &self,
        _input: santi_core::port::effect_ledger::UpdateSessionEffect,
    ) -> Result<Option<SessionEffect>> {
        unreachable!()
    }
}

#[tokio::test]
async fn returns_snapshot_with_messages_effects_and_latest_seq() {
    let session = Session {
        id: "sess_123".to_string(),
        parent_session_id: None,
        fork_point: None,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    };
    let messages = vec![
        SessionMessage {
            relation: SessionMessageRef {
                session_id: session.id.clone(),
                message_id: "msg_1".to_string(),
                session_seq: 1,
                created_at: "2026-01-01T00:00:00Z".to_string(),
            },
            message: Message {
                id: "msg_1".to_string(),
                actor_type: ActorType::Account,
                actor_id: "acct_1".to_string(),
                content: MessageContent {
                    parts: vec![MessagePart::Text {
                        text: "hello watch".to_string(),
                    }],
                },
                state: MessageState::Fixed,
                version: 1,
                deleted_at: None,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
        },
        SessionMessage {
            relation: SessionMessageRef {
                session_id: session.id.clone(),
                message_id: "msg_2".to_string(),
                session_seq: 2,
                created_at: "2026-01-01T00:01:00Z".to_string(),
            },
            message: Message {
                id: "msg_2".to_string(),
                actor_type: ActorType::Soul,
                actor_id: "soul_default".to_string(),
                content: MessageContent {
                    parts: vec![MessagePart::Text {
                        text: "reply text".to_string(),
                    }],
                },
                state: MessageState::Fixed,
                version: 1,
                deleted_at: None,
                created_at: "2026-01-01T00:01:00Z".to_string(),
                updated_at: "2026-01-01T00:01:00Z".to_string(),
            },
        },
    ];
    let effects = vec![SessionEffect {
        id: "effect_1".to_string(),
        session_id: session.id.clone(),
        effect_type: "hook_fork_handoff".to_string(),
        idempotency_key: "idem_1".to_string(),
        status: "completed".to_string(),
        source_hook_id: "hook_1".to_string(),
        source_turn_id: "turn_1".to_string(),
        result_ref: Some("sess_child".to_string()),
        error_text: None,
        created_at: "2026-01-01T00:02:00Z".to_string(),
        updated_at: "2026-01-01T00:02:01Z".to_string(),
    }];

    let query = Arc::new(SessionQueryService::new(
        Arc::new(FakeSessionLedger {
            session: Some(session.clone()),
            messages: messages.clone(),
        }),
        Arc::new(FakeSoulPort),
        Arc::new(FakeSoulRuntime),
        Arc::new(FakeCompactLedger),
        "soul_default".to_string(),
    ));
    let service = SessionWatchService::new(
        query,
        Arc::new(FakeEffectLedger {
            effects: Arc::new(Mutex::new(effects)),
        }),
        Arc::new(SessionWatchHub::new()),
    );

    let snapshot = service
        .get_session_watch_snapshot(&session.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(snapshot.session_id, "sess_123");
    assert_eq!(snapshot.latest_seq, 2);
    assert_eq!(snapshot.messages.len(), 2);
    assert_eq!(snapshot.messages[0].content_text, "hello watch");
    assert_eq!(snapshot.messages[1].actor_type, "soul");
    assert_eq!(snapshot.effects.len(), 1);
    assert_eq!(snapshot.effects[0].status, "completed");
    assert_eq!(snapshot.effects[0].result_ref.as_deref(), Some("sess_child"));
}

#[tokio::test]
async fn returns_none_when_session_missing() {
    let query = Arc::new(SessionQueryService::new(
        Arc::new(FakeSessionLedger {
            session: None,
            messages: vec![],
        }),
        Arc::new(FakeSoulPort),
        Arc::new(FakeSoulRuntime),
        Arc::new(FakeCompactLedger),
        "soul_default".to_string(),
    ));
    let service = SessionWatchService::new(
        query,
        Arc::new(FakeEffectLedger {
            effects: Arc::new(Mutex::new(vec![])),
        }),
        Arc::new(SessionWatchHub::new()),
    );

    assert!(service
        .get_session_watch_snapshot("missing")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn watch_session_emits_connected_then_runtime_events() {
    let session = Session {
        id: "sess_watch".to_string(),
        parent_session_id: None,
        fork_point: None,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    };
    let query = Arc::new(SessionQueryService::new(
        Arc::new(FakeSessionLedger {
            session: Some(session.clone()),
            messages: vec![],
        }),
        Arc::new(FakeSoulPort),
        Arc::new(FakeSoulRuntime),
        Arc::new(FakeCompactLedger),
        "soul_default".to_string(),
    ));
    let hub = Arc::new(SessionWatchHub::new());
    let service = SessionWatchService::new(
        query,
        Arc::new(FakeEffectLedger {
            effects: Arc::new(Mutex::new(vec![])),
        }),
        hub.clone(),
    );

    let mut stream = service.watch_session(&session.id).await.unwrap();
    let connected = stream.next().await.unwrap().unwrap();
    assert!(matches!(
        connected,
        SessionWatchEvent::Connected(SessionWatchConnected { latest_seq: 0, .. })
    ));

    hub.publish(
        &session.id,
        SessionWatchEvent::StateChanged(SessionWatchStateChanged {
            session_id: session.id.clone(),
            state: SessionWatchState::Running,
        }),
    );

    let next = stream.next().await.unwrap().unwrap();
    assert!(matches!(
        next,
        SessionWatchEvent::StateChanged(SessionWatchStateChanged {
            state: SessionWatchState::Running,
            ..
        })
    ));
}
