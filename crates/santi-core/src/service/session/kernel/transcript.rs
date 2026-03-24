use uuid::Uuid;

use crate::{model::message::Message, provider::ProviderInputMessage};

pub fn to_input_message(message: &Message) -> Option<ProviderInputMessage> {
    match message.r#type.as_str() {
        "user" | "assistant" => message.role.clone().map(|role| ProviderInputMessage {
            role,
            content: message.content.clone(),
        }),
        _ => None,
    }
}

pub fn merge_history(
    persisted: &[ProviderInputMessage],
    incoming: &[ProviderInputMessage],
) -> Vec<ProviderInputMessage> {
    let overlap = overlap_len(persisted, incoming);
    let mut merged = persisted.to_vec();
    merged.extend(incoming.iter().skip(overlap).cloned());
    merged
}

pub fn latest_message_by_role<'a>(
    messages: &'a [ProviderInputMessage],
    role: &str,
) -> Option<&'a ProviderInputMessage> {
    messages.iter().rev().find(|message| message.role == role)
}

pub fn build_chat_message(role: &'static str, content: String) -> Message {
    Message {
        id: format!("msg_{}", Uuid::new_v4().simple()),
        r#type: role.to_string(),
        role: Some(role.to_string()),
        content,
        created_at: now_seconds_string(),
    }
}

fn overlap_len(history: &[ProviderInputMessage], current: &[ProviderInputMessage]) -> usize {
    let max = history.len().min(current.len());

    for size in (1..=max).rev() {
        let history_slice = &history[history.len() - size..];
        let current_slice = &current[..size];

        if history_slice
            .iter()
            .zip(current_slice.iter())
            .all(|(left, right)| left.role == right.role && left.content == right.content)
        {
            return size;
        }
    }

    0
}

fn now_seconds_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
