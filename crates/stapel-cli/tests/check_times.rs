//! STP-45: the times, the log and the run settings of `stapel check` with the cargo runner.

mod common;

use common::{read, stapel, stapel_repo};
use std::path::Path;

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
fn repo_with_a_step() -> tempfile::TempDir {
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

/// Sets `[check] cargo` to `cargo` and adds the `extra` lines to `[check]`; commits it.
fn configure(dir: &Path, cargo: &str, extra: &str) {
    let text = read(dir, ".stapel/stapel.toml");
    let text: String = text
        .lines()
        .map(|l| {
            if l.starts_with("timeout_secs = ") {
                format!("{l}\ncargo = \"{cargo}\"\n{extra}\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: the check keys");
}

/// The arguments of every cargo run of `stapel check ABC-1 --level head` (exit 0 expected), one
/// line per run, for a repository whose `[check]` section has the extra lines.
fn logged_runs(extra: &str) -> Vec<String> {
    let repo = repo_with_a_step();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    configure(repo.path(), &recording_fake(scripts.path(), &log), extra);
    std::fs::write(&log, "").unwrap();
    let out = stapel(repo.path())
        .args(["check", "ABC-1", "--level", "head"])
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(0), "{extra}: {text}");
    std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("--version"))
        .map(str::to_string)
        .collect()
}

// AC-11: `jobs` gives `-j` after `--locked` to every cargo run, `test_threads` gives
// `--test-threads` right after `--` to every run that runs tests; no keys, the old arguments.
#[test]
fn run_settings() {
    let steps = |runs: &[String]| -> Vec<String> {
        runs.iter().filter(|l| l.contains("--exact")).cloned().collect()
    };
    let suites = |runs: &[String]| -> Vec<String> {
        runs.iter().filter(|l| l.contains("--workspace")).cloned().collect()
    };

    let runs = logged_runs("jobs = 3\ntest_threads = 2");
    let step_runs = steps(&runs);
    assert_eq!(step_runs.len(), 3, "{runs:?}");
    for run in &step_runs {
        assert_eq!(
            run,
            "test --locked -j 3 -p tiny --test basic -- --test-threads=2 --exact needs_two"
        );
    }
    assert_eq!(
        suites(&runs),
        ["test --workspace --locked -j 3 -- --test-threads=2"],
        "{runs:?}"
    );

    let runs = logged_runs("jobs = 3\ntest_threads = 2\nsuite_exclude = [\"slow\", \"other\"]");
    assert_eq!(
        suites(&runs),
        ["test --workspace --locked -j 3 -- --test-threads=2 --skip slow --skip other"],
        "{runs:?}"
    );

    let runs = logged_runs("");
    for run in steps(&runs) {
        assert_eq!(run, "test --locked -p tiny --test basic -- --exact needs_two");
    }
    assert_eq!(suites(&runs), ["test --workspace --locked"], "{runs:?}");

    let runs = logged_runs("jobs = 3");
    for run in steps(&runs) {
        assert_eq!(
            run,
            "test --locked -j 3 -p tiny --test basic -- --exact needs_two"
        );
    }
    let runs = logged_runs("test_threads = 2\nsuite_exclude = [\"slow\"]");
    for run in steps(&runs) {
        assert_eq!(
            run,
            "test --locked -p tiny --test basic -- --test-threads=2 --exact needs_two"
        );
    }
    assert_eq!(
        suites(&runs),
        ["test --workspace --locked -- --test-threads=2 --skip slow"],
        "{runs:?}"
    );
}

// AC-12: out-of-range keys fail at load with exit 1; the junit runner refuses them; a text value
// is a TOML error.
#[test]
fn refuses_bad_settings() {
    let load = |extra: &str| -> (i32, String) {
        let repo = stapel_repo();
        add_to_check(repo.path(), extra);
        let out = stapel(repo.path()).arg("status").output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    for (line, key, max) in [
        ("jobs = 0", "jobs", 1024),
        ("jobs = 1025", "jobs", 1024),
        ("test_threads = 0", "test_threads", 1024),
        ("test_threads = 1025", "test_threads", 1024),
        ("progress_secs = 0", "progress_secs", 86400),
        ("progress_secs = 86401", "progress_secs", 86400),
    ] {
        let (code, err) = load(line);
        assert_eq!(code, 1, "{line}: {err}");
        assert!(
            err.contains(&format!("check.{key} must be a whole number from 1 to {max}")),
            "{line}: {err}"
        );
    }
    for good in [
        "jobs = 1024",
        "jobs = 1",
        "test_threads = 1024",
        "progress_secs = 86400",
        "progress_secs = 1",
    ] {
        let (code, err) = load(good);
        assert_eq!(code, 0, "{good}: {err}");
    }

    let junit = |extra: &str| -> (i32, String) {
        let repo = stapel_repo();
        let text = format!(
            "{}\n[check]\ntimeout_secs = 3600\nrunner = \"junit\"\ncommand = \"sh run.sh {{file}}\"\nreport = \"out.xml\"\n{extra}\n",
            read(repo.path(), ".stapel/stapel.toml")
        );
        std::fs::write(repo.path().join(".stapel/stapel.toml"), text).unwrap();
        let out = stapel(repo.path()).arg("status").output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    for key in ["jobs", "test_threads", "progress_secs"] {
        let (code, err) = junit(&format!("{key} = 2"));
        assert_eq!(code, 1, "{key}: {err}");
        assert!(
            err.contains(&format!("check.{key} needs the cargo runner")),
            "{key}: {err}"
        );
    }

    let (code, err) = load("jobs = \"x\"");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("stapel.toml"), "{err}");
    assert!(!err.contains("must be a whole number"), "{err}");
}

/// Adds a `[check]` section with the lines to the repository's `stapel.toml`.
fn add_to_check(dir: &Path, lines: &str) {
    let text = format!(
        "{}\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n{lines}\n",
        read(dir, ".stapel/stapel.toml")
    );
    std::fs::write(dir.join(".stapel/stapel.toml"), text).unwrap();
}
