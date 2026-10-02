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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Init { prefix } => init::run(prefix),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("stapel: {message}");
            ExitCode::FAILURE
        }
    }
}
