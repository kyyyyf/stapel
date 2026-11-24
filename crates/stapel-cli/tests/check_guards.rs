//! STP-41: guards, machine checks the agent cannot loosen unseen (step 1: the config).

mod common;

use common::{read, stapel, stapel_repo};
use std::path::Path;

/// Appends the text to the repository's `stapel.toml`, with a `[check]` section when asked.
fn add_config(dir: &Path, with_check: bool, tail: &str) {
    let check = if with_check {
        "\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n"
    } else {
        ""
    };
    let text = format!("{}{check}\n{tail}\n", read(dir, ".stapel/stapel.toml"));
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

// AC-1: the dir is a relative folder outside .stapel, .git and .claude; guards need [check].
#[test]
fn refuses_a_bad_dir() {
    for dir in [
        "",
        "/abs",
        ".",
        "a/../b",
        "./g",
        "a//b",
        "a/./b",
        "a\\\\b",
        ".stapel/g",
        ".GIT",
        ".Claude/x",
    ] {
        let repo = stapel_repo();
        add_config(repo.path(), true, &format!("[guards]\ndir = \"{dir}\"\n"));
        let (code, err) = check(repo.path());
        let shown = dir.replace("\\\\", "\\");
        let refusal = format!(
            "guards.dir must be a relative folder of the repository outside .stapel, .git and .claude: {shown}"
        );
        assert_eq!(code, 2, "{dir}: {err}");
        assert!(err.contains(&refusal), "{dir}: {err}");
    }

    let repo = stapel_repo();
    add_config(repo.path(), false, "[guards]\ndir = \"guards\"\n");
    let (code, err) = check(repo.path());
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("guards need [check]"), "{err}");

    for dir in ["tests/guards", "guards/"] {
        let repo = stapel_repo();
        add_config(repo.path(), true, &format!("[guards]\ndir = \"{dir}\"\n"));
        let (_, err) = check(repo.path());
        assert!(
            !err.contains("stapel.toml") && !err.contains("guards"),
            "{dir}: {err}"
        );
    }
}

// STP-43 AC-1: a dir that is a work folder is refused; a nested or another name loads.
#[test]
fn refuses_an_overlapping_dir() {
    for dir in ["tests", "Tests/", "src", "docs", "crates", "packages"] {
        for with_check in [true, false] {
            let repo = stapel_repo();
            add_config(
                repo.path(),
                with_check,
                &format!("[guards]\ndir = \"{dir}\"\n"),
            );
            let (code, err) = check(repo.path());
            let refusal = format!(
                "guards.dir {dir} is a folder that agents write: name a folder of its own, such as guards"
            );
            assert_eq!(code, 2, "{dir}: {err}");
            assert!(err.contains(&refusal), "{dir}: {err}");
        }
    }

    for dir in ["tests/guards", "src/x", "guards", "testing"] {
        let repo = stapel_repo();
        add_config(repo.path(), true, &format!("[guards]\ndir = \"{dir}\"\n"));
        let (_, err) = check(repo.path());
        assert!(
            !err.contains("stapel.toml") && !err.contains("guards"),
            "{dir}: {err}"
        );
    }
}

// AC-2: a guard needs a name and a command, a unique name and a relative baseline.
#[test]
fn refuses_bad_entries() {
    let entry = |name: &str, command: &str, baseline: Option<&str>| {
        let base = baseline.map_or(String::new(), |b| format!("baseline = \"{b}\"\n"));
        format!("[[guards.check]]\nname = \"{name}\"\ncommand = \"{command}\"\n{base}")
    };
    let head = "[guards]\ndir = \"guards\"\n";
    let need = "guards.check entries need a name and a command";
    let twice = "guards.check name is used twice: size";
    let cases: Vec<(String, String)> = vec![
        (entry("", "true", None), need.into()),
        (entry("size", "  ", None), need.into()),
        (
            format!(
                "{}{}",
                entry("size", "true", None),
                entry("size", "true", None)
            ),
            twice.into(),
        ),
    ]
    .into_iter()
    .chain(["", "/b", "../b", "a/./b", "a//b", "b/"].map(|b| {
        (
            entry("size", "true", Some(b)),
            format!("guards.check baseline must be a relative path inside guards.dir: {b}"),
        )
    }))
    .collect();
    for (entries, refusal) in cases {
        let repo = stapel_repo();
        add_config(repo.path(), true, &format!("{head}{entries}"));
        let (code, err) = check(repo.path());
        assert_eq!(code, 2, "{entries}: {err}");
        assert!(err.contains(&refusal), "{entries}: {err}");
    }

    let repo = stapel_repo();
    add_config(repo.path(), true, head);
    let (_, err) = check(repo.path());
    assert!(
        !err.contains("stapel.toml") && !err.contains("guards"),
        "{err}"
    );
}

// ---- step 2: exit-code guards ----

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

/// A repository with a one-crate workspace, ticket ABC-1 and one passing step, all committed, and a
/// tracked `notes.txt` ("head"). The fake cargo is its `[check] cargo`; `timeout` is its time limit,
/// `extra` more `[check]` lines and `guards` the text of the `[guards]` tables at the end of the file.
fn guards_repo(extra: &str, timeout: u32, guards: &str) -> (tempfile::TempDir, tempfile::TempDir) {
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
    write(dir, "notes.txt", "head\n");
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
    std::fs::write(&path, format!("#!/bin/sh\nexec {real} \"$@\"\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut text: String = read(dir, ".stapel/stapel.toml")
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
    text.push_str(&format!("\n[guards]\ndir = \"guards\"\n{guards}\n"));
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: the check keys");
    (repo, scripts)
}

/// One `[[guards.check]]` entry with a literal-string command.
fn guard(name: &str, command: &str) -> String {
    format!("[[guards.check]]\nname = \"{name}\"\ncommand = '{command}'\n")
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

// AC-3: guards run in order after the project commands in the check copy, each on the HEAD text.
#[test]
fn exit_code_guards() {
    let log_dir = tempfile::tempdir().unwrap();
    let log = log_dir.path().join("log");
    let l = log.display();
    let project = format!("echo changed > notes.txt; echo project >> {l}");
    let g1 = format!("pwd >> {l}; cat notes.txt >> {l}; echo scribble > notes.txt; touch keep");
    let g2 = format!("cat notes.txt >> {l}; test -f keep");
    let guards = format!(
        "{}{}{}",
        guard("one", &g1),
        guard("two", &g2),
        "[[guards.check]]\nname = \"a\\nb\"\ncommand = 'true'\n"
    );
    let (repo, _scripts) = guards_repo(&format!("project = ['{project}']"), 3600, &guards);
    let dir = repo.path();
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = |needle: &str| lines.iter().position(|l| *l == needle).expect(&text);
    let (p, g1, g2, g3) = (
        at(&format!("project: {project}: pass")),
        at("guard: one: pass"),
        at("guard: two: pass"),
        at("guard: a b: pass"),
    );
    assert!(p < g1 && g1 < g2 && g2 < g3, "{text}");
    assert!(text.contains("result: pass"), "{text}");
    let logged = std::fs::read_to_string(&log).unwrap();
    let logged: Vec<&str> = logged.lines().collect();
    assert_eq!(&logged[0], &"project", "{logged:?}");
    assert_eq!(&logged[2..], ["head", "head"], "{logged:?}");
    let root = std::fs::canonicalize(dir).unwrap();
    assert!(Path::new(logged[1]).is_absolute(), "{logged:?}");
    assert_ne!(Path::new(logged[1]), root, "{logged:?}");
    assert_eq!(read(dir, "notes.txt"), "head\n");

    // No project command: the guards run at `head` and at `full`.
    for level in ["head", "full"] {
        let (repo, _scripts) = guards_repo("", 3600, &guard("only", "true"));
        let (code, text) = run(repo.path(), &["--level", level]);
        assert_eq!(code, 0, "{level}: {text}");
        assert!(
            text.lines().any(|l| l.trim() == "guard: only: pass"),
            "{text}"
        );
    }
}

// AC-3: a failing guard prints its last 20 lines and fails the check; the close refuses it.
#[test]
fn a_failing_guard_fails_the_check() {
    let bad = "i=1; while [ $i -le 30 ]; do echo out$i; i=$((i+1)); done; exit 3";
    let (repo, _scripts) = guards_repo("", 3600, &guard("bad", bad));
    let dir = repo.path();
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim() == "guard: bad: fail (exit 3)"),
        "{text}"
    );
    let has = |s: &str| text.lines().any(|l| l.trim() == s);
    assert!(has("out30") && has("out11"), "{text}");
    assert!(!has("out10") && !has("out1"), "{text}");
    assert!(text.contains("result: fail"), "{text}");
    let out = stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("current passing check"), "{err}");
}

// AC-4: at level `steps` no guard runs.
#[test]
fn steps_level_skips() {
    let marker_dir = tempfile::tempdir().unwrap();
    let marker = marker_dir.path().join("ran");
    let guards = format!(
        "{}{}",
        guard("one", &format!("touch {}", marker.display())),
        guard("two", "true")
    );
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let (code, text) = run(repo.path(), &["--level", "steps"]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim() == "guards: skipped: level steps"),
        "{text}"
    );
    assert!(!text.contains("guard: one"), "{text}");
    assert!(!marker.exists(), "{text}");

    let (code, text) = run(repo.path(), &["--level", "full"]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.lines().any(|l| l.trim() == "guard: one: pass"),
        "{text}"
    );
    assert!(marker.exists(), "{text}");
}

// AC-5: the time limit kills a running guard and its child; the later guards are unverified too; a
// guard ended by a signal is unverified.
#[test]
fn time_limit_stops_a_guard() {
    let marker_dir = tempfile::tempdir().unwrap();
    let marker = marker_dir.path().join("second-ran");
    let guards = format!(
        "{}{}",
        guard("slow", "sleep 327 & sleep 328"),
        guard("second", &format!("touch {}", marker.display()))
    );
    let (repo, _scripts) = guards_repo("", 10, &guards);
    let started = std::time::Instant::now();
    let (code, text) = run(repo.path(), &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(60),
        "{text}"
    );
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"guard: slow: unverified: time limit"),
        "{text}"
    );
    assert!(
        lines.contains(&"guard: second: unverified: time limit"),
        "{text}"
    );
    assert!(!marker.exists(), "{text}");
    assert!(text.contains("result: fail"), "{text}");
    let left = std::process::Command::new("pgrep")
        .args(["-f", "sleep 32[78]"])
        .output()
        .unwrap();
    assert!(
        left.stdout.is_empty(),
        "a child survived: {}",
        String::from_utf8_lossy(&left.stdout)
    );

    let (repo, _scripts) = guards_repo("", 3600, &guard("killed", "kill -9 $$"));
    let (code, text) = run(repo.path(), &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim() == "guard: killed: unverified: ended by a signal"),
        "{text}"
    );
}

// AC-11: the record lists the guards (name, outcome, new, removed), the name cut to 200 characters,
// at most 20 of them with `guards_truncated`; without `[guards]` entries there is no `guards` key.
#[test]
fn the_record_lists_the_guards() {
    fn last_record(dir: &Path) -> serde_json::Value {
        let text = read(dir, ".stapel/tickets/ABC-1/runs.jsonl");
        let line = text
            .lines()
            .rfind(|l| l.contains("\"kind\":\"check\""))
            .unwrap();
        serde_json::from_str(line).unwrap()
    }

    let guards = format!("{}{}", guard("good", "true"), guard("bad", "exit 3"));
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let (code, text) = run(repo.path(), &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    let record = last_record(repo.path());
    assert_eq!(
        record["guards"],
        serde_json::json!([
            {"name": "good", "outcome": "pass", "new": 0, "removed": 0},
            {"name": "bad", "outcome": "fail", "new": 0, "removed": 0},
        ]),
        "{record}"
    );
    assert!(record.get("guards_truncated").is_none(), "{record}");

    // 22 guards, the first named with 300 characters.
    let long = "ж".repeat(300);
    let mut all = guard(&long, "true");
    for i in 1..22 {
        all.push_str(&guard(&format!("g{i}"), "true"));
    }
    let (repo, _scripts) = guards_repo("", 3600, &all);
    let (code, text) = run(repo.path(), &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    let record = last_record(repo.path());
    let list = record["guards"].as_array().expect("guards");
    assert_eq!(list.len(), 20, "{record}");
    assert_eq!(list[0]["name"], "ж".repeat(200), "{record}");
    assert_eq!(list[19]["name"], "g19", "{record}");
    assert_eq!(record["guards_truncated"], true, "{record}");

    // No guard entries: no `guards` key.
    let (repo, _scripts) = guards_repo("", 3600, "");
    let (code, text) = run(repo.path(), &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    let record = last_record(repo.path());
    assert!(record.get("guards").is_none(), "{record}");
    assert!(record.get("guards_truncated").is_none(), "{record}");
}

// ---- step 4: ratchet guards ----

/// One `[[guards.check]]` entry with a literal-string command and a baseline.
fn ratchet(name: &str, command: &str, baseline: &str) -> String {
    format!("{}baseline = \"{baseline}\"\n", guard(name, command))
}

/// The last `check` record of ticket ABC-1.
fn last_check_record(dir: &Path) -> serde_json::Value {
    let text = read(dir, ".stapel/tickets/ABC-1/runs.jsonl");
    let line = text
        .lines()
        .rfind(|l| l.contains("\"kind\":\"check\""))
        .unwrap();
    serde_json::from_str(line).unwrap()
}

// AC-6: known violations pass; blank lines, CRLF, trailing spaces and stderr lines are no violations.
#[test]
fn known_violations_pass() {
    let guards = [
        ratchet("equal", "printf \"a\\nb\\n\"", "equal.txt"),
        ratchet("subset", "printf \"a\\n\"", "subset.txt"),
        ratchet("crlf", "printf \"a\\r\\nb  \\n\\n\"", "crlf.txt"),
        ratchet("exit1", "printf \"a\\n\"; exit 1", "exit1.txt"),
        ratchet("stderr", "printf \"a\\n\"; echo oops >&2", "stderr.txt"),
    ]
    .concat();
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/equal.txt", "a\nb\n");
    write(dir, "guards/subset.txt", "a\n\n\n");
    write(dir, "guards/crlf.txt", "a\nb\n");
    write(dir, "guards/exit1.txt", "a\n");
    write(dir, "guards/stderr.txt", "a\n");
    commit(dir, "ABC-1: baselines");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    for name in ["equal", "subset", "crlf", "exit1", "stderr"] {
        assert!(
            text.lines()
                .any(|l| l.trim() == format!("guard: {name}: pass")),
            "{name}: {text}"
        );
    }
    assert!(text.contains("result: pass"), "{text}");
    let record = last_check_record(dir);
    let list = record["guards"].as_array().expect("guards");
    assert_eq!(list.len(), 5, "{record}");
    for item in list {
        assert_eq!(item["new"], 0, "{record}");
        assert_eq!(item["removed"], 0, "{record}");
    }
}

// AC-7: new violations fail with their count and the first 20; the baseline is read from git.
#[test]
fn a_new_violation_fails() {
    // One new line among known ones; the same line three times counts once; a control character.
    let command = "printf \"a\\nb\\nfresh\\nfresh\\nfresh\\nx\\ty\\n\"";
    let (repo, _scripts) = guards_repo("", 3600, &ratchet("size", command, "size.txt"));
    let dir = repo.path();
    write(dir, "guards/size.txt", "a\nb\n");
    commit(dir, "ABC-1: baseline");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(lines.contains(&"guard: size: fail: 2 new"), "{text}");
    assert!(lines.contains(&"fresh"), "{text}");
    assert!(lines.contains(&"x y"), "{text}");
    assert!(!lines.contains(&"a"), "{text}");
    assert!(text.contains("result: fail"), "{text}");
    assert_eq!(read(dir, "guards/size.txt"), "a\nb\n");
    let record = last_check_record(dir);
    assert_eq!(record["guards"][0]["outcome"], "fail", "{record}");
    assert_eq!(record["guards"][0]["new"], 2, "{record}");

    // 25 new lines: the count and the first 20 in output order.
    let many = "i=1; while [ $i -le 25 ]; do echo v$i; i=$((i+1)); done";
    let (repo, _scripts) = guards_repo("", 3600, &ratchet("many", many, "many.txt"));
    let dir = repo.path();
    write(dir, "guards/many.txt", "old\n");
    commit(dir, "ABC-1: baseline");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(lines.contains(&"guard: many: fail: 25 new"), "{text}");
    assert!(lines.contains(&"v1") && lines.contains(&"v20"), "{text}");
    assert!(!lines.contains(&"v21"), "{text}");

    // A command that appends its violations to its baseline in the copy still fails.
    let cheat = "echo cheat >> guards/cheat.txt; echo cheat";
    let (repo, _scripts) = guards_repo("", 3600, &ratchet("cheat", cheat, "cheat.txt"));
    let dir = repo.path();
    write(dir, "guards/cheat.txt", "old\n");
    commit(dir, "ABC-1: baseline");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim() == "guard: cheat: fail: 1 new"),
        "{text}"
    );
    assert_eq!(read(dir, "guards/cheat.txt"), "old\n");
}

// ---- step 5: missing baseline ----

// AC-8: no baseline file at HEAD (missing, a folder, only in the working tree) fails with the path
// and the violations; every distinct violation is new; nothing is written.
#[test]
fn a_missing_baseline_fails() {
    let command = "printf \"v1\\nv2\\nv3\\n\"";
    let guards = [
        ratchet("missing", command, "missing.txt"),
        ratchet("folder", command, "folder"),
        ratchet("untracked", command, "untracked.txt"),
    ]
    .concat();
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/folder/inner.txt", "v1\n");
    commit(dir, "ABC-1: a folder");
    // Present in the working tree, ignored by git, so the check accepts the tree but HEAD lacks it.
    write(dir, ".gitignore", "/target\n/guards/untracked.txt\n");
    commit(dir, "ABC-1: ignore a file");
    write(dir, "guards/untracked.txt", "v1\nv2\nv3\n");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    for (name, path) in [
        ("missing", "missing.txt"),
        ("folder", "folder"),
        ("untracked", "untracked.txt"),
    ] {
        let want = format!("guard: {name}: fail: no baseline guards/{path}");
        assert!(lines.contains(&want.as_str()), "{want}: {text}");
    }
    assert!(lines.contains(&"v1") && lines.contains(&"v3"), "{text}");
    assert!(text.contains("result: fail"), "{text}");
    assert!(!dir.join("guards/missing.txt").exists());
    assert_eq!(read(dir, "guards/untracked.txt"), "v1\nv2\nv3\n");
    let record = last_check_record(dir);
    for i in 0..3 {
        assert_eq!(record["guards"][i]["outcome"], "fail", "{record}");
        assert_eq!(record["guards"][i]["new"], 3, "{record}");
        assert_eq!(record["guards"][i]["removed"], 0, "{record}");
    }

    // Exit 2 with no baseline: the line of AC-9.
    let (repo, _scripts) = guards_repo("", 3600, &ratchet("broken", "echo v1; exit 2", "none.txt"));
    let (code, text) = run(repo.path(), &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(lines.contains(&"guard: broken: fail (exit 2)"), "{text}");
    assert!(!text.contains("no baseline"), "{text}");
}

// ---- step 5: broken command and cut output ----

// AC-9: a ratchet guard that exits with a code other than 0 and 1 fails as broken, with the tail of
// its output; the baseline file is not written.
#[test]
fn a_broken_ratchet_guard_fails() {
    let guards = [
        ratchet("gone", "exit 127", "gone.txt"),
        ratchet("loud", "echo v1; echo oops >&2; exit 2", "loud.txt"),
    ]
    .concat();
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/gone.txt", "old\n");
    write(dir, "guards/loud.txt", "v1\nold\n");
    commit(dir, "ABC-1: baselines");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(lines.contains(&"guard: gone: fail (exit 127)"), "{text}");
    assert!(lines.contains(&"guard: loud: fail (exit 2)"), "{text}");
    assert!(lines.contains(&"v1") && lines.contains(&"oops"), "{text}");
    assert!(text.contains("result: fail"), "{text}");
    assert_eq!(read(dir, "guards/gone.txt"), "old\n");
    assert_eq!(read(dir, "guards/loud.txt"), "v1\nold\n");
    let record = last_check_record(dir);
    for i in 0..2 {
        assert_eq!(record["guards"][i]["outcome"], "fail", "{record}");
        assert_eq!(record["guards"][i]["new"], 0, "{record}");
        assert_eq!(record["guards"][i]["removed"], 0, "{record}");
    }
}

// AC-16: a ratchet guard whose standard output passes the output limit is unverified and writes
// nothing; an exit-code guard with the same output and exit 0 passes.
#[test]
fn a_cut_output_is_unverified() {
    let flood = "yes violation | head -c 17000000";
    let guards = [ratchet("cut", flood, "cut.txt"), guard("plain", flood)].concat();
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/cut.txt", "violation\nfixed\n");
    commit(dir, "ABC-1: baseline");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"guard: cut: unverified: output cut"),
        "{text}"
    );
    assert!(lines.contains(&"guard: plain: pass"), "{text}");
    assert!(text.contains("result: fail"), "{text}");
    assert_eq!(read(dir, "guards/cut.txt"), "violation\nfixed\n");
    let record = last_check_record(dir);
    assert_eq!(record["guards"][0]["outcome"], "unverified", "{record}");
    assert_eq!(record["guards"][0]["new"], 0, "{record}");
    assert_eq!(record["guards"][0]["removed"], 0, "{record}");
    assert_eq!(record["guards"][1]["outcome"], "pass", "{record}");
}

// AC-16: a ratchet guard whose standard output is still held open by a process outside the group at
// the drain deadline is unverified, writes nothing and fails.
#[test]
fn a_held_output_is_unverified() {
    use std::os::unix::fs::PermissionsExt;
    /// Kills the detached holder on every exit of the test, by the pid its launcher wrote.
    struct Reap(std::path::PathBuf);
    impl Drop for Reap {
        fn drop(&mut self) {
            if let Ok(pid) = std::fs::read_to_string(&self.0) {
                let _ = std::process::Command::new("kill")
                    .args(["-KILL", pid.trim()])
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
    }
    let holder = tempfile::tempdir().unwrap();
    let pidfile = holder.path().join("holder.pid");
    let _reap = Reap(pidfile.clone());
    let script = holder.path().join("hold.pl");
    std::fs::write(
        &script,
        format!(
            "use POSIX qw(setsid);\nmy $pid = fork();\nif ($pid) {{\n    open(my $f, '>', '{}') or die;\n    print $f $pid;\n    close($f);\n    exit 0;\n}}\nsetsid();\nexec('sleep', '30');\n",
            pidfile.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let command = format!("printf \"violation\\n\"; perl {}", script.display());
    let (repo, _scripts) = guards_repo("", 3600, &ratchet("held", &command, "held.txt"));
    let dir = repo.path();
    write(dir, "guards/held.txt", "violation\nfixed\n");
    commit(dir, "ABC-1: baseline");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"guard: held: unverified: output cut"),
        "{text}"
    );
    assert!(text.contains("result: fail"), "{text}");
    assert_eq!(read(dir, "guards/held.txt"), "violation\nfixed\n");
    let record = last_check_record(dir);
    assert_eq!(record["guards"][0]["outcome"], "unverified", "{record}");
}

// ---- step 6: tightening ----

// AC-10: a passing ratchet guard writes its baseline without the fixed lines (order kept, first of
// equal lines only, each line ended by `\n`); a guard without fixed lines writes nothing; a failed
// write prints its line; the next check refuses the dirty tree.
#[test]
fn fixed_violations_leave_the_baseline() {
    use std::os::unix::fs::PermissionsExt;
    let guards = [
        ratchet("tight", "printf \"c\\na\\nb\\n\"", "tight.txt"),
        ratchet("same", "printf \"a\\n\"", "same.txt"),
        ratchet("locked", "printf \"a\\n\"", "locked.txt"),
        ratchet("failing", "printf \"a\\nnew\\n\"", "failing.txt"),
    ]
    .concat();
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/tight.txt", "a\nx\nb\nx\nc\nb\n\n");
    write(dir, "guards/same.txt", "a\r\n\r\na\n");
    write(dir, "guards/locked.txt", "a\nfixed\n");
    write(dir, "guards/failing.txt", "a\nfixed\n");
    commit(dir, "ABC-1: baselines");
    let locked = dir.join("guards/locked.txt");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o444)).unwrap();
    let (code, text) = run(dir, &["--level", "head"]);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(code, 1, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"guard: tight: pass, 1 fixed removed from guards/tight.txt; commit it"),
        "{text}"
    );
    assert_eq!(read(dir, "guards/tight.txt"), "a\nb\nc\n");
    assert!(lines.contains(&"guard: same: pass"), "{text}");
    assert_eq!(read(dir, "guards/same.txt"), "a\r\n\r\na\n");
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("guard: locked: pass; baseline not written: ")),
        "{text}"
    );
    assert_eq!(read(dir, "guards/locked.txt"), "a\nfixed\n");
    assert!(lines.contains(&"guard: failing: fail: 1 new"), "{text}");
    assert_eq!(read(dir, "guards/failing.txt"), "a\nfixed\n");
    let record = last_check_record(dir);
    let list = record["guards"].as_array().expect("guards");
    assert_eq!(list[0]["outcome"], "pass", "{record}");
    assert_eq!(list[0]["removed"], 1, "{record}");
    assert_eq!(list[1]["removed"], 0, "{record}");
    assert_eq!(list[2]["outcome"], "pass", "{record}");
    assert_eq!(list[2]["removed"], 0, "{record}");
    assert_eq!(list[3]["removed"], 0, "{record}");

    // The tightened baseline is an uncommitted change: the next check refuses it.
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("guards/tight.txt"), "{text}");
    assert!(text.contains("uncommitted"), "{text}");
}

// ---- step 10: check after tightening ----

// AC-15: a passing check stays current across baseline subsets, committed or not; any other change
// inside the guards folder, `docs/` too, makes it stale.
#[test]
fn a_tightening_commit_keeps_the_check() {
    use std::process::Command;

    /// A repository whose guards folder is `folder`, with a passing head check that has tightened
    /// the baseline `tight.txt` from "a b" to "a"; the status line of that pass.
    fn tightened(folder: &str) -> (tempfile::TempDir, tempfile::TempDir, String) {
        let guards = ratchet("tight", "printf \"a\\n\"", "tight.txt");
        let (repo, scripts) = guards_repo("", 3600, &guards);
        let dir = repo.path();
        if folder != "guards" {
            let text = read(dir, ".stapel/stapel.toml")
                .replace("dir = \"guards\"", &format!("dir = \"{folder}\""));
            write(dir, ".stapel/stapel.toml", &text);
        }
        write(dir, &format!("{folder}/tight.txt"), "a\nb\n");
        write(dir, &format!("{folder}/script.sh"), "echo 1\n");
        commit(dir, "ABC-1: guards folder");
        let (code, text) = run(dir, &["--level", "head"]);
        assert_eq!(code, 0, "{text}");
        assert_eq!(read(dir, &format!("{folder}/tight.txt")), "a\n");
        let head = Command::new("git")
            .args(["rev-parse", "--short=7", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
        (repo, scripts, format!("check: pass at {head}"))
    }

    fn status_line(dir: &Path) -> String {
        let out = stapel(dir).args(["status", "ABC-1"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find(|l| l.starts_with("check: "))
            .unwrap_or("(no check line)")
            .to_string()
    }

    // (label, folder, what changes after the check, committed, stays current)
    type Change = fn(&Path, &str);
    let cases: Vec<(&str, &str, Change, bool, bool)> = vec![
        ("tightened, uncommitted", "guards", |_, _| {}, false, true),
        ("tightened, committed", "guards", |_, _| {}, true, true),
        ("tightened under docs", "docs/guards", |_, _| {}, true, true),
        (
            "a line added",
            "guards",
            |d, f| write(d, &format!("{f}/tight.txt"), "a\nb\nc\n"),
            true,
            false,
        ),
        (
            "a script edited",
            "guards",
            |d, f| write(d, &format!("{f}/script.sh"), "echo 2\n"),
            true,
            false,
        ),
        (
            "a script edited under docs",
            "docs/guards",
            |d, f| write(d, &format!("{f}/script.sh"), "echo 2\n"),
            true,
            false,
        ),
        (
            "a script edited, uncommitted",
            "guards",
            |d, f| write(d, &format!("{f}/script.sh"), "echo 2\n"),
            false,
            false,
        ),
        (
            "the baseline deleted",
            "guards",
            |d, f| std::fs::remove_file(d.join(format!("{f}/tight.txt"))).unwrap(),
            true,
            false,
        ),
        (
            "a file absent at the record commit",
            "guards",
            |d, f| write(d, &format!("{f}/extra.txt"), "a\n"),
            true,
            false,
        ),
        (
            "[guards] changed",
            "guards",
            |d, _| {
                let text = read(d, ".stapel/stapel.toml");
                let text = text.replace("name = \"tight\"", "name = \"tight2\"");
                write(d, ".stapel/stapel.toml", &text);
            },
            true,
            false,
        ),
        (
            "a path beside the folder",
            "docs/guards",
            |d, _| write(d, "docs/guardsx/f", "x\n"),
            true,
            true,
        ),
    ];
    for (label, folder, change, committed, current) in cases {
        let (repo, _scripts, pass) = tightened(folder);
        let dir = repo.path();
        change(dir, folder);
        if committed {
            commit(dir, "ABC-1: after the check");
        }
        let line = status_line(dir);
        if current {
            assert_eq!(line, pass, "{label}");
        } else {
            assert!(line.starts_with("check: stale"), "{label}: {line}");
        }
    }

    // Close accepts the check after the tightened baseline is committed.
    let (repo, _scripts, pass) = tightened("guards");
    let dir = repo.path();
    commit(dir, "ABC-1: tighten the baseline");
    assert_eq!(status_line(dir), pass);
    common::confirm_these(
        dir,
        "ABC-1",
        &["description", "spec", "design", "proof", "plan"],
    );
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .assert()
        .success();

    // Without `[guards]` a `docs/` change keeps the check.
    let (repo, _scripts) = guards_repo("", 3600, "");
    let dir = repo.path();
    let text = read(dir, ".stapel/stapel.toml");
    let cut = text.find("\n[guards]").unwrap();
    write(dir, ".stapel/stapel.toml", &text[..cut]);
    commit(dir, "ABC-1: no guards");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    let pass = status_line(dir);
    assert!(pass.starts_with("check: pass at"), "{pass}");
    write(dir, "docs/note.md", "text\n");
    commit(dir, "ABC-1: a doc");
    assert_eq!(status_line(dir), pass);
}

// ---- step 12: tightening cases ----

// AC-10: a fixed line that returns after the tightening commit is a new violation; a check whose step
// fails still tightens a passing guard; a time-limit stop leaves the baseline unchanged.
#[test]
fn tightening_edge_cases() {
    // A fixed line that returns after the tightening commit fails as `1 new`.
    let out_dir = tempfile::tempdir().unwrap();
    let out = out_dir.path().join("out.txt");
    std::fs::write(&out, "a\n").unwrap();
    let guards = ratchet("back", &format!("cat {}", out.display()), "back.txt");
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/back.txt", "a\nb\n");
    commit(dir, "ABC-1: baseline");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    assert_eq!(read(dir, "guards/back.txt"), "a\n");
    commit(dir, "ABC-1: tighten the baseline");
    std::fs::write(&out, "a\nb\n").unwrap();
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.lines().any(|l| l.trim() == "guard: back: fail: 1 new"),
        "{text}"
    );
    assert_eq!(read(dir, "guards/back.txt"), "a\n");

    // A check whose step fails still tightens a passing guard.
    let guards = ratchet("tight", "printf \"a\\n\"", "tight.txt");
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/tight.txt", "a\nfixed\n");
    write(
        dir,
        "crates/tiny/tests/broken.rs",
        "#[test]\nfn needs_three() {\n    // fake: needs pub fn three\n}\n",
    );
    commit(dir, "ABC-1: a failing test and a baseline");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("result: fail"), "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim()
                == "guard: tight: pass, 1 fixed removed from guards/tight.txt; commit it"),
        "{text}"
    );
    assert_eq!(read(dir, "guards/tight.txt"), "a\n");

    // A time-limit stop leaves the baseline unchanged.
    let guards = ratchet("slow", "echo a; exec sleep 329", "slow.txt");
    let (repo, _scripts) = guards_repo("", 10, &guards);
    let dir = repo.path();
    write(dir, "guards/slow.txt", "a\nfixed\n");
    commit(dir, "ABC-1: baseline");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim() == "guard: slow: unverified: time limit"),
        "{text}"
    );
    assert_eq!(read(dir, "guards/slow.txt"), "a\nfixed\n");
    let left = std::process::Command::new("pgrep")
        .args(["-f", "sleep 329"])
        .output()
        .unwrap();
    assert!(
        left.stdout.is_empty(),
        "a child survived: {}",
        String::from_utf8_lossy(&left.stdout)
    );
}

// ---- step 13: off-HEAD baseline ----

// AC-10: a closed ticket is checked at its end commit; a check of a commit that is not HEAD writes no
// baseline into the working tree.
#[test]
fn no_tightening_off_head() {
    let guards = ratchet("tight", "printf \"a\\n\"", "tight.txt");
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/tight.txt", "a\nfixed\n");
    commit(dir, "ABC-1: baseline");
    let mut state = common::state_json(dir, "ABC-1");
    state["closed"] =
        serde_json::json!({"by": "test-user", "at": "2026-01-02T00:00:00Z", "reason": "done"});
    write(
        dir,
        ".stapel/tickets/ABC-1/state.json",
        &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
    );
    commit(dir, "ABC-1: close");
    let short = |rev: &str| {
        let out = std::process::Command::new("git")
            .args(["rev-parse", "--short=7", rev])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    let end = short("HEAD");
    // A later commit of another ticket: the person adds a line to the baseline.
    write(dir, "guards/tight.txt", "a\nfixed\nadded\n");
    commit(dir, "XYZ-2: a later baseline");
    assert_ne!(short("HEAD"), end);

    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    let want = format!("guard: tight: pass; baseline not tightened: checked {end} is not HEAD");
    assert!(text.lines().any(|l| l.trim() == want), "{text}");
    assert_eq!(read(dir, "guards/tight.txt"), "a\nfixed\nadded\n");
    let status = std::process::Command::new("git")
        .args(["status", "--porcelain", "--", "guards"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(status.stdout.is_empty(), "the guards folder changed");
}

// ---- step 14: linked baseline ----

// AC-10: a baseline that is a symlink in the working tree is not written; neither the link nor its
// target changes.
#[test]
fn a_linked_baseline_is_not_written() {
    use std::os::unix::fs::symlink;
    // A symlink is committed as a blob holding its target text, so that text is the baseline: the
    // targets carry a second line ("\nfixed") so that one baseline line is fixed and one still prints.
    let other = tempfile::tempdir().unwrap();
    let far_path = other.path().join("far.txt");
    let far = other.path().join("far.txt\nfixed");
    let guards = [
        ratchet("inside", "printf \"../notes.txt\\n\"", "inside.txt"),
        ratchet(
            "outside",
            &format!("printf \"{}\\n\"", far_path.display()),
            "outside.txt",
        ),
    ]
    .concat();
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    std::fs::write(&far, "far\n").unwrap();
    write(dir, "notes.txt\nfixed", "notes\n");
    std::fs::create_dir_all(dir.join("guards")).unwrap();
    // One link to a file outside the guards folder, one to a file outside the repository.
    symlink("../notes.txt\nfixed", dir.join("guards/inside.txt")).unwrap();
    symlink(&far, dir.join("guards/outside.txt")).unwrap();
    commit(dir, "ABC-1: linked baselines");
    let notes = read(dir, "notes.txt\nfixed");
    let (_code, text) = run(dir, &["--level", "head"]);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    for name in ["inside", "outside"] {
        let want = format!("guard: {name}: pass; baseline not written: a symlink");
        assert!(lines.contains(&want.as_str()), "{text}");
    }
    assert_eq!(read(dir, "notes.txt\nfixed"), notes);
    assert_eq!(std::fs::read_to_string(&far).unwrap(), "far\n");
    let link = |rel: &str| std::fs::read_link(dir.join(rel)).unwrap();
    assert_eq!(link("guards/inside.txt"), Path::new("../notes.txt\nfixed"));
    assert_eq!(link("guards/outside.txt"), far);
}

// ---- step 15: crashing guard ----

// AC-10: a ratchet guard that exits 1 with standard error is a crash, not a clean list: the baseline
// stays; exit 1 with empty standard error (a linter) still tightens.
#[test]
fn a_crashing_guard_keeps_the_baseline() {
    let guards = [
        ratchet("crash", "echo Traceback >&2; exit 1", "crash.txt"),
        ratchet(
            "crashone",
            "printf \"a\\n\"; echo boom >&2; exit 1",
            "crashone.txt",
        ),
        ratchet("linter", "printf \"a\\n\"; exit 1", "linter.txt"),
    ]
    .concat();
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/crash.txt", "a\nb\nc\n");
    write(dir, "guards/crashone.txt", "a\nb\nc\n");
    write(dir, "guards/linter.txt", "a\nfixed\n");
    commit(dir, "ABC-1: baselines");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    for name in ["crash", "crashone"] {
        let want = format!("guard: {name}: pass; baseline not tightened: standard error");
        assert!(lines.contains(&want.as_str()), "{text}");
    }
    assert!(
        lines.contains(&"guard: linter: pass, 1 fixed removed from guards/linter.txt; commit it"),
        "{text}"
    );
    assert_eq!(read(dir, "guards/crash.txt"), "a\nb\nc\n");
    assert_eq!(read(dir, "guards/crashone.txt"), "a\nb\nc\n");
    assert_eq!(read(dir, "guards/linter.txt"), "a\n");
    let record = last_check_record(dir);
    assert_eq!(record["guards"][0]["removed"], 0, "{record}");
    assert_eq!(record["guards"][1]["removed"], 0, "{record}");
    assert_eq!(record["guards"][2]["removed"], 1, "{record}");
}

#[test]
fn a_silent_or_masked_crash_keeps_the_baseline() {
    let guards = [
        ratchet("masked", "grep -rn x srcx | sort", "masked.txt"),
        ratchet("silent", "[ -x tools/lint ] || exit 1", "silent.txt"),
        ratchet("control", "printf \"a\\n\"; exit 0", "control.txt"),
    ]
    .concat();
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "guards/masked.txt", "a\nb\n");
    write(dir, "guards/silent.txt", "a\nb\n");
    write(dir, "guards/control.txt", "a\nb\n");
    commit(dir, "ABC-1: baselines");
    let (code, text) = run(dir, &["--level", "head"]);
    assert_eq!(code, 0, "{text}");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"guard: masked: pass; baseline not tightened: standard error"),
        "{text}"
    );
    assert!(
        lines.contains(&"guard: silent: pass; baseline not tightened: no output"),
        "{text}"
    );
    assert!(
        lines.contains(&"guard: control: pass, 1 fixed removed from guards/control.txt; commit it"),
        "{text}"
    );
    assert_eq!(read(dir, "guards/masked.txt"), "a\nb\n");
    assert_eq!(read(dir, "guards/silent.txt"), "a\nb\n");
    assert_eq!(read(dir, "guards/control.txt"), "a\n");
}

// STP-43 AC-3: a guard's command that reads an existing file outside `dir` makes one warning line on
// standard error; the run goes on with the same outcomes.
#[test]
fn warns_about_a_file_outside_dir() {
    let guards = [
        guard("a", "sh scripts/cov.sh; ./scripts/cov.sh; sh scripts/cov.sh"),
        guard("b", "true --config=clippy.toml"),
        guard("c", "true --config=\"clippy.toml\" --config=clippy.toml"),
        guard("d", "true --config clippy.toml"),
        guard("e", "sh guards/run.sh"),
        guard("f", "grep -rn TODO src || true; grep TODO README.md || true"),
        guard(
            "g",
            "missing/x.sh || true; target/dangling.sh || true; $HOME/x || true; *.rs || true; /abs/x || true; ../x || true; scripts/./cov.sh || true",
        ),
        guard("h", "sh target/l.sh || true"),
    ]
    .concat();
    let (repo, _scripts) = guards_repo("", 3600, &guards);
    let dir = repo.path();
    write(dir, "scripts/cov.sh", "exit 0\n");
    write(dir, "guards/run.sh", "exit 0\n");
    write(dir, "clippy.toml", "\n");
    write(dir, "README.md", "readme\n");
    write(dir, "src/lib.rs", "\n");
    commit(dir, "ABC-1: guard files");
    // An ignored folder holds the links: a link in the tracked tree stops the check.
    std::fs::create_dir_all(dir.join("target")).unwrap();
    std::os::unix::fs::symlink("../scripts/cov.sh", dir.join("target/l.sh")).unwrap();
    std::os::unix::fs::symlink("no-such-target", dir.join("target/dangling.sh")).unwrap();

    let warning = |name: &str, word: &str| {
        format!(
            "warning: guard {name} reads {word} outside guards; a guard's scripts and configs belong in guards"
        )
    };
    let expected = [
        warning("a", "scripts/cov.sh"),
        warning("b", "clippy.toml"),
        warning("c", "clippy.toml"),
        warning("d", "clippy.toml"),
        warning("h", "target/l.sh"),
    ];
    let both = |level: &str| {
        let out = stapel(dir)
            .args(["check", "ABC-1", "--level", level])
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    for level in ["head", "full"] {
        let (code, out, err) = both(level);
        let seen: Vec<&str> = err
            .lines()
            .filter(|l| l.starts_with("warning: guard "))
            .collect();
        assert_eq!(
            seen,
            expected.iter().map(String::as_str).collect::<Vec<_>>(),
            "{level}: {err}"
        );
        assert_eq!(code, 0, "{level}: {out}{err}");
        for name in ["a", "b", "c", "d", "e", "f", "g", "h"] {
            assert!(
                out.lines()
                    .any(|l| l.trim() == format!("guard: {name}: pass")),
                "{level} {name}: {out}"
            );
        }
        assert!(!out.contains("warning: guard"), "{level}: {out}");
    }
    let (code, out, err) = both("steps");
    assert_eq!(code, 0, "{out}{err}");
    assert!(!err.contains("warning: guard"), "{err}");

    // DR1-1: `dir` is compared in lower case, so `dir = "Guards"` covers `guards/run.sh`.
    let toml = read(dir, ".stapel/stapel.toml").replace("dir = \"guards\"", "dir = \"Guards\"");
    write(dir, ".stapel/stapel.toml", &toml);
    commit(dir, "ABC-1: dir in capitals");
    let (code, out, err) = both("head");
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        !err.contains("guard e reads"),
        "dir = Guards covers guards/run.sh: {err}"
    );
    assert!(
        err.contains("warning: guard a reads scripts/cov.sh outside Guards;"),
        "{err}"
    );
}
