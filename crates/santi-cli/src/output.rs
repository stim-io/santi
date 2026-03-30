use std::io::Read;

use futures::StreamExt;
use serde::Serialize;

use crate::backend::{BackendError, CliMessage, SendEvent, SendStream};

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

pub fn render_json<T: Serialize>(value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    println!("{text}");
    Ok(())
}

pub fn render_error(err: BackendError) -> String {
    err.to_string()
}
