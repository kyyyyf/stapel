//! STP-31: `stapel init --local`.

mod common;

use common::{git_in, git_repo, read, snapshot, stapel};
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
