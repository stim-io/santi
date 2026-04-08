use std::{env, net::SocketAddr};

use santi_core::hook::HookSpecSource;

#[derive(Clone, Debug)]
pub struct Config {
    pub mode: Mode,
    pub bind_addr: SocketAddr,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_model: String,
    pub database_url: String,
    pub redis_url: String,
    pub standalone_sqlite_path: String,
    pub execution_root: String,
    pub runtime_root: String,
    pub hook_source: Option<HookSpecSource>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Distributed,
    Standalone,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let mode_raw = env::var("MODE").unwrap_or_else(|_| "distributed".to_string());
        let mode = match mode_raw.to_lowercase().as_str() {
            "distributed" => Mode::Distributed,
            "standalone" => Mode::Standalone,
            _ => return Err(format!("invalid MODE: {mode_raw}")),
        };

        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .map_err(|err| format!("invalid BIND_ADDR: {err}"))?;

        let execution_root = env::var("EXECUTION_ROOT").unwrap_or_else(|_| "/app".to_string());

        let runtime_root =
            env::var("RUNTIME_ROOT").unwrap_or_else(|_| "/tmp/santi-runtime".to_string());

        let (openai_api_key, openai_base_url, openai_model, database_url, redis_url) = match mode {
            Mode::Distributed => (
                env::var("OPENAI_API_KEY").map_err(|_| "missing OPENAI_API_KEY".to_string())?,
                env::var("OPENAI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
                env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.4".to_string()),
                env::var("DATABASE_URL").unwrap_or_else(|_| {
                    "postgres://santi:santi@postgres:5432/santi?sslmode=disable".to_string()
                }),
                env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379/0".to_string()),
            ),
            Mode::Standalone => (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ),
        };
        let standalone_sqlite_path = env::var("STANDALONE_SQLITE_PATH")
            .unwrap_or_else(|_| "./santi-standalone.sqlite".to_string());

        let hook_source = env::var("HOOK_SPECS_JSON")
            .ok()
            .filter(|raw| !raw.trim().is_empty())
            .map(|raw| parse_hook_source_json(&raw))
            .transpose()?
            .or_else(|| {
                env::var("HOOK_SPECS_FILE")
                    .ok()
                    .filter(|raw| !raw.trim().is_empty())
                    .map(|path| HookSpecSource::Path { path })
            })
            .or_else(|| {
                env::var("HOOK_SPECS_URL")
                    .ok()
                    .filter(|raw| !raw.trim().is_empty())
                    .map(|url| HookSpecSource::Url { url })
            });

        Ok(Self {
            mode,
            bind_addr,
            openai_api_key,
            openai_base_url,
            openai_model,
            database_url,
            redis_url,
            standalone_sqlite_path,
            execution_root,
            runtime_root,
            hook_source,
        })
    }
}

fn parse_hook_source_json(raw: &str) -> Result<HookSpecSource, String> {
    HookSpecSource::from_json_str(raw).map_err(|err| format!("parse HOOK_SPECS_JSON failed: {err}"))
}
