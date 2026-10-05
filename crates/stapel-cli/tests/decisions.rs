//! STP-3 AC-1: `ok` and `close` record their decisions.

mod common;

use common::{repo_with_ticket, stapel, state_json};
use predicates::str::contains;
use std::path::Path;

fn decisions(dir: &Path) -> Vec<serde_json::Value> {
    let path = dir.join(".stapel/tickets/ABC-1/decisions.jsonl");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn ok_and_close_append_a_decision() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    stapel(dir)
        .args(["close", "--reason", "merged"])
        .assert()
        .success();

    let d = decisions(dir);
    assert_eq!(d.len(), 2);
    assert_eq!(d[0]["v"], 1);
    assert_eq!(d[0]["action"], "ok");
    assert_eq!(d[0]["section"], "spec");
    assert_eq!(d[0]["by"], "test-user");
    assert_eq!(d[0]["via"], "terminal");
    assert_eq!(
        d[0]["hash"],
        state_json(dir, "ABC-1")["confirmations"][0]["hash"]
    );
    assert!(d[0]["at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(d[1]["action"], "close");
    assert_eq!(d[1]["reason"], "merged");
    let ids: Vec<&str> = d.iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert!(
        ids.iter().all(|i| i.starts_with("d-") && i.len() == 14),
        "{ids:?}"
    );
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn refusal_appends_nothing() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "review"]).assert().code(1);
    stapel(dir)
        .args(["close", "--reason", " "])
        .assert()
        .code(1);
    assert!(decisions(dir).is_empty());
}

#[test]
fn append_failure_is_reported() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    // A folder where the journal file should be makes the append fail.
    std::fs::create_dir_all(dir.join(".stapel/tickets/ABC-1/decisions.jsonl")).unwrap();
    stapel(dir)
        .args(["ok", "spec"])
        .assert()
        .code(1)
        .stdout(contains("confirmed: ABC-1 spec"))
        .stderr(contains("warning: decision not recorded:"));
    assert_eq!(
        state_json(dir, "ABC-1")["confirmations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
