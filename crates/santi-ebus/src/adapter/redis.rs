use redis::Client;

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
