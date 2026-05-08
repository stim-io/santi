use tokio::io::AsyncWriteExt;

use santi_runtime::runtime::tools::{
    bash_model_tool_output, capture_bash_stream, github_git_env_from, BashOutputLimits,
    BashToolResult, BashToolResultEnvelope, ToolCallFeedbackMsg,
};

#[tokio::test]
async fn capture_truncates_projection() {
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
async fn capture_reports_hard_limit() {
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
async fn capture_allows_exact_limit() {
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
fn model_output_uses_preview() {
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

#[test]
fn github_env_without_tokens() {
    let env = github_git_env_from(None, None);

    assert!(env.is_empty());
}

#[test]
fn github_env_prefers_token() {
    let env = github_git_env_from(Some("github-token"), Some("gh-token"));

    assert_eq!(env.get("GIT_CONFIG_COUNT").map(String::as_str), Some("1"));
    assert_eq!(
        env.get("GIT_CONFIG_KEY_0").map(String::as_str),
        Some("url.https://x-access-token:github-token@github.com/.insteadOf")
    );
    assert_eq!(
        env.get("GIT_CONFIG_VALUE_0").map(String::as_str),
        Some("https://github.com/")
    );
}
