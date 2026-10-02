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
            .stderr(contains("hook input"));
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
                .stderr(contains("build is not allowed"));
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
                .stderr(contains("only stapel"));
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

// AC-7, R-1 (F2-3, F2-4, E2-1, E2-2): review round 2.
#[test]
fn denies_git_push_round_two() {
    let repo = initialized_repo();
    for command in [
        "cat <(git push)",
        "diff <(ls) <(git push)",
        "git \\\npush",
        "git pu\\\nsh origin",
        "P=push; git $P",
        "git $(echo push)",
        "git `echo push`",
        "git $'push'",
        "x=`git push`",
        "echo git push | sh",
        "echo git push | bash -s",
        "bash <<< 'git push'",
        "find . -exec git push \\;",
        "watch git push",
        "parallel git push ::: a",
        "echo push | xargs git",
        "env -S 'git push'",
        "bash -c -- 'git push'",
        "git subtree push --prefix=a o m",
        "git submodule foreach 'git push'",
        "git rebase --exec 'git push' HEAD~1",
        "git rebase -x 'git push' HEAD~1",
        "GIT_CONFIG_PARAMETERS=\"'alias.p=push'\" git p",
        "git --config-env=alias.p=X p",
        "git -c Alias.P=push p",
    ] {
        bash(repo.path(), command)
            .code(DENY)
            .stderr(contains("git push"));
    }
}

#[test]
fn allows_lookalikes_after_round_two() {
    let repo = initialized_repo();
    for command in [
        "git commit -m 'git push later'",
        "git commit -m \"see $(date) before push\"",
        "timeout 5 cargo build",
        "echo ls | sh",
        "git submodule update --init",
        "git subtree add --prefix=a o m",
        "git rebase --exec 'cargo test' HEAD~2",
        "find . -name '*.rs' -exec rustfmt {} \\;",
        "cargo test \\\n  --workspace",
        "echo $(git rev-parse HEAD)",
    ] {
        bash(repo.path(), command).success();
    }
}

// AC-13: commands that would switch off the pre-push layer are denied.
#[test]
fn denies_pre_push_bypass() {
    let repo = initialized_repo();
    for command in [
        "unset CLAUDECODE; git push",
        "unset CLAUDECODE",
        "env -u CLAUDECODE sh x.sh",
        "CLAUDECODE= sh x.sh",
        "export CLAUDECODE=",
        "git config core.hooksPath /tmp/none",
        "git -c core.hooksPath=/dev/null status",
        "rm .git/hooks/pre-push",
        "chmod -x .git/hooks/pre-push",
    ] {
        bash(repo.path(), command)
            .code(DENY)
            .stderr(contains("pre-push"));
    }
}

// F2-1, F2-2: pathological input is denied quickly instead of crashing or timing out.
#[test]
fn pathological_input_is_denied_not_crashed() {
    let repo = initialized_repo();
    let chain = "env ".repeat(40);

    let within = |command: String| {
        let payload = json!({
            "tool_name": "Bash",
            "cwd": repo.path(),
            "tool_input": { "command": command },
        });
        stapel(repo.path())
            .args(["hook", "pre-tool-use"])
            .timeout(std::time::Duration::from_secs(5))
            .write_stdin(payload.to_string())
            .assert()
    };
    within(format!("{chain}ls")).success();
    within(format!("{chain}ls; git push")).code(DENY);

    let nested = "echo \"$(".repeat(5_000);
    bash(repo.path(), &nested).code(DENY);
    bash(repo.path(), &"eval ".repeat(10_000)).code(DENY);
    bash(repo.path(), &"x".repeat(100_000)).code(DENY);
}

// E2-3, E2-6: .claude/settings.json and case variants of machine files are protected too.
#[test]
fn denies_settings_and_case_variants() {
    let repo = initialized_repo();
    let root = repo.path();
    set_state(root, "ABC-1", r#"{"build": {"allowed": true}}"#);
    for rel in [
        ".claude/settings.json",
        ".stapel/tickets/ABC-1/STATE.json",
        ".stapel/STAPEL.toml",
    ] {
        write_call(root, "Write", &root.join(rel).display().to_string())
            .code(DENY)
            .stderr(contains("only stapel"));
    }
}

// F2-5: a dangling symlink is followed to where the write would land.
#[cfg(unix)]
#[test]
fn follows_dangling_symlink() {
    let repo = initialized_repo();
    let root = repo.path();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::os::unix::fs::symlink("../src/new.rs", root.join("docs/lnk")).unwrap();

    write_call(root, "Write", &root.join("docs/lnk").display().to_string()).code(DENY);
}

fn bash_within(dir: &Path, command: &str) -> assert_cmd::assert::Assert {
    let payload = json!({
        "tool_name": "Bash",
        "cwd": dir,
        "tool_input": { "command": command },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .timeout(std::time::Duration::from_secs(5))
        .write_stdin(payload.to_string())
        .assert()
}

// F3-1, D3-1: wrappers chained with eval are bounded by a work budget.
#[test]
fn wrapper_eval_chains_are_bounded() {
    let repo = initialized_repo();
    for unit in ["xargs eval ", "env eval ", "find . -exec eval "] {
        bash_within(repo.path(), &format!("{}true; git push", unit.repeat(26))).code(DENY);
    }
    bash_within(
        repo.path(),
        &format!("{}true; git push", "xargs eval ".repeat(4000)),
    )
    .code(DENY);
}

// AC-7 (F3-4, F3-6, E3-1, E3-5): review round 3.
#[test]
fn denies_git_push_round_three() {
    let repo = initialized_repo();
    for command in [
        "git submodule foreach git push",
        "git submodule foreach --recursive git push",
        "watch 'git push'",
        "parallel 'git push' ::: a",
        "sudo -i 'git push'",
        "echo push | xargs -I{} git {}",
        "git -calias.p=push p",
        "git rebase -ix 'git push' HEAD~1",
        "function f { git push; }; f",
        "git bisect run git push",
        "git bisect run sh -c 'git push'",
        "git -c core.pager='git push' log",
        "builtin command git push",
        "ash -c 'git push'",
        "busybox sh -c 'git push'",
        "git p$'ush'",
        "ssh localhost git push",
        "ssh localhost 'cd x && git push'",
        "tmux new-session -d 'git push'",
        "su -c 'git push' me",
        "script -qc 'git push' /dev/null",
        "flock /tmp/l git push",
        "strace -f git push",
        "systemd-run --user git push",
        "docker run img git push",
        "python3 -c \"import subprocess; subprocess.run(['git', 'push', '--no-verify'])\"",
        "python3 -c \"import os; os.system('git send-pack origin')\"",
        "perl -e 'system(\"git http-push x\")'",
    ] {
        bash(repo.path(), command)
            .code(DENY)
            .stderr(contains("git push"));
    }
}

// AC-13 (F3-2, F3-5, E3-4): more ways to switch off the second layer.
#[test]
fn denies_pre_push_bypass_round_three() {
    let repo = initialized_repo();
    for command in [
        "env -i bash -l",
        "env - sh",
        "env -i PATH=$PATH python3 x.py",
        "env --ignore-environment sh x.sh",
        "exec -c sh",
        "declare +x CLAUDECODE; python3 p.py",
        "typeset +x CLAUDECODE",
        "command unset CLAUDECODE",
        "builtin unset CLAUDECODE",
        "echo 'exit 0' > .git/hooks/pre-push",
        ": > .git/hooks/pre-push",
        "exec 3> .git/hooks/pre-push",
        "cd .git && cp x hooks/pre-push",
    ] {
        bash(repo.path(), command)
            .code(DENY)
            .stderr(contains("pre-push"));
    }
}

#[test]
fn allows_lookalikes_after_round_three() {
    let repo = initialized_repo();
    for command in [
        "echo \"CLAUDECODE=$CLAUDECODE\"",
        "printenv CLAUDECODE",
        "env RUST_LOG=debug cargo test",
        "cargo test -- --nocapture",
        "git status --short",
        "git rebase main",
        "git submodule update --init --recursive",
        "watch -n 5 'cargo check'",
    ] {
        bash(repo.path(), command).success();
    }
}

// AC-8 (F3-3, E3-2): git and Claude Code configuration is never written by the write tools.
#[test]
fn denies_git_and_claude_writes_even_with_build() {
    let repo = initialized_repo();
    let root = repo.path();
    set_state(root, "ABC-1", r#"{"build": {"allowed": true}}"#);
    let status = std::process::Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success());
    for rel in [
        ".git/hooks/pre-push",
        ".git/config",
        ".GIT/config",
        ".githooks/pre-push",
        ".claude/settings.local.json",
        ".claude/hooks/x.sh",
    ] {
        for tool in WRITE_TOOLS {
            write_call(root, tool, &root.join(rel).display().to_string())
                .code(DENY)
                .stderr(contains("only stapel"));
        }
    }
    write_call(
        root,
        "Write",
        &root.join("src/main.rs").display().to_string(),
    )
    .success();
}
