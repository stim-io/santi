use std::path::PathBuf;

use santi_runtime::runtime::{
    context::{RuntimeSelfFacts, ToolRuntimeContext},
    prompt::{render_runtime_meta, render_self_assessment_instructions},
};

#[test]
fn self_assessment_grounding() {
    let instructions = render_self_assessment_instructions();

    assert!(instructions.contains("<santi-self-assessment>"));
    assert!(instructions.contains("Ground the answer in visible facts"));
    assert!(instructions.contains("tool-confirmed facts separately"));
    assert!(instructions.contains("requested language, section names"));
    assert!(instructions.contains("Treat missing facts as unknown"));
    assert!(instructions.contains("stim -> santi"));
}

#[test]
fn runtime_meta_grounding() {
    let ctx = ToolRuntimeContext {
        session_id: "session-1".to_string(),
        soul_id: "soul-1".to_string(),
        soul_memory_dir: PathBuf::from("/runtime/souls/soul-1/memory"),
        session_memory_dir: PathBuf::from("/runtime/sessions/session-1/memory"),
        fallback_cwd: PathBuf::from("/workspace"),
    };
    let facts = RuntimeSelfFacts {
        service_name: "santi".to_string(),
        assembly_mode: "standalone".to_string(),
        launch_profile: Some("local-foreground".to_string()),
        bind_addr: None,
        provider_model: "gpt-5.4".to_string(),
        provider_api: "responses".to_string(),
        provider_gateway_base_url: Some("http://127.0.0.1:18082/openai/v1".to_string()),
    };

    let rendered = render_runtime_meta(&ctx, &facts).join("\n");

    assert!(rendered.contains("service_name: santi"));
    assert!(rendered.contains("assembly_mode: standalone"));
    assert!(rendered.contains("launch_profile: local-foreground"));
    assert!(rendered.contains("bind_addr: unknown"));
    assert!(rendered.contains("provider_model: gpt-5.4"));
    assert!(rendered.contains("provider_api: responses"));
    assert!(rendered.contains("provider_gateway_base_url: http://127.0.0.1:18082/openai/v1"));
}
