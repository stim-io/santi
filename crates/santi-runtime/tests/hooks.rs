use santi_core::hook::{HookKind, HookPoint, HookSpec};
use santi_runtime::hooks::compile_hook_specs;

#[test]
fn compile_enabled_specs() {
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
fn compile_fork_handoff() {
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
