use std::sync::{Arc, Mutex};

use santi_core::{
    model::effect::SessionEffect,
    port::effect_ledger::{CreateSessionEffect, EffectLedgerPort, UpdateSessionEffect},
};
use santi_runtime::session::{
    effect::{ForkExecutor, ForkHandoffEffectRequest, SeededSendExecutor, SessionEffectService},
    fork::{ForkError, ForkResult},
    send::SendSessionError,
    watch::SessionWatchHub,
};

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
impl ForkExecutor for FakeFork {
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
async fn marks_seeded_send_failed() {
    let ledger = Arc::new(FakeLedger::default());
    let service = SessionEffectService::from_executors(
        ledger.clone(),
        Arc::new(FakeFork),
        Arc::new(FailingSeededSend),
        Arc::new(SessionWatchHub::new()),
    );

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
