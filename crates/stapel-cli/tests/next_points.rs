//! STP-35 AC-14: the points of `[check] points` in `stapel next` (the step point).

mod common;

use common::{repo_with_ticket, set_section, stapel};
use std::path::Path;

const RUN_STEPS: &str = "next: run-check — stapel check ABC-1 --level steps";
const COMMIT: &str = "next: commit — git status";
const BUILD_TWO: &str = "next: build-step step 2 — stapel brief ABC-1 2";

/// The one action line of `stapel next`.
fn action(dir: &Path) -> String {
    let out = stapel(dir).arg("next").assert().code(0).get_output().stdout.clone();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("next: ")).collect();
    assert_eq!(lines.len(), 1, "one action line in: {text}");
    lines[0].to_string()
}

/// Commits everything with `subject` and returns the new `HEAD`.
fn commit(dir: &Path, files: &[(&str, &str)], subject: &str) -> String {
    for (rel, text) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    for args in [
        vec!["add", "-A"],
        vec!["commit", "-q", "--allow-empty", "-m", subject],
    ] {
        let out = std::process::Command::new("git")
            .args(&args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "test-user")
            .env("GIT_AUTHOR_EMAIL", "test-user.invalid")
            .env("GIT_COMMITTER_NAME", "test-user")
            .env("GIT_COMMITTER_EMAIL", "test-user.invalid")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A confirmed ticket with a two-step Plan and `[check]` with `points` (a TOML line, or none);
/// nothing is committed except `initial`.
fn ticket(points: Option<&str>) -> tempfile::TempDir {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n\
         | AC-1 | WHEN a thing happens THE command does it. | `next::t_one` |",
    );
    set_section(
        dir,
        "ABC-1",
        "Plan",
        "Route: full, auto.\n\n1. One (AC-1).\n2. Two (AC-1).",
    );
    let toml = dir.join(".stapel/stapel.toml");
    let mut text = std::fs::read_to_string(&toml).unwrap();
    text.push_str("\n[check]\nrunner = \"cargo\"\ntimeout_secs = 900\n");
    if let Some(points) = points {
        text.push_str(&format!("points = {points}\n"));
    }
    std::fs::write(&toml, text).unwrap();
    for s in ["spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", s]).assert().success();
    }
    commit(dir, &[], "initial");
    repo
}

/// Commits the RED and the GREEN of step 1; returns the GREEN.
fn step_one(dir: &Path) -> String {
    commit(
        dir,
        &[("crates/tiny/tests/t1.rs", "#[test]\nfn t() {\n    assert!(true);\n}\n")],
        "ABC-1 step 1 RED: test",
    );
    commit(dir, &[("crates/tiny/src/lib.rs", "// one\n")], "ABC-1 step 1 GREEN: code")
}

fn record(dir: &Path, head: &str, result: &str) {
    let record = serde_json::json!({
        "v": 1, "kind": "check", "id": "c1", "at": "2026-10-19T10:00:00Z",
        "ticket": "ABC-1", "head": head, "result": result, "steps": [], "level": "steps",
    });
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/runs.jsonl"),
        format!("{record}\n"),
    )
    .unwrap();
}

// AC-14: with the step point, a clean tree after a GREEN runs the check at level `steps`.
#[test]
fn step_point() {
    // The default points hold `step`.
    let repo = ticket(None);
    let dir = repo.path();
    // The first step: no check before it.
    assert_eq!(action(dir), "next: build-step step 1 — stapel brief ABC-1 1");
    let green = step_one(dir);
    assert_eq!(action(dir), RUN_STEPS);

    // A dirty tree: `commit` first, also for an untracked file.
    std::fs::write(dir.join("f1.txt"), "x\n").unwrap();
    assert_eq!(action(dir), COMMIT);
    std::fs::remove_file(dir.join("f1.txt")).unwrap();
    assert_eq!(action(dir), RUN_STEPS);

    // A current pass: the build goes on.
    record(dir, &green, "pass");
    assert_eq!(action(dir), BUILD_TWO);

    // The explicit list with `step`.
    let repo = ticket(Some("[\"step\", \"close\"]"));
    step_one(repo.path());
    assert_eq!(action(repo.path()), RUN_STEPS);
}

// AC-14: without the step point, `build-step` follows the GREEN.
#[test]
fn without_the_step_point() {
    for points in ["[\"close\"]", "[\"review\", \"close\"]"] {
        // The same ticket demands the check with the default points; the key takes it away.
        let repo = ticket(None);
        let dir = repo.path();
        step_one(dir);
        assert_eq!(action(dir), RUN_STEPS);
        let toml = dir.join(".stapel/stapel.toml");
        let mut text = std::fs::read_to_string(&toml).unwrap();
        text.push_str(&format!("points = {points}\n"));
        std::fs::write(&toml, text).unwrap();
        commit(dir, &[], "set the points");
        assert_eq!(action(dir), BUILD_TWO, "points {points}");
        std::fs::write(dir.join("f1.txt"), "x\n").unwrap();
        assert_eq!(action(dir), BUILD_TWO, "points {points}, dirty tree");
    }
}

const REVIEW_ONE: &str = "next: code-review — stapel review start ABC-1 --round 1";
const RUN_PLAIN: &str = "next: run-check — stapel check ABC-1";
const RUN_HEAD: &str = "next: run-check — stapel check ABC-1 --level head";
const SUMMARY: &str = "next: write-summary — .stapel/tickets/ABC-1/ticket.md";

/// A ticket with every Plan step done; `extra` are lines added to `[check]` (committed).
fn done(extra: &str) -> (tempfile::TempDir, String) {
    let repo = ticket(None);
    let dir = repo.path();
    step_one(dir);
    commit(
        dir,
        &[("crates/tiny/tests/t2.rs", "#[test]\nfn t() {\n    assert!(true);\n}\n")],
        "ABC-1 step 2 RED: test",
    );
    commit(dir, &[("crates/tiny/src/two.rs", "// two\n")], "ABC-1 step 2 GREEN: code");
    let toml = dir.join(".stapel/stapel.toml");
    let mut text = std::fs::read_to_string(&toml).unwrap();
    text.push_str(extra);
    std::fs::write(&toml, text).unwrap();
    let head = commit(dir, &[], "set the check keys");
    (repo, head)
}

/// Writes a check record; `level` is left out of the record when `None` (an old record).
fn level_record(dir: &Path, head: &str, result: &str, level: Option<&str>) {
    let mut record = serde_json::json!({
        "v": 1, "kind": "check", "id": "c1", "at": "2026-10-19T10:00:00Z",
        "ticket": "ABC-1", "head": head, "result": result, "steps": [],
    });
    if let Some(level) = level {
        record["level"] = serde_json::json!(level);
    }
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/runs.jsonl"),
        format!("{record}\n"),
    )
    .unwrap();
}

// AC-15: the review point demands a check at the configured level before `code-review`.
#[test]
fn review_point() {
    // Default points and level: a check of level `head` or higher is needed.
    let (repo, head) = done("");
    let dir = repo.path();
    assert_eq!(action(dir), RUN_PLAIN);
    level_record(dir, &head, "pass", Some("steps"));
    assert_eq!(action(dir), RUN_PLAIN, "a steps pass is below head");
    level_record(dir, &head, "pass", Some("head"));
    assert_eq!(action(dir), REVIEW_ONE);
    level_record(dir, &head, "pass", Some("full"));
    assert_eq!(action(dir), REVIEW_ONE);
    level_record(dir, &head, "pass", None);
    assert_eq!(action(dir), REVIEW_ONE, "a record without level counts as full");

    // The key `full`: a `head` pass does not count.
    let (repo, head) = done("level = \"full\"\n");
    let dir = repo.path();
    level_record(dir, &head, "pass", Some("head"));
    assert_eq!(action(dir), RUN_PLAIN);
    level_record(dir, &head, "pass", Some("full"));
    assert_eq!(action(dir), REVIEW_ONE);

    // The key `steps`: a `steps` pass counts.
    let (repo, head) = done("level = \"steps\"\n");
    let dir = repo.path();
    assert_eq!(action(dir), RUN_PLAIN);
    level_record(dir, &head, "pass", Some("steps"));
    assert_eq!(action(dir), REVIEW_ONE);

    // Without the review point: `code-review` with no check.
    for points in ["[\"close\"]", "[\"step\", \"close\"]"] {
        let (repo, _) = done(&format!("points = {points}\n"));
        assert_eq!(action(repo.path()), REVIEW_ONE, "points {points}");
    }
    // With it: the check is demanded.
    for points in ["[\"review\", \"close\"]", "[\"step\", \"review\", \"close\"]"] {
        let (repo, _) = done(&format!("points = {points}\n"));
        assert_eq!(action(repo.path()), RUN_PLAIN, "points {points}");
    }
}

fn status_line(dir: &Path) -> String {
    let out = stapel(dir).args(["status", "ABC-1"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| l.starts_with("check: "))
        .unwrap_or("(no check line)")
        .to_string()
}

// AC-16: the close needs a `head` or `full` pass: `next` before `write-summary`, `status`, `close`.
#[test]
fn close_needs_head_or_full() {
    const GATE: &str =
        "stapel: the close needs a check of level head or full; run stapel check ABC-1 --level head";
    // Each case: the record (result, level) or none, whether it is made stale, whether it passes
    // the gate.
    type Case = (&'static str, Option<(&'static str, Option<&'static str>)>, bool, bool);
    let cases: [Case; 7] = [
        ("none", None, false, false),
        ("fail", Some(("fail", Some("head"))), false, false),
        ("stale", Some(("pass", Some("head"))), true, false),
        ("steps", Some(("pass", Some("steps"))), false, false),
        ("head", Some(("pass", Some("head"))), false, true),
        ("full", Some(("pass", Some("full"))), false, true),
        ("old", Some(("pass", None)), false, true),
    ];
    for (name, rec, stale, ok) in cases {
        let (repo, _) = done("points = [\"close\"]\n");
        let dir = repo.path();
        set_section(dir, "ABC-1", "Review", "Reviewed: no findings.");
        let head = commit(dir, &[], "ABC-1: review");
        if let Some((result, level)) = rec {
            level_record(dir, &head, result, level);
        }
        if stale {
            commit(dir, &[("crates/tiny/src/three.rs", "// three\n")], "later code");
        }
        let line = action(dir);
        if ok {
            assert_eq!(line, SUMMARY, "{name}");
        } else if name == "fail" {
            // A current fail is met earlier by `fix-check`.
            assert_eq!(line, "next: fix-check — .stapel/tickets/ABC-1/runs.jsonl", "{name}");
        } else {
            assert_eq!(line, RUN_HEAD, "{name}");
        }
        let short = &head[..7];
        match name {
            "steps" => assert_eq!(status_line(dir), format!("check: pass at {short} (level steps)")),
            "head" | "full" | "old" => {
                assert_eq!(status_line(dir), format!("check: pass at {short}"), "{name}")
            }
            _ => {}
        }
        let close = stapel(dir).args(["close", "ABC-1", "--reason", "merged"]).assert();
        if ok {
            close.success();
        } else {
            let out = close.code(1).get_output().stderr.clone();
            let err = String::from_utf8_lossy(&out).to_string();
            if name == "steps" {
                assert!(err.contains(GATE), "{name}: {err}");
            }
        }
    }
}
