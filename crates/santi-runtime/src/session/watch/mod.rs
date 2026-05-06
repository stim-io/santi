use std::sync::Arc;

use async_stream::try_stream;
use santi_core::port::effect_ledger::EffectLedgerPort;
use tokio::sync::broadcast;

use crate::session::query::SessionQueryService;

mod projection;
mod shapes;

#[cfg(test)]
mod tests;

pub use shapes::{
    SessionWatchActivityChanged, SessionWatchActivityKind, SessionWatchActivityState,
    SessionWatchConnected, SessionWatchEffectSummary, SessionWatchError, SessionWatchEvent,
    SessionWatchMessageChange, SessionWatchMessageChanged, SessionWatchMessageSummary,
    SessionWatchSnapshot, SessionWatchState, SessionWatchStateChanged, SessionWatchStream,
};

#[derive(Clone, Debug)]
struct SessionWatchEnvelope {
    session_id: String,
    event: SessionWatchEvent,
}

#[derive(Clone)]
pub struct SessionWatchHub {
    tx: broadcast::Sender<SessionWatchEnvelope>,
}

impl Default for SessionWatchHub {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionWatchHub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    pub fn publish(&self, session_id: &str, event: SessionWatchEvent) {
        let _ = self.tx.send(SessionWatchEnvelope {
            session_id: session_id.to_string(),
            event,
        });
    }

    fn subscribe(&self) -> broadcast::Receiver<SessionWatchEnvelope> {
        self.tx.subscribe()
    }
}

#[derive(Clone)]
pub struct SessionWatchService {
    query: Arc<SessionQueryService>,
    effect_ledger: Arc<dyn EffectLedgerPort>,
    hub: Arc<SessionWatchHub>,
}

impl SessionWatchService {
    pub fn new(
        query: Arc<SessionQueryService>,
        effect_ledger: Arc<dyn EffectLedgerPort>,
        hub: Arc<SessionWatchHub>,
    ) -> Self {
        Self {
            query,
            effect_ledger,
            hub,
        }
    }

    pub fn hub(&self) -> Arc<SessionWatchHub> {
        self.hub.clone()
    }

    pub async fn get_session_watch_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionWatchSnapshot>, String> {
        projection::get_session_watch_snapshot(
            self.query.as_ref(),
            self.effect_ledger.as_ref(),
            session_id,
        )
        .await
    }

    pub async fn watch_session(
        &self,
        session_id: &str,
    ) -> Result<SessionWatchStream, SessionWatchError> {
        let snapshot = self
            .get_session_watch_snapshot(session_id)
            .await
            .map_err(SessionWatchError::Internal)?
            .ok_or(SessionWatchError::NotFound)?;
        let session_id = session_id.to_string();
        let latest_seq = snapshot.latest_seq;
        let mut rx = self.hub.subscribe();

        Ok(Box::pin(try_stream! {
            yield SessionWatchEvent::Connected(SessionWatchConnected {
                session_id: session_id.clone(),
                latest_seq,
            });

            loop {
                match rx.recv().await {
                    Ok(envelope) if envelope.session_id == session_id => yield envelope.event,
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }))
    }
}
