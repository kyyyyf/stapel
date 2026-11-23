//! STP-40: `stapel inbox`, the list of the decisions that wait for the person.

mod common;

use common::{git_in, repo_with_ticket, stapel};
use serde_json::json;
use std::path::Path;

/// Puts the checkout on the branch `ABC-1` after one commit, so that the branch names the ticket.
fn on_ticket_branch(dir: &Path) {
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
}

fn finding(id: &str, stage: &str, round: Option<u64>, severity: &str, promise: &str) -> String {
    let mut v = json!({
        "v": 1,
        "id": id,
        "stage": stage,
        "severity": severity,
        "promise": promise,
        "claim": format!("claim of {id}"),
        "fate": "open",
    });
    if let Some(round) = round {
        v["round"] = json!(round);
    }
    v.to_string()
}

fn write_findings(dir: &Path, lines: &[String]) {
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/findings.jsonl"),
        lines.iter().map(|l| format!("{l}\n")).collect::<String>(),
    )
    .unwrap();
}

// AC-1: the owed findings of the branch ticket, in file order, in the golden file.
#[test]
fn lists_owed_findings() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    on_ticket_branch(dir);

    let mut gap = finding("F-1", "spec-review", Some(1), "HIGH", "inside");
    let mut value: serde_json::Value = serde_json::from_str(&gap).unwrap();
    value["fate"] = json!("intent_gap");
    value["claim"] = json!("The claim runs over two lines\nand holds a line\nnote: x here.");
    gap = value.to_string();
    let mut outside = finding("F-2", "code-review", None, "LOW", "outside");
    outside = outside.replace("claim of F-2", "Outside the promise.");
    let mut round_two = finding("F-3", "code-review", Some(2), "HIGH", "inside");
    round_two = round_two.replace("claim of F-3", "Round two, high.");
    let not_owed_round_one = finding("F-4", "code-review", Some(1), "MEDIUM", "inside");
    let not_owed_low = finding("F-5", "code-review", Some(2), "LOW", "inside");
    let mut triaged = finding("F-6", "code-review", Some(2), "HIGH", "inside");
    triaged = triaged.replace("\"fate\":\"open\"", "\"fate\":\"intent_gap\"");
    let triage = json!({
        "v": 1, "kind": "triage", "id": "F-6", "ticket": "ABC-1",
        "fate": "patch", "note": "done",
    })
    .to_string();
    write_findings(
        dir,
        &[
            gap,
            outside,
            round_two,
            not_owed_round_one,
            not_owed_low,
            triaged,
            triage,
        ],
    );

    let out = stapel(dir).arg("inbox").assert().code(0).get_output().clone();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "inbox: 3 decisions wait in .stapel/inbox.md\n"
    );
    let root = dir.canonicalize().unwrap();
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/inbox.md"),
    )
    .unwrap()
    .replace("<worktree>", &root.display().to_string());
    assert_eq!(common::read(dir, ".stapel/inbox.md"), golden);

    // One decision: the singular line, and the file is written over.
    write_findings(
        dir,
        &[finding("F-7", "code-review", Some(2), "HIGH", "inside")],
    );
    let out = stapel(dir).arg("inbox").assert().code(0).get_output().clone();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "inbox: 1 decision waits in .stapel/inbox.md\n"
    );
    let text = common::read(dir, ".stapel/inbox.md");
    assert!(text.contains("### ABC-1 finding F-7\n"), "{text}");
    assert!(!text.contains("F-1"), "{text}");
}

/// The ticket ABC-1 past its spec review: every section written, a spec-review finding that is
/// fixed and no confirmation yet.
fn past_spec_review(dir: &Path) {
    common::set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    common::set_section(dir, "ABC-1", "Plan", "Route: full, auto.\n\n1. One (AC-1).");
    write_findings(dir, &[fixed_spec_finding()]);
}

fn fixed_spec_finding() -> String {
    finding("S-1", "spec-review", Some(1), "LOW", "inside").replace("\"open\"", "\"fixed\"")
}

fn confirmation(root: &Path, waiting: &str) -> String {
    let root = root.canonicalize().unwrap().display().to_string();
    format!(
        "### ABC-1 confirmation\nwaiting: {waiting}\npage: {root}/.stapel/tickets/ABC-1/page.md\n"
    )
}

fn inbox_text(dir: &Path) -> String {
    stapel(dir).arg("inbox").assert().code(0);
    common::read(dir, ".stapel/inbox.md")
}

// AC-2: a confirmation entry for a ticket past its spec review with a waiting section.
#[test]
fn lists_owed_confirmations() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    on_ticket_branch(dir);
    let root = dir.canonicalize().unwrap().display().to_string();
    let head = format!("# Inbox\n\n## ABC-1 — First\nworktree: {root}\n\n");

    // Before the spec review: no entry.
    common::set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    common::set_section(dir, "ABC-1", "Plan", "Route: full, auto.\n\n1. One (AC-1).");
    assert!(!inbox_text(dir).contains("confirmation"));

    // write-page: the page is not written; all sections wait, in config order.
    past_spec_review(dir);
    let out = stapel(dir).arg("inbox").assert().code(0).get_output().clone();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "inbox: 1 decision waits in .stapel/inbox.md\n"
    );
    let want = format!("{head}{}", confirmation(dir, "spec, design, proof, plan"));
    assert_eq!(common::read(dir, ".stapel/inbox.md"), want);

    // confirm: the page is fresh; the same entry.
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    assert_eq!(inbox_text(dir), want);

    // An owed finding and a waiting section: the finding first; waiting ids in config order.
    stapel(dir).args(["ok", "ABC-1", "spec"]).assert().success();
    stapel(dir).args(["ok", "ABC-1", "design"]).assert().success();
    let owed = finding("F-1", "code-review", Some(2), "HIGH", "inside");
    write_findings(dir, &[fixed_spec_finding(), owed]);
    assert_eq!(
        inbox_text(dir),
        format!(
            "{head}### ABC-1 finding F-1\ncode-review round 2, HIGH, inside: claim of F-1\nnote:\n\n{}",
            confirmation(dir, "proof, plan")
        )
    );

    // A section changed after its confirmation waits again, in the build.
    for id in ["proof", "plan"] {
        stapel(dir).args(["ok", "ABC-1", id]).assert().success();
    }
    write_findings(dir, &[fixed_spec_finding()]);
    common::set_section(
        dir,
        "ABC-1",
        "Plan",
        "Route: full, auto.\n\n1. One (AC-1).\n2. Two (AC-1).",
    );
    assert_eq!(
        inbox_text(dir),
        format!("{head}{}", confirmation(dir, "plan"))
    );
}

// AC-3: no decision waits; the file is written over an older one.
#[test]
fn nothing_waits() {
    // A fixture with no open ticket.
    let repo = common::stapel_repo();
    let dir = repo.path();
    let out = stapel(dir).arg("inbox").assert().code(0).get_output().clone();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "inbox: nothing waits\n");
    assert_eq!(common::read(dir, ".stapel/inbox.md"), "# Inbox\n\nNothing waits.\n");
    stapel(dir).args(["inbox", "extra"]).assert().code(2);

    // A ticket that owes a confirmation, then nothing: the old file is written over.
    let repo = repo_with_ticket();
    let dir = repo.path();
    on_ticket_branch(dir);
    past_spec_review(dir);
    assert!(inbox_text(dir).contains("### ABC-1 confirmation\n"));
    for id in ["spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", "ABC-1", id]).assert().success();
    }
    let out = stapel(dir).arg("inbox").assert().code(0).get_output().clone();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "inbox: nothing waits\n");
    assert_eq!(common::read(dir, ".stapel/inbox.md"), "# Inbox\n\nNothing waits.\n");
}
