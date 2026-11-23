//! `stapel inbox` (STP-40): writes `.stapel/inbox.md`, the decisions that wait for the person.

use crate::fail::Fail;
use crate::repo;
use stapel_core::inbox::{Entry, Kind, owed_findings, render};
use stapel_core::journal::{self, Entry as Line};
use stapel_core::tickets::{Status, branch_ticket};

pub fn run() -> Result<(), Fail> {
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    let mut entries: Vec<Entry> = Vec::new();
    if let Some(ticket) = branch_ticket(&root)
        && let Status::Open(state) = &ticket.status
    {
        let lines = journal::read(&ticket.dir.join("findings.jsonl")).map_err(Fail::failed)?;
        let records: Vec<_> = lines
            .into_iter()
            .filter_map(|(_, e)| match e {
                Line::V1(m) | Line::Legacy(m) => Some(m),
                Line::Problem(_) => None,
            })
            .collect();
        let worktree = root.canonicalize().unwrap_or_else(|_| root.clone());
        for (id, line) in owed_findings(&records) {
            entries.push(Entry {
                key: ticket.key.clone(),
                title: state.title.clone(),
                worktree: worktree.display().to_string(),
                kind: Kind::Finding { id, line },
            });
        }
    }
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
