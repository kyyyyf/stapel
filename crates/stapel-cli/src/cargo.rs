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
    file: File,
}

impl Drop for CargoLock {
    fn drop(&mut self) {
        // The holder line is read only while the lock is held; a dropped lock leaves no line.
        let _ = self.file.set_len(0);
    }
}

fn lock_path() -> Result<PathBuf, String> {
    match std::env::var_os("HOME") {
        Some(home) if !home.is_empty() => {
            Ok(PathBuf::from(home).join(".cache/stapel/cargo.lock"))
        }
        _ => Err("no folder for the cargo lock: HOME is not set".into()),
    }
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

/// Takes the cargo lock, waiting with no time limit, and writes the holder line for `command`.
/// `Ok(None)` when the held mark is set: an outer holder has the lock already. The lock descriptor
/// stays open in every child, so a killed holder frees the lock only when its cargo child ends.
pub fn take(command: &str) -> Result<Option<CargoLock>, String> {
    if mark_is_set() {
        return Ok(None);
    }
    let path = lock_path()?;
    let dir = path.parent().expect("the lock path has a folder");
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("cannot make the folder of the cargo lock {}: {e}", dir.display()))?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("cannot open the cargo lock {}: {e}", path.display()))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            let holder = std::fs::read_to_string(&path).unwrap_or_default();
            let holder = holder.trim();
            warn!(
                "waiting for the cargo lock: held by {}",
                if holder.is_empty() {
                    "an unknown holder"
                } else {
                    holder
                }
            );
            file.lock()
                .map_err(|e| format!("cannot take the cargo lock {}: {e}", path.display()))?;
        }
        Err(TryLockError::Error(e)) => {
            return Err(format!(
                "cannot take the cargo lock {}: {e}",
                path.display()
            ));
        }
    }
    let line = format!(
        "{}, pid {}, since {}, {command}",
        branch_ticket_key(),
        std::process::id(),
        stapel_core::time::now_rfc3339()
    );
    file.set_len(0)
        .and_then(|()| (&file).write_all(line.as_bytes()))
        .map_err(|e| format!("cannot write the cargo lock {}: {e}", path.display()))?;
    inherit(&file)?;
    Ok(Some(CargoLock { file }))
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
