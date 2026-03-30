use std::{env, fs, path::PathBuf};

use dirs::home_dir;
use santi_runtime::hooks::{HookSpec, HookSpecSource};
use serde::Deserialize;

use crate::cli::{BackendKind, Cli};

#[derive(Clone, Debug)]
pub struct Config {
    pub backend: BackendKind,
    pub base_url: String,
    pub database_url: String,
    pub redis_url: String,
    pub runtime_root: String,
    pub execution_root: String,
    pub openai_api_key: Option<String>,
    pub openai_base_url: String,
    pub openai_model: String,
    pub hook_source: Option<HookSpecSource>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ConfigFile {
    backend: Option<BackendKind>,
    base_url: Option<String>,
    database_url: Option<String>,
    redis_url: Option<String>,
    runtime_root: Option<String>,
    execution_root: Option<String>,
    openai_api_key: Option<String>,
    openai_base_url: Option<String>,
    openai_model: Option<String>,
    hooks: Option<Vec<HookSpec>>,
    hooks_file: Option<String>,
    hooks_url: Option<String>,
}

impl Config {
    pub fn from_env_and_cli(cli: &Cli) -> Result<Self, String> {
        let home_dir = env::var("SANTI_CLI_HOME").unwrap_or_else(|_| {
            home_dir()
                .map(|path| path.join(".santi-cli").display().to_string())
                .unwrap_or_else(|| ".santi-cli".to_string())
        });
        let config_file =
            env::var("SANTI_CLI_CONFIG_FILE").unwrap_or_else(|_| format!("{home_dir}/config.json"));
        let file_config = read_config_file(&config_file)?;

        let backend = cli
            .backend
            .or_else(|| {
                env::var("SANTI_CLI_BACKEND")
                    .ok()
                    .and_then(parse_backend_kind)
            })
            .or(file_config.backend)
            .unwrap_or(BackendKind::Local);

        let base_url = cli
            .base_url
            .clone()
            .or_else(|| env::var("SANTI_CLI_BASE_URL").ok())
            .or(file_config.base_url.clone())
            .unwrap_or_else(|| "http://127.0.0.1:18081".to_string());

        let database_url = env::var("SANTI_CLI_DATABASE_URL")
            .ok()
            .or(file_config.database_url.clone())
            .unwrap_or_else(|| {
                "postgres://santi:santi@127.0.0.1:15432/santi?sslmode=disable".to_string()
            });

        let redis_url = env::var("SANTI_CLI_REDIS_URL")
            .ok()
            .or(file_config.redis_url.clone())
            .unwrap_or_else(|| "redis://127.0.0.1:16379/0".to_string());

        let runtime_root = env::var("SANTI_CLI_RUNTIME_ROOT")
            .ok()
            .or(file_config.runtime_root.clone())
            .unwrap_or_else(|| format!("{home_dir}/runtime"));
        let execution_root = env::var("SANTI_CLI_EXECUTION_ROOT")
            .ok()
            .or(file_config.execution_root.clone())
            .unwrap_or_else(default_execution_root);
        let openai_api_key = env::var("SANTI_CLI_OPENAI_API_KEY")
            .ok()
            .or(file_config.openai_api_key.clone());
        let openai_base_url = env::var("SANTI_CLI_OPENAI_BASE_URL")
            .ok()
            .or(file_config.openai_base_url.clone())
            .unwrap_or_else(|| "http://127.0.0.1:18082/openai/v1".to_string());
        let openai_model = env::var("SANTI_CLI_OPENAI_MODEL")
            .ok()
            .or(file_config.openai_model.clone())
            .unwrap_or_else(|| "gpt-5.4".to_string());
        let hook_source = env::var("SANTI_CLI_HOOKS_JSON")
            .ok()
            .filter(|raw| !raw.trim().is_empty())
            .map(|raw| parse_hook_source_json(&raw, "SANTI_CLI_HOOKS_JSON"))
            .transpose()?
            .or_else(|| {
                env::var("SANTI_CLI_HOOKS_FILE")
                    .ok()
                    .filter(|raw| !raw.trim().is_empty())
                    .map(|path| HookSpecSource::Path { path })
            })
            .or_else(|| {
                env::var("SANTI_CLI_HOOKS_URL")
                    .ok()
                    .filter(|raw| !raw.trim().is_empty())
                    .map(|url| HookSpecSource::Url { url })
            })
            .or_else(|| {
                file_config
                    .hooks
                    .clone()
                    .map(|hooks| HookSpecSource::Value { hooks })
            })
            .or_else(|| {
                file_config
                    .hooks_file
                    .clone()
                    .map(|path| HookSpecSource::Path { path })
            })
            .or_else(|| {
                file_config
                    .hooks_url
                    .clone()
                    .map(|url| HookSpecSource::Url { url })
            });

        Ok(Self {
            backend,
            base_url,
            database_url,
            redis_url,
            runtime_root,
            execution_root,
            openai_api_key,
            openai_base_url,
            openai_model,
            hook_source,
        })
    }
}

fn parse_hook_source_json(raw: &str, source: &str) -> Result<HookSpecSource, String> {
    HookSpecSource::from_json_str(raw)
        .map_err(|err| format!("parse hooks failed ({source}): {err}"))
}

fn parse_backend_kind(raw: String) -> Option<BackendKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "local" => Some(BackendKind::Local),
        "api" => Some(BackendKind::Api),
        _ => None,
    }
}

fn read_config_file(path: &str) -> Result<ConfigFile, String> {
    let path_buf = PathBuf::from(path);
    if !path_buf.exists() {
        return Ok(ConfigFile::default());
    }

    let raw = fs::read_to_string(&path_buf)
        .map_err(|err| format!("read config file failed ({}): {err}", path_buf.display()))?;
    serde_json::from_str::<ConfigFile>(&raw)
        .map_err(|err| format!("parse config file failed ({}): {err}", path_buf.display()))
}

fn default_execution_root() -> String {
    env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| ".".to_string())
}
