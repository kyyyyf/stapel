mod close;
mod hook;
mod init;
mod new;
mod ok;
mod repo;
mod status;

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
    /// Create a ticket: a folder with ticket.md and state.json.
    New {
        /// The ticket title.
        title: String,
        /// Link to the ticket in an external tracker.
        #[arg(long)]
        tracker: Option<String>,
    },
    /// Confirm a section of a ticket: `stapel ok [KEY] <section>`. Only a person confirms.
    Ok {
        /// The ticket key (optional) and the section id.
        #[arg(num_args = 1..=2, required = true, value_names = ["KEY", "SECTION"])]
        args: Vec<String>,
    },
    /// Show whose decision a ticket waits for and which confirmations went stale.
    Status {
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
    },
    /// Close a ticket: its confirmations stop permitting code writes. Only a person closes.
    Close {
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
        /// Why the ticket is done.
        #[arg(long)]
        reason: String,
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
        Command::New { title, tracker } => new::run(&title, tracker.as_deref()),
        Command::Close { key, reason } => close::run(key.as_deref(), &reason),
        Command::Ok { args } => match args.as_slice() {
            [section] => ok::run(None, section),
            [key, section] => ok::run(Some(key), section),
            _ => unreachable!("clap limits the arguments"),
        },
        Command::Hook(HookCommand::PreToolUse) => return hook::pre_tool_use(),
        Command::Status { key } => match status::run(key.as_deref()) {
            Ok(code) => return code,
            Err(message) => Err(message),
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("stapel: {message}");
            ExitCode::FAILURE
        }
    }
}
