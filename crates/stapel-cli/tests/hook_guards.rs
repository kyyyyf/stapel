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
