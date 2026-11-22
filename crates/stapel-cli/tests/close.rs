//! STP-2 AC-14: `stapel close`.

mod common;

use common::{confirm_sections, repo_with_ticket, set_section, stapel, state_json};
use predicates::str::contains;

#[test]
fn records_closed_fact() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm_sections(dir, "ABC-1");
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
    confirm_sections(dir, "ABC-1");
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
    confirm_sections(dir, "ABC-1");
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

// ---- STP-2 AC-16: grants from the permission dialog ----

use common::{ask, token_of};

#[test]
fn accepts_valid_grant_once() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm_sections(dir, "ABC-1");
    let (reason, replaced) = ask(dir, "stapel close --reason merged");
    assert!(
        reason.contains("ABC-1") && reason.contains("merged"),
        "{reason}"
    );
    assert!(
        replaced.contains("close ABC-1 --reason=merged --grant "),
        "{replaced}"
    );
    let token = token_of(&replaced);
    let run = || {
        stapel(dir)
            .args(["close", "ABC-1", "--reason", "merged", "--grant", &token])
            .env("CLAUDECODE", "1")
            .assert()
    };
    run().success().stdout(contains("closed: ABC-1"));
    assert!(state_json(dir, "ABC-1").get("closed").is_some());
}

// ---- STP-36 AC-5: the close waits for the ok ----

#[test]
fn refuses_waiting_sections() {
    use common::{grants_dir, hook_bash};
    use predicates::prelude::PredicateBooleanExt;
    let repo = repo_with_ticket();
    let dir = repo.path();
    for id in ["spec", "design", "proof"] {
        stapel(dir).args(["ok", id]).assert().success();
    }
    // The Plan waits.
    let text = "stapel: plan wait for stapel ok ABC-1";
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "merged"])
        .assert()
        .code(1)
        .stderr(contains(text));
    assert!(state_json(dir, "ABC-1").get("closed").is_none());
    hook_bash(dir, "stapel close --reason merged", "auto")
        .code(2)
        .stderr(contains(text));
    assert!(
        !grants_dir(dir).exists() || std::fs::read_dir(grants_dir(dir)).unwrap().count() == 0
    );
    // The Spec waits too.
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "merged"])
        .assert()
        .code(1)
        .stderr(contains("spec").and(contains("wait for stapel ok ABC-1")));
    assert!(state_json(dir, "ABC-1").get("closed").is_none());
}

#[test]
fn refuses_ok_grant() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let (_, replaced) = ask(dir, "stapel ok spec");
    stapel(dir)
        .args([
            "close",
            "ABC-1",
            "--reason",
            "merged",
            "--grant",
            &token_of(&replaced),
        ])
        .env("CLAUDECODE", "1")
        .assert()
        .code(1);
    assert!(state_json(dir, "ABC-1").get("closed").is_none());
}
