use std::path::PathBuf;

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};

use super::{DEFAULT_BASH_OUTPUT_HARD_BYTES, DEFAULT_BASH_OUTPUT_TRUNCATE_CHARS};

#[derive(Debug, Clone, Copy)]
pub struct BashOutputLimits {
    pub truncate_chars: usize,
    pub hard_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedBashStream {
    pub text: String,
    pub raw_chars: u64,
    pub truncated: bool,
    pub artifact_path: Option<String>,
    pub hard_limit_exceeded: bool,
}

pub async fn capture_bash_stream<R>(
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

pub fn nonzero_or_default(value: u64, default: u64) -> u64 {
    if value == 0 {
        default
    } else {
        value
    }
}

pub fn nonzero_or_default_usize(value: usize, default: usize) -> usize {
    if value == 0 {
        default
    } else {
        value
    }
}

pub fn normalized_hard_bytes(hard_bytes: usize, truncate_chars: usize) -> usize {
    let hard_bytes = nonzero_or_default_usize(hard_bytes, DEFAULT_BASH_OUTPUT_HARD_BYTES);
    let truncate_floor =
        nonzero_or_default_usize(truncate_chars, DEFAULT_BASH_OUTPUT_TRUNCATE_CHARS);
    hard_bytes.max(truncate_floor)
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
