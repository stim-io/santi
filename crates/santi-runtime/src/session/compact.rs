use std::sync::Arc;

use santi_core::{
    error::Error,
    model::runtime::{AssemblyTarget, Compact},
    port::{
        session_ledger::SessionLedgerPort,
        soul_runtime::{AppendCompact, SoulRuntimePort, StartTurn},
    },
};
use uuid::Uuid;

use crate::hooks::CompactReason;

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
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
    default_soul_id: String,
}

impl SessionCompactService {
    pub fn new(
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_runtime: Arc<dyn SoulRuntimePort>,
        default_soul_id: String,
    ) -> Self {
        Self {
            session_ledger,
            soul_runtime,
            default_soul_id,
        }
    }

    pub async fn compact_session(
        &self,
        session_id: &str,
        summary: &str,
    ) -> Result<Compact, String> {
        self.execute_compact(CompactRequest {
            session_id: session_id.to_string(),
            summary: summary.to_string(),
            start_session_seq: None,
            end_session_seq: None,
            reason: CompactReason::Manual,
        })
        .await
    }

    pub async fn execute_compact(&self, request: CompactRequest) -> Result<Compact, String> {
        if request.summary.trim().is_empty() {
            return Err("expected non-empty compact summary".to_string());
        }

        let session = self
            .session_ledger
            .get_session(&request.session_id)
            .await
            .map_err(render_error)?
            .ok_or_else(|| "session not found".to_string())?;

        let messages = self
            .session_ledger
            .list_messages(&request.session_id, None)
            .await
            .map_err(render_error)?;
        let computed_end_session_seq = messages
            .last()
            .map(|message| message.relation.session_seq)
            .ok_or_else(|| "cannot compact empty session".to_string())?;

        let soul_session = self
            .soul_runtime
            .get_or_create_soul_session(&self.default_soul_id, &session.id)
            .await
            .map_err(render_error)?;

        let assembly_items = self
            .soul_runtime
            .list_assembly_items(&soul_session.id, None)
            .await
            .map_err(render_error)?;

        let start_session_seq = assembly_items
            .iter()
            .filter_map(|item| match &item.target {
                AssemblyTarget::Compact(compact) => Some(compact.end_session_seq),
                _ => None,
            })
            .max()
            .map(|seq| seq + 1)
            .unwrap_or(1);

        let start_session_seq = request.start_session_seq.unwrap_or(start_session_seq);
        let end_session_seq = request.end_session_seq.unwrap_or(computed_end_session_seq);

        if start_session_seq > end_session_seq {
            return Err("no uncompacted session range".to_string());
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
            .soul_runtime
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

        match compact.target {
            AssemblyTarget::Compact(compact) => Ok(compact),
            _ => Err("unexpected assembly target while compacting session".to_string()),
        }
    }
}

fn render_error(err: Error) -> String {
    match err {
        Error::NotFound { resource } => format!("{resource} not found"),
        Error::Busy { resource } => format!("{resource} busy"),
        Error::InvalidInput { message } => message,
        Error::Upstream { message } => message,
        Error::Internal { message } => message,
    }
}
