//! STP-24: `stapel review start KEY --round N` and its refusals (step 1).

mod common;

use common::{repo_with_ticket, stapel, state_json};
use predicates::prelude::*;
use predicates::str::contains;
use std::path::Path;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(["-c", "user.email=none", "-c", "user.name=test-user"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8(out.stdout).unwrap()
}

fn write_state(dir: &Path, key: &str, v: &serde_json::Value) {
    std::fs::write(
        dir.join(format!(".stapel/tickets/{key}/state.json")),
        serde_json::to_string_pretty(v).unwrap(),
    )
    .unwrap();
}

/// A ticket ABC-1 with a base commit and one later commit; the tree is clean.
fn built() -> tempfile::TempDir {
    let repo = repo_with_ticket();
    let dir = repo.path();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "initial"]);
    let base = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let mut state = state_json(dir, "ABC-1");
    state["base"] = serde_json::json!(base);
    write_state(dir, "ABC-1", &state);
    std::fs::write(dir.join("code.txt"), "one\n").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "ABC-1 step 1 GREEN: code"]);
    repo
}

fn findings(dir: &Path, lines: &[&str]) {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(dir.join(".stapel/tickets/ABC-1/findings.jsonl"), text).unwrap();
}

const ROUND_ONE: &str = r#"{"v":1,"stage":"code-review","round":"1","id":"R-1"}"#;

fn start(dir: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    stapel(dir).args(["review", "start"]).args(args).assert()
}

fn no_review_folder(dir: &Path) {
    assert!(
        !dir.join(".stapel/scratch/review").exists(),
        "a review folder was written"
    );
}

fn close(dir: &Path) {
    let mut state = state_json(dir, "ABC-1");
    state["closed"] =
        serde_json::json!({"by": "test-user", "at": "2026-10-19T10:00:00Z", "reason": "done"});
    write_state(dir, "ABC-1", &state);
}

#[test]
fn refuses_tickets() {
    let repo = built();
    let dir = repo.path();
    start(dir, &["ABC-9", "--round", "1"])
        .code(2)
        .stderr(contains("no ticket ABC-9 in .stapel/tickets"));
    no_review_folder(dir);

    // The key is case-insensitive.
    start(dir, &["abc-1", "--round", "1"]).code(0);

    let mut state = state_json(dir, "ABC-1");
    close(dir);
    start(dir, &["ABC-1", "--round", "1"])
        .code(2)
        .stderr(contains("stapel: ticket ABC-1 is closed"));
    no_review_folder(dir);

    state.as_object_mut().unwrap().remove("schema_version");
    write_state(dir, "ABC-1", &state);
    start(dir, &["ABC-1", "--round", "1"])
        .code(1)
        .stderr(contains(
            "stapel: ticket ABC-1 has a legacy (STP-1) state.json",
        ));
    no_review_folder(dir);
}

#[test]
fn refuses_a_dirty_tree() {
    let repo = built();
    let dir = repo.path();
    let run = |dir: &Path| start(dir, &["ABC-1", "--round", "1"]);

    std::fs::write(dir.join("stray.txt"), "x").unwrap();
    run(dir)
        .code(2)
        .stderr(contains("uncommitted changes").and(contains("stray.txt")));
    std::fs::remove_file(dir.join("stray.txt")).unwrap();

    std::fs::write(dir.join("code.txt"), "two\n").unwrap();
    git(dir, &["add", "code.txt"]);
    run(dir)
        .code(2)
        .stderr(contains("uncommitted changes").and(contains("code.txt")));
    git(dir, &["restore", "--staged", "--worktree", "code.txt"]);

    let ticket = dir.join(".stapel/tickets/ABC-1/ticket.md");
    let text = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{text}\nmore\n")).unwrap();
    run(dir)
        .code(2)
        .stderr(contains("uncommitted changes").and(contains(".stapel/tickets/ABC-1/ticket.md")));
    no_review_folder(dir);
    git(dir, &["restore", ".stapel/tickets/ABC-1/ticket.md"]);

    // An ignored file and a machine file do not count.
    std::fs::write(dir.join(".git/info/exclude"), "ignored.txt\n").unwrap();
    std::fs::write(dir.join("ignored.txt"), "x").unwrap();
    findings(dir, &[ROUND_ONE]);
    run(dir).code(0);
}

#[test]
fn refuses_bad_rounds() {
    let repo = built();
    let dir = repo.path();
    for round in ["0", "3"] {
        start(dir, &["ABC-1", "--round", round])
            .code(2)
            .stderr(contains("stapel: round must be 1 or 2"));
    }
    for args in [
        vec!["ABC-1", "--round", "x"],
        vec!["ABC-1", "--round", "-1"],
        vec!["ABC-1"],
    ] {
        start(dir, &args).code(2).stderr(contains("error:"));
    }
    no_review_folder(dir);
}

#[test]
fn refuses_a_bad_base() {
    let repo = built();
    let dir = repo.path();
    let good = state_json(dir, "ABC-1");

    let mut state = good.clone();
    state.as_object_mut().unwrap().remove("base");
    write_state(dir, "ABC-1", &state);
    start(dir, &["ABC-1", "--round", "1"])
        .code(1)
        .stderr(contains("stapel: ticket ABC-1 has no base in state.json"));
    no_review_folder(dir);

    let mut state = good;
    state["base"] = serde_json::json!("0".repeat(40));
    write_state(dir, "ABC-1", &state);
    start(dir, &["ABC-1", "--round", "1"])
        .code(1)
        .stderr(contains("stapel: state.json has base"));
    no_review_folder(dir);
}

#[test]
fn round_two_needs_round_one() {
    let repo = built();
    let dir = repo.path();
    let text = "stapel: round 2 needs the code review findings of round 1 in .stapel/tickets/ABC-1/findings.jsonl";
    let refuse = |dir: &Path| {
        start(dir, &["ABC-1", "--round", "2"])
            .code(2)
            .stderr(contains(text));
        no_review_folder(dir);
    };
    refuse(dir);
    findings(
        dir,
        &[
            r#"{"v":1,"stage":"spec-review","round":"1","id":"S-1"}"#,
            r#"{"v":1,"stage":"step-drift","round":"1","id":"D-1"}"#,
        ],
    );
    refuse(dir);
    findings(
        dir,
        &[r#"{"v":1,"stage":"code-review","round":"2","id":"R-1"}"#],
    );
    refuse(dir);
    findings(dir, &[ROUND_ONE]);
    start(dir, &["ABC-1", "--round", "2"]).code(0);
}

#[test]
fn refusal_order() {
    let repo = built();
    let dir = repo.path();
    let good = state_json(dir, "ABC-1");
    std::fs::write(dir.join("stray.txt"), "x").unwrap();

    close(dir);
    start(dir, &["ABC-1", "--round", "3"])
        .code(2)
        .stderr(contains("is closed"));

    let mut legacy = good.clone();
    legacy.as_object_mut().unwrap().remove("schema_version");
    write_state(dir, "ABC-1", &legacy);
    start(dir, &["ABC-1", "--round", "1"])
        .code(1)
        .stderr(contains("legacy (STP-1)"));

    std::fs::remove_file(dir.join("stray.txt")).unwrap();
    let mut state = good;
    state["base"] = serde_json::json!("0".repeat(40));
    write_state(dir, "ABC-1", &state);
    start(dir, &["ABC-1", "--round", "2"])
        .code(1)
        .stderr(contains("state.json has base"));
    no_review_folder(dir);
}
