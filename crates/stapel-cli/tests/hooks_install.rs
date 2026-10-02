//! STP-1 AC-6: `stapel init` installs Claude Code hooks into `.claude/settings.json`.

mod common;

use common::{git_repo, read, stapel};
use predicates::str::contains;
use serde_json::{Value, json};
use std::path::Path;

const HOOK_COMMAND: &str = "stapel hook pre-tool-use";
const SETTINGS: &str = ".claude/settings.json";

fn settings(dir: &Path) -> Value {
    serde_json::from_str(&read(dir, SETTINGS)).unwrap()
}

fn stapel_entries(settings: &Value) -> Vec<&Value> {
    settings["hooks"]["PreToolUse"]
        .as_array()
        .map(|a| a.iter().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| {
            e["hooks"]
                .as_array()
                .is_some_and(|hs| hs.iter().any(|h| h["command"] == HOOK_COMMAND))
        })
        .collect()
}

#[test]
fn hooks_into_missing_settings() {
    let repo = git_repo();
    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success()
        .stdout(contains(SETTINGS));

    let s = settings(repo.path());
    let entries = stapel_entries(&s);
    assert_eq!(entries.len(), 1, "{s:#}");
    let matcher = entries[0]["matcher"].as_str().unwrap();
    for tool in ["Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"] {
        assert!(
            matcher.split('|').any(|m| m == tool),
            "{tool} not in {matcher}"
        );
    }
    assert_eq!(entries[0]["hooks"][0]["type"], "command");
}

// R-2
#[test]
fn hooks_merge_with_foreign_settings() {
    let repo = git_repo();
    let dir = repo.path();
    let foreign = json!({
        "permissions": { "allow": ["Bash(ls:*)"] },
        "model": "opus",
        "hooks": {
            "PreToolUse": [
                { "matcher": "Bash", "hooks": [{ "type": "command", "command": "other-guard" }] }
            ],
            "Stop": [ { "hooks": [{ "type": "command", "command": "notify" }] } ]
        }
    });
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    std::fs::write(
        dir.join(SETTINGS),
        serde_json::to_string_pretty(&foreign).unwrap(),
    )
    .unwrap();

    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();

    let s = settings(dir);
    assert_eq!(s["permissions"], foreign["permissions"]);
    assert_eq!(s["model"], foreign["model"]);
    assert_eq!(s["hooks"]["Stop"], foreign["hooks"]["Stop"]);
    let pre = s["hooks"]["PreToolUse"].as_array().unwrap();
    assert!(pre.contains(&foreign["hooks"]["PreToolUse"][0]), "{s:#}");
    assert_eq!(pre.len(), 2);
    assert_eq!(stapel_entries(&s).len(), 1);
}

#[test]
fn hooks_not_duplicated() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    // Force a second pass that has work to do elsewhere.
    std::fs::remove_file(dir.join(".stapel/allowlist.toml")).unwrap();
    stapel(dir).arg("init").assert().success();

    assert_eq!(stapel_entries(&settings(dir)).len(), 1);
}

// R-2: a settings file that is not JSON is never overwritten.
#[test]
fn refuses_broken_settings() {
    let repo = git_repo();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    std::fs::write(dir.join(SETTINGS), "{ not json").unwrap();

    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .code(1)
        .stderr(contains(SETTINGS));

    assert_eq!(read(dir, SETTINGS), "{ not json");
    assert!(
        !dir.join(".stapel").exists(),
        "init wrote files before failing"
    );
    assert!(!dir.join(".gitignore").exists());
}

// Question 3: the hook silently does nothing when `stapel` is not on PATH, so init warns.
#[test]
fn warns_when_stapel_missing_from_path() {
    let repo = git_repo();
    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .env("PATH", "/usr/bin:/bin")
        .assert()
        .success()
        .stderr(contains("PATH"));
}

#[test]
fn no_warning_when_stapel_on_path() {
    let repo = git_repo();
    let bin = tempfile::tempdir().unwrap();
    let fake = bin.path().join("stapel");
    std::fs::write(&fake, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!("{}:/usr/bin:/bin", bin.path().display());

    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .env("PATH", path)
        .assert()
        .success()
        .stderr(predicates::str::is_empty());
}

fn installed_command(dir: &Path) -> String {
    let s = settings(dir);
    stapel_entries(&s)[0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .to_string()
}

fn fake_stapel(script: &str) -> tempfile::TempDir {
    let bin = tempfile::tempdir().unwrap();
    let fake = bin.path().join("stapel");
    std::fs::write(&fake, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    bin
}

fn run_line(command: &str, path: &str) -> std::process::Output {
    std::process::Command::new("/bin/sh")
        .args(["-c", command])
        .env("PATH", path)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap()
}

// E-4: without `stapel` on PATH the installed hook line blocks the call instead of passing it.
#[test]
fn hook_command_fails_closed_without_stapel() {
    let repo = git_repo();
    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();

    let out = run_line(&installed_command(repo.path()), "/usr/bin:/bin");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("PATH"));
}

// E-4: with `stapel` on PATH the line hands over to `stapel hook pre-tool-use`.
#[test]
fn hook_command_runs_stapel_when_present() {
    let repo = git_repo();
    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let bin = fake_stapel("#!/bin/sh\necho \"$@\" >&2\nexit 7\n");

    let out = run_line(
        &installed_command(repo.path()),
        &format!("{}:/usr/bin:/bin", bin.path().display()),
    );
    assert_eq!(out.status.code(), Some(7));
    assert!(String::from_utf8_lossy(&out.stderr).contains("hook pre-tool-use"));
}
