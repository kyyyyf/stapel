//! `stapel review start KEY --round N`: the refusals before a code review round (STP-24 AC-1, AC-2).
//! A success writes and prints nothing yet.

use crate::check::read_head;
use crate::fail::Fail;
use crate::repo;
use stapel_core::build_inputs::check_base;
use stapel_core::journal::{self, Entry};
use stapel_core::tickets::{Status, resolve};
use stapel_core::triage::whole_round;
use stapel_core::worktree::dirty_paths;

pub fn start(key: &str, round: u64) -> Result<(), Fail> {
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    let ticket = resolve(&root, Some(key)).map_err(Fail::refused)?;
    let state = match &ticket.status {
        Status::Open(state) => state,
        Status::Closed(_) => {
            return Err(Fail::refused(format!(
                "ticket {} is closed; it has no review round",
                ticket.key
            )));
        }
        Status::Legacy(_) => {
            return Err(Fail::failed(format!(
                "ticket {} is in the STP-1 format; it has no review pack",
                ticket.key
            )));
        }
        Status::Unreadable(reason) => return Err(Fail::failed(reason.clone())),
    };
    if !(1..=2).contains(&round) {
        return Err(Fail::refused(
            "--round must be 1 or 2; after round 2 the person decides",
        ));
    }
    let dirty = dirty_paths(&root).map_err(Fail::failed)?;
    if !dirty.is_empty() {
        let first: Vec<&str> = dirty.iter().take(5).map(String::as_str).collect();
        return Err(Fail::refused(format!(
            "uncommitted changes: {}; commit them before a review round",
            first.join(", ")
        )));
    }
    let head = read_head(&root).map_err(Fail::failed)?;
    let Some(base) = &state.base else {
        return Err(Fail::failed(format!(
            "state.json of {} has no base commit",
            ticket.key
        )));
    };
    check_base(&root, base, &head).map_err(Fail::failed)?;
    if round == 2 {
        let path = ticket.dir.join("findings.jsonl");
        let entries = journal::read(&path).map_err(Fail::failed)?;
        let has_round_one = entries.into_iter().any(|(_, entry)| match entry {
            Entry::V1(map) | Entry::Legacy(map) => {
                map.get("stage").and_then(|s| s.as_str()) == Some("code-review")
                    && map.get("round").and_then(whole_round) == Some(1)
            }
            Entry::Problem(_) => false,
        });
        if !has_round_one {
            return Err(Fail::refused(format!(
                "round 2 needs the code review findings of round 1 in \
                 .stapel/tickets/{}/findings.jsonl",
                ticket.key
            )));
        }
    }
    Ok(())
}
