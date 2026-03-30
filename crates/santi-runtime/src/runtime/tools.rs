use santi_core::port::provider::{
    FunctionCallOutput, ProviderFunctionCall, ProviderFunctionTool, ProviderTool,
};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, time::Instant};
use tokio::{
    io::AsyncReadExt,
    process::Command,
    time::{timeout, Duration},
};

use crate::{runtime::context::ToolRuntimeContext, session::memory::SessionMemoryService};

#[derive(Clone, Debug)]
pub struct ToolExecutorConfig {
    pub runtime_root: String,
    pub execution_root: String,
}

#[derive(Clone)]
pub struct ToolExecutor {
    memory_service: SessionMemoryService,
    runtime_root: PathBuf,
    execution_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolCallOk {
    pub ok: bool,
}

#[derive(Debug, Clone)]
pub struct ToolDispatchResult {
    pub tool_name: String,
    pub ok: bool,
    pub tool_output: Value,
    pub function_call_output: FunctionCallOutput,
}

#[derive(Debug, Clone, Deserialize)]
struct WriteSessionMemoryArgs {
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BashToolInput {
    pub command: String,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BashToolResultEnvelope {
    pub feedback_msg: ToolCallFeedbackMsg,
    pub duration_ms: u128,
    pub bash_result: BashToolResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallFeedbackMsg {
    NormalToolCall,
    ToolCallTimeout,
}

impl Serialize for ToolCallFeedbackMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::NormalToolCall => "normal tool call",
            Self::ToolCallTimeout => "tool call timeout",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BashToolResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ToolExecutor {
    pub fn new(memory_service: SessionMemoryService, config: ToolExecutorConfig) -> Self {
        Self {
            memory_service,
            runtime_root: PathBuf::from(config.runtime_root),
            execution_root: PathBuf::from(config.execution_root),
        }
    }

    pub fn render_tooling_instructions(&self) -> Option<String> {
        Some(
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
                "- Do not claim memory has been updated unless the tool call has completed.",
                "- After a successful memory update, reply briefly and do not repeat the saved content unless the user asks.",
                "</santi-tools>",
            ]
            .join("\n"),
        )
    }

    pub fn provider_tools(&self) -> Vec<ProviderTool> {
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

    pub async fn dispatch(
        &self,
        ctx: &ToolRuntimeContext,
        call: &ProviderFunctionCall,
    ) -> Result<ToolDispatchResult, String> {
        match call.name.as_str() {
            "write_soul_memory" => {
                let args: WriteSessionMemoryArgs =
                    match serde_json::from_value(call.arguments.clone()) {
                        Ok(args) => args,
                        Err(err) => {
                            return Ok(build_failed_dispatch_result(
                                &call.name,
                                &call.call_id,
                                format!("invalid write_soul_memory arguments: {err}"),
                            ))
                        }
                    };
                let result = match self.write_soul_memory(ctx, args.text).await {
                    Ok(result) => result,
                    Err(err) => {
                        return Ok(build_failed_dispatch_result(&call.name, &call.call_id, err))
                    }
                };
                let tool_output = serde_json::to_value(&result)
                    .map_err(|err| format!("serialize tool output failed: {err}"))?;

                Ok(ToolDispatchResult {
                    tool_name: call.name.clone(),
                    ok: true,
                    function_call_output: FunctionCallOutput {
                        call_id: call.call_id.clone(),
                        output: tool_output.to_string(),
                    },
                    tool_output,
                })
            }
            "write_session_memory" => {
                let args: WriteSessionMemoryArgs =
                    match serde_json::from_value(call.arguments.clone()) {
                        Ok(args) => args,
                        Err(err) => {
                            return Ok(build_failed_dispatch_result(
                                &call.name,
                                &call.call_id,
                                format!("invalid write_session_memory arguments: {err}"),
                            ))
                        }
                    };
                let result = match self.write_session_memory(ctx, args.text).await {
                    Ok(result) => result,
                    Err(err) => {
                        return Ok(build_failed_dispatch_result(&call.name, &call.call_id, err))
                    }
                };
                let tool_output = serde_json::to_value(&result)
                    .map_err(|err| format!("serialize tool output failed: {err}"))?;

                Ok(ToolDispatchResult {
                    tool_name: call.name.clone(),
                    ok: true,
                    function_call_output: FunctionCallOutput {
                        call_id: call.call_id.clone(),
                        output: tool_output.to_string(),
                    },
                    tool_output,
                })
            }
            "bash" => {
                let args: BashToolInput = match serde_json::from_value(call.arguments.clone()) {
                    Ok(args) => args,
                    Err(err) => {
                        return Ok(build_failed_dispatch_result(
                            &call.name,
                            &call.call_id,
                            format!("invalid bash arguments: {err}"),
                        ))
                    }
                };
                let result = match self.bash(ctx, args).await {
                    Ok(result) => result,
                    Err(err) => {
                        return Ok(build_failed_dispatch_result(&call.name, &call.call_id, err))
                    }
                };
                let tool_output = serde_json::to_value(&result)
                    .map_err(|err| format!("serialize tool output failed: {err}"))?;

                Ok(ToolDispatchResult {
                    tool_name: call.name.clone(),
                    ok: true,
                    function_call_output: FunctionCallOutput {
                        call_id: call.call_id.clone(),
                        output: tool_output.to_string(),
                    },
                    tool_output,
                })
            }
            name => Ok(build_failed_dispatch_result(
                name,
                &call.call_id,
                format!("unsupported tool: {name}"),
            )),
        }
    }

    pub async fn write_session_memory(
        &self,
        ctx: &ToolRuntimeContext,
        text: String,
    ) -> Result<ToolCallOk, String> {
        tracing::info!(session_id = %ctx.session_id, soul_id = %ctx.soul_id, text_chars = text.len(), "runtime tool call: write_session_memory");
        self.memory_service
            .write_session_memory(&ctx.session_id, &text)
            .await?
            .ok_or_else(|| "session not found".to_string())?;
        tracing::info!(session_id = %ctx.session_id, "runtime tool call completed: write_session_memory");
        Ok(ToolCallOk { ok: true })
    }

    pub async fn write_soul_memory(
        &self,
        ctx: &ToolRuntimeContext,
        text: String,
    ) -> Result<ToolCallOk, String> {
        tracing::info!(soul_id = %ctx.soul_id, text_chars = text.len(), "runtime tool call: write_soul_memory");
        self.memory_service
            .write_soul_memory(&ctx.soul_id, &text)
            .await?
            .ok_or_else(|| "soul not found".to_string())?;
        tracing::info!("runtime tool call completed: write_soul_memory");
        Ok(ToolCallOk { ok: true })
    }

    pub async fn bash(
        &self,
        ctx: &ToolRuntimeContext,
        input: BashToolInput,
    ) -> Result<BashToolResultEnvelope, String> {
        tracing::info!(
            session_id = %ctx.session_id,
            soul_id = %ctx.soul_id,
            command_chars = input.command.len(),
            cwd = input.cwd.as_deref().unwrap_or(""),
            "runtime tool call: bash"
        );

        std::fs::create_dir_all(&ctx.soul_memory_dir)
            .map_err(|err| format!("failed to create soul_memory_dir: {err}"))?;
        std::fs::create_dir_all(&ctx.session_memory_dir)
            .map_err(|err| format!("failed to create session_memory_dir: {err}"))?;

        let workdir = resolve_cwd(ctx, input.cwd.as_deref())?;
        std::fs::create_dir_all(&workdir)
            .map_err(|err| format!("failed to create workdir: {err}"))?;

        let started_at = Instant::now();
        let mut child = Command::new("/bin/bash")
            .arg("-lc")
            .arg(&input.command)
            .current_dir(&workdir)
            .env("SANTI_SOUL_MEMORY_DIR", &ctx.soul_memory_dir)
            .env("SANTI_SESSION_MEMORY_DIR", &ctx.session_memory_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("failed to spawn bash: {err}"))?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| "missing child stdout".to_string())?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| "missing child stderr".to_string())?;

        let stdout_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = stdout.read_to_end(&mut buf).await;
            String::from_utf8_lossy(&buf).to_string()
        });
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = stderr.read_to_end(&mut buf).await;
            String::from_utf8_lossy(&buf).to_string()
        });

        let wait_result = timeout(Duration::from_secs(24 * 60 * 60), child.wait()).await;

        let (feedback_msg, exit_code) = match wait_result {
            Ok(wait_status) => {
                let status =
                    wait_status.map_err(|err| format!("failed to wait for bash: {err}"))?;
                (
                    ToolCallFeedbackMsg::NormalToolCall,
                    status.code().unwrap_or(-1),
                )
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                (ToolCallFeedbackMsg::ToolCallTimeout, -1)
            }
        };

        let duration_ms = started_at.elapsed().as_millis();
        let stdout = stdout_task
            .await
            .map_err(|err| format!("stdout task failed: {err}"))?;
        let stderr = stderr_task
            .await
            .map_err(|err| format!("stderr task failed: {err}"))?;

        tracing::info!(
            session_id = %ctx.session_id,
            duration_ms,
            exit_code,
            "runtime tool call completed: bash"
        );

        Ok(BashToolResultEnvelope {
            feedback_msg,
            duration_ms,
            bash_result: BashToolResult {
                exit_code,
                stdout,
                stderr,
            },
        })
    }
}

fn build_failed_dispatch_result(
    tool_name: &str,
    call_id: &str,
    message: String,
) -> ToolDispatchResult {
    let tool_output = serde_json::json!({
        "ok": false,
        "error": {
            "type": "tool_error",
            "message": message,
        }
    });

    ToolDispatchResult {
        tool_name: tool_name.to_string(),
        ok: false,
        function_call_output: FunctionCallOutput {
            call_id: call_id.to_string(),
            output: tool_output.to_string(),
        },
        tool_output,
    }
}

fn resolve_cwd(ctx: &ToolRuntimeContext, cwd: Option<&str>) -> Result<PathBuf, String> {
    match cwd {
        None => Ok(ctx.fallback_cwd.clone()),
        Some(path) => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                Ok(path)
            } else {
                Ok(ctx.fallback_cwd.join(path))
            }
        }
    }
}

impl ToolExecutor {
    pub fn build_context(&self, session_id: &str, soul_id: &str) -> ToolRuntimeContext {
        ToolRuntimeContext {
            session_id: session_id.to_string(),
            soul_id: soul_id.to_string(),
            soul_memory_dir: self.runtime_root.join("souls").join(soul_id).join("memory"),
            session_memory_dir: self
                .runtime_root
                .join("sessions")
                .join(session_id)
                .join("memory"),
            fallback_cwd: self.execution_root.clone(),
        }
    }
}
