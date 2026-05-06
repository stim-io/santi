use santi_core::service::session::kernel::runtime_prompt::RuntimePrompt;

use crate::runtime::{
    context::{RuntimeSelfFacts, ToolRuntimeContext},
    tools::ToolExecutor,
};

pub fn render_runtime_instructions(
    core_prompt: &RuntimePrompt,
    runtime_context: &ToolRuntimeContext,
    self_facts: &RuntimeSelfFacts,
    tools: &ToolExecutor,
) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(core) = core_prompt.render() {
        parts.push(core);
    }

    parts.push(render_self_assessment_instructions());

    let runtime_meta = render_runtime_meta(runtime_context, self_facts);
    if !runtime_meta.is_empty() {
        parts.push(format!(
            "<santi-runtime>\n{}\n</santi-runtime>",
            runtime_meta.join("\n")
        ));
    }

    if let Some(tooling) = tools.render_tooling_instructions() {
        parts.push(tooling);
    }

    let rendered = parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if rendered.trim().is_empty() {
        None
    } else {
        Some(rendered)
    }
}

fn render_self_assessment_instructions() -> String {
    [
        "<santi-self-assessment>",
        "When asked to assess your own runtime or product capability:",
        "- Ground the answer in visible facts from the santi-meta block, the santi-runtime block, the tool list, and the current conversation.",
        "- When tool results are available, label tool-confirmed facts separately from runtime/context-only facts and unknowns.",
        "- Preserve the user's requested language, section names, and bullet limits when they provide them.",
        "- If the user does not provide section names, separate connected capabilities, unknowns, blockers, and the next delivery action.",
        "- Keep self-assessments concise enough for the product loop; default to 1-3 bullets per section unless the user asks for a deep audit.",
        "- Treat missing facts as unknown; do not infer service health, permissions, durable product-ledger state, or external process state unless visible or tool-confirmed.",
        "- Keep the next action tied to the integrated stim -> santi product loop.",
        "</santi-self-assessment>",
    ]
    .join("\n")
}

fn render_runtime_meta(
    runtime_context: &ToolRuntimeContext,
    self_facts: &RuntimeSelfFacts,
) -> Vec<String> {
    vec![
        format!("service_name: {}", self_facts.service_name),
        format!("assembly_mode: {}", self_facts.assembly_mode),
        format!(
            "launch_profile: {}",
            self_facts.launch_profile.as_deref().unwrap_or("unknown")
        ),
        format!(
            "bind_addr: {}",
            self_facts.bind_addr.as_deref().unwrap_or("unknown")
        ),
        format!("provider_model: {}", self_facts.provider_model),
        format!("provider_api: {}", self_facts.provider_api),
        format!(
            "provider_gateway_base_url: {}",
            self_facts
                .provider_gateway_base_url
                .as_deref()
                .unwrap_or("unknown")
        ),
        format!(
            "SANTI_SOUL_MEMORY_DIR: {}",
            runtime_context.soul_memory_dir.display()
        ),
        format!(
            "SANTI_SESSION_MEMORY_DIR: {}",
            runtime_context.session_memory_dir.display()
        ),
        format!("fallback_cwd: {}", runtime_context.fallback_cwd.display()),
    ]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{render_runtime_meta, render_self_assessment_instructions};
    use crate::runtime::context::{RuntimeSelfFacts, ToolRuntimeContext};

    #[test]
    fn self_assessment_instructions_require_grounded_unknowns() {
        let instructions = render_self_assessment_instructions();

        assert!(instructions.contains("<santi-self-assessment>"));
        assert!(instructions.contains("Ground the answer in visible facts"));
        assert!(instructions.contains("tool-confirmed facts separately"));
        assert!(instructions.contains("requested language, section names"));
        assert!(instructions.contains("Treat missing facts as unknown"));
        assert!(instructions.contains("stim -> santi"));
    }

    #[test]
    fn runtime_meta_exposes_self_facts_without_guessing_missing_values() {
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
}
