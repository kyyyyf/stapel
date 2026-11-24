//! The machine-wide cargo lock and `stapel cargo` (STP-27 AC-10, AC-11): one cargo run at a time,
//! because parallel builds crashed the machine (STP-4).

use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

/// The variable the lock holder sets for its children; with it set, no lock is taken.
pub const HELD_MARK: &str = "STAPEL_CARGO_LOCK_HELD";

/// The held cargo lock: an OS lock on the lock file, kept until the value is dropped or every
/// process that inherited the descriptor has ended.
pub struct CargoLock {
    files: Vec<File>,
}

impl Drop for CargoLock {
    fn drop(&mut self) {
        // The holder line is read only while the lock is held; a dropped lock leaves no line.
        for file in &self.files {
            let _ = file.set_len(0);
        }
    }
}

/// The lock folder `~/.cache/stapel`.
fn lock_dir() -> Result<PathBuf, String> {
    match std::env::var_os("HOME") {
        Some(home) if !home.is_empty() => Ok(PathBuf::from(home).join(".cache/stapel")),
        _ => Err("no folder for the cargo lock: HOME is not set".into()),
    }
}

/// The lock file of slot `i` (from 1): `cargo.lock`, then `cargo-<i>.lock`.
fn slot_path(dir: &std::path::Path, i: u32) -> PathBuf {
    if i == 1 {
        dir.join("cargo.lock")
    } else {
        dir.join(format!("cargo-{i}.lock"))
    }
}

/// N: `[cargo] slots` of the repository of the current folder, 3 outside a repository.
fn slot_count() -> u32 {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| stapel_core::guard::stapel_root(&cwd))
        .and_then(|root| crate::repo::load(&root).ok())
        .map(|c| c.cargo_slots())
        .unwrap_or(stapel_core::config::DEFAULT_CARGO_SLOTS)
}

fn mark_is_set() -> bool {
    std::env::var_os(HELD_MARK).is_some_and(|v| !v.is_empty())
}

/// The ticket of the current branch, or `-`.
fn branch_ticket_key() -> String {
    crate::repo::open()
        .ok()
        .and_then(|(root, _)| stapel_core::tickets::branch_ticket(&root))
        .map(|t| t.key)
        .unwrap_or_else(|| "-".into())
}

/// The lock file of the worktree of the current folder: `worktree-<h>.lock`, `<h>` the first 16 hex
/// digits of the SHA-256 of the canonical root of its git working tree (the folder itself outside git).
fn worktree_path(dir: &std::path::Path) -> PathBuf {
    use sha2::Digest;
    let cwd = std::env::current_dir().unwrap_or_default();
    let top = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim_end_matches('\n')))
        .unwrap_or_else(|| cwd.clone());
    let root = top.canonicalize().unwrap_or(top);
    let digest = sha2::Sha256::digest(root.to_string_lossy().as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    dir.join(format!("worktree-{hex}.lock"))
}

fn open_lock(path: &std::path::Path) -> Result<File, String> {
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|e| format!("cannot open the cargo lock {}: {e}", path.display()))
}

/// The holder line of a lock file, or the unknown holder.
fn holder_of(path: &std::path::Path) -> String {
    let holder = std::fs::read_to_string(path).unwrap_or_default();
    let holder = holder.trim();
    if holder.is_empty() {
        "an unknown holder (a process started by an earlier cargo run may hold it)".to_string()
    } else {
        holder.to_string()
    }
}

/// Takes one of `locks` (a file and its path), polling with no time limit; prints one waiting line
/// naming every holder on the first miss. Returns the index taken.
fn take_one(locks: &[(PathBuf, File)]) -> Result<usize, String> {
    let mut warned = false;
    loop {
        for (n, (path, file)) in locks.iter().enumerate() {
            match file.try_lock() {
                Ok(()) => return Ok(n),
                Err(TryLockError::WouldBlock) => {}
                Err(TryLockError::Error(e)) => {
                    return Err(format!("cannot take the cargo lock {}: {e}", path.display()));
                }
            }
        }
        if !warned {
            warned = true;
            let holders: Vec<String> = locks.iter().map(|(path, _)| holder_of(path)).collect();
            warn!("waiting for the cargo lock: held by {}", holders.join("; "));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Takes the worktree lock, then one slot, waiting with no time limit, and writes the holder line for
/// `command` into both. `Ok(None)` when the held mark is set: an outer holder has the locks already.
/// The descriptors stay open in every child, so a killed holder frees the locks only when its cargo
/// child ends.
pub fn take(command: &str) -> Result<Option<CargoLock>, String> {
    if mark_is_set() {
        return Ok(None);
    }
    let dir = lock_dir()?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("cannot make the folder of the cargo lock {}: {e}", dir.display()))?;
    let wt_path = worktree_path(&dir);
    let wt_file = open_lock(&wt_path)?;
    take_one(&[(wt_path.clone(), wt_file.try_clone().map_err(|e| e.to_string())?)])?;
    let line = format!(
        "{}, pid {}, since {}, {command}",
        branch_ticket_key(),
        std::process::id(),
        stapel_core::time::now_rfc3339()
    );
    // The worktree holder is named before the slot wait, so a second run of the worktree sees it.
    write_line(&wt_path, &wt_file, &line)?;
    let mut slots = Vec::new();
    for i in 1..=slot_count() {
        let path = slot_path(&dir, i);
        let file = open_lock(&path)?;
        slots.push((path, file));
    }
    let n = take_one(&slots)?;
    let (path, file) = slots.swap_remove(n);
    write_line(&path, &file, &line)?;
    inherit(&wt_file)?;
    inherit(&file)?;
    Ok(Some(CargoLock {
        files: vec![wt_file, file],
    }))
}

/// Writes the holder line into a held lock file.
fn write_line(path: &std::path::Path, file: &File, line: &str) -> Result<(), String> {
    file.set_len(0)
        .and_then(|()| (&*file).write_all(line.as_bytes()))
        .map_err(|e| format!("cannot write the cargo lock {}: {e}", path.display()))
}

/// Clears close-on-exec on the lock descriptor, so children keep the lock.
fn inherit(file: &File) -> Result<(), String> {
    let fd = file.as_raw_fd();
    // SAFETY: `fd` is an open descriptor of this process; F_GETFD and F_SETFD change only its flags.
    let ok = unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFD);
        flags >= 0 && libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) >= 0
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "cannot share the cargo lock with cargo: {}",
            std::io::Error::last_os_error()
        ))
    }
}

/// `stapel cargo <args>`: runs `cargo <args>` under the cargo lock with the same stdio.
pub fn run(args: &[String]) -> ExitCode {
    let command = if args.is_empty() {
        "stapel cargo".to_string()
    } else {
        format!("stapel cargo {}", args.join(" "))
    };
    // A `stapel.toml` that fails loading refuses the run; outside a stapel repository N is 3.
    let in_repo = std::env::current_dir()
        .ok()
        .and_then(|cwd| stapel_core::guard::stapel_root(&cwd));
    if let Some(root) = in_repo {
        if let Err(e) = crate::repo::load(&root) {
            warn!("stapel: {e}");
            return ExitCode::FAILURE;
        }
    }
    let lock = match take(&command) {
        Ok(l) => l,
        Err(e) => {
            warn!("stapel: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut cmd = Command::new("cargo");
    cmd.args(args);
    if lock.is_some() {
        cmd.env(HELD_MARK, std::process::id().to_string());
    }
    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) => {
            warn!("stapel: cargo did not start: {e}");
            return ExitCode::FAILURE;
        }
    };
    drop(lock);
    let code = status
        .code()
        .or_else(|| status.signal().map(|s| 128 + s))
        .unwrap_or(1);
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
