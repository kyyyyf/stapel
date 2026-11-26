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
        let out = stapel(repo.path()).arg("inbox").output().unwrap();
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
        let out = stapel(repo.path()).arg("inbox").output().unwrap();
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

/// The `check` records of ABC-1 in `runs.jsonl`.
fn check_records(dir: &Path) -> Vec<serde_json::Value> {
    read(dir, ".stapel/tickets/ABC-1/runs.jsonl")
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["kind"] == "check")
        .collect()
}

/// `stapel check ABC-1 --level <level>` in `dir`: exit code, standard output, standard error.
fn run_check(dir: &Path, level: &str) -> (i32, String, String) {
    let out = stapel(dir)
        .args(["check", "ABC-1", "--level", level])
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// A repository with a step whose `[check] cargo` is the fake cargo; the scripts folder lives as
/// long as the returned value.
fn repo_with_fake_cargo() -> (tempfile::TempDir, tempfile::TempDir) {
    let repo = repo_with_a_step();
    let scripts = tempfile::tempdir().unwrap();
    let fake = common::fake_cargo(scripts.path());
    configure(repo.path(), &fake, "");
    (repo, scripts)
}

// AC-1: the log is made at the first cargo run, its path is printed before the first step line,
// the record holds it, and each run has its header and its output; no cargo run, no log.
#[test]
fn log_holds_every_run() {
    let (repo, _scripts) = repo_with_fake_cargo();
    let dir = repo.path();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");

    let lines: Vec<&str> = out.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("log: "))
        .unwrap_or_else(|| panic!("no log line: {out}"));
    let first_step = lines
        .iter()
        .position(|l| l.starts_with("step 1"))
        .unwrap_or_else(|| panic!("no step line: {out}"));
    assert!(at < first_step, "{out}");
    let path = lines[at].strip_prefix("log: ").unwrap();

    let records = check_records(dir);
    assert_eq!(records.len(), 1);
    let id = records[0]["id"].as_str().unwrap();
    assert!(id.starts_with("c-") && id.len() == 14, "{id}");
    assert_eq!(records[0]["log"], path);
    assert!(
        path.ends_with(&format!("stapel/check-logs/ABC-1-{id}.log")),
        "{path}"
    );

    let log = std::fs::read_to_string(path).unwrap();
    let mut sections: Vec<(&str, String)> = Vec::new();
    for line in log.lines() {
        if let Some(head) = line.strip_prefix("== ") {
            sections.push((head, String::new()));
        } else if let Some((_, body)) = sections.last_mut() {
            body.push_str(line);
            body.push('\n');
        } else {
            panic!("text before the first header: {log}");
        }
    }
    let stages: Vec<&str> = sections
        .iter()
        .map(|(h, _)| h.split(": ").next().unwrap())
        .collect();
    assert_eq!(
        stages,
        ["step 1 RED", "step 1 GREEN", "step 1 HEAD", "suite run"],
        "{log}"
    );
    for (head, _) in &sections[..3] {
        assert!(
            head.contains(" test --locked -p tiny --test basic -- --exact needs_two"),
            "{head}"
        );
    }
    assert!(
        sections[3].0.ends_with(" test --workspace --locked"),
        "{}",
        sections[3].0
    );
    assert!(sections[0].1.contains("test needs_two ... FAILED"), "{log}");
    assert!(sections[1].1.contains("test needs_two ... ok"), "{log}");
    assert!(sections[2].1.contains("test needs_two ... ok"), "{log}");
    assert!(sections[3].1.contains("test result: ok."), "{log}");

    // Level `steps` with every step cached: no cargo run, no log, no field.
    let before = std::fs::read_dir(Path::new(path).parent().unwrap())
        .unwrap()
        .count();
    let (code, out, err) = run_check(dir, "steps");
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("cached"), "{out}");
    assert!(!out.lines().any(|l| l.starts_with("log: ")), "{out}");
    let after = std::fs::read_dir(Path::new(path).parent().unwrap())
        .unwrap()
        .count();
    assert_eq!(before, after);
    let records = check_records(dir);
    assert_eq!(records.len(), 2);
    assert!(records[1].get("log").is_none(), "{}", records[1]);
}

// AC-3: a log that cannot be made gives one warning, no `log:` line and no `log` field; the check
// runs and passes as without the fault.
#[test]
fn unwritable_log_folder() {
    let no_log = |out: &str, err: &str, dir: &Path| {
        assert_eq!(
            err.matches("warning: no check log: ").count(),
            1,
            "{err}"
        );
        assert!(!out.lines().any(|l| l.starts_with("log: ")), "{out}");
        assert!(out.contains("result: pass"), "{out}");
        let records = check_records(dir);
        assert_eq!(records.len(), 1);
        assert!(records[0].get("log").is_none(), "{}", records[0]);
    };

    // A plain file in the place of the log folder.
    let (repo, _scripts) = repo_with_fake_cargo();
    let dir = repo.path();
    write(dir, ".git/stapel/check-logs", "not a folder\n");
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");
    no_log(&out, &err, dir);

    // A read-only folder: the same warning (not promised further); skipped where the mode
    // does not stop the writer.
    use std::os::unix::fs::PermissionsExt;
    let (repo, _scripts) = repo_with_fake_cargo();
    let dir = repo.path();
    let folder = dir.join(".git/stapel/check-logs");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(folder.join("probe"), "").is_ok() {
        return;
    }
    let (code, out, err) = run_check(dir, "head");
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(code, 0, "{out}{err}");
    no_log(&out, &err, dir);
}
