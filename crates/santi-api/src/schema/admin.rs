use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use santi_core::hook::{HookKind, HookPoint, HookSpec, HookSpecSource};

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct HookReloadRequest {
    #[serde(flatten)]
    #[schema(value_type = HookReloadSourcePayload)]
    pub source: HookReloadSourcePayload,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct HookReloadResponse {
    pub hook_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(untagged)]
pub enum HookReloadSourcePayload {
    Value { hooks: Vec<HookSpecPayload> },
    Path { path: String },
    Url { url: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct HookSpecPayload {
    pub id: String,
    pub enabled: bool,
    pub hook_point: HookPointPayload,
    pub kind: HookKindPayload,
    pub params: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookPointPayload {
    TurnCompleted,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookKindPayload {
    CompactThreshold,
    CompactHandoff,
    ForkHandoffThreshold,
}

impl From<HookPointPayload> for HookPoint {
    fn from(value: HookPointPayload) -> Self {
        match value {
            HookPointPayload::TurnCompleted => HookPoint::TurnCompleted,
        }
    }
}

impl From<HookKindPayload> for HookKind {
    fn from(value: HookKindPayload) -> Self {
        match value {
            HookKindPayload::CompactThreshold => HookKind::CompactThreshold,
            HookKindPayload::CompactHandoff => HookKind::CompactHandoff,
            HookKindPayload::ForkHandoffThreshold => HookKind::ForkHandoffThreshold,
        }
    }
}

impl From<HookSpecPayload> for HookSpec {
    fn from(value: HookSpecPayload) -> Self {
        HookSpec {
            id: value.id,
            enabled: value.enabled,
            hook_point: value.hook_point.into(),
            kind: value.kind.into(),
            params: value.params,
        }
    }
}

impl From<HookReloadSourcePayload> for HookSpecSource {
    fn from(value: HookReloadSourcePayload) -> Self {
        match value {
            HookReloadSourcePayload::Value { hooks } => HookSpecSource::Value {
                hooks: hooks.into_iter().map(Into::into).collect(),
            },
            HookReloadSourcePayload::Path { path } => HookSpecSource::Path { path },
            HookReloadSourcePayload::Url { url } => HookSpecSource::Url { url },
        }
    }
}
