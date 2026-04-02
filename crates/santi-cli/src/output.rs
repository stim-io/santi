use std::io::Read;

use futures::StreamExt;
use serde::Serialize;

use crate::backend::{
    BackendError, CliCompact, CliMessage, CliSessionEffect, SendEvent, SendStream,
};

pub async fn read_stdin() -> Result<String, String> {
    let mut chunks = Vec::new();
    std::io::stdin()
        .read_to_end(&mut chunks)
        .map_err(|err| err.to_string())?;
    String::from_utf8(chunks).map_err(|err| err.to_string())
}

pub async fn read_message_input(message: Option<String>) -> Result<String, String> {
    match message {
        Some(message) => Ok(message),
        None => read_stdin().await,
    }
}

pub fn render_session_hint(session_id: &str) {
    eprintln!("session: {session_id}");
}

pub async fn render_send_stream(mut stream: SendStream, raw: bool) -> Result<(), String> {
    let mut saw_text = false;

    while let Some(event) = stream.next().await {
        match event.map_err(render_error)? {
            SendEvent::OutputTextDelta(delta) => {
                if raw {
                    println!(
                        "{{\"type\":\"response.output_text.delta\",\"delta\":{}}}",
                        serde_json::to_string(&delta).map_err(|err| err.to_string())?
                    );
                } else {
                    print!("{delta}");
                    saw_text = true;
                }
            }
            SendEvent::Completed => {
                if raw {
                    println!("{{\"type\":\"response.completed\"}}");
                }
            }
        }
    }

    if raw {
        println!("[DONE]");
    } else if saw_text {
        println!();
    }

    Ok(())
}

pub fn render_messages(messages: Vec<CliMessage>) -> Result<(), String> {
    render_json(&messages)
}

pub fn render_compacts(compacts: &[CliCompact]) -> Result<(), String> {
    if compacts.is_empty() {
        println!("no compacts recorded for this session");
        return Ok(());
    }

    for (index, compact) in compacts.iter().enumerate() {
        if index > 0 {
            println!();
        }

        println!("compact #{}", index + 1);
        println!("  id: {}", compact.id);
        println!("  turn_id: {}", compact.turn_id);
        println!("  summary: {}", compact.summary);
        println!("  start_session_seq: {}", compact.start_session_seq);
        println!("  end_session_seq: {}", compact.end_session_seq);
        println!("  created_at: {}", compact.created_at);
    }

    Ok(())
}

pub fn render_session_effects(effects: &[CliSessionEffect]) {
    if effects.is_empty() {
        println!("no session effects recorded");
        return;
    }

    for (index, effect) in effects.iter().enumerate() {
        if index > 0 {
            println!();
        }

        println!("effect #{}", index + 1);
        println!("  effect_type: {}", effect.effect_type);
        println!("  status: {}", effect.status);
        println!("  source_hook_id: {}", effect.source_hook_id);
        println!("  source_turn_id: {}", effect.source_turn_id);
        println!(
            "  result_ref: {}",
            effect.result_ref.as_deref().unwrap_or("-")
        );
        println!(
            "  error_text: {}",
            effect.error_text.as_deref().unwrap_or("-")
        );

        if effect.effect_type == "hook_fork_handoff" {
            if let Some(child_session_id) = effect.result_ref.as_deref() {
                println!("  next_step: santi-cli session messages {child_session_id}");
                println!("  follow_up: santi-cli chat --session {child_session_id} <message>");
            }
        }
    }
}

pub fn render_json<T: Serialize>(value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    println!("{text}");
    Ok(())
}

pub fn render_error(err: BackendError) -> String {
    err.to_string()
}
