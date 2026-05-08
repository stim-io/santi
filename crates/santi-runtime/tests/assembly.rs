use santi_core::model::{
    message::{ActorType, Message, MessageContent, MessagePart, MessageState},
    runtime::{AssemblyItem, AssemblyTarget, Compact, SoulSessionEntry, SoulSessionTargetType},
    session::{SessionMessage, SessionMessageRef},
};
use santi_runtime::session::send::assembly::assembly_to_provider_input;

#[test]
fn compact_replaces_messages() {
    let items = vec![
        message_item(1, "first"),
        message_item(2, "second"),
        compact_item(3, 1, 2, "summary one"),
        message_item(4, "third"),
    ];

    let input = assembly_to_provider_input(&items);
    let contents = input
        .into_iter()
        .map(|m| (m.role, m.content))
        .collect::<Vec<_>>();

    assert_eq!(
        contents,
        vec![
            (
                "system".to_string(),
                "[compact 1-2]\nsummary one".to_string()
            ),
            ("user".to_string(), "third".to_string()),
        ]
    );
}

#[test]
fn wider_compact_supersedes() {
    let items = vec![
        message_item(1, "first"),
        compact_item(2, 1, 1, "summary one"),
        message_item(2, "second"),
        compact_item(4, 1, 2, "summary two"),
    ];

    let input = assembly_to_provider_input(&items);
    let contents = input.into_iter().map(|m| m.content).collect::<Vec<_>>();

    assert_eq!(contents, vec!["[compact 1-2]\nsummary two".to_string()]);
}

fn message_item(session_seq: i64, text: &str) -> AssemblyItem {
    AssemblyItem {
        entry: SoulSessionEntry {
            soul_session_id: "ss_1".to_string(),
            target_type: SoulSessionTargetType::Message,
            target_id: format!("msg_{session_seq}"),
            soul_session_seq: session_seq,
            created_at: "now".to_string(),
        },
        target: AssemblyTarget::Message(SessionMessage {
            relation: SessionMessageRef {
                session_id: "sess_1".to_string(),
                message_id: format!("msg_{session_seq}"),
                session_seq,
                created_at: "now".to_string(),
            },
            message: Message {
                id: format!("msg_{session_seq}"),
                actor_type: ActorType::Account,
                actor_id: "acct_1".to_string(),
                content: MessageContent {
                    parts: vec![MessagePart::Text {
                        text: text.to_string(),
                    }],
                },
                state: MessageState::Fixed,
                version: 1,
                deleted_at: None,
                created_at: "now".to_string(),
                updated_at: "now".to_string(),
            },
        }),
    }
}

fn compact_item(
    soul_session_seq: i64,
    start_session_seq: i64,
    end_session_seq: i64,
    summary: &str,
) -> AssemblyItem {
    AssemblyItem {
        entry: SoulSessionEntry {
            soul_session_id: "ss_1".to_string(),
            target_type: SoulSessionTargetType::Compact,
            target_id: format!("compact_{soul_session_seq}"),
            soul_session_seq,
            created_at: "now".to_string(),
        },
        target: AssemblyTarget::Compact(Compact {
            id: format!("compact_{soul_session_seq}"),
            turn_id: format!("turn_{soul_session_seq}"),
            summary: summary.to_string(),
            start_session_seq,
            end_session_seq,
            created_at: "now".to_string(),
        }),
    }
}
