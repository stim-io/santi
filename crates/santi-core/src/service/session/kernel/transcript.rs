use crate::{
    model::{
        message::{ActorType, MessagePart},
        session::SessionMessage,
    },
    provider::ProviderInputMessage,
};

pub fn to_input_message(message: &SessionMessage) -> Option<ProviderInputMessage> {
    let role = match message.message.actor_type {
        ActorType::Account => "user",
        ActorType::Soul => "assistant",
        ActorType::System => return None,
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
