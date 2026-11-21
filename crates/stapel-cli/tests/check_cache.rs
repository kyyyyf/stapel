//! STP-35: the step cache of `stapel check` and the fields of the record that it reads.

mod common;

use common::{read, stapel};
use std::path::Path;
use tempfile::TempDir;

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

/// A repository with a one-crate workspace, ticket ABC-1 and one passing step (a RED, then its
/// GREEN), all committed. The fake cargo compiles nothing.
fn repo_with_a_step() -> TempDir {
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
    repo
}

/// `stapel check ABC-1` with the fake cargo first on the path; the exit code must be 0.
fn check(dir: &Path, args: &[&str]) {
    let bin = tempfile::tempdir().unwrap();
    common::fake_cargo(bin.path());
    let path = std::env::var_os("PATH").unwrap();
    let mut paths = vec![bin.path().to_path_buf()];
    paths.extend(std::env::split_paths(&path));
    let out = stapel(dir)
        .args(["check", "ABC-1"])
        .args(args)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The last record of the ticket's `runs.jsonl`.
fn last_record(dir: &Path) -> serde_json::Value {
    let text = read(dir, ".stapel/tickets/ABC-1/runs.jsonl");
    let line = text
        .lines()
        .rfind(|l| l.contains("\"kind\":\"check\""))
        .unwrap();
    serde_json::from_str(line).unwrap()
}

fn is_hash(v: &serde_json::Value) -> bool {
    v.as_str()
        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}

// AC-6: the record has the fields that a later check reads.
#[test]
fn record_fields() {
    let repo = repo_with_a_step();
    let dir = repo.path();
    check(dir, &[]);
    let record = last_record(dir);
    assert_eq!(record["level"], "head", "{record}");
    assert!(is_hash(&record["lock"]), "lock: {record}");
    let toolchain = record["toolchain"].as_str().unwrap_or("");
    assert!(toolchain.contains("cargo"), "toolchain: {record}");
    let plan = record["plan_hash"].as_str().unwrap_or("");
    assert!(
        plan.strip_prefix("sha256:").is_some_and(|h| h.len() == 64),
        "plan_hash: {record}"
    );
    assert_eq!(record["suite_exclude"], serde_json::json!([]), "{record}");
    let steps = record["steps"].as_array().unwrap();
    assert!(!steps.is_empty(), "{record}");
    for step in steps {
        assert!(is_hash(&step["tests_hash"]), "tests_hash: {step}");
    }

    check(dir, &["--level", "full"]);
    assert_eq!(last_record(dir)["level"], "full");
    check(dir, &["--level", "steps"]);
    assert_eq!(last_record(dir)["level"], "steps");
}

/// A `[check] cargo` stand-in that appends the arguments of every run to `log`, then runs the fake.
fn recording_fake(scripts: &Path, log: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(scripts.join("real")).unwrap();
    let real = common::fake_cargo(&scripts.join("real"));
    let path = scripts.join("cargo");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\necho \"$@\" >> {}\nexec {real} \"$@\"\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

/// Sets `[check] cargo` to `cargo` and commits it.
fn set_cargo_key(dir: &Path, cargo: &str) {
    let text = read(dir, ".stapel/stapel.toml");
    let text: String = text
        .lines()
        .map(|l| {
            if l.starts_with("timeout_secs = ") {
                format!("{l}\ncargo = \"{cargo}\"\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: the cargo key");
}

/// Runs `stapel check ABC-1` (exit 0 expected); returns its output and the number of cargo runs
/// of a step's tests (`--exact`) that the log holds, after emptying the log first.
fn run_logged(dir: &Path, log: &Path) -> (String, usize) {
    std::fs::write(log, "").unwrap();
    let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(0), "{text}");
    let runs = std::fs::read_to_string(log)
        .unwrap()
        .lines()
        .filter(|l| l.contains("--exact"))
        .count();
    (text, runs)
}

// AC-4: a second check runs no test of the step at RED and GREEN; a third keeps the first `from`.
#[test]
fn reuses_a_passing_step() {
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    set_cargo_key(dir, &recording_fake(scripts.path(), &log));

    let (text, runs) = run_logged(dir, &log);
    assert_eq!(runs, 3, "RED, GREEN and HEAD run at the first check: {text}");
    let first_id = last_record(dir)["id"].as_str().unwrap().to_string();

    let (text, runs) = run_logged(dir, &log);
    assert_eq!(runs, 1, "only HEAD runs: {text}");
    assert!(text.contains(&format!("cached from {first_id}")), "{text}");
    let second = last_record(dir);
    assert_eq!(second["steps"][0]["outcome"], "cached", "{second}");
    assert_eq!(second["steps"][0]["from"], first_id.as_str(), "{second}");

    let (text, runs) = run_logged(dir, &log);
    assert_eq!(runs, 1, "{text}");
    let third = last_record(dir);
    assert_ne!(third["id"], second["id"]);
    assert_eq!(third["steps"][0]["outcome"], "cached", "{third}");
    assert_eq!(third["steps"][0]["from"], first_id.as_str(), "{third}");
}

// AC-5: one part of the key differs, or the record is no source: the step runs at RED, GREEN, HEAD.
#[test]
fn a_changed_key_runs_the_step() {
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    set_cargo_key(dir, &recording_fake(scripts.path(), &log));
    let (text, runs) = run_logged(dir, &log);
    assert_eq!(runs, 3, "{text}");
    let original = last_record(dir);
    let path = dir.join(".stapel/tickets/ABC-1/runs.jsonl");
    let zeros = "0".repeat(64);

    type Edit = Box<dyn Fn(&mut serde_json::Value)>;
    let z = zeros.clone();
    let z2 = zeros.clone();
    let variants: Vec<(&str, Option<Edit>)> = vec![
        ("unchanged", None),
        ("green", Some(Box::new(|r| r["steps"][0]["green"] = "1".repeat(40).into()))),
        ("red", Some(Box::new(|r| r["steps"][0]["red"] = "2".repeat(40).into()))),
        ("tests_hash", Some(Box::new(move |r| r["steps"][0]["tests_hash"] = z.clone().into()))),
        ("lock", Some(Box::new(move |r| r["lock"] = z2.clone().into()))),
        ("toolchain", Some(Box::new(|r| r["toolchain"] = "another cargo".into()))),
        ("tool", Some(Box::new(|r| r["tool"] = "stapel 0.0.0-other".into()))),
        ("fail", Some(Box::new(|r| r["result"] = "fail".into()))),
        ("no-red", Some(Box::new(|r| r["steps"][0]["outcome"] = "no-red".into()))),
        (
            "no tests_hash",
            Some(Box::new(|r| {
                r["steps"][0].as_object_mut().unwrap().remove("tests_hash");
            })),
        ),
        (
            "no lock",
            Some(Box::new(|r| {
                r.as_object_mut().unwrap().remove("lock");
            })),
        ),
        ("ticket", Some(Box::new(|r| r["ticket"] = "ABC-2".into()))),
    ];

    for (name, edit) in &variants {
        let mut record = original.clone();
        if let Some(edit) = edit {
            edit(&mut record);
        }
        std::fs::write(&path, format!("{record}\n")).unwrap();
        let (text, runs) = run_logged(dir, &log);
        if edit.is_none() {
            assert_eq!(runs, 1, "{name}: {text}");
            assert!(text.contains("cached from"), "{name}: {text}");
        } else {
            assert_eq!(runs, 3, "{name}: {text}");
            assert!(!text.contains("cached from"), "{name}: {text}");
            assert_ne!(last_record(dir)["steps"][0]["outcome"], "cached", "{name}");
        }
    }

    // A malformed line is no source and does not stop the check.
    std::fs::write(&path, format!("not json\n{original}\n")).unwrap();
    let (text, runs) = run_logged(dir, &log);
    assert_eq!(runs, 1, "{text}");
    // No record at all: nothing is cached.
    std::fs::write(&path, "").unwrap();
    let (text, runs) = run_logged(dir, &log);
    assert_eq!(runs, 3, "{text}");
}

// AC-5, XR2-1: a later RED that changes the lock file at HEAD makes the earlier steps run again:
// `Cargo.lock` under cargo, a nested `package-lock.json` under junit.
#[test]
fn a_changed_lock_file_runs_the_step() {
    // cargo
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    set_cargo_key(dir, &recording_fake(scripts.path(), &log));
    let (text, runs) = run_logged(dir, &log);
    assert_eq!(runs, 3, "{text}");

    // Control: a second step that leaves the lock file alone keeps step 1 cached.
    write(
        dir,
        "crates/tiny/tests/second.rs",
        "#[test]\nfn needs_three() {\n    // fake: needs pub fn three\n}\n",
    );
    commit(dir, "ABC-1 step 2 RED: three is missing");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        "pub fn one() {}\npub fn two() {}\npub fn three() {}\n",
    );
    commit(dir, "ABC-1 step 2 GREEN: three");
    let (text, _) = run_logged(dir, &log);
    assert_eq!(last_record(dir)["steps"][0]["outcome"], "cached", "{text}");

    // A third step whose RED changes `Cargo.lock`: steps 1 and 2 run again.
    write(
        dir,
        "crates/tiny/tests/third.rs",
        "#[test]\nfn needs_four() {\n    // fake: needs pub fn four\n}\n",
    );
    write(
        dir,
        "Cargo.lock",
        "version = 3\n\n[[package]]\nname = \"tiny\"\nversion = \"0.1.1\"\n",
    );
    commit(dir, "ABC-1 step 3 RED: four is missing");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        "pub fn one() {}\npub fn two() {}\npub fn three() {}\npub fn four() {}\n",
    );
    commit(dir, "ABC-1 step 3 GREEN: four");
    let (text, _) = run_logged(dir, &log);
    let record = last_record(dir);
    assert_eq!(record["steps"][0]["outcome"], "pass", "{text}");
    assert_eq!(record["steps"][1]["outcome"], "pass", "{text}");
    assert!(!text.contains("cached from"), "{text}");

    // junit
    let repo = common::junit_repo();
    let dir = repo.dir();
    let ok = |repo: &common::JunitRepo| {
        let out = stapel(repo.dir()).args(["check", "ABC-1"]).output().unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.status.code(), Some(0), "{text}");
        (text, last_record(repo.dir()))
    };
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok1.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok1.ts", "export const ok = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one");
    let (text, record) = ok(&repo);
    assert_eq!(record["steps"][0]["outcome"], "pass", "{text}");

    write(
        dir,
        "__tests__/y.test.ts",
        "it(\"two\", () => { /* needs src/ok2.ts */ });\n",
    );
    commit(dir, "ABC-1 step 2 RED: two");
    write(dir, "src/ok2.ts", "export const ok = 2;\n");
    commit(dir, "ABC-1 step 2 GREEN: two");
    let (text, record) = ok(&repo);
    assert_eq!(record["steps"][0]["outcome"], "cached", "{text}");
    assert_eq!(record["steps"][1]["outcome"], "pass", "{text}");

    // The lock file is no test: step 3 itself ends `red-changes-code`; steps 1 and 2 are the subject.
    write(
        dir,
        "__tests__/z.test.ts",
        "it(\"three\", () => { /* needs src/ok3.ts */ });\n",
    );
    write(dir, "web/package-lock.json", "{\"lockfileVersion\": 3}\n");
    commit(dir, "ABC-1 step 3 RED: three");
    write(dir, "src/ok3.ts", "export const ok = 3;\n");
    commit(dir, "ABC-1 step 3 GREEN: three");
    let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let record = last_record(dir);
    assert_eq!(record["steps"][0]["outcome"], "pass", "{text}");
    assert_eq!(record["steps"][1]["outcome"], "pass", "{text}");
    assert!(!text.contains("cached from"), "{text}");
}

// AC-9, XR2-2: a cached step whose test fails at HEAD runs whole and is not `cached`.
#[test]
fn a_cached_step_failing_at_head_runs_whole() {
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    set_cargo_key(dir, &recording_fake(scripts.path(), &log));
    let (text, runs) = run_logged(dir, &log);
    assert_eq!(runs, 3, "{text}");

    // A later commit takes `two` away: the test of step 1 fails at HEAD.
    write(dir, "crates/tiny/src/lib.rs", "pub fn one() {}\n");
    commit(dir, "ABC-1: drop two");

    std::fs::write(&log, "").unwrap();
    let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(!text.contains("cached from"), "{text}");
    let record = last_record(dir);
    assert_eq!(record["result"], "fail", "{record}");
    assert_ne!(record["steps"][0]["outcome"], "cached", "{record}");
    let runs = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .filter(|l| l.contains("--exact"))
        .count();
    assert_eq!(runs, 4, "the HEAD probe, then RED, GREEN and HEAD: {text}");
}
