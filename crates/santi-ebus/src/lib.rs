use std::sync::{Arc, RwLock};

use redis::Client;
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

#[derive(Clone)]
pub struct RedisEbusConfig {
    pub channel_prefix: Option<String>,
}

#[derive(Clone)]
pub struct RedisEbusClient {
    client: Client,
    config: RedisEbusConfig,
}

impl RedisEbusClient {
    pub fn new(redis_url: &str, config: RedisEbusConfig) -> Result<Self, redis::RedisError> {
        Ok(Self {
            client: Client::open(redis_url)?,
            config,
        })
    }

    pub async fn publish_signal(
        &self,
        topic: &str,
        payload: &str,
    ) -> Result<(), redis::RedisError> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let channel = match &self.config.channel_prefix {
            Some(prefix) => format!("{prefix}:{topic}"),
            None => topic.to_string(),
        };
        let _: () = redis::cmd("PUBLISH")
            .arg(channel)
            .arg(payload)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::InMemorySubscriberSet;
    use santi_core::port::ebus::SubscriberSetPort;

    #[test]
    fn in_memory_subscriber_set_replaces_and_snapshots() {
        let set = InMemorySubscriberSet::new();
        set.replace_all(vec![1, 2, 3]);
        assert_eq!(set.snapshot(), vec![1, 2, 3]);

        set.replace_all(vec![4]);
        assert_eq!(set.snapshot(), vec![4]);
    }
}
