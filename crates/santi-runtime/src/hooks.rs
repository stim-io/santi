use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use santi_core::model::{
    runtime::AssemblyItem, runtime::SoulSession, runtime::Turn, session::Session,
    session::SessionMessage,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPoint {
    TurnCompleted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookKind {
    CompactThreshold,
    CompactHandoff,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HookSpec {
    pub id: String,
    pub enabled: bool,
    pub hook_point: HookPoint,
    pub kind: HookKind,
    pub params: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum HookSpecSource {
    Value { hooks: Vec<HookSpec> },
    Path { path: String },
    Url { url: String },
}

pub struct TurnCompletedHookInput<'a> {
    pub turn: &'a Turn,
    pub session: &'a Session,
    pub soul_session: &'a SoulSession,
    pub assistant_message: Option<&'a SessionMessage>,
    pub assembly_tail: &'a [AssemblyItem],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompactReason {
    Manual,
    Threshold,
    Handoff,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeAction {
    Compact {
        session_id: String,
        soul_session_id: String,
        start_session_seq: i64,
        end_session_seq: i64,
        summary: String,
        reason: CompactReason,
        source_hook_id: String,
        source_turn_id: String,
    },
    ForkReserved {
        source_hook_id: String,
        source_turn_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionStatus {
    Executed,
    Skipped,
    Failed,
}

#[derive(Clone, Debug)]
pub struct ActionRecord {
    pub hook_id: String,
    pub turn_id: String,
    pub action_type: String,
    pub status: ActionStatus,
    pub result_ref: Option<String>,
    pub error_text: Option<String>,
}

pub trait HookEvaluator: Send + Sync {
    fn id(&self) -> &str;
    fn hook_point(&self) -> HookPoint;
    fn evaluate_turn_completed(&self, input: TurnCompletedHookInput<'_>) -> Vec<RuntimeAction>;
}

#[derive(Clone, Default)]
pub struct HookRegistry {
    turn_completed: Arc<Vec<Arc<dyn HookEvaluator>>>,
}

#[derive(Clone, Default)]
pub struct HookRegistryHolder {
    current: Arc<RwLock<Arc<HookRegistry>>>,
}

impl HookRegistryHolder {
    pub fn empty() -> Self {
        Self {
            current: Arc::new(RwLock::new(Arc::new(HookRegistry::empty()))),
        }
    }

    pub fn from_specs(specs: &[HookSpec]) -> Self {
        Self {
            current: Arc::new(RwLock::new(Arc::new(HookRegistry::from_specs(specs)))),
        }
    }

    pub fn snapshot(&self) -> Arc<HookRegistry> {
        self.current
            .read()
            .expect("hook registry holder poisoned")
            .clone()
    }

    pub fn replace_all(&self, specs: &[HookSpec]) {
        let next = Arc::new(HookRegistry::from_specs(specs));
        *self.current.write().expect("hook registry holder poisoned") = next;
    }

    pub fn reload_from_specs(&self, specs: &[HookSpec]) -> usize {
        self.replace_all(specs);
        self.snapshot().turn_completed().len()
    }
}

impl HookSpecSource {
    pub fn from_json_str(raw: &str) -> Result<Self, String> {
        if let Ok(hooks) = serde_json::from_str::<Vec<HookSpec>>(raw) {
            return Ok(HookSpecSource::Value { hooks });
        }

        serde_json::from_str::<HookSpecSource>(raw)
            .map_err(|err| format!("parse hook source failed: {err}"))
    }
}

pub async fn load_hook_specs(source: &HookSpecSource) -> Result<Vec<HookSpec>, String> {
    match source {
        HookSpecSource::Value { hooks } => Ok(hooks.clone()),
        HookSpecSource::Path { path } => {
            let raw = tokio::fs::read_to_string(path)
                .await
                .map_err(|err| format!("read hook file failed ({path}): {err}"))?;
            HookSpecSource::from_json_str(&raw).and_then(|source| match source {
                HookSpecSource::Value { hooks } => Ok(hooks),
                _ => Err("hook file must resolve to value source".to_string()),
            })
        }
        HookSpecSource::Url { url } => {
            let response = reqwest::get(url)
                .await
                .map_err(|err| format!("fetch hook url failed ({url}): {err}"))?;
            if !response.status().is_success() {
                return Err(format!(
                    "fetch hook url failed ({url}): status {}",
                    response.status()
                ));
            }
            let raw = response
                .text()
                .await
                .map_err(|err| format!("read hook url body failed ({url}): {err}"))?;
            HookSpecSource::from_json_str(&raw).and_then(|source| match source {
                HookSpecSource::Value { hooks } => Ok(hooks),
                _ => Err("hook url must resolve to value source".to_string()),
            })
        }
    }
}

impl HookRegistry {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_specs(specs: &[HookSpec]) -> Self {
        let mut grouped: HashMap<HookPoint, Vec<Arc<dyn HookEvaluator>>> = HashMap::new();
        for spec in specs.iter().filter(|spec| spec.enabled) {
            if let Some(evaluator) = compile_spec(spec) {
                grouped.entry(spec.hook_point).or_default().push(evaluator);
            }
        }

        Self {
            turn_completed: Arc::new(
                grouped
                    .remove(&HookPoint::TurnCompleted)
                    .unwrap_or_default(),
            ),
        }
    }

    pub fn turn_completed(&self) -> &[Arc<dyn HookEvaluator>] {
        self.turn_completed.as_slice()
    }

    pub fn replace_all(&mut self, specs: &[HookSpec]) {
        *self = Self::from_specs(specs);
    }
}

fn compile_spec(spec: &HookSpec) -> Option<Arc<dyn HookEvaluator>> {
    match spec.kind {
        HookKind::CompactThreshold => CompactThresholdHook::from_spec(spec)
            .map(|hook| Arc::new(hook) as Arc<dyn HookEvaluator>),
        HookKind::CompactHandoff => {
            CompactHandoffHook::from_spec(spec).map(|hook| Arc::new(hook) as Arc<dyn HookEvaluator>)
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct CompactThresholdParams {
    min_messages_since_last_compact: usize,
    summary_template: Option<String>,
}

#[derive(Clone, Debug)]
struct CompactThresholdHook {
    id: String,
    min_messages_since_last_compact: usize,
    summary_template: Option<String>,
}

impl CompactThresholdHook {
    fn from_spec(spec: &HookSpec) -> Option<Self> {
        let params: CompactThresholdParams = serde_json::from_value(spec.params.clone()).ok()?;
        Some(Self {
            id: spec.id.clone(),
            min_messages_since_last_compact: params.min_messages_since_last_compact,
            summary_template: params.summary_template,
        })
    }
}

impl HookEvaluator for CompactThresholdHook {
    fn id(&self) -> &str {
        &self.id
    }

    fn hook_point(&self) -> HookPoint {
        HookPoint::TurnCompleted
    }

    fn evaluate_turn_completed(&self, input: TurnCompletedHookInput<'_>) -> Vec<RuntimeAction> {
        let last_compact_end = input
            .assembly_tail
            .iter()
            .filter_map(|item| match &item.target {
                santi_core::model::runtime::AssemblyTarget::Compact(compact) => {
                    Some(compact.end_session_seq)
                }
                _ => None,
            })
            .max()
            .unwrap_or(0);

        let messages_since_last_compact = input
            .assembly_tail
            .iter()
            .filter(|item| {
                matches!(
                    item.target,
                    santi_core::model::runtime::AssemblyTarget::Message(_)
                )
            })
            .filter(|item| match &item.target {
                santi_core::model::runtime::AssemblyTarget::Message(message) => {
                    message.relation.session_seq > last_compact_end
                }
                _ => false,
            })
            .count();

        let assistant_message = match input.assistant_message {
            Some(message) => message,
            None => return Vec::new(),
        };

        if messages_since_last_compact < self.min_messages_since_last_compact {
            return Vec::new();
        }

        let start_session_seq = last_compact_end + 1;
        let end_session_seq = assistant_message.relation.session_seq;
        let summary = self.summary_template.clone().unwrap_or_else(|| {
            format!(
                "Auto compact for session range {}-{} after turn {}",
                start_session_seq, end_session_seq, input.turn.id
            )
        });

        vec![RuntimeAction::Compact {
            session_id: input.session.id.clone(),
            soul_session_id: input.soul_session.id.clone(),
            start_session_seq,
            end_session_seq,
            summary,
            reason: CompactReason::Threshold,
            source_hook_id: self.id.clone(),
            source_turn_id: input.turn.id.clone(),
        }]
    }
}

#[derive(Clone, Debug, Deserialize)]
struct CompactHandoffParams {
    summary: String,
}

#[derive(Clone, Debug)]
struct CompactHandoffHook {
    id: String,
    summary: String,
}

impl CompactHandoffHook {
    fn from_spec(spec: &HookSpec) -> Option<Self> {
        let params: CompactHandoffParams = serde_json::from_value(spec.params.clone()).ok()?;
        Some(Self {
            id: spec.id.clone(),
            summary: params.summary,
        })
    }
}

impl HookEvaluator for CompactHandoffHook {
    fn id(&self) -> &str {
        &self.id
    }
    fn hook_point(&self) -> HookPoint {
        HookPoint::TurnCompleted
    }
    fn evaluate_turn_completed(&self, _input: TurnCompletedHookInput<'_>) -> Vec<RuntimeAction> {
        let _ = &self.summary;
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_compiles_enabled_specs() {
        let registry = HookRegistry::from_specs(&[HookSpec {
            id: "compact-threshold".to_string(),
            enabled: true,
            hook_point: HookPoint::TurnCompleted,
            kind: HookKind::CompactThreshold,
            params: serde_json::json!({"min_messages_since_last_compact": 3}),
        }]);

        assert_eq!(registry.turn_completed().len(), 1);
        assert_eq!(registry.turn_completed()[0].id(), "compact-threshold");
    }

    #[test]
    fn holder_replaces_registry_atomically() {
        let holder = HookRegistryHolder::empty();
        assert_eq!(holder.snapshot().turn_completed().len(), 0);

        holder.replace_all(&[HookSpec {
            id: "compact-threshold".to_string(),
            enabled: true,
            hook_point: HookPoint::TurnCompleted,
            kind: HookKind::CompactThreshold,
            params: serde_json::json!({"min_messages_since_last_compact": 2}),
        }]);

        let snapshot = holder.snapshot();
        assert_eq!(snapshot.turn_completed().len(), 1);
        assert_eq!(snapshot.turn_completed()[0].id(), "compact-threshold");
    }
}
