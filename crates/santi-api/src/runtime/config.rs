use std::{env, net::SocketAddr};

use santi_core::hook::HookSpecSource;

use crate::schema::meta::{MetaProvider, MetaRuntime};
use santi_runtime::runtime::tools::{
    DEFAULT_BASH_OUTPUT_HARD_BYTES, DEFAULT_BASH_OUTPUT_TRUNCATE_CHARS, DEFAULT_BASH_TIMEOUT_SECS,
};

#[derive(Clone, Debug)]
pub struct Config {
    pub mode: Mode,
    pub bind_addr: SocketAddr,
    pub launch_profile: Option<String>,
    pub provider_api: ProviderApi,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_model: String,
    pub database_url: String,
    pub redis_url: String,
    pub standalone_sqlite_path: String,
    pub execution_root: String,
    pub runtime_root: String,
    pub bash_timeout_secs: u64,
    pub bash_output_truncate_chars: usize,
    pub bash_output_hard_bytes: usize,
    pub hook_source: Option<HookSpecSource>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Distributed,
    Standalone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderApi {
    Responses,
    ChatCompletions,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let mode_raw = env::var("MODE").unwrap_or_else(|_| "standalone".to_string());
        let mode = match mode_raw.to_lowercase().as_str() {
            "distributed" => Mode::Distributed,
            "standalone" => Mode::Standalone,
            _ => return Err(format!("invalid MODE: {mode_raw}")),
        };

        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:18081".to_string())
            .parse()
            .map_err(|err| format!("invalid BIND_ADDR: {err}"))?;

        let launch_profile =
            optional_env("SANTI_LAUNCH_PROFILE").or_else(|| Some("local-foreground".to_string()));
        let provider_api = ProviderApi::from_env_value(
            env::var("SANTI_PROVIDER_API").unwrap_or_else(|_| "responses".to_string()),
        )?;

        let execution_root = env::var("EXECUTION_ROOT").unwrap_or_else(|_| ".".to_string());

        let runtime_root =
            env::var("RUNTIME_ROOT").unwrap_or_else(|_| "./.tmp/santi-runtime".to_string());
        let bash_timeout_secs =
            parse_env_u64("SANTI_BASH_TIMEOUT_SECS", DEFAULT_BASH_TIMEOUT_SECS)?;
        let bash_output_truncate_chars = parse_env_usize(
            "SANTI_BASH_OUTPUT_TRUNCATE_CHARS",
            DEFAULT_BASH_OUTPUT_TRUNCATE_CHARS,
        )?;
        let bash_output_hard_bytes = parse_env_usize(
            "SANTI_BASH_OUTPUT_HARD_BYTES",
            DEFAULT_BASH_OUTPUT_HARD_BYTES,
        )?;

        let openai_api_key =
            env::var("OPENAI_API_KEY").unwrap_or_else(|_| "codex-local-dev".to_string());
        let openai_base_url = env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:18082/openai/v1".to_string());
        let openai_model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.4".to_string());
        let (database_url, redis_url) = match mode {
            Mode::Distributed => (
                env::var("DATABASE_URL").unwrap_or_else(|_| {
                    "postgres://santi:santi@postgres:5432/santi?sslmode=disable".to_string()
                }),
                env::var("REDIS_URL").unwrap_or_else(|_| "redis://redis:6379/0".to_string()),
            ),
            Mode::Standalone => (String::new(), String::new()),
        };
        let standalone_sqlite_path = env::var("STANDALONE_SQLITE_PATH")
            .unwrap_or_else(|_| "./.tmp/santi-standalone.sqlite".to_string());

        let hook_source = env::var("HOOK_SPECS_JSON")
            .ok()
            .filter(|raw| !raw.trim().is_empty())
            .map(|raw| parse_hook_source_json(&raw))
            .transpose()?
            .or_else(|| optional_env("HOOK_SPECS_FILE").map(|path| HookSpecSource::Path { path }))
            .or_else(|| optional_env("HOOK_SPECS_URL").map(|url| HookSpecSource::Url { url }));

        Ok(Self {
            mode,
            bind_addr,
            launch_profile,
            provider_api,
            openai_api_key,
            openai_base_url,
            openai_model,
            database_url,
            redis_url,
            standalone_sqlite_path,
            execution_root,
            runtime_root,
            bash_timeout_secs,
            bash_output_truncate_chars,
            bash_output_hard_bytes,
            hook_source,
        })
    }

    pub fn runtime_self_facts(&self) -> santi_runtime::runtime::context::RuntimeSelfFacts {
        santi_runtime::runtime::context::RuntimeSelfFacts {
            service_name: "santi".to_string(),
            assembly_mode: self.mode.as_str().to_string(),
            launch_profile: self.launch_profile.clone(),
            bind_addr: Some(self.bind_addr.to_string()),
            provider_model: self.openai_model.clone(),
            provider_api: self.provider_api.as_str().to_string(),
            provider_gateway_base_url: Some(redact_runtime_url(&self.openai_base_url)),
        }
    }

    pub fn meta_provider(&self) -> MetaProvider {
        MetaProvider {
            api: self.provider_api.as_str().to_string(),
            model: self.openai_model.clone(),
            gateway_base_url: Some(redact_runtime_url(&self.openai_base_url)),
        }
    }

    pub fn meta_runtime(&self) -> MetaRuntime {
        MetaRuntime {
            execution_root: self.execution_root.clone(),
            runtime_root: self.runtime_root.clone(),
            standalone_sqlite_path: match self.mode {
                Mode::Standalone => Some(self.standalone_sqlite_path.clone()),
                Mode::Distributed => None,
            },
        }
    }

    pub fn provider_probe_url(&self) -> String {
        provider_health_url(&self.openai_base_url)
    }

    pub fn provider_probe_display_url(&self) -> String {
        redact_runtime_url(&self.provider_probe_url())
    }
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Distributed => "distributed",
            Mode::Standalone => "standalone",
        }
    }
}

impl ProviderApi {
    pub fn from_env_value(value: String) -> Result<Self, String> {
        match value.trim().to_lowercase().as_str() {
            "responses" | "openai-responses" | "openai_responses" => Ok(Self::Responses),
            "chat-completions" | "chat_completions" | "openai-chat" | "deepseek" => {
                Ok(Self::ChatCompletions)
            }
            other => Err(format!("invalid SANTI_PROVIDER_API: {other}")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat-completions",
        }
    }
}

fn optional_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_env_u64(key: &str, default: u64) -> Result<u64, String> {
    optional_env(key)
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|err| format!("invalid {key}: {err}"))
                .and_then(|value| {
                    if value == 0 {
                        Err(format!("{key} must be greater than zero"))
                    } else {
                        Ok(value)
                    }
                })
        })
        .unwrap_or(Ok(default))
}

fn parse_env_usize(key: &str, default: usize) -> Result<usize, String> {
    optional_env(key)
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|err| format!("invalid {key}: {err}"))
                .and_then(|value| {
                    if value == 0 {
                        Err(format!("{key} must be greater than zero"))
                    } else {
                        Ok(value)
                    }
                })
        })
        .unwrap_or(Ok(default))
}

pub(crate) fn redact_runtime_url(value: &str) -> String {
    let without_query = strip_query_and_fragment(value);

    let Some(scheme_end) = without_query.find("://") else {
        return without_query.to_string();
    };
    let authority_start = scheme_end + 3;
    let authority_end = without_query[authority_start..]
        .find('/')
        .map(|index| authority_start + index)
        .unwrap_or(without_query.len());
    let authority = &without_query[authority_start..authority_end];

    let Some(userinfo_end) = authority.rfind('@') else {
        return without_query.to_string();
    };

    format!(
        "{}{}",
        &without_query[..authority_start],
        &without_query[authority_start + userinfo_end + 1..]
    )
}

pub(crate) fn provider_health_url(base_url: &str) -> String {
    format!(
        "{}/health",
        strip_query_and_fragment(base_url).trim_end_matches('/')
    )
}

fn strip_query_and_fragment(value: &str) -> &str {
    let without_fragment = value.split_once('#').map(|(base, _)| base).unwrap_or(value);
    without_fragment
        .split_once('?')
        .map(|(base, _)| base)
        .unwrap_or(without_fragment)
}

fn parse_hook_source_json(raw: &str) -> Result<HookSpecSource, String> {
    HookSpecSource::from_json_str(raw).map_err(|err| format!("parse HOOK_SPECS_JSON failed: {err}"))
}
