use std::{future::Future, sync::{atomic::{AtomicBool, Ordering}, Arc}, time::Duration};

use redis::{aio::MultiplexedConnection, Client, Script};
use tokio::{sync::oneshot, time::{sleep, timeout}};
use tracing::{debug, warn};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RedisLockConfig {
    pub ttl: Duration,
    pub renew_interval: Duration,
    pub acquire_timeout: Duration,
    pub key_prefix: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LockContext {
    pub key: String,
    pub token: String,
    pub ttl: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum RedisLockError {
    #[error("lock busy: {key}")]
    Busy { key: String },

    #[error("redis unavailable: {message}")]
    Redis { message: String },

    #[error("lock lost while running: {key}")]
    Lost { key: String },

    #[error("lock release failed: {key}: {message}")]
    Release { key: String, message: String },

    #[error("invalid lock config: {message}")]
    InvalidConfig { message: String },
}

#[derive(Clone)]
pub struct RedisLockClient {
    client: Client,
    config: RedisLockConfig,
}

impl RedisLockClient {
    pub async fn new(redis_url: &str, config: RedisLockConfig) -> Result<Self, RedisLockError> {
        validate_config(&config)?;
        let client = Client::open(redis_url).map_err(|err| RedisLockError::Redis {
            message: err.to_string(),
        })?;

        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|err| RedisLockError::Redis {
                message: err.to_string(),
            })?;
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|err| RedisLockError::Redis {
                message: err.to_string(),
            })?;

        Ok(Self { client, config })
    }

    pub async fn with_lock<F, Fut, T>(
        &self,
        key: impl Into<String>,
        f: F,
    ) -> Result<T, RedisLockError>
    where
        F: FnOnce(LockContext) -> Fut,
        Fut: Future<Output = Result<T, RedisLockError>>,
    {
        let key = self.lock_key(key.into());
        let token = format!("lock_{}", Uuid::new_v4().simple());
        let token_for_release = token.clone();
        let ttl_ms = self.config.ttl.as_millis().min(u64::MAX as u128) as u64;

        debug!(lock_key = %key, ttl_ms, "lock acquire start");

        let acquired = timeout(
            self.config.acquire_timeout,
            self.try_acquire(&key, &token, ttl_ms),
        )
        .await
        .map_err(|_| RedisLockError::Busy { key: key.clone() })??;

        if !acquired {
            return Err(RedisLockError::Busy { key });
        }

        debug!(lock_key = %key, ttl_ms, "lock acquire success");

        let lost = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let renew_client = self.client.clone();
        let renew_key = key.clone();
        let renew_token = token.clone();
        let renew_interval = self.config.renew_interval;
        let lost_flag = lost.clone();

        let renew_task = tokio::spawn(async move {
            renew_loop(
                renew_client,
                renew_key,
                renew_token,
                ttl_ms,
                renew_interval,
                lost_flag,
                shutdown_rx,
            )
            .await;
        });

        let context = LockContext {
            key: key.clone(),
            token,
            ttl: self.config.ttl,
        };

        let result = f(context).await;

        let _ = shutdown_tx.send(());
        let _ = renew_task.await;

        if lost.load(Ordering::SeqCst) {
            return Err(RedisLockError::Lost { key });
        }

        match self.release(&key, &token_for_release).await {
            Ok(()) => debug!(lock_key = %key, "lock release success"),
            Err(err) => return Err(err),
        }

        result
    }

    fn lock_key(&self, key: String) -> String {
        if let Some(prefix) = &self.config.key_prefix {
            if key.starts_with(prefix) {
                key
            } else {
                format!("{}:{}", prefix.trim_end_matches(':'), key)
            }
        } else {
            key
        }
    }

    async fn try_acquire(&self, key: &str, token: &str, ttl_ms: u64) -> Result<bool, RedisLockError> {
        let mut conn = self.connection().await?;
        redis::cmd("SET")
            .arg(key)
            .arg(token)
            .arg("NX")
            .arg("PX")
            .arg(ttl_ms)
            .query_async::<Option<String>>(&mut conn)
            .await
            .map(|value| value.is_some())
            .map_err(|err| RedisLockError::Redis {
                message: err.to_string(),
            })
    }

    async fn release(&self, key: &str, token: &str) -> Result<(), RedisLockError> {
        let mut conn = self.connection().await?;
        let removed: i32 = release_script()
            .key(key)
            .arg(token)
            .invoke_async(&mut conn)
            .await
            .map_err(|err| RedisLockError::Release {
                key: key.to_string(),
                message: err.to_string(),
            })?;

        if removed == 0 {
            return Err(RedisLockError::Release {
                key: key.to_string(),
                message: "lock not owned by current holder".to_string(),
            });
        }

        Ok(())
    }

    async fn connection(&self) -> Result<MultiplexedConnection, RedisLockError> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|err| RedisLockError::Redis {
                message: err.to_string(),
            })
    }
}

fn validate_config(config: &RedisLockConfig) -> Result<(), RedisLockError> {
    if config.ttl.is_zero() {
        return Err(RedisLockError::InvalidConfig {
            message: "ttl must be greater than zero".to_string(),
        });
    }
    if config.renew_interval.is_zero() {
        return Err(RedisLockError::InvalidConfig {
            message: "renew_interval must be greater than zero".to_string(),
        });
    }
    if config.acquire_timeout.is_zero() {
        return Err(RedisLockError::InvalidConfig {
            message: "acquire_timeout must be greater than zero".to_string(),
        });
    }
    if config.renew_interval >= config.ttl {
        return Err(RedisLockError::InvalidConfig {
            message: "renew_interval must be smaller than ttl".to_string(),
        });
    }
    Ok(())
}

async fn renew_loop(
    client: Client,
    key: String,
    token: String,
    ttl_ms: u64,
    renew_interval: Duration,
    lost: Arc<AtomicBool>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                return;
            }
            _ = sleep(renew_interval) => {}
        }

        let renew_result = async {
            let mut conn = client
                .get_multiplexed_async_connection()
                .await
                .map_err(|err| err.to_string())?;
            let renewed: i32 = renew_script()
                .key(&key)
                .arg(&token)
                .arg(ttl_ms)
                .invoke_async(&mut conn)
                .await
                .map_err(|err| err.to_string())?;
            Ok::<i32, String>(renewed)
        }
        .await;

        match renew_result {
            Ok(1) => {
                debug!(lock_key = %key, ttl_ms, "lock renew success");
            }
            Ok(_) => {
                lost.store(true, Ordering::SeqCst);
                warn!(lock_key = %key, "lock renew lost ownership");
                return;
            }
            Err(message) => {
                lost.store(true, Ordering::SeqCst);
                warn!(lock_key = %key, error = %message, "lock renew failed");
                return;
            }
        }
    }
}

fn release_script() -> Script {
    Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("DEL", KEYS[1])
        else
            return 0
        end
        "#,
    )
}

fn renew_script() -> Script {
    Script::new(
        r#"
        if redis.call("GET", KEYS[1]) == ARGV[1] then
            return redis.call("PEXPIRE", KEYS[1], ARGV[2])
        else
            return 0
        end
        "#,
    )
}
