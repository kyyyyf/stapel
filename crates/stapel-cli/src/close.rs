//! `stapel close [KEY] --reason <text>`: a person closes a ticket (STP-2 AC-14).

use crate::repo;
use stapel_core::dialog::close_facts;
use stapel_core::grant::consume;
use stapel_core::identity::{in_agent_shell, user_name};
use stapel_core::state::{Closed, save};
use stapel_core::tickets::{Status, resolve};
use stapel_core::time::now_rfc3339;

pub fn run(key: Option<&str>, reason: &str, grant: Option<&str>) -> Result<(), String> {
    if in_agent_shell() && grant.is_none() {
        return Err(
            "only a person closes a ticket: this shell belongs to an agent (CLAUDECODE or \
             CLAUDE_CODE_ENTRYPOINT is set); ask the agent to run `stapel close` so that Claude \
             Code shows you the permission dialog, or, from a Zed task started from a Claude Code \
             shell, run `env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT stapel close`"
                .into(),
        );
    }
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("a closing reason is required: --reason \"<why the ticket is done>\"".into());
    }
    let (root, _config) = repo::open()?;
    let by = user_name(&root)?;
    let ticket = resolve(&root, key)?;
    let mut state = match ticket.status {
        Status::Open(state) => state,
        Status::Closed(state) => {
            let c = state.closed.expect("closed");
            return Err(format!(
                "ticket {} is already closed by {} at {}: {}",
                ticket.key, c.by, c.at, c.reason
            ));
        }
        Status::Legacy(_) => {
            return Err(format!(
                "ticket {} has a legacy (STP-1) state.json; close it by hand",
                ticket.key
            ));
        }
        Status::Unreadable(r) => return Err(r),
    };
    if let Some(token) = grant {
        consume(&root, token, &close_facts(&ticket.key, reason))?;
    }
    state.closed = Some(Closed {
        by,
        at: now_rfc3339(),
        reason: reason.to_string(),
    });
    let dropped_flag = state.extra.remove("build").is_some();
    save(&ticket.dir.join("state.json"), &state)?;
    println!("closed: {}", ticket.key);
    if dropped_flag {
        println!("removed: build.allowed (phase 0 hand permit)");
    }
    let c = state.closed.as_ref().expect("just set");
    stapel_core::journal::record_decision(
        &ticket.dir,
        serde_json::json!({ "by": c.by, "action": "close", "reason": c.reason }),
    )
}
