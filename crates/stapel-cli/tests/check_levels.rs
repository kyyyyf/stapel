//! STP-35: the levels of `stapel check` and the points where `stapel next` demands one.

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

/// Exit code and the text of stderr of `stapel check` with the args.
fn check(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = stapel(dir).arg("check").args(args).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// AC-13: `points` holds `close` and only `step`, `review`, `close`; absent means all three.
#[test]
fn points_need_close() {
    let refusal = "check.points must hold close and only step, review, close";
    for bad in [
        "points = []",
        "points = [\"review\"]",
        "points = [\"step\", \"review\"]",
        "points = [\"close\", \"deploy\"]",
        "points = [\"Close\"]",
    ] {
        let repo = stapel_repo();
        add_to_check(repo.path(), bad);
        let (code, err) = check(repo.path(), &[]);
        assert_eq!(code, 2, "{bad}: {err}");
        assert!(err.contains(refusal), "{bad}: {err}");
    }
    for good in [
        "",
        "points = [\"close\"]",
        "points = [\"step\", \"close\"]",
        "points = [\"close\", \"close\", \"review\"]",
        "points = [\"step\", \"review\", \"close\"]",
    ] {
        let repo = stapel_repo();
        if !good.is_empty() {
            add_to_check(repo.path(), good);
        }
        let (_, err) = check(repo.path(), &[]);
        assert!(!err.contains("stapel.toml"), "{good}: {err}");
    }
}

// AC-7: the key `level` and the flag `--level` take `steps`, `head` or `full`, nothing else.
#[test]
fn refuses_an_unknown_level() {
    for bad in ["fast", "", "HEAD", "Full"] {
        let repo = stapel_repo();
        add_to_check(repo.path(), &format!("level = \"{bad}\""));
        let (code, err) = check(repo.path(), &[]);
        assert_eq!(code, 2, "key {bad:?}: {err}");
        assert!(
            err.contains(&format!(
                "check.level must be steps, head or full: {bad}"
            )),
            "key {bad:?}: {err}"
        );
    }
    let repo = stapel_repo();
    for good in ["steps", "head", "full"] {
        let repo = stapel_repo();
        add_to_check(repo.path(), &format!("level = \"{good}\""));
        let (_, err) = check(repo.path(), &[]);
        assert!(!err.contains("stapel.toml"), "key {good}: {err}");
    }
    for bad in ["fast", "", "HEAD"] {
        let (code, err) = check(repo.path(), &["--level", bad]);
        assert_eq!(code, 2, "flag {bad:?}: {err}");
        assert!(
            err.contains(&format!("stapel: --level must be steps, head or full: {bad}")),
            "flag {bad:?}: {err}"
        );
    }
    for other in ["--list", "--history"] {
        let (code, err) = check(repo.path(), &["--level", "head", other]);
        assert_eq!(code, 2, "{other}: {err}");
        assert!(
            err.contains("stapel: --level does not go with --list or --history"),
            "{other}: {err}"
        );
    }
}

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

/// The last record of the ticket's `runs.jsonl`.
fn last_record(dir: &Path) -> serde_json::Value {
    let text = read(dir, ".stapel/tickets/ABC-1/runs.jsonl");
    let line = text
        .lines()
        .rfind(|l| l.contains("\"kind\":\"check\""))
        .unwrap();
    serde_json::from_str(line).unwrap()
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

/// What one `stapel check ABC-1` did: its output, the cargo runs of a step's tests (`--exact`)
/// and the cargo runs of the whole suite (`--workspace`). The log is emptied first.
struct Run {
    text: String,
    step_runs: usize,
    suite_runs: usize,
}

/// Runs `stapel check ABC-1` with the args (exit 0 expected).
fn run_level(dir: &Path, log: &Path, args: &[&str]) -> Run {
    std::fs::write(log, "").unwrap();
    let out = stapel(dir)
        .args(["check", "ABC-1"])
        .args(args)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(0), "{args:?}: {text}");
    let log = std::fs::read_to_string(log).unwrap();
    Run {
        step_runs: log.lines().filter(|l| l.contains("--exact")).count(),
        suite_runs: log.lines().filter(|l| l.contains("--workspace")).count(),
        text,
    }
}

/// A repository with the recording fake as its cargo and the extra `[check]` lines.
fn logged_repo(extra: &str) -> (tempfile::TempDir, tempfile::TempDir, std::path::PathBuf) {
    let repo = repo_with_a_step();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    configure(
        repo.path(),
        &recording_fake(scripts.path(), &log),
        extra,
    );
    (repo, scripts, log)
}

// AC-8: at `steps` an uncached step runs at RED, GREEN and HEAD; a cached one runs nothing; no
// suite runs; the suite outcome is `skipped`.
#[test]
fn steps_level() {
    let (repo, _scripts, log) = logged_repo("");
    let dir = repo.path();

    let first = run_level(dir, &log, &["--level", "steps"]);
    assert_eq!(first.step_runs, 3, "{}", first.text);
    assert_eq!(first.suite_runs, 0, "{}", first.text);
    assert!(first.text.contains("suite: skipped: level steps"), "{}", first.text);
    assert!(first.text.contains("result: pass"), "{}", first.text);
    let record = last_record(dir);
    assert_eq!(record["suite"]["outcome"], "skipped", "{record}");
    assert_eq!(record["result"], "pass", "{record}");
    assert_eq!(record["steps"][0]["outcome"], "pass", "{record}");

    // The step passed, so the next check finds it cached and runs nothing of it.
    let second = run_level(dir, &log, &["--level", "steps"]);
    assert_eq!(second.step_runs, 0, "{}", second.text);
    assert_eq!(second.suite_runs, 0, "{}", second.text);
    assert!(second.text.contains("cached from"), "{}", second.text);
    assert!(second.text.contains("suite: skipped: level steps"), "{}", second.text);
    let record = last_record(dir);
    assert_eq!(record["steps"][0]["outcome"], "cached", "{record}");
    assert_eq!(record["suite"]["outcome"], "skipped", "{record}");
    assert_eq!(record["result"], "pass", "{record}");
    assert_eq!(record["level"], "steps", "{record}");
}

// AC-9: at `head` a cached step runs its tests at HEAD only, and the suite runs.
#[test]
fn head_level() {
    let (repo, _scripts, log) = logged_repo("");
    let dir = repo.path();

    let first = run_level(dir, &log, &["--level", "head"]);
    assert_eq!(first.step_runs, 3, "{}", first.text);
    assert_eq!(first.suite_runs, 1, "{}", first.text);

    let second = run_level(dir, &log, &["--level", "head"]);
    assert_eq!(second.step_runs, 1, "{}", second.text);
    assert_eq!(second.suite_runs, 1, "{}", second.text);
    assert!(second.text.contains("cached from"), "{}", second.text);
    assert!(!second.text.contains("skipped: level"), "{}", second.text);
    assert_eq!(line_after_cargo(&second.text), "level: head", "{}", second.text);
    let record = last_record(dir);
    assert_eq!(record["steps"][0]["outcome"], "cached", "{record}");
    assert_eq!(record["suite"]["outcome"], "pass", "{record}");
    assert_eq!(record["level"], "head", "{record}");
}

// AC-10: at `full` every step runs at RED, GREEN and HEAD, and the suite runs; no cache.
#[test]
fn full_level() {
    let (repo, _scripts, log) = logged_repo("");
    let dir = repo.path();
    let first = run_level(dir, &log, &["--level", "head"]);
    assert_eq!(first.step_runs, 3, "{}", first.text);

    let full = run_level(dir, &log, &["--level", "full"]);
    assert_eq!(full.step_runs, 3, "{}", full.text);
    assert_eq!(full.suite_runs, 1, "{}", full.text);
    assert!(!full.text.contains("cached"), "{}", full.text);
    assert_eq!(line_after_cargo(&full.text), "level: full", "{}", full.text);
    let record = last_record(dir);
    assert_eq!(record["steps"][0]["outcome"], "pass", "{record}");
    assert_eq!(record["suite"]["outcome"], "pass", "{record}");
    assert_eq!(record["level"], "full", "{record}");
}

/// The line that follows the `cargo:` line of the report.
fn line_after_cargo(text: &str) -> String {
    let mut lines = text.lines();
    lines.find(|l| l.starts_with("cargo: ")).expect(text);
    lines.next().unwrap_or("").to_string()
}

// AC-11: the default is `head`; the flag wins over the key; `level: <level>` follows `cargo:`.
#[test]
fn flag_wins_over_the_key() {
    // No key, no flag.
    let (repo, _scripts, log) = logged_repo("");
    let run = run_level(repo.path(), &log, &[]);
    assert_eq!(line_after_cargo(&run.text), "level: head", "{}", run.text);
    assert_eq!(run.suite_runs, 1, "{}", run.text);
    assert_eq!(last_record(repo.path())["level"], "head");

    // Key `steps`, no flag.
    let (repo, _scripts, log) = logged_repo("level = \"steps\"");
    let run = run_level(repo.path(), &log, &[]);
    assert_eq!(line_after_cargo(&run.text), "level: steps", "{}", run.text);
    assert_eq!(run.suite_runs, 0, "{}", run.text);
    assert_eq!(last_record(repo.path())["level"], "steps");

    // Key `steps`, flag `head`: the flag wins.
    let run = run_level(repo.path(), &log, &["--level", "head"]);
    assert_eq!(line_after_cargo(&run.text), "level: head", "{}", run.text);
    assert_eq!(run.suite_runs, 1, "{}", run.text);
    assert_eq!(last_record(repo.path())["level"], "head");

    // Key `head`, flag `full`.
    let (repo, _scripts, log) = logged_repo("level = \"head\"");
    let run = run_level(repo.path(), &log, &["--level", "full"]);
    assert_eq!(line_after_cargo(&run.text), "level: full", "{}", run.text);
    assert_eq!(last_record(repo.path())["level"], "full");
}
