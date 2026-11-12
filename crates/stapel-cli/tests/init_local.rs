//! STP-31: `stapel init --local`.

mod common;

use common::{git_repo, read, snapshot, stapel};
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
