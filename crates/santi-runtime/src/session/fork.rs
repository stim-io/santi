use std::sync::Arc;

use santi_core::{
    error::{Error, LockError},
    port::{
        lock::Lock, soul_session_fork::SoulSessionForkPort,
        soul_session_query::SoulSessionQueryPort,
    },
};
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionForkService {
    lock: Arc<dyn Lock>,
    soul_session_query: Arc<dyn SoulSessionQueryPort>,
    soul_session_fork: Arc<dyn SoulSessionForkPort>,
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
    ) -> Self {
        Self {
            lock,
            soul_session_query,
            soul_session_fork,
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
            .get_soul_session_by_session_id(&parent_session_id)
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
            .get_soul_session_by_session_id(&new_session_id)
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use santi_core::{
        error::LockError,
        model::runtime::SoulSession,
        port::{
            lock::{Lock, LockGuard},
            soul_runtime::{
                AppendMessageRef, AppendToolCall, AppendToolResult, CompleteTurn, FailTurn,
                SoulRuntimePort, StartTurn,
            },
            soul_session_fork::SoulSessionForkPort,
            soul_session_query::SoulSessionQueryPort,
        },
    };
    use uuid::Uuid;

    use super::{ForkError, SessionForkService};

    #[derive(Default)]
    struct FakeLock {
        released: Arc<Mutex<usize>>,
    }

    struct FakeGuard {
        released: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl LockGuard for FakeGuard {
        async fn release(self: Box<Self>) -> std::result::Result<(), LockError> {
            *self.released.lock().expect("poisoned") += 1;
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl Lock for FakeLock {
        async fn acquire(
            &self,
            _key: &str,
        ) -> std::result::Result<Box<dyn LockGuard + Send>, LockError> {
            Ok(Box::new(FakeGuard {
                released: self.released.clone(),
            }))
        }
    }

    #[derive(Clone)]
    struct FakeSoulRuntime {
        parent: Option<SoulSession>,
        existing_child: Option<SoulSession>,
    }

    #[derive(Clone)]
    struct FakeSoulSessionFork {
        fork_calls: Arc<Mutex<Vec<(String, i64, String, String)>>>,
    }

    impl FakeSoulRuntime {
        fn new(parent: Option<SoulSession>, existing_child: Option<SoulSession>) -> Self {
            Self {
                parent,
                existing_child,
            }
        }
    }

    impl FakeSoulSessionFork {
        fn new() -> Self {
            Self {
                fork_calls: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl SoulRuntimePort for FakeSoulRuntime {
        async fn acquire_soul_session(
            &self,
            _input: santi_core::port::soul_runtime::AcquireSoulSession,
        ) -> santi_core::error::Result<SoulSession> {
            unimplemented!()
        }

        async fn get_soul_session(
            &self,
            _soul_session_id: &str,
        ) -> santi_core::error::Result<Option<SoulSession>> {
            Ok(None)
        }

        async fn write_session_memory(
            &self,
            _soul_session_id: &str,
            _text: &str,
        ) -> santi_core::error::Result<Option<SoulSession>> {
            unimplemented!()
        }

        async fn start_turn(
            &self,
            _input: StartTurn,
        ) -> santi_core::error::Result<santi_core::model::runtime::Turn> {
            unimplemented!()
        }

        async fn append_message_ref(
            &self,
            _input: AppendMessageRef,
        ) -> santi_core::error::Result<santi_core::model::runtime::AssemblyItem> {
            unimplemented!()
        }

        async fn append_tool_call(
            &self,
            _input: AppendToolCall,
        ) -> santi_core::error::Result<santi_core::model::runtime::AssemblyItem> {
            unimplemented!()
        }

        async fn append_tool_result(
            &self,
            _input: AppendToolResult,
        ) -> santi_core::error::Result<santi_core::model::runtime::AssemblyItem> {
            unimplemented!()
        }

        async fn complete_turn(
            &self,
            _input: CompleteTurn,
        ) -> santi_core::error::Result<santi_core::model::runtime::Turn> {
            unimplemented!()
        }

        async fn fail_turn(
            &self,
            _input: FailTurn,
        ) -> santi_core::error::Result<santi_core::model::runtime::Turn> {
            unimplemented!()
        }
    }

    #[async_trait::async_trait]
    impl SoulSessionQueryPort for FakeSoulRuntime {
        async fn get_soul_session_by_session_id(
            &self,
            session_id: &str,
        ) -> santi_core::error::Result<Option<SoulSession>> {
            if let Some(existing_child) = &self.existing_child {
                if existing_child.session_id == session_id {
                    return Ok(Some(existing_child.clone()));
                }
            }
            if let Some(parent) = &self.parent {
                if parent.session_id == session_id {
                    return Ok(Some(parent.clone()));
                }
            }
            Ok(None)
        }
    }

    #[async_trait::async_trait]
    impl SoulSessionForkPort for FakeSoulSessionFork {
        async fn fork_soul_session(
            &self,
            parent_soul_session_id: &str,
            fork_point: i64,
            new_soul_session_id: &str,
            new_session_id: &str,
        ) -> santi_core::error::Result<SoulSession> {
            self.fork_calls.lock().expect("poisoned").push((
                parent_soul_session_id.to_string(),
                fork_point,
                new_soul_session_id.to_string(),
                new_session_id.to_string(),
            ));

            Ok(SoulSession {
                id: new_soul_session_id.to_string(),
                soul_id: "soul_default".to_string(),
                session_id: new_session_id.to_string(),
                session_memory: "memory".to_string(),
                provider_state: None,
                next_seq: fork_point + 1,
                last_seen_session_seq: fork_point,
                parent_soul_session_id: Some(parent_soul_session_id.to_string()),
                fork_point: Some(fork_point),
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
            })
        }
    }

    fn parent_session() -> SoulSession {
        SoulSession {
            id: "ss_parent".to_string(),
            soul_id: "soul_default".to_string(),
            session_id: "sess_parent".to_string(),
            session_memory: "memory".to_string(),
            provider_state: None,
            next_seq: 5,
            last_seen_session_seq: 4,
            parent_soul_session_id: None,
            fork_point: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[tokio::test]
    async fn returns_existing_idempotent_fork_result() {
        let parent = parent_session();
        let existing_child = SoulSession {
            id: "ss_child".to_string(),
            soul_id: "soul_default".to_string(),
            session_id: format!(
                "sess_{}",
                Uuid::new_v5(&Uuid::NAMESPACE_OID, b"santi_fork:sess_parent:3:req_1").simple()
            ),
            session_memory: "memory".to_string(),
            provider_state: None,
            next_seq: 4,
            last_seen_session_seq: 3,
            parent_soul_session_id: Some("ss_parent".to_string()),
            fork_point: Some(3),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        };
        let runtime = Arc::new(FakeSoulRuntime::new(
            Some(parent),
            Some(existing_child.clone()),
        ));
        let fork_port = Arc::new(FakeSoulSessionFork::new());
        let service = SessionForkService::new(
            Arc::new(FakeLock::default()),
            runtime.clone(),
            fork_port.clone(),
        );

        let result = service
            .fork_session("sess_parent".to_string(), 3, "req_1".to_string())
            .await
            .expect("fork should succeed");

        assert_eq!(result.new_session_id, existing_child.session_id);
        assert!(fork_port.fork_calls.lock().expect("poisoned").is_empty());
    }

    #[tokio::test]
    async fn rejects_invalid_fork_point_before_copy() {
        let runtime = Arc::new(FakeSoulRuntime::new(Some(parent_session()), None));
        let fork_port = Arc::new(FakeSoulSessionFork::new());
        let service = SessionForkService::new(
            Arc::new(FakeLock::default()),
            runtime.clone(),
            fork_port.clone(),
        );

        let err = service
            .fork_session("sess_parent".to_string(), 5, "req_1".to_string())
            .await
            .expect_err("fork should fail");

        assert!(matches!(err, ForkError::InvalidForkPoint(_)));
        assert!(fork_port.fork_calls.lock().expect("poisoned").is_empty());
    }
}
