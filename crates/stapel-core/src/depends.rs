//! The dependency review of parallel tickets (STP-27 AC-20 to AC-22): the pairs of a ticket with
//! its earlier parallel tickets, their rounds, the rounds still open and the stops a done round
//! gives.

use crate::journal::{self, Entry};
use crate::parallel::{key_order, parallel_tickets, spec_time, stops_dir, write_stop_kind};
use crate::records::spec_hash8;
use crate::state::State;
use crate::tickets::{Status, Ticket, branch_ticket};
use crate::triage::state_of;
use serde_json::{Map, Value};
use std::path::Path;

/// A later ticket and one earlier parallel ticket, with the branch of its worktree and the
/// dependency round of the pair.
#[derive(Debug)]
pub struct Pair {
    pub earlier: Ticket,
    pub branch: String,
    /// `<EARLIER>:<first 8 hex of the earlier spec hash>:<first 8 hex of the later spec hash>`.
    pub round: String,
}

fn state(ticket: &Ticket) -> Option<&State> {
    match &ticket.status {
        Status::Open(s) | Status::Closed(s) => Some(s),
        _ => None,
    }
}

/// The earlier parallel tickets of `own`, each with its current round: the parallel tickets whose
/// spec time ranks before that of `own` (the order of AC-15). A ticket without a spec time and
/// hash takes no part.
pub fn earlier_pairs(root: &Path, own: &Ticket) -> Vec<Pair> {
    let Some(own_state) = state(own) else {
        return Vec::new();
    };
    let (Some(own_at), Some(own_hash)) = (spec_time(own_state), spec_hash8(own_state)) else {
        return Vec::new();
    };
    let rank = |at: &str, key: &str| (crate::time::parse_utc(at), key_order(key));
    let mut pairs = Vec::new();
    for (other, branch) in parallel_tickets(root, own) {
        let Some(other_state) = state(&other) else {
            continue;
        };
        let (Some(other_at), Some(other_hash)) = (spec_time(other_state), spec_hash8(other_state))
        else {
            continue;
        };
        if rank(other_at, &other.key) < rank(own_at, &own.key) {
            let round = format!("{}:{other_hash}:{own_hash}", other.key);
            pairs.push(Pair {
                earlier: other,
                branch,
                round,
            });
        }
    }
    pairs
}

/// The readable records of the `findings.jsonl` of `ticket`; empty when it cannot be read.
fn records_of(ticket: &Ticket) -> Vec<Map<String, Value>> {
    journal::read(&ticket.dir.join("findings.jsonl"))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, entry)| match entry {
            Entry::V1(map) | Entry::Legacy(map) => Some(map),
            Entry::Problem(_) => None,
        })
        .collect()
}

fn text<'a>(m: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    m.get(name).and_then(Value::as_str)
}

/// Whether the records hold a record of stage `dependency` with `round`.
fn round_done(records: &[Map<String, Value>], round: &str) -> bool {
    records
        .iter()
        .any(|m| text(m, "stage") == Some("dependency") && text(m, "round") == Some(round))
}

/// A dependency round of an open ticket that has no record yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub later: String,
    pub earlier: String,
    pub round: String,
}

impl Pending {
    /// `dependency review of <LATER> against <EARLIER> is not done`.
    pub fn line(&self) -> String {
        format!(
            "dependency review of {} against {} is not done",
            self.later, self.earlier
        )
    }
}

/// The current dependency rounds of the open ticket `own` that have no record of stage `dependency`
/// (AC-20).
pub fn pending_for(root: &Path, own: &Ticket) -> Vec<Pending> {
    if !matches!(own.status, Status::Open(_)) {
        return Vec::new();
    }
    let pairs = earlier_pairs(root, own);
    if pairs.is_empty() {
        return Vec::new();
    }
    let records = records_of(own);
    pairs
        .into_iter()
        .filter(|p| !round_done(&records, &p.round))
        .map(|p| Pending {
            later: own.key.clone(),
            earlier: p.earlier.key.clone(),
            round: p.round,
        })
        .collect()
}

/// The open dependency rounds of the branch ticket at `root`.
pub fn pending(root: &Path) -> Vec<Pending> {
    match branch_ticket(root) {
        Some(own) => pending_for(root, &own),
        None => Vec::new(),
    }
}

/// Writes or removes the dependency stop of each done round of `own` (AC-22): the stop of a pair
/// holds the HIGH or MEDIUM records of its round whose verdict is not `false`; its kind is that of
/// their `waits`, `merge` when both kinds are present. With no such record the stop is removed. A
/// round with no record is left as it is.
pub fn sync_stops(root: &Path, own: &Ticket) -> Result<(), String> {
    let Some(stops) = stops_dir(root) else {
        return Ok(());
    };
    let records = records_of(own);
    let mut errors = Vec::new();
    for pair in earlier_pairs(root, own) {
        if !round_done(&records, &pair.round) {
            continue;
        }
        let path = stops
            .join(&own.key)
            .join(format!("{}-dependency.json", pair.earlier.key));
        let (mut spec, mut merge) = (false, false);
        let mut seen: Vec<&str> = Vec::new();
        for m in records.iter().filter(|m| !m.contains_key("kind")) {
            let Some(id) = text(m, "id") else { continue };
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            let current = state_of(&records, id);
            let Some(finding) = &current.finding else {
                continue;
            };
            if text(finding, "stage") != Some("dependency")
                || text(finding, "round") != Some(pair.round.as_str())
                || current.verdict.as_deref() == Some("false")
                || !matches!(current.severity.as_deref(), Some("HIGH" | "MEDIUM"))
            {
                continue;
            }
            match text(finding, "waits") {
                Some("spec") => spec = true,
                Some("merge") => merge = true,
                _ => merge = true,
            }
        }
        let kind = if merge {
            Some("merge")
        } else if spec {
            Some("spec")
        } else {
            None
        };
        let done = match kind {
            Some(kind) => write_stop_kind(&path, kind, &[]),
            None => match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            },
        };
        if let Err(e) = done {
            errors.push(format!("{}: {e}", path.display()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// The named files of a Plan: the words in backticks that hold a `/` or a file extension, once
/// each, in Plan order. A word with a space, a `:`, a leading `/`, a `..` part or a final `/` is no
/// file; an extension needs a letter.
pub fn named_files(plan: &str) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    for (i, span) in plan.split('`').enumerate() {
        if i % 2 == 0 || span.is_empty() {
            continue;
        }
        let plain = !span.contains(char::is_whitespace)
            && !span.contains(':')
            && !span.starts_with('/')
            && !span.ends_with('/')
            && !span.split('/').any(|part| part == ".." || part.is_empty());
        let extension = span
            .rsplit('/')
            .next()
            .and_then(|name| name.rsplit_once('.'))
            .is_some_and(|(stem, ext)| {
                !stem.is_empty() && ext.chars().any(|c| c.is_ascii_alphabetic())
                    && ext.chars().all(|c| c.is_ascii_alphanumeric())
            });
        if plain && (span.contains('/') || extension) && !files.iter().any(|f| f == span) {
            files.push(span.to_string());
        }
    }
    files
}
