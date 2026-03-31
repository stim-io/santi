#[derive(Clone, Debug)]
pub struct SessionEffect {
    pub id: String,
    pub session_id: String,
    pub effect_type: String,
    pub idempotency_key: String,
    pub status: String,
    pub source_hook_id: String,
    pub source_turn_id: String,
    pub result_ref: Option<String>,
    pub error_text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
