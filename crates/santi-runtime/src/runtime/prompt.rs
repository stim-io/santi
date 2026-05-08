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

pub fn render_self_assessment_instructions() -> String {
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

pub fn render_runtime_meta(
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
