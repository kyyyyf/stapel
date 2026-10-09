//! STP-2 AC-1..AC-3: `stapel new`.

mod common;

use common::{git_repo, read, stapel, stapel_repo, state_json};
use predicates::str::contains;
use std::path::Path;

fn tickets(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join(".stapel/tickets"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

#[test]
fn creates_ticket_files() {
    let repo = stapel_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["new", "First ticket"])
        .assert()
        .success()
        .stdout(contains("created: ABC-1"));

    let ticket = read(dir, ".stapel/tickets/ABC-1/ticket.md");
    let headings: Vec<&str> = ticket.lines().filter(|l| l.starts_with("## ")).collect();
    assert_eq!(
        headings,
        [
            "## Description",
            "## Spec",
            "## Design",
            "## Proof",
            "## Plan",
            "## Review",
            "## Summary"
        ]
    );
    for heading in headings {
        let after = ticket.split(&format!("{heading}\n")).nth(1).unwrap();
        assert_eq!(
            after.lines().find(|l| !l.is_empty()),
            Some("TODO"),
            "{heading}"
        );
    }
    let state = state_json(dir, "ABC-1");
    assert_eq!(state["schema_version"], 1);
    assert_eq!(state["key"], "ABC-1");
    assert_eq!(state["title"], "First ticket");
    assert_eq!(state["confirmations"], serde_json::json!([]));
    assert!(state.get("tracker").is_none());
}

#[test]
fn numbers_after_largest_existing() {
    let repo = stapel_repo();
    let dir = repo.path();
    for key in ["ABC-1", "ABC-7"] {
        std::fs::create_dir_all(dir.join(".stapel/tickets").join(key)).unwrap();
    }
    stapel(dir)
        .args(["new", "x"])
        .assert()
        .success()
        .stdout(contains("created: ABC-8"));
}

#[test]
fn numbering_ignores_case_and_zero_padding() {
    let repo = stapel_repo();
    let dir = repo.path();
    for key in ["abc-3", "ABC-005", "XYZ-90", "ABC-x"] {
        std::fs::create_dir_all(dir.join(".stapel/tickets").join(key)).unwrap();
    }
    stapel(dir)
        .args(["new", "x"])
        .assert()
        .success()
        .stdout(contains("created: ABC-6"));
}

#[test]
fn refuses_bad_title() {
    let repo = stapel_repo();
    let dir = repo.path();
    for bad in ["", "   ", "a\nb", "a\tb", "a\u{7}b"] {
        stapel(dir)
            .args(["new", bad])
            .assert()
            .code(1)
            .stderr(contains("title"));
    }
    assert!(tickets(dir).is_empty(), "{:?}", tickets(dir));
}

#[test]
fn stores_tracker_link() {
    let repo = stapel_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["new", "x", "--tracker", "https://example.com/T-1"])
        .assert()
        .success();
    assert_eq!(
        state_json(dir, "ABC-1")["tracker"],
        "https://example.com/T-1"
    );
    let ticket = read(dir, ".stapel/tickets/ABC-1/ticket.md");
    let description = ticket.split("## Description\n").nth(1).unwrap();
    assert_eq!(
        description.lines().find(|l| !l.is_empty()),
        Some("Tracker: https://example.com/T-1")
    );
}

#[test]
fn refuses_bad_tracker_url() {
    let repo = stapel_repo();
    let dir = repo.path();
    for bad in [
        "ftp://example.com/x",
        "https://a b",
        "javascript:alert(1)",
        "https://x\ny",
        "",
    ] {
        stapel(dir)
            .args(["new", "x", "--tracker", bad])
            .assert()
            .code(1)
            .stderr(contains("tracker"));
    }
    assert!(tickets(dir).is_empty());
}

#[test]
fn refuses_existing_key() {
    let repo = stapel_repo();
    let dir = repo.path();
    // A plain file takes the name the next ticket folder would get.
    std::fs::write(dir.join(".stapel/tickets/ABC-1"), "").unwrap();
    stapel(dir)
        .args(["new", "x"])
        .assert()
        .code(1)
        .stderr(contains("ABC-1"));
}

#[test]
fn refuses_without_config() {
    let repo = git_repo();
    stapel(repo.path())
        .args(["new", "x"])
        .assert()
        .code(1)
        .stderr(contains("stapel.toml"));
    assert!(!repo.path().join(".stapel").exists());
}

#[test]
fn records_the_base_commit() {
    // No commits: no `base`.
    let repo = stapel_repo();
    stapel(repo.path())
        .args(["new", "First"])
        .assert()
        .success();
    assert!(state_json(repo.path(), "ABC-1").get("base").is_none());

    // With a commit: `base` is the full id of HEAD.
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
            ])
            .args(args)
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "{args:?}");
        String::from_utf8(out.stdout).unwrap()
    };
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "initial"]);
    let head = git(&["rev-parse", "HEAD"]).trim().to_string();
    stapel(repo.path())
        .args(["new", "Second"])
        .assert()
        .success();
    let state = state_json(repo.path(), "ABC-2");
    assert_eq!(state["base"], head.as_str());
    assert_eq!(head.len(), 40);
}

#[test]
fn base_read_errors_fail_new() {
    // A corrupted `.git/HEAD` is a git error that is not a repository without commits.
    let repo = stapel_repo();
    let dir = repo.path();
    std::fs::write(dir.join(".git/HEAD"), "garbage\n").unwrap();
    stapel(dir)
        .args(["new", "First"])
        .assert()
        .code(1)
        .stderr(contains("git"));
    assert!(tickets(dir).is_empty(), "{:?}", tickets(dir));
}
