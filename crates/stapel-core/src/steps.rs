//! A ticket's steps from git history (STP-4 AC-1): step commits, labels and step tests.

use crate::rust_tests::test_functions;
use std::collections::BTreeMap;
use std::path::Path;

/// The empty tree of the repository's hash (SHA-1 or SHA-256), the parent of a root commit.
fn empty_tree(root: &Path) -> String {
    git_text(root, &["hash-object", "-t", "tree", "/dev/null"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    Red,
    Green,
}

#[derive(Debug, Clone)]
pub struct StepCommit {
    pub sha: String,
    pub subject: String,
    pub message: String,
    pub label: String,
    pub marker: Marker,
}

#[derive(Debug, Clone)]
pub struct Step {
    pub label: String,
    /// Step commits of this label, oldest first.
    pub commits: Vec<StepCommit>,
}

impl Step {
    pub fn reds(&self) -> impl Iterator<Item = &StepCommit> {
        self.commits.iter().filter(|c| c.marker == Marker::Red)
    }
    pub fn greens(&self) -> impl Iterator<Item = &StepCommit> {
        self.commits.iter().filter(|c| c.marker == Marker::Green)
    }
}

/// A step test: `crates/<dir>/tests/<file>.rs`, function `name`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestId {
    pub dir: String,
    pub file: String,
    pub name: String,
}

impl std::fmt::Display for TestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}::{}", self.dir, self.file, self.name)
    }
}

impl TestId {
    pub fn path(&self) -> String {
        format!("crates/{}/tests/{}.rs", self.dir, self.file)
    }
}

pub fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

pub fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&git(root, args)?).into_owned())
}

/// The label and marker of a subject `<KEY> <label> RED: …` or `<KEY> <label> GREEN: …`: the marker
/// is the first ` RED: ` or ` GREEN: ` after the key, the label the trimmed text before it.
pub fn parse_subject(key: &str, subject: &str) -> Option<(String, Marker)> {
    let rest = subject.strip_prefix(key)?.strip_prefix(' ')?;
    let red = rest.find(" RED: ").map(|i| (i, Marker::Red));
    let green = rest.find(" GREEN: ").map(|i| (i, Marker::Green));
    let (at, marker) = match (red, green) {
        (Some(r), Some(g)) => {
            if r.0 < g.0 {
                r
            } else {
                g
            }
        }
        (Some(m), None) | (None, Some(m)) => m,
        (None, None) => return None,
    };
    let label = rest[..at].trim();
    (!label.is_empty()).then(|| (label.to_string(), marker))
}

/// The ticket's step commits on the first-parent history of `head`, oldest first.
pub fn step_commits(root: &Path, key: &str, head: &str) -> Result<Vec<StepCommit>, String> {
    // Fields are split on NUL, which a commit message cannot hold.
    let raw = git(
        root,
        &[
            "log",
            "-z",
            "--first-parent",
            "--reverse",
            "--format=%H%x00%s%x00%B",
            head,
        ],
    )?;
    let fields: Vec<String> = raw
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect();
    let mut out = Vec::new();
    for c in fields.chunks(3) {
        let [sha, subject, message] = c else { continue };
        let sha = sha.trim_start_matches('\n');
        if sha.is_empty() {
            continue;
        }
        if let Some((label, marker)) = parse_subject(key, subject) {
            out.push(StepCommit {
                sha: sha.to_string(),
                subject: subject.to_string(),
                message: message.to_string(),
                label,
                marker,
            });
        }
    }
    Ok(out)
}

/// Step commits grouped by label, labels in the order of their first commit.
pub fn steps(commits: Vec<StepCommit>) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    for c in commits {
        match steps.iter_mut().find(|s| s.label == c.label) {
            Some(step) => step.commits.push(c),
            None => steps.push(Step {
                label: c.label.clone(),
                commits: vec![c],
            }),
        }
    }
    steps
}

/// `crates/<dir>/tests/<file>.rs` as (dir, file).
fn test_file(path: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["crates", dir, "tests", file] => {
            let file = file.strip_suffix(".rs")?;
            Some((dir.to_string(), file.to_string()))
        }
        _ => None,
    }
}

/// The first parent of a commit, or the empty tree for a root commit.
pub fn parent(root: &Path, sha: &str) -> String {
    git_text(root, &["rev-parse", "--verify", "-q", &format!("{sha}^1")])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| empty_tree(root))
}

/// The file at a commit, or `None` when it does not exist there.
pub fn show(root: &Path, commit: &str, path: &str) -> Option<String> {
    git(root, &["show", &format!("{commit}:{path}")])
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// One entry of `git diff --name-status -M -z`: the status letter, the old and the new path.
#[derive(Debug, Clone)]
pub struct Change {
    pub status: char,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// The changes of a commit against its first parent, renames detected.
pub fn changes(root: &Path, sha: &str) -> Result<Vec<Change>, String> {
    let base = parent(root, sha);
    let raw = git(root, &["diff", "--name-status", "-M", "-z", &base, sha])?;
    let fields: Vec<String> = raw
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        let status = fields[i].chars().next();
        let Some(status) = status else {
            i += 1;
            continue;
        };
        if status == 'R' || status == 'C' {
            if i + 2 >= fields.len() {
                break;
            }
            out.push(Change {
                status,
                old: Some(fields[i + 1].clone()),
                new: Some(fields[i + 2].clone()),
            });
            i += 3;
        } else {
            let Some(path) = fields.get(i + 1) else {
                break;
            };
            let (old, new) = match status {
                'A' => (None, Some(path.clone())),
                'D' => (Some(path.clone()), None),
                _ => (Some(path.clone()), Some(path.clone())),
            };
            out.push(Change { status, old, new });
            i += 2;
        }
    }
    Ok(out)
}

/// The step tests of a RED commit: test functions whose text it adds or changes.
pub fn step_tests(root: &Path, red: &str) -> Result<Vec<TestId>, String> {
    let base = parent(root, red);
    let mut out = Vec::new();
    for change in changes(root, red)? {
        let Some(new) = &change.new else { continue };
        let Some((dir, file)) = test_file(new) else {
            continue;
        };
        let now = show(root, red, new).unwrap_or_default();
        let before: BTreeMap<String, String> = change
            .old
            .as_deref()
            .and_then(|old| show(root, &base, old))
            .map(|src| test_functions(&src).into_iter().collect())
            .unwrap_or_default();
        for (name, text) in test_functions(&now) {
            if before.get(&name) != Some(&text) {
                out.push(TestId {
                    dir: dir.clone(),
                    file: file.clone(),
                    name,
                });
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// The package name in `crates/<dir>/Cargo.toml` at a commit.
pub fn package_name(root: &Path, commit: &str, dir: &str) -> Result<String, String> {
    let at = &commit[..commit.len().min(7)];
    let manifest = show(root, commit, &format!("crates/{dir}/Cargo.toml"))
        .ok_or_else(|| format!("crates/{dir}/Cargo.toml is missing at {at}"))?;
    let table: toml::Table =
        toml::from_str(&manifest).map_err(|e| format!("crates/{dir}/Cargo.toml at {at}: {e}"))?;
    table
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(String::from)
        .ok_or_else(|| format!("crates/{dir}/Cargo.toml at {at} has no package name"))
}
