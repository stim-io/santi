use std::sync::Arc;

use santi_core::{hook::RuntimeAction, port::ebus::SubscriberSetPort};

use crate::{
    hooks::{HookEvaluator, TurnCompletedHookInput},
    session::compact::{CompactRequest, SessionCompactService},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionStatus {
    Executed,
    Skipped,
    Failed,
}

#[derive(Clone, Debug)]
pub struct ActionRecord {
    pub hook_id: String,
    pub turn_id: String,
    pub action_type: String,
    pub status: ActionStatus,
    pub result_ref: Option<String>,
    pub error_text: Option<String>,
}

#[derive(Clone)]
pub struct HookRuntime {
    subscriber_set: Arc<dyn SubscriberSetPort<Arc<dyn HookEvaluator>>>,
    compact_service: Arc<SessionCompactService>,
}

impl HookRuntime {
    pub fn new(
        subscriber_set: Arc<dyn SubscriberSetPort<Arc<dyn HookEvaluator>>>,
        compact_service: Arc<SessionCompactService>,
    ) -> Self {
        Self {
            subscriber_set,
            compact_service,
        }
    }

    pub async fn run_turn_completed(&self, input: TurnCompletedHookInput<'_>) -> Vec<ActionRecord> {
        let mut records = Vec::new();

        for evaluator in self.subscriber_set.snapshot() {
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

    pub fn replace_subscribers(&self, subscribers: Vec<Arc<dyn HookEvaluator>>) {
        self.subscriber_set.replace_all(subscribers);
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
                    error_text: Some(format!("{err:?}")),
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
