use axum::{body::Body, http::Request, http::StatusCode};
use serde_json::Value;
use stim_proto::{
    ContentPart, MessageContent, MessageEnvelope, MessageOperation, MessageState, MutationPayload,
    TextPart,
};

use crate::common::{
    bootstrap_test_app, request_json, request_text, start_mock_gateway, wait_for_reply_completion,
};

#[tokio::test]
async fn standalone_http_accepts_stim_protocol_envelope() {
    let (_dir, app) = bootstrap_test_app(start_mock_gateway().await).await;
    let create_envelope = MessageEnvelope {
        protocol_version: stim_proto::CURRENT_PROTOCOL_VERSION.into(),
        envelope_id: "env-stim-1".into(),
        message_id: "msg-stim-1".into(),
        conversation_id: "conv-stim-1".into(),
        sender_node_id: "stim-node-a".into(),
        sender_endpoint_id: "stim-endpoint-a".into(),
        created_at: "2026-04-15T00:00:00Z".into(),
        session_bootstrap: Some(stim_proto::SessionBootstrap {
            participants: vec!["stim-endpoint-a".into(), "santi-endpoint-b".into()],
            created_by: "stim-endpoint-a".into(),
            created_at: "2026-04-15T00:00:00Z".into(),
        }),
        sender_assertion: None,
        encryption_scope: None,
        recipient_key_refs: vec![],
        signature_ref: None,
        integrity_ref: None,
        state: MessageState::Pending,
        operation: MessageOperation::Create,
        base_version: None,
        new_version: 1,
        payload: MutationPayload::Create {
            content: MessageContent {
                parts: vec![ContentPart::Text(TextPart {
                    part_id: "part-1".into(),
                    revision: 1,
                    metadata: None,
                    text: "hello from stim envelope".into(),
                })],
                layout_hint: None,
            },
        },
    };

    let (status, submission) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/stim/envelopes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&create_envelope).unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        submission
            .pointer("/acknowledgement/ack_result")
            .and_then(Value::as_str),
        Some("applied")
    );
    assert_eq!(
        submission
            .pointer("/acknowledgement/ack_message_id")
            .and_then(Value::as_str),
        Some("msg-stim-1")
    );
    assert_eq!(submission.get("reply"), Some(&Value::Null));

    let insert_envelope = MessageEnvelope {
        protocol_version: stim_proto::CURRENT_PROTOCOL_VERSION.into(),
        envelope_id: "env-stim-2".into(),
        message_id: "msg-stim-1".into(),
        conversation_id: "conv-stim-1".into(),
        sender_node_id: "stim-node-a".into(),
        sender_endpoint_id: "stim-endpoint-a".into(),
        created_at: "2026-04-15T00:00:01Z".into(),
        session_bootstrap: None,
        sender_assertion: None,
        encryption_scope: None,
        recipient_key_refs: vec![],
        signature_ref: None,
        integrity_ref: None,
        state: MessageState::Pending,
        operation: MessageOperation::Insert,
        base_version: Some(1),
        new_version: 2,
        payload: MutationPayload::Insert {
            items: vec![stim_proto::InsertOperation {
                index: 1,
                part: ContentPart::Text(TextPart {
                    part_id: "part-2".into(),
                    revision: 1,
                    metadata: None,
                    text: "temporary extra part".into(),
                }),
            }],
        },
    };

    let (status, submission) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/stim/envelopes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&insert_envelope).unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        submission
            .pointer("/acknowledgement/ack_version")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(submission.get("reply"), Some(&Value::Null));

    let (status, messages) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/sessions/conv-stim-1/messages")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = messages.get("messages").and_then(Value::as_array).unwrap();
    assert_eq!(
        items[0].get("content_text").and_then(Value::as_str),
        Some("hello from stim envelope\n\ntemporary extra part")
    );

    let patch_envelope = MessageEnvelope {
        protocol_version: stim_proto::CURRENT_PROTOCOL_VERSION.into(),
        envelope_id: "env-stim-3".into(),
        message_id: "msg-stim-1".into(),
        conversation_id: "conv-stim-1".into(),
        sender_node_id: "stim-node-a".into(),
        sender_endpoint_id: "stim-endpoint-a".into(),
        created_at: "2026-04-15T00:00:02Z".into(),
        session_bootstrap: None,
        sender_assertion: None,
        encryption_scope: None,
        recipient_key_refs: vec![],
        signature_ref: None,
        integrity_ref: None,
        state: MessageState::Pending,
        operation: MessageOperation::Patch,
        base_version: Some(2),
        new_version: 3,
        payload: MutationPayload::Patch {
            patches: vec![stim_proto::PatchOperation {
                index: 0,
                merge: serde_json::json!({ "text": "hello from patched stim envelope" }),
            }],
        },
    };

    let (status, submission) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/stim/envelopes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&patch_envelope).unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        submission
            .pointer("/acknowledgement/ack_version")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(submission.get("reply"), Some(&Value::Null));

    let remove_envelope = MessageEnvelope {
        protocol_version: stim_proto::CURRENT_PROTOCOL_VERSION.into(),
        envelope_id: "env-stim-4".into(),
        message_id: "msg-stim-1".into(),
        conversation_id: "conv-stim-1".into(),
        sender_node_id: "stim-node-a".into(),
        sender_endpoint_id: "stim-endpoint-a".into(),
        created_at: "2026-04-15T00:00:03Z".into(),
        session_bootstrap: None,
        sender_assertion: None,
        encryption_scope: None,
        recipient_key_refs: vec![],
        signature_ref: None,
        integrity_ref: None,
        state: MessageState::Pending,
        operation: MessageOperation::Remove,
        base_version: Some(3),
        new_version: 4,
        payload: MutationPayload::Remove { indexes: vec![1] },
    };

    let (status, submission) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/stim/envelopes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&remove_envelope).unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        submission
            .pointer("/acknowledgement/ack_version")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(submission.get("reply"), Some(&Value::Null));

    let fix_envelope = MessageEnvelope {
        protocol_version: stim_proto::CURRENT_PROTOCOL_VERSION.into(),
        envelope_id: "env-stim-5".into(),
        message_id: "msg-stim-1".into(),
        conversation_id: "conv-stim-1".into(),
        sender_node_id: "stim-node-a".into(),
        sender_endpoint_id: "stim-endpoint-a".into(),
        created_at: "2026-04-15T00:00:04Z".into(),
        session_bootstrap: None,
        sender_assertion: None,
        encryption_scope: None,
        recipient_key_refs: vec![],
        signature_ref: None,
        integrity_ref: None,
        state: MessageState::Fixed,
        operation: MessageOperation::Fix,
        base_version: Some(4),
        new_version: 5,
        payload: MutationPayload::Fix {},
    };

    let (status, submission) = request_json(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/stim/envelopes")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&fix_envelope).unwrap()))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        submission
            .pointer("/acknowledgement/ack_version")
            .and_then(Value::as_u64),
        Some(5)
    );
    let reply_id = submission
        .pointer("/reply/reply_id")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    assert_eq!(
        submission.pointer("/reply/status").and_then(Value::as_str),
        Some("pending")
    );

    let (status, event_text) = request_text(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/stim/replies/{reply_id}/events"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(event_text.contains("output_text_delta"));
    assert!(event_text.contains("hello from gateway"));
    assert!(event_text.contains("completed"));
    assert!(event_text.contains("[DONE]"));

    let snapshot = wait_for_reply_completion(&app, &reply_id).await;
    assert_eq!(
        snapshot.get("output_text").and_then(Value::as_str),
        Some("hello from gateway")
    );
    assert_eq!(
        snapshot.get("status").and_then(Value::as_str),
        Some("completed")
    );

    let (status, messages) = request_json(
        &app,
        Request::builder()
            .method("GET")
            .uri("/api/v1/sessions/conv-stim-1/messages")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = messages.get("messages").and_then(Value::as_array).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0].get("content_text").and_then(Value::as_str),
        Some("hello from patched stim envelope")
    );
    assert_eq!(items[0].get("state").and_then(Value::as_str), Some("fixed"));
    assert_eq!(
        items[0].get("actor_type").and_then(Value::as_str),
        Some("account")
    );
    assert_eq!(
        items[1].get("content_text").and_then(Value::as_str),
        Some("hello from gateway")
    );
    assert_eq!(
        items[1].get("actor_type").and_then(Value::as_str),
        Some("soul")
    );
}
