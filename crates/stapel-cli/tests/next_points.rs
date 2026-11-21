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
