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
