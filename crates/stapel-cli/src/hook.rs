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
/// `stapel next` succeeds there, `stapel next:` and its text. Stdin is read for the payload's
/// `cwd` (the hook root); a run from a terminal waits for input. Everything else, a panic
/// included, prints nothing more and exits 0.
pub fn session_start() -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));
    let root = std::panic::catch_unwind(|| {
        let mut text = String::new();
        let _ = std::io::stdin().read_to_string(&mut text);
        let payload = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        token_root(&payload)
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
        let mut lines = crate::check::hook_faults(&root).unwrap_or_default();
        let trials = if stapel_core::parallel::operation_in_progress(&root) {
            Vec::new()
        } else {
            stapel_core::parallel::trial_merges(&root)
        };
        for trial in trials {
            match trial {
                stapel_core::parallel::Trial::Conflict {
                    later,
                    earlier,
                    files,
                } => lines.push(format!(
                    "stapel: {later} conflicts with {earlier} in {}; {later} waits for {earlier}",
                    files.join(", ")
                )),
                stapel_core::parallel::Trial::StopNotWritten(why) => {
                    lines.push(format!("stop not written: {why}"));
                }
                _ => {}
            }
        }
        if lines.is_empty() {
            return None;
        }
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
/// PostToolUse or PostToolUseFailure payload of a Bash call with a commit command: the hook root
/// of the payload (`token_root`).
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
    Some((event.to_string(), token_root(&payload)?))
}

fn deny(reason: &str) -> ExitCode {
    warn!("stapel: {reason}");
    ExitCode::from(DENY)
}

/// The default `[roles]` rows, used when `stapel.toml` has no table (STP-26).
const DEFAULT_ROLES: [(&str, &str); 4] = [
    ("stapel-builder", "builder"),
    ("stapel-drift", "drift"),
    ("stapel-reviewer", "reviewer"),
    ("stapel-spec-reviewer", "reviewer"),
];

/// The root of the hook repository: the stapel root of the input's `cwd`, else of
/// `CLAUDE_PROJECT_DIR`, else of the working directory. An empty source is absent; a `cwd`
/// outside a stapel repository gives way to the next source, the other sources do not.
fn token_root(payload: &serde_json::Value) -> Option<PathBuf> {
    let here = std::env::current_dir().ok()?;
    let from_cwd = payload
        .get("cwd")
        .and_then(|c| c.as_str())
        .filter(|c| !c.is_empty())
        .and_then(|c| stapel_root(&here.join(c)));
    if from_cwd.is_some() {
        return from_cwd;
    }
    let source = match std::env::var_os("CLAUDE_PROJECT_DIR").filter(|d| !d.is_empty()) {
        Some(dir) => here.join(dir),
        None => here,
    };
    stapel_root(&source)
}

/// The agent type as shown in a note: each control character as one space, cut to 64 characters.
fn shown_type(agent_type: &str) -> String {
    agent_type
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(64)
        .collect()
}

/// `stapel hook subagent-stop` (STP-26): one `measured` record per model for the subagent
/// transcript, into the one open ticket. Prints nothing and exits 0 in every case.
pub fn subagent_stop() -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(|| -> Option<()> {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).ok()?;
        let payload: serde_json::Value = serde_json::from_str(&text).ok()?;
        let agent_type = payload.get("agent_type")?.as_str()?;
        let path = PathBuf::from(payload.get("agent_transcript_path")?.as_str()?);
        let root = token_root(&payload)?;
        let config = crate::repo::load(&root).ok()?;
        let (role, note) = match &config.roles {
            Some(table) => table.get(agent_type).map(|r| (r.as_str(), false)),
            None => DEFAULT_ROLES
                .iter()
                .find(|(t, _)| *t == agent_type)
                .map(|(_, r)| (*r, false)),
        }
        .unwrap_or(("research", true));
        let note = note.then(|| format!("agent type {}", shown_type(agent_type)));
        crate::tokens::hook_records(
            &root,
            &path,
            role,
            note.as_deref(),
            "subagent-stop",
            crate::tokens::Reach::All,
        )
    });
    ExitCode::SUCCESS
}

/// `stapel hook stop` and `stapel hook session-end` (STP-26): the main transcript's unrecorded
/// part with the role `orchestrator`. `stop` waits 10 minutes after the transcript's last record
/// and ends at the last complete message; `session-end` takes everything. Prints nothing and
/// exits 0 in every case.
fn main_transcript_hook(name: &str, reach: crate::tokens::Reach) -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(|| -> Option<()> {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).ok()?;
        let payload: serde_json::Value = serde_json::from_str(&text).ok()?;
        let path = PathBuf::from(payload.get("transcript_path")?.as_str()?);
        let root = token_root(&payload)?;
        crate::repo::load(&root).ok()?;
        crate::tokens::hook_records(&root, &path, "orchestrator", None, name, reach)
    });
    ExitCode::SUCCESS
}

pub fn stop() -> ExitCode {
    main_transcript_hook("stop", crate::tokens::Reach::Stop)
}

pub fn session_end() -> ExitCode {
    main_transcript_hook("session-end", crate::tokens::Reach::All)
}
