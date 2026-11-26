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
    repo_with_a_step_and(&[])
}

/// `repo_with_a_step` with more files, `(path, text)`, in the initial commit.
fn repo_with_a_step_and(extra: &[(String, String)]) -> tempfile::TempDir {
    let repo = common::git_repo();
    // An entry for `basic.rs` is the head of the step's test file, not a file of its own.
    let mut head = String::new();
    for (rel, text) in extra {
        if rel == "crates/tiny/tests/basic.rs" {
            head = text.clone();
        } else {
            write(repo.path(), rel, text);
        }
    }
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
        &format!("{head}#[test]\nfn needs_two() {{\n    // fake: needs pub fn two\n}}\n"),
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
        runs.iter()
            .filter(|l| l.contains("--workspace") && !l.contains("--no-run"))
            .cloned()
            .collect()
    };
    let builds = |runs: &[String]| -> Vec<String> {
        runs.iter().filter(|l| l.contains("--no-run")).cloned().collect()
    };

    let runs = logged_runs("jobs = 3\ntest_threads = 2");
    assert_eq!(
        builds(&runs),
        ["test --workspace --locked -j 3 --no-run"],
        "{runs:?}"
    );
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
        [
            "step 1 RED",
            "step 1 GREEN",
            "step 1 HEAD",
            "suite build",
            "suite run"
        ],
        "{log}"
    );
    for (head, _) in &sections[..3] {
        assert!(
            head.contains(" test --locked -p tiny --test basic -- --exact needs_two"),
            "{head}"
        );
    }
    assert!(
        sections[3].0.ends_with(" test --workspace --locked --no-run"),
        "{}",
        sections[3].0
    );
    assert!(
        sections[4].0.ends_with(" test --workspace --locked"),
        "{}",
        sections[4].0
    );
    assert!(sections[0].1.contains("test needs_two ... FAILED"), "{log}");
    assert!(sections[1].1.contains("test needs_two ... ok"), "{log}");
    assert!(sections[2].1.contains("test needs_two ... ok"), "{log}");
    assert!(sections[3].1.contains("Finished"), "{log}");
    assert!(sections[4].1.contains("test result: ok."), "{log}");

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

// AC-2: a check keeps the newest 10 logs of its ticket (the new one counted, ties by name) and
// leaves the logs of other tickets and other files.
#[test]
fn keeps_ten_logs_per_ticket() {
    let (repo, _scripts) = repo_with_fake_cargo();
    let dir = repo.path();
    let folder = dir.join(".git/stapel/check-logs");
    std::fs::create_dir_all(&folder).unwrap();
    let make = |name: &str, secs: u64| {
        let file = std::fs::File::create(folder.join(name)).unwrap();
        file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
            .unwrap();
    };
    // Eleven logs of ABC-1: the oldest, then two with one time, then eight newer.
    let names: Vec<String> = (0..11)
        .map(|i| format!("ABC-1-c-{:012x}.log", i + 1))
        .collect();
    make(&names[0], 1000);
    make(&names[1], 2000);
    make(&names[2], 2000);
    for (i, name) in names.iter().enumerate().skip(3) {
        make(name, 3000 + i as u64);
    }
    let other: Vec<String> = (0..2)
        .map(|i| format!("ABC-1-X-c-{:012x}.log", i + 1))
        .collect();
    make(&other[0], 500);
    make(&other[1], 500);
    make("notes.txt", 400);

    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");

    let present = |name: &str| folder.join(name).exists();
    let ticket_logs = std::fs::read_dir(&folder)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("ABC-1-c-"))
        .count();
    assert_eq!(ticket_logs, 10, "{out}");
    assert!(!present(&names[0]), "the oldest stays");
    assert!(!present(&names[1]), "the tie is not broken by name");
    assert!(present(&names[2]));
    for name in &names[3..] {
        assert!(present(name), "{name} removed");
    }
    for name in &other {
        assert!(present(name), "{name} removed");
    }
    assert!(present("notes.txt"));
    assert!(!err.contains("old check log not removed"), "{err}");
}

// AC-4: the copy time and the slot wait are printed, in this order, before the `ticket:` line, and
// recorded; an outer holder's mark makes the wait 0.
#[test]
fn copy_and_slot_times() {
    let seconds = |line: &str, prefix: &str, suffix: &str| -> u64 {
        line.strip_prefix(prefix)
            .and_then(|r| r.strip_suffix(suffix))
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("not `{prefix}<n>{suffix}`: {line}"))
    };
    let (repo, _scripts) = repo_with_fake_cargo();
    let dir = repo.path();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");
    let lines: Vec<&str> = out.lines().collect();
    let at = |prefix: &str| {
        lines
            .iter()
            .position(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("no `{prefix}` line: {out}"))
    };
    let (copy, slot, ticket) = (at("copy: "), at("cargo slot: "), at("ticket: "));
    assert!(copy < slot && slot < ticket, "{out}");
    let copy_secs = seconds(lines[copy], "copy: ", " s");
    let wait = seconds(lines[slot], "cargo slot: waited ", " s");
    let records = check_records(dir);
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0]["times"]["copy"].as_u64(), Some(copy_secs));
    assert_eq!(records[0]["slot_wait"].as_u64(), Some(wait));

    // The held mark of an outer `stapel cargo`: no wait.
    let out = stapel(dir)
        .args(["check", "ABC-1", "--level", "head"])
        .env("STAPEL_CARGO_LOCK_HELD", "1")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.lines().any(|l| l == "cargo slot: waited 0 s"),
        "{text}"
    );
    let records = check_records(dir);
    assert_eq!(records.len(), 2);
    assert_eq!(records[1]["slot_wait"].as_u64(), Some(0));
}

// AC-5: the line after a step line (and its notes) names the commits the step ran at and the
// seconds of its cargo runs; the record of the step holds `times`; a step that ran no cargo has
// neither.
#[test]
fn step_times() {
    use std::os::unix::fs::PermissionsExt;
    // The line after the step line.
    let after_step = |out: &str| -> String {
        let lines: Vec<&str> = out.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.starts_with("step 1"))
            .unwrap_or_else(|| panic!("no step line: {out}"));
        lines[at + 1].to_string()
    };
    // A `cargo` that sleeps a second per run, and hangs at the run numbered `hang` (0: never).
    let slow = |scripts: &Path, hang: u32, nap: u32| -> String {
        std::fs::create_dir_all(scripts.join("real")).unwrap();
        let real = common::fake_cargo(&scripts.join("real"));
        let count = scripts.join("count");
        let path = scripts.join("cargo");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\n[ \"$1\" = --version ] && exec {real} \"$@\"\necho x >> {count}\n\
                 n=$(wc -l < {count} | tr -d ' ')\n\
                 [ \"$n\" = {hang} ] && exec sleep 60\nsleep {nap}\nexec {real} \"$@\"\n",
                count = count.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path.to_string_lossy().into_owned()
    };

    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    configure(dir, &slow(scripts.path(), 0, 1), "");

    // A passing step: RED, GREEN and HEAD, each at least the second its run slept.
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");
    let line = after_step(&out);
    let times: Vec<u64> = line
        .strip_prefix("  times: RED ")
        .and_then(|r| r.split_once(" s, GREEN "))
        .and_then(|(red, r)| r.split_once(" s, HEAD ").map(|(g, h)| (red, g, h)))
        .and_then(|(red, g, h)| {
            Some(vec![
                red.parse().ok()?,
                g.parse().ok()?,
                h.strip_suffix(" s")?.parse().ok()?,
            ])
        })
        .unwrap_or_else(|| panic!("not a times line: {line}\n{out}"));
    assert!(times.iter().all(|t| *t >= 1), "{line}");
    let records = check_records(dir);
    let t = &records[0]["steps"][0]["times"];
    assert_eq!(
        [t["red"].as_u64(), t["green"].as_u64(), t["head"].as_u64()],
        [Some(times[0]), Some(times[1]), Some(times[2])],
        "{}",
        records[0]
    );

    // The cached step at the level `head`: HEAD only.
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("cached"), "{out}");
    let line = after_step(&out);
    assert!(
        line.starts_with("  times: HEAD ") && line.ends_with(" s") && !line.contains("RED"),
        "{line}\n{out}"
    );
    let records = check_records(dir);
    let t = &records[1]["steps"][0]["times"];
    assert!(t["head"].as_u64().is_some(), "{}", records[1]);
    assert!(t.get("red").is_none() && t.get("green").is_none(), "{t}");

    // The level `steps` with the cached step: no cargo run, no line, no field.
    let (code, out, err) = run_check(dir, "steps");
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.lines().any(|l| l.contains("times: ")), "{out}");
    let records = check_records(dir);
    assert!(records[2]["steps"][0].get("times").is_none(), "{}", records[2]);

    // A step cut by the time limit at GREEN: RED and GREEN.
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    configure(dir, &slow(scripts.path(), 2, 0), "");
    let text = read(dir, ".stapel/stapel.toml").replace("timeout_secs = 3600", "timeout_secs = 10");
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: a short limit");
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 1, "{out}{err}");
    let lines: Vec<&str> = out.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("step 1: unverified"))
        .unwrap_or_else(|| panic!("no unverified step: {out}"));
    let line = lines[at + 1];
    assert!(
        line.starts_with("  times: RED ") && line.contains(" s, GREEN ") && !line.contains("HEAD"),
        "{line}\n{out}"
    );
    let t = &check_records(dir)[0]["steps"][0]["times"];
    assert!(t["red"].is_u64() && t["green"].is_u64(), "{t}");
    assert!(t.get("head").is_none(), "{t}");
}

// AC-6: the suite build (`--no-run`, with `-j`) runs before the suite run and has its line, its
// log header and its record time; a build that runs out of the time limit starts no run.
#[test]
fn suite_build_and_run() {
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    configure(dir, &recording_fake(scripts.path(), &log), "jobs = 3");
    std::fs::write(&log, "").unwrap();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");

    let logged = std::fs::read_to_string(&log).unwrap();
    let runs: Vec<&str> = logged
        .lines()
        .filter(|l| l.contains("--workspace"))
        .collect();
    assert_eq!(runs.len(), 2, "{logged}");
    assert!(runs[0].contains("--no-run"), "{logged}");
    assert!(runs[0].contains("-j 3"), "{logged}");
    assert!(!runs[1].contains("--no-run"), "{logged}");

    let lines: Vec<&str> = out.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("suite build: "))
        .unwrap_or_else(|| panic!("no suite build line: {out}"));
    let secs: u64 = lines[at]
        .strip_prefix("suite build: ")
        .and_then(|r| r.strip_suffix(" s"))
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("not `suite build: <n> s`: {}", lines[at]));
    assert!(lines[at + 1].starts_with("suite: pass"), "{out}");
    let records = check_records(dir);
    let times = &records[0]["times"];
    assert_eq!(times["suite_build"].as_u64(), Some(secs), "{}", records[0]);
    assert!(times["suite_run"].is_u64(), "{}", records[0]);
    let text = std::fs::read_to_string(records[0]["log"].as_str().unwrap()).unwrap();
    assert!(text.contains("== suite build: "), "{text}");

    // A build that outlasts the time limit: no run, no run time.
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        "pub fn one() {}\npub fn two() {}\n// fake: build sleeps\n",
    );
    commit(dir, "ABC-1: a slow build");
    configure(dir, &recording_fake(scripts.path(), &log), "");
    let text = read(dir, ".stapel/stapel.toml").replace("timeout_secs = 3600", "timeout_secs = 10");
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: a short limit");
    std::fs::write(&log, "").unwrap();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("suite: unverified: the time limit"), "{out}");
    let logged = std::fs::read_to_string(&log).unwrap();
    let runs: Vec<&str> = logged
        .lines()
        .filter(|l| l.contains("--workspace"))
        .collect();
    assert_eq!(runs.len(), 1, "{logged}");
    assert!(runs[0].contains("--no-run"), "{logged}");
    let records = check_records(dir);
    let times = &records[0]["times"];
    assert!(times.get("suite_run").is_none(), "{}", records[0]);
}

// AC-14: a build that fails judges the suite by its output: no run starts, the counts are 0 and
// there is no run time. The outcomes are `fail: … the workspace does not compile`,
// `unverified: Cargo.lock is out of date` and `unverified: build-input-outside: …`.
#[test]
fn failed_build_judges_the_suite() {
    // A workspace that does not compile.
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        "pub fn one() {}\npub fn two() {}\n// this does not compile\n",
    );
    commit(dir, "ABC-1: a broken library");
    configure(dir, &recording_fake(scripts.path(), &log), "");
    std::fs::write(&log, "").unwrap();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 1, "{out}{err}");
    let logged = std::fs::read_to_string(&log).unwrap();
    let runs: Vec<&str> = logged
        .lines()
        .filter(|l| l.contains("--workspace"))
        .collect();
    assert_eq!(runs.len(), 1, "{logged}");
    assert!(runs[0].contains("--no-run"), "{logged}");
    assert!(
        out.contains("suite: fail: 0 passed, 0 failed, 0 ignored; the workspace does not compile"),
        "{out}"
    );
    let records = check_records(dir);
    assert!(
        records[0]["times"].get("suite_run").is_none(),
        "{}",
        records[0]
    );

    // A stale lock file.
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let log = scripts.path().join("log");
    write(
        dir,
        "Cargo.lock",
        "# fake: stale lock\nversion = 3\n\n[[package]]\nname = \"tiny\"\nversion = \"0.1.0\"\n",
    );
    commit(dir, "ABC-1: a stale lock file");
    configure(dir, &recording_fake(scripts.path(), &log), "");
    std::fs::write(&log, "").unwrap();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 1, "{out}{err}");
    let logged = std::fs::read_to_string(&log).unwrap();
    let runs: Vec<&str> = logged
        .lines()
        .filter(|l| l.contains("--workspace"))
        .collect();
    assert_eq!(runs.len(), 1, "{logged}");
    assert!(
        out.contains("suite: unverified: Cargo.lock is out of date"),
        "{out}"
    );
    let records = check_records(dir);
    assert!(
        records[0]["times"].get("suite_run").is_none(),
        "{}",
        records[0]
    );
}

// AC-7: the `suite:` line of a suite that ran ends with `; run <n> s, last pass <m> s`, `<m>` being
// the `times.suite_run` of the last passing record of the ticket that has one, or `none`; a
// malformed line is skipped; an unverified suite has no such end.
#[test]
fn last_pass_time() {
    let suite_line = |out: &str| -> String {
        out.lines()
            .find(|l| l.starts_with("suite: "))
            .unwrap_or_else(|| panic!("no suite line: {out}"))
            .to_string()
    };
    let end_of = |line: &str, last: &str| -> bool {
        let Some((_, end)) = line.split_once("; run ") else {
            return false;
        };
        let Some((secs, rest)) = end.split_once(" s, last pass ") else {
            return false;
        };
        !secs.is_empty() && secs.chars().all(|c| c.is_ascii_digit()) && rest == last
    };

    // No earlier record.
    let (repo, _scripts) = repo_with_fake_cargo();
    let (code, out, err) = run_check(repo.path(), "head");
    assert_eq!(code, 0, "{out}{err}");
    let line = suite_line(&out);
    assert!(end_of(&line, "none"), "{line}");

    // Earlier records: a pass with 7, a fail with 9, a malformed line, a later pass without the
    // field and a pass of another ticket with 5.
    let (repo, _scripts) = repo_with_fake_cargo();
    let dir = repo.path();
    let record = |ticket: &str, result: &str, times: serde_json::Value| {
        serde_json::json!({
            "v": 1, "kind": "check", "ticket": ticket, "result": result, "times": times
        })
        .to_string()
    };
    let lines = [
        record("ABC-1", "pass", serde_json::json!({"suite_run": 7})),
        record("ABC-1", "fail", serde_json::json!({"suite_run": 9})),
        "{ this is not json".to_string(),
        record("ABC-1", "pass", serde_json::json!({"copy": 1})),
        record("ABC-2", "pass", serde_json::json!({"suite_run": 5})),
    ];
    let path = dir.join(".stapel/tickets/ABC-1/runs.jsonl");
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    for l in &lines {
        text.push_str(l);
        text.push('\n');
    }
    std::fs::write(&path, text).unwrap();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");
    let line = suite_line(&out);
    assert!(end_of(&line, "7 s"), "{line}");

    // An unverified suite has no such end.
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    write(
        dir,
        "Cargo.lock",
        "# fake: stale lock\nversion = 3\n\n[[package]]\nname = \"tiny\"\nversion = \"0.1.0\"\n",
    );
    commit(dir, "ABC-1: a stale lock file");
    configure(dir, &common::fake_cargo(scripts.path()), "");
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 1, "{out}{err}");
    let line = suite_line(&out);
    assert!(line.starts_with("suite: unverified: "), "{line}");
    assert!(!line.contains("; run "), "{line}");
}

// AC-8: after `suite:` and `failing:` the check prints `slowest: <target> <x> s, …` with the five
// largest target times (blocks that ran tests), largest first, equal ones in output order, `<x>` as
// libtest printed it; the record holds `slowest` as `{"target", "secs"}` objects. No target time,
// no line and no field.
#[test]
fn slowest_files() {
    let one = |time: &str| format!("// fake: finished {time}\n#[test]\nfn it_runs() {{}}\n");
    // A block of 0 tests with the largest time, then six targets with tests; `basic` prints 0.00.
    let mut extra = vec![(
        "crates/tiny/tests/a_zero.rs".to_string(),
        "// fake: finished 99.00\n".to_string(),
    )];
    for (name, time) in [
        ("b1", "1.50"),
        ("b2", "3.25"),
        ("b3", "0.40"),
        ("b4", "3.25"),
        ("b5", "12.00"),
        ("b6", "0.07"),
    ] {
        extra.push((format!("crates/tiny/tests/{name}.rs"), one(time)));
    }
    let repo = repo_with_a_step_and(&extra);
    let scripts = tempfile::tempdir().unwrap();
    configure(repo.path(), &common::fake_cargo(scripts.path()), "");
    let dir = repo.path();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");
    let lines: Vec<&str> = out.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("suite: "))
        .unwrap_or_else(|| panic!("no suite line: {out}"));
    assert_eq!(
        lines.get(at + 1).copied(),
        Some("slowest: b5 12.00 s, b2 3.25 s, b4 3.25 s, b1 1.50 s, b3 0.40 s"),
        "{out}"
    );
    let records = check_records(dir);
    let slowest = records[0]["slowest"].as_array().unwrap_or_else(|| {
        panic!("no slowest array: {}", records[0]);
    });
    let got: Vec<(String, f64)> = slowest
        .iter()
        .map(|e| {
            (
                e["target"].as_str().unwrap().to_string(),
                e["secs"].as_f64().unwrap(),
            )
        })
        .collect();
    let want: Vec<(String, f64)> = [
        ("b5", 12.0),
        ("b2", 3.25),
        ("b4", 3.25),
        ("b1", 1.5),
        ("b3", 0.4),
    ]
    .iter()
    .map(|(t, s)| (t.to_string(), *s))
    .collect();
    assert_eq!(got, want, "{}", records[0]);

    // A suite with no `finished in`: no line, no field.
    let extra = [(
        "crates/tiny/tests/basic.rs".to_string(),
        "// fake: finished none\n".to_string(),
    )];
    let repo = repo_with_a_step_and(&extra);
    let scripts = tempfile::tempdir().unwrap();
    configure(repo.path(), &common::fake_cargo(scripts.path()), "");
    let dir = repo.path();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("slowest:"), "{out}");
    let records = check_records(dir);
    assert!(records[0].get("slowest").is_none(), "{}", records[0]);
}

// AC-9: with `progress_secs = 1` a run that sleeps 3 s prints `progress: <stage>, <file>, <n> s`
// lines, `<n>` the seconds since the start, rising; a step run names `<dir>/<file>`, a suite run
// the target of its newest `Running` line.
#[test]
fn progress_lines() {
    let extra = [
        (
            "crates/tiny/tests/basic.rs".to_string(),
            "// fake: sleeps 3\n".to_string(),
        ),
        (
            "crates/tiny/tests/slow.rs".to_string(),
            "// fake: sleeps 3\n#[test]\nfn it_runs() {}\n".to_string(),
        ),
    ];
    let repo = repo_with_a_step_and(&extra);
    let scripts = tempfile::tempdir().unwrap();
    configure(
        repo.path(),
        &common::fake_cargo(scripts.path()),
        "progress_secs = 1",
    );
    let dir = repo.path();
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 0, "{out}{err}");
    // `(stage, file, n)` of each progress line, in output order.
    let lines: Vec<(String, String, u64)> = out
        .lines()
        .filter_map(|l| l.strip_prefix("progress: "))
        .map(|rest| {
            let parts: Vec<&str> = rest.split(", ").collect();
            assert_eq!(parts.len(), 3, "not `progress: <stage>, <file>, <n> s`: {rest}");
            let n = parts[2]
                .strip_suffix(" s")
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| panic!("not `<n> s`: {rest}"));
            (parts[0].to_string(), parts[1].to_string(), n)
        })
        .collect();
    let step: Vec<&(String, String, u64)> = lines
        .iter()
        .filter(|(stage, _, _)| stage == "step 1 RED")
        .collect();
    assert!(step.len() >= 2, "{out}");
    assert!(step.iter().all(|(_, file, _)| file == "tiny/basic"), "{out}");
    let suite: Vec<&(String, String, u64)> = lines
        .iter()
        .filter(|(stage, file, _)| stage == "suite run" && file == "slow")
        .collect();
    assert!(!suite.is_empty(), "{out}");
    assert!(lines.iter().all(|l| l.2 >= 1), "{out}");
    assert!(lines.windows(2).all(|w| w[0].2 <= w[1].2), "{out}");
    // Every run of a step prints its own stage.
    assert!(lines.iter().any(|l| l.0 == "step 1 GREEN"), "{out}");
}

// AC-10: the limit runs out in a step run: `time limit: <stage>, <file>`, then the last 20 lines of
// each kept stream, control characters replaced; the check ends `unverified`, exit 1; the log
// holds the timed-out run.
#[test]
fn limit_names_the_stage() {
    let extra = [(
        "crates/tiny/tests/basic.rs".to_string(),
        "// fake: noise 30\n// fake: sleeps 60\n".to_string(),
    )];
    let repo = repo_with_a_step_and(&extra);
    let scripts = tempfile::tempdir().unwrap();
    configure(repo.path(), &common::fake_cargo(scripts.path()), "");
    let dir = repo.path();
    let text = read(dir, ".stapel/stapel.toml").replace("timeout_secs = 3600", "timeout_secs = 10");
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: a short limit");
    let (code, out, err) = run_check(dir, "head");
    assert_eq!(code, 1, "{out}{err}");
    let lines: Vec<&str> = out.lines().collect();
    let at = lines
        .iter()
        .position(|l| *l == "time limit: step 1 RED, tiny/basic")
        .unwrap_or_else(|| panic!("no time limit line: {out}"));
    let tail = &lines[at + 1..at + 41];
    let want_out: Vec<String> = (11..=30)
        .map(|i| {
            if i == 25 {
                format!("  out | out line {i}  [31m red")
            } else {
                format!("  out | out line {i}")
            }
        })
        .collect();
    let want_err: Vec<String> = want_out.iter().map(|l| l.replace("out", "err")).collect();
    assert_eq!(tail[..20], want_out[..], "{out}");
    assert_eq!(tail[20..], want_err[..], "{out}");
    assert!(!out.contains('\u{1b}'), "{out}");
    assert!(out.contains("result: fail"), "{out}");
    let log = std::fs::read_dir(dir.join(".git").join("stapel").join("check-logs"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let log = std::fs::read_to_string(log).unwrap();
    assert!(log.contains("out line 30") && log.contains("err line 30"), "{log}");
}
