//! Whether a ticket's newest check is current (STP-4 AC-6): the `check:` line of `status`, the
//! close gate and the close dialog.

use crate::config::Config;
use crate::journal::{Entry, read};
use crate::steps::git;
use crate::worktree::{dirty_paths, is_machine_file};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckLine {
    NotConfigured,
    None,
    Pass(String),
    Fail(String),
    Stale(String),
}

impl std::fmt::Display for CheckLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckLine::NotConfigured => write!(f, "check: not configured"),
            CheckLine::None => write!(f, "check: none"),
            CheckLine::Pass(sha) => write!(f, "check: pass at {sha}"),
            CheckLine::Fail(sha) => write!(f, "check: fail at {sha}"),
            CheckLine::Stale(reason) => write!(f, "check: stale ({reason})"),
        }
    }
}

fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// The newest well-formed check record of a ticket, judged against the working tree.
pub fn current(root: &Path, config: &Config, ticket_dir: &Path) -> CheckLine {
    if config.check.is_none() {
        return CheckLine::NotConfigured;
    }
    let last = read(&ticket_dir.join("runs.jsonl"))
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find_map(|(_, e)| match e {
            Entry::V1(m) if m.get("kind").and_then(|k| k.as_str()) == Some("check") => {
                let head = m.get("head")?.as_str()?.to_string();
                let result = m.get("result")?.as_str()?.to_string();
                Some((head, result))
            }
            _ => None,
        });
    let Some((head, result)) = last else {
        return CheckLine::None;
    };
    let known = git(root, &["cat-file", "-e", &format!("{head}^{{commit}}")]).is_ok();
    if !known {
        return CheckLine::Stale(format!("unknown commit {}", short(&head)));
    }
    let mut changed: Vec<String> = git(root, &["diff", "--name-only", "-z", &head])
        .map(|raw| {
            raw.split(|b| *b == 0)
                .filter(|p| !p.is_empty())
                .map(|p| String::from_utf8_lossy(p).into_owned())
                .collect()
        })
        .unwrap_or_default();
    changed.extend(dirty_paths(root).unwrap_or_default());
    changed.retain(|p| !is_machine_file(p));
    changed.sort();
    changed.dedup();
    if !changed.is_empty() {
        let first: Vec<&str> = changed.iter().take(5).map(String::as_str).collect();
        return CheckLine::Stale(format!("changed: {}", first.join(", ")));
    }
    if result == "pass" {
        CheckLine::Pass(short(&head))
    } else {
        CheckLine::Fail(short(&head))
    }
}
