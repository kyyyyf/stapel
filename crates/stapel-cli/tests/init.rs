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
