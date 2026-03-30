use std::sync::Arc;

use crate::{
    hooks::{
        ActionRecord, ActionStatus, HookRegistryHolder, RuntimeAction, TurnCompletedHookInput,
    },
    session::compact::{CompactRequest, SessionCompactService},
};

#[derive(Clone)]
pub struct HookRuntime {
    registry_holder: HookRegistryHolder,
    compact_service: Arc<SessionCompactService>,
}

impl HookRuntime {
    pub fn new(
        registry_holder: HookRegistryHolder,
        compact_service: Arc<SessionCompactService>,
    ) -> Self {
        Self {
            registry_holder,
            compact_service,
        }
    }

    pub async fn run_turn_completed(&self, input: TurnCompletedHookInput<'_>) -> Vec<ActionRecord> {
        let mut records = Vec::new();
        let registry = self.registry_holder.snapshot();

        for evaluator in registry.turn_completed() {
            let actions = evaluator.evaluate_turn_completed(TurnCompletedHookInput {
                turn: input.turn,
                session: input.session,
                soul_session: input.soul_session,
                assistant_message: input.assistant_message,
                assembly_tail: input.assembly_tail,
            });

            for action in actions {
                let record = self.execute_action(action).await;
                tracing::info!(
                    hook_id = %record.hook_id,
                    turn_id = %record.turn_id,
                    action_type = %record.action_type,
                    status = ?record.status,
                    result_ref = ?record.result_ref,
                    error_text = ?record.error_text,
                    "hook action executed"
                );
                records.push(record);
            }
        }

        records
    }

    async fn execute_action(&self, action: RuntimeAction) -> ActionRecord {
        match action {
            RuntimeAction::Compact {
                session_id,
                soul_session_id: _,
                start_session_seq,
                end_session_seq,
                summary,
                reason,
                source_hook_id,
                source_turn_id,
            } => match self
                .compact_service
                .execute_compact(CompactRequest {
                    session_id,
                    summary,
                    start_session_seq: Some(start_session_seq),
                    end_session_seq: Some(end_session_seq),
                    reason,
                })
                .await
            {
                Ok(compact) => ActionRecord {
                    hook_id: source_hook_id,
                    turn_id: source_turn_id,
                    action_type: "compact".to_string(),
                    status: ActionStatus::Executed,
                    result_ref: Some(compact.id),
                    error_text: None,
                },
                Err(err) => ActionRecord {
                    hook_id: source_hook_id,
                    turn_id: source_turn_id,
                    action_type: "compact".to_string(),
                    status: ActionStatus::Failed,
                    result_ref: None,
                    error_text: Some(err),
                },
            },
            RuntimeAction::ForkReserved {
                source_hook_id,
                source_turn_id,
            } => ActionRecord {
                hook_id: source_hook_id,
                turn_id: source_turn_id,
                action_type: "fork".to_string(),
                status: ActionStatus::Skipped,
                result_ref: None,
                error_text: Some("fork action reserved but not implemented".to_string()),
            },
        }
    }
}
