//! STP-41: the hook dialog for a write into the guards folder (step 7).

mod common;

use common::{git_repo, read, stapel};
use serde_json::json;

// AC-12: the main agent's write into the guards folder in a dialog mode gets the permission
// dialog, whatever `always_writable` says.
#[test]
fn guards_folder_edit_asks() {
    let repo = git_repo();
    let root = repo.path();
    stapel(root)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let config = read(root, ".stapel/stapel.toml");
    let config = config.replace(
        "always_writable = [\".stapel/\", \"docs/\"]",
        "always_writable = [\".stapel/\", \"docs/\", \"checks/\"]",
    );
    assert!(config.contains("\"checks/\""));
    let config = if config.lines().any(|l| l.trim() == "[check]") {
        config
    } else {
        format!("{config}\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n")
    };
    std::fs::write(
        root.join(".stapel/stapel.toml"),
        format!("{config}\n[guards]\ndir = \"checks\"\n"),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("checks")).unwrap();
    std::fs::write(root.join("checks/base.txt"), "a\n").unwrap();
    std::os::unix::fs::symlink(root.join("checks"), root.join("link")).unwrap();
    // A case variant is the same folder on a case-insensitive file system; on a case-sensitive
    // one a symlink with a case-variant name stands for it.
    let probe = root.join("CaseProbe.tmp");
    std::fs::write(&probe, "x").unwrap();
    let case_sensitive = !root.join("caseprobe.tmp").exists();
    std::fs::remove_file(&probe).unwrap();
    if case_sensitive {
        std::os::unix::fs::symlink(root.join("checks"), root.join("CHECKS")).unwrap();
    }
    let paths = [
        "checks/x".to_string(),
        "checks/base.txt".to_string(),
        root.join("checks/x").display().to_string(),
        "./a/../checks/x".to_string(),
        "link/x".to_string(),
        "CHECKS/x".to_string(),
    ];
    for mode in ["default", "acceptEdits", "auto", "plan"] {
        for path in &paths {
            let calls = [
                ("Write", json!({ "file_path": path, "content": "new\n" })),
                (
                    "Edit",
                    json!({ "file_path": path, "old_string": "a", "new_string": "b" }),
                ),
                (
                    "MultiEdit",
                    json!({ "file_path": path, "edits": [
                        { "old_string": "a", "new_string": "b" },
                    ] }),
                ),
                (
                    "NotebookEdit",
                    json!({ "notebook_path": path, "new_source": "x" }),
                ),
            ];
            for (tool, input) in calls {
                let payload = json!({
                    "session_id": "test",
                    "hook_event_name": "PreToolUse",
                    "cwd": root,
                    "permission_mode": mode,
                    "tool_name": tool,
                    "tool_input": input,
                });
                let out = stapel(root)
                    .args(["hook", "pre-tool-use"])
                    .write_stdin(payload.to_string())
                    .assert()
                    .success()
                    .get_output()
                    .stdout
                    .clone();
                let what = format!("{tool} {path} in {mode}");
                let v: serde_json::Value = serde_json::from_slice(&out)
                    .unwrap_or_else(|e| panic!("no JSON answer for {what}: {e}"));
                let h = &v["hookSpecificOutput"];
                assert_eq!(h["permissionDecision"], "ask", "{what}");
                assert_eq!(
                    h["permissionDecisionReason"],
                    "a write into the guards folder checks: the person confirms it in this dialog",
                    "{what}"
                );
            }
        }
    }
    // The ask changes nothing on disk.
    assert_eq!(read(root, "checks/base.txt"), "a\n");
    assert!(!root.join("checks/x").exists());
}

// AC-13: a subagent and the modes without the dialog are denied; a broken `stapel.toml` and a path
// beside the folder follow the old write rules.
#[test]
fn guards_folder_refusals() {
    let repo = git_repo();
    let root = repo.path();
    stapel(root)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let config = read(root, ".stapel/stapel.toml");
    let config = config.replace(
        "always_writable = [\".stapel/\", \"docs/\"]",
        "always_writable = [\".stapel/\", \"docs/\", \"checks/\"]",
    );
    assert!(config.contains("\"checks/\""));
    let config = if config.lines().any(|l| l.trim() == "[check]") {
        config
    } else {
        format!("{config}\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n")
    };
    let good = format!("{config}\n[guards]\ndir = \"checks\"\n");
    std::fs::write(root.join(".stapel/stapel.toml"), &good).unwrap();
    std::fs::create_dir_all(root.join("checks")).unwrap();
    std::fs::write(root.join("checks/base.txt"), "a\n").unwrap();

    // A denial is exit code 2 with the reason on standard error; the reason is returned.
    let deny = |path: &str, mode: &str, agent: Option<&str>| -> String {
        let mut payload = json!({
            "session_id": "test",
            "hook_event_name": "PreToolUse",
            "cwd": root,
            "permission_mode": mode,
            "tool_name": "Write",
            "tool_input": { "file_path": path, "content": "new\n" },
        });
        if let Some(agent) = agent {
            payload["agent_id"] = json!(agent);
        }
        let out = stapel(root)
            .args(["hook", "pre-tool-use"])
            .write_stdin(payload.to_string())
            .assert()
            .code(2)
            .get_output()
            .stderr
            .clone();
        let text = String::from_utf8_lossy(&out).to_string();
        text.strip_prefix("stapel: ")
            .unwrap_or_else(|| panic!("no stapel prefix for {path} in {mode}: {text}"))
            .trim_end()
            .to_string()
    };

    // A subagent is denied in every mode.
    for mode in ["default", "acceptEdits", "auto", "plan"] {
        let reason = deny("checks/x", mode, Some("agent-1"));
        assert_eq!(
            reason,
            "only the main agent writes into the guards folder checks, through the person's dialog",
            "subagent in {mode}"
        );
    }
    // The main agent without the dialog is denied.
    for mode in ["bypassPermissions", "dontAsk"] {
        let reason = deny("checks/x", mode, None);
        assert_eq!(
            reason,
            format!(
                "a write into the guards folder checks needs the permission dialog, which Claude \
                 Code does not show in permission mode \"{mode}\""
            ),
            "{mode}"
        );
    }
    // A path beside the folder follows the old rules: no ask, no guards text.
    let reason = deny("checksx/f", "default", None);
    assert!(
        reason.starts_with("writing checksx/f is denied: the build is not allowed"),
        "{reason}"
    );
    assert!(!reason.contains("guards folder"), "{reason}");
    // A broken stapel.toml has no guards folder: the old rules stand.
    std::fs::write(
        root.join(".stapel/stapel.toml"),
        format!("{good}\n[guards\n"),
    )
    .unwrap();
    for (mode, agent) in [
        ("default", None),
        ("bypassPermissions", None),
        ("default", Some("agent-1")),
    ] {
        let reason = deny("checks/x", mode, agent);
        assert!(
            reason.starts_with("writing checks/x is denied:") && reason.contains("is invalid"),
            "{reason}"
        );
        assert!(!reason.contains("guards folder"), "{reason}");
    }
    assert!(!root.join("checks/x").exists());
}

// XR1-2 (AC-12): a `[check] cargo` pin that fails validation does not hide the guards folder from
// the hook: the dialog stays.
#[test]
fn a_bad_cargo_pin_keeps_the_dialog() {
    let repo = git_repo();
    let root = repo.path();
    stapel(root)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let config = read(root, ".stapel/stapel.toml").replace(
        "always_writable = [\".stapel/\", \"docs/\"]",
        "always_writable = [\".stapel/\", \"docs/\", \"checks/\"]",
    );
    assert!(config.contains("\"checks/\""));
    let pin = "cargo = \"/nonexistent/stapel-test/cargo\"";
    let config = if config.lines().any(|l| l.trim() == "[check]") {
        config.replacen("[check]\n", &format!("[check]\n{pin}\n"), 1)
    } else {
        format!("{config}\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n{pin}\n")
    };
    std::fs::write(
        root.join(".stapel/stapel.toml"),
        format!("{config}\n[guards]\ndir = \"checks\"\n"),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("checks")).unwrap();
    let payload = json!({
        "session_id": "test",
        "hook_event_name": "PreToolUse",
        "cwd": root,
        "permission_mode": "default",
        "tool_name": "Write",
        "tool_input": { "file_path": "checks/x", "content": "new\n" },
    });
    let out = stapel(root)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("a JSON answer");
    let h = &v["hookSpecificOutput"];
    assert_eq!(h["permissionDecision"], "ask");
    assert_eq!(
        h["permissionDecisionReason"],
        "a write into the guards folder checks: the person confirms it in this dialog"
    );
}

// XR1-4 (AC-12): a guards folder that is a link inside the repository is the folder its target
// names: a write through the link and a write to the target both ask.
#[test]
fn a_linked_guards_folder_asks() {
    let repo = git_repo();
    let root = repo.path();
    stapel(root)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let config = read(root, ".stapel/stapel.toml");
    let config = if config.lines().any(|l| l.trim() == "[check]") {
        config
    } else {
        format!("{config}\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n")
    };
    std::fs::write(
        root.join(".stapel/stapel.toml"),
        format!("{config}\n[guards]\ndir = \"tests/guards\"\n"),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("ci/guards")).unwrap();
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::os::unix::fs::symlink(root.join("ci/guards"), root.join("tests/guards")).unwrap();
    for path in ["tests/guards/check.sh", "ci/guards/check.sh"] {
        let payload = json!({
            "session_id": "test",
            "hook_event_name": "PreToolUse",
            "cwd": root,
            "permission_mode": "default",
            "tool_name": "Write",
            "tool_input": { "file_path": path, "content": "new\n" },
        });
        let out = stapel(root)
            .args(["hook", "pre-tool-use"])
            .write_stdin(payload.to_string())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let v: serde_json::Value = serde_json::from_slice(&out)
            .unwrap_or_else(|e| panic!("no JSON answer for {path}: {e}"));
        let h = &v["hookSpecificOutput"];
        assert_eq!(h["permissionDecision"], "ask", "{path}");
        assert_eq!(
            h["permissionDecisionReason"],
            "a write into the guards folder tests/guards: the person confirms it in this dialog",
            "{path}"
        );
    }
}
