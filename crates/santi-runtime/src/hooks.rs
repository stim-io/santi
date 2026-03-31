use std::sync::Arc;

use santi_core::{
    hook::{HookKind, HookPoint, HookSpec, HookSpecSource, RuntimeAction},
    model::{runtime::AssemblyItem, runtime::SoulSession, runtime::Turn, runtime::TurnTriggerType, session::Session, session::SessionMessage},
};
use serde::Deserialize;

pub struct TurnCompletedHookInput<'a> {
    pub turn: &'a Turn,
    pub session: &'a Session,
    pub soul_session: &'a SoulSession,
    pub assistant_message: Option<&'a SessionMessage>,
    pub assembly_tail: &'a [AssemblyItem],
}

pub trait HookEvaluator: Send + Sync {
    fn id(&self) -> &str;
    fn hook_point(&self) -> HookPoint;
    fn evaluate_turn_completed(&self, input: TurnCompletedHookInput<'_>) -> Vec<RuntimeAction>;
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

pub fn compile_hook_specs(specs: &[HookSpec]) -> Vec<Arc<dyn HookEvaluator>> {
    specs
        .iter()
        .filter(|spec| spec.enabled)
        .filter_map(compile_spec)
        .collect()
}

fn compile_spec(spec: &HookSpec) -> Option<Arc<dyn HookEvaluator>> {
    match spec.kind {
        HookKind::CompactThreshold => CompactThresholdHook::from_spec(spec)
            .map(|hook| Arc::new(hook) as Arc<dyn HookEvaluator>),
        HookKind::CompactHandoff => CompactHandoffHook::from_spec(spec)
            .map(|hook| Arc::new(hook) as Arc<dyn HookEvaluator>),
        HookKind::ForkHandoffThreshold => ForkHandoffThresholdHook::from_spec(spec)
            .map(|hook| Arc::new(hook) as Arc<dyn HookEvaluator>),
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
                santi_core::model::runtime::AssemblyTarget::Compact(compact) => Some(compact.end_session_seq),
                _ => None,
            })
            .max()
            .unwrap_or(0);

        let messages_since_last_compact = input
            .assembly_tail
            .iter()
            .filter(|item| matches!(item.target, santi_core::model::runtime::AssemblyTarget::Message(_)))
            .filter(|item| match &item.target {
                santi_core::model::runtime::AssemblyTarget::Message(message) => message.relation.session_seq > last_compact_end,
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
            reason: santi_core::hook::CompactReason::Threshold,
            source_hook_id: self.id.clone(),
            source_turn_id: input.turn.id.clone(),
        }]
    }
}

#[derive(Clone, Debug, Deserialize)]
struct CompactHandoffParams {
    summary: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ForkHandoffThresholdParams {
    min_messages_since_last_compact: usize,
    seed_text: Option<String>,
}

#[derive(Clone, Debug)]
struct CompactHandoffHook {
    id: String,
    summary: String,
}

impl CompactHandoffHook {
    fn from_spec(spec: &HookSpec) -> Option<Self> {
        let params: CompactHandoffParams = serde_json::from_value(spec.params.clone()).ok()?;
        Some(Self { id: spec.id.clone(), summary: params.summary })
    }
}

impl HookEvaluator for CompactHandoffHook {
    fn id(&self) -> &str { &self.id }
    fn hook_point(&self) -> HookPoint { HookPoint::TurnCompleted }
    fn evaluate_turn_completed(&self, _input: TurnCompletedHookInput<'_>) -> Vec<RuntimeAction> {
        let _ = &self.summary;
        Vec::new()
    }
}

#[derive(Clone, Debug)]
struct ForkHandoffThresholdHook {
    id: String,
    min_messages_since_last_compact: usize,
    seed_text: Option<String>,
}

impl ForkHandoffThresholdHook {
    fn from_spec(spec: &HookSpec) -> Option<Self> {
        let params: ForkHandoffThresholdParams = serde_json::from_value(spec.params.clone()).ok()?;
        Some(Self {
            id: spec.id.clone(),
            min_messages_since_last_compact: params.min_messages_since_last_compact,
            seed_text: params.seed_text,
        })
    }
}

impl HookEvaluator for ForkHandoffThresholdHook {
    fn id(&self) -> &str {
        &self.id
    }

    fn hook_point(&self) -> HookPoint {
        HookPoint::TurnCompleted
    }

    fn evaluate_turn_completed(&self, input: TurnCompletedHookInput<'_>) -> Vec<RuntimeAction> {
        if !matches!(input.turn.trigger_type, TurnTriggerType::SessionSend) {
            return Vec::new();
        }

        let assistant_message = match input.assistant_message {
            Some(message) => message,
            None => return Vec::new(),
        };

        let last_compact_end = input
            .assembly_tail
            .iter()
            .filter_map(|item| match &item.target {
                santi_core::model::runtime::AssemblyTarget::Compact(compact) => Some(compact.end_session_seq),
                _ => None,
            })
            .max()
            .unwrap_or(0);

        let messages_since_last_compact = input
            .assembly_tail
            .iter()
            .filter(|item| matches!(item.target, santi_core::model::runtime::AssemblyTarget::Message(_)))
            .filter(|item| match &item.target {
                santi_core::model::runtime::AssemblyTarget::Message(message) => message.relation.session_seq > last_compact_end,
                _ => false,
            })
            .count();

        if messages_since_last_compact < self.min_messages_since_last_compact {
            return Vec::new();
        }

        let seed_text = self.seed_text.clone().unwrap_or_else(|| {
            format!(
                "Recommend to use compact before continuing. <santi-meta effect=\"hook_fork_handoff\" source_hook_id=\"{}\" source_turn_id=\"{}\" fork_point=\"{}\"></santi-meta>",
                self.id,
                input.turn.id,
                assistant_message.relation.session_seq,
            )
        });

        vec![RuntimeAction::ForkHandoff {
            session_id: input.session.id.clone(),
            fork_point: assistant_message.relation.session_seq,
            seed_text,
            source_hook_id: self.id.clone(),
            source_turn_id: input.turn.id.clone(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use santi_core::hook::{HookKind, HookPoint, HookSpec};

    #[test]
    fn compile_enabled_specs_into_subscribers() {
        let subscribers = compile_hook_specs(&[HookSpec {
            id: "compact-threshold".to_string(),
            enabled: true,
            hook_point: HookPoint::TurnCompleted,
            kind: HookKind::CompactThreshold,
            params: serde_json::json!({"min_messages_since_last_compact": 3}),
        }]);

        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0].id(), "compact-threshold");
    }

    #[test]
    fn compile_fork_handoff_threshold_spec() {
        let subscribers = compile_hook_specs(&[HookSpec {
            id: "fork-handoff".to_string(),
            enabled: true,
            hook_point: HookPoint::TurnCompleted,
            kind: HookKind::ForkHandoffThreshold,
            params: serde_json::json!({"min_messages_since_last_compact": 3}),
        }]);

        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0].id(), "fork-handoff");
    }
}
