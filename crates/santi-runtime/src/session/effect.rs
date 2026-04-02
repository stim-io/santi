use std::sync::Arc;

use santi_core::{
    error::Error,
    model::effect::SessionEffect,
    port::effect_ledger::{CreateSessionEffect, EffectLedgerPort, UpdateSessionEffect},
};
use uuid::Uuid;

use crate::session::{
    fork::{ForkError, ForkResult, SessionForkService},
    send::{SendSessionError, SessionTurnService, TurnExecutionRequest, TurnInput},
};

const EFFECT_TYPE_FORK_HANDOFF: &str = "hook_fork_handoff";

#[derive(Clone, Debug)]
pub struct ForkHandoffEffectRequest {
    pub parent_session_id: String,
    pub source_hook_id: String,
    pub source_turn_id: String,
    pub fork_point: i64,
    pub seed_text: String,
}

#[async_trait::async_trait]
trait ForkExecutor: Send + Sync {
    async fn fork_session(
        &self,
        parent_session_id: String,
        fork_point: i64,
        request_id: String,
    ) -> Result<ForkResult, ForkError>;
}

#[async_trait::async_trait]
impl ForkExecutor for SessionForkService {
    async fn fork_session(
        &self,
        parent_session_id: String,
        fork_point: i64,
        request_id: String,
    ) -> Result<ForkResult, ForkError> {
        SessionForkService::fork_session(self, parent_session_id, fork_point, request_id).await
    }
}

#[async_trait::async_trait]
trait SeededSendExecutor: Send + Sync {
    async fn seeded_send(
        &self,
        session_id: String,
        seed_text: String,
    ) -> Result<String, SendSessionError>;
}

#[async_trait::async_trait]
impl SeededSendExecutor for SessionTurnService {
    async fn seeded_send(
        &self,
        session_id: String,
        seed_text: String,
    ) -> Result<String, SendSessionError> {
        self.execute(
            TurnExecutionRequest {
                session_id,
                input: TurnInput::SystemSeed {
                    actor_id: "hook_fork_handoff".to_string(),
                    text: seed_text,
                },
                emit_events: false,
                run_hooks: false,
            },
            None,
            None,
        )
        .await
        .map(|turn| turn.id)
    }
}

#[derive(Clone)]
pub struct SessionEffectService {
    ledger: Arc<dyn EffectLedgerPort>,
    fork_service: Arc<dyn ForkExecutor>,
    seeded_send: Arc<dyn SeededSendExecutor>,
}

impl SessionEffectService {
    pub fn new(
        ledger: Arc<dyn EffectLedgerPort>,
        fork_service: Arc<SessionForkService>,
        seeded_send: Arc<SessionTurnService>,
    ) -> Self {
        Self {
            ledger,
            fork_service,
            seeded_send,
        }
    }

    pub async fn execute_fork_handoff(
        &self,
        request: ForkHandoffEffectRequest,
    ) -> Result<SessionEffect, String> {
        let idempotency_key = format!(
            "{EFFECT_TYPE_FORK_HANDOFF}:{}:{}:{}",
            request.source_hook_id, request.source_turn_id, request.fork_point
        );

        if let Some(existing) = self
            .ledger
            .get_effect(
                &request.parent_session_id,
                EFFECT_TYPE_FORK_HANDOFF,
                &idempotency_key,
            )
            .await
            .map_err(render_core_error)?
        {
            if existing.status == "completed" {
                return Ok(existing);
            }

            let rerun = self.run_fork_handoff(&request).await;
            return self
                .finish_effect(existing.id, existing.result_ref, rerun)
                .await;
        }

        let effect_id = format!("effect_{}", Uuid::new_v4().simple());
        let created = self
            .ledger
            .create_effect(CreateSessionEffect {
                effect_id: effect_id.clone(),
                session_id: request.parent_session_id.clone(),
                effect_type: EFFECT_TYPE_FORK_HANDOFF.to_string(),
                idempotency_key,
                status: "started".to_string(),
                source_hook_id: request.source_hook_id.clone(),
                source_turn_id: request.source_turn_id.clone(),
                result_ref: None,
                error_text: None,
            })
            .await
            .map_err(render_core_error)?;

        self.finish_effect(
            effect_id,
            created.result_ref,
            self.run_fork_handoff(&request).await,
        )
        .await
    }

    async fn run_fork_handoff(
        &self,
        request: &ForkHandoffEffectRequest,
    ) -> Result<String, (Option<String>, String)> {
        let fork_result = self
            .fork_service
            .fork_session(
                request.parent_session_id.clone(),
                request.fork_point,
                format!(
                    "hook_fork_handoff:{}:{}:{}",
                    request.source_hook_id, request.source_turn_id, request.fork_point
                ),
            )
            .await
            .map_err(|err| (None, render_fork_error(err)))?;

        self.seeded_send
            .seeded_send(
                fork_result.new_session_id.clone(),
                request.seed_text.clone(),
            )
            .await
            .map_err(|err| {
                (
                    Some(fork_result.new_session_id.clone()),
                    render_send_error(err),
                )
            })?;

        Ok(fork_result.new_session_id)
    }

    async fn finish_effect(
        &self,
        effect_id: String,
        prior_result_ref: Option<String>,
        run_result: Result<String, (Option<String>, String)>,
    ) -> Result<SessionEffect, String> {
        match run_result {
            Ok(result_ref) => self
                .ledger
                .update_effect(UpdateSessionEffect {
                    effect_id,
                    status: "completed".to_string(),
                    result_ref: Some(result_ref),
                    error_text: None,
                })
                .await
                .map_err(render_core_error)?
                .ok_or_else(|| "effect disappeared before completion".to_string()),
            Err((result_ref, err)) => self
                .ledger
                .update_effect(UpdateSessionEffect {
                    effect_id,
                    status: "failed".to_string(),
                    result_ref: result_ref.or(prior_result_ref),
                    error_text: Some(err.clone()),
                })
                .await
                .map_err(render_core_error)?
                .ok_or(err),
        }
    }
}

fn render_core_error(err: Error) -> String {
    match err {
        Error::NotFound { resource } => format!("{resource} not found"),
        Error::Busy { resource } => format!("{resource} busy"),
        Error::InvalidInput { message }
        | Error::Upstream { message }
        | Error::Internal { message } => message,
    }
}

fn render_fork_error(err: ForkError) -> String {
    match err {
        ForkError::Busy => "fork busy".to_string(),
        ForkError::ParentNotFound => "parent session not found".to_string(),
        ForkError::InvalidForkPoint(message) | ForkError::Internal(message) => message,
    }
}

fn render_send_error(err: SendSessionError) -> String {
    match err {
        SendSessionError::Busy => "seeded send busy".to_string(),
        SendSessionError::NotFound => "seeded send session not found".to_string(),
        SendSessionError::Internal(message) => message,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use santi_core::{
        model::effect::SessionEffect,
        port::effect_ledger::{CreateSessionEffect, EffectLedgerPort, UpdateSessionEffect},
    };

    use super::{
        ForkError, ForkHandoffEffectRequest, ForkResult, SeededSendExecutor, SessionEffectService,
    };
    use crate::session::send::SendSessionError;

    #[derive(Default)]
    struct FakeLedger {
        created: Arc<Mutex<Vec<SessionEffect>>>,
        updated: Arc<Mutex<Vec<SessionEffect>>>,
    }

    #[async_trait::async_trait]
    impl EffectLedgerPort for FakeLedger {
        async fn list_effects(
            &self,
            _session_id: &str,
        ) -> santi_core::error::Result<Vec<SessionEffect>> {
            Ok(Vec::new())
        }

        async fn get_effect(
            &self,
            _session_id: &str,
            _effect_type: &str,
            _idempotency_key: &str,
        ) -> santi_core::error::Result<Option<SessionEffect>> {
            Ok(None)
        }

        async fn create_effect(
            &self,
            input: CreateSessionEffect,
        ) -> santi_core::error::Result<SessionEffect> {
            let effect = SessionEffect {
                id: input.effect_id,
                session_id: input.session_id,
                effect_type: input.effect_type,
                idempotency_key: input.idempotency_key,
                status: input.status,
                source_hook_id: input.source_hook_id,
                source_turn_id: input.source_turn_id,
                result_ref: input.result_ref,
                error_text: input.error_text,
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
            };
            self.created.lock().expect("poisoned").push(effect.clone());
            Ok(effect)
        }

        async fn update_effect(
            &self,
            input: UpdateSessionEffect,
        ) -> santi_core::error::Result<Option<SessionEffect>> {
            let effect = SessionEffect {
                id: input.effect_id,
                session_id: "sess_parent".to_string(),
                effect_type: "hook_fork_handoff".to_string(),
                idempotency_key: "idemp".to_string(),
                status: input.status,
                source_hook_id: "hook_1".to_string(),
                source_turn_id: "turn_1".to_string(),
                result_ref: input.result_ref,
                error_text: input.error_text,
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
            };
            self.updated.lock().expect("poisoned").push(effect.clone());
            Ok(Some(effect))
        }
    }

    struct FakeFork;

    #[async_trait::async_trait]
    impl super::ForkExecutor for FakeFork {
        async fn fork_session(
            &self,
            parent_session_id: String,
            fork_point: i64,
            _request_id: String,
        ) -> Result<ForkResult, ForkError> {
            Ok(ForkResult {
                new_session_id: "sess_child".to_string(),
                parent_session_id,
                fork_point,
            })
        }
    }

    struct FailingSeededSend;

    #[async_trait::async_trait]
    impl SeededSendExecutor for FailingSeededSend {
        async fn seeded_send(
            &self,
            _session_id: String,
            _seed_text: String,
        ) -> Result<String, SendSessionError> {
            Err(SendSessionError::Internal("seed failed".to_string()))
        }
    }

    #[tokio::test]
    async fn marks_effect_failed_when_seeded_send_fails() {
        let ledger = Arc::new(FakeLedger::default());
        let service = SessionEffectService {
            ledger: ledger.clone(),
            fork_service: Arc::new(FakeFork),
            seeded_send: Arc::new(FailingSeededSend),
        };

        let effect = service
            .execute_fork_handoff(ForkHandoffEffectRequest {
                parent_session_id: "sess_parent".to_string(),
                source_hook_id: "hook_1".to_string(),
                source_turn_id: "turn_1".to_string(),
                fork_point: 3,
                seed_text: "recommend compact".to_string(),
            })
            .await
            .expect("failed effect should still persist");

        assert_eq!(effect.status, "failed");
        assert_eq!(effect.error_text.as_deref(), Some("seed failed"));
        assert_eq!(effect.result_ref.as_deref(), Some("sess_child"));
    }
}
