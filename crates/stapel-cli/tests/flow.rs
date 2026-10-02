//! STP-2 AC-16: the command the guard puts into the dialog really runs after "yes".

mod common;

use common::{ask, repo_with_ticket, set_section, state_json};

/// Runs the replaced command the way Claude Code would after "yes": through `sh -c`, in the
/// agent's shell (CLAUDECODE set), with the built `stapel` on PATH.
fn run_after_yes(dir: &std::path::Path, command: &str) -> std::process::Output {
    let bin = assert_cmd::cargo::cargo_bin("stapel");
    let path = format!("{}:/usr/bin:/bin", bin.parent().unwrap().display());
    std::process::Command::new("/bin/sh")
        .args(["-c", command])
        .current_dir(dir)
        .env("PATH", path)
        .env("CLAUDECODE", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env(
            "GIT_CEILING_DIRECTORIES",
            std::env::temp_dir().canonicalize().unwrap(),
        )
        .output()
        .unwrap()
}

#[test]
fn grant_round_trip() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let (_, replaced) = ask(dir, "stapel ok spec");
    let out = run_after_yes(dir, &replaced);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("confirmed: ABC-1 spec"));
    assert_eq!(state_json(dir, "ABC-1")["confirmations"][0]["via"], "grant");
}

#[test]
fn grant_round_trip_refuses_after_edit() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let (_, replaced) = ask(dir, "stapel ok spec");
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "Edited between the dialog and the yes.",
    );
    let out = run_after_yes(dir, &replaced);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        state_json(dir, "ABC-1")["confirmations"],
        serde_json::json!([])
    );
}
