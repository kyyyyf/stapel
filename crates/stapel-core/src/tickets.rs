//! Ticket folders under `.stapel/tickets/` and their keys (STP-2 AC-1, AC-4).

use crate::config::Config;
use std::path::{Path, PathBuf};

pub fn tickets_dir(root: &Path) -> PathBuf {
    root.join(".stapel/tickets")
}

/// The number in a folder name that matches `tickets.key`, compared without case; leading zeros
/// are ignored.
pub fn key_number(config: &Config, name: &str) -> Option<u64> {
    let (prefix, suffix) = config.tickets.key.split_once("{n}")?;
    let lower = name.to_lowercase();
    let rest = lower.strip_prefix(&prefix.to_lowercase())?;
    let digits = rest.strip_suffix(&suffix.to_lowercase())?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn largest_in_folder(config: &Config, dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| key_number(config, &e.file_name().to_string_lossy()))
        .max()
        .unwrap_or(0)
}

/// The key of the next ticket: one more than the largest number among the ticket folders here, in
/// each worktree that is not prunable, in each local and remote-tracking branch, and in the names
/// of those branches.
pub fn next_key(config: &Config, root: &Path) -> String {
    let mut largest = largest_in_folder(config, &tickets_dir(root));
    for wt in crate::parallel::worktrees(root) {
        if !wt.prunable {
            largest = largest.max(largest_in_folder(config, &tickets_dir(&wt.path)));
        }
    }
    for branch in crate::parallel::branches(root) {
        let names = crate::parallel::branch_ticket_names(root, &branch.full_ref);
        largest = names
            .iter()
            .chain(std::iter::once(&branch.name))
            .filter_map(|n| key_number(config, n))
            .fold(largest, u64::max);
    }
    config
        .tickets
        .key
        .replace("{n}", &(largest + 1).to_string())
}

use crate::state::{Loaded, State, load};

/// What a ticket folder's `state.json` says about the ticket.
#[derive(Debug)]
pub enum Status {
    Open(State),
    Closed(State),
    /// STP-1 format, without `schema_version`.
    Legacy(serde_json::Value),
    Unreadable(String),
}

#[derive(Debug)]
pub struct Ticket {
    /// The folder name, which is the key.
    pub key: String,
    pub dir: PathBuf,
    pub status: Status,
}

/// Every ticket folder, sorted by name.
pub fn list(root: &Path) -> Vec<Ticket> {
    let mut tickets: Vec<Ticket> = std::fs::read_dir(tickets_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| {
            let dir = e.path();
            let status = match load(&dir.join("state.json")) {
                Loaded::State(s) if s.closed.is_some() => Status::Closed(s),
                Loaded::State(s) => Status::Open(s),
                Loaded::Legacy(v) => Status::Legacy(v),
                Loaded::Unreadable(reason) => Status::Unreadable(reason),
            };
            Ticket {
                key: e.file_name().to_string_lossy().into_owned(),
                dir,
                status,
            }
        })
        .collect();
    tickets.sort_by(|a, b| a.key.cmp(&b.key));
    tickets
}

/// The ticket of the current checkout, open or closed, whose key the current branch names
/// (without case); none on a base branch, a detached HEAD or an unborn HEAD (STP-27 AC-6).
pub fn branch_ticket(root: &Path) -> Option<Ticket> {
    let branch = crate::parallel::current_branch(root)?;
    list(root)
        .into_iter()
        .find(|t| t.key.eq_ignore_ascii_case(&branch))
}

/// Refuses `key` when the checkout has a branch ticket and `key` names another ticket (STP-27 AC-7).
pub fn match_branch(root: &Path, key: Option<&str>) -> Result<(), String> {
    let (Some(key), Some(ticket)) = (key, branch_ticket(root)) else {
        return Ok(());
    };
    if ticket.key.eq_ignore_ascii_case(key) {
        return Ok(());
    }
    let branch = crate::parallel::current_branch(root).unwrap_or_default();
    let key = list(root)
        .into_iter()
        .find(|t| t.key.eq_ignore_ascii_case(key))
        .map_or_else(|| key.to_string(), |t| t.key);
    Err(format!(
        "{key} is not the ticket of branch {branch}; run it in .claude/worktrees/{key}"
    ))
}

/// The ticket a command acts on: the given key (without case), the branch ticket, or the only open
/// ticket.
pub fn resolve(root: &Path, key: Option<&str>) -> Result<Ticket, String> {
    if key.is_none() {
        if let Some(ticket) = branch_ticket(root) {
            return Ok(ticket);
        }
    }
    let tickets = list(root);
    match key {
        Some(key) => {
            let mut matching: Vec<Ticket> = tickets
                .into_iter()
                .filter(|t| t.key.eq_ignore_ascii_case(key))
                .collect();
            match matching.len() {
                0 => Err(format!("no ticket {key} in .stapel/tickets")),
                1 => Ok(matching.remove(0)),
                _ => Err(format!(
                    "{key} matches several folders that differ only in case: {}",
                    matching
                        .iter()
                        .map(|t| t.key.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
        None => {
            let mut open: Vec<Ticket> = tickets
                .into_iter()
                .filter(|t| matches!(t.status, Status::Open(_)))
                .collect();
            match open.len() {
                1 => Ok(open.remove(0)),
                0 => Err("no open ticket; name one, e.g. stapel status <KEY>".into()),
                _ => Err(format!(
                    "several open tickets, name one: {}",
                    open.iter()
                        .map(|t| t.key.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
    }
}
