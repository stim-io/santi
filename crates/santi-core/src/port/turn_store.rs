use crate::{
    error::Result,
    model::{message::Message, session::Session},
};

#[derive(Clone, Debug)]
pub struct TurnContext {
    pub session: Session,
    pub soul_memory: String,
}

#[derive(Clone, Debug)]
pub struct NewTurnMessage {
    pub r#type: String,
    pub role: Option<String>,
    pub content: String,
}

pub trait TurnStore {
    async fn load_turn_context(&self, session_id: &str) -> Result<Option<TurnContext>>;
    async fn list_messages(&self, session_id: &str) -> Result<Vec<Message>>;
    async fn append_message(&self, session_id: &str, message: NewTurnMessage) -> Result<Message>;
}
