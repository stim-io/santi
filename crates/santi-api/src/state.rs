use std::sync::{Arc, RwLock};

use santi_core::port::session_ledger::SessionLedgerPort;
use santi_runtime::{runtime::context::RuntimeSelfFacts, session::send::SessionProviderConfig};

use crate::{
    chat_client::ChatCompletionsClient,
    config::{provider_health_url, redact_url_for_runtime_fact, Mode, ProviderApi},
    link_client::OpenAiResponsesClient,
    protocol_reply::ProtocolReplyStore,
    schema::admin::{
        ConfigApplyRequest, ConfigApplyResponse, ConfigApplyStatus, ConfigCurrentResponse,
    },
    schema::meta::{MetaProvider, MetaRuntime},
    surface::{AdminApi, ApiCapabilities, ApiError, SessionApi, SoulApi},
};

#[derive(Clone)]
pub struct AppState {
    meta: Arc<RwLock<AppMetaState>>,
    session_api: Arc<dyn SessionApi>,
    session_ledger: Arc<dyn SessionLedgerPort>,
    soul_api: Arc<dyn SoulApi>,
    admin_api: Arc<dyn AdminApi>,
    protocol_replies: Arc<ProtocolReplyStore>,
    standalone_bootstrap_lock: Option<Arc<std::fs::File>>,
}

#[derive(Clone)]
pub struct AppMetaState {
    pub mode: Mode,
    pub launch_profile: Option<String>,
    pub bind_addr: String,
    pub provider: MetaProvider,
    pub provider_probe_url: String,
    pub provider_probe_display_url: String,
    pub runtime: MetaRuntime,
    pub capabilities: ApiCapabilities,
    pub config_version: u64,
    pub config_source: String,
    pub last_config_event_id: String,
}

impl AppState {
    pub fn new(
        meta: AppMetaState,
        session_api: Arc<dyn SessionApi>,
        session_ledger: Arc<dyn SessionLedgerPort>,
        soul_api: Arc<dyn SoulApi>,
        admin_api: Arc<dyn AdminApi>,
        standalone_bootstrap_lock: Option<Arc<std::fs::File>>,
    ) -> Self {
        Self {
            meta: Arc::new(RwLock::new(meta)),
            session_api,
            session_ledger,
            soul_api,
            admin_api,
            protocol_replies: Arc::new(ProtocolReplyStore::default()),
            standalone_bootstrap_lock,
        }
    }

    pub fn mode(&self) -> Mode {
        self.meta
            .read()
            .expect("app meta lock poisoned")
            .mode
            .clone()
    }

    pub fn launch_profile(&self) -> Option<String> {
        self.meta
            .read()
            .expect("app meta lock poisoned")
            .launch_profile
            .clone()
    }

    pub fn bind_addr(&self) -> String {
        self.meta
            .read()
            .expect("app meta lock poisoned")
            .bind_addr
            .clone()
    }

    pub fn provider(&self) -> MetaProvider {
        self.meta
            .read()
            .expect("app meta lock poisoned")
            .provider
            .clone()
    }

    pub fn provider_probe_url(&self) -> String {
        self.meta
            .read()
            .expect("app meta lock poisoned")
            .provider_probe_url
            .clone()
    }

    pub fn provider_probe_display_url(&self) -> String {
        self.meta
            .read()
            .expect("app meta lock poisoned")
            .provider_probe_display_url
            .clone()
    }

    pub fn runtime(&self) -> MetaRuntime {
        self.meta
            .read()
            .expect("app meta lock poisoned")
            .runtime
            .clone()
    }

    pub fn capabilities(&self) -> ApiCapabilities {
        self.meta
            .read()
            .expect("app meta lock poisoned")
            .capabilities
            .clone()
    }

    pub fn current_config(&self) -> ConfigCurrentResponse {
        let meta = self.meta.read().expect("app meta lock poisoned");
        ConfigCurrentResponse {
            config_version: meta.config_version,
            last_event_id: meta.last_config_event_id.clone(),
            source: meta.config_source.clone(),
            launch_profile: meta.launch_profile.clone(),
            provider: meta.provider.clone(),
            runtime: meta.runtime.clone(),
        }
    }

    pub fn session_api(&self) -> Arc<dyn SessionApi> {
        self.session_api.clone()
    }

    pub fn session_ledger(&self) -> Arc<dyn SessionLedgerPort> {
        self.session_ledger.clone()
    }

    pub fn soul_api(&self) -> Arc<dyn SoulApi> {
        self.soul_api.clone()
    }

    pub fn admin_api(&self) -> Arc<dyn AdminApi> {
        self.admin_api.clone()
    }

    pub fn protocol_replies(&self) -> Arc<ProtocolReplyStore> {
        self.protocol_replies.clone()
    }

    pub fn standalone_bootstrap_lock(&self) -> Option<Arc<std::fs::File>> {
        self.standalone_bootstrap_lock.clone()
    }

    pub async fn apply_config(
        &self,
        request: ConfigApplyRequest,
    ) -> Result<ConfigApplyResponse, ApiError> {
        let provider = request
            .provider
            .ok_or_else(|| ApiError::Validation("provider config is required".into()))?;
        let provider_api =
            ProviderApi::from_env_value(provider.api.clone()).map_err(ApiError::Validation)?;

        validate_non_empty("provider.model", &provider.model)?;
        validate_non_empty("provider.gateway_base_url", &provider.gateway_base_url)?;
        validate_non_empty("provider.api_key", &provider.api_key)?;

        let launch_profile = request.launch_profile.or_else(|| self.launch_profile());
        let meta_provider = MetaProvider {
            api: provider_api.as_str().to_string(),
            model: provider.model.clone(),
            gateway_base_url: Some(redact_url_for_runtime_fact(&provider.gateway_base_url)),
        };
        let runtime = self.runtime();
        let session_provider_config = SessionProviderConfig {
            model: provider.model,
            provider: build_provider(
                provider_api,
                provider.api_key,
                provider.gateway_base_url.clone(),
            ),
            runtime_facts: RuntimeSelfFacts {
                service_name: "santi".to_string(),
                assembly_mode: self.mode().as_str().to_string(),
                launch_profile: launch_profile.clone(),
                bind_addr: Some(self.bind_addr()),
                provider_model: meta_provider.model.clone(),
                provider_api: meta_provider.api.clone(),
                provider_gateway_base_url: meta_provider.gateway_base_url.clone(),
            },
        };

        self.admin_api()
            .apply_provider_config(session_provider_config)
            .await?;

        let probe_url = provider_health_url(&provider.gateway_base_url);
        let probe_display_url = redact_url_for_runtime_fact(&probe_url);
        let event_id = config_event_id("config.applied");
        let config_version;
        {
            let mut meta = self.meta.write().expect("app meta lock poisoned");
            config_version = meta.config_version + 1;
            meta.launch_profile = launch_profile.clone();
            meta.provider = meta_provider.clone();
            meta.provider_probe_url = probe_url;
            meta.provider_probe_display_url = probe_display_url;
            meta.config_version = config_version;
            meta.config_source = "admin-apply".to_string();
            meta.last_config_event_id = event_id.clone();
        }

        Ok(ConfigApplyResponse {
            event_id,
            config_version,
            source: "admin-apply".to_string(),
            status: ConfigApplyStatus::Applied,
            launch_profile,
            provider: meta_provider,
            runtime,
            detail: Some("provider config applied for subsequent turns".into()),
        })
    }
}

fn build_provider(
    provider_api: ProviderApi,
    api_key: String,
    base_url: String,
) -> Arc<dyn santi_core::port::provider::Provider> {
    match provider_api {
        ProviderApi::Responses => Arc::new(OpenAiResponsesClient::new(api_key, base_url)),
        ProviderApi::ChatCompletions => Arc::new(ChatCompletionsClient::new(api_key, base_url)),
    }
}

fn validate_non_empty(name: &str, value: &str) -> Result<(), ApiError> {
    if value.trim().is_empty() {
        return Err(ApiError::Validation(format!("{name} must not be empty")));
    }
    Ok(())
}

fn config_event_id(prefix: &str) -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch");
    format!(
        "{}-{}-{:03}",
        prefix,
        duration.as_secs(),
        duration.subsec_millis()
    )
}
