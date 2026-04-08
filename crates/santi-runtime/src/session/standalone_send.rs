use std::sync::Arc;

use santi_core::{
    error::{Error, LockError},
    model::message::{ActorType, MessageContent, MessagePart, MessageState},
    model::runtime::TurnTriggerType,
    port::{
        lock::Lock,
        session_ledger::{AppendSessionMessage, SessionLedgerPort},
        soul_runtime::{AppendMessageRef, CompleteTurn, SoulRuntimePort, StartTurn},
    },
};
use uuid::Uuid;

#[derive(Clone)]
pub struct StandaloneSessionSendService {
    lock: Arc<dyn Lock>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_runtime: Arc<dyn SoulRuntimePort>,
}

#[derive(Clone, Debug)]
pub enum StandaloneSendError {
    Busy,
    NotFound,
    Internal(String),
}

impl StandaloneSessionSendService {
    pub fn new(
        lock: Arc<dyn Lock>,
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_runtime: Arc<dyn SoulRuntimePort>,
    ) -> Self {
        Self {
            lock,
            session_ledger,
            soul_runtime,
        }
    }

    pub async fn send_text(&self, session_id: &str, text: &str) -> Result<(), StandaloneSendError> {
        let guard = self
            .lock
            .acquire(&format!("lock:session_send:{session_id}"))
            .await
            .map_err(map_lock_error)?;

        let append_result = self
            .session_ledger
            .append_message(AppendSessionMessage {
                session_id: session_id.to_string(),
                message_id: Uuid::new_v4().to_string(),
                actor_type: ActorType::Account,
                actor_id: "user".to_string(),
                content: MessageContent {
                    parts: vec![MessagePart::Text {
                        text: text.to_string(),
                    }],
                },
                state: MessageState::Fixed,
            })
            .await
            .map_err(map_error);

        let turn_result: Result<(), StandaloneSendError> =
            if let Ok(trigger_message) = &append_result {
                let soul_session = self
                    .soul_runtime
                    .acquire_soul_session(santi_core::port::soul_runtime::AcquireSoulSession {
                        soul_id: "soul_default".to_string(),
                        session_id: session_id.to_string(),
                    })
                    .await
                    .map_err(map_error)?;
                self.soul_runtime
                    .append_message_ref(AppendMessageRef {
                        soul_session_id: soul_session.id.clone(),
                        message_id: trigger_message.message.id.clone(),
                    })
                    .await
                    .map_err(map_error)?;
                let turn = self
                    .soul_runtime
                    .start_turn(StartTurn {
                        turn_id: Uuid::new_v4().to_string(),
                        soul_session_id: soul_session.id,
                        trigger_type: TurnTriggerType::SessionSend,
                        trigger_ref: Some(trigger_message.message.id.clone()),
                        input_through_session_seq: trigger_message.relation.session_seq,
                    })
                    .await
                    .map_err(map_error)?;
                self.soul_runtime
                    .complete_turn(CompleteTurn {
                        turn_id: turn.id,
                        last_seen_session_seq: trigger_message.relation.session_seq,
                        provider_state: None,
                    })
                    .await
                    .map_err(map_error)?;
                Ok(())
            } else {
                Ok(())
            };

        let release_result = guard.release().await.map_err(map_lock_error);

        match (append_result, turn_result, release_result) {
            (Err(err), _, _) => Err(err),
            (Ok(_), Err(err), _) => Err(err),
            (Ok(_), Ok(_), Err(err)) => Err(err),
            (Ok(_), Ok(_), Ok(())) => Ok(()),
        }
    }
}

fn map_error(err: Error) -> StandaloneSendError {
    match err {
        Error::NotFound { .. } => StandaloneSendError::NotFound,
        Error::Busy { .. } => StandaloneSendError::Busy,
        Error::InvalidInput { message }
        | Error::Upstream { message }
        | Error::Internal { message } => StandaloneSendError::Internal(message),
    }
}

fn map_lock_error(err: LockError) -> StandaloneSendError {
    match err {
        LockError::Busy => StandaloneSendError::Busy,
        LockError::Lost => {
            StandaloneSendError::Internal("standalone session send lock lost".to_string())
        }
        LockError::Backend { message } => StandaloneSendError::Internal(message),
    }
}
