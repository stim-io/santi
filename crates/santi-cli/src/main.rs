mod backend;
mod cli;
mod config;
mod output;

use clap::Parser;

use crate::{
    backend::{api::ApiBackend, local::LocalBackend, CliBackend},
    cli::{
        AdminCommand, ApiCommand, BackendKind, ChatCommand, Cli, Command, HookAdminCommand,
        SessionCommand, SessionMemoryCommand, SoulCommand, SoulMemoryCommand,
    },
    config::Config,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let exit_code = match run().await {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            1
        }
    };

    std::process::exit(exit_code);
}

async fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let config = Config::from_env_and_cli(&cli)?;

    match cli.command {
        Command::Health => {
            let backend = build_backend(config.clone(), config.backend).await?;
            handle_health(backend.as_ref()).await?;
        }
        Command::Chat { command } => {
            let backend = build_backend(config.clone(), config.backend).await?;
            handle_chat_command(backend.as_ref(), command).await?;
        }
        Command::Session { command } => {
            let backend = build_backend(config.clone(), config.backend).await?;
            handle_session_command(backend.as_ref(), command).await?;
        }
        Command::Admin { command } => {
            let backend = build_backend(config.clone(), config.backend).await?;
            handle_admin_command(backend.as_ref(), command).await?;
        }
        Command::Soul { command } => {
            let backend = build_backend(config.clone(), config.backend).await?;
            handle_soul_command(backend.as_ref(), command).await?;
        }
        Command::Api { command } => match command {
            ApiCommand::Health => {
                let backend = build_backend(config.clone(), BackendKind::Api).await?;
                handle_health(backend.as_ref()).await?;
            }
            ApiCommand::Chat { command } => {
                let backend = build_backend(config.clone(), BackendKind::Api).await?;
                handle_chat_command(backend.as_ref(), command).await?;
            }
            ApiCommand::Session { command } => {
                let backend = build_backend(config.clone(), BackendKind::Api).await?;
                handle_session_command(backend.as_ref(), command).await?;
            }
            ApiCommand::Admin { command } => {
                let backend = build_backend(config.clone(), BackendKind::Api).await?;
                handle_admin_command(backend.as_ref(), command).await?;
            }
            ApiCommand::Soul { command } => {
                let backend = build_backend(config.clone(), BackendKind::Api).await?;
                handle_soul_command(backend.as_ref(), command).await?;
            }
        },
    }

    Ok(())
}

async fn build_backend(config: Config, kind: BackendKind) -> Result<Box<dyn CliBackend>, String> {
    match kind {
        BackendKind::Api => Ok(Box::new(ApiBackend::new(config))),
        BackendKind::Local => Ok(Box::new(LocalBackend::new(config).await?)),
    }
}

async fn handle_health(backend: &dyn CliBackend) -> Result<(), String> {
    let health = backend.health().await.map_err(output::render_error)?;
    output::render_json(&health)
}

async fn handle_session_command(
    backend: &dyn CliBackend,
    command: SessionCommand,
) -> Result<(), String> {
    match command {
        SessionCommand::Create => {
            let session = backend
                .create_session()
                .await
                .map_err(output::render_error)?;
            println!("{}", session.id);
        }
        SessionCommand::Get { session_id } => {
            let session = backend
                .get_session(session_id)
                .await
                .map_err(output::render_error)?;
            output::render_json(&session)?;
        }
        SessionCommand::Send {
            session_id,
            raw,
            wait,
        } => {
            let content = output::read_stdin().await?;
            if content.trim().is_empty() {
                return Err("expected stdin content".to_string());
            }
            let stream = backend
                .send_session(session_id, content, wait)
                .await
                .map_err(output::render_error)?;
            output::render_send_stream(stream, raw).await?;
        }
        SessionCommand::Compact { session_id } => {
            let summary = output::read_stdin().await?;
            if summary.trim().is_empty() {
                return Err("expected stdin summary content".to_string());
            }
            let compact = backend
                .compact_session(session_id, summary)
                .await
                .map_err(output::render_error)?;
            output::render_json(&compact)?;
        }
        SessionCommand::Messages { session_id } => {
            let messages = backend
                .list_messages(session_id)
                .await
                .map_err(output::render_error)?;
            output::render_messages(messages)?;
        }
        SessionCommand::Memory { command } => match command {
            SessionMemoryCommand::Set { session_id } => {
                let text = output::read_stdin().await?;
                let memory = backend
                    .set_session_memory(session_id, text)
                    .await
                    .map_err(output::render_error)?;
                output::render_json(&memory)?;
            }
        },
    }

    Ok(())
}

async fn handle_chat_command(backend: &dyn CliBackend, command: ChatCommand) -> Result<(), String> {
    let content = output::read_message_input(command.message).await?;
    if content.trim().is_empty() {
        return Err("expected message argument or stdin content".to_string());
    }

    let session_id = match command.session {
        Some(session_id) => session_id,
        None => {
            let session = backend
                .create_session()
                .await
                .map_err(output::render_error)?;
            session.id
        }
    };

    let stream = backend
        .send_session(session_id.clone(), content, command.wait)
        .await
        .map_err(output::render_error)?;
    output::render_session_hint(&session_id);
    output::render_send_stream(stream, command.raw).await
}

async fn handle_soul_command(backend: &dyn CliBackend, command: SoulCommand) -> Result<(), String> {
    match command {
        SoulCommand::Get => {
            let soul = backend
                .get_default_soul()
                .await
                .map_err(output::render_error)?;
            output::render_json(&soul)?;
        }
        SoulCommand::Memory { command } => match command {
            SoulMemoryCommand::Set => {
                let text = output::read_stdin().await?;
                let memory = backend
                    .set_default_soul_memory(text)
                    .await
                    .map_err(output::render_error)?;
                output::render_json(&memory)?;
            }
        },
    }

    Ok(())
}

async fn handle_admin_command(
    backend: &dyn CliBackend,
    command: AdminCommand,
) -> Result<(), String> {
    match command {
        AdminCommand::Hooks { command } => match command {
            HookAdminCommand::Reload => {
                let raw = output::read_stdin().await?;
                if raw.trim().is_empty() {
                    return Err("expected stdin hook payload".to_string());
                }
                let source = santi_runtime::hooks::HookSpecSource::from_json_str(&raw)
                    .map_err(|err| format!("parse hook payload failed: {err}"))?;
                let result = backend
                    .reload_hooks(source)
                    .await
                    .map_err(output::render_error)?;
                output::render_json(&result)?;
            }
        },
    }

    Ok(())
}
