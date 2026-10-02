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
    git_config(dir.path(), "user.name", "test-user");
    dir
}

/// Sets a local git config value in the repository at `dir`.
pub fn git_config(dir: &Path, key: &str, value: &str) {
    let status = std::process::Command::new("git")
        .args(["config", key, value])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

/// `stapel` running in `dir`, with stdin piped so it is never a terminal.
pub fn stapel(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("stapel").unwrap();
    cmd.current_dir(dir)
        .env(
            "GIT_CEILING_DIRECTORIES",
            std::env::temp_dir().canonicalize().unwrap(),
        )
        .env_remove("STAPEL_ASSUME_TTY")
        .env_remove("CLAUDE_PROJECT_DIR")
        // The tests run the same inside and outside an agent's shell, and never see the
        // developer's global git identity.
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .write_stdin("");
    cmd
}

pub fn read(dir: &Path, rel: &str) -> String {
    std::fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Every file under `dir` except `.git/`, with its content and modification time.
pub fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>, std::time::SystemTime)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>, std::time::SystemTime)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().display().to_string();
                let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
                out.push((rel, std::fs::read(&path).unwrap(), mtime));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// A git repository with `stapel init --prefix ABC` done.
pub fn stapel_repo() -> TempDir {
    let repo = git_repo();
    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    repo
}

pub fn state_json(dir: &Path, key: &str) -> serde_json::Value {
    serde_json::from_str(&read(dir, &format!(".stapel/tickets/{key}/state.json"))).unwrap()
}
