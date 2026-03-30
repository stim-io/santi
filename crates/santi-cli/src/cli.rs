use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, ValueEnum, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    Local,
    Api,
}

#[derive(Debug, Parser)]
#[command(name = "santi-cli")]
pub struct Cli {
    #[arg(long, global = true, value_enum)]
    pub backend: Option<BackendKind>,

    #[arg(long, global = true)]
    pub base_url: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Health,
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Soul {
        #[command(subcommand)]
        command: SoulCommand,
    },
    Api {
        #[command(subcommand)]
        command: ApiCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ApiCommand {
    Health,
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Soul {
        #[command(subcommand)]
        command: SoulCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    Create,
    Get {
        session_id: String,
    },
    Send {
        session_id: String,
        #[arg(long)]
        raw: bool,
        #[arg(long)]
        wait: bool,
    },
    Messages {
        session_id: String,
    },
    Memory {
        #[command(subcommand)]
        command: SessionMemoryCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum SessionMemoryCommand {
    Set { session_id: String },
}

#[derive(Debug, Subcommand)]
pub enum SoulCommand {
    Get,
    Memory {
        #[command(subcommand)]
        command: SoulMemoryCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum SoulMemoryCommand {
    Set,
}
