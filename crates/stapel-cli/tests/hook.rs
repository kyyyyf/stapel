//! STP-1: `stapel hook pre-tool-use`, the Claude Code PreToolUse guard.

mod common;

use common::{bare_dir, git_repo, stapel};
use predicates::str::contains;
use serde_json::json;
use std::path::Path;

/// Exit code with which Claude Code blocks a tool call and shows stderr to the agent.
const DENY: i32 = 2;

fn initialized_repo() -> tempfile::TempDir {
    let repo = git_repo();
    stapel(repo.path()).args(["init", "--prefix", "ABC"]).assert().success();
    repo
}

fn hook(dir: &Path, tool: &str, input: serde_json::Value) -> assert_cmd::assert::Assert {
    let payload = json!({
        "session_id": "test",
        "hook_event_name": "PreToolUse",
        "cwd": dir,
        "tool_name": tool,
        "tool_input": input,
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .assert()
}

fn bash(dir: &Path, command: &str) -> assert_cmd::assert::Assert {
    hook(dir, "Bash", json!({ "command": command }))
}

// AC-7, R-1
#[test]
fn denies_git_push_variants() {
    let repo = initialized_repo();
    for command in [
        "git push",
        "git push origin main --force",
        "git -C some/dir push",
        "git -c user.name=x push",
        "git --git-dir=.git push",
        "git --git-dir .git --work-tree . push",
        "/usr/bin/git push",
        "cd sub && git push",
        "cd sub&&git push",
        "FOO=1 git push",
        "env FOO=1 git push",
        "git status; git push",
        "false || git push",
        "git fetch | git push",
        "git status\ngit push",
        "(git push)",
        "bash -c 'git push'",
        "sh -c \"cd x && git push\"",
    ] {
        bash(repo.path(), command)
            .code(DENY)
            .stderr(contains("git push"));
    }
}

// AC-7
#[test]
fn allows_non_push_commands() {
    let repo = initialized_repo();
    for command in [
        "git status",
        "git log --grep push",
        "echo \"git push\"",
        "echo 'git push' > notes.txt",
        "git commit -m 'push later'",
        "git stash push -m wip",
        "cargo test push",
        "ls pushd",
    ] {
        bash(repo.path(), command).success();
    }
}

// AC-10
#[test]
fn rejects_garbage_input() {
    let repo = initialized_repo();
    for garbage in ["", "not json", "{\"tool_input\": {}}", "[1, 2]"] {
        stapel(repo.path())
            .args(["hook", "pre-tool-use"])
            .write_stdin(garbage)
            .assert()
            .code(DENY)
            .stderr(contains("вход хука"));
    }
}

// AC-10
#[test]
fn passes_outside_stapel_repo() {
    let plain = git_repo();
    bash(plain.path(), "git push").success();
    let bare = bare_dir();
    bash(bare.path(), "git push").success();
}
