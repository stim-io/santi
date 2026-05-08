use std::sync::{Arc, Mutex};

use santi_core::{
    error::Result,
    model::{
        runtime::{AssemblyItem, Compact, ProviderState, SoulSession, ToolActivity, Turn},
        session::{Session, SessionMessage},
        soul::Soul,
    },
    port::{
        compact_ledger::CompactLedgerPort,
        session_ledger::{AppendSessionMessage, ApplyMessageEvent, SessionLedgerPort},
        soul::SoulPort,
        soul_runtime::{
            AcquireSoulSession, AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn,
            FailTurn, SoulRuntimePort, StartTurn,
        },
        soul_session_query::SoulSessionQueryPort,
    },
};
use santi_runtime::session::query::SessionQueryService;

struct UnusedLedger;

#[async_trait::async_trait]
impl SessionLedgerPort for UnusedLedger {
    async fn create_session(&self, _session_id: &str) -> Result<Session> {
        unreachable!()
    }

    async fn get_session(&self, _session_id: &str) -> Result<Option<Session>> {
        unreachable!()
    }

    async fn get_message(&self, _message_id: &str) -> Result<Option<SessionMessage>> {
        unreachable!()
    }

    async fn list_messages(
        &self,
        _session_id: &str,
        _after_session_seq: Option<i64>,
    ) -> Result<Vec<SessionMessage>> {
        unreachable!()
    }

    async fn append_message(&self, _input: AppendSessionMessage) -> Result<SessionMessage> {
        unreachable!()
    }

    async fn apply_message_event(&self, _input: ApplyMessageEvent) -> Result<SessionMessage> {
        unreachable!()
    }
}

struct UnusedSoulPort;

#[async_trait::async_trait]
impl SoulPort for UnusedSoulPort {
    async fn get_soul(&self, _soul_id: &str) -> Result<Option<Soul>> {
        unreachable!()
    }

    async fn write_soul_memory(&self, _soul_id: &str, _text: &str) -> Result<Option<Soul>> {
        unreachable!()
    }
}

struct FakeSoulRuntime {
    soul_session: Option<SoulSession>,
}

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
    async fn get_session_soul(&self, _session_id: &str) -> Result<Option<SoulSession>> {
        Ok(self.soul_session.clone())
    }

    async fn list_tool_activities(&self, _soul_session_id: &str) -> Result<Vec<ToolActivity>> {
        Ok(vec![])
    }
}

struct FakeCompactLedger {
    compacts: Vec<Compact>,
    listed_ids: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl CompactLedgerPort for FakeCompactLedger {
    async fn list_compacts(&self, soul_session_id: &str) -> Result<Vec<Compact>> {
        self.listed_ids
            .lock()
            .expect("poisoned")
            .push(soul_session_id.to_string());
        Ok(self.compacts.clone())
    }
}

#[tokio::test]
async fn list_compacts_without_soul() {
    let listed_ids = Arc::new(Mutex::new(Vec::new()));
    let service = SessionQueryService::new(
        Arc::new(UnusedLedger),
        Arc::new(UnusedSoulPort),
        Arc::new(FakeSoulRuntime { soul_session: None }),
        Arc::new(FakeCompactLedger {
            compacts: vec![],
            listed_ids: listed_ids.clone(),
        }),
        "soul_default".to_string(),
    );

    let compacts = service
        .list_session_compacts("sess_missing")
        .await
        .expect("query should succeed");

    assert!(compacts.is_empty());
    assert!(listed_ids.lock().expect("poisoned").is_empty());
}

#[tokio::test]
async fn list_compacts_from_runtime() {
    let listed_ids = Arc::new(Mutex::new(Vec::new()));
    let service = SessionQueryService::new(
        Arc::new(UnusedLedger),
        Arc::new(UnusedSoulPort),
        Arc::new(FakeSoulRuntime {
            soul_session: Some(SoulSession {
                id: "ss_123".to_string(),
                soul_id: "soul_default".to_string(),
                session_id: "sess_123".to_string(),
                session_memory: String::new(),
                provider_state: Some(ProviderState {
                    provider: "test".to_string(),
                    basis_soul_session_seq: 1,
                    opaque: serde_json::json!({}),
                    schema_version: None,
                }),
                next_seq: 3,
                last_seen_session_seq: 2,
                parent_soul_session_id: None,
                fork_point: None,
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
            }),
        }),
        Arc::new(FakeCompactLedger {
            compacts: vec![Compact {
                id: "compact_1".to_string(),
                turn_id: "turn_1".to_string(),
                summary: "summary one".to_string(),
                start_session_seq: 1,
                end_session_seq: 2,
                created_at: "now".to_string(),
            }],
            listed_ids: listed_ids.clone(),
        }),
        "soul_default".to_string(),
    );

    let compacts = service
        .list_session_compacts("sess_123")
        .await
        .expect("query should succeed");

    assert_eq!(compacts.len(), 1);
    assert_eq!(compacts[0].summary, "summary one");
    assert_eq!(compacts[0].start_session_seq, 1);
    assert_eq!(compacts[0].end_session_seq, 2);
    assert_eq!(
        listed_ids.lock().expect("poisoned").as_slice(),
        &["ss_123".to_string()]
    );
}
