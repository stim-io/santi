use santi_core::service::session::kernel::runtime_prompt::RuntimePrompt;

use crate::runtime::{context::ToolRuntimeContext, tools::ToolExecutor};

pub fn render_runtime_instructions(
    core_prompt: &RuntimePrompt,
    runtime_context: &ToolRuntimeContext,
    tools: &ToolExecutor,
) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(core) = core_prompt.render() {
        parts.push(core);
    }

    let runtime_meta = render_runtime_meta(runtime_context);
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

fn render_runtime_meta(runtime_context: &ToolRuntimeContext) -> Vec<String> {
    vec![
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
