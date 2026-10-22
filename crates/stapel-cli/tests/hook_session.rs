//! STP-22 AC-13, AC-14: `stapel hook session-start`, the SessionStart hook command.

mod common;

use common::{bare_dir, git_repo, repo_with_ticket, stapel, stapel_repo};
use std::path::Path;

const LINE: &str =
    "This repository follows the stapel process: load the skill stapel and follow it.\n";

fn next_text(dir: &Path) -> String {
    let out = stapel(dir)
        .arg("next")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(out).unwrap()
}

// AC-13
#[test]
fn prints_the_skill_line_and_next() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let next = next_text(dir);
    assert!(next.starts_with("ticket: ABC-1"), "{next}");
    let expected = format!("{LINE}stapel next:\n{next}");

    // The hook ignores its stdin.
    let out = stapel(dir)
        .args(["hook", "session-start"])
        .write_stdin("{\"hook_event_name\": \"SessionStart\"}")
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(out).unwrap(), expected);

    // CLAUDE_PROJECT_DIR names the repository; the working folder is elsewhere.
    let away = bare_dir();
    let out = stapel(away.path())
        .env("CLAUDE_PROJECT_DIR", dir)
        .args(["hook", "session-start"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(out).unwrap(), expected);
}

// AC-13, when `next` fails
#[test]
fn next_fails() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["new", "Second"]).assert().success();
    stapel(dir).arg("next").assert().code(2);
    let out = stapel(dir)
        .args(["hook", "session-start"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(out).unwrap(), LINE);

    let repo = stapel_repo();
    let dir = repo.path();
    std::fs::write(dir.join(".stapel/stapel.toml"), "not [valid").unwrap();
    let out = stapel(dir)
        .args(["hook", "session-start"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(out).unwrap(), LINE);
}

// AC-13 (D4B-3): a stapel.toml that cannot be read gives only the first line.
#[test]
fn unreadable_config() {
    let repo = stapel_repo();
    let dir = repo.path();
    let toml = dir.join(".stapel/stapel.toml");
    let run = || {
        let out = stapel(dir)
            .args(["hook", "session-start"])
            .assert()
            .code(0)
            .get_output()
            .stdout
            .clone();
        String::from_utf8(out).unwrap()
    };

    // A file that is not UTF-8: only the first line.
    std::fs::write(&toml, b"[ticket]\nprefix = \"\xff\"\n").unwrap();
    assert_eq!(run(), LINE);

    // A folder at that path is no file: the hook is silent (AC-14).
    std::fs::remove_file(&toml).unwrap();
    std::fs::create_dir(&toml).unwrap();
    assert_eq!(run(), "");
}

// AC-14
#[test]
fn silent_outside_a_stapel_repository() {
    let plain = git_repo();
    let bare = bare_dir();
    for dir in [plain.path(), bare.path()] {
        let out = stapel(dir)
            .args(["hook", "session-start"])
            .assert()
            .code(0)
            .get_output()
            .stdout
            .clone();
        assert_eq!(String::from_utf8(out).unwrap(), "");
    }

    // CLAUDE_PROJECT_DIR without the file wins over a working folder that has it.
    let repo = repo_with_ticket();
    let out = stapel(repo.path())
        .env("CLAUDE_PROJECT_DIR", plain.path())
        .args(["hook", "session-start"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(out).unwrap(), "");
}
