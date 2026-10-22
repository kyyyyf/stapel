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

/// `stapel hook session-start` (STP-22): in a stapel repository, the skill line and, when
/// `stapel next` succeeds there, `stapel next:` and its text. Stdin is not read. Everything
/// else, a panic included, prints nothing more and exits 0.
pub fn session_start() -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));
    let root = std::panic::catch_unwind(|| {
        let here = std::env::current_dir().ok()?;
        let source = match std::env::var_os("CLAUDE_PROJECT_DIR").filter(|d| !d.is_empty()) {
            Some(dir) => here.join(dir),
            None => here,
        };
        stapel_root(&source)
    });
    let Ok(Some(root)) = root else {
        return ExitCode::SUCCESS;
    };
    say!("This repository follows the stapel process: load the skill stapel and follow it.");
    if let Ok(Ok(lines)) = std::panic::catch_unwind(|| crate::next::text_at(&root)) {
        say!("stapel next:");
        for line in lines {
            say!("{line}");
        }
    }
    ExitCode::SUCCESS
}

/// The hook events `stapel hook post-tool-use` answers (STP-14 AC-11, AC-19).
const POST_EVENTS: [&str; 2] = ["PostToolUse", "PostToolUseFailure"];

/// `stapel hook post-tool-use` (STP-14): after a commit command in a Bash call, the hook faults
/// of the ticket that `HEAD` names, as `additionalContext` in JSON on stdout with exit 0. Every
/// other case, an error or a panic included, prints nothing and exits 0: advice, not a guard.
pub fn post_tool_use() -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));
    let answer = std::panic::catch_unwind(|| {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).ok()?;
        let (event, root) = hook_repository(&text)?;
        let lines = crate::check::hook_faults(&root)?;
        Some(serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": event,
                "additionalContext": lines.join("\n"),
            }
        }))
    });
    if let Ok(Some(answer)) = answer {
        say!("{answer}");
    }
    ExitCode::SUCCESS
}

/// The payload's hook event and the stapel root of the hook repository when the payload is a
/// PostToolUse or PostToolUseFailure payload of a Bash call with a commit command: the root of the
/// first present source among the payload's `cwd`, `CLAUDE_PROJECT_DIR` and the working
/// directory, with no fallback to the next. An empty `cwd` or `CLAUDE_PROJECT_DIR` is absent.
fn hook_repository(text: &str) -> Option<(String, PathBuf)> {
    let payload: serde_json::Value = serde_json::from_str(text).ok()?;
    let event = payload.get("hook_event_name")?.as_str()?;
    if !POST_EVENTS.contains(&event) {
        return None;
    }
    if payload.get("tool_name")?.as_str()? != "Bash" {
        return None;
    }
    let command = payload.get("tool_input")?.get("command")?.as_str()?;
    if !holds_git_commit(command) {
        return None;
    }
    let here = std::env::current_dir().ok()?;
    let source = match payload
        .get("cwd")
        .and_then(|c| c.as_str())
        .filter(|c| !c.is_empty())
    {
        Some(cwd) => here.join(cwd),
        None => match std::env::var_os("CLAUDE_PROJECT_DIR").filter(|d| !d.is_empty()) {
            Some(dir) => here.join(dir),
            None => here,
        },
    };
    Some((event.to_string(), stapel_root(&source)?))
}

fn deny(reason: &str) -> ExitCode {
    warn!("stapel: {reason}");
    ExitCode::from(DENY)
}
