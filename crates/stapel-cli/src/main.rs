mod hook;
mod init;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

/// Carries a development ticket from description to merge.
#[derive(Parser)]
#[command(name = "stapel", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Prepare this repository: create .stapel/ and install Claude Code hooks.
    Init {
        /// Ticket key prefix, 2 to 8 uppercase Latin letters (STP gives STP-1, STP-2, ...).
        /// Asked interactively when omitted.
        #[arg(long)]
        prefix: Option<String>,
    },
    /// Hook entry points called by Claude Code; not meant to be run by hand.
    #[command(subcommand)]
    Hook(HookCommand),
}

#[derive(Subcommand)]
enum HookCommand {
    /// Decide on a tool call: exit 0 allows it, exit 2 blocks it with the reason on stderr.
    PreToolUse,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Init { prefix } => init::run(prefix),
        Command::Hook(HookCommand::PreToolUse) => return hook::pre_tool_use(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("stapel: {message}");
            ExitCode::FAILURE
        }
    }
}
