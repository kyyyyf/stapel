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
        &["ls-tree", "-d", "--name-only", &format!("{full_ref}:.stapel/tickets")],
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

/// Whether the `state.json` of `key` at `rev` holds `closed`.
fn closed_at(root: &Path, rev: &str, key: &str) -> bool {
    let spec = format!("{rev}:.stapel/tickets/{key}/state.json");
    git(root, &["show", &spec])
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .is_some_and(|v| v.get("closed").is_some_and(|c| !c.is_null()))
}

/// Whether the base branch's `state.json` of `key` holds `closed`.
fn closed_on_base(root: &Path, base: &str, key: &str) -> bool {
    closed_at(root, &format!("refs/heads/{base}"), key)
}

/// The base branch of the branch ticket at `root` when that branch holds a close that `HEAD` lacks
/// (STP-27 AC-18); other commits of the base branch ask for nothing.
pub fn merge_main_needed(root: &Path) -> Option<String> {
    let own = branch_ticket(root)?;
    let base = state_of(&own)?.base_branch.clone()?;
    let names = branch_ticket_names(root, &format!("refs/heads/{base}"));
    names
        .iter()
        .any(|key| {
            !key.eq_ignore_ascii_case(&own.key)
                && closed_on_base(root, &base, key)
                && !closed_at(root, "HEAD", key)
        })
        .then_some(base)
}

/// The latest commit of `HEAD` that merged the base branch of the branch ticket and brought the
/// close of another ticket (STP-27 AC-19).
pub fn merge_commit(root: &Path) -> Option<String> {
    let own = branch_ticket(root)?;
    let base = state_of(&own)?.base_branch.clone()?;
    let base_ref = format!("refs/heads/{base}");
    let merges = git(root, &["rev-list", "--merges", "HEAD"])?;
    for sha in merges.lines() {
        let line = git(root, &["rev-list", "--parents", "-n", "1", sha])?;
        let parents: Vec<&str> = line.split_whitespace().skip(1).collect();
        let [first, second, ..] = parents[..] else {
            continue;
        };
        if git(root, &["merge-base", "--is-ancestor", second, &base_ref]).is_none() {
            continue;
        }
        let brought = branch_ticket_names(root, sha).iter().any(|key| {
            !key.eq_ignore_ascii_case(&own.key)
                && closed_at(root, sha, key)
                && !closed_at(root, first, key)
        });
        if brought {
            return Some(sha.to_string());
        }
    }
    None
}

/// The merge round of a merge commit: `merge:` and the first 8 hex digits.
pub fn merge_round(sha: &str) -> String {
    format!("merge:{}", sha.chars().take(8).collect::<String>())
}

/// The merge round of `HEAD` when the code review of it has no finding record yet (STP-27 AC-19).
pub fn merge_review_due(root: &Path, ticket_dir: &Path) -> Option<String> {
    let round = merge_round(&merge_commit(root)?);
    let entries = crate::journal::read(&ticket_dir.join("findings.jsonl")).unwrap_or_default();
    let done = entries.into_iter().any(|(_, entry)| match entry {
        crate::journal::Entry::V1(m) | crate::journal::Entry::Legacy(m) => {
            !m.contains_key("kind")
                && m.get("stage").and_then(|s| s.as_str()) == Some("code-review")
                && m.get("round").and_then(|r| r.as_str()) == Some(round.as_str())
        }
        crate::journal::Entry::Problem(_) => false,
    });
    (!done).then_some(round)
}

/// A stop that blocks the branch ticket: it waits for `earlier`, for `reason`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wait {
    pub later: String,
    pub earlier: String,
    pub reason: String,
}

impl Wait {
    /// `<LATER> waits for <EARLIER> (<reason>)`.
    pub fn line(&self) -> String {
        format!("{} waits for {} ({})", self.later, self.earlier, self.reason)
    }
}

/// The largest stop file that is read; a bigger one is an unreadable stop.
const STOP_LIMIT: u64 = 4096;

enum StopKind {
    Merge,
    Spec,
}

/// What a stop file says: its kind and the reason text; an unreadable file is a `merge` stop.
fn read_stop(path: &Path, reason: &str) -> (StopKind, String) {
    let unreadable = || (StopKind::Merge, "unreadable stop file".to_string());
    let small = std::fs::metadata(path).is_ok_and(|m| m.len() <= STOP_LIMIT);
    let Some(v) = small
        .then(|| std::fs::read_to_string(path).ok())
        .flatten()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        return unreadable();
    };
    match v.get("kind").and_then(|k| k.as_str()) {
        Some("spec") => (StopKind::Spec, reason.to_string()),
        Some("merge") => {
            let files: Vec<&str> = v
                .get("files")
                .and_then(|f| f.as_array())
                .map(|a| a.iter().filter_map(|f| f.as_str()).collect())
                .unwrap_or_default();
            if reason == "conflict" && !files.is_empty() {
                (StopKind::Merge, format!("conflict in {}", files.join(", ")))
            } else {
                (StopKind::Merge, reason.to_string())
            }
        }
        _ => unreadable(),
    }
}

/// Whether the `spec` confirmation of the parallel ticket `earlier` is fresh. A ticket that no
/// worktree holds counts as stale.
fn earlier_spec_fresh(root: &Path, own: &Ticket, earlier: &str) -> bool {
    let Some(config) = std::fs::read_to_string(root.join(".stapel/stapel.toml"))
        .ok()
        .and_then(|t| crate::config::Config::parse(&t).ok())
    else {
        return false;
    };
    let Some((ticket, _)) = parallel_tickets(root, own)
        .into_iter()
        .find(|(t, _)| t.key.eq_ignore_ascii_case(earlier))
    else {
        return false;
    };
    let Some(state) = state_of(&ticket) else {
        return false;
    };
    let Some(text) = crate::stage::read_bounded(&ticket.dir.join("ticket.md")) else {
        return false;
    };
    crate::stage::is_fresh(&config, &crate::ticket::parse(&text), state, "spec")
}

/// The stop files of `key` in `stops`, sorted; a missing folder holds none, an unreadable one is an
/// error.
fn stop_files(stops: &Path, key: &str) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(stops.join(key)) {
        Ok(entries) => entries
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|e| e.path())
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    files.sort();
    Ok(files)
}

/// Removes the stops of the branch ticket at `root` whose earlier ticket is closed in `HEAD`
/// (STP-27 AC-18). Only `next` and the post-tool-use hook call it; the guard only reads.
pub fn prune_stops(root: &Path) {
    let (Some(own), Some(stops)) = (branch_ticket(root), stops_dir(root)) else {
        return;
    };
    for path in stop_files(&stops, &own.key).unwrap_or_default() {
        let earlier = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".json"))
            .and_then(|n| n.rsplit_once('-'))
            .map(|(e, _)| e);
        if let Some(earlier) = earlier
            && !earlier.is_empty()
            && closed_at(root, "HEAD", earlier)
        {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// The stops that block the branch ticket at `root` (STP-27 AC-17); it only reads. A stop whose
/// earlier ticket is closed in `HEAD` or on the base branch does not block. A `merge` stop blocks
/// otherwise; a `spec` stop blocks while the earlier spec is stale. A ticket whose `state.json` or
/// stops folder cannot be read waits, so the guard denies code.
pub fn waits(root: &Path) -> Vec<Wait> {
    let mut found = Vec::new();
    let Some(own) = branch_ticket(root) else {
        return found;
    };
    let unreadable = |why: &str| {
        vec![Wait {
            later: own.key.clone(),
            earlier: "unknown".to_string(),
            reason: why.to_string(),
        }]
    };
    if matches!(own.status, Status::Unreadable(_)) {
        return unreadable("its state.json cannot be read");
    }
    let Some(stops) = stops_dir(root) else {
        return unreadable("the stops folder is not known");
    };
    let base = state_of(&own).and_then(|s| s.base_branch.clone());
    let Ok(files) = stop_files(&stops, &own.key) else {
        return unreadable("the stops folder cannot be read");
    };
    for path in files {
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".json"))
        else {
            continue;
        };
        let Some((earlier, reason)) = name.rsplit_once('-') else {
            continue;
        };
        if earlier.is_empty() || reason.is_empty() {
            continue;
        }
        if closed_at(root, "HEAD", earlier) {
            continue;
        }
        if base
            .as_deref()
            .is_some_and(|b| closed_on_base(root, b, earlier))
        {
            continue;
        }
        let (kind, reason) = read_stop(&path, reason);
        let reason = match kind {
            StopKind::Merge => reason,
            StopKind::Spec if earlier_spec_fresh(root, &own, earlier) => continue,
            StopKind::Spec => format!("spec of {earlier} is stale"),
        };
        found.push(Wait {
            later: own.key.clone(),
            earlier: earlier.to_string(),
            reason,
        });
    }
    found
}

/// What a closed branch ticket still needs (STP-27 AC-23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchClose {
    /// The close is written and `HEAD` does not hold it.
    Commit,
    /// `HEAD` holds the close and the base branch does not: merge the branch in the main checkout.
    Merge { main: PathBuf, not_ready: Option<String> },
}

/// The step of a closed branch ticket `own` at `root`; none when it has no base branch or the base
/// branch holds the close.
pub fn branch_close(root: &Path, own: &Ticket) -> Option<BranchClose> {
    let base = state_of(own)?.base_branch.clone()?;
    if !closed_at(root, "HEAD", &own.key) {
        return Some(BranchClose::Commit);
    }
    if closed_on_base(root, &base, &own.key) {
        return None;
    }
    let main = worktrees(root).into_iter().next()?.path;
    let mut reasons = Vec::new();
    match current_branch(&main) {
        Some(b) if b == base => {}
        Some(b) => reasons.push(format!("it is on {b}, not on {base}")),
        None => reasons.push(format!("it is not on {base}")),
    }
    match crate::worktree::dirty_paths(&main) {
        Ok(dirty) if dirty.is_empty() => {}
        Ok(dirty) => reasons.push(format!("it has changes in {}", dirty.join(", "))),
        Err(e) => reasons.push(format!("its state is unknown: {e}")),
    }
    let not_ready = (!reasons.is_empty()).then(|| reasons.join("; "));
    Some(BranchClose::Merge { main, not_ready })
}

/// The notices `can go on: <LATER> (<EARLIER> is merged into <branch>)` for a checkout without a
/// branch ticket: a listed worktree's ticket has a stop whose earlier ticket is closed in `HEAD`
/// (STP-27 AC-24).
pub fn go_on_notices(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if branch_ticket(root).is_some() {
        return out;
    }
    let (Some(branch), Some(stops)) = (current_branch(root), stops_dir(root)) else {
        return out;
    };
    let here = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    for wt in worktrees(root) {
        if wt.prunable || wt.path.canonicalize().unwrap_or_else(|_| wt.path.clone()) == here {
            continue;
        }
        let Some(later) = branch_ticket(&wt.path) else {
            continue;
        };
        let mut files: Vec<PathBuf> = std::fs::read_dir(stops.join(&later.key))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        files.sort();
        let mut seen: Vec<String> = Vec::new();
        for path in files {
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".json"))
            else {
                continue;
            };
            let Some((earlier, _)) = name.rsplit_once('-') else {
                continue;
            };
            if earlier.is_empty()
                || seen.iter().any(|s| s == earlier)
                || !closed_at(root, "HEAD", earlier)
            {
                continue;
            }
            seen.push(earlier.to_string());
            out.push(format!(
                "can go on: {} ({earlier} is merged into {branch})",
                later.key
            ));
        }
    }
    out
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
    /// A conflict whose stop file could not be written: `<path>: <error>`.
    StopNotWritten(String),
}

/// Whether a rebase, a merge or a cherry-pick is in progress in the checkout at `root`.
pub fn operation_in_progress(root: &Path) -> bool {
    ["rebase-merge", "rebase-apply", "MERGE_HEAD", "CHERRY_PICK_HEAD"]
        .iter()
        .any(|name| {
            git(root, &["rev-parse", "--path-format=absolute", "--git-path", name])
                .is_some_and(|p| Path::new(p.trim()).exists())
        })
}

/// The `.../stapel/stops` folder of the repository at `root`.
pub(crate) fn stops_dir(root: &Path) -> Option<PathBuf> {
    let common = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    Some(PathBuf::from(common.trim()).join("stapel/stops"))
}

pub(crate) fn key_order(key: &str) -> (u64, String) {
    let n = key
        .rsplit('-')
        .next()
        .and_then(|d| d.parse::<u64>().ok())
        .unwrap_or(0);
    (n, key.to_lowercase())
}

fn write_stop(path: &Path, files: &[String]) -> std::io::Result<()> {
    write_stop_kind(path, "merge", files)
}

/// Writes a stop file of `kind` (`merge` or `spec`) to a temporary file and renames it.
pub(crate) fn write_stop_kind(path: &Path, kind: &str, files: &[String]) -> std::io::Result<()> {
    let dir = path.parent().expect("stop folder");
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().expect("stop name").to_string_lossy();
    let tmp = dir.join(format!("{name}.{}.tmp", std::process::id()));
    let body = serde_json::json!({
        "kind": kind,
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
                    Err(e) => {
                        results.push(Trial::StopNotWritten(format!("{}: {e}", stop.display())))
                    }
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
