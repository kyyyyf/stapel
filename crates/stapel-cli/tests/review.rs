//! STP-24: `stapel review start KEY --round N` and its refusals (step 1).

mod common;

use common::{repo_with_ticket, snapshot, stapel, state_json};
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

/// The command refuses with `code` and `text` after `stapel: `, and writes nothing.
fn refused(dir: &Path, args: &[&str], code: i32, text: &str) {
    let before = snapshot(dir);
    start(dir, args)
        .code(code)
        .stderr(format!("stapel: {text}\n"));
    assert_eq!(snapshot(dir), before, "{text}: files changed");
    no_review_folder(dir);
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
    refused(
        dir,
        &["ABC-9", "--round", "1"],
        2,
        "no ticket ABC-9 in .stapel/tickets",
    );

    // The key is case-insensitive.
    start(dir, &["abc-1", "--round", "1"]).code(0);

    let mut state = state_json(dir, "ABC-1");
    close(dir);
    refused(
        dir,
        &["ABC-1", "--round", "1"],
        2,
        "ticket ABC-1 is closed; it has no review round",
    );

    state.as_object_mut().unwrap().remove("schema_version");
    write_state(dir, "ABC-1", &state);
    refused(
        dir,
        &["ABC-1", "--round", "1"],
        1,
        "ticket ABC-1 is in the STP-1 format; it has no review pack",
    );
}

#[test]
fn refuses_a_dirty_tree() {
    let repo = built();
    let dir = repo.path();
    let run = |dir: &Path| start(dir, &["ABC-1", "--round", "1"]);
    let dirty =
        |path: &str| format!("uncommitted changes: {path}; commit them before a review round");
    let args = ["ABC-1", "--round", "1"];

    std::fs::write(dir.join("stray.txt"), "x").unwrap();
    refused(dir, &args, 2, &dirty("stray.txt"));
    std::fs::remove_file(dir.join("stray.txt")).unwrap();

    std::fs::write(dir.join("code.txt"), "two\n").unwrap();
    git(dir, &["add", "code.txt"]);
    refused(dir, &args, 2, &dirty("code.txt"));
    git(dir, &["restore", "--staged", "--worktree", "code.txt"]);

    let ticket = dir.join(".stapel/tickets/ABC-1/ticket.md");
    let text = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{text}\nmore\n")).unwrap();
    refused(dir, &args, 2, &dirty(".stapel/tickets/ABC-1/ticket.md"));
    git(dir, &["restore", ".stapel/tickets/ABC-1/ticket.md"]);

    // Seven dirty paths: the text names the first five.
    for n in 1..=7 {
        std::fs::write(dir.join(format!("d{n}.txt")), "x").unwrap();
    }
    refused(
        dir,
        &args,
        2,
        &dirty("d1.txt, d2.txt, d3.txt, d4.txt, d5.txt"),
    );
    for n in 1..=7 {
        std::fs::remove_file(dir.join(format!("d{n}.txt"))).unwrap();
    }

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
        refused(
            dir,
            &["ABC-1", "--round", round],
            2,
            "--round must be 1 or 2; after round 2 the person decides",
        );
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
    refused(
        dir,
        &["ABC-1", "--round", "1"],
        1,
        "state.json of ABC-1 has no base commit",
    );

    let bad = |base: &str| {
        format!(
            "state.json has base {base}, which is not a full commit id on the first-parent history of HEAD"
        )
    };
    let mut state = good.clone();
    let unknown = "0".repeat(40);
    state["base"] = serde_json::json!(unknown);
    write_state(dir, "ABC-1", &state);
    refused(dir, &["ABC-1", "--round", "1"], 1, &bad(&unknown));

    // A base that exists but lies off the first-parent history: the second parent of a merge.
    let first = good["base"].as_str().unwrap().to_string();
    let tree = git(dir, &["rev-parse", &format!("{first}^{{tree}}")])
        .trim()
        .to_string();
    let side = git(dir, &["commit-tree", &tree, "-p", &first, "-m", "side"])
        .trim()
        .to_string();
    git(dir, &["merge", "-q", "--no-ff", "-m", "merge", &side]);
    let mut state = good;
    state["base"] = serde_json::json!(side);
    write_state(dir, "ABC-1", &state);
    refused(dir, &["ABC-1", "--round", "1"], 1, &bad(&side));
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
    refused(
        dir,
        &["ABC-1", "--round", "3"],
        2,
        "ticket ABC-1 is closed; it has no review round",
    );

    // Legacy and closed in one state.json, with a dirty tree and a bad round: legacy wins.
    let mut legacy = state_json(dir, "ABC-1");
    legacy.as_object_mut().unwrap().remove("schema_version");
    write_state(dir, "ABC-1", &legacy);
    refused(
        dir,
        &["ABC-1", "--round", "3"],
        1,
        "ticket ABC-1 is in the STP-1 format; it has no review pack",
    );

    std::fs::remove_file(dir.join("stray.txt")).unwrap();
    let mut state = good;
    let unknown = "0".repeat(40);
    state["base"] = serde_json::json!(unknown);
    write_state(dir, "ABC-1", &state);
    refused(
        dir,
        &["ABC-1", "--round", "2"],
        1,
        &format!(
            "state.json has base {unknown}, which is not a full commit id on the first-parent history of HEAD"
        ),
    );
}

#[test]
fn refusal_order_of_the_later_cases() {
    let repo = built();
    let dir = repo.path();
    let good = state_json(dir, "ABC-1");
    let dirty = "uncommitted changes: stray.txt; commit them before a review round";

    // An unknown key wins over a bad round.
    refused(
        dir,
        &["ABC-9", "--round", "3"],
        2,
        "no ticket ABC-9 in .stapel/tickets",
    );

    // A bad round wins over a dirty tree.
    std::fs::write(dir.join("stray.txt"), "x").unwrap();
    refused(
        dir,
        &["ABC-1", "--round", "3"],
        2,
        "--round must be 1 or 2; after round 2 the person decides",
    );

    // A dirty tree wins over a missing base and over a bad base.
    let mut no_base = good.clone();
    no_base.as_object_mut().unwrap().remove("base");
    write_state(dir, "ABC-1", &no_base);
    refused(dir, &["ABC-1", "--round", "1"], 2, dirty);

    let mut bad_base = good;
    bad_base["base"] = serde_json::json!("0".repeat(40));
    write_state(dir, "ABC-1", &bad_base);
    refused(dir, &["ABC-1", "--round", "1"], 2, dirty);
}
