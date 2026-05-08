use std::sync::Arc;

use santi_core::{
    error::LockError,
    port::{
        compact_runtime::CompactRuntimePort,
        lock::{Lock, LockGuard},
        session_ledger::SessionLedgerPort,
        soul_runtime::SoulRuntimePort,
    },
};
use santi_runtime::session::{
    compact::{CompactSessionError, SessionCompactService},
    watch::SessionWatchHub,
};

struct BusyLock;

#[async_trait::async_trait]
impl Lock for BusyLock {
    async fn acquire(
        &self,
        _key: &str,
    ) -> std::result::Result<Box<dyn LockGuard + Send>, LockError> {
        Err(LockError::Busy)
    }
}

struct UnusedLedger;

#[async_trait::async_trait]
impl SessionLedgerPort for UnusedLedger {
    async fn create_session(
        &self,
        _session_id: &str,
    ) -> santi_core::error::Result<santi_core::model::session::Session> {
        unreachable!()
    }

    async fn get_session(
        &self,
        _session_id: &str,
    ) -> santi_core::error::Result<Option<santi_core::model::session::Session>> {
        unreachable!()
    }

    async fn get_message(
        &self,
        _message_id: &str,
    ) -> santi_core::error::Result<Option<santi_core::model::session::SessionMessage>> {
        unreachable!()
    }

    async fn list_messages(
        &self,
        _session_id: &str,
        _after_session_seq: Option<i64>,
    ) -> santi_core::error::Result<Vec<santi_core::model::session::SessionMessage>> {
        unreachable!()
    }

    async fn append_message(
        &self,
        _input: santi_core::port::session_ledger::AppendSessionMessage,
    ) -> santi_core::error::Result<santi_core::model::session::SessionMessage> {
        unreachable!()
    }

    async fn apply_message_event(
        &self,
        _input: santi_core::port::session_ledger::ApplyMessageEvent,
    ) -> santi_core::error::Result<santi_core::model::session::SessionMessage> {
        unreachable!()
    }
}

struct UnusedSoulRuntime;

struct UnusedCompactRuntime;

#[async_trait::async_trait]
impl SoulRuntimePort for UnusedSoulRuntime {
    async fn acquire_soul_session(
        &self,
        _input: santi_core::port::soul_runtime::AcquireSoulSession,
    ) -> santi_core::error::Result<santi_core::model::runtime::SoulSession> {
        unreachable!()
    }

    async fn get_soul_session(
        &self,
        _soul_session_id: &str,
    ) -> santi_core::error::Result<Option<santi_core::model::runtime::SoulSession>> {
        unreachable!()
    }

    async fn write_session_memory(
        &self,
        _soul_session_id: &str,
        _text: &str,
    ) -> santi_core::error::Result<Option<santi_core::model::runtime::SoulSession>> {
        unreachable!()
    }

    async fn start_turn(
        &self,
        _input: santi_core::port::soul_runtime::StartTurn,
    ) -> santi_core::error::Result<santi_core::model::runtime::Turn> {
        unreachable!()
    }

    async fn append_message_ref(
        &self,
        _input: santi_core::port::soul_runtime::AppendMessageRef,
    ) -> santi_core::error::Result<santi_core::model::runtime::AssemblyItem> {
        unreachable!()
    }

    async fn append_tool_call(
        &self,
        _input: santi_core::port::soul_runtime::AppendToolCall,
    ) -> santi_core::error::Result<santi_core::model::runtime::AssemblyItem> {
        unreachable!()
    }

    async fn append_tool_result(
        &self,
        _input: santi_core::port::soul_runtime::AppendToolResult,
    ) -> santi_core::error::Result<santi_core::model::runtime::AssemblyItem> {
        unreachable!()
    }

    async fn complete_turn(
        &self,
        _input: santi_core::port::soul_runtime::CompleteTurn,
    ) -> santi_core::error::Result<santi_core::model::runtime::Turn> {
        unreachable!()
    }

    async fn fail_turn(
        &self,
        _input: santi_core::port::soul_runtime::FailTurn,
    ) -> santi_core::error::Result<santi_core::model::runtime::Turn> {
        unreachable!()
    }
}

#[async_trait::async_trait]
impl CompactRuntimePort for UnusedCompactRuntime {
    async fn append_compact(
        &self,
        _input: santi_core::port::compact_runtime::AppendCompact,
    ) -> santi_core::error::Result<santi_core::model::runtime::AssemblyItem> {
        unreachable!()
    }
}

#[tokio::test]
async fn compact_lock_returns_busy() {
    let service = SessionCompactService::new(
        Arc::new(BusyLock),
        Arc::new(UnusedLedger),
        Arc::new(UnusedSoulRuntime),
        Arc::new(UnusedCompactRuntime),
        "soul_default".to_string(),
        Arc::new(SessionWatchHub::new()),
    );

    let err = service
        .compact_session("sess_1", "summary")
        .await
        .expect_err("compact should fail when lock is busy");

    assert_eq!(err, CompactSessionError::Busy);
}
