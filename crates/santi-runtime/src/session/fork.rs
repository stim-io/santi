use std::sync::Arc;

use santi_core::{
    error::{Error, LockError},
    port::{
        lock::Lock, soul_session_fork::SoulSessionForkPort,
        soul_session_query::SoulSessionQueryPort,
    },
};
use uuid::Uuid;

use crate::session::watch::{
    SessionWatchActivityChanged, SessionWatchActivityKind, SessionWatchActivityState,
    SessionWatchEvent, SessionWatchHub,
};

#[derive(Clone)]
pub struct SessionForkService {
    lock: Arc<dyn Lock>,
    soul_session_query: Arc<dyn SoulSessionQueryPort>,
    soul_session_fork: Arc<dyn SoulSessionForkPort>,
    watch: Arc<SessionWatchHub>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkResult {
    pub new_session_id: String,
    pub parent_session_id: String,
    pub fork_point: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForkError {
    Busy,
    ParentNotFound,
    InvalidForkPoint(String),
    Internal(String),
}

impl SessionForkService {
    pub fn new(
        lock: Arc<dyn Lock>,
        soul_session_query: Arc<dyn SoulSessionQueryPort>,
        soul_session_fork: Arc<dyn SoulSessionForkPort>,
        watch: Arc<SessionWatchHub>,
    ) -> Self {
        Self {
            lock,
            soul_session_query,
            soul_session_fork,
            watch,
        }
    }

    pub async fn fork_session(
        &self,
        parent_session_id: String,
        fork_point: i64,
        request_id: String,
    ) -> std::result::Result<ForkResult, ForkError> {
        let lock_key = format!("lock:session_send:{}", parent_session_id);
        let guard = self.lock.acquire(&lock_key).await.map_err(map_lock_error)?;

        let parent_soul_session = self
            .soul_session_query
            .get_session_soul(&parent_session_id)
            .await
            .map_err(map_core_error)?
            .ok_or(ForkError::ParentNotFound)?;

        let namespace = Uuid::NAMESPACE_OID;
        let hash_input = format!(
            "santi_fork:{}:{}:{}",
            parent_session_id, fork_point, request_id
        );
        let new_session_id = format!(
            "sess_{}",
            Uuid::new_v5(&namespace, hash_input.as_bytes()).simple()
        );

        if let Some(existing) = self
            .soul_session_query
            .get_session_soul(&new_session_id)
            .await
            .map_err(map_core_error)?
        {
            if existing.parent_soul_session_id.as_deref() == Some(parent_soul_session.id.as_str())
                && existing.fork_point == Some(fork_point)
            {
                guard.release().await.map_err(map_lock_error)?;
                return Ok(ForkResult {
                    new_session_id,
                    parent_session_id,
                    fork_point,
                });
            }

            guard.release().await.map_err(map_lock_error)?;
            return Err(ForkError::Internal(
                "existing fork session id collided with incompatible lineage".to_string(),
            ));
        }

        if fork_point < 1 || fork_point >= parent_soul_session.next_seq {
            guard.release().await.map_err(map_lock_error)?;
            return Err(ForkError::InvalidForkPoint(format!(
                "illegal fork_point {}: must be 1 <= fp < {}",
                fork_point, parent_soul_session.next_seq
            )));
        }

        let new_soul_session_id = format!("ss_{}", Uuid::new_v4().simple());

        self.watch.publish(
            &parent_session_id,
            SessionWatchEvent::ActivityChanged(SessionWatchActivityChanged {
                session_id: parent_session_id.clone(),
                activity: SessionWatchActivityKind::Fork,
                state: SessionWatchActivityState::Started,
                label: None,
            }),
        );

        self.soul_session_fork
            .fork_soul_session(
                &parent_soul_session.id,
                fork_point,
                &new_soul_session_id,
                &new_session_id,
            )
            .await
            .map_err(map_core_error)?;

        guard.release().await.map_err(map_lock_error)?;

        self.watch.publish(
            &parent_session_id,
            SessionWatchEvent::ActivityChanged(SessionWatchActivityChanged {
                session_id: parent_session_id.clone(),
                activity: SessionWatchActivityKind::Fork,
                state: SessionWatchActivityState::Completed,
                label: Some(new_session_id.clone()),
            }),
        );

        Ok(ForkResult {
            new_session_id,
            parent_session_id,
            fork_point,
        })
    }
}

fn map_core_error(err: Error) -> ForkError {
    match err {
        Error::NotFound { resource: _ } => ForkError::ParentNotFound,
        Error::Busy { resource } => ForkError::Internal(format!("{resource} busy")),
        Error::InvalidInput { message } => ForkError::InvalidForkPoint(message),
        Error::Upstream { message } | Error::Internal { message } => ForkError::Internal(message),
    }
}

fn map_lock_error(err: LockError) -> ForkError {
    match err {
        LockError::Busy => ForkError::Busy,
        LockError::Lost => ForkError::Internal("fork session lock lost".to_string()),
        LockError::Backend { message } => ForkError::Internal(message),
    }
}
