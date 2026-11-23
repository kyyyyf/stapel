#[macro_use]
mod out;
mod brief;
mod build_commit;
mod cargo;
mod check;
mod close;
mod depends;
mod export;
mod fail;
mod findings;
mod hook;
mod init;
mod ledger;
mod lint;
mod new;
mod next;
mod ok;
mod page;
mod repo;
mod review;
mod status;
mod token_records;
mod tokens;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

/// Carries a development ticket from description to merge.
#[derive(Parser)]
#[command(name = "stapel", version = build_commit::version_text())]
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
        /// Keep the process files out of the shared history (first run only).
        #[arg(long)]
        local: bool,
    },
    /// Create a ticket: a folder with ticket.md and state.json.
    New {
        /// The ticket title.
        title: String,
        /// Link to the ticket in an external tracker.
        #[arg(long)]
        tracker: Option<String>,
    },
    /// Confirm a section, `stapel ok [KEY] <section>`, or every waiting section, `stapel ok KEY`.
    /// Only a person confirms.
    Ok {
        /// One argument: a section id, else a ticket key. Two: the ticket key and the section id.
        #[arg(num_args = 1..=2, required = true, value_names = ["KEY", "SECTION"])]
        args: Vec<String>,
        /// One-time grant from the permission dialog; set by the guard, never by hand.
        #[arg(long, hide = true)]
        grant: Option<String>,
    },
    /// Write the page of the waiting sections of a ticket to `.stapel/tickets/<KEY>/page.md`.
    Page {
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
    },
    /// Show whose decision a ticket waits for and which confirmations went stale.
    Status {
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
    },
    /// Name the next action of a ticket from the facts on disk. Writes nothing.
    Next {
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
        /// Print the action as one JSON line.
        #[arg(long)]
        json: bool,
    },
    /// Print the brief of one Plan step to a builder. Writes nothing.
    Brief {
        /// The ticket key.
        key: String,
        /// The Plan step, such as `2` or `2c`.
        step: String,
    },
    /// Write the dependency review pack of a ticket against each earlier parallel ticket.
    Depends {
        /// The ticket key; without it, the branch ticket.
        key: Option<String>,
    },
    /// Write the branch `export/<KEY>`: the ticket's commits without the process files.
    Export {
        /// The ticket key; without it, the current branch.
        #[arg(allow_hyphen_values = true)]
        key: Option<String>,
        /// The ref the commits are taken onto.
        #[arg(long, allow_hyphen_values = true)]
        onto: Option<String>,
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
        /// Print the outcomes history alone gives, without cargo; other steps read `needs-run`.
        #[arg(long, conflicts_with = "list")]
        history: bool,
        /// How much to run: steps, head or full; the flag wins over `[check] level`.
        #[arg(long)]
        level: Option<String>,
    },
    /// Report the tests a ticket names that do not exist and the step tests no ticket names.
    Lint {
        /// The ticket key; without it, the branch ticket or the only open ticket.
        key: Option<String>,
    },
    /// Run `cargo` under the machine-wide cargo lock; agents run cargo only this way.
    Cargo {
        /// The arguments of `cargo`, passed as given.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        args: Vec<String>,
    },
    /// Review findings kept in a ticket's findings.jsonl.
    #[command(subcommand)]
    Findings(FindingsAction),
    /// The code review rounds of a ticket.
    #[command(subcommand)]
    Review(ReviewAction),
    /// The process ledger kept in .stapel/process-ledger.jsonl.
    #[command(subcommand)]
    Ledger(LedgerAction),
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
    /// Append every record of a JSON lines file to the ticket's tokens.jsonl, all or nothing.
    #[command(name = "import-records")]
    ImportRecords {
        /// A regular UTF-8 file of JSON lines.
        file: String,
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
        /// One-time grant from the permission dialog; set by the guard, never by hand.
        #[arg(long, hide = true)]
        grant: Option<String>,
    },
    /// Record the session totals of a Claude Code transcript's last cost-state line.
    Session { transcript: std::path::PathBuf },
    /// Print the tokens and the attention numbers of a main transcript; writes nothing.
    Measure {
        /// A main transcript of Claude Code.
        file: std::path::PathBuf,
    },
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
enum FindingsAction {
    /// Append every record of a JSON lines file to the ticket's findings.jsonl, all or nothing.
    Import {
        /// A regular UTF-8 file of JSON lines.
        file: String,
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
    },
    /// Append triage records (a fate and more for a finding id) to the ticket's findings.jsonl, all
    /// or nothing.
    Triage {
        /// A regular UTF-8 file of JSON lines.
        file: String,
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
    },
}

#[derive(Subcommand)]
enum ReviewAction {
    /// Prepare one code review round of a ticket.
    Start {
        /// The round: a whole number of 1 or more, or `merge`.
        #[arg(long, allow_hyphen_values = true)]
        round: review::Round,
        /// The ticket key; without it the ticket of the current branch.
        key: Option<String>,
    },
}

#[derive(Subcommand)]
enum LedgerAction {
    /// Append one record to the process ledger.
    Add {
        /// `finding`, `catches` or `decision`.
        #[arg(long, allow_hyphen_values = true)]
        kind: String,
        /// The note; not empty.
        #[arg(long, allow_hyphen_values = true)]
        note: String,
        /// The ticket key; without it, the only open ticket.
        key: Option<String>,
        /// One-time grant from the permission dialog; set by the guard, never by hand.
        #[arg(long, hide = true)]
        grant: Option<String>,
    },
    /// Count, per rule and ticket, the findings inside the promise; writes nothing.
    Rules {
        /// How many of the newest tickets get a column; default 5.
        #[arg(long, allow_hyphen_values = true)]
        last: Option<String>,
    },
}

#[derive(Subcommand)]
enum HookCommand {
    /// Decide on a tool call: exit 0 allows it, exit 2 blocks it with the reason on stderr.
    PreToolUse,
    /// After a commit in a Bash call: exit 2 with the history faults on stderr, else silent.
    PostToolUse,
    /// At session start: the skill line and `stapel next` in a stapel repository, else silent.
    SessionStart,
    /// When a subagent stops: its token records into the ticket's journal; silent, exit 0.
    SubagentStop,
    /// When a turn ends: the main transcript's token records, at most every 10 minutes; silent, exit 0.
    Stop,
    /// When a session ends: the rest of the main transcript's token records; silent, exit 0.
    SessionEnd,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Init { prefix, local } => init::run(prefix, local),
        Command::New { title, tracker } => {
            return fail::finish(new::run(&title, tracker.as_deref()));
        }
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
            Some(TokensAction::ImportRecords { file, key, grant }) => {
                return fail::finish(token_records::import(
                    &file,
                    key.as_deref(),
                    grant.as_deref(),
                ));
            }
            Some(TokensAction::Session { transcript }) => tokens::session(&transcript),
            Some(TokensAction::Measure { file }) => {
                return fail::finish(tokens::measure(&file));
            }
            None => match tokens::report(key.as_deref()) {
                Ok(code) => return code,
                Err(message) => Err(message),
            },
        },
        Command::Close { key, reason, grant } => {
            if let Some(refusal) = fail::branch_refusal(key.as_deref(), false) {
                return refusal;
            }
            close::run(key.as_deref(), &reason, grant.as_deref())
        }
        Command::Ok { args, grant } => {
            let first = args.first().map(String::as_str);
            if let Some(refusal) = fail::branch_refusal(first, args.len() == 1) {
                return refusal;
            }
            match args.as_slice() {
                [arg] => ok::run_one(arg, grant.as_deref()),
                [key, section] => ok::run(Some(key), section, grant.as_deref()),
                _ => unreachable!("clap limits the arguments"),
            }
        }
        Command::Page { key } => {
            if let Some(refusal) = fail::branch_refusal(key.as_deref(), false) {
                return refusal;
            }
            page::run(key.as_deref())
        }
        Command::Findings(FindingsAction::Import { file, key }) => {
            return fail::finish(findings::import(&file, key.as_deref()));
        }
        Command::Findings(FindingsAction::Triage { file, key }) => {
            return fail::finish(findings::triage(&file, key.as_deref()));
        }
        Command::Review(ReviewAction::Start { key, round }) => {
            return fail::finish(review::start(key.as_deref(), round));
        }
        Command::Ledger(LedgerAction::Add {
            kind,
            note,
            key,
            grant,
        }) => {
            return fail::finish(ledger::add(&kind, &note, key.as_deref(), grant.as_deref()));
        }
        Command::Ledger(LedgerAction::Rules { last }) => {
            return fail::finish(ledger::rules(last.as_deref()));
        }
        Command::Hook(HookCommand::PreToolUse) => return hook::pre_tool_use(),
        Command::Hook(HookCommand::PostToolUse) => return hook::post_tool_use(),
        Command::Hook(HookCommand::SessionStart) => return hook::session_start(),
        Command::Hook(HookCommand::SubagentStop) => return hook::subagent_stop(),
        Command::Hook(HookCommand::Stop) => return hook::stop(),
        Command::Hook(HookCommand::SessionEnd) => return hook::session_end(),
        Command::Check {
            key,
            list,
            history,
            level,
        } => {
            return check::run(key.as_deref(), list, history, level.as_deref());
        }
        Command::Export { key, onto } => {
            return fail::finish(export::run(key.as_deref(), onto.as_deref()));
        }
        Command::Lint { key } => return lint::run(key.as_deref()),
        Command::Cargo { args } => return cargo::run(&args),
        Command::Brief { key, step } => return fail::finish(brief::run(&key, &step)),
        Command::Depends { key } => return fail::finish(depends::run(key.as_deref())),
        Command::Next { key, json } => return fail::finish(next::run(key.as_deref(), json)),
        Command::Status { key } => match status::run(key.as_deref()) {
            Ok(code) => return code,
            Err(message) => Err(message),
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            warn!("stapel: {message}");
            ExitCode::FAILURE
        }
    }
}
