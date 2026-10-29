//! STP-25: the dialog page of `stapel ok KEY` and its section lines.

mod common;

use common::{ask, repo_with_ticket, set_section, stapel, state_json};
use std::path::Path;

const ANSWER: &str = "Answer Yes or No; do not choose \"don't ask again\".";

fn confirm(dir: &Path, section: &str) {
    stapel(dir)
        .args(["ok", "ABC-1", section])
        .assert()
        .success();
}

#[test]
fn section_lines() {
    // Spec changed, design stale by its dependency, proof and plan never confirmed.
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm(dir, "spec");
    confirm(dir, "design");
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert_eq!(
        reason,
        format!(
            "stapel: confirm 4 waiting sections of ABC-1; runs: stapel\n\
             spec: 2 lines changed (+1 -1)\n\
             design: unchanged; depends on spec, which changed\n\
             proof: new, 1 line\n\
             plan: new, 1 line\n\
             {ANSWER}"
        )
    );

    // A confirmation without stored text shows the section as new.
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "line one\nline two\nline three");
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    let path = dir.join(".stapel/tickets/ABC-1/state.json");
    let mut state = state_json(dir, "ABC-1");
    for c in state["confirmations"].as_array_mut().unwrap() {
        if c["section"] == "plan" {
            c.as_object_mut().unwrap().remove("text");
        }
    }
    std::fs::write(path, state.to_string()).unwrap();
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("\nplan: new, 3 lines; depends on spec, which changed\n"),
        "{reason}"
    );

    // A re-wrapped Spec paragraph: four changed lines, and no Criteria part.
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Spec", "keep one\nkeep two");
    confirm(dir, "spec");
    set_section(dir, "ABC-1", "Spec", "keep one keep\ntwo");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("\nspec: 4 lines changed (+2 -2)\n"),
        "{reason}"
    );
    assert!(!reason.contains("Criteria:"), "{reason}");
    assert!(
        reason.starts_with("stapel: confirm 4 waiting sections of ABC-1; runs: stapel\n"),
        "{reason}"
    );
    assert!(reason.ends_with(&format!("\n{ANSWER}")), "{reason}");
}
