use crate::{
    model::{
        message::{ActorType, MessagePart},
        runtime::Compact,
        session::SessionMessage,
    },
    provider::ProviderInputMessage,
};

pub fn to_input_message(message: &SessionMessage) -> Option<ProviderInputMessage> {
    let role = match message.message.actor_type {
        ActorType::Account => "user",
        ActorType::Soul => "assistant",
        ActorType::System => "system",
    };

    let content = message
        .message
        .content
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(text.as_str()),
            MessagePart::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if content.trim().is_empty() {
        None
    } else {
        Some(ProviderInputMessage {
            role: role.to_string(),
            content,
        })
    }
}

pub fn compact_to_input_message(compact: &Compact) -> Option<ProviderInputMessage> {
    let content = format!(
        "[compact {}-{}]\n{}",
        compact.start_session_seq, compact.end_session_seq, compact.summary
    );

    if compact.summary.trim().is_empty() {
        None
    } else {
        Some(ProviderInputMessage {
            role: "system".to_string(),
            content,
        })
    }
}
