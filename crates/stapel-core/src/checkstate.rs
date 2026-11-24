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
    /// A current pass: the short commit id and the level (`steps`, `head`, `full`).
    Pass(String, String),
    Fail(String),
    Stale(String),
}

impl std::fmt::Display for CheckLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckLine::NotConfigured => write!(f, "check: not configured"),
            CheckLine::None => write!(f, "check: none"),
            CheckLine::Pass(sha, level) if level == "steps" => {
                write!(f, "check: pass at {sha} (level steps)")
            }
            CheckLine::Pass(sha, _) => write!(f, "check: pass at {sha}"),
            CheckLine::Fail(sha) => write!(f, "check: fail at {sha}"),
            CheckLine::Stale(reason) => write!(f, "check: stale ({reason})"),
        }
    }
}

fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// The lines of a text: split at `\n`, trailing whitespace removed, empty lines left out.
fn line_set(text: &str) -> std::collections::HashSet<String> {
    text.split('\n')
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Whether `path` is a regular file at `head` and now, and its lines now are a subset of its lines at `head`.
fn baseline_subset(root: &Path, head: &str, path: &str) -> bool {
    let regular_at_head = git(root, &["ls-tree", head, "--", path])
        .map(|o| String::from_utf8_lossy(&o).starts_with("100"))
        .unwrap_or(false);
    if !regular_at_head {
        return false;
    }
    let Ok(old) = git(root, &["show", &format!("{head}:{path}")]) else {
        return false;
    };
    let full = root.join(path);
    if !std::fs::symlink_metadata(&full).is_ok_and(|m| m.is_file()) {
        return false;
    }
    let Ok(now) = std::fs::read(&full) else {
        return false;
    };
    let old = line_set(&String::from_utf8_lossy(&old));
    line_set(&String::from_utf8_lossy(&now)).is_subset(&old)
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
                // Well-formed: a full commit id and a result of pass or fail.
                let head = m.get("head")?.as_str()?.to_string();
                let result = m.get("result")?.as_str()?.to_string();
                let full = (head.len() == 40 || head.len() == 64)
                    && head.bytes().all(|b| b.is_ascii_hexdigit());
                let plan = m
                    .get("plan_hash")
                    .and_then(|p| p.as_str())
                    .map(String::from);
                // A record without `level` was written before levels and counts as `full`.
                let level = match m.get("level").and_then(|l| l.as_str()) {
                    Some(l @ ("steps" | "head" | "full")) => l.to_string(),
                    _ => "full".to_string(),
                };
                (full && (result == "pass" || result == "fail"))
                    .then_some((head, result, plan, level))
            }
            _ => None,
        });
    let Some((head, result, recorded_plan, level)) = last else {
        return CheckLine::None;
    };
    let known = git(root, &["cat-file", "-e", &format!("{head}^{{commit}}")]).is_ok();
    if !known {
        return CheckLine::Stale(format!("unknown commit {}", short(&head)));
    }
    if git(root, &["merge-base", "--is-ancestor", &head, "HEAD"]).is_err() {
        return CheckLine::Stale(format!(
            "history rewritten: {} is not an ancestor of HEAD",
            short(&head)
        ));
    }
    // When git cannot read the tree, the check is not current: the gate fails closed.
    let diff = match git(root, &["diff", "--name-only", "--no-renames", "-z", &head]) {
        Ok(raw) => raw,
        Err(e) => return CheckLine::Stale(format!("cannot read the tree: {e}")),
    };
    // A `ticket.md` that differs from the checked commit only inside the generated block is not a change.
    let mut changed: Vec<String> = diff
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .filter(|p| !crate::worktree::block_only(root, &head, p))
        .collect();
    match dirty_paths(root) {
        Ok(d) => changed.extend(d),
        Err(e) => return CheckLine::Stale(format!("cannot read the tree: {e}")),
    }
    // Machine files and `docs/` do not change what a check ran (STP-28 AC-3).
    // Under junit a `docs/` path that matches a test or input glob stays (AC-36).
    let junit_globs: Vec<&String> = match config.check.as_ref().filter(|c| c.runner == "junit") {
        Some(c) => c.tests.iter().chain(c.inputs.iter()).flatten().collect(),
        None => Vec::new(),
    };
    // This ticket's own folder keeps the check while its Plan section is unchanged (STP-35 AC-17),
    // unless a junit test or input glob matches the path.
    let own_folder = ticket_dir
        .file_name()
        .map(|k| format!(".stapel/tickets/{}/", k.to_string_lossy()));
    let plan_same = recorded_plan.is_some_and(|recorded| {
        let text = std::fs::read_to_string(ticket_dir.join("ticket.md")).unwrap_or_default();
        let sections = crate::ticket::parse(&text);
        let now = match crate::ticket::find(&sections, "Plan") {
            crate::ticket::Lookup::Found(body) => crate::hash::section_hash(body),
            _ => "none".to_string(),
        };
        now == recorded
    });
    let globbed = |p: &str| {
        junit_globs
            .iter()
            .any(|g| crate::config::glob_matches(g, p))
    };
    // A path inside the guards folder stays a change, under `docs/` too, unless it is a baseline
    // path whose lines now are a subset of its lines at the checked commit (STP-41 AC-15).
    let mut guarded: Vec<String> = Vec::new();
    if let Some(guards) = &config.guards {
        let dir = guards.dir.strip_suffix('/').unwrap_or(&guards.dir);
        let prefix = format!("{dir}/");
        let baselines: Vec<String> = guards
            .check
            .iter()
            .filter_map(|g| g.baseline.as_ref())
            .map(|b| format!("{prefix}{b}"))
            .collect();
        changed.retain(|p| {
            if !p.starts_with(&prefix) {
                return true;
            }
            if !(baselines.contains(p) && baseline_subset(root, &head, p)) {
                guarded.push(p.clone());
            }
            false
        });
    }
    changed.retain(|p| {
        !is_machine_file(p)
            && (!p.starts_with("docs/") || globbed(p))
            && !(plan_same
                && own_folder.as_deref().is_some_and(|f| p.starts_with(f))
                && !globbed(p))
    });
    changed.extend(guarded);
    changed.sort();
    changed.dedup();
    if !changed.is_empty() {
        let first: Vec<&str> = changed.iter().take(5).map(String::as_str).collect();
        return CheckLine::Stale(format!("changed: {}", first.join(", ")));
    }
    if result == "pass" {
        CheckLine::Pass(short(&head), level)
    } else {
        CheckLine::Fail(short(&head))
    }
}
