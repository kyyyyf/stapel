//! `stapel hook pre-tool-use`: reads a Claude Code PreToolUse payload from stdin.

use stapel_core::guard::{Decision, HookInput, decide};
use std::io::Read;
use std::process::ExitCode;

/// Claude Code blocks the tool call on this exit code and shows stderr to the agent.
const DENY: u8 = 2;

pub fn pre_tool_use() -> ExitCode {
    let mut text = String::new();
    let input = std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| format!("вход хука не прочитан: {e}"))
        .and_then(|_| HookInput::parse(&text));
    let input = match input {
        Ok(input) => input,
        Err(reason) => return deny(&reason),
    };

    let cwd = std::env::current_dir().unwrap_or_default();
    let project_dir = std::env::var_os("CLAUDE_PROJECT_DIR").map(std::path::PathBuf::from);
    match decide(&input, &cwd, project_dir.as_deref()) {
        Decision::Allow => ExitCode::SUCCESS,
        Decision::Deny(reason) => deny(&reason),
    }
}

fn deny(reason: &str) -> ExitCode {
    eprintln!("stapel: {reason}");
    ExitCode::from(DENY)
}
