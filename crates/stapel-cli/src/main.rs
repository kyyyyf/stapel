mod check;
mod close;
mod hook;
mod init;
mod new;
mod ok;
mod repo;
mod status;
mod tokens;

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
        /// One-time grant from the permission dialog; set by the guard, never by hand.
        #[arg(long, hide = true)]
        grant: Option<String>,
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
        /// One-time grant from the permission dialog; set by the guard, never by hand.
        #[arg(long, hide = true)]
        grant: Option<String>,
    },
    /// The token journal: record model calls and show totals by role.
    #[command(args_conflicts_with_subcommands = true)]
    Tokens {
        #[command(subcommand)]
        action: Option<TokensAction>,
        /// The ticket key for the report; without it, every ticket.
        key: Option<String>,
    },
    /// Check that every step's tests failed at its RED commit and pass at its GREEN commit and HEAD.
    Check {
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
        /// Print the steps, their commits and their tests without running anything.
        #[arg(long)]
        list: bool,
    },
    /// Hook entry points called by Claude Code; not meant to be run by hand.
    #[command(subcommand)]
    Hook(HookCommand),
}

#[derive(Subcommand)]
enum TokensAction {
    /// Record one model call: measured counts, or an estimate.
    Add {
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        role: String,
        /// The model; defaults to the role's model in stapel.toml.
        #[arg(long, allow_hyphen_values = true)]
        model: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        input: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        output: Option<String>,
        #[arg(long = "cache-read", allow_hyphen_values = true)]
        cache_read: Option<String>,
        #[arg(long = "cache-write", allow_hyphen_values = true)]
        cache_write: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        estimate: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        step: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        note: Option<String>,
    },
    /// Record the session totals of a Claude Code transcript's last cost-state line.
    Session { transcript: std::path::PathBuf },
    /// Record the measured usage of a Claude Code transcript (main or subagent file).
    Import {
        transcript: std::path::PathBuf,
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        role: String,
        /// Count messages from this time (YYYY-MM-DDTHH:MM:SSZ).
        #[arg(long)]
        since: Option<String>,
        /// Count messages before this time (YYYY-MM-DDTHH:MM:SSZ).
        #[arg(long)]
        until: Option<String>,
    },
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
        Command::Tokens { action, key } => match action {
            Some(TokensAction::Add {
                key,
                role,
                model,
                input,
                output,
                cache_read,
                cache_write,
                estimate,
                step,
                note,
            }) => tokens::add(tokens::Add {
                key,
                role,
                model,
                input,
                output,
                cache_read,
                cache_write,
                estimate,
                step,
                note,
            }),
            Some(TokensAction::Import {
                transcript,
                key,
                role,
                since,
                until,
            }) => tokens::import(tokens::Import {
                transcript,
                key,
                role,
                since,
                until,
            }),
            Some(TokensAction::Session { transcript }) => tokens::session(&transcript),
            None => match tokens::report(key.as_deref()) {
                Ok(code) => return code,
                Err(message) => Err(message),
            },
        },
        Command::Close { key, reason, grant } => {
            close::run(key.as_deref(), &reason, grant.as_deref())
        }
        Command::Ok { args, grant } => match args.as_slice() {
            [section] => ok::run(None, section, grant.as_deref()),
            [key, section] => ok::run(Some(key), section, grant.as_deref()),
            _ => unreachable!("clap limits the arguments"),
        },
        Command::Hook(HookCommand::PreToolUse) => return hook::pre_tool_use(),
        Command::Check { key, list } => return check::run(key.as_deref(), list),
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
