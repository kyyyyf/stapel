//! Worktrees and branches of the repository, for parallel tickets (STP-27).
//!
//! Every helper answers with an empty list when git is missing or fails, so a repository without
//! worktrees behaves as before.

use crate::state::State;
use crate::tickets::{Status, Ticket, branch_ticket};
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The branch checked out at `root`; none on a detached HEAD or without git. An unborn HEAD names
/// its branch.
pub fn current_branch(root: &Path) -> Option<String> {
    let name = git(root, &["symbolic-ref", "-q", "--short", "HEAD"])?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
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

/// The `at` of the first confirmation of the section `spec`.
pub fn spec_time(state: &State) -> Option<&str> {
    state
        .confirmations
        .iter()
        .find(|c| c.section == "spec")
        .map(|c| c.at.as_str())
}

fn state_of(ticket: &Ticket) -> Option<&State> {
    match &ticket.status {
        Status::Open(s) | Status::Closed(s) => Some(s),
        _ => None,
    }
}

/// Whether the base branch's `state.json` of `key` holds `closed`.
fn closed_on_base(root: &Path, base: &str, key: &str) -> bool {
    let spec = format!("refs/heads/{base}:.stapel/tickets/{key}/state.json");
    git(root, &["show", &spec])
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .is_some_and(|v| v.get("closed").is_some_and(|c| !c.is_null()))
}

/// The parallel tickets of `own`, each with the branch of its worktree: the branch tickets of the
/// other listed worktrees that are not prunable and not closed on the base branch.
pub fn parallel_tickets(root: &Path, own: &Ticket) -> Vec<(Ticket, String)> {
    let base = state_of(own).and_then(|s| s.base_branch.clone());
    let here = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut found = Vec::new();
    for wt in worktrees(root) {
        if wt.prunable {
            continue;
        }
        if wt.path.canonicalize().unwrap_or_else(|_| wt.path.clone()) == here {
            continue;
        }
        let (Some(branch), Some(ticket)) = (current_branch(&wt.path), branch_ticket(&wt.path))
        else {
            continue;
        };
        if ticket.key.eq_ignore_ascii_case(&own.key) {
            continue;
        }
        if let Some(base) = &base {
            if closed_on_base(root, base, &ticket.key) {
                continue;
            }
        }
        found.push((ticket, branch));
    }
    found
}

/// What one trial merge of a pair found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trial {
    /// A conflict: the stop of `later` is written.
    Conflict {
        later: String,
        earlier: String,
        files: Vec<String>,
    },
    /// A clean merge: the conflict stop of the pair is removed.
    Clean,
    /// Git did not answer with 0 or 1; no stop changed.
    NotRun(String),
}

/// The `.../stapel/stops` folder of the repository at `root`.
fn stops_dir(root: &Path) -> Option<PathBuf> {
    let common = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    Some(PathBuf::from(common.trim()).join("stapel/stops"))
}

fn key_order(key: &str) -> (u64, String) {
    let n = key
        .rsplit('-')
        .next()
        .and_then(|d| d.parse::<u64>().ok())
        .unwrap_or(0);
    (n, key.to_lowercase())
}

fn write_stop(path: &Path, files: &[String]) -> std::io::Result<()> {
    let dir = path.parent().expect("stop folder");
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().expect("stop name").to_string_lossy();
    let tmp = dir.join(format!("{name}.{}.tmp", std::process::id()));
    let body = serde_json::json!({
        "kind": "merge",
        "files": files,
        "at": crate::time::now_rfc3339(),
    });
    std::fs::write(&tmp, body.to_string())?;
    std::fs::rename(&tmp, path)
}

/// The trial merge of the branch ticket of `root` with each parallel ticket (STP-27 AC-15, AC-16).
/// Both tickets need a spec time; the one with the later spec time (the higher key on a tie) is
/// the later. A conflict writes the stop `<LATER>/<EARLIER>-conflict.json`, a clean merge removes
/// it, and any other answer of git changes no stop.
pub fn trial_merges(root: &Path) -> Vec<Trial> {
    let mut results = Vec::new();
    let (Some(own), Some(own_branch)) = (branch_ticket(root), current_branch(root)) else {
        return results;
    };
    let Some(own_spec) = state_of(&own).and_then(spec_time).map(String::from) else {
        return results;
    };
    let Some(stops) = stops_dir(root) else {
        return results;
    };
    for (other, other_branch) in parallel_tickets(root, &own) {
        let Some(other_spec) = state_of(&other).and_then(spec_time).map(String::from) else {
            continue;
        };
        let rank = |spec: &str, key: &str| (crate::time::parse_utc(spec), key_order(key));
        let own_is_later = rank(&own_spec, &own.key) > rank(&other_spec, &other.key);
        let (later, earlier) = if own_is_later {
            (own.key.clone(), other.key.clone())
        } else {
            (other.key.clone(), own.key.clone())
        };
        let out = Command::new("git")
            .args(["merge-tree", "--write-tree", "--name-only", "--no-messages"])
            .arg(format!("refs/heads/{own_branch}"))
            .arg(format!("refs/heads/{other_branch}"))
            .current_dir(root)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output();
        let out = match out {
            Ok(out) => out,
            Err(e) => {
                results.push(Trial::NotRun(format!("git: {e}")));
                continue;
            }
        };
        let stop = stops.join(&later).join(format!("{earlier}-conflict.json"));
        match out.status.code() {
            Some(0) => {
                let _ = std::fs::remove_file(&stop);
                results.push(Trial::Clean);
            }
            Some(1) => {
                let text = String::from_utf8_lossy(&out.stdout).into_owned();
                let mut files: Vec<String> = Vec::new();
                for line in text.lines().skip(1).take_while(|l| !l.is_empty()) {
                    if !files.iter().any(|f| f == line) {
                        files.push(line.to_string());
                    }
                }
                match write_stop(&stop, &files) {
                    Ok(()) => results.push(Trial::Conflict {
                        later,
                        earlier,
                        files,
                    }),
                    Err(e) => results.push(Trial::NotRun(format!("{}: {e}", stop.display()))),
                }
            }
            code => {
                let first = String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .next()
                    .map(String::from)
                    .filter(|l| !l.is_empty())
                    .unwrap_or_else(|| match code {
                        Some(c) => format!("git exited with {c}"),
                        None => "git was stopped by a signal".to_string(),
                    });
                results.push(Trial::NotRun(first));
            }
        }
    }
    results
}
