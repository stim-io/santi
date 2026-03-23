use std::{env, net::SocketAddr};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_model: String,
    pub database_url: String,
    pub execution_root: String,
    pub runtime_root: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .map_err(|err| format!("invalid BIND_ADDR: {err}"))?;

        let openai_api_key =
            env::var("OPENAI_API_KEY").map_err(|_| "missing OPENAI_API_KEY".to_string())?;

        let openai_base_url =
            env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        let openai_model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.4".to_string());

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://santi:santi@postgres:5432/santi".to_string());

        let execution_root = env::var("EXECUTION_ROOT").unwrap_or_else(|_| "/app".to_string());

        let runtime_root =
            env::var("RUNTIME_ROOT").unwrap_or_else(|_| "/tmp/santi-runtime".to_string());

        Ok(Self {
            bind_addr,
            openai_api_key,
            openai_base_url,
            openai_model,
            database_url,
            execution_root,
            runtime_root,
        })
    }
}
