//! STP-14 AC-1 to AC-10: `stapel check --history`, the history outcomes without a cargo run.

mod common;

use common::{read, stapel};
use std::path::Path;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Commits everything with fixed author, committer and dates, so commit ids repeat.
fn commit(dir: &Path, message: &str) {
    let n = std::process::Command::new("git")
        .args(["rev-list", "--count", "--all"])
        .current_dir(dir)
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u32>()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    let date = format!("2026-01-01T00:{:02}:{:02}Z", n / 60, n % 60);
    git(dir, &["add", "-A"]);
    let out = std::process::Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", message])
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "test-user")
        .env("GIT_AUTHOR_EMAIL", "test-user.invalid")
        .env("GIT_COMMITTER_NAME", "test-user")
        .env("GIT_COMMITTER_EMAIL", "test-user.invalid")
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_DATE", &date)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

const LIB: &str = "pub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n";

/// A stapel repository with a one-crate cargo workspace (`crates/tiny`) and ticket ABC-1, all
/// committed.
fn cargo_repo() -> TempDir {
    fill_cargo_repo(common::git_repo())
}

/// The cargo repository of `cargo_repo` in an existing git repository.
fn fill_cargo_repo(repo: TempDir) -> TempDir {
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
    write(dir, "crates/tiny/src/lib.rs", LIB);
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn starts() {\n    assert_eq!(tiny::add(1, 1), 2);\n}\n",
    );
    write(dir, ".gitignore", "/target\n");
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    stapel(dir).args(["new", "First"]).assert().success();
    // `stapel check` runs cargo with `--locked`, so the lock file is part of the repository.
    let lock = std::process::Command::new("cargo")
        .args(["generate-lockfile", "--offline"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    commit(dir, "initial");
    repo
}

/// Appends a test function to `crates/tiny/tests/<file>.rs`.
fn add_test(dir: &Path, file: &str, name: &str, body: &str) {
    let rel = format!("crates/tiny/tests/{file}.rs");
    let old = std::fs::read_to_string(dir.join(&rel)).unwrap_or_default();
    write(
        dir,
        &rel,
        &format!("{old}\n#[test]\nfn {name}() {{\n    {body}\n}}\n"),
    );
}

/// Replaces text in a file.
fn edit(dir: &Path, rel: &str, from: &str, to: &str) {
    let old = read(dir, rel);
    assert!(old.contains(from), "{rel} has no {from:?}");
    write(dir, rel, &old.replacen(from, to, 1));
}

/// One full `stapel check` at a time, with two build jobs: every run builds with cargo, and many
/// at once exhaust the machine's memory.
static CARGO: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn full_check(dir: &Path) -> std::process::Output {
    let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
    stapel(dir)
        .args(["check", "ABC-1"])
        .env("CARGO_BUILD_JOBS", "2")
        .output()
        .unwrap()
}

/// `stapel check ABC-1`: exit code and output.
fn check(dir: &Path) -> (i32, String) {
    let out = full_check(dir);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
}

/// The outcome printed for a label: the word after `<label>: `.
fn outcome<'a>(text: &'a str, label: &str) -> &'a str {
    let prefix = format!("{label}: ");
    text.lines()
        .find_map(|l| l.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("no line for {label}:\n{text}"))
        .split([' ', ':'])
        .next()
        .unwrap()
}

/// Commit ids replaced by `<sha>`, so golden files hold no ids.
fn without_ids(text: &str) -> String {
    text.lines()
        .map(|line| {
            let words: Vec<&str> = line
                .split(' ')
                .map(|w| {
                    if w.len() == 7
                        && w.chars()
                            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                    {
                        "<sha>"
                    } else {
                        w
                    }
                })
                .collect();
            format!("{}\n", words.join(" "))
        })
        .collect()
}

// ---- helpers of this file ----

/// `stapel check <args> --history`: exit code, standard output and standard error.
fn history_args(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = stapel(dir)
        .args(["check"])
        .args(args)
        .arg("--history")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// `stapel check ABC-1 --history`.
fn history(dir: &Path) -> (i32, String, String) {
    history_args(dir, &["ABC-1"])
}

/// A library function `triple` that is wrong at first, committed before the steps.
fn with_wrong_triple(dir: &Path) {
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{LIB}\npub fn triple(x: u32) -> u32 {{\n    x * 2\n}}\n"),
    );
    commit(dir, "ABC-1: a library function to fix");
}

fn fix_triple(dir: &Path) {
    edit(dir, "crates/tiny/src/lib.rs", "x * 2", "x * 3");
}

/// Appends a library function.
fn add_fn(dir: &Path, name: &str) {
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{lib}\npub fn {name}() -> u32 {{\n    1\n}}\n"),
    );
}

/// A clean pair for `label`: the RED adds a test that fails, the GREEN adds the function.
fn clean_pair(dir: &Path, label: &str, name: &str) {
    add_test(dir, name, name, &format!("assert_eq!(tiny::{name}(), 1);"));
    commit(dir, &format!("ABC-1 {label} RED: {name} is missing"));
    add_fn(dir, name);
    commit(dir, &format!("ABC-1 {label} GREEN: {name}"));
}

/// Two REDs, then their GREENs: the first step reads `interleaved`.
fn interleaved_pairs(dir: &Path, first: &str, second: &str) {
    add_test(dir, "outer", "outer", "assert_eq!(tiny::outer(), 1);");
    commit(dir, &format!("ABC-1 {first} RED: outer is missing"));
    add_test(dir, "inner", "inner", "assert_eq!(tiny::inner(), 1);");
    commit(dir, &format!("ABC-1 {second} RED: inner is missing"));
    add_fn(dir, "outer");
    commit(dir, &format!("ABC-1 {first} GREEN: outer"));
    add_fn(dir, "inner");
    commit(dir, &format!("ABC-1 {second} GREEN: inner"));
}

/// One step of each history outcome and one clean pair (step 1):
/// step 2 `unpaired`, step 3 `duplicate`, step 4 `no-tests`, step 5 `red-changes-code`,
/// step 6 `tests-changed`, step 7 the notes `superseded by` and `retired by`, step 8 `retired`,
/// step 9 the spec change, step 10 `interleaved` by step 11.
fn every_outcome(dir: &Path) {
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");

    add_test(dir, "lonely", "lonely", "assert!(true);");
    commit(dir, "ABC-1 step 2 RED: a red without a green");

    add_test(dir, "twice", "first", "assert!(true);");
    commit(dir, "ABC-1 step 3 RED: red");
    add_test(dir, "twice", "second", "assert!(true);");
    commit(dir, "ABC-1 step 3 RED: red again");
    commit(dir, "ABC-1 step 3 GREEN: green");

    commit(dir, "ABC-1 step 4 RED: no test");
    commit(dir, "ABC-1 step 4 GREEN: code");

    add_test(dir, "code", "five", "assert_eq!(tiny::five(), 1);");
    add_fn(dir, "five");
    commit(dir, "ABC-1 step 5 RED: test and code");
    commit(dir, "ABC-1 step 5 GREEN: green");

    add_test(dir, "changed", "six", "assert!(true);");
    commit(dir, "ABC-1 step 6 RED: six");
    commit(dir, "ABC-1 step 6 GREEN: green");
    edit(
        dir,
        "crates/tiny/tests/changed.rs",
        "fn six() {\n",
        "fn six() {\n    let _changed = 1;\n",
    );
    commit(dir, "ABC-1: edits the test of step 6");

    add_test(dir, "rules", "evolves", "assert_eq!(tiny::add(1, 1), 2);");
    add_test(dir, "rules", "wrong_rule", "assert!(true);");
    commit(dir, "ABC-1 step 7 RED: two rules");
    commit(dir, "ABC-1 step 7 GREEN: green");
    add_test(dir, "rules", "only_one", "assert!(true);");
    commit(dir, "ABC-1 step 8 RED: one rule");
    commit(dir, "ABC-1 step 8 GREEN: green");
    add_test(dir, "rules", "new_rule", "assert_eq!(tiny::rule(), 1);");
    edit(
        dir,
        "crates/tiny/tests/rules.rs",
        "assert_eq!(tiny::add(1, 1), 2);",
        "assert_eq!(tiny::add(1, 1), 1 + tiny::rule());",
    );
    commit(dir, "ABC-1 step 9 RED: the new rule");
    add_fn(dir, "rule");
    edit(
        dir,
        "crates/tiny/tests/rules.rs",
        "#[test]\nfn wrong_rule() {\n    assert!(true);\n}\n",
        "",
    );
    edit(
        dir,
        "crates/tiny/tests/rules.rs",
        "#[test]\nfn only_one() {\n    assert!(true);\n}\n",
        "",
    );
    commit(
        dir,
        "ABC-1 step 9 GREEN: rule\n\nspec change: AC-1, the old rules were wrong",
    );

    interleaved_pairs(dir, "step 10", "step 11");
}

/// The step blocks of a report: label, outcome line and note lines, in order.
fn blocks(text: &str) -> Vec<(String, String, Vec<String>)> {
    let mut out: Vec<(String, String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        if let Some(note) = line.strip_prefix("  ") {
            if let Some(last) = out.last_mut() {
                last.2.push(note.to_string());
            }
            continue;
        }
        if !line.starts_with("step ") {
            continue;
        }
        let label = line.split(':').next().unwrap().to_string();
        out.push((label, line.to_string(), Vec::new()));
    }
    out
}

/// The non-step lines a check prints first: `ticket:` and `build inputs:`.
fn header(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.starts_with("ticket: ") || l.starts_with("build inputs: "))
        .map(String::from)
        .collect()
}

// ---- AC-1 ----

#[test]
fn prints_the_full_check_form() {
    let repo = cargo_repo();
    let dir = repo.path();
    every_outcome(dir);
    let (code, out, err) = history(dir);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(err, "", "{out}");
    assert!(
        !out.lines()
            .any(|l| l.starts_with("cargo:") || l.starts_with("suite:")),
        "{out}"
    );
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/check_history.txt"),
    )
    .unwrap();
    assert_eq!(without_ids(&out), golden, "actual:\n{out}");
    let head = git(dir, &["rev-parse", "--short=7", "HEAD"]);
    assert_eq!(
        out.lines().next().unwrap(),
        format!("ticket: ABC-1 at {}", head.trim())
    );
}

// ---- AC-2 ----

#[test]
fn same_lines_as_the_full_check() {
    let repo = cargo_repo();
    let dir = repo.path();
    every_outcome(dir);
    let (_, full) = check(dir);
    let (_, hist, err) = history(dir);
    assert_eq!(err, "", "{hist}");
    assert_eq!(header(&full), header(&hist), "{full}\n{hist}");
    let full_blocks = blocks(&full);
    let hist_blocks = blocks(&hist);
    assert_eq!(
        full_blocks.iter().map(|b| &b.0).collect::<Vec<_>>(),
        hist_blocks.iter().map(|b| &b.0).collect::<Vec<_>>(),
        "{full}\n{hist}"
    );
    let mut kinds = Vec::new();
    for (f, h) in full_blocks.iter().zip(&hist_blocks) {
        // The notes are equal for every step.
        assert_eq!(f.2, h.2, "{}\n{full}\n{hist}", f.0);
        if h.1 == format!("{}: needs-run", h.0) {
            continue;
        }
        assert_eq!(f.1, h.1, "{full}\n{hist}");
        kinds.push(outcome(&hist, &h.0).to_string());
        if h.1.contains("interleaved") {
            kinds.push("interleaved".into());
        }
    }
    for kind in [
        "unpaired",
        "duplicate",
        "no-tests",
        "red-changes-code",
        "tests-changed",
        "retired",
        "interleaved",
    ] {
        assert!(kinds.iter().any(|k| k == kind), "{kind}\n{hist}");
    }
    let notes: Vec<&String> = hist_blocks.iter().flat_map(|b| &b.2).collect();
    for note in ["superseded by ", "retired by "] {
        assert!(notes.iter().any(|n| n.starts_with(note)), "{note}\n{hist}");
    }
    // The clean pair passes in the full check and needs a run here.
    assert_eq!(outcome(&full, "step 1"), "pass", "{full}");
    assert!(hist.lines().any(|l| l == "step 1: needs-run"), "{hist}");
}

#[test]
fn same_lines_when_a_build_input_changed() {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");
    add_test(dir, "manifest", "described", "assert!(true);");
    // A target table of a manifest is a build input.
    edit(
        dir,
        "crates/tiny/Cargo.toml",
        "edition = \"2021\"\n",
        "edition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n",
    );
    commit(dir, "ABC-1 step 2 RED: a test and a manifest change");
    commit(dir, "ABC-1 step 2 GREEN: green");
    let changed = git(dir, &["rev-parse", "--short=7", "HEAD~1"])
        .trim()
        .to_string();
    clean_pair(dir, "step 3", "later");
    interleaved_pairs(dir, "step 4", "step 5");
    let (code, full) = check(dir);
    assert_eq!(code, 1, "{full}");
    let (code, hist, err) = history(dir);
    assert_eq!(code, 1, "{hist}{err}");
    assert_eq!(header(&full), header(&hist), "{full}\n{hist}");
    assert_eq!(blocks(&full), blocks(&hist), "{full}\n{hist}");
    let want = format!("unverified: build-input-changed: crates/tiny/Cargo.toml at {changed}");
    for label in ["step 1", "step 3", "step 4", "step 5"] {
        assert!(
            hist.lines().any(|l| l == format!("{label}: {want}")),
            "{label}\n{hist}"
        );
    }
    assert_eq!(outcome(&hist, "step 2"), "red-changes-code", "{hist}");
}

// ---- AC-3 ----

#[test]
fn exit_code_follows_the_outcomes() {
    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    clean_pair(dir, "step 2", "two");
    let (code, out, err) = history(dir);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.lines().any(|l| l == "step 1: needs-run"), "{out}");
    assert!(out.lines().any(|l| l == "step 2: needs-run"), "{out}");
    assert_eq!(
        out.lines().last(),
        Some("result: no history faults"),
        "{out}"
    );

    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    add_test(dir, "lonely", "lonely", "assert!(true);");
    commit(dir, "ABC-1 step 2 RED: a red without a green");
    let (code, out, err) = history(dir);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(outcome(&out, "step 2"), "unpaired", "{out}");
    assert_eq!(out.lines().last(), Some("result: fail"), "{out}");

    let repo = cargo_repo();
    let dir = repo.path();
    interleaved_pairs(dir, "step 1", "step 2");
    let (code, out, err) = history(dir);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        out.lines()
            .any(|l| l == "step 1: unverified: interleaved: step 2"),
        "{out}"
    );
    assert!(out.lines().any(|l| l == "step 2: needs-run"), "{out}");
    assert_eq!(out.lines().last(), Some("result: fail"), "{out}");
}

// ---- AC-4 ----

#[test]
fn refuses_as_the_full_check() {
    /// Both checks on `key`: exit 2 with the same text, and nothing on the history's stdout.
    fn same_refusal(dir: &Path, key: &str, what: &str, want: &str) {
        let out = {
            let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
            stapel(dir).args(["check", key]).output().unwrap()
        };
        let full = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.status.code(), Some(2), "{what}: {full}");
        assert!(full.contains(want), "{what}: {full}");
        let (code, hist, err) = history_args(dir, &[key]);
        assert_eq!(code, 2, "{what}: {hist}{err}");
        assert_eq!(hist, "", "{what}");
        assert_eq!(err, full, "{what}");
    }

    // A legacy and an unreadable ticket.
    let repo = cargo_repo();
    let dir = repo.path();
    write(
        dir,
        ".stapel/tickets/ABC-9/state.json",
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    );
    write(dir, ".stapel/tickets/ABC-8/state.json", "not json");
    clean_pair(dir, "step 1", "one");
    same_refusal(dir, "ABC-9", "legacy", "legacy");
    same_refusal(dir, "ABC-8", "unreadable", "ABC-8");

    // A shallow clone.
    let away = tempfile::tempdir().unwrap();
    let clone = away.path().join("clone");
    git(
        away.path(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            &format!("file://{}", dir.display()),
            "clone",
        ],
    );
    common::git_config(&clone, "user.name", "test-user");
    same_refusal(&clone, "ABC-1", "shallow", "shallow");

    // A base that is not an ancestor of HEAD.
    let mut state = common::state_json(dir, "ABC-1");
    state["base"] = serde_json::json!("0".repeat(40));
    write(
        dir,
        ".stapel/tickets/ABC-1/state.json",
        &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
    );
    commit(dir, "ABC-1: bad base");
    same_refusal(dir, "ABC-1", "base", "state.json has base");

    // A closed ticket without commits of its key.
    let repo = cargo_repo();
    let dir = repo.path();
    let mut state = common::state_json(dir, "ABC-1");
    state["closed"] =
        serde_json::json!({"by": "test-user", "at": "2026-01-02T00:00:00Z", "reason": "done"});
    write(
        dir,
        ".stapel/tickets/ABC-1/state.json",
        &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
    );
    commit(dir, "setup: a closed ticket without commits of its own");
    same_refusal(dir, "ABC-1", "closed", "is closed and has no commit");

    // No step commits.
    let repo = cargo_repo();
    let dir = repo.path();
    same_refusal(dir, "ABC-1", "no steps", "no step commits");
}

// ---- AC-5 ----

#[test]
fn runs_without_the_run_preconditions() {
    fn runs(dir: &Path, what: &str) {
        let (code, out, err) = history(dir);
        assert_eq!(code, 0, "{what}: {out}{err}");
        assert!(
            out.lines().any(|l| l == "step 1: needs-run"),
            "{what}: {out}"
        );
        assert_eq!(
            out.lines().last(),
            Some("result: no history faults"),
            "{what}: {out}"
        );
    }

    // A dirty tree.
    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{}\n// uncommitted\n", read(dir, "crates/tiny/src/lib.rs")),
    );
    write(dir, "crates/tiny/tests/untracked.rs", "\n");
    runs(dir, "dirty tree");

    // No `[check]` in stapel.toml.
    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    let config = read(dir, ".stapel/stapel.toml");
    let without: String = config
        .lines()
        .filter(|l| {
            !(l.starts_with("[check]") || l.starts_with("runner") || l.starts_with("timeout_secs"))
        })
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(without, config);
    write(dir, ".stapel/stapel.toml", &without);
    commit(dir, "ABC-1: no check section");
    runs(dir, "no [check]");

    // Another check holds the lock.
    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    let lock_path = dir.join(".git/stapel/check.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();
    held.try_lock().unwrap();
    std::fs::write(&lock_path, format!("{}\n", std::process::id())).unwrap();
    runs(dir, "lock held");
    held.unlock().unwrap();
}

// ---- AC-6 ----

#[test]
fn writes_nothing_and_starts_no_cargo() {
    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    add_test(dir, "manifest", "described", "assert!(true);");
    commit(dir, "ABC-1 step 2 RED: a red without a green");
    let bin = tempfile::tempdir().unwrap();
    let marker = bin.path().join("cargo-started");
    let cargo = bin.path().join("cargo");
    std::fs::write(
        &cargo,
        format!(
            "#!/bin/sh\necho started >> '{}'\nexit 1\n",
            marker.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let git_stapel = |dir: &Path| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir.join(".git/stapel"))
            .map(|r| {
                r.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    };
    let tree = common::snapshot(dir);
    let worktrees = git(dir, &["worktree", "list"]);
    let machine = git_stapel(dir);
    let runs = dir.join(".stapel/tickets/ABC-1/runs.jsonl");
    let records = std::fs::read(&runs).ok();
    let out = stapel(dir)
        .args(["check", "ABC-1", "--history"])
        .env("PATH", &path)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.lines().any(|l| l == "step 1: needs-run"), "{text}");
    assert!(!marker.exists(), "cargo was started\n{text}");
    assert_eq!(common::snapshot(dir), tree, "{text}");
    assert_eq!(git(dir, &["worktree", "list"]), worktrees, "{text}");
    assert_eq!(git_stapel(dir), machine, "{text}");
    assert_eq!(std::fs::read(&runs).ok(), records, "{text}");
}

// ---- AC-7 ----

#[test]
fn closed_ticket_at_the_end_of_its_range() {
    let repo = cargo_repo();
    let dir = repo.path();
    write(dir, "crates/tiny/tests/golden/out.txt", "four\n");
    commit(dir, "setup: a golden file before the ticket");
    add_test(
        dir,
        "golden",
        "renders",
        "assert_eq!(tiny::render(), include_str!(\"golden/out.txt\"));",
    );
    commit(dir, "ABC-1 step 1 RED: compare with the golden file");
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{lib}\npub fn render() -> &'static str {{\n    \"four\\n\"\n}}\n"),
    );
    commit(dir, "ABC-1 step 1 GREEN: render");
    // Before the close, a later change of the golden file by another ticket is `tests-changed`.
    let open = tempfile::tempdir().unwrap();
    let open_clone = open.path().join("clone");
    git(
        open.path(),
        &["clone", "-q", &format!("file://{}", dir.display()), "clone"],
    );
    common::git_config(&open_clone, "user.name", "test-user");
    write(
        &open_clone,
        "crates/tiny/tests/golden/out.txt",
        "four\nmore\n",
    );
    commit(&open_clone, "XYZ-2: another ticket changes the golden file");
    let (code, out, err) = history(&open_clone);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(outcome(&out, "step 1"), "tests-changed", "{out}");

    let mut state = common::state_json(dir, "ABC-1");
    state["closed"] =
        serde_json::json!({"by": "test-user", "at": "2026-01-02T00:00:00Z", "reason": "done"});
    write(
        dir,
        ".stapel/tickets/ABC-1/state.json",
        &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
    );
    commit(dir, "ABC-1: close");
    let end = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let (code, at_end, err) = history(dir);
    assert_eq!(code, 0, "{at_end}{err}");
    assert_eq!(
        at_end.lines().next().unwrap(),
        format!("ticket: ABC-1 at {} (closed)", &end[..7]),
        "{at_end}"
    );
    assert!(at_end.lines().any(|l| l == "step 1: needs-run"), "{at_end}");

    write(dir, "crates/tiny/tests/golden/out.txt", "four\nmore\n");
    commit(dir, "XYZ-2: another ticket changes the golden file");
    let (code, later, err) = history(dir);
    assert_eq!(code, 0, "{later}{err}");
    assert_eq!(later, at_end);
    // A detached HEAD is read as HEAD.
    git(dir, &["checkout", "-q", "--detach"]);
    let (code, detached, err) = history(dir);
    assert_eq!(code, 0, "{detached}{err}");
    assert_eq!(detached, at_end);
}

// ---- AC-8 ----

#[test]
fn history_with_list_is_a_usage_error() {
    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    for args in [
        ["check", "ABC-1", "--history", "--list"],
        ["check", "ABC-1", "--list", "--history"],
    ] {
        let out = stapel(dir).args(args).output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains("cannot be used with"), "{args:?}: {stderr}");
        assert!(stderr.contains("Usage:"), "{args:?}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "", "{args:?}");
    }
}

// ---- AC-9 ----

#[test]
fn does_not_count_for_close() {
    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    assert!(read(dir, ".stapel/stapel.toml").contains("[check]"));
    let (code, out, err) = history(dir);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out.lines().last(),
        Some("result: no history faults"),
        "{out}"
    );
    assert!(!dir.join(".stapel/tickets/ABC-1/runs.jsonl").exists());
    let out = stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0), "{stderr}");
    assert!(
        stderr.contains("close needs a current passing check"),
        "{stderr}"
    );
    assert!(common::state_json(dir, "ABC-1").get("closed").is_none());
}

// ---- AC-10 ----

#[test]
fn twenty_steps_within_five_seconds() {
    let repo = cargo_repo();
    let dir = repo.path();
    for n in 1..=20 {
        clean_pair(dir, &format!("step {n}"), &format!("f{n}"));
    }
    let start = std::time::Instant::now();
    let (code, out, err) = history(dir);
    let took = start.elapsed();
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out.lines().filter(|l| l.ends_with(": needs-run")).count(),
        20,
        "{out}"
    );
    assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
}
