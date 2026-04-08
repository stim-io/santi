use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use santi_core::{
    error::LockError,
    port::lock::{Lock, LockGuard},
};

#[derive(Clone, Default)]
pub struct InProcessLock {
    held: Arc<Mutex<HashSet<String>>>,
}

pub struct InProcessLockGuard {
    key: String,
    held: Arc<Mutex<HashSet<String>>>,
}

#[async_trait::async_trait]
impl Lock for InProcessLock {
    async fn acquire(
        &self,
        key: &str,
    ) -> std::result::Result<Box<dyn LockGuard + Send>, LockError> {
        let mut held = self.held.lock().map_err(|_| LockError::Backend {
            message: "in-process lock poisoned".to_string(),
        })?;
        if held.contains(key) {
            return Err(LockError::Busy);
        }
        held.insert(key.to_string());
        Ok(Box::new(InProcessLockGuard {
            key: key.to_string(),
            held: self.held.clone(),
        }))
    }
}

#[async_trait::async_trait]
impl LockGuard for InProcessLockGuard {
    async fn release(self: Box<Self>) -> std::result::Result<(), LockError> {
        let mut held = self.held.lock().map_err(|_| LockError::Backend {
            message: "in-process lock poisoned".to_string(),
        })?;
        held.remove(&self.key);
        Ok(())
    }
}
