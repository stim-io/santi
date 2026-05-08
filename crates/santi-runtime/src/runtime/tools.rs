use santi_core::port::provider::{FunctionCallOutput, ProviderFunctionCall, ProviderTool};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, time::Instant};
use tokio::{
    process::Command,
    sync::mpsc,
    time::{timeout, Duration},
};
use uuid::Uuid;

use crate::{runtime::context::ToolRuntimeContext, session::memory::SessionMemoryService};

mod bash_stream;
mod catalog;
mod dispatch_result;
mod github_https_git_env;

pub use self::bash_stream::{
    capture_bash_stream, nonzero_or_default, nonzero_or_default_usize, normalized_hard_bytes,
    BashOutputLimits, CapturedBashStream,
};
pub use self::dispatch_result::bash_model_tool_output;
use self::dispatch_result::{
    build_bash_dispatch_result, build_failed_dispatch_result, build_success_dispatch_result,
    parse_tool_args,
};
pub use self::github_https_git_env::github_git_env_from;
use self::github_https_git_env::github_https_git_env;

pub const DEFAULT_BASH_TIMEOUT_SECS: u64 = 24 * 60 * 60;
pub const DEFAULT_BASH_OUTPUT_TRUNCATE_CHARS: usize = 10_000;
pub const DEFAULT_BASH_OUTPUT_HARD_BYTES: usize = 1024 * 1024;

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

    fn bash_truncate_chars(&self) -> usize {
        self.bash_output_truncate_chars
    }

    fn bash_hard_bytes(&self) -> usize {
        self.bash_output_hard_bytes
    }

    pub fn render_tooling_instructions(&self) -> Option<String> {
        Some(catalog::tooling_instructions())
    }

    pub fn provider_tools(&self) -> Vec<ProviderTool> {
        catalog::provider_tools()
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
            truncate_chars: self.bash_truncate_chars(),
            hard_bytes: self.bash_hard_bytes(),
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
                    hard_bytes = self.bash_hard_bytes(),
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
