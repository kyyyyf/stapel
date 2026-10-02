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
    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
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

fn write_call(dir: &Path, tool: &str, path: &str) -> assert_cmd::assert::Assert {
    let input = match tool {
        "NotebookEdit" => json!({ "notebook_path": path, "new_source": "x" }),
        "MultiEdit" => json!({ "file_path": path, "edits": [] }),
        _ => json!({ "file_path": path, "content": "x", "old_string": "a", "new_string": "b" }),
    };
    hook(dir, tool, input)
}

const WRITE_TOOLS: [&str; 4] = ["Write", "Edit", "MultiEdit", "NotebookEdit"];

fn set_state(dir: &Path, ticket: &str, state: &str) {
    let path = dir.join(".stapel/tickets").join(ticket);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("state.json"), state).unwrap();
}

// AC-8
#[test]
fn denies_code_write_without_build() {
    let repo = initialized_repo();
    let root = repo.path();
    for tool in WRITE_TOOLS {
        for path in [
            root.join("src/main.rs").display().to_string(),
            root.join("docs/../src/main.rs").display().to_string(),
            root.join("docs-old/notes.md").display().to_string(),
            "src/relative.rs".to_string(),
        ] {
            write_call(root, tool, &path)
                .code(DENY)
                .stderr(contains("сборка не разрешена"));
        }
    }
}

// AC-8
#[test]
fn allows_stapel_and_docs_writes() {
    let repo = initialized_repo();
    let root = repo.path();
    for tool in WRITE_TOOLS {
        for rel in [
            ".stapel/tickets/ABC-1/ticket.md",
            "docs/PLAN.md",
            "docs/deep/x.md",
        ] {
            write_call(root, tool, &root.join(rel).display().to_string()).success();
        }
        write_call(root, tool, "docs/relative.md").success();
    }
}

// AC-8: the list comes from stapel.toml.
#[test]
fn always_writable_comes_from_config() {
    let repo = initialized_repo();
    let root = repo.path();
    let config = root.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap().replace(
        "always_writable = [\".stapel/\", \"docs/\"]",
        "always_writable = [\".stapel/\", \"scripts/\"]",
    );
    std::fs::write(&config, text).unwrap();

    write_call(
        root,
        "Write",
        &root.join("scripts/a.sh").display().to_string(),
    )
    .success();
    write_call(
        root,
        "Write",
        &root.join("docs/PLAN.md").display().to_string(),
    )
    .code(DENY);
}

// AC-8: files outside the repository are not this repository's code.
#[test]
fn allows_writes_outside_repo() {
    let repo = initialized_repo();
    let elsewhere = bare_dir();
    write_call(
        repo.path(),
        "Write",
        &elsewhere.path().join("x.rs").display().to_string(),
    )
    .success();
}

// AC-9
#[test]
fn allows_code_write_when_build_allowed() {
    let repo = initialized_repo();
    let root = repo.path();
    let target = root.join("src/main.rs").display().to_string();

    set_state(root, "ABC-1", r#"{"build": {"allowed": false}}"#);
    set_state(root, "ABC-2", "{ broken");
    write_call(root, "Write", &target).code(DENY);

    set_state(
        root,
        "ABC-3",
        r#"{"key": "ABC-3", "build": {"allowed": true}}"#,
    );
    for tool in WRITE_TOOLS {
        write_call(root, tool, &target).success();
    }
}

// AC-7, R-1 (F-1, F-2, E-2, E-3, D-1): review round 1.
#[test]
fn denies_git_push_in_compound_forms() {
    let repo = initialized_repo();
    for command in [
        "{ git push; }",
        "if true; then git push; fi",
        "if false; then :; else git push; fi",
        "! git push",
        "for r in origin; do git push $r; done",
        "while true; do git push; done",
        "echo `git push`",
        "echo \"$(git push)\"",
        "x=$(git push)",
        "eval \"git push\"",
        "eval git push",
        "echo origin | xargs git push",
        "nice git push",
        "nice -n 5 git push",
        "timeout 30 git push",
        "env -u FOO git push",
        "sudo -u me git push",
        "2>&1 git push",
        ">log git push",
        "git send-pack origin HEAD",
        "git http-push url",
        "/usr/lib/git-core/git-push origin main",
        "git-push origin",
        "git -c alias.p=push p",
        "git -c alias.p='!git push' p",
    ] {
        bash(repo.path(), command)
            .code(DENY)
            .stderr(contains("git push"));
    }
}

#[test]
fn allows_lookalikes_after_round_one() {
    let repo = initialized_repo();
    for command in [
        "timeout 30 cargo test",
        "nice git status",
        "echo '$(git push)'",
        "echo '`git push`'",
        "git log --format=%s | grep push",
        "if git diff --quiet; then echo clean; fi",
        "git -c color.ui=never log",
    ] {
        bash(repo.path(), command).success();
    }
}

// E-1, F-5: machine files are written only by stapel, never by an agent's write tool.
#[test]
fn denies_machine_file_writes_even_with_build() {
    let repo = initialized_repo();
    let root = repo.path();
    set_state(root, "ABC-1", r#"{"build": {"allowed": true}}"#);
    for rel in [
        ".stapel/stapel.toml",
        ".stapel/tickets/ABC-1/state.json",
        ".stapel/tickets/ABC-9/state.json",
        ".stapel/tickets/ABC-1/decisions.jsonl",
        ".stapel/tickets/ABC-1/findings.jsonl",
        ".stapel/tickets/ABC-1/runs.jsonl",
        ".stapel/tickets/ABC-1/tokens.jsonl",
    ] {
        for tool in WRITE_TOOLS {
            write_call(root, tool, &root.join(rel).display().to_string())
                .code(DENY)
                .stderr(contains("только stapel"));
        }
    }
    for rel in [".stapel/tickets/ABC-1/ticket.md", ".stapel/allowlist.toml"] {
        write_call(root, "Write", &root.join(rel).display().to_string()).success();
    }
}

// F-3, E-6: the repository is found from the target path and from CLAUDE_PROJECT_DIR too.
#[test]
fn finds_repo_from_target_and_project_dir() {
    let repo = initialized_repo();
    let root = repo.path();
    let elsewhere = bare_dir();

    write_call(
        elsewhere.path(),
        "Write",
        &root.join("src/x.rs").display().to_string(),
    )
    .code(DENY);

    let payload = json!({
        "tool_name": "Bash",
        "cwd": elsewhere.path(),
        "tool_input": { "command": format!("git -C {} push", root.display()) },
    });
    stapel(elsewhere.path())
        .args(["hook", "pre-tool-use"])
        .env("CLAUDE_PROJECT_DIR", root)
        .write_stdin(payload.to_string())
        .assert()
        .code(DENY);
}

// F-4, E-7: symlinks are resolved before the path is matched.
#[cfg(unix)]
#[test]
fn resolves_symlinks_before_matching() {
    let repo = initialized_repo();
    let root = repo.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::os::unix::fs::symlink("../src", root.join("docs/link")).unwrap();

    write_call(
        root,
        "Write",
        &root.join("docs/link/x.rs").display().to_string(),
    )
    .code(DENY);
    write_call(
        root,
        "Write",
        &root.join("docs/link/new/x.rs").display().to_string(),
    )
    .code(DENY);

    // The same checkout reached through a symlinked path.
    let alias_dir = bare_dir();
    let alias = alias_dir.path().join("alias");
    std::os::unix::fs::symlink(root, &alias).unwrap();
    write_call(
        &alias,
        "Write",
        &alias.join("src/x.rs").display().to_string(),
    )
    .code(DENY);
    write_call(root, "Write", &alias.join("src/x.rs").display().to_string()).code(DENY);
    write_call(
        &alias,
        "Write",
        &alias.join("docs/a.md").display().to_string(),
    )
    .success();
}

// E-8: a config with a bad always_writable entry falls back to .stapel/ only.
#[test]
fn bad_always_writable_does_not_open_everything() {
    let repo = initialized_repo();
    let root = repo.path();
    let config = root.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap().replace(
        "always_writable = [\".stapel/\", \"docs/\"]",
        "always_writable = [\".stapel/\", \"/\"]",
    );
    std::fs::write(&config, text).unwrap();

    write_call(
        root,
        "Write",
        &root.join("src/main.rs").display().to_string(),
    )
    .code(DENY);
}
