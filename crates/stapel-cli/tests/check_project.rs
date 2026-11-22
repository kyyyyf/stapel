//! STP-37: the project commands of `stapel check`.

mod common;

use common::{read, stapel, stapel_repo};
use std::path::Path;

/// Adds a `[check]` section with the lines to the repository's `stapel.toml`.
fn add_to_check(dir: &Path, lines: &str) {
    let text = format!(
        "{}\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n{lines}\n",
        read(dir, ".stapel/stapel.toml")
    );
    std::fs::write(dir.join(".stapel/stapel.toml"), text).unwrap();
}

/// Exit code and the text of stderr of `stapel check`.
fn check(dir: &Path) -> (i32, String) {
    let out = stapel(dir).arg("check").output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// AC-1: an empty or whitespace entry of `project` fails the load; a list of two commands loads.
#[test]
fn refuses_an_empty_command() {
    let refusal = "check.project entries must not be empty";
    for bad in [
        "project = [\"\"]",
        "project = [\"  \"]",
        "project = [\"true\", \"\"]",
    ] {
        let repo = stapel_repo();
        add_to_check(repo.path(), bad);
        let (code, err) = check(repo.path());
        assert_eq!(code, 2, "{bad}: {err}");
        assert!(err.contains(refusal), "{bad}: {err}");
    }
    let repo = stapel_repo();
    add_to_check(repo.path(), "project = [\"true\", \"echo two\"]");
    let (_, err) = check(repo.path());
    assert!(!err.contains("stapel.toml"), "{err}");
    assert!(!err.contains(refusal), "{err}");
}

// ---- AC-2, AC-4, AC-5, AC-14: running the commands ----

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "test-user")
        .env("GIT_AUTHOR_EMAIL", "test-user.invalid")
        .env("GIT_COMMITTER_NAME", "test-user")
        .env("GIT_COMMITTER_EMAIL", "test-user.invalid")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn commit(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", message]);
}

/// A repository with a one-crate workspace, ticket ABC-1 and one passing step, all committed; the
/// fake cargo is its `[check] cargo`, `timeout` its time limit and `extra` more `[check]` lines.
/// `wrap` is a shell line run before the fake for every call of it (it may use `$*`).
fn project_repo(extra: &str, timeout: u32, wrap: &str) -> (tempfile::TempDir, tempfile::TempDir) {
    use std::os::unix::fs::PermissionsExt;
    let repo = common::git_repo();
    let dir = repo.path();
    write(
        dir,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/tiny\"]\nresolver = \"2\"\n",
    );
    write(
        dir,
        "crates/tiny/Cargo.toml",
        "[package]\nname = \"tiny\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(dir, "crates/tiny/src/lib.rs", "pub fn one() {}\n");
    write(
        dir,
        "Cargo.lock",
        "version = 3\n\n[[package]]\nname = \"tiny\"\nversion = \"0.1.0\"\n",
    );
    write(dir, ".gitignore", "/target\n");
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    stapel(dir).args(["new", "First"]).assert().success();
    commit(dir, "initial");
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn needs_two() {\n    // fake: needs pub fn two\n}\n",
    );
    commit(dir, "ABC-1 step 1 RED: two is missing");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        "pub fn one() {}\npub fn two() {}\n",
    );
    commit(dir, "ABC-1 step 1 GREEN: two");

    let scripts = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(scripts.path().join("real")).unwrap();
    let real = common::fake_cargo(&scripts.path().join("real"));
    let path = scripts.path().join("cargo");
    std::fs::write(&path, format!("#!/bin/sh\n{wrap}\nexec {real} \"$@\"\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let text: String = read(dir, ".stapel/stapel.toml")
        .lines()
        .map(|l| {
            if l.starts_with("timeout_secs = ") {
                format!(
                    "timeout_secs = {timeout}\ncargo = \"{}\"\n{extra}\n",
                    path.display()
                )
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: the check keys");
    (repo, scripts)
}

/// Exit code and the text of stdout and stderr of `stapel check ABC-1` with the args.
fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = stapel(dir)
        .args(["check", "ABC-1"])
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// The last check record of the ticket's `runs.jsonl`.
fn last_record(dir: &Path) -> serde_json::Value {
    let text = read(dir, ".stapel/tickets/ABC-1/runs.jsonl");
    let line = text
        .lines()
        .rfind(|l| l.contains("\"kind\":\"check\""))
        .unwrap();
    serde_json::from_str(line).unwrap()
}

// AC-2, AC-14: the commands run in order in the check copy, after the suite; the record lists them,
// cut to 200 characters, and past 20 entries it says `project_truncated`.
#[test]
fn runs_the_commands() {
    let log_dir = tempfile::tempdir().unwrap();
    let log = log_dir.path().join("log");
    let one = format!("pwd >> {}; echo one >> {}", log.display(), log.display());
    let two = format!("echo two >> {}", log.display());
    let extra = format!("project = ['{one}', '{two}']");
    let (repo, _scripts) = project_repo(&extra, 3600, "");
    let dir = repo.path();
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    let lines: Vec<&str> = text.lines().collect();
    let at = |needle: &str| lines.iter().position(|l| l.trim() == needle).expect(&text);
    let (p1, p2) = (
        at(&format!("project: {one}: pass")),
        at(&format!("project: {two}: pass")),
    );
    assert!(p1 < p2, "{text}");
    let suite = lines.iter().position(|l| l.starts_with("suite")).expect(&text);
    assert!(at("level: head") < suite && suite < p1, "{text}");
    let logged = std::fs::read_to_string(&log).unwrap();
    let logged: Vec<&str> = logged.lines().collect();
    assert_eq!(&logged[1..], ["one", "two"], "{logged:?}");
    // The copy is removed after the check; its path is not the repository's.
    let root = std::fs::canonicalize(dir).unwrap();
    assert!(Path::new(logged[0]).is_absolute(), "{logged:?}");
    assert_ne!(Path::new(logged[0]), root, "{logged:?}");
    let record = last_record(dir);
    assert_eq!(
        record["project"],
        serde_json::json!([
            {"command": one.chars().take(200).collect::<String>(), "outcome": "pass"},
            {"command": two, "outcome": "pass"},
        ]),
        "{record}"
    );
    assert!(record.get("project_truncated").is_none(), "{record}");

    // 22 commands, the first of 300 characters.
    let long = format!(": {}", "x".repeat(298));
    assert_eq!(long.len(), 300);
    let mut list = vec![format!("'{long}'")];
    list.extend((1..22).map(|i| format!("'true # {i}'")));
    let (repo, _scripts) = project_repo(&format!("project = [{}]", list.join(", ")), 3600, "");
    let (code, text) = run(repo.path(), &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    let record = last_record(repo.path());
    let project = record["project"].as_array().expect("project");
    assert_eq!(project.len(), 20, "{record}");
    assert_eq!(project[0]["command"], long[..200], "{record}");
    assert_eq!(project[19]["command"], "true # 19", "{record}");
    assert_eq!(record["project_truncated"], true, "{record}");
}

// AC-2: a failing command prints its last 20 lines and fails the check; the close refuses it.
#[test]
fn a_failing_command_fails_the_check() {
    let bad = "i=1; while [ $i -le 30 ]; do echo out$i; i=$((i+1)); done; exit 3";
    let (repo, _scripts) = project_repo(&format!("project = ['{bad}']"), 3600, "");
    let dir = repo.path();
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim() == format!("project: {bad}: fail (exit 3)")),
        "{text}"
    );
    let has = |s: &str| text.lines().any(|l| l.trim() == s);
    assert!(has("out30") && has("out11"), "{text}");
    assert!(!has("out10") && !has("out1"), "{text}");
    assert!(text.contains("result: fail"), "{text}");
    let record = last_record(dir);
    assert_eq!(record["result"], "fail", "{record}");
    assert_eq!(record["project"][0]["outcome"], "fail", "{record}");
    let out = stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("current passing check"), "{err}");
}

// AC-4: the time limit kills a running command and its child; the commands not run, and all after
// a suite that used the limit, are `unverified: time limit`.
#[test]
fn time_limit_stops_a_command() {
    let marker_dir = tempfile::tempdir().unwrap();
    let marker = marker_dir.path().join("second-ran");
    let slow = "sleep 317 & sleep 318";
    let second = format!("touch {}", marker.display());
    let extra = format!("project = ['{slow}', '{second}']");
    let (repo, _scripts) = project_repo(&extra, 10, "");
    let started = std::time::Instant::now();
    let (code, text) = run(repo.path(), &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(60),
        "{text}"
    );
    let lines: Vec<String> = text.lines().map(|l| l.trim().to_string()).collect();
    assert!(
        lines.contains(&format!("project: {slow}: unverified: time limit")),
        "{text}"
    );
    assert!(
        lines.contains(&format!("project: {second}: unverified: time limit")),
        "{text}"
    );
    assert!(!marker.exists(), "{text}");
    assert!(text.contains("result: fail"), "{text}");
    let left = std::process::Command::new("pgrep")
        .args(["-f", "sleep 31[78]"])
        .output()
        .unwrap();
    assert!(
        left.stdout.is_empty(),
        "a child survived: {}",
        String::from_utf8_lossy(&left.stdout)
    );
    let record = last_record(repo.path());
    assert_eq!(record["project"][0]["outcome"], "unverified", "{record}");
    assert_eq!(record["project"][1]["outcome"], "unverified", "{record}");

    // A suite that uses the whole limit: no command runs.
    let first = format!("touch {}", marker.display());
    let (repo, _scripts) = project_repo(
        &format!("project = ['{first}']"),
        10,
        "case \"$*\" in *--workspace*) sleep 60 ;; esac",
    );
    let (code, text) = run(repo.path(), &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim() == format!("project: {first}: unverified: time limit")),
        "{text}"
    );
    assert!(!marker.exists(), "{text}");
}

// AC-5: at level `steps` no project command runs.
#[test]
fn steps_level_skips() {
    let marker_dir = tempfile::tempdir().unwrap();
    let marker = marker_dir.path().join("ran");
    let one = format!("touch {}", marker.display());
    let extra = format!("project = ['{one}', 'echo two']");
    let (repo, _scripts) = project_repo(&extra, 3600, "");
    let (code, text) = run(repo.path(), &["--level", "steps"]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim() == "project: skipped: level steps"),
        "{text}"
    );
    assert!(!text.contains(&format!("project: {one}")), "{text}");
    assert!(!marker.exists(), "{text}");
}
