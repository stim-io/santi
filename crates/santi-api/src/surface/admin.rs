use std::sync::Arc;

use async_trait::async_trait;
use santi_core::{hook::HookSpecSource, port::ebus::SubscriberSetPort};
use santi_runtime::{
    hooks::{compile_hook_specs, load_hook_specs, HookEvaluator},
    session::send::SessionSendService,
};

use super::error::ApiError;

#[async_trait]
pub trait AdminApi: Send + Sync {
    async fn reload_hooks_from_source(&self, source: HookSpecSource) -> Result<usize, ApiError>;
}

#[derive(Clone)]
pub struct DistributedAdminApi {
    pub send: Arc<SessionSendService>,
}

#[derive(Clone)]
pub struct StandaloneAdminApi {
    pub ebus: Arc<dyn SubscriberSetPort<Arc<dyn HookEvaluator>>>,
}

#[async_trait]
impl AdminApi for DistributedAdminApi {
    async fn reload_hooks_from_source(&self, source: HookSpecSource) -> Result<usize, ApiError> {
        let specs = santi_runtime::hooks::load_hook_specs(&source)
            .await
            .map_err(ApiError::BadRequest)?;
        Ok(self.send.replace_hooks(&specs))
    }
}

#[async_trait]
impl AdminApi for StandaloneAdminApi {
    async fn reload_hooks_from_source(&self, source: HookSpecSource) -> Result<usize, ApiError> {
        let specs = load_hook_specs(&source)
            .await
            .map_err(ApiError::BadRequest)?;
        let subscribers = compile_hook_specs(&specs);
        let count = subscribers.len();
        self.ebus.replace_all(subscribers);
        Ok(count)
    }
}
