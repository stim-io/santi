use std::sync::Arc;

use santi_core::{
    error::{Error, LockError},
    hook::CompactReason,
    model::runtime::{AssemblyTarget, Compact},
    port::{
        compact_runtime::{AppendCompact, CompactRuntimePort},
        lock::Lock,
        session_ledger::SessionLedgerPort,
        soul_runtime::{AcquireSoulSession, SoulRuntimePort, StartTurn},
    },
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct CompactRequest {
    pub session_id: String,
    pub summary: String,
    pub start_session_seq: Option<i64>,
    pub end_session_seq: Option<i64>,
    pub reason: CompactReason,
}

#[derive(Clone)]
pub struct SessionCompactService {
    lock: Arc<dyn Lock>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    compact_runtime: Arc<dyn CompactRuntimePort>,
    default_soul_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompactSessionError {
    Busy,
    NotFound,
    Invalid(String),
    Internal(String),
}

impl SessionCompactService {
    pub fn new(
        lock: Arc<dyn Lock>,
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_runtime: Arc<dyn SoulRuntimePort>,
        compact_runtime: Arc<dyn CompactRuntimePort>,
        default_soul_id: String,
    ) -> Self {
        Self {
            lock,
            session_ledger,
            soul_runtime,
            compact_runtime,
            default_soul_id,
        }
    }

    pub async fn compact_session(
        &self,
        session_id: &str,
        summary: &str,
    ) -> Result<Compact, CompactSessionError> {
        self.execute_compact(CompactRequest {
            session_id: session_id.to_string(),
            summary: summary.to_string(),
            start_session_seq: None,
            end_session_seq: None,
            reason: CompactReason::Manual,
        })
        .await
    }

    pub async fn execute_compact(
        &self,
        request: CompactRequest,
    ) -> Result<Compact, CompactSessionError> {
        let guard = self
            .lock
            .acquire(&format!("lock:session_send:{}", request.session_id))
            .await
            .map_err(map_lock_error)?;

        if request.summary.trim().is_empty() {
            guard.release().await.map_err(map_lock_error)?;
            return Err(CompactSessionError::Invalid(
                "expected non-empty compact summary".to_string(),
            ));
        }

        let session = self
            .session_ledger
            .get_session(&request.session_id)
            .await
            .map_err(render_error)?
            .ok_or(CompactSessionError::NotFound)?;

        let messages = self
            .session_ledger
            .list_messages(&request.session_id, None)
            .await
            .map_err(render_error)?;
        let computed_end_session_seq = messages
            .last()
            .map(|message| message.relation.session_seq)
            .ok_or_else(|| {
                CompactSessionError::Invalid("cannot compact empty session".to_string())
            })?;

        let soul_session = self
            .soul_runtime
            .acquire_soul_session(AcquireSoulSession {
                soul_id: self.default_soul_id.clone(),
                session_id: session.id.clone(),
            })
            .await
            .map_err(render_error)?;

        let start_session_seq = 1;

        let start_session_seq = request.start_session_seq.unwrap_or(start_session_seq);
        let end_session_seq = request.end_session_seq.unwrap_or(computed_end_session_seq);

        if start_session_seq > end_session_seq {
            guard.release().await.map_err(map_lock_error)?;
            return Err(CompactSessionError::Invalid(
                "no uncompacted session range".to_string(),
            ));
        }

        let turn = self
            .soul_runtime
            .start_turn(StartTurn {
                turn_id: format!("turn_{}", Uuid::new_v4().simple()),
                soul_session_id: soul_session.id.clone(),
                trigger_type: santi_core::model::runtime::TurnTriggerType::System,
                trigger_ref: Some(format!("session_compact:{:?}", request.reason)),
                input_through_session_seq: end_session_seq,
            })
            .await
            .map_err(render_error)?;

        let compact = self
            .compact_runtime
            .append_compact(AppendCompact {
                compact_id: format!("compact_{}", Uuid::new_v4().simple()),
                turn_id: turn.id.clone(),
                summary: request.summary.trim().to_string(),
                start_session_seq,
                end_session_seq,
            })
            .await
            .map_err(render_error)?;

        self.soul_runtime
            .complete_turn(santi_core::port::soul_runtime::CompleteTurn {
                turn_id: turn.id,
                last_seen_session_seq: end_session_seq,
                provider_state: None,
            })
            .await
            .map_err(render_error)?;

        guard.release().await.map_err(map_lock_error)?;

        match compact.target {
            AssemblyTarget::Compact(compact) => Ok(compact),
            _ => Err(CompactSessionError::Internal(
                "unexpected assembly target while compacting session".to_string(),
            )),
        }
    }
}

fn render_error(err: Error) -> CompactSessionError {
    match err {
        Error::NotFound { .. } => CompactSessionError::NotFound,
        Error::Busy { resource } => CompactSessionError::Internal(format!("{resource} busy")),
        Error::InvalidInput { message } => CompactSessionError::Invalid(message),
        Error::Upstream { message } | Error::Internal { message } => {
            CompactSessionError::Internal(message)
        }
    }
}

fn map_lock_error(err: LockError) -> CompactSessionError {
    match err {
        LockError::Busy => CompactSessionError::Busy,
        LockError::Lost => CompactSessionError::Internal("session compact lock lost".to_string()),
        LockError::Backend { message } => CompactSessionError::Internal(message),
    }
}

#[cfg(test)]
mod tests {
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

    use super::{CompactSessionError, SessionCompactService};

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
    async fn returns_busy_when_compact_lock_is_held() {
        let service = SessionCompactService::new(
            Arc::new(BusyLock),
            Arc::new(UnusedLedger),
            Arc::new(UnusedSoulRuntime),
            Arc::new(UnusedCompactRuntime),
            "soul_default".to_string(),
        );

        let err = service
            .compact_session("sess_1", "summary")
            .await
            .expect_err("compact should fail when lock is busy");

        assert_eq!(err, CompactSessionError::Busy);
    }
}
