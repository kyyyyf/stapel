//! Worktrees and branches of the repository, for parallel tickets (STP-27).
//!
//! Every helper answers with an empty list when git is missing or fails, so a repository without
//! worktrees behaves as before.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A listed worktree: its folder and whether git marks it prunable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub prunable: bool,
}

/// The worktrees of the repository at `root`, the main one included.
pub fn worktrees(root: &Path) -> Vec<Worktree> {
    let text = git(root, &["worktree", "list", "--porcelain"]).unwrap_or_default();
    let mut list: Vec<Worktree> = Vec::new();
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            list.push(Worktree {
                path: PathBuf::from(path),
                prunable: false,
            });
        } else if line == "prunable" || line.starts_with("prunable ") {
            if let Some(last) = list.last_mut() {
                last.prunable = true;
            }
        }
    }
    list
}

/// A local or remote-tracking branch: the full ref and the branch name without the remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub full_ref: String,
    pub name: String,
}

/// Every local and remote-tracking branch.
pub fn branches(root: &Path) -> Vec<Branch> {
    let text = git(
        root,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
        ],
    )
    .unwrap_or_default();
    text.lines()
        .filter_map(|full| {
            let name = if let Some(n) = full.strip_prefix("refs/heads/") {
                n
            } else {
                full.strip_prefix("refs/remotes/")?.split_once('/')?.1
            };
            Some(Branch {
                full_ref: full.to_string(),
                name: name.to_string(),
            })
        })
        .collect()
}

/// The folder names under `.stapel/tickets/` in the tree of `full_ref`.
pub fn branch_ticket_names(root: &Path, full_ref: &str) -> Vec<String> {
    git(
        root,
        &["ls-tree", "--name-only", &format!("{full_ref}:.stapel/tickets")],
    )
    .unwrap_or_default()
    .lines()
    .map(String::from)
    .collect()
}
