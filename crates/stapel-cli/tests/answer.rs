//! STP-40: `stapel answer`, the person's triage of a finding that waits for the person.

mod common;

use common::{git_in, repo_with_ticket, stapel};
use serde_json::json;
use std::path::Path;

const TARGET: &str = ".stapel/tickets/ABC-1/findings.jsonl";

/// A ticket past its spec, on the branch `ABC-1`.
fn past_spec_on_branch() -> tempfile::TempDir {
    let repo = repo_with_ticket();
    let dir = repo.path();
    common::set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    common::set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | WHEN a thing happens THE command does it. | `next::t_one` |",
    );
    common::set_section(
        dir,
        "ABC-1",
        "Plan",
        "Route: full, auto.\n\n1. One (AC-1).\n2. Two (AC-1).",
    );
    for s in ["spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", s]).assert().success();
    }
    git_in(dir, &["add", "-A"]);
    git_in(
        dir,
        &[
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-q",
            "-m",
            "base",
        ],
    );
    git_in(dir, &["checkout", "-q", "-b", "ABC-1"]);
    repo
}

/// An open HIGH finding of code-review round 2: it waits for the person.
fn owed(id: &str) -> String {
    json!({
        "v": 1,
        "id": id,
        "stage": "code-review",
        "round": 2,
        "severity": "HIGH",
        "promise": "inside",
        "claim": format!("claim of {id}"),
        "fate": "open",
    })
    .to_string()
}

fn next_text(dir: &Path) -> String {
    let out = stapel(dir)
        .args(["next", "ABC-1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(out).unwrap()
}

// AC-6: each person's fate on an owed finding, the line byte for byte, `stapel next` afterwards.
#[test]
fn appends_a_person_triage() {
    let repo = past_spec_on_branch();
    let dir = repo.path();
    let cases = [
        ("F-1", "patch", "- fix it", r#""- fix it""#),
        ("F-2", "bad_plan", r#"say "no" \ later"#, r#""say \"no\" \\ later""#),
        ("F-3", "defer", "later", r#""later""#),
        ("F-4", "no-change", "as built", r#""as built""#),
    ];
    let seeded: String = cases.iter().map(|c| format!("{}\n", owed(c.0))).collect();
    std::fs::write(dir.join(TARGET), &seeded).unwrap();
    for (id, _, _, _) in cases {
        assert!(next_text(dir).contains(&format!("blocked: finding {id} ")));
    }

    let mut expected = seeded.clone();
    for (id, fate, note, quoted) in cases {
        stapel(dir)
            .args(["answer", "ABC-1", id, fate, note])
            .assert()
            .code(0)
            .stdout(format!("answered: {id} of ABC-1: {fate}\n"));
        expected.push_str(&format!(
            "{{\"kind\":\"triage\",\"ticket\":\"ABC-1\",\"id\":\"{id}\",\"fate\":\"{fate}\",\"note\":{quoted},\"imported_by\":{{\"by\":\"person\",\"user\":\"test-user\"}}}}\n"
        ));
        assert_eq!(common::read(dir, TARGET), expected);
        let next = next_text(dir);
        assert!(!next.contains(&format!("finding {id} ")), "{next}");
    }
    let next = next_text(dir);
    assert!(!next.contains("ask-human"), "{next}");
}

/// `stapel answer` refuses: exit 2, the text on stderr, nothing on stdout, `TARGET` as it was.
fn refuses(dir: &Path, args: &[&str], message: &str) {
    let before = std::fs::read(dir.join(TARGET)).ok();
    let out = stapel(dir).arg("answer").args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "{args:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "", "{args:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!("stapel: {message}\n"),
        "{args:?}"
    );
    assert_eq!(std::fs::read(dir.join(TARGET)).ok(), before, "{args:?}");
}

// AC-7: every refusal prints its reason, exits 2 and appends nothing.
#[test]
fn refusals() {
    let repo = past_spec_on_branch();
    let dir = repo.path();
    let mut low = serde_json::from_str::<serde_json::Value>(&owed("F-3")).unwrap();
    low["severity"] = json!("LOW");
    let triaged = json!({
        "kind": "triage", "ticket": "ABC-1", "id": "F-2", "fate": "defer", "note": "later",
        "imported_by": {"by": "person", "user": "x"},
    });
    let seeded = format!("{}\n{}\n{triaged}\n{low}\n", owed("F-1"), owed("F-2"));
    std::fs::write(dir.join(TARGET), &seeded).unwrap();

    refuses(dir, &["ABC-1", "F-9", "patch", "n"], "F-9 is not a finding of ABC-1");
    refuses(
        dir,
        &["ABC-1", "F-2", "patch", "n"],
        "F-2 of ABC-1 does not wait for a person",
    );
    refuses(
        dir,
        &["ABC-1", "F-3", "patch", "n"],
        "F-3 of ABC-1 does not wait for a person",
    );
    let fates = "the fate must be one of patch, bad_plan, defer, no-change";
    for fate in ["open", "intent_gap", "PATCH"] {
        refuses(dir, &["ABC-1", "F-1", fate, "n"], fates);
    }
    refuses(dir, &["ABC-1", "F-1", "patch", "   "], "the note is empty");
    let long = "x".repeat(70_000);
    refuses(
        dir,
        &["ABC-1", "F-1", "patch", &long],
        "the record is longer than the journal line limit",
    );

    // No git user name.
    git_in(dir, &["config", "--unset", "user.name"]);
    refuses(
        dir,
        &["ABC-1", "F-1", "patch", "n"],
        "git config user.name is not set; set it to the name confirmations are recorded under",
    );
    git_in(dir, &["config", "user.name", "test-user"]);

    // A line of findings.jsonl that does not parse.
    std::fs::write(dir.join(TARGET), format!("{seeded}not json\n")).unwrap();
    let before = std::fs::read(dir.join(TARGET)).unwrap();
    let out = stapel(dir)
        .args(["answer", "ABC-1", "F-1", "patch", "n"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(err.starts_with(&format!("stapel: {TARGET}:5: ")), "{err}");
    assert_eq!(std::fs::read(dir.join(TARGET)).unwrap(), before);
    std::fs::write(dir.join(TARGET), &seeded).unwrap();

    // Another ticket than the branch's.
    git_in(dir, &["checkout", "-q", "-"]);
    stapel(dir).args(["new", "Second"]).assert().success();
    git_in(dir, &["checkout", "-q", "ABC-1"]);
    refuses(
        dir,
        &["ABC-2", "F-1", "patch", "n"],
        "ABC-2 is not the ticket of branch ABC-1; run it in .claude/worktrees/ABC-2",
    );

    // A closed ticket.
    stapel(dir)
        .args(["close", "--reason", "done"])
        .assert()
        .success();
    refuses(dir, &["ABC-1", "F-1", "patch", "n"], "ABC-1 is closed");

    // A legacy state.json on its own branch.
    git_in(dir, &["checkout", "-q", "-b", "ABC-9"]);
    let legacy = dir.join(".stapel/tickets/ABC-9");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(
        legacy.join("state.json"),
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    )
    .unwrap();
    refuses(
        dir,
        &["ABC-9", "F-1", "patch", "n"],
        "ticket ABC-9 has a legacy (STP-1) state.json; its journal is kept by hand",
    );

    // A checkout whose branch names no ticket.
    git_in(dir, &["checkout", "-q", "-b", "side"]);
    refuses(
        dir,
        &["ABC-1", "F-1", "patch", "n"],
        "stapel answer runs in the worktree of ABC-1; this checkout's branch names no ticket",
    );
}
