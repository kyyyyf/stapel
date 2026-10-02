//! STP-2 AC-4, AC-5: `stapel ok`.

mod common;

use common::{git_config, repo_with_ticket, set_section, stapel, state_json};
use predicates::str::contains;

fn confirmations(dir: &std::path::Path) -> Vec<serde_json::Value> {
    state_json(dir, "ABC-1")["confirmations"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn records_confirmation() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args(["ok", "spec"])
        .assert()
        .success()
        .stdout(contains("confirmed: ABC-1 spec (sha256:"));
    stapel(dir)
        .args(["ok", "abc-1", "design"])
        .assert()
        .success();

    let c = confirmations(dir);
    assert_eq!(c.len(), 2);
    assert_eq!(c[0]["section"], "spec");
    assert_eq!(c[0]["by"], "test-user");
    assert_eq!(c[0]["normal_form"], 1);
    assert_eq!(c[0]["via"], "terminal");
    assert_eq!(c[0]["text"], "The spec.");
    assert!(c[0]["hash"].as_str().unwrap().starts_with("sha256:"));
    let at = c[0]["at"].as_str().unwrap();
    assert!(at.ends_with('Z') && at.len() == 20, "{at}");
    assert_eq!(c[0]["depends_on"], serde_json::json!({}));
    // design depends on spec: the current spec hash is recorded with it.
    assert_eq!(c[1]["depends_on"]["spec"], c[0]["hash"]);
}

#[test]
fn keeps_text_only_on_latest() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    stapel(dir).args(["ok", "spec"]).assert().success();
    let c = confirmations(dir);
    assert!(c[0].get("text").is_none(), "{:?}", c[0]);
    assert_eq!(c[1]["text"], "The spec, revised.");
}

#[test]
fn confirming_twice_keeps_history() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    stapel(dir).args(["ok", "spec"]).assert().success();
    let c = confirmations(dir);
    assert_eq!(c.len(), 2);
    assert_eq!(c[0]["hash"], c[1]["hash"]);
}

#[test]
fn refuses_generated_and_unknown_sections() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for bad in ["review", "summary", "nope"] {
        stapel(dir)
            .args(["ok", bad])
            .assert()
            .code(1)
            .stderr(contains("spec"))
            .stderr(contains("design"));
    }
    assert!(confirmations(dir).is_empty());
}

#[test]
fn refuses_on_closed_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let path = dir.join(".stapel/tickets/ABC-1/state.json");
    let mut state = state_json(dir, "ABC-1");
    state["closed"] =
        serde_json::json!({"by": "x", "at": "2026-10-02T00:00:00Z", "reason": "done"});
    std::fs::write(&path, state.to_string()).unwrap();
    stapel(dir)
        .args(["ok", "ABC-1", "spec"])
        .assert()
        .code(1)
        .stderr(contains("closed"));
}

#[test]
fn refuses_without_identity() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let status = std::process::Command::new("git")
        .args(["config", "--unset", "user.name"])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
    stapel(dir)
        .args(["ok", "spec"])
        .assert()
        .code(1)
        .stderr(contains("user.name"));
    assert!(confirmations(dir).is_empty());
}

#[test]
fn refuses_blank_identity() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    git_config(dir, "user.name", "   ");
    stapel(dir)
        .args(["ok", "spec"])
        .assert()
        .code(1)
        .stderr(contains("user.name"));
}

#[test]
fn warns_on_name_with_space() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    git_config(dir, "user.name", "Some Person");
    stapel(dir)
        .args(["ok", "spec"])
        .assert()
        .success()
        .stderr(contains("tracked"));
    assert_eq!(confirmations(dir)[0]["by"], "Some Person");
}

#[test]
fn refuses_missing_dependency() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let path = dir.join(".stapel/tickets/ABC-1/ticket.md");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("## Spec\n", "## Specification\n");
    std::fs::write(&path, text).unwrap();
    // design depends on spec, whose heading is now missing.
    stapel(dir)
        .args(["ok", "design"])
        .assert()
        .code(1)
        .stderr(contains("spec"));
    stapel(dir)
        .args(["ok", "spec"])
        .assert()
        .code(1)
        .stderr(contains("missing"));
    assert!(confirmations(dir).is_empty());
}

#[test]
fn refuses_in_agent_shell_without_grant() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for (var, value) in [
        ("CLAUDECODE", "1"),
        ("CLAUDECODE", ""),
        ("CLAUDE_CODE_ENTRYPOINT", "cli"),
        ("CLAUDE_CODE_ENTRYPOINT", ""),
    ] {
        stapel(dir)
            .args(["ok", "spec"])
            .env(var, value)
            .assert()
            .code(1)
            .stderr(contains("person"))
            .stderr(contains("env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT"));
    }
    assert!(confirmations(dir).is_empty());
}

#[test]
fn resolves_single_open_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["new", "Second"]).assert().success();
    stapel(dir)
        .args(["ok", "spec"])
        .assert()
        .code(1)
        .stderr(contains("ABC-1"))
        .stderr(contains("ABC-2"));
    stapel(dir).args(["ok", "ABC-1", "spec"]).assert().success();
    // A legacy (STP-1 format) ticket is never the implicit target.
    std::fs::remove_dir_all(dir.join(".stapel/tickets/ABC-2")).unwrap();
    std::fs::create_dir_all(dir.join(".stapel/tickets/ABC-9")).unwrap();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-9/state.json"),
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    )
    .unwrap();
    stapel(dir).args(["ok", "spec"]).assert().success();
}

// ---- STP-2 AC-16: grants from the permission dialog ----

use common::{ask, grants_dir, token_of};

fn ok_with_grant(dir: &std::path::Path, token: &str) -> assert_cmd::assert::Assert {
    stapel(dir)
        .args(["ok", "ABC-1", "spec", "--grant", token])
        .env("CLAUDECODE", "1")
        .assert()
}

#[test]
fn accepts_valid_grant_once() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let (_, replaced) = ask(dir, "stapel ok spec");
    let token = token_of(&replaced);
    ok_with_grant(dir, &token)
        .success()
        .stdout(contains("confirmed: ABC-1 spec"));
    assert_eq!(confirmations(dir)[0]["via"], "grant");
    ok_with_grant(dir, &token).code(1).stderr(contains("grant"));
    assert_eq!(confirmations(dir).len(), 1);
    assert_eq!(std::fs::read_dir(grants_dir(dir)).unwrap().count(), 0);
}

#[test]
fn refuses_expired_or_mismatched_grant() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    ok_with_grant(dir, "00112233445566778899aabbccddeeff")
        .code(1)
        .stderr(contains("changed since the dialog"));
    let (_, replaced) = ask(dir, "stapel ok spec");
    for entry in std::fs::read_dir(grants_dir(dir)).unwrap() {
        std::fs::write(entry.unwrap().path(), r#"{"expires_at":1}"#).unwrap();
    }
    ok_with_grant(dir, &token_of(&replaced))
        .code(1)
        .stderr(contains("expired"));
    assert!(confirmations(dir).is_empty());
}

#[test]
fn refuses_grant_after_section_edit() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let (_, replaced) = ask(dir, "stapel ok spec");
    set_section(dir, "ABC-1", "Spec", "Rewritten while the dialog was open.");
    ok_with_grant(dir, &token_of(&replaced))
        .code(1)
        .stderr(contains("changed since the dialog"));
    assert!(confirmations(dir).is_empty());
}

#[test]
fn ignores_rewritten_grant_file() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let (_, replaced) = ask(dir, "stapel ok spec");
    set_section(dir, "ABC-1", "Spec", "Swapped text.");
    let fake = serde_json::json!({
        "expires_at": u64::MAX,
        "key": "ABC-1",
        "action": "ok",
        "section": "spec",
        "section_hash": "anything",
    });
    for entry in std::fs::read_dir(grants_dir(dir)).unwrap() {
        std::fs::write(entry.unwrap().path(), fake.to_string()).unwrap();
    }
    ok_with_grant(dir, &token_of(&replaced)).code(1);
    assert!(confirmations(dir).is_empty());
}
