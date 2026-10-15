//! `stapel hook pre-tool-use`: reads a Claude Code PreToolUse payload from stdin.
//! `stapel hook post-tool-use`: the history check after an agent's commit (STP-14).

use stapel_core::guard::{Decision, HookInput, decide, holds_git_commit, stapel_root};
use std::io::Read;
use std::path::PathBuf;
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
            say!("{answer}");
            ExitCode::SUCCESS
        }
        Ok(Decision::Deny(reason)) => deny(&reason),
        Err(_) => deny("internal error in the check; the call is denied"),
    }
}

/// `stapel hook post-tool-use` (STP-14): after a commit command in a Bash call, the hook faults
/// of the ticket that `HEAD` names. Claude Code shows the agent stderr on exit 2 only, so every
/// other case, an error or a panic included, prints nothing and exits 0: advice, not a guard.
pub fn post_tool_use() -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));
    let lines = std::panic::catch_unwind(|| {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).ok()?;
        let root = hook_repository(&text)?;
        crate::check::hook_faults(&root)
    });
    match lines {
        Ok(Some(lines)) => {
            for line in lines {
                warn!("{line}");
            }
            ExitCode::from(DENY)
        }
        _ => ExitCode::SUCCESS,
    }
}

/// The stapel root of the hook repository when the payload is a Bash call with a commit command:
/// the first present source among the payload's `cwd`, `CLAUDE_PROJECT_DIR` and the working
/// directory, with no fallback to the next.
fn hook_repository(text: &str) -> Option<PathBuf> {
    let payload: serde_json::Value = serde_json::from_str(text).ok()?;
    if payload.get("tool_name")?.as_str()? != "Bash" {
        return None;
    }
    let command = payload.get("tool_input")?.get("command")?.as_str()?;
    if !holds_git_commit(command) {
        return None;
    }
    let here = std::env::current_dir().ok()?;
    let source = match payload.get("cwd").and_then(|c| c.as_str()) {
        Some(cwd) => here.join(cwd),
        None => match std::env::var_os("CLAUDE_PROJECT_DIR") {
            Some(dir) => here.join(dir),
            None => here,
        },
    };
    stapel_root(&source)
}

fn deny(reason: &str) -> ExitCode {
    warn!("stapel: {reason}");
    ExitCode::from(DENY)
}
