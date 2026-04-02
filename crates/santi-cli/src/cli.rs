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
    Chat {
        #[command(flatten)]
        command: ChatCommand,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
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
    Chat {
        #[command(flatten)]
        command: ChatCommand,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
    Soul {
        #[command(subcommand)]
        command: SoulCommand,
    },
}

#[derive(Debug, clap::Args)]
pub struct ChatCommand {
    #[arg(long)]
    pub session: Option<String>,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub wait: bool,

    pub message: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    Create,
    Get {
        session_id: String,
    },
    Fork {
        session_id: String,
        #[arg(long = "fork-point")]
        fork_point: i64,
    },
    Send {
        session_id: String,
        #[arg(long)]
        raw: bool,
        #[arg(long)]
        wait: bool,
    },
    Compact {
        session_id: String,
    },
    Compacts {
        session_id: String,
        #[arg(long)]
        raw: bool,
    },
    Effects {
        session_id: String,
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
    Get { session_id: String },
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

#[derive(Debug, Subcommand)]
pub enum AdminCommand {
    Hooks {
        #[command(subcommand)]
        command: HookAdminCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum HookAdminCommand {
    Reload,
}
