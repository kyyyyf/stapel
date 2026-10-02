//! STP-2 AC-14: `stapel close`.

mod common;

use common::{repo_with_ticket, stapel, state_json};
use predicates::str::contains;

#[test]
fn records_closed_fact() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args(["close", "abc-1", "--reason", "merged"])
        .assert()
        .success()
        .stdout(contains("closed: ABC-1"));
    let closed = &state_json(dir, "ABC-1")["closed"];
    assert_eq!(closed["by"], "test-user");
    assert_eq!(closed["reason"], "merged");
    assert!(closed["at"].as_str().unwrap().ends_with('Z'));
    stapel(dir)
        .args(["status", "ABC-1"])
        .assert()
        .success()
        .stdout(contains("state: closed by test-user"));
}

#[test]
fn drops_hand_build_flag() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut state = state_json(dir, "ABC-1");
    state["build"] = serde_json::json!({"allowed": true, "by": "human"});
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/state.json"),
        state.to_string(),
    )
    .unwrap();
    stapel(dir)
        .args(["close", "--reason", "merged"])
        .assert()
        .success()
        .stdout(contains("removed: build.allowed (phase 0 hand permit)"));
    assert!(state_json(dir, "ABC-1").get("build").is_none());
}

#[test]
fn refuses_twice_and_without_reason() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for empty in ["", "   "] {
        stapel(dir)
            .args(["close", "--reason", empty])
            .assert()
            .code(1)
            .stderr(contains("reason"));
    }
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "merged"])
        .assert()
        .success();
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "again"])
        .assert()
        .code(1)
        .stderr(contains("closed"));
    assert_eq!(state_json(dir, "ABC-1")["closed"]["reason"], "merged");
}

#[test]
fn refuses_in_agent_shell_without_grant() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for var in ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"] {
        stapel(dir)
            .args(["close", "--reason", "merged"])
            .env(var, "")
            .assert()
            .code(1)
            .stderr(contains("person"));
    }
    assert!(state_json(dir, "ABC-1").get("closed").is_none());
}
