use crate::model::message::Message;

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub parent_session_id: Option<String>,
    pub fork_point: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct SessionMessageRef {
    pub session_id: String,
    pub message_id: String,
    pub session_seq: i64,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct SessionMessage {
    pub relation: SessionMessageRef,
    pub message: Message,
}
