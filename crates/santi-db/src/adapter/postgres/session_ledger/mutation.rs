use serde_json::{Map, Value};

use santi_core::{
    error::{Error, Result},
    model::message::{
        ActorType, Message, MessageContent, MessageEventPayload, MessagePart, MessageState,
    },
};

pub(super) fn apply_message_event_to_message(
    mut message: Message,
    actor_type: &ActorType,
    actor_id: &str,
    base_version: i64,
    payload: &MessageEventPayload,
) -> Result<Message> {
    if &message.actor_type != actor_type || message.actor_id != actor_id {
        return Err(Error::InvalidInput {
            message: "only the original actor may mutate a message".to_string(),
        });
    }

    if message.version != base_version {
        return Err(Error::InvalidInput {
            message: format!(
                "message version mismatch: expected {}, got {}",
                message.version, base_version
            ),
        });
    }

    if message.deleted_at.is_some() {
        return Err(Error::InvalidInput {
            message: "deleted messages cannot be mutated".to_string(),
        });
    }

    if message.state == MessageState::Fixed {
        return Err(Error::InvalidInput {
            message: "fixed messages cannot be mutated".to_string(),
        });
    }

    match payload {
        MessageEventPayload::Patch { patches } => {
            let mut parts = message.content.parts.clone();
            for patch in patches {
                let index = valid_index(parts.len(), patch.index, "patch")?;
                parts[index] = merge_message_part(&parts[index], &patch.merge)?;
            }
            message.content = MessageContent { parts };
        }
        MessageEventPayload::Insert { items } => {
            let mut parts = message.content.parts.clone();
            let mut sorted_items = items.clone();
            sorted_items.sort_by_key(|item| item.index);
            for item in sorted_items {
                let index = valid_insert_index(parts.len(), item.index)?;
                parts.insert(index, item.part);
            }
            message.content = MessageContent { parts };
        }
        MessageEventPayload::Remove { indexes } => {
            let mut unique_indexes = indexes.clone();
            unique_indexes.sort_unstable();
            unique_indexes.dedup();
            let parts_len = message.content.parts.len();
            for index in &unique_indexes {
                let _ = valid_index(parts_len, *index, "remove")?;
            }

            let mut parts = message.content.parts.clone();
            for index in unique_indexes.into_iter().rev() {
                parts.remove(index as usize);
            }
            message.content = MessageContent { parts };
        }
        MessageEventPayload::Fix => {
            message.state = MessageState::Fixed;
        }
        MessageEventPayload::Delete { .. } => {
            message.deleted_at = Some(String::new());
        }
    }

    message.version += 1;
    Ok(message)
}

fn valid_index(len: usize, raw: i64, action: &str) -> Result<usize> {
    if raw < 0 || raw as usize >= len {
        return Err(Error::InvalidInput {
            message: format!("{action} index out of bounds: {raw}"),
        });
    }
    Ok(raw as usize)
}

fn valid_insert_index(len: usize, raw: i64) -> Result<usize> {
    if raw < 0 || raw as usize > len {
        return Err(Error::InvalidInput {
            message: format!("insert index out of bounds: {raw}"),
        });
    }
    Ok(raw as usize)
}

fn merge_message_part(part: &MessagePart, merge: &Value) -> Result<MessagePart> {
    let mut base = serde_json::to_value(part).map_err(|err| Error::Internal {
        message: format!("message part serialize failed: {err}"),
    })?;

    let merge_object = merge.as_object().ok_or(Error::InvalidInput {
        message: "patch merge must be an object".to_string(),
    })?;

    let base_object = base.as_object_mut().ok_or(Error::Internal {
        message: "message part must serialize to an object".to_string(),
    })?;

    merge_json_object(base_object, merge_object);

    serde_json::from_value(base).map_err(|err| Error::InvalidInput {
        message: format!("patch produced invalid message part: {err}"),
    })
}

fn merge_json_object(base: &mut Map<String, Value>, merge: &Map<String, Value>) {
    for (key, value) in merge {
        base.insert(key.clone(), value.clone());
    }
}

#[cfg(test)]
mod tests {
    use santi_core::{
        error::Error,
        model::message::{
            ActorType, Message, MessageContent, MessageEventPayload, MessageInsertItem,
            MessagePart, MessagePartPatch, MessageState,
        },
    };

    use super::apply_message_event_to_message;

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
    fn patch_updates_part_and_increments_version() {
        let message = pending_message(vec![MessagePart::Text {
            text: "hello".to_string(),
        }]);

        let updated = apply_message_event_to_message(
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
    fn remove_rejects_out_of_bounds_index() {
        let message = pending_message(vec![MessagePart::Text {
            text: "hello".to_string(),
        }]);

        let err = apply_message_event_to_message(
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
    fn fixed_message_rejects_further_mutation() {
        let mut message = pending_message(vec![MessagePart::Text {
            text: "hello".to_string(),
        }]);
        message.state = MessageState::Fixed;

        let err = apply_message_event_to_message(
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
    fn insert_uses_exact_indexes_without_gap_creation() {
        let message = pending_message(vec![MessagePart::Text {
            text: "a".to_string(),
        }]);

        let updated = apply_message_event_to_message(
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
}
