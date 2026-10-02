#![allow(dead_code)]

use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;

/// A temporary directory that git does not see as part of any enclosing repository.
pub fn bare_dir() -> TempDir {
    tempfile::tempdir().unwrap()
}

/// A temporary directory with a fresh `git init`.
pub fn git_repo() -> TempDir {
    let dir = bare_dir();
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    dir
}

/// `stapel` running in `dir`, with stdin piped so it is never a terminal.
pub fn stapel(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("stapel").unwrap();
    cmd.current_dir(dir)
        .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir().canonicalize().unwrap())
        .env_remove("STAPEL_ASSUME_TTY")
        .write_stdin("");
    cmd
}

pub fn read(dir: &Path, rel: &str) -> String {
    std::fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}
