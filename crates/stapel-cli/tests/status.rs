//! STP-2 AC-4, AC-6, AC-7, AC-10: `stapel status`.

mod common;

use common::{repo_with_ticket, set_section, snapshot, stapel, state_json};
use predicates::prelude::*;
use predicates::str::contains;
use std::path::Path;

fn write_state(dir: &Path, key: &str, v: &serde_json::Value) {
    std::fs::write(
        dir.join(format!(".stapel/tickets/{key}/state.json")),
        v.to_string(),
    )
    .unwrap();
}

#[test]
fn fresh_ticket_waits_for_spec() {
    let repo = repo_with_ticket();
    stapel(repo.path())
        .arg("status")
        .assert()
        .success()
        .stdout(contains("ticket: ABC-1 — First"))
        .stdout(contains("state: open"))
        .stdout(contains("waiting for: spec (owner: product)"))
        .stdout(contains("build: not allowed"));
}

#[test]
fn key_matched_without_case() {
    let repo = repo_with_ticket();
    stapel(repo.path())
        .args(["status", "abc-1"])
        .assert()
        .success()
        .stdout(contains("ticket: ABC-1"));
}

#[test]
fn without_key_needs_one_open_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["new", "Second"]).assert().success();
    stapel(dir)
        .arg("status")
        .assert()
        .code(1)
        .stderr(contains("ABC-1"))
        .stderr(contains("ABC-2"));
}

#[test]
fn legacy_ticket_is_not_open() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".stapel/tickets/ABC-9")).unwrap();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-9/state.json"),
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    )
    .unwrap();
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("ticket: ABC-1"));
    stapel(dir)
        .args(["status", "ABC-9"])
        .assert()
        .success()
        .stdout(contains("state: legacy (STP-1 format)"));
}

#[test]
fn refuses_ambiguous_case_variant_keys() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".stapel/tickets/abc-1")).unwrap();
    if !dir.join(".stapel/tickets/ABC-1/ticket.md").exists() {
        return; // case-insensitive file system: the two names are one folder
    }
    stapel(dir)
        .args(["status", "abc-1"])
        .assert()
        .code(1)
        .stderr(contains("differ only in case"));
}

#[test]
fn edit_makes_confirmation_stale_with_diff() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    set_section(dir, "ABC-1", "Spec", "The spec, edited.");
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains(
            "stale: spec — changed since confirmed by test-user at ",
        ))
        .stdout(contains("-The spec."))
        .stdout(contains("+The spec, edited."));
}

#[test]
fn revert_makes_it_fresh_again() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    set_section(dir, "ABC-1", "Spec", "The spec, edited.");
    set_section(dir, "ABC-1", "Spec", "The spec.\r\n");
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("stale:").not())
        .stdout(contains("waiting for: design"));
}

#[test]
fn upstream_change_makes_dependents_stale() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    stapel(dir).args(["ok", "design"]).assert().success();
    set_section(dir, "ABC-1", "Spec", "The spec, edited.");
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("stale: design — depends on spec, which changed"));
}

#[test]
fn missing_or_duplicate_section_is_stale() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    let path = dir.join(".stapel/tickets/ABC-1/ticket.md");
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, original.replace("## Spec\n", "## Specification\n")).unwrap();
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("stale: spec — section missing"));
    std::fs::write(&path, format!("{original}\n## Spec\n\nagain\n")).unwrap();
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("stale: spec — section duplicated"));
}

#[test]
fn removed_dependency_is_stale() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    let mut state = state_json(dir, "ABC-1");
    state["confirmations"][0]["depends_on"] = serde_json::json!({"gone": "sha256:00"});
    write_state(dir, "ABC-1", &state);
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains(
            "stale: spec — depends on gone, which is no longer configured",
        ));
}

#[test]
fn older_normal_form_is_reported() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    let mut state = state_json(dir, "ABC-1");
    state["confirmations"][0]["normal_form"] = serde_json::json!(0);
    write_state(dir, "ABC-1", &state);
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("stale: spec — confirmed under normal form 0"));
}

#[test]
fn waiting_for_is_first_unconfirmed() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("waiting for: design (owner: engineer)"));
    set_section(dir, "ABC-1", "Plan", "The plan.");
    for s in ["design", "proof", "plan"] {
        stapel(dir).args(["ok", s]).assert().success();
    }
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("waiting for: none"));
}

#[test]
fn build_line_names_the_granting_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for s in ["spec", "design", "proof"] {
        stapel(dir).args(["ok", s]).assert().success();
    }
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("build: allowed (by ABC-1)"));

    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut state = state_json(dir, "ABC-1");
    state["build"] = serde_json::json!({"allowed": true});
    write_state(dir, "ABC-1", &state);
    stapel(dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("build: allowed by hand (phase 0, ABC-1)"));
}

#[test]
fn exit_codes() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    set_section(dir, "ABC-1", "Spec", "edited");
    stapel(dir).arg("status").assert().code(0);
    std::fs::write(dir.join(".stapel/tickets/ABC-1/state.json"), "{ broken").unwrap();
    stapel(dir)
        .args(["status", "ABC-1"])
        .assert()
        .code(1)
        .stdout(contains("ticket: ABC-1"));
}

#[test]
fn never_writes() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    set_section(dir, "ABC-1", "Spec", "edited");
    let before = snapshot(dir);
    std::thread::sleep(std::time::Duration::from_millis(30));
    stapel(dir).arg("status").assert().success();
    stapel(dir).args(["status", "ABC-1"]).assert().success();
    assert_eq!(snapshot(dir), before);
}

#[test]
fn missing_ticket_md_is_reported() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::remove_file(dir.join(".stapel/tickets/ABC-1/ticket.md")).unwrap();
    stapel(dir)
        .arg("status")
        .assert()
        .code(1)
        .stdout(contains("ticket.md"));
}
