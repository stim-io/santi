use crate::error::LockError;

#[async_trait::async_trait]
pub trait LockGuard: Send {
    async fn release(self: Box<Self>) -> std::result::Result<(), LockError>;
}

#[async_trait::async_trait]
pub trait Lock: Send + Sync {
    async fn acquire(&self, key: &str)
        -> std::result::Result<Box<dyn LockGuard + Send>, LockError>;
}
