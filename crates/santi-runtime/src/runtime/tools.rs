use serde::{Serialize, Serializer};
use std::{path::PathBuf, process::Stdio, time::Instant};
use tokio::{io::AsyncReadExt, process::Command, time::{timeout, Duration}};

use crate::{runtime::context::ToolRuntimeContext, session::memory::SessionMemoryService};

#[derive(Clone)]
pub struct ToolExecutor {
    memory_service: SessionMemoryService,
    runtime_root: PathBuf,
    execution_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallOk {
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn new(memory_service: SessionMemoryService, runtime_root: String, execution_root: String) -> Self {
        Self {
            memory_service,
            runtime_root: PathBuf::from(runtime_root),
            execution_root: PathBuf::from(execution_root),
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

        std::fs::create_dir_all(&ctx.soul_dir)
            .map_err(|err| format!("failed to create soul_dir: {err}"))?;
        std::fs::create_dir_all(&ctx.session_dir)
            .map_err(|err| format!("failed to create session_dir: {err}"))?;

        let workdir = resolve_cwd(ctx, input.cwd.as_deref())?;
        std::fs::create_dir_all(&workdir)
            .map_err(|err| format!("failed to create workdir: {err}"))?;

        let started_at = Instant::now();
        let mut child = Command::new("/bin/bash")
            .arg("-lc")
            .arg(&input.command)
            .current_dir(&workdir)
            .env("SANTI_RUNTIME_SOUL_DIR", &ctx.soul_dir)
            .env("SANTI_RUNTIME_SESSION_DIR", &ctx.session_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| format!("failed to spawn bash: {err}"))?;

        let mut stdout = child.stdout.take().ok_or_else(|| "missing child stdout".to_string())?;
        let mut stderr = child.stderr.take().ok_or_else(|| "missing child stderr".to_string())?;

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
                let status = wait_status.map_err(|err| format!("failed to wait for bash: {err}"))?;
                (ToolCallFeedbackMsg::NormalToolCall, status.code().unwrap_or(-1))
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                (ToolCallFeedbackMsg::ToolCallTimeout, -1)
            }
        };

        let duration_ms = started_at.elapsed().as_millis();
        let stdout = stdout_task.await.map_err(|err| format!("stdout task failed: {err}"))?;
        let stderr = stderr_task.await.map_err(|err| format!("stderr task failed: {err}"))?;

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
            soul_dir: self.runtime_root.join("souls").join(soul_id),
            session_dir: self.runtime_root.join("sessions").join(session_id),
            fallback_cwd: self.execution_root.join(soul_id).join(session_id),
        }
    }
}
