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

use crate::session::watch::{
    SessionWatchActivityChanged, SessionWatchActivityKind, SessionWatchActivityState,
    SessionWatchEvent, SessionWatchHub,
};

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
    watch: Arc<SessionWatchHub>,
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
        watch: Arc<SessionWatchHub>,
    ) -> Self {
        Self {
            lock,
            session_ledger,
            soul_runtime,
            compact_runtime,
            default_soul_id,
            watch,
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

        self.watch.publish(
            &request.session_id,
            SessionWatchEvent::ActivityChanged(SessionWatchActivityChanged {
                session_id: request.session_id.clone(),
                activity: SessionWatchActivityKind::Compact,
                state: SessionWatchActivityState::Started,
                label: None,
            }),
        );

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
            AssemblyTarget::Compact(compact) => {
                self.watch.publish(
                    &request.session_id,
                    SessionWatchEvent::ActivityChanged(SessionWatchActivityChanged {
                        session_id: request.session_id.clone(),
                        activity: SessionWatchActivityKind::Compact,
                        state: SessionWatchActivityState::Completed,
                        label: None,
                    }),
                );
                Ok(compact)
            }
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
