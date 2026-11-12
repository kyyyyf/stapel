//! STP-31: `stapel init --local`.

mod common;

use common::{git_config, git_in, git_repo, read, snapshot, stapel};
use predicates::str::contains;
use stapel_core::config::{Config, History};

fn history_of(dir: &std::path::Path) -> Option<History> {
    Config::parse(&read(dir, ".stapel/stapel.toml"))
        .unwrap()
        .repository
        .map(|r| r.history)
}

// STP-31 AC-1: the starter `stapel.toml` records the local mode.
#[test]
fn local_mode_is_recorded() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    let text = read(dir, ".stapel/stapel.toml");
    assert!(
        text.contains("[repository]\nhistory = \"local\"\n"),
        "{text}"
    );
    assert_eq!(history_of(dir), Some(History::Local));
}

// STP-31 AC-2: later runs, with or without the flag, keep the mode and change nothing.
#[test]
fn second_run_keeps_local_mode() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    let before = snapshot(dir);
    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("already set up"));
    assert_eq!(snapshot(dir), before);
    stapel(dir)
        .args(["init", "--local"])
        .assert()
        .success()
        .stdout(contains("already set up"));
    assert_eq!(snapshot(dir), before);
    assert_eq!(history_of(dir), Some(History::Local));
}

// STP-31 AC-3: `--local` on a shared `stapel.toml` is refused and writes nothing.
#[test]
fn local_flag_refused_on_a_shared_config() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let before = snapshot(dir);
    stapel(dir)
        .args(["init", "--local"])
        .assert()
        .code(1)
        .stderr(contains(
            ".stapel/stapel.toml is in shared mode; --local applies only to the first stapel init. \
             To switch, add [repository] with history = \"local\" to it, remove stapel's lines from \
             .gitignore and .gitattributes, and run stapel init",
        ));
    assert_eq!(snapshot(dir), before);
}

// STP-31 AC-5: the ignore and merge lines go to the git folder; `.gitignore` and `.gitattributes`
// stay as they are.
#[test]
fn lines_go_to_the_git_folder() {
    let repo = git_repo();
    let dir = repo.path();
    std::fs::write(dir.join(".gitignore"), "node_modules/\n").unwrap();
    git_in(dir, &["add", ".gitignore"]);
    git_in(
        dir,
        &[
            "-c",
            "user.email=a@b.c",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "ignore",
        ],
    );
    // The exclude file has no final newline and the info folder is missing.
    let _ = std::fs::remove_dir_all(dir.join(".git/info"));
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success()
        .stdout(contains(".git/info/exclude"))
        .stdout(contains(".git/info/attributes"));
    assert_eq!(read(dir, ".gitignore"), "node_modules/\n");
    assert!(!dir.join(".gitattributes").exists());
    let exclude = read(dir, ".git/info/exclude");
    assert!(exclude.contains("/.stapel/index/\n"), "{exclude}");
    assert!(exclude.contains("/.claude/worktrees/\n"), "{exclude}");
    let attributes = read(dir, ".git/info/attributes");
    assert!(
        attributes.contains(".stapel/process-ledger.jsonl merge=union\n"),
        "{attributes}"
    );
    assert!(
        attributes.contains(".stapel/sessions.jsonl merge=union\n"),
        "{attributes}"
    );
    let status = git_in(dir, &["status", "--porcelain"]);
    assert!(!status.contains(".gitignore"), "{status}");
    assert!(!status.contains(".gitattributes"), "{status}");

    // A second run adds no line twice.
    let before = (exclude.clone(), attributes.clone());
    stapel(dir).arg("init").assert().success();
    assert_eq!(
        (
            read(dir, ".git/info/exclude"),
            read(dir, ".git/info/attributes")
        ),
        before
    );

    // An exclude file without a final newline gets a newline first.
    std::fs::write(dir.join(".git/info/exclude"), "keep-me").unwrap();
    stapel(dir).arg("init").assert().success();
    let exclude = read(dir, ".git/info/exclude");
    assert!(
        exclude.starts_with("keep-me\n/.stapel/index/\n"),
        "{exclude}"
    );

    // A linked worktree uses the shared files.
    git_in(dir, &["add", "-A"]);
    git_in(
        dir,
        &[
            "-c",
            "user.email=a@b.c",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "first",
        ],
    );
    let linked = tempfile::tempdir().unwrap();
    let wt = linked.path().join("wt");
    git_in(dir, &["worktree", "add", "-q", wt.to_str().unwrap()]);
    let shared = read(dir, ".git/info/exclude");
    stapel(&wt).arg("init").assert().success();
    assert_eq!(read(dir, ".git/info/exclude"), shared);
    assert!(!wt.join(".gitattributes").exists());
}

// STP-31 AC-6: a file in the git folder that is not UTF-8 is refused and nothing is written.
#[test]
fn refuses_a_non_utf8_exclude_file() {
    for name in ["exclude", "attributes"] {
        let repo = git_repo();
        let dir = repo.path();
        stapel(dir)
            .args(["init", "--local", "--prefix", "ABC"])
            .assert()
            .success();
        let file = dir.join(".git/info").join(name);
        std::fs::write(&file, b"abc\xff\n").unwrap();
        let _ = std::fs::remove_file(dir.join(".stapel/allowlist.toml"));
        let before = snapshot(dir);
        stapel(dir)
            .arg("init")
            .assert()
            .code(1)
            .stderr(contains(format!(".git/info/{name}")));
        assert_eq!(snapshot(dir), before);
        assert_eq!(std::fs::read(&file).unwrap(), b"abc\xff\n");
    }
}

fn commit_all(dir: &std::path::Path, message: &str) -> String {
    git_in(dir, &["add", "-A"]);
    git_in(
        dir,
        &[
            "-c",
            "user.email=a@b.c",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            message,
        ],
    );
    git_in(dir, &["rev-parse", "HEAD"]).trim().to_string()
}

fn make_local(dir: &std::path::Path) {
    let path = dir.join(".stapel/stapel.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n[repository]\nhistory = \"local\"\n");
    std::fs::write(&path, text).unwrap();
}

// STP-31 AC-7: a stapel pre-push inside the work tree goes to the exclude file.
#[test]
fn pre_push_in_a_tracked_hooks_folder() {
    // A relative hooks folder with a tracked hook.
    let repo = git_repo();
    let dir = repo.path();
    git_config(dir, "core.hooksPath", ".husky");
    std::fs::create_dir_all(dir.join(".husky")).unwrap();
    std::fs::write(dir.join(".husky/pre-commit"), "#!/bin/sh\nexit 0\n").unwrap();
    commit_all(dir, "husky");
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    assert!(dir.join(".husky/pre-push").exists());
    let exclude = read(dir, ".git/info/exclude");
    assert!(exclude.contains("/.husky/pre-push\n"), "{exclude}");
    let status = git_in(dir, &["status", "--porcelain", "--", ".husky"]);
    assert_eq!(status, "");
    // A second run adds nothing.
    stapel(dir).arg("init").assert().success();
    assert_eq!(read(dir, ".git/info/exclude"), exclude);

    // An absolute hooks folder inside the work tree.
    let repo = git_repo();
    let dir = repo.path();
    let hooks = dir.canonicalize().unwrap().join("hooks-here");
    git_config(dir, "core.hooksPath", hooks.to_str().unwrap());
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    assert!(hooks.join("pre-push").exists());
    let exclude = read(dir, ".git/info/exclude");
    assert!(exclude.contains("/hooks-here/pre-push\n"), "{exclude}");

    // The default folder inside the git folder: no line.
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    let exclude = read(dir, ".git/info/exclude");
    assert!(!exclude.contains("pre-push"), "{exclude}");

    // A hooks folder outside the work tree: the current warning, no line.
    let repo = git_repo();
    let dir = repo.path();
    let outside = tempfile::tempdir().unwrap();
    git_config(dir, "core.hooksPath", outside.path().to_str().unwrap());
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success()
        .stderr(contains("is outside the repository"));
    let exclude = read(dir, ".git/info/exclude");
    assert!(!exclude.contains("pre-push"), "{exclude}");

    // A hook that is not stapel's stays and gets no line.
    let repo = git_repo();
    let dir = repo.path();
    git_config(dir, "core.hooksPath", ".husky");
    std::fs::create_dir_all(dir.join(".husky")).unwrap();
    std::fs::write(dir.join(".husky/pre-push"), "#!/bin/sh\nexit 0\n").unwrap();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success()
        .stderr(contains("not from stapel"));
    let exclude = read(dir, ".git/info/exclude");
    assert!(!exclude.contains("pre-push"), "{exclude}");

    // A stapel hook from a shared install, then the local mode: the line is added.
    let repo = git_repo();
    let dir = repo.path();
    git_config(dir, "core.hooksPath", ".husky");
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    assert!(dir.join(".husky/pre-push").exists());
    make_local(dir);
    stapel(dir).arg("init").assert().success();
    let exclude = read(dir, ".git/info/exclude");
    assert!(exclude.contains("/.husky/pre-push\n"), "{exclude}");
}

fn run_hook(dir: &std::path::Path, input: &str, agent: bool) -> std::process::Output {
    use std::io::Write;
    let mut cmd = std::process::Command::new(dir.join(".git/hooks/pre-push"));
    cmd.current_dir(dir)
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if agent {
        cmd.env("CLAUDECODE", "1");
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

// STP-31 AC-8: the local pre-push refuses new commits that carry `.stapel/`.
#[test]
fn pre_push_refuses_process_files() {
    let repo = git_repo();
    let dir = repo.path();
    std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    let plain = commit_all(dir, "plain");
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    let with = commit_all(dir, "process files");
    git_in(dir, &["rm", "-r", "-q", "--cached", ".stapel"]);
    git_in(
        dir,
        &[
            "-c",
            "user.email=a@b.c",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "drop process files",
        ],
    );
    let without = git_in(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let zero = "0".repeat(40);
    let line = |sha: &str| format!("refs/heads/feat {sha} refs/heads/feat {zero}\n");
    let code = |out: &std::process::Output| out.status.code();

    let out = run_hook(dir, &line(&plain), false);
    assert_eq!(code(&out), Some(0));
    let out = run_hook(dir, &line(&with), false);
    assert_eq!(code(&out), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.contains(
            "stapel: refs/heads/feat carries .stapel/; push the branch that stapel export writes"
        ),
        "{stderr}"
    );
    // A tip that removes `.stapel/` on top of a commit that has it, neither on a remote.
    assert_eq!(code(&run_hook(dir, &line(&without), false)), Some(1));
    // Two lines, the second carrying `.stapel/`.
    let two = format!("{}{}", line(&plain), line(&with));
    assert_eq!(code(&run_hook(dir, &two, false)), Some(1));
    // A line of three words is skipped; an all-zero local sha; empty input.
    let three = format!("refs/heads/feat {with} refs/heads/feat\n");
    assert_eq!(code(&run_hook(dir, &three, false)), Some(0));
    let deleted = format!("(delete) {zero} refs/heads/feat {with}\n");
    assert_eq!(code(&run_hook(dir, &deleted, false)), Some(0));
    assert_eq!(code(&run_hook(dir, "", false)), Some(0));
    // The agent refusal comes first.
    let out = run_hook(dir, &line(&plain), true);
    assert_eq!(code(&out), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("agent session"));
    // The same history already on the remote's ref.
    git_in(dir, &["update-ref", "refs/remotes/origin/feat", &without]);
    assert_eq!(code(&run_hook(dir, &line(&without), false)), Some(0));
}

// STP-31 AC-9: a stapel pre-push gets the text of the run's mode; a foreign one stays.
#[test]
fn pre_push_text_follows_the_mode() {
    use std::os::unix::fs::PermissionsExt;
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let hook = dir.join(".git/hooks/pre-push");
    let shared_text = std::fs::read_to_string(&hook).unwrap();

    // To the local mode: the hook is updated and stays executable.
    make_local(dir);
    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("updated: .git/hooks/pre-push (pre-push"));
    let local_text = std::fs::read_to_string(&hook).unwrap();
    assert_ne!(local_text, shared_text);
    assert_ne!(hook.metadata().unwrap().permissions().mode() & 0o111, 0);
    // A second run changes nothing.
    stapel(dir).arg("init").assert().success();
    assert_eq!(std::fs::read_to_string(&hook).unwrap(), local_text);

    // Back to the shared mode: the hook is updated again.
    let config = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        text.replace("history = \"local\"", "history = \"shared\""),
    )
    .unwrap();
    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("updated: .git/hooks/pre-push (pre-push"));
    assert_eq!(std::fs::read_to_string(&hook).unwrap(), shared_text);
    assert_ne!(hook.metadata().unwrap().permissions().mode() & 0o111, 0);

    // A foreign hook is kept with the warning.
    let repo = git_repo();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".git/hooks")).unwrap();
    std::fs::write(dir.join(".git/hooks/pre-push"), "#!/bin/sh\nexit 0\n").unwrap();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success()
        .stderr(contains("not from stapel"));
    assert_eq!(read(dir, ".git/hooks/pre-push"), "#!/bin/sh\nexit 0\n");
}
