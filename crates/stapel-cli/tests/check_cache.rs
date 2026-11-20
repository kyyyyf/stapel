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
