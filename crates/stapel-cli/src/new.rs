//! `stapel new`: a ticket folder with `ticket.md` and `state.json` (STP-2 AC-1..AC-3).

use crate::fail::Fail;
use crate::repo;
use stapel_core::state::{State, save};
use stapel_core::tickets::{key_number, next_key, tickets_dir};
use std::path::{Path, PathBuf};

pub fn run(title: &str, tracker: Option<&str>) -> Result<(), Fail> {
    create(title, tracker).map_err(|e| match e {
        Refusal::Refused(m) => Fail::refused(m),
        Refusal::Failed(m) => Fail::failed(m),
    })
}

enum Refusal {
    Refused(String),
    Failed(String),
}

impl From<String> for Refusal {
    fn from(m: String) -> Self {
        Refusal::Failed(m)
    }
}

fn create(title: &str, tracker: Option<&str>) -> Result<(), Refusal> {
    let (root, config) = repo::open()?;
    let title = title.trim();
    if title.is_empty() || title.chars().any(char::is_control) {
        return Err("the title must be non-empty and contain no control characters"
            .to_string()
            .into());
    }
    if let Some(url) = tracker {
        let scheme_ok = ["http://", "https://"]
            .iter()
            .any(|s| url.len() > s.len() && url.starts_with(s));
        if !scheme_ok || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(format!(
                "the tracker URL \"{}\" must start with http:// or https:// and contain no spaces",
                url.escape_debug()
            )
            .into());
        }
    }

    let base = read_base(&root)?;
    let branch = current_branch(&root);
    let mut worktree_note = None;
    let (key, dest, base_branch) = match (&base, &branch) {
        (Some(_), Some(branch)) => {
            if key_number(&config, branch).is_some() {
                return Err(Refusal::Refused(format!(
                    "stapel new runs on the base branch, not on the ticket branch {branch}"
                )));
            }
            let dirty = dirty_paths(&root)?;
            if !dirty.is_empty() {
                return Err(Refusal::Refused(format!(
                    "commit the changes of {} first; the new worktree starts at HEAD",
                    dirty.join(", ")
                )));
            }
            let (key, dest) = add_worktree(&config, &root)?;
            worktree_note = Some(key.clone());
            (key, dest, Some(branch.clone()))
        }
        _ => (next_key(&config, &root), root.clone(), None),
    };
    let dir = tickets_dir(&dest).join(&key);
    if std::fs::symlink_metadata(&dir).is_ok() {
        return Err(format!("ticket {key} already exists at {}", dir.display()).into());
    }
    std::fs::create_dir_all(dir.parent().expect("tickets dir"))
        .and_then(|()| std::fs::create_dir(&dir))
        .map_err(|e| format!("{}: {e}", dir.display()))?;

    // A configured `description` section is the Description; without one, write it here.
    let is_description = |id: &str| id.eq_ignore_ascii_case("description");
    let has_description = config.sections.iter().any(|s| is_description(&s.id));
    let mut text = format!("# {key} — {title}\n");
    let tracker_line = tracker.map(|url| format!("Tracker: {url}\n\n"));
    if !has_description {
        text.push_str("\n## Description\n\n");
        text.push_str(tracker_line.as_deref().unwrap_or(""));
        text.push_str("TODO\n");
    }
    for section in &config.sections {
        text.push_str(&format!("\n## {}\n\n", section.title));
        if is_description(&section.id) {
            text.push_str(tracker_line.as_deref().unwrap_or(""));
        }
        text.push_str("TODO\n");
    }
    std::fs::write(dir.join("ticket.md"), text).map_err(|e| format!("ticket.md: {e}"))?;

    let mut state = State::new(&key, title);
    state.tracker = tracker.map(String::from);
    state.base = base.clone();
    state.base_branch = base_branch.clone();
    save(&dir.join("state.json"), &state)?;
    say!("created: {key}");
    if let (Some(key), Some(base_branch)) = (worktree_note, base_branch) {
        let short = git(&root, &["rev-parse", "--short", "HEAD"])?;
        say!("branch: {key} from {base_branch} at {short}");
        say!("worktree: .claude/worktrees/{key}");
        if let Some(setup) = config.worktree.as_ref().and_then(|w| w.setup.as_ref()) {
            run_setup(setup, &dest)?;
            say!("setup: ok");
        }
    } else if base.is_none() {
        say!("note: no commit yet; the ticket stays in this checkout");
    }
    Ok(())
}

/// Runs `[worktree] setup` through `sh -c` in the worktree: no input, all output on stderr.
fn run_setup(command: &str, worktree: &Path) -> Result<(), Refusal> {
    use std::os::unix::process::ExitStatusExt;
    let failed = |status: String| {
        Refusal::Failed(format!(
            "the setup command failed ({status}); the worktree is kept: {}",
            worktree.display()
        ))
    };
    let stderr = crate::out::stderr_duplicate()
        .map_err(|e| failed(format!("could not start: {e}")))?;
    let stdout = stderr
        .try_clone()
        .map_err(|e| failed(format!("could not start: {e}")))?;
    let status = std::process::Command::new("sh")
        .args(["-c", command])
        .current_dir(worktree)
        .stdin(std::process::Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .status()
        .map_err(|e| failed(format!("could not start: {e}")))?;
    if status.success() {
        return Ok(());
    }
    Err(failed(match (status.code(), status.signal()) {
        (Some(n), _) => format!("exit {n}"),
        (None, Some(n)) => format!("signal {n}"),
        _ => "unknown status".to_string(),
    }))
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// The checked-out branch; `None` on a detached HEAD.
fn current_branch(root: &Path) -> Option<String> {
    git(root, &["symbolic-ref", "--short", "-q", "HEAD"])
        .ok()
        .filter(|b| !b.is_empty())
}

/// A file the tool writes itself: JSON records under `.stapel/`.
fn is_machine_file(path: &str) -> bool {
    path.starts_with(".stapel/") && (path.ends_with(".json") || path.ends_with(".jsonl"))
}

/// Paths with changes in the tree, machine files left out.
fn dirty_paths(root: &Path) -> Result<Vec<String>, String> {
    let out = std::process::Command::new("git")
        .args(["status", "--porcelain", "-z", "-uall"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git status: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut parts = text.split('\0').filter(|p| !p.is_empty());
    let mut paths = Vec::new();
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        if matches!(entry.as_bytes()[0], b'R' | b'C') {
            parts.next();
        }
        let path = entry[3..].to_string();
        if !is_machine_file(&path)
            && !stapel_core::worktree::is_machine_file(&path)
            && !stapel_core::worktree::block_only(root, "HEAD", &path)
        {
            paths.push(path);
        }
    }
    Ok(paths)
}

/// Takes the next free key and makes its branch and worktree.
fn add_worktree(
    config: &stapel_core::config::Config,
    root: &Path,
) -> Result<(String, PathBuf), String> {
    let main = stapel_core::parallel::worktrees(root)
        .into_iter()
        .next()
        .map(|w| w.path)
        .unwrap_or_else(|| root.to_path_buf());
    // The lock holds from the key scan to `git worktree add -b`; it is freed when `_lock` drops.
    let common = git(root, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    let lock_dir = PathBuf::from(common).join("stapel");
    std::fs::create_dir_all(&lock_dir).map_err(|e| format!("{}: {e}", lock_dir.display()))?;
    let lock_path = lock_dir.join("new.lock");
    let _lock = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| format!("{}: {e}", lock_path.display()))?;
    _lock
        .lock()
        .map_err(|e| format!("{}: {e}", lock_path.display()))?;
    let key = next_key(config, root);
    let path = main.join(".claude/worktrees").join(&key);
    let path_arg = path.to_string_lossy().into_owned();
    git(root, &["worktree", "add", "-q", "-b", &key, &path_arg, "HEAD"])?;
    Ok((key, path))
}

/// The full id of HEAD. A repository without commits (an unborn HEAD) has no base; any other git
/// error is returned.
fn read_base(root: &std::path::Path) -> Result<Option<String>, String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "-q", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        return Ok(Some(
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        ));
    }
    // With `-q`, git exits 1 without a message when HEAD names no commit yet.
    if out.status.code() == Some(1) && out.stderr.is_empty() {
        return Ok(None);
    }
    Err(format!(
        "git rev-parse --verify HEAD: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    ))
}
