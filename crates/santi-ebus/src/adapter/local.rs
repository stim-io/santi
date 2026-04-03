use std::sync::{Arc, RwLock};

use santi_core::port::ebus::SubscriberSetPort;

#[derive(Clone, Default)]
pub struct InMemorySubscriberSet<S> {
    current: Arc<RwLock<Vec<S>>>,
}

impl<S> InMemorySubscriberSet<S> {
    pub fn new() -> Self {
        Self {
            current: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl<S> SubscriberSetPort<S> for InMemorySubscriberSet<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn replace_all(&self, subscribers: Vec<S>) {
        *self.current.write().expect("subscriber set poisoned") = subscribers;
    }

    fn snapshot(&self) -> Vec<S> {
        self.current
            .read()
            .expect("subscriber set poisoned")
            .clone()
    }
}
