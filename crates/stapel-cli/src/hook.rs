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
        .map_err(|e| format!("hook input could not be read: {e}"))
        .and_then(|_| HookInput::parse(&text));
    let input = match input {
        Ok(input) => input,
        Err(reason) => return deny(&reason),
    };

    let cwd = std::env::current_dir().unwrap_or_default();
    let project_dir = std::env::var_os("CLAUDE_PROJECT_DIR").map(std::path::PathBuf::from);
    // Claude Code lets the call through on any exit code but 2, so a panic must not escape.
    std::panic::set_hook(Box::new(|_| {}));
    let decision = std::panic::catch_unwind(|| decide(&input, &cwd, project_dir.as_deref()));
    match decision {
        Ok(Decision::Allow) => ExitCode::SUCCESS,
        Ok(Decision::Ask { reason, command }) => {
            let mut updated = input.tool_input.clone();
            updated["command"] = serde_json::Value::String(command);
            let answer = serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "ask",
                    "permissionDecisionReason": reason,
                    "updatedInput": updated,
                }
            });
            println!("{answer}");
            ExitCode::SUCCESS
        }
        Ok(Decision::Deny(reason)) => deny(&reason),
        Err(_) => deny("internal error in the check; the call is denied"),
    }
}

fn deny(reason: &str) -> ExitCode {
    eprintln!("stapel: {reason}");
    ExitCode::from(DENY)
}
