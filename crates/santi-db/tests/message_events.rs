use santi_core::{
    error::Error,
    model::message::{
        ActorType, Message, MessageContent, MessageEventPayload, MessageInsertItem, MessagePart,
        MessagePartPatch, MessageState,
    },
};
use santi_db::message_events::apply_message_event;

fn pending_message(parts: Vec<MessagePart>) -> Message {
    Message {
        id: "msg_1".to_string(),
        actor_type: ActorType::Account,
        actor_id: "acct_1".to_string(),
        content: MessageContent { parts },
        state: MessageState::Pending,
        version: 1,
        deleted_at: None,
        created_at: "now".to_string(),
        updated_at: "now".to_string(),
    }
}

#[test]
fn patch_updates_part() {
    let message = pending_message(vec![MessagePart::Text {
        text: "hello".to_string(),
    }]);

    let updated = apply_message_event(
        message,
        &ActorType::Account,
        "acct_1",
        1,
        &MessageEventPayload::Patch {
            patches: vec![MessagePartPatch {
                index: 0,
                merge: serde_json::json!({ "text": "world" }),
            }],
        },
    )
    .unwrap();

    assert_eq!(updated.version, 2);
    assert_eq!(
        updated.content.parts,
        vec![MessagePart::Text {
            text: "world".to_string()
        }]
    );
}

#[test]
fn remove_rejects_bounds() {
    let message = pending_message(vec![MessagePart::Text {
        text: "hello".to_string(),
    }]);

    let err = apply_message_event(
        message,
        &ActorType::Account,
        "acct_1",
        1,
        &MessageEventPayload::Remove { indexes: vec![1] },
    )
    .unwrap_err();

    assert_eq!(
        err,
        Error::InvalidInput {
            message: "remove index out of bounds: 1".to_string(),
        }
    );
}

#[test]
fn fixed_message_rejects() {
    let mut message = pending_message(vec![MessagePart::Text {
        text: "hello".to_string(),
    }]);
    message.state = MessageState::Fixed;

    let err = apply_message_event(
        message,
        &ActorType::Account,
        "acct_1",
        1,
        &MessageEventPayload::Delete { reason: None },
    )
    .unwrap_err();

    assert_eq!(
        err,
        Error::InvalidInput {
            message: "fixed messages cannot be mutated".to_string(),
        }
    );
}

#[test]
fn insert_preserves_indexes() {
    let message = pending_message(vec![MessagePart::Text {
        text: "a".to_string(),
    }]);

    let updated = apply_message_event(
        message,
        &ActorType::Account,
        "acct_1",
        1,
        &MessageEventPayload::Insert {
            items: vec![MessageInsertItem {
                index: 1,
                part: MessagePart::Text {
                    text: "b".to_string(),
                },
            }],
        },
    )
    .unwrap();

    assert_eq!(
        updated.content.parts,
        vec![
            MessagePart::Text {
                text: "a".to_string()
            },
            MessagePart::Text {
                text: "b".to_string()
            }
        ]
    );
}
