use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::sync::broadcast;

use stim_proto::{
    ConversationId, MessageId, ReplyEvent, ReplyEventKind, ReplyFailure, ReplyHandle, ReplyId,
    ReplySnapshot, ReplyStatus,
};

#[derive(Clone)]
pub struct ProtocolReplyStore {
    inner: Arc<Mutex<ProtocolReplyStoreState>>,
}

struct ProtocolReplyStoreState {
    next_reply: u64,
    replies: HashMap<ReplyId, ReplyRecord>,
}

struct ReplyRecord {
    snapshot: ReplySnapshot,
    events: Vec<ReplyEvent>,
    next_sequence: u64,
    sender: broadcast::Sender<ReplyEvent>,
    terminal: bool,
}

pub struct ProtocolReplySubscription {
    pub history: Vec<ReplyEvent>,
    pub receiver: broadcast::Receiver<ReplyEvent>,
    pub terminal: bool,
}

impl Default for ProtocolReplyStore {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ProtocolReplyStoreState {
                next_reply: 1,
                replies: HashMap::new(),
            })),
        }
    }
}

impl ProtocolReplyStore {
    pub fn create_reply(
        &self,
        conversation_id: ConversationId,
        message_id: MessageId,
    ) -> ReplyHandle {
        let mut state = self.inner.lock().expect("reply store poisoned");
        let reply_id = format!("reply-{}", state.next_reply);
        state.next_reply += 1;

        let (sender, _) = broadcast::channel(64);
        let snapshot = ReplySnapshot {
            reply_id: reply_id.clone(),
            conversation_id: conversation_id.clone(),
            message_id: message_id.clone(),
            status: ReplyStatus::Pending,
            output_text: String::new(),
            error: None,
        };
        state.replies.insert(
            reply_id.clone(),
            ReplyRecord {
                snapshot,
                events: Vec::new(),
                next_sequence: 1,
                sender,
                terminal: false,
            },
        );

        ReplyHandle {
            reply_id,
            conversation_id,
            message_id,
            status: ReplyStatus::Pending,
        }
    }

    pub fn snapshot(&self, reply_id: &str) -> Option<ReplySnapshot> {
        let state = self.inner.lock().ok()?;
        state
            .replies
            .get(reply_id)
            .map(|record| record.snapshot.clone())
    }

    pub fn subscribe(&self, reply_id: &str) -> Option<ProtocolReplySubscription> {
        let state = self.inner.lock().ok()?;
        let record = state.replies.get(reply_id)?;
        Some(ProtocolReplySubscription {
            history: record.events.clone(),
            receiver: record.sender.subscribe(),
            terminal: record.terminal,
        })
    }

    pub fn emit_text_delta(&self, reply_id: &str, delta: String) {
        self.update(reply_id, |record, event| {
            record.snapshot.status = ReplyStatus::Streaming;
            record.snapshot.output_text.push_str(&delta);
            event.event = ReplyEventKind::OutputTextDelta { delta };
        });
    }

    pub fn complete(&self, reply_id: &str) {
        self.update(reply_id, |record, event| {
            record.snapshot.status = ReplyStatus::Completed;
            record.terminal = true;
            event.event = ReplyEventKind::Completed;
        });
    }

    pub fn fail(&self, reply_id: &str, code: &str, message: String) {
        self.update(reply_id, |record, event| {
            let error = ReplyFailure {
                code: code.to_string(),
                message,
            };
            record.snapshot.status = ReplyStatus::Failed;
            record.snapshot.error = Some(error.clone());
            record.terminal = true;
            event.event = ReplyEventKind::Failed { error };
        });
    }

    fn update<F>(&self, reply_id: &str, mutate: F)
    where
        F: FnOnce(&mut ReplyRecord, &mut ReplyEvent),
    {
        let mut state = match self.inner.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        let Some(record) = state.replies.get_mut(reply_id) else {
            return;
        };
        if record.terminal {
            return;
        }

        let mut event = ReplyEvent {
            reply_id: reply_id.to_string(),
            sequence: record.next_sequence,
            event: ReplyEventKind::Completed,
        };
        mutate(record, &mut event);
        record.next_sequence += 1;
        record.events.push(event.clone());
        let _ = record.sender.send(event);
    }
}
