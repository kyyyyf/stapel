//! STP-1: `stapel hook pre-tool-use`, the Claude Code PreToolUse guard.

mod common;

use common::{bare_dir, git_repo, stapel};
use predicates::prelude::*;
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

// E4-1: env and exec options are read only before the program, so its own flags are not theirs.
#[test]
fn allows_lookalikes_after_round_four() {
    let repo = initialized_repo();
    for command in [
        "env RUST_LOG=debug grep -i foo file",
        "env FOO=1 sed -i 's/a/b/' file",
        "env FOO=1 ls -li",
        "env FOO=1 grep -rin foo .",
        "sudo env FOO=1 grep -i x f",
        "exec cargo test --color always",
        "exec cargo test -- --nocapture",
        "env FOO=1 printenv CLAUDECODE",
    ] {
        bash(repo.path(), command).success();
    }
}

// E4-2, E4-3: the global git config and where git looks for it are part of the second layer.
#[test]
fn denies_pre_push_bypass_round_four() {
    let repo = initialized_repo();
    for command in [
        "env --ignore-e python3 x.py",
        "GIT_CONFIG_GLOBAL=/tmp/g python3 x.py",
        "GIT_CONFIG_SYSTEM=/tmp/g sh p.sh",
        "XDG_CONFIG_HOME=/tmp/x sh p.sh",
        "HOME=/tmp/h python3 x.py",
        "export HOME=/tmp/h",
        "GIT_DIR=/tmp/other/.git sh p.sh",
    ] {
        bash(repo.path(), command)
            .code(DENY)
            .stderr(contains("pre-push"));
    }
}

// E4-2: write tools may not change the user's global git or Claude Code configuration.
#[test]
fn denies_global_config_writes() {
    let repo = initialized_repo();
    let home = bare_dir();
    let xdg = bare_dir();
    for path in [
        home.path().join(".gitconfig"),
        home.path().join(".config/git/config"),
        xdg.path().join("git/config"),
        home.path().join(".claude/settings.json"),
        home.path().join(".claude/settings.local.json"),
    ] {
        let payload = json!({
            "tool_name": "Write",
            "cwd": repo.path(),
            "tool_input": { "file_path": path, "content": "x" },
        });
        stapel(repo.path())
            .args(["hook", "pre-tool-use"])
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", xdg.path())
            .write_stdin(payload.to_string())
            .assert()
            .code(DENY)
            .stderr(contains("only stapel"));
    }
}

// E4-6: a Bash command that is not a string is unparsable input.
#[test]
fn rejects_non_string_command() {
    let repo = initialized_repo();
    hook(repo.path(), "Bash", json!({ "command": ["git", "x"] }))
        .code(DENY)
        .stderr(contains("hook input"));
}

// ---- STP-2 AC-11: the permit is computed from confirmations ----

fn confirmed_repo() -> tempfile::TempDir {
    let repo = common::repo_with_ticket();
    for s in ["spec", "design", "proof"] {
        stapel(repo.path()).args(["ok", s]).assert().success();
    }
    repo
}

fn code_write(dir: &Path) -> assert_cmd::assert::Assert {
    write_call(dir, "Write", &dir.join("src/x.rs").display().to_string())
}

#[test]
fn allows_code_write_with_fresh_confirmations() {
    let repo = confirmed_repo();
    code_write(repo.path()).success();
}

#[test]
fn denies_code_write_after_spec_edit() {
    let repo = confirmed_repo();
    let dir = repo.path();
    common::set_section(dir, "ABC-1", "Spec", "The spec, edited.");
    code_write(dir)
        .code(DENY)
        .stderr(contains("not confirmed"))
        .stderr(contains("stapel status"))
        .stderr(predicates::str::contains("stapel ok").not());
}

#[test]
fn closed_ticket_gives_no_permit() {
    let repo = confirmed_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["close", "--reason", "merged"])
        .assert()
        .success();
    code_write(dir).code(DENY);
}

#[test]
fn legacy_build_flag_still_honoured() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    set_state(dir, "ABC-9", r#"{"key":"ABC-9","build":{"allowed":true}}"#);
    code_write(dir).success();
}

#[test]
fn closed_ticket_flag_not_honoured() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    let mut state = common::state_json(dir, "ABC-1");
    state["build"] = json!({"allowed": true});
    state["closed"] = json!({"by": "x", "at": "2026-10-02T00:00:00Z", "reason": "done"});
    set_state(dir, "ABC-1", &state.to_string());
    code_write(dir).code(DENY);
}

#[test]
fn invalid_config_gives_no_computed_permit() {
    let repo = confirmed_repo();
    let dir = repo.path();
    let path = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("id = \"plan\"", "id = \"spec\"");
    std::fs::write(&path, text).unwrap();
    code_write(dir).code(DENY);
    // The phase-0 hand flag works even then.
    set_state(dir, "ABC-9", r#"{"key":"ABC-9","build":{"allowed":true}}"#);
    code_write(dir).success();
}

#[test]
fn unreadable_ticket_gives_no_permit() {
    let repo = confirmed_repo();
    let dir = repo.path();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/ticket.md"),
        b"## Spec\n\xff\n",
    )
    .unwrap();
    code_write(dir).code(DENY);
}

fn pad_summary(dir: &Path, bytes: usize) {
    let path = dir.join(".stapel/tickets/ABC-1/ticket.md");
    let mut text = std::fs::read_to_string(&path).unwrap();
    let line = "padding line in the summary section, not a required one\n";
    while text.len() + line.len() < bytes {
        text.push_str(line);
    }
    std::fs::write(&path, text).unwrap();
}

#[test]
fn oversized_ticket_gives_no_permit() {
    let repo = confirmed_repo();
    let dir = repo.path();
    pad_summary(dir, 4 * 1024 * 1024 + 4096);
    code_write(dir).code(DENY);
}

#[test]
fn permit_check_on_4mib_ticket_is_fast() {
    let repo = confirmed_repo();
    let dir = repo.path();
    pad_summary(dir, 4 * 1024 * 1024 - 4096);
    let started = std::time::Instant::now();
    code_write(dir).success();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn non_regular_file_gives_no_permit() {
    let repo = confirmed_repo();
    let dir = repo.path();
    let ticket = dir.join(".stapel/tickets/ABC-1/ticket.md");
    std::fs::remove_file(&ticket).unwrap();
    let status = std::process::Command::new("mkfifo")
        .arg(&ticket)
        .status()
        .unwrap();
    assert!(status.success());
    let payload = json!({
        "tool_name": "Write",
        "cwd": dir,
        "tool_input": { "file_path": dir.join("src/x.rs"), "content": "x" },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .timeout(std::time::Duration::from_secs(5))
        .write_stdin(payload.to_string())
        .assert()
        .code(DENY);
}

// ---- STP-2 AC-15..AC-17: the permission dialog and grants ----

use common::{ask, grants_dir, hook_bash, repo_with_ticket, token_of};

#[test]
fn asks_for_plain_stapel_ok() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for command in [
        "stapel ok spec",
        "FOO=1 stapel ok abc-1 spec 2>&1",
        "/usr/local/bin/stapel OK spec",
    ] {
        let (reason, replaced) = ask(dir, command);
        assert!(
            reason.contains("ABC-1") && reason.contains("spec") && reason.contains("sha256:"),
            "{reason}"
        );
        assert!(
            replaced.ends_with(&format!("ok ABC-1 spec --grant {}", token_of(&replaced))),
            "{replaced}"
        );
        assert!(
            !replaced.contains("FOO=1") && !replaced.contains("2>&1"),
            "{replaced}"
        );
    }
}

#[test]
fn asks_for_cargo_run_ok() {
    let repo = repo_with_ticket();
    let (_, replaced) = ask(repo.path(), "cargo run -q -p stapel-cli -- ok spec");
    assert!(
        replaced.starts_with("cargo run -q -p stapel-cli -- ok ABC-1 spec --grant "),
        "{replaced}"
    );
}

#[test]
fn denies_ok_in_bypass_mode() {
    let repo = repo_with_ticket();
    for mode in ["bypassPermissions", "dontAsk", ""] {
        hook_bash(repo.path(), "stapel ok spec", mode)
            .code(DENY)
            .stderr(contains("dialog"));
    }
    assert!(
        !grants_dir(repo.path()).exists()
            || std::fs::read_dir(grants_dir(repo.path())).unwrap().count() == 0
    );
}

#[test]
fn denies_command_with_own_grant() {
    let repo = repo_with_ticket();
    for command in [
        "stapel ok spec --grant abc",
        "stapel ok --grant=abc spec",
        "sh -c 'stapel ok spec --grant x'",
    ] {
        hook_bash(repo.path(), command, "auto")
            .code(DENY)
            .stderr(contains("grant"));
    }
}

#[test]
fn denies_unresolvable_ok() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    hook_bash(dir, "stapel ok review", "auto")
        .code(DENY)
        .stderr(contains("cannot be confirmed"));
    hook_bash(dir, "stapel ok ABC-7 spec", "auto")
        .code(DENY)
        .stderr(contains("ABC-7"));
    hook_bash(dir, "stapel close", "auto")
        .code(DENY)
        .stderr(contains("reason"));
    common::git_config(dir, "user.name", " ");
    hook_bash(dir, "stapel ok spec", "auto")
        .code(DENY)
        .stderr(contains("user.name"));
}

#[test]
fn allows_mentions_of_stapel_ok() {
    let repo = repo_with_ticket();
    for command in [
        "echo \"stapel ok\"",
        "git commit -m 'STP-2 step 6: stapel ok refuses in agent shell'",
        "grep -rn \"stapel ok\" crates",
        "cargo test ok::records_confirmation",
        "grep -rn -- --grant crates/",
        "echo --grant-me",
    ] {
        let out = hook_bash(repo.path(), command, "auto")
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(
            out.is_empty(),
            "{command}: {}",
            String::from_utf8_lossy(&out)
        );
    }
}

#[test]
fn allows_stapel_status_and_new_from_bash() {
    let repo = repo_with_ticket();
    for command in [
        "stapel status",
        "stapel new 'Another'",
        "stapel status ABC-1",
    ] {
        let out = hook_bash(repo.path(), command, "auto")
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(out.is_empty(), "{command}");
    }
}

#[test]
fn ask_carries_grant_and_reason() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let (reason, replaced) = ask(dir, "stapel ok spec");
    assert!(reason.contains("1 line"), "{reason}");
    assert!(reason.contains("not in HEAD"), "{reason}");
    assert!(reason.contains("don't ask again"), "{reason}");
    let files: Vec<_> = std::fs::read_dir(grants_dir(dir)).unwrap().collect();
    assert_eq!(files.len(), 1);
    assert!(!token_of(&replaced).is_empty());
    // The token is not the file name.
    let name = files[0]
        .as_ref()
        .unwrap()
        .file_name()
        .to_string_lossy()
        .into_owned();
    assert!(!name.contains(&token_of(&replaced)));
}

#[test]
fn sweeps_expired_grants() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::create_dir_all(grants_dir(dir)).unwrap();
    std::fs::write(grants_dir(dir).join("0000.json"), r#"{"expires_at":0}"#).unwrap();
    ask(dir, "stapel ok spec");
    assert!(!grants_dir(dir).join("0000.json").exists());
}

#[test]
fn denies_when_allow_rule_exists() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::write(
        dir.join(".claude/settings.local.json"),
        r#"{"permissions":{"allow":["Bash(stapel ok:*)"]}}"#,
    )
    .unwrap();
    hook_bash(dir, "stapel ok spec", "auto")
        .code(DENY)
        .stderr(contains("allow rule"));
}

#[test]
fn denies_write_tools_on_grants() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    write_call(
        dir,
        "Write",
        &grants_dir(dir).join("x.json").display().to_string(),
    )
    .code(DENY);
}

#[test]
fn denies_terminal_input_injection() {
    let repo = repo_with_ticket();
    for command in [
        "tmux send-keys -t \"$TMUX_PANE\" Enter",
        "(sleep 5; tmux send -t 0 Enter) &",
        "screen -X stuff $'\\n'",
        "xdotool key Return",
        "ydotool key 28:1",
        "wtype -k Return",
        "python3 -c 'import fcntl, termios; fcntl.ioctl(0, termios.TIOCSTI, b\"y\")'",
        "printf y > /dev/pts/3",
        "echo y > /dev/tty",
    ] {
        hook_bash(repo.path(), command, "auto")
            .code(DENY)
            .stderr(contains("person"));
    }
}

#[test]
fn allows_ordinary_tmux_use() {
    let repo = repo_with_ticket();
    for command in [
        "tmux ls",
        "tmux new -d -s work",
        "screen -ls",
        "tty",
        "ls /dev/pts",
    ] {
        hook_bash(repo.path(), command, "auto").success();
    }
}

// ---- STP-2 code review round 1 ----

fn bash_bounded(dir: &Path, command: &str) -> assert_cmd::assert::Assert {
    let payload = json!({
        "tool_name": "Bash",
        "cwd": dir,
        "permission_mode": "auto",
        "tool_input": { "command": command },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .timeout(std::time::Duration::from_secs(5))
        .write_stdin(payload.to_string())
        .assert()
}

// F-1: an agent may not run the hook entry point itself and mint a grant.
#[test]
fn denies_running_the_hook_from_bash() {
    let repo = repo_with_ticket();
    for command in [
        "stapel hook pre-tool-use < payload.json",
        "echo '{}' | stapel hook pre-tool-use",
        "sh -c 'stapel hook pre-tool-use'",
        "cargo run -q -p stapel-cli -- hook pre-tool-use",
        "/home/x/.cargo/bin/stapel hook pre-tool-use",
    ] {
        bash_bounded(repo.path(), command)
            .code(DENY)
            .stderr(contains("hook"));
    }
}

// F-2: a stapel ok or close whose arguments are built at run time is denied.
#[test]
fn denies_stapel_ok_with_run_time_words() {
    let repo = repo_with_ticket();
    for command in [
        "a=--gr; b=ant; stapel ok ABC-1 spec \"$a$b\" T",
        "stapel ok ABC-1 $S",
        "stapel close ABC-1 --reason x `echo --grant` y",
    ] {
        bash_bounded(repo.path(), command).code(DENY);
    }
}

// F-3, E-1: a special or oversized state.json gives no permit and does not hang the guard.
#[cfg(unix)]
#[test]
fn special_state_json_gives_no_permit_quickly() {
    let repo = confirmed_repo();
    let dir = repo.path();
    let other = dir.join(".stapel/tickets/ABC-7");
    std::fs::create_dir_all(&other).unwrap();
    let status = std::process::Command::new("mkfifo")
        .arg(other.join("state.json"))
        .status()
        .unwrap();
    assert!(status.success());
    let payload = json!({
        "tool_name": "Write",
        "cwd": dir,
        "tool_input": { "file_path": dir.join("src/x.rs"), "content": "x" },
    });
    // ABC-1 is confirmed and still grants; the FIFO beside it must not stall the check.
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .timeout(std::time::Duration::from_secs(5))
        .write_stdin(payload.to_string())
        .assert()
        .success();
    std::fs::remove_file(other.join("state.json")).unwrap();
    std::os::unix::fs::symlink("/dev/zero", other.join("state.json")).unwrap();
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .timeout(std::time::Duration::from_secs(5))
        .write_stdin(payload.to_string())
        .assert()
        .success();
    stapel(dir)
        .args(["status", "ABC-7"])
        .timeout(std::time::Duration::from_secs(5))
        .assert()
        .code(1);
}

// F-4, E-3: allow rules that would skip the dialog are found, unrelated ones are not.
#[test]
fn allow_rules_are_matched_like_claude_code() {
    for rule in [
        "Bash",
        "Bash(*)",
        "Bash(stapel:*)",
        "Bash(*stapel*)",
        "Bash(stapel ok:*)",
    ] {
        let repo = repo_with_ticket();
        let dir = repo.path();
        std::fs::write(
            dir.join(".claude/settings.local.json"),
            json!({"permissions": {"allow": [rule]}}).to_string(),
        )
        .unwrap();
        hook_bash(dir, "stapel ok spec", "auto")
            .code(DENY)
            .stderr(contains("allow rule"));
    }
    for rule in ["Bash(stapel status:*)", "Bash(cargo test:*)", "Read"] {
        let repo = repo_with_ticket();
        let dir = repo.path();
        std::fs::write(
            dir.join(".claude/settings.local.json"),
            json!({"permissions": {"allow": [rule]}}).to_string(),
        )
        .unwrap();
        ask(dir, "stapel ok spec");
    }
}

#[test]
fn allow_rule_in_user_settings_is_found() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".claude")).unwrap();
    std::fs::write(
        home.path().join(".claude/settings.json"),
        r#"{"permissions":{"allow":["Bash(stapel ok:*)"]}}"#,
    )
    .unwrap();
    let payload = json!({
        "tool_name": "Bash",
        "cwd": dir,
        "permission_mode": "auto",
        "tool_input": { "command": "stapel ok spec" },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .env("HOME", home.path())
        .write_stdin(payload.to_string())
        .assert()
        .code(DENY)
        .stderr(contains("allow rule"));
}

// F-5: the denial names the tickets and their unconfirmed or stale sections.
#[test]
fn denial_names_tickets_and_sections() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    code_write(dir)
        .code(DENY)
        .stderr(contains("ABC-1"))
        .stderr(contains("design not confirmed"))
        .stderr(contains("proof not confirmed"));
    common::set_section(dir, "ABC-1", "Spec", "edited");
    code_write(dir).code(DENY).stderr(contains("spec stale"));
}

// E-2: other spellings of the listed injection tools.
#[test]
fn denies_injection_spellings() {
    let repo = repo_with_ticket();
    for command in [
        "tmux send-k -t 0 y Enter",
        "tmux set-buffer y; tmux paste-buffer -t 0",
        "tmux pasteb -t 0",
        "tmux load-buffer x",
        "python3 -c 'import fcntl; fcntl.ioctl(0, 0x5412, b\"y\")'",
    ] {
        bash_bounded(repo.path(), command)
            .code(DENY)
            .stderr(contains("person"));
    }
}

// E-5: the replaced command runs the installed stapel, and the dialog names it.
#[test]
fn replaced_command_runs_installed_stapel() {
    let repo = repo_with_ticket();
    let (reason, replaced) = ask(repo.path(), "./nope/stapel ok spec");
    assert!(
        replaced.starts_with("stapel ok ABC-1 spec --grant "),
        "{replaced}"
    );
    assert!(reason.contains("runs: stapel"), "{reason}");
}

// F-9: a reason starting with '-' survives the replaced command.
#[test]
fn close_reason_with_dash_round_trips() {
    let repo = repo_with_ticket();
    let (_, replaced) = ask(repo.path(), "stapel close --reason=-x");
    assert!(replaced.contains("--reason=-x"), "{replaced}");
}

// ---- STP-2 code review round 2: the second layer for the dialog ----

// F2-1, F2-2: a hook process started from an agent's shell never issues a grant, whatever the
// command text looked like (cargo aliases, computed program words, copies of the binary).
#[test]
fn grant_needs_the_claude_code_hook_process() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let payload = json!({
        "tool_name": "Bash",
        "cwd": dir,
        "permission_mode": "auto",
        "tool_input": { "command": "stapel ok spec" },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .env("CLAUDE_CODE_EXECPATH", "/usr/local/bin/claude")
        .write_stdin(payload.to_string())
        .assert()
        .code(DENY)
        .stderr(contains("agent"));
    assert!(!grants_dir(dir).exists() || std::fs::read_dir(grants_dir(dir)).unwrap().count() == 0);
}

// Removing the variable the second layer relies on is a bypass, like removing CLAUDECODE.
#[test]
fn removing_claude_variables_is_a_bypass() {
    let repo = repo_with_ticket();
    for command in [
        "unset CLAUDE_CODE_EXECPATH",
        "env -u CLAUDE_CODE_EXECPATH sh x.sh",
        "CLAUDE_CODE_EXECPATH= sh x.sh",
        "export CLAUDE_CODE_EXECPATH=",
    ] {
        hook_bash(repo.path(), command, "auto")
            .code(DENY)
            .stderr(contains("second layer"));
    }
}

// F2-3: the prefix form of a rule needs a word boundary, and rules see the grant tail.
#[test]
fn allow_rule_prefix_needs_word_boundary() {
    for (rule, command, denied) in [
        ("Bash(s:*)", "stapel ok spec", false),
        ("Bash(sta:*)", "stapel ok spec", false),
        ("Bash(stapel c:*)", "stapel close --reason x", false),
        (
            "Bash(stapel ok ABC-1 spec --grant *)",
            "stapel ok spec",
            true,
        ),
        ("Bash(stapel:*)", "stapel ok spec", true),
    ] {
        let repo = repo_with_ticket();
        let dir = repo.path();
        std::fs::write(
            dir.join(".claude/settings.local.json"),
            json!({"permissions": {"allow": [rule]}}).to_string(),
        )
        .unwrap();
        if denied {
            hook_bash(dir, command, "auto")
                .code(DENY)
                .stderr(contains("allow rule"));
        } else {
            ask(dir, command);
        }
    }
}
