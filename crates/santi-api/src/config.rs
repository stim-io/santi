use std::{env, net::SocketAddr};

use santi_core::hook::HookSpecSource;

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

        let launch_profile = optional_env("SANTI_LAUNCH_PROFILE");
        let provider_api = ProviderApi::from_env_value(
            env::var("SANTI_PROVIDER_API").unwrap_or_else(|_| "responses".to_string()),
        )?;

        let execution_root = env::var("EXECUTION_ROOT").unwrap_or_else(|_| "/app".to_string());

        let runtime_root =
            env::var("RUNTIME_ROOT").unwrap_or_else(|_| "/tmp/santi-runtime".to_string());

        let openai_api_key =
            env::var("OPENAI_API_KEY").map_err(|_| "missing OPENAI_API_KEY".to_string())?;
        let openai_base_url =
            env::var("OPENAI_BASE_URL").map_err(|_| "missing OPENAI_BASE_URL".to_string())?;
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
            provider_gateway_base_url: Some(redact_url_for_runtime_fact(&self.openai_base_url)),
        }
    }
}

impl Mode {
    fn as_str(&self) -> &'static str {
        match self {
            Mode::Distributed => "distributed",
            Mode::Standalone => "standalone",
        }
    }
}

impl ProviderApi {
    fn from_env_value(value: String) -> Result<Self, String> {
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

fn redact_url_for_runtime_fact(value: &str) -> String {
    let without_fragment = value.split_once('#').map(|(base, _)| base).unwrap_or(value);
    let without_query = without_fragment
        .split_once('?')
        .map(|(base, _)| base)
        .unwrap_or(without_fragment);

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

fn parse_hook_source_json(raw: &str) -> Result<HookSpecSource, String> {
    HookSpecSource::from_json_str(raw).map_err(|err| format!("parse HOOK_SPECS_JSON failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{redact_url_for_runtime_fact, ProviderApi};

    #[test]
    fn runtime_fact_url_redaction_removes_credentials_query_and_fragment() {
        assert_eq!(
            redact_url_for_runtime_fact(
                "https://user:secret@example.test/openai/v1?token=abc#frag"
            ),
            "https://example.test/openai/v1"
        );
    }

    #[test]
    fn runtime_fact_url_redaction_keeps_plain_local_gateway_url() {
        assert_eq!(
            redact_url_for_runtime_fact("http://127.0.0.1:18082/openai/v1"),
            "http://127.0.0.1:18082/openai/v1"
        );
    }

    #[test]
    fn provider_api_accepts_deepseek_chat_alias() {
        assert_eq!(
            ProviderApi::from_env_value("deepseek".to_string()).unwrap(),
            ProviderApi::ChatCompletions
        );
        assert_eq!(
            ProviderApi::from_env_value("responses".to_string()).unwrap(),
            ProviderApi::Responses
        );
    }
}
