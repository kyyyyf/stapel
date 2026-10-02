//! STP-1: `stapel init`.

mod common;

use common::{bare_dir, git_repo, read, stapel};
use predicates::str::contains;
use stapel_core::config::Config;

fn key_of(dir: &std::path::Path) -> String {
    Config::parse(&read(dir, ".stapel/stapel.toml")).unwrap().tickets.key
}

// AC-2
#[test]
fn creates_layout_in_empty_repo() {
    let repo = git_repo();
    let dir = repo.path();

    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success()
        .stdout(contains(".stapel/stapel.toml"))
        .stdout(contains(".stapel/allowlist.toml"));

    assert!(dir.join(".stapel/stapel.toml").is_file());
    assert!(dir.join(".stapel/allowlist.toml").is_file());
    assert!(dir.join(".stapel/tickets").is_dir());
    assert!(read(dir, ".gitignore").lines().any(|l| l == "/.stapel/index/"));
}

// AC-2: the repository root is found from a subdirectory.
#[test]
fn creates_layout_at_repo_root_from_subdir() {
    let repo = git_repo();
    let sub = repo.path().join("src/deep");
    std::fs::create_dir_all(&sub).unwrap();

    stapel(&sub).args(["init", "--prefix", "ABC"]).assert().success();

    assert!(repo.path().join(".stapel/stapel.toml").is_file());
    assert!(!sub.join(".stapel").exists());
}

// AC-2: an existing .gitignore keeps its lines.
#[test]
fn appends_to_existing_gitignore() {
    let repo = git_repo();
    std::fs::write(repo.path().join(".gitignore"), "/target").unwrap();

    stapel(repo.path()).args(["init", "--prefix", "ABC"]).assert().success();

    assert_eq!(read(repo.path(), ".gitignore"), "/target\n/.stapel/index/\n");
}

// AC-11
#[test]
fn refuses_outside_git() {
    let dir = bare_dir();

    stapel(dir.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .code(1)
        .stderr(contains("не git-репозиторий"));

    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

// AC-12
#[test]
fn prefix_from_flag() {
    let repo = git_repo();
    stapel(repo.path()).args(["init", "--prefix", "ABC"]).assert().success();
    assert_eq!(key_of(repo.path()), "ABC-{n}");
}

// AC-12
#[test]
fn prefix_from_prompt() {
    let repo = git_repo();
    stapel(repo.path())
        .arg("init")
        .env("STAPEL_ASSUME_TTY", "1")
        .write_stdin("QA\n")
        .assert()
        .success()
        .stdout(contains("Префикс ключа"));
    assert_eq!(key_of(repo.path()), "QA-{n}");
}

// AC-12
#[test]
fn rejects_bad_prefix() {
    for bad in ["A", "ABCDEFGHI", "ab", "A1", "АБ", ""] {
        let repo = git_repo();
        stapel(repo.path())
            .args(["init", "--prefix", bad])
            .assert()
            .code(1)
            .stderr(contains("префикс"));
        assert!(!repo.path().join(".stapel").exists(), "created for {bad:?}");
    }
}

// AC-12
#[test]
fn refuses_without_prefix_noninteractive() {
    let repo = git_repo();
    stapel(repo.path())
        .arg("init")
        .assert()
        .code(1)
        .stderr(contains("--prefix"));
    assert!(!repo.path().join(".stapel").exists());
}

// AC-4, R-3; AC-12: the second run does not ask for a prefix.
#[test]
fn second_run_changes_nothing() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir).args(["init", "--prefix", "ABC"]).assert().success();
    let before = common::snapshot(dir);
    std::thread::sleep(std::time::Duration::from_millis(50));

    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("уже готово"));

    assert_eq!(common::snapshot(dir), before);
}

// AC-5
#[test]
fn keeps_user_edited_config() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir).args(["init", "--prefix", "ABC"]).assert().success();

    let edited = read(dir, ".stapel/stapel.toml").replace("claude-haiku-4-5", "my-cheap-model")
        + "\n# hand edit\n";
    std::fs::write(dir.join(".stapel/stapel.toml"), &edited).unwrap();
    std::fs::remove_file(dir.join(".stapel/allowlist.toml")).unwrap();

    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("создано: .stapel/allowlist.toml"));

    assert_eq!(read(dir, ".stapel/stapel.toml"), edited);
    assert!(dir.join(".stapel/allowlist.toml").is_file());
}

// AC-5: a prefix given again does not rewrite an existing config.
#[test]
fn second_run_with_other_prefix_keeps_key() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir).args(["init", "--prefix", "ABC"]).assert().success();

    stapel(dir)
        .args(["init", "--prefix", "XYZ"])
        .assert()
        .success()
        .stdout(contains("ABC-{n}"));

    assert_eq!(key_of(dir), "ABC-{n}");
}
