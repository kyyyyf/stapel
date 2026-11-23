//! `stapel inbox` (STP-40): writes `.stapel/inbox.md`, the decisions that wait for the person.

use crate::fail::Fail;
use crate::repo;
use stapel_core::inbox::{Entry, Kind, owed_findings, render, waiting_sections};
use stapel_core::journal::{self, Entry as Line};
use stapel_core::ticket::{parse, read};
use stapel_core::tickets::{Status, Ticket, branch_ticket, key_number};
use std::path::{Path, PathBuf};

/// The owed decisions of the open ticket of the checkout at `path`; one unreadable entry when its
/// files, its `stapel.toml` or its `stapel next` cannot be read.
fn entries_of(path: &Path, worktree: &str, ticket: &Ticket) -> Vec<Entry> {
    let (key, title) = match &ticket.status {
        Status::Open(state) => (ticket.key.clone(), state.title.clone()),
        _ => (ticket.key.clone(), String::new()),
    };
    let unreadable = |title: String, reason: String| {
        vec![Entry {
            key: key.clone(),
            title,
            worktree: worktree.to_string(),
            kind: Kind::Unreadable { reason },
        }]
    };
    let state = match &ticket.status {
        Status::Open(state) => state,
        Status::Unreadable(reason) => return unreadable(title, reason.clone()),
        _ => return unreadable(title, "state.json is not in the current format".to_string()),
    };
    match read_entries(path, worktree, ticket, state) {
        Ok(entries) => entries,
        Err(reason) => unreadable(title, reason),
    }
}

fn read_entries(
    path: &Path,
    worktree: &str,
    ticket: &Ticket,
    state: &stapel_core::state::State,
) -> Result<Vec<Entry>, String> {
    let config = repo::load(path)?;
    let lines = journal::read(&ticket.dir.join("findings.jsonl"))?;
    let text = read(&ticket.dir.join("ticket.md"))?;
    let mut records = Vec::new();
    for (n, e) in lines {
        match e {
            Line::V1(m) | Line::Legacy(m) => records.push(m),
            Line::Problem(reason) => {
                return Err(format!("findings.jsonl line {n}: {reason}"));
            }
        }
    }
    let entry = |kind| Entry {
        key: ticket.key.clone(),
        title: state.title.clone(),
        worktree: worktree.to_string(),
        kind,
    };
    let mut entries: Vec<Entry> = owed_findings(&records)
        .into_iter()
        .map(|(id, line)| entry(Kind::Finding { id, line }))
        .collect();
    let stages: Vec<String> = records
        .iter()
        .filter_map(|m| m.get("stage")?.as_str().map(String::from))
        .collect();
    let waiting = waiting_sections(&config, &parse(&text), state, &stages);
    if !waiting.is_empty() {
        entries.push(entry(Kind::Confirmation { waiting }));
    }
    Ok(entries)
}

pub fn run() -> Result<(), Fail> {
    let (root, config) = repo::open().map_err(Fail::failed)?;
    let mut paths: Vec<PathBuf> = vec![root.clone()];
    for wt in stapel_core::parallel::worktrees(&root) {
        if !wt.prunable && wt.path.is_dir() {
            paths.push(wt.path);
        }
    }
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut found: Vec<((Option<u64>, String), Vec<Entry>)> = Vec::new();
    for path in paths {
        let real = path.canonicalize().unwrap_or_else(|_| path.clone());
        if seen.contains(&real) {
            continue;
        }
        seen.push(real.clone());
        let Some(ticket) = branch_ticket(&path) else {
            continue;
        };
        if matches!(ticket.status, Status::Closed(_)) {
            continue;
        }
        let entries = entries_of(&path, &real.display().to_string(), &ticket);
        let order = (key_number(&config, &ticket.key), ticket.key.to_lowercase());
        found.push((order, entries));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    let entries: Vec<Entry> = found.into_iter().flat_map(|(_, e)| e).collect();
    let path = root.join(".stapel/inbox.md");
    std::fs::write(&path, render(&entries))
        .map_err(|e| Fail::failed(format!("{}: {e}", path.display())))?;
    match entries.len() {
        0 => say!("inbox: nothing waits"),
        1 => say!("inbox: 1 decision waits in .stapel/inbox.md"),
        n => say!("inbox: {n} decisions wait in .stapel/inbox.md"),
    }
    Ok(())
}
