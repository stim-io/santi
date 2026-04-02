use crate::{error::Result, model::effect::SessionEffect};

#[derive(Clone, Debug)]
pub struct CreateSessionEffect {
    pub effect_id: String,
    pub session_id: String,
    pub effect_type: String,
    pub idempotency_key: String,
    pub status: String,
    pub source_hook_id: String,
    pub source_turn_id: String,
    pub result_ref: Option<String>,
    pub error_text: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UpdateSessionEffect {
    pub effect_id: String,
    pub status: String,
    pub result_ref: Option<String>,
    pub error_text: Option<String>,
}

#[async_trait::async_trait]
pub trait EffectLedgerPort: Send + Sync {
    async fn list_effects(&self, session_id: &str) -> Result<Vec<SessionEffect>>;

    async fn get_effect(
        &self,
        session_id: &str,
        effect_type: &str,
        idempotency_key: &str,
    ) -> Result<Option<SessionEffect>>;

    async fn create_effect(&self, input: CreateSessionEffect) -> Result<SessionEffect>;

    async fn update_effect(&self, input: UpdateSessionEffect) -> Result<Option<SessionEffect>>;
}
