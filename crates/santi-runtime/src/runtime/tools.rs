use santi_core::port::provider::{
    FunctionCallOutput, ProviderFunctionCall, ProviderFunctionTool, ProviderTool,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, time::Instant};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::mpsc,
    time::{timeout, Duration},
};
use uuid::Uuid;

use crate::{runtime::context::ToolRuntimeContext, session::memory::SessionMemoryService};

mod github_https_git_env;

use self::github_https_git_env::github_https_git_env;

pub const DEFAULT_BASH_TIMEOUT_SECS: u64 = 24 * 60 * 60;
pub const DEFAULT_BASH_OUTPUT_TRUNCATE_CHARS: usize = 10_000;
pub const DEFAULT_BASH_OUTPUT_HARD_BYTES: usize = 1024 * 1024;
const BASH_MODEL_OUTPUT_PREVIEW_CHARS: usize = 2_000;

#[derive(Clone, Debug)]
pub struct ToolExecutorConfig {
    pub runtime_root: String,
    pub execution_root: String,
    pub bash_timeout_secs: u64,
    pub bash_output_truncate_chars: usize,
    pub bash_output_hard_bytes: usize,
}

#[derive(Clone)]
pub struct ToolExecutor {
    memory_service: SessionMemoryService,
    runtime_root: PathBuf,
    execution_root: PathBuf,
    bash_timeout_secs: u64,
    bash_output_truncate_chars: usize,
    bash_output_hard_bytes: usize,
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
    ToolOutputLimitExceeded,
}

impl Serialize for ToolCallFeedbackMsg {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::NormalToolCall => "normal tool call",
            Self::ToolCallTimeout => "tool call timeout",
            Self::ToolOutputLimitExceeded => "tool output limit exceeded",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BashToolResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub stdout_chars: u64,
    pub stderr_chars: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub stdout_artifact_path: Option<String>,
    pub stderr_artifact_path: Option<String>,
}

impl ToolExecutor {
    pub fn new(memory_service: SessionMemoryService, config: ToolExecutorConfig) -> Self {
        Self {
            memory_service,
            runtime_root: PathBuf::from(config.runtime_root),
            execution_root: PathBuf::from(config.execution_root),
            bash_timeout_secs: nonzero_or_default(
                config.bash_timeout_secs,
                DEFAULT_BASH_TIMEOUT_SECS,
            ),
            bash_output_truncate_chars: nonzero_or_default_usize(
                config.bash_output_truncate_chars,
                DEFAULT_BASH_OUTPUT_TRUNCATE_CHARS,
            ),
            bash_output_hard_bytes: normalized_hard_bytes(
                config.bash_output_hard_bytes,
                config.bash_output_truncate_chars,
            ),
        }
    }

    fn runtime_bash_timeout_secs(&self) -> u64 {
        self.bash_timeout_secs
    }

    fn runtime_bash_output_truncate_chars(&self) -> usize {
        self.bash_output_truncate_chars
    }

    fn runtime_bash_output_hard_bytes(&self) -> usize {
        self.bash_output_hard_bytes
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
            "write_soul_memory" => self.dispatch_write_soul_memory(ctx, call).await,
            "write_session_memory" => self.dispatch_write_session_memory(ctx, call).await,
            "bash" => self.dispatch_bash(ctx, call).await,
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
        let mut command = Command::new("/bin/bash");
        command
            .arg("-lc")
            .arg(&input.command)
            .current_dir(&workdir)
            .env("SANTI_SOUL_MEMORY_DIR", &ctx.soul_memory_dir)
            .env("SANTI_SESSION_MEMORY_DIR", &ctx.session_memory_dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in github_https_git_env() {
            command.env(key, value);
        }

        let mut child = command
            .spawn()
            .map_err(|err| format!("failed to spawn bash: {err}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "missing child stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "missing child stderr".to_string())?;

        let artifact_dir = ctx.session_memory_dir.join(".santi").join("tool-artifacts");
        tokio::fs::create_dir_all(&artifact_dir)
            .await
            .map_err(|err| format!("failed to create tool artifact dir: {err}"))?;
        let artifact_id = Uuid::new_v4().simple().to_string();
        let limits = BashOutputLimits {
            truncate_chars: self.runtime_bash_output_truncate_chars(),
            hard_bytes: self.runtime_bash_output_hard_bytes(),
        };
        let (limit_sender, mut limit_receiver) = mpsc::unbounded_channel::<String>();

        let stdout_task = tokio::spawn(capture_bash_stream(
            stdout,
            artifact_dir.join(format!("bash-{artifact_id}-stdout.txt")),
            "stdout",
            limits,
            limit_sender.clone(),
        ));
        let stderr_task = tokio::spawn(capture_bash_stream(
            stderr,
            artifact_dir.join(format!("bash-{artifact_id}-stderr.txt")),
            "stderr",
            limits,
            limit_sender,
        ));

        let mut feedback_msg = ToolCallFeedbackMsg::NormalToolCall;
        let mut exit_code = -1;
        let wait_result = timeout(
            Duration::from_secs(self.runtime_bash_timeout_secs()),
            child.wait(),
        );

        tokio::select! {
            wait_status = wait_result => {
                match wait_status {
                    Ok(wait_status) => {
                        let status = wait_status
                            .map_err(|err| format!("failed to wait for bash: {err}"))?;
                        exit_code = status.code().unwrap_or(-1);
                    }
                    Err(_) => {
                        feedback_msg = ToolCallFeedbackMsg::ToolCallTimeout;
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                    }
                }
            }
            Some(stream_name) = limit_receiver.recv() => {
                feedback_msg = ToolCallFeedbackMsg::ToolOutputLimitExceeded;
                tracing::warn!(
                    session_id = %ctx.session_id,
                    stream = %stream_name,
                    hard_bytes = self.runtime_bash_output_hard_bytes(),
                    "runtime tool call output limit exceeded: bash"
                );
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        }

        let duration_ms = started_at.elapsed().as_millis();
        let stdout = stdout_task
            .await
            .map_err(|err| format!("stdout task failed: {err}"))??;
        let stderr = stderr_task
            .await
            .map_err(|err| format!("stderr task failed: {err}"))??;
        if stdout.hard_limit_exceeded || stderr.hard_limit_exceeded {
            feedback_msg = ToolCallFeedbackMsg::ToolOutputLimitExceeded;
            exit_code = -1;
        }

        tracing::info!(
            session_id = %ctx.session_id,
            duration_ms,
            exit_code,
            stdout_chars = stdout.raw_chars,
            stderr_chars = stderr.raw_chars,
            stdout_truncated = stdout.truncated,
            stderr_truncated = stderr.truncated,
            "runtime tool call completed: bash"
        );

        Ok(BashToolResultEnvelope {
            feedback_msg,
            duration_ms,
            bash_result: BashToolResult {
                exit_code,
                stdout: stdout.text,
                stderr: stderr.text,
                stdout_chars: stdout.raw_chars,
                stderr_chars: stderr.raw_chars,
                stdout_truncated: stdout.truncated,
                stderr_truncated: stderr.truncated,
                stdout_artifact_path: stdout.artifact_path,
                stderr_artifact_path: stderr.artifact_path,
            },
        })
    }

    async fn dispatch_write_soul_memory(
        &self,
        ctx: &ToolRuntimeContext,
        call: &ProviderFunctionCall,
    ) -> Result<ToolDispatchResult, String> {
        let args = match parse_tool_args::<WriteSessionMemoryArgs>(call, "write_soul_memory") {
            Ok(args) => args,
            Err(result) => return Ok(result),
        };
        let result = match self.write_soul_memory(ctx, args.text).await {
            Ok(result) => result,
            Err(err) => return Ok(build_failed_dispatch_result(&call.name, &call.call_id, err)),
        };

        build_success_dispatch_result(&call.name, &call.call_id, &result)
    }

    async fn dispatch_write_session_memory(
        &self,
        ctx: &ToolRuntimeContext,
        call: &ProviderFunctionCall,
    ) -> Result<ToolDispatchResult, String> {
        let args = match parse_tool_args::<WriteSessionMemoryArgs>(call, "write_session_memory") {
            Ok(args) => args,
            Err(result) => return Ok(result),
        };
        let result = match self.write_session_memory(ctx, args.text).await {
            Ok(result) => result,
            Err(err) => return Ok(build_failed_dispatch_result(&call.name, &call.call_id, err)),
        };

        build_success_dispatch_result(&call.name, &call.call_id, &result)
    }

    async fn dispatch_bash(
        &self,
        ctx: &ToolRuntimeContext,
        call: &ProviderFunctionCall,
    ) -> Result<ToolDispatchResult, String> {
        let args = match parse_tool_args::<BashToolInput>(call, "bash") {
            Ok(args) => args,
            Err(result) => return Ok(result),
        };
        let result = match self.bash(ctx, args).await {
            Ok(result) => result,
            Err(err) => return Ok(build_failed_dispatch_result(&call.name, &call.call_id, err)),
        };

        build_bash_dispatch_result(&call.name, &call.call_id, &result)
    }
}

fn build_bash_dispatch_result(
    tool_name: &str,
    call_id: &str,
    result: &BashToolResultEnvelope,
) -> Result<ToolDispatchResult, String> {
    let tool_output = serde_json::to_value(result)
        .map_err(|err| format!("serialize bash tool output failed: {err}"))?;
    let model_output = bash_model_tool_output(result)?;

    Ok(ToolDispatchResult {
        tool_name: tool_name.to_string(),
        ok: matches!(result.feedback_msg, ToolCallFeedbackMsg::NormalToolCall),
        function_call_output: FunctionCallOutput {
            call_id: call_id.to_string(),
            output: model_output.to_string(),
        },
        tool_output,
    })
}

fn bash_model_tool_output(result: &BashToolResultEnvelope) -> Result<Value, String> {
    let bash = &result.bash_result;
    let needs_model_projection =
        !matches!(result.feedback_msg, ToolCallFeedbackMsg::NormalToolCall)
            || bash.stdout_truncated
            || bash.stderr_truncated;

    if !needs_model_projection {
        return serde_json::to_value(result)
            .map_err(|err| format!("serialize bash model output failed: {err}"));
    }

    Ok(serde_json::json!({
        "feedback_msg": &result.feedback_msg,
        "duration_ms": result.duration_ms,
        "model_projection": {
            "kind": "bash_output_preview",
            "note": "Large or incomplete bash output is summarized for the model. Use artifact paths with bash if exact content is needed."
        },
        "bash_result": {
            "exit_code": bash.exit_code,
            "stdout_preview": preview_for_model(&bash.stdout),
            "stderr_preview": preview_for_model(&bash.stderr),
            "stdout_chars": bash.stdout_chars,
            "stderr_chars": bash.stderr_chars,
            "stdout_truncated": bash.stdout_truncated,
            "stderr_truncated": bash.stderr_truncated,
            "stdout_artifact_path": bash.stdout_artifact_path.clone(),
            "stderr_artifact_path": bash.stderr_artifact_path.clone(),
        }
    }))
}

fn preview_for_model(text: &str) -> String {
    if text.chars().count() <= BASH_MODEL_OUTPUT_PREVIEW_CHARS {
        return text.to_string();
    }

    let preview = text
        .chars()
        .take(BASH_MODEL_OUTPUT_PREVIEW_CHARS)
        .collect::<String>();
    format!("{preview}\n[model-facing preview truncated]")
}

fn parse_tool_args<T: DeserializeOwned>(
    call: &ProviderFunctionCall,
    tool_name: &str,
) -> Result<T, ToolDispatchResult> {
    serde_json::from_value(call.arguments.clone()).map_err(|err| {
        build_failed_dispatch_result(
            &call.name,
            &call.call_id,
            format!("invalid {tool_name} arguments: {err}"),
        )
    })
}

fn build_success_dispatch_result<T: Serialize>(
    tool_name: &str,
    call_id: &str,
    result: &T,
) -> Result<ToolDispatchResult, String> {
    let tool_output = serde_json::to_value(result)
        .map_err(|err| format!("serialize tool output failed: {err}"))?;

    Ok(ToolDispatchResult {
        tool_name: tool_name.to_string(),
        ok: true,
        function_call_output: FunctionCallOutput {
            call_id: call_id.to_string(),
            output: tool_output.to_string(),
        },
        tool_output,
    })
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

#[derive(Debug, Clone, Copy)]
struct BashOutputLimits {
    truncate_chars: usize,
    hard_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedBashStream {
    text: String,
    raw_chars: u64,
    truncated: bool,
    artifact_path: Option<String>,
    hard_limit_exceeded: bool,
}

async fn capture_bash_stream<R>(
    mut reader: R,
    artifact_path: PathBuf,
    stream_name: &'static str,
    limits: BashOutputLimits,
    limit_sender: mpsc::UnboundedSender<String>,
) -> Result<CapturedBashStream, String>
where
    R: AsyncRead + Unpin,
{
    let mut file = tokio::fs::File::create(&artifact_path)
        .await
        .map_err(|err| format!("failed to create bash {stream_name} artifact: {err}"))?;
    let mut output = Vec::new();
    let mut total_bytes = 0_usize;
    let mut hard_limit_exceeded = false;
    let mut buf = [0_u8; 8192];

    loop {
        let read = reader
            .read(&mut buf)
            .await
            .map_err(|err| format!("failed to read bash {stream_name}: {err}"))?;
        if read == 0 {
            break;
        }

        let remaining = limits.hard_bytes.saturating_sub(total_bytes);
        let accepted = read.min(remaining);
        if accepted > 0 {
            file.write_all(&buf[..accepted])
                .await
                .map_err(|err| format!("failed to write bash {stream_name} artifact: {err}"))?;
            output.extend_from_slice(&buf[..accepted]);
            total_bytes += accepted;
        }

        if accepted < read {
            hard_limit_exceeded = true;
            let _ = limit_sender.send(stream_name.to_string());
            break;
        }
    }

    file.flush()
        .await
        .map_err(|err| format!("failed to flush bash {stream_name} artifact: {err}"))?;

    let raw = String::from_utf8_lossy(&output).to_string();
    let raw_chars = raw.chars().count() as u64;
    let text = truncate_chars(&raw, limits.truncate_chars);
    let truncated = hard_limit_exceeded || text.chars().count() as u64 != raw_chars;

    Ok(CapturedBashStream {
        text,
        raw_chars,
        truncated,
        artifact_path: truncated.then(|| artifact_path.display().to_string()),
        hard_limit_exceeded,
    })
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        truncated
    } else {
        value.to_string()
    }
}

fn nonzero_or_default(value: u64, default: u64) -> u64 {
    if value == 0 {
        default
    } else {
        value
    }
}

fn nonzero_or_default_usize(value: usize, default: usize) -> usize {
    if value == 0 {
        default
    } else {
        value
    }
}

fn normalized_hard_bytes(hard_bytes: usize, truncate_chars: usize) -> usize {
    let hard_bytes = nonzero_or_default_usize(hard_bytes, DEFAULT_BASH_OUTPUT_HARD_BYTES);
    let truncate_floor =
        nonzero_or_default_usize(truncate_chars, DEFAULT_BASH_OUTPUT_TRUNCATE_CHARS);
    hard_bytes.max(truncate_floor)
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

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;

    use super::{
        bash_model_tool_output, capture_bash_stream, BashOutputLimits, BashToolResult,
        BashToolResultEnvelope, ToolCallFeedbackMsg,
    };

    #[tokio::test]
    async fn capture_bash_stream_truncates_projection_but_keeps_artifact() {
        let (mut writer, reader) = tokio::io::duplex(64);
        let payload = "abcdef";
        let artifact_path = std::env::temp_dir().join(format!(
            "santi-bash-capture-{}.txt",
            uuid::Uuid::new_v4().simple()
        ));
        let artifact_path_for_assertion = artifact_path.clone();
        let (limit_sender, mut limit_receiver) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            writer.write_all(payload.as_bytes()).await.unwrap();
        });

        let captured = capture_bash_stream(
            reader,
            artifact_path,
            "stdout",
            BashOutputLimits {
                truncate_chars: 3,
                hard_bytes: 1024,
            },
            limit_sender,
        )
        .await
        .unwrap();

        assert_eq!(captured.text, "abc");
        assert_eq!(captured.raw_chars, 6);
        assert!(captured.truncated);
        assert!(!captured.hard_limit_exceeded);
        assert!(captured.artifact_path.is_some());
        assert_eq!(
            tokio::fs::read_to_string(&artifact_path_for_assertion)
                .await
                .unwrap(),
            payload
        );
        assert!(limit_receiver.try_recv().is_err());
        let _ = tokio::fs::remove_file(&artifact_path_for_assertion).await;
    }

    #[tokio::test]
    async fn capture_bash_stream_reports_hard_limit() {
        let (mut writer, reader) = tokio::io::duplex(64);
        let artifact_path = std::env::temp_dir().join(format!(
            "santi-bash-capture-{}.txt",
            uuid::Uuid::new_v4().simple()
        ));
        let artifact_path_for_assertion = artifact_path.clone();
        let (limit_sender, mut limit_receiver) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            writer.write_all(b"abcdef").await.unwrap();
        });

        let captured = capture_bash_stream(
            reader,
            artifact_path,
            "stdout",
            BashOutputLimits {
                truncate_chars: 10,
                hard_bytes: 4,
            },
            limit_sender,
        )
        .await
        .unwrap();

        assert_eq!(captured.text, "abcd");
        assert_eq!(captured.raw_chars, 4);
        assert!(captured.truncated);
        assert!(captured.hard_limit_exceeded);
        assert_eq!(limit_receiver.try_recv().unwrap(), "stdout");
        assert_eq!(
            tokio::fs::read_to_string(&artifact_path_for_assertion)
                .await
                .unwrap(),
            "abcd"
        );
        let _ = tokio::fs::remove_file(&artifact_path_for_assertion).await;
    }

    #[tokio::test]
    async fn capture_bash_stream_allows_exact_hard_limit() {
        let (mut writer, reader) = tokio::io::duplex(64);
        let artifact_path = std::env::temp_dir().join(format!(
            "santi-bash-capture-{}.txt",
            uuid::Uuid::new_v4().simple()
        ));
        let artifact_path_for_cleanup = artifact_path.clone();
        let (limit_sender, mut limit_receiver) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            writer.write_all(b"abcd").await.unwrap();
        });

        let captured = capture_bash_stream(
            reader,
            artifact_path,
            "stdout",
            BashOutputLimits {
                truncate_chars: 10,
                hard_bytes: 4,
            },
            limit_sender,
        )
        .await
        .unwrap();

        assert_eq!(captured.text, "abcd");
        assert_eq!(captured.raw_chars, 4);
        assert!(!captured.truncated);
        assert!(!captured.hard_limit_exceeded);
        assert!(captured.artifact_path.is_none());
        assert!(limit_receiver.try_recv().is_err());
        let _ = tokio::fs::remove_file(&artifact_path_for_cleanup).await;
    }

    #[test]
    fn bash_model_tool_output_uses_preview_for_truncated_streams() {
        let result = BashToolResultEnvelope {
            feedback_msg: ToolCallFeedbackMsg::NormalToolCall,
            duration_ms: 12,
            bash_result: BashToolResult {
                exit_code: 0,
                stdout: "a".repeat(10_000),
                stderr: String::new(),
                stdout_chars: 15_000,
                stderr_chars: 0,
                stdout_truncated: true,
                stderr_truncated: false,
                stdout_artifact_path: Some("/tmp/stdout.txt".to_string()),
                stderr_artifact_path: None,
            },
        };

        let output = bash_model_tool_output(&result).unwrap();
        let bash_result = output.get("bash_result").unwrap();
        let preview = bash_result
            .get("stdout_preview")
            .and_then(serde_json::Value::as_str)
            .unwrap();

        assert!(preview.contains("[model-facing preview truncated]"));
        assert!(preview.len() < 10_000);
        assert_eq!(
            bash_result
                .get("stdout_chars")
                .and_then(serde_json::Value::as_u64),
            Some(15_000)
        );
        assert_eq!(
            bash_result
                .get("stdout_artifact_path")
                .and_then(serde_json::Value::as_str),
            Some("/tmp/stdout.txt")
        );
        assert!(bash_result.get("stdout").is_none());
    }
}
