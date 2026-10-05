//! Where `stapel check` runs (STP-4 AC-4): the lock, the worktree, and the dirty-tree test.

use crate::steps::{git, git_text};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Files of a ticket that tools write; their changes do not make a tree dirty or a check stale.
pub const MACHINE_FILES: [&str; 5] = [
    "state.json",
    "decisions.jsonl",
    "findings.jsonl",
    "runs.jsonl",
    "tokens.jsonl",
];

pub fn is_machine_file(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    matches!(parts.as_slice(), [".stapel", "tickets", _, f] if MACHINE_FILES.contains(f))
}

/// `<git common dir>/stapel`, created when missing.
pub fn stapel_dir(root: &Path) -> Result<PathBuf, String> {
    let common = git_text(root, &["rev-parse", "--git-common-dir"])?;
    let common = root.join(common.trim());
    let dir = common.join("stapel");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// Paths with uncommitted changes, tracked or untracked and not ignored, other than machine
/// files; both sides of a rename.
pub fn dirty_paths(root: &Path) -> Result<Vec<String>, String> {
    let raw = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let fields: Vec<String> = raw
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        let f = &fields[i];
        if f.len() < 4 {
            i += 1;
            continue;
        }
        let status = &f[..2];
        out.push(f[3..].to_string());
        if status.contains('R') || status.contains('C') {
            if let Some(old) = fields.get(i + 1) {
                out.push(old.clone());
            }
            i += 1;
        }
        i += 1;
    }
    out.retain(|p| !is_machine_file(p));
    Ok(out)
}

/// The OS lock on `check.lock`; the file holds the pid of its holder.
pub struct Lock {
    file: File,
}

pub enum LockError {
    Held(String),
    Io(String),
}

impl Lock {
    /// Takes the lock; returns the pid a previous holder left in the file, if any.
    pub fn take(dir: &Path) -> Result<(Lock, Option<String>), LockError> {
        let path = dir.join("check.lock");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| LockError::Io(format!("{}: {e}", path.display())))?;
        let read_pid = |file: &mut File| {
            let mut text = String::new();
            let _ = file.seek(SeekFrom::Start(0));
            let _ = file.read_to_string(&mut text);
            text.trim().to_string()
        };
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(LockError::Held(read_pid(&mut file)));
            }
            Err(std::fs::TryLockError::Error(e)) => {
                return Err(LockError::Io(format!("{}: {e}", path.display())));
            }
        }
        // Read only once the lock is held, so a holder that just left is not named.
        let previous = read_pid(&mut file);
        let own = std::process::id().to_string();
        let _ = file.set_len(0);
        let _ = file.seek(SeekFrom::Start(0));
        let _ = writeln!(file, "{own}");
        let previous = (!previous.is_empty() && previous != own).then_some(previous);
        Ok((Lock { file }, previous))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.file.set_len(0);
        let _ = self.file.unlock();
    }
}

/// The detached worktree `check-worktree`, removed when dropped.
pub struct Worktree {
    root: PathBuf,
    pub path: PathBuf,
}

impl Worktree {
    pub fn create(root: &Path, dir: &Path, commit: &str) -> Result<Worktree, String> {
        let path = dir.join("check-worktree");
        let wt = Worktree {
            root: root.to_path_buf(),
            path,
        };
        wt.remove();
        let p = wt.path.to_string_lossy().into_owned();
        // `--force` re-uses a registration of its own path whose folder is gone; no other
        // worktree is pruned.
        git(
            root,
            &["worktree", "add", "-q", "--force", "--detach", &p, commit],
        )?;
        Ok(wt)
    }

    pub fn checkout(&self, commit: &str) -> Result<(), String> {
        git(
            &self.path,
            &["checkout", "-q", "--detach", "--force", commit],
        )?;
        git(&self.path, &["clean", "-q", "-fdx"])?;
        Ok(())
    }

    fn remove(&self) {
        let p = self.path.to_string_lossy().into_owned();
        let _ = git(&self.root, &["worktree", "remove", "--force", &p]);
        if self.path.exists() {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        self.remove();
    }
}
