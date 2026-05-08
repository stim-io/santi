use santi_core::port::provider::{ProviderFunctionTool, ProviderTool};

pub(super) fn tooling_instructions() -> String {
    [
        "<santi-tools>",
        "Available tools:",
        "- write_soul_memory(text: string): replace the current soul_memory core index text.",
        "- write_session_memory(text: string): replace the current session_memory core index text.",
        "- bash(command: string, cwd?: string): run a local bash command inside the current execution workspace.",
        "Rules:",
        "- soul_memory and session_memory are replace-whole core indexes, not append-only note stores.",
        "- Use write_soul_memory or write_session_memory only when you intend to replace the full core index text for that layer.",
        "- Do not pretend that repeated memory writes create separate durable note objects.",
        "- When the user wants multiple notes, structured records, drafts, or richer memory material, use bash with SANTI_SOUL_MEMORY_DIR or SANTI_SESSION_MEMORY_DIR to manage files, then optionally refresh the corresponding core index.",
        "- Treat the core memory text as the stable index and the *_MEMORY_DIR directories as free-form working memory spaces.",
        "- Use bash when the user asks you to inspect or run something in the local workspace, especially when working with files inside SANTI_SOUL_MEMORY_DIR or SANTI_SESSION_MEMORY_DIR.",
        "- Prefer a single bash call that contains the exact command sequence needed for the current task.",
        "- Bash stdout/stderr are captured to runtime artifact files. Normal tool output is truncated in the stored tool result when it is large; model-facing tool output uses a short preview plus original sizes and artifact paths so the next reply can continue without repeating large stdout/stderr.",
        "- If bash output exceeds the hard runtime limit or the command times out, treat the tool result as incomplete and explain the fallback.",
        "- Inside the container runtime, prefer plain HTTPS git URLs for GitHub clones (for example `git clone https://github.com/owner/repo.git`).",
        "- Do not rely on SSH GitHub clone paths inside the container unless the runtime explicitly says SSH is available.",
        "- When GitHub tokens are present, the bash tool automatically rewrites plain `https://github.com/...` git operations to authenticated HTTPS for that command.",
        "- Prefer `git clone https://github.com/...` over `gh repo clone` for private GitHub workspace bootstrap inside the container.",
        "- Do not claim memory has been updated unless the tool call has completed.",
        "- After a successful memory update, reply briefly and do not repeat the saved content unless the user asks.",
        "</santi-tools>",
    ]
    .join("\n")
}

pub(super) fn provider_tools() -> Vec<ProviderTool> {
    vec![
        ProviderTool::Function(ProviderFunctionTool {
            name: "write_soul_memory".to_string(),
            description: "Replace the current soul_memory core index text.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The full replacement text for the current soul_memory core index."
                    }
                },
                "required": ["text"],
                "additionalProperties": false
            }),
        }),
        ProviderTool::Function(ProviderFunctionTool {
            name: "write_session_memory".to_string(),
            description: "Replace the current session_memory core index text.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The full replacement text for the current session_memory core index."
                    }
                },
                "required": ["text"],
                "additionalProperties": false
            }),
        }),
        ProviderTool::Function(ProviderFunctionTool {
            name: "bash".to_string(),
            description: "Run a local bash command inside the current execution workspace."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute."
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Optional working directory. Relative paths resolve from the session fallback cwd."
                    }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        }),
    ]
}
