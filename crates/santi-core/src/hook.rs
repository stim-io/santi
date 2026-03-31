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
    ForkHandoffThreshold,
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
    ForkHandoff {
        session_id: String,
        fork_point: i64,
        seed_text: String,
        source_hook_id: String,
        source_turn_id: String,
    },
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
