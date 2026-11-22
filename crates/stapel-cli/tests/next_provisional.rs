//! STP-36: the build goes on without a `stapel ok` for the fix steps that follow a patch finding.

mod common;

use common::{repo_with_ticket, set_section, stapel};
use std::path::Path;

const BUILD_TWO: &str = "next: build-step step 2 — stapel brief ABC-1 2";
const FACT: &str = "provisional: plan waits for stapel ok ABC-1 before the close";

const PLAN_ONE: &str = "Route: full, auto.\n\n1. One (AC-1).";
const PLAN_TWO: &str = "Route: full, auto.\n\n1. One (AC-1).\n2. Two (AC-1).";

/// The standard output of `stapel next`.
fn next(dir: &Path) -> String {
    let out = stapel(dir).arg("next").assert().code(0).get_output().stdout.clone();
    String::from_utf8(out).unwrap()
}

/// The one action line of `stapel next`.
fn action(dir: &Path) -> String {
    let text = next(dir);
    let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("next: ")).collect();
    assert_eq!(lines.len(), 1, "one action line in: {text}");
    lines[0].to_string()
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
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

/// Commits everything with `subject`.
fn commit(dir: &Path, files: &[(&str, &str)], subject: &str) {
    for (rel, text) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", subject]);
}

/// A finding record of the code review.
fn finding(id: &str, round: u32, fate: &str) -> String {
    format!(
        "{{\"id\":\"{id}\",\"stage\":\"code-review\",\"round\":{round},\"severity\":\"LOW\",\
         \"claim\":\"A claim.\",\"fate\":\"{fate}\"}}\n"
    )
}

/// A confirmed ticket with a one-step Plan, step 1 built, and the findings file `findings`.
fn ticket(findings: &str) -> tempfile::TempDir {
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
    set_section(dir, "ABC-1", "Plan", PLAN_ONE);
    for s in ["spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", s]).assert().success();
    }
    std::fs::write(dir.join(".stapel/tickets/ABC-1/findings.jsonl"), findings).unwrap();
    commit(dir, &[], "initial");
    commit(
        dir,
        &[("crates/tiny/tests/t1.rs", "#[test]\nfn t() {\n    assert!(true);\n}\n")],
        "ABC-1 step 1 RED: test",
    );
    commit(dir, &[("crates/tiny/src/lib.rs", "// one\n")], "ABC-1 step 1 GREEN: code");
    repo
}

/// The hook's answer for a Write of `src/x.rs`: whether it is allowed.
fn write_allowed(dir: &Path) -> bool {
    let payload = serde_json::json!({
        "session_id": "test",
        "hook_event_name": "PreToolUse",
        "cwd": dir,
        "tool_name": "Write",
        "tool_input": { "file_path": dir.join("src/x.rs"), "content": "x" },
    });
    let out = stapel(dir)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .output()
        .unwrap();
    out.status.code() == Some(0)
}

fn status_build_line(dir: &Path) -> String {
    let out = stapel(dir).arg("status").output().unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .find(|l| l.starts_with("build: "))
        .unwrap_or("")
        .to_string()
}

/// The ticket with `findings`, the Plan `plan` and a change `edit` of another section, if any;
/// the action line of `stapel next`.
fn case(findings: &str, plan: &str, edit: Option<(&str, &str)>) -> (tempfile::TempDir, String) {
    let repo = ticket(findings);
    set_section(repo.path(), "ABC-1", "Plan", plan);
    if let Some((title, body)) = edit {
        set_section(repo.path(), "ABC-1", title, body);
    }
    let line = action(repo.path());
    (repo, line)
}

// AC-2, AC-3
#[test]
fn fix_steps_build_without_a_dialog() {
    let patch_one = finding("R1-1", 1, "patch");

    // A patch round 1 and a step appended to the Plan.
    let (repo, line) = case(&patch_one, PLAN_TWO, None);
    assert_eq!(line, BUILD_TWO);
    let text = next(repo.path());
    assert!(
        text.lines().any(|l| l == FACT),
        "no provisional fact in: {text}"
    );

    // The same with the patch finding in round 2.
    let both = format!("{patch_one}{}", finding("R2-1", 2, "patch"));
    let (_repo, line) = case(&both, PLAN_TWO, None);
    assert_eq!(line, BUILD_TWO);

    // The Plan in `[build] requires`: a code write is allowed and the status says so.
    let repo = ticket(&patch_one);
    let dir = repo.path();
    let toml = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&toml).unwrap();
    assert!(text.contains("\"proof\"]"), "requires line changed: {text}");
    std::fs::write(&toml, text.replacen("\"proof\"]", "\"proof\", \"plan\"]", 1)).unwrap();
    commit(dir, &[], "requires the plan");
    assert!(write_allowed(dir), "fresh Plan: write allowed");
    set_section(dir, "ABC-1", "Plan", PLAN_TWO);
    assert!(write_allowed(dir), "provisional: write allowed");
    assert_eq!(status_build_line(dir), "build: provisional ABC-1");

    // Stops: the Plan waits for the `stapel ok`, as today.
    let stop = |line: String, why: &str| {
        assert_eq!(line, "next: write-page — stapel page ABC-1", "{why}");
    };
    stop(
        case(&patch_one, PLAN_TWO, Some(("Spec", "The spec, changed."))).1,
        "Spec changed too",
    );
    stop(
        case(&patch_one, PLAN_TWO, Some(("Design", "The design, changed."))).1,
        "Design changed",
    );
    stop(
        case(
            &patch_one,
            "Route: full, auto.\n\n1. Uno (AC-1).\n2. Two (AC-1).",
            None,
        )
        .1,
        "an old step changed",
    );
    stop(
        case(
            &patch_one,
            "Route: light, auto.\n\n1. One (AC-1).\n2. Two (AC-1).",
            None,
        )
        .1,
        "the Route line changed",
    );
    stop(
        case(
            &patch_one,
            "Route: full, auto.\n\n1. One (AC-1), extended.\n2. Two (AC-1).",
            None,
        )
        .1,
        "the last confirmed line extended",
    );
    let round_two_clean = format!("{patch_one}{}", finding("R2-1", 2, "fixed"));
    stop(
        case(&round_two_clean, PLAN_TWO, None).1,
        "round 2 without a patch finding",
    );
    stop(
        case(&finding("R1-1", 1, "defer"), PLAN_TWO, None).1,
        "the fate defer only",
    );
    stop(case("", PLAN_TWO, None).1, "no code review round");

    // Before any confirmation: no provisional build.
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    set_section(dir, "ABC-1", "Plan", PLAN_TWO);
    std::fs::write(dir.join(".stapel/tickets/ABC-1/findings.jsonl"), &patch_one).unwrap();
    let text = next(dir);
    assert!(!text.contains("build-step"), "built before any ok: {text}");
    assert!(!text.contains("provisional:"), "{text}");
}

// AC-4
#[test]
fn close_waits_for_the_ok() {
    let repo = ticket(&finding("R1-1", 1, "patch"));
    let dir = repo.path();
    let waits = "next: write-page — stapel page ABC-1";

    // Both steps built, the review written, the Plan waiting.
    set_section(dir, "ABC-1", "Plan", PLAN_TWO);
    set_section(dir, "ABC-1", "Review", "Reviewed: one fix.");
    commit(dir, &[], "the fix step and the review");
    commit(
        dir,
        &[("crates/tiny/tests/t2.rs", "#[test]\nfn t() {\n    assert!(true);\n}\n")],
        "ABC-1 step 2 RED: test",
    );
    commit(dir, &[("crates/tiny/src/lib.rs", "// two\n")], "ABC-1 step 2 GREEN: code");
    assert_eq!(action(dir), waits, "instead of write-summary");

    // With the Summary written: instead of close.
    set_section(dir, "ABC-1", "Summary", "Done.");
    commit(dir, &[], "the summary");
    assert_eq!(action(dir), waits, "instead of close");
}
