use crate::error::LockError;

#[async_trait::async_trait]
pub trait LockGuard: Send {
    async fn release(self) -> std::result::Result<(), LockError>;
}

#[async_trait::async_trait]
pub trait Lock {
    type Guard: LockGuard + Send;

    async fn acquire(&self, key: &str) -> std::result::Result<Self::Guard, LockError>;
}
