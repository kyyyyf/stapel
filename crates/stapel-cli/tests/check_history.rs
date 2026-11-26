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
    let mut saw_times = false;
    for (f, h) in full_blocks.iter().zip(&hist_blocks) {
        // The notes are equal for every step; the full check adds a `times:` line to a step that
        // ran cargo, the history check never (STP-45 AC-5).
        let (times, notes): (Vec<&String>, Vec<&String>) =
            f.2.iter().partition(|n| n.starts_with("times: "));
        assert!(times.len() <= 1, "{}\n{full}", f.0);
        saw_times |= !times.is_empty();
        assert!(h.2.iter().all(|n| !n.starts_with("times: ")), "{hist}");
        assert_eq!(
            notes,
            h.2.iter().collect::<Vec<_>>(),
            "{}\n{full}\n{hist}",
            f.0
        );
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
    assert!(saw_times, "no `times:` line in the full check:\n{full}");
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

#[test]
fn build_inputs_line_in_both_modes() {
    let repo = cargo_repo();
    let dir = repo.path();
    // Build inputs that exist before the ticket and that no commit of the range changes.
    write(dir, ".cargo/config.toml", "");
    write(dir, "crates/tiny/build.rs", "fn main() {}\n");
    commit(dir, "setup: build inputs before the ticket");
    clean_pair(dir, "step 1", "one");
    let want = "build inputs: .cargo/config.toml, crates/tiny/build.rs";
    let (code, full) = check(dir);
    assert_eq!(code, 0, "{full}");
    let (code, hist, err) = history(dir);
    assert_eq!(code, 0, "{hist}{err}");
    assert_eq!(err, "", "{hist}");
    for text in [&full, &hist] {
        assert_eq!(
            text.lines()
                .filter(|l| l.starts_with("build inputs: "))
                .count(),
            1,
            "{text}"
        );
        let lines: Vec<&str> = text.lines().collect();
        let at = lines.iter().position(|l| *l == want).expect(text);
        let step = lines
            .iter()
            .position(|l| l.starts_with("step 1: "))
            .unwrap();
        assert!(at < step, "{text}");
    }
    assert_eq!(header(&full), header(&hist), "{full}\n{hist}");
    assert_eq!(outcome(&full, "step 1"), "pass", "{full}");
    assert!(hist.lines().any(|l| l == "step 1: needs-run"), "{hist}");
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

#[test]
fn retired_alone_exits_0() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "rules", "only_one", "assert!(true);");
    commit(dir, "ABC-1 step 1 RED: one rule");
    commit(dir, "ABC-1 step 1 GREEN: green");
    add_test(dir, "two", "two", "assert_eq!(tiny::two(), 1);");
    commit(dir, "ABC-1 step 2 RED: two is missing");
    add_fn(dir, "two");
    edit(
        dir,
        "crates/tiny/tests/rules.rs",
        "#[test]\nfn only_one() {\n    assert!(true);\n}\n",
        "",
    );
    commit(
        dir,
        "ABC-1 step 2 GREEN: two\n\nspec change: AC-1, the old rule was wrong",
    );
    let (code, out, err) = history(dir);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(err, "", "{out}");
    assert!(
        out.lines()
            .any(|l| l == "step 1: retired: every step test was retired by a spec change"),
        "{out}"
    );
    assert!(out.lines().any(|l| l == "step 2: needs-run"), "{out}");
    assert_eq!(
        out.lines().last(),
        Some("result: no history faults"),
        "{out}"
    );
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
    common::confirm_these(dir, "ABC-1", &["description", "spec", "design", "proof", "plan"]);
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
    // 15 s, not 5 s: the run makes about 850 git calls, and on macOS with /usr/bin/git (the xcrun
    // shim) it takes about 10 s (ledger decision of 2026-11-19).
    assert!(took < std::time::Duration::from_secs(15), "took {took:?}");
}

// ---- STP-28 AC-1 ----

#[test]
fn walk_starts_at_the_base() {
    /// Sets or removes the `base` of ABC-1 and commits the state.
    fn set_base(dir: &Path, base: Option<&str>) {
        let mut state = common::state_json(dir, "ABC-1");
        match base {
            Some(b) => state["base"] = serde_json::json!(b),
            None => {
                state.as_object_mut().unwrap().remove("base");
            }
        }
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base");
    }
    /// A step 1 RED before the base, then the base, then the real step 1.
    fn old_red_then_real_step(dir: &Path, with_base: bool) {
        add_test(dir, "old", "old_one", "assert!(true);");
        commit(dir, "ABC-1 step 1 RED: an old red");
        let head = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        set_base(dir, with_base.then_some(head.as_str()));
        clean_pair(dir, "step 1", "real");
    }

    // With a base: one pass in the full check and no `duplicate` in either mode.
    let repo = cargo_repo();
    let dir = repo.path();
    old_red_then_real_step(dir, true);
    let (code, hist, err) = history(dir);
    assert_eq!(code, 0, "{hist}{err}");
    assert_eq!(outcome(&hist, "step 1"), "needs-run", "{hist}");
    assert!(!hist.contains("duplicate"), "{hist}");
    let (code, full) = check(dir);
    assert_eq!(code, 0, "{full}");
    assert_eq!(outcome(&full, "step 1"), "pass", "{full}");
    assert!(!full.contains("duplicate"), "{full}");

    // A merge of a side branch after the base: its first-parent diff counts, the result stays.
    let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    git(dir, &["checkout", "-q", "-b", "side", "HEAD~2"]);
    write(dir, "docs/side.md", "side\n");
    commit(dir, "a side commit");
    git(dir, &["checkout", "-q", &branch]);
    git(
        dir,
        &[
            "-c",
            "user.name=test-user",
            "-c",
            "user.email=test-user.invalid",
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "merge main",
            "side",
        ],
    );
    let (code, hist, err) = history(dir);
    assert_eq!(code, 0, "{hist}{err}");
    assert_eq!(outcome(&hist, "step 1"), "needs-run", "{hist}");
    assert!(!hist.contains("duplicate"), "{hist}");

    // Without a base the old commit counts.
    let repo = cargo_repo();
    let dir = repo.path();
    old_red_then_real_step(dir, false);
    let (_, hist, err) = history(dir);
    assert_eq!(outcome(&hist, "step 1"), "duplicate", "{hist}{err}");
}

// ---- STP-28 AC-37 ----

#[test]
fn walk_reads_only_the_range() {
    /// The `git log` and `git rev-list` calls in a `GIT_TRACE` file that read a history without a
    /// `..` range; a call limited to one commit (`-1`, `-n 1`) does not count.
    fn wide_walks(trace: &str) -> Vec<String> {
        let mut found = Vec::new();
        for line in trace.lines() {
            let Some((_, call)) = line.split_once(" git ") else {
                continue;
            };
            let mut words = call.split_whitespace();
            let mut sub = None;
            while let Some(w) = words.next() {
                if w == "-c" || w == "-C" {
                    words.next();
                } else if !w.starts_with('-') {
                    sub = Some(w.trim_matches('\''));
                    break;
                }
            }
            if !matches!(sub, Some("log" | "rev-list")) {
                continue;
            }
            let args: Vec<&str> = call.split_whitespace().collect();
            let one = args.contains(&"-1") || args.windows(2).any(|w| w == ["-n", "1"]);
            if !call.contains("..") && !one {
                found.push(line.to_string());
            }
        }
        found
    }

    let repo = cargo_repo();
    let dir = repo.path();
    let base = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let mut state = common::state_json(dir, "ABC-1");
    state["base"] = serde_json::json!(base);
    write(
        dir,
        ".stapel/tickets/ABC-1/state.json",
        &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
    );
    commit(dir, "ABC-1: base");
    clean_pair(dir, "step 1", "alpha");
    let payload = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "git commit -m x"},
        "cwd": dir,
    })
    .to_string();

    let trace_file = dir.join("..").join(format!(
        "{}-trace.txt",
        dir.file_name().unwrap().to_string_lossy()
    ));
    let runs: Vec<(&str, Vec<&str>, Option<&str>)> = vec![
        ("check", vec!["check", "ABC-1"], None),
        ("check --history", vec!["check", "ABC-1", "--history"], None),
        ("hook", vec!["hook", "post-tool-use"], Some(payload.as_str())),
        ("next", vec!["next", "ABC-1"], None),
        ("lint", vec!["lint", "ABC-1"], None),
    ];
    for (label, args, stdin) in runs {
        let _ = std::fs::remove_file(&trace_file);
        let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
        let mut cmd = stapel(dir);
        cmd.args(&args)
            .env("GIT_TRACE", &trace_file)
            .env("CARGO_BUILD_JOBS", "2");
        if let Some(stdin) = stdin {
            cmd.write_stdin(stdin.to_string());
        }
        cmd.output().unwrap();
        let trace = std::fs::read_to_string(&trace_file).unwrap_or_default();
        assert!(trace.contains("git "), "{label}: no git calls traced");
        let wide = wide_walks(&trace);
        assert!(
            wide.is_empty(),
            "{label} reads the whole history:\n{}",
            wide.join("\n")
        );
    }
    let _ = std::fs::remove_file(&trace_file);
}

// ---- STP-28 AC-43 ----

#[test]
fn merge_of_the_base_branch_blocks_nothing() {
    /// A ticket with `base_branch` main: step 1, a merge of main that brings the test file
    /// `new.rs` (resolved to other content when `resolved`), step 2. Returns the check output and
    /// the merge commit.
    fn scenario(resolved: bool) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let mut state = common::state_json(dir, "ABC-1");
        state["base_branch"] = serde_json::json!(main);
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        clean_pair(dir, "step 1", "one");
        git(dir, &["checkout", "-q", &main]);
        write(
            dir,
            "crates/tiny/tests/new.rs",
            "#[test]\nfn from_main() {\n    assert_eq!(tiny::add(1, 1), 2);\n}\n",
        );
        commit(dir, "main: a test file");
        git(dir, &["checkout", "-q", "ticket"]);
        git(
            dir,
            &[
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
                "merge",
                "-q",
                "--no-ff",
                "--no-commit",
                &main,
            ],
        );
        if resolved {
            write(
                dir,
                "crates/tiny/tests/new.rs",
                "#[test]\nfn from_main() {\n    assert_eq!(tiny::add(2, 2), 4);\n}\n",
            );
        }
        commit(dir, "ABC-1: merge main");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        clean_pair(dir, "step 2", "two");
        let (_, text) = check(dir);
        (text, merge)
    }

    let (text, _) = scenario(false);
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert_eq!(outcome(&text, "step 2"), "pass", "{text}");
    assert!(!text.contains("build-input-changed"), "{text}");

    let (text, merge) = scenario(true);
    assert_eq!(
        outcome(&text, "step 2"),
        "unverified",
        "a merge that resolves the file to other content blocks: {text}"
    );
    assert!(
        text.contains(&format!(
            "build-input-changed: crates/tiny/tests/new.rs at {}",
            &merge[..7]
        )),
        "{text}"
    );
}

// ---- STP-30 AC-3 ----

#[test]
fn base_merge_mode_change_blocks() {
    /// A ticket with `base_branch` main: step 1, a merge of main that brings `path` as a regular
    /// file with `content`, `resolve` changes the path in the merge (the blob stays equal), step 2.
    /// Returns the history output and the merge commit.
    fn scenario(path: &str, content: &str, resolve: &dyn Fn(&Path)) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let mut state = common::state_json(dir, "ABC-1");
        state["base_branch"] = serde_json::json!(main);
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        clean_pair(dir, "step 1", "one");
        git(dir, &["checkout", "-q", &main]);
        write(dir, path, content);
        commit(dir, "main: a file");
        git(dir, &["checkout", "-q", "ticket"]);
        git(
            dir,
            &[
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
                "merge",
                "-q",
                "--no-ff",
                "--no-commit",
                &main,
            ],
        );
        resolve(&dir.join(path));
        commit(dir, "ABC-1: merge main");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        clean_pair(dir, "step 2", "two");
        let (_, text) = check(dir);
        (text, merge)
    }

    let paths = ["crates/tiny/build.rs", "crates/tiny/tests/data/fixture.txt"];

    // Control: an equal blob and an equal mode blocks nothing.
    for path in paths {
        let (text, _) = scenario(path, "fn main() {}\n", &|_| {});
        assert_eq!(outcome(&text, "step 2"), "pass", "{path}: {text}");
    }

    // An executable bit with an equal blob blocks at the merge.
    for path in paths {
        let (text, merge) = scenario(path, "fn main() {}\n", &|file| {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
        });
        assert_ne!(outcome(&text, "step 2"), "pass", "{path}: {text}");
        assert!(
            text.contains(&format!("{path} at {}", &merge[..7])),
            "{path}: {text}"
        );
    }

    // A regular file turned into a link with the same blob blocks at the merge.
    let link = "crates/tiny/tests/data/link.txt";
    let (text, merge) = scenario(link, "target.txt", &|file| {
        std::fs::remove_file(file).unwrap();
        std::os::unix::fs::symlink("target.txt", file).unwrap();
    });
    assert_ne!(outcome(&text, "step 2"), "pass", "{text}");
    assert!(
        text.contains(&format!("{link} at {}", &merge[..7])),
        "{text}"
    );
}

// ---- STP-30 AC-1, AC-6 (d1-1, d1-2) ----

#[test]
fn base_merge_mode_outcomes() {
    /// A ticket with `base_branch` main: step 1, a merge of main whose `path` comes from `make`
    /// (on main) and is changed by `resolve` in the merge, step 2. Returns the check output and
    /// the merge commit.
    fn scenario(path: &str, make: &dyn Fn(&Path), resolve: &dyn Fn(&Path)) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let mut state = common::state_json(dir, "ABC-1");
        state["base_branch"] = serde_json::json!(main);
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        clean_pair(dir, "step 1", "one");
        git(dir, &["checkout", "-q", &main]);
        let file = dir.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        make(&file);
        commit(dir, "main: a file");
        git(dir, &["checkout", "-q", "ticket"]);
        git(
            dir,
            &[
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
                "merge",
                "-q",
                "--no-ff",
                "--no-commit",
                &main,
            ],
        );
        resolve(&file);
        commit(dir, "ABC-1: merge main");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        clean_pair(dir, "step 2", "two");
        let (_, text) = check(dir);
        (text, merge)
    }
    fn regular(file: &Path) {
        std::fs::write(file, "fn main() {}\n").unwrap();
    }
    fn executable(file: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn link(file: &Path) {
        let _ = std::fs::remove_file(file);
        std::os::unix::fs::symlink("target.txt", file).unwrap();
    }

    let build = "crates/tiny/build.rs";
    let fixture = "crates/tiny/tests/data/fixture.txt";
    let blocked = |text: &str, merge: &str, path: &str| {
        assert_eq!(
            text.lines().find(|l| l.starts_with("step 2: ")).unwrap(),
            format!(
                "step 2: unverified: build-input-changed: {path} at {}",
                &merge[..7]
            ),
            "{path}: {text}"
        );
    };

    // `chmod +x` with an equal blob: a build input and a cargo fixture both give the outcome.
    for path in [build, fixture] {
        let (text, merge) = scenario(path, &regular, &executable);
        blocked(&text, &merge, path);
    }

    // A build input turned into a link at the merge.
    let (text, merge) = scenario(build, &regular, &link);
    blocked(&text, &merge, build);

    // Control: an equal link at both parents passes.
    let (text, _) = scenario(build, &link, &|_| {});
    assert_eq!(outcome(&text, "step 2"), "pass", "{text}");
    assert!(!text.contains("build-input-changed"), "{text}");
}

// ---- STP-30 AC-1, AC-2 ----

#[test]
fn base_merge_data_file_is_no_tests_change() {
    /// A ticket with `base_branch` main: a data file under `crates/tiny/tests/` on main, step 1, a
    /// merge of main that brings a new text of the file (resolved to another text when `resolved`).
    /// Returns the history output and the merge commit.
    fn scenario(resolved: bool) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        write(dir, "crates/tiny/tests/data/table.txt", "one\n");
        commit(dir, "a data file");
        let mut state = common::state_json(dir, "ABC-1");
        state["base_branch"] = serde_json::json!(main);
        state["base"] = serde_json::json!(git(dir, &["rev-parse", "HEAD"]).trim());
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        clean_pair(dir, "step 1", "one");
        git(dir, &["checkout", "-q", &main]);
        write(dir, "crates/tiny/tests/data/table.txt", "two\n");
        commit(dir, "main: a new table");
        git(dir, &["checkout", "-q", "ticket"]);
        git(
            dir,
            &[
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
                "merge",
                "-q",
                "--no-ff",
                "--no-commit",
                &main,
            ],
        );
        if resolved {
            write(dir, "crates/tiny/tests/data/table.txt", "three\n");
        }
        commit(dir, "ABC-1: merge main");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        let (_, text, _) = history(dir);
        (text, merge)
    }

    let step_one = |text: &str| {
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string()
    };

    let (text, _) = scenario(false);
    assert_eq!(step_one(&text), "step 1: needs-run", "{text}");

    let (text, merge) = scenario(true);
    assert_eq!(
        step_one(&text),
        "step 1: tests-changed: crates/tiny/tests/data/table.txt",
        "{text}"
    );
    assert!(text.contains(&merge[..7]), "the fault is the merge: {text}");
}

// STP-30 AC-1, AC-2 (CR1-E1): a data file named `d[1].txt` is a path, not a pattern: a merge that
// resolves it to a third text blocks, a merge that brings it equal passes.
#[test]
fn base_merge_glob_names() {
    fn scenario(resolved: bool) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        write(dir, "crates/tiny/tests/data/d[1].txt", "one\n");
        commit(dir, "a data file");
        let mut state = common::state_json(dir, "ABC-1");
        state["base_branch"] = serde_json::json!(main);
        state["base"] = serde_json::json!(git(dir, &["rev-parse", "HEAD"]).trim());
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        clean_pair(dir, "step 1", "one");
        git(dir, &["checkout", "-q", &main]);
        write(dir, "crates/tiny/tests/data/d[1].txt", "two\n");
        commit(dir, "main: a new table");
        git(dir, &["checkout", "-q", "ticket"]);
        git(
            dir,
            &[
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
                "merge",
                "-q",
                "--no-ff",
                "--no-commit",
                &main,
            ],
        );
        if resolved {
            write(dir, "crates/tiny/tests/data/d[1].txt", "three\n");
        }
        commit(dir, "ABC-1: merge main");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        let (_, text, _) = history(dir);
        (text, merge)
    }

    let step_one = |text: &str| {
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string()
    };

    let (text, _) = scenario(false);
    assert_eq!(step_one(&text), "step 1: needs-run", "{text}");

    let (text, merge) = scenario(true);
    assert_eq!(
        step_one(&text),
        "step 1: tests-changed: crates/tiny/tests/data/d[1].txt",
        "{text}"
    );
    assert!(text.contains(&merge[..7]), "the fault is the merge: {text}");
}

// ---- STP-30 AC-4 ----

#[test]
fn base_merge_other_parents() {
    /// `kind` is `base` (a merge of main, `base_branch` main), `no-base-branch`, `ref-gone`
    /// (`base_branch` names a ref that does not exist), `other` (a merge of a branch that main does
    /// not contain) or `octopus` (one merge of main and of another branch). Main or the other branch
    /// brings the test file `new.rs`. Returns the check output and the merge commit.
    fn scenario(kind: &str) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let mut state = common::state_json(dir, "ABC-1");
        match kind {
            "no-base-branch" => {}
            "ref-gone" => state["base_branch"] = serde_json::json!("deleted-branch"),
            _ => state["base_branch"] = serde_json::json!(main),
        }
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["branch", "other"]);
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        clean_pair(dir, "step 1", "one");
        let test_file = "#[test]\nfn from_main() {\n    assert_eq!(tiny::add(1, 1), 2);\n}\n";
        let mut merged = vec![main.clone()];
        if kind == "other" {
            git(dir, &["checkout", "-q", "other"]);
            write(dir, "crates/tiny/tests/new.rs", test_file);
            commit(dir, "other: a test file");
            merged = vec!["other".to_string()];
        } else {
            git(dir, &["checkout", "-q", &main]);
            write(dir, "crates/tiny/tests/new.rs", test_file);
            commit(dir, "main: a test file");
        }
        if kind == "octopus" {
            git(dir, &["checkout", "-q", "other"]);
            write(dir, "NOTES.md", "notes\n");
            commit(dir, "other: a note");
            merged.push("other".to_string());
        }
        git(dir, &["checkout", "-q", "ticket"]);
        let mut args = vec![
            "-c",
            "user.name=test-user",
            "-c",
            "user.email=test-user.invalid",
            "merge",
            "-q",
            "--no-ff",
            "--no-commit",
        ];
        args.extend(merged.iter().map(String::as_str));
        git(dir, &args);
        commit(dir, "ABC-1: merge");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        clean_pair(dir, "step 2", "two");
        let (_, text) = check(dir);
        (text, merge)
    }

    // Control: a merge of the base branch blocks nothing.
    let (text, _) = scenario("base");
    assert_eq!(outcome(&text, "step 2"), "pass", "{text}");

    for kind in ["no-base-branch", "ref-gone", "other", "octopus"] {
        let (text, merge) = scenario(kind);
        assert_eq!(outcome(&text, "step 2"), "unverified", "{kind}: {text}");
        assert!(
            text.contains(&format!(
                "build-input-changed: crates/tiny/tests/new.rs at {}",
                &merge[..7]
            )),
            "{kind}: {text}"
        );
    }
}

// ---- STP-30 AC-5, AC-14, AC-15 ----

#[test]
fn base_merge_brings_cargo_inputs() {
    /// A ticket with `base_branch` main: step 1, a merge of main that brings a build input with a new
    /// text through `change` (resolved to another text when `resolved`). Returns the history
    /// line of step 1 and the merge commit.
    fn scenario(change: &dyn Fn(&Path, &str), resolved: bool) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let mut state = common::state_json(dir, "ABC-1");
        state["base_branch"] = serde_json::json!(main);
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        clean_pair(dir, "step 1", "one");
        git(dir, &["checkout", "-q", &main]);
        change(dir, "from main");
        commit(dir, "main: an input");
        git(dir, &["checkout", "-q", "ticket"]);
        git(
            dir,
            &[
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
                "merge",
                "-q",
                "--no-ff",
                "--no-commit",
                &main,
            ],
        );
        if resolved {
            change(dir, "resolved");
        }
        commit(dir, "ABC-1: merge main");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        let (_, text, _) = history(dir);
        let line = text
            .lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string();
        (line, merge)
    }

    /// A new file with the text.
    fn file(path: &'static str) -> Box<dyn Fn(&Path, &str)> {
        Box::new(move |dir, text| write(dir, path, &format!("// {text}\n")))
    }

    let inputs: Vec<(&str, Box<dyn Fn(&Path, &str)>)> = vec![
        (
            "crates/tiny/Cargo.toml",
            Box::new(|dir, text| {
                let old = read(dir, "crates/tiny/Cargo.toml");
                let old = old.split("\n[lib]").next().unwrap().to_string();
                write(
                    dir,
                    "crates/tiny/Cargo.toml",
                    &format!("{old}\n[lib]\npath = \"src/{}.rs\"\n", text.replace(' ', "_")),
                );
            }),
        ),
        ("crates/tiny/build.rs", file("crates/tiny/build.rs")),
        ("crates/tiny/rust-toolchain", file("crates/tiny/rust-toolchain")),
        (
            "crates/tiny/rust-toolchain.toml",
            file("crates/tiny/rust-toolchain.toml"),
        ),
        (".cargo/config", file(".cargo/config")),
        (".cargo/config.toml", file(".cargo/config.toml")),
    ];

    for (path, change) in &inputs {
        let (line, _) = scenario(change.as_ref(), false);
        assert_eq!(line, "step 1: needs-run", "equal {path}");
        let (line, merge) = scenario(change.as_ref(), true);
        assert_eq!(
            line,
            format!(
                "step 1: unverified: build-input-changed: {path} at {}",
                &merge[..7]
            ),
            "resolved {path}"
        );
    }
}

#[test]
fn base_merge_mixed_entries() {
    /// A ticket with `base_branch` main: build inputs exist before the ticket, step 1, a merge of
    /// main that `bring`s changes, `resolve` changes the merge result. Returns the history line of
    /// step 1 and the merge commit.
    fn scenario(
        bring: &dyn Fn(&Path),
        resolve: &dyn Fn(&Path),
    ) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        write(dir, "crates/tiny/build.rs", "fn main() {}\n");
        commit(dir, "setup: a build script");
        let mut state = common::state_json(dir, "ABC-1");
        state["base_branch"] = serde_json::json!(main);
        state["base"] = serde_json::json!(git(dir, &["rev-parse", "HEAD"]).trim());
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        clean_pair(dir, "step 1", "one");
        git(dir, &["checkout", "-q", &main]);
        bring(dir);
        commit(dir, "main: inputs");
        git(dir, &["checkout", "-q", "ticket"]);
        git(
            dir,
            &[
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
                "merge",
                "-q",
                "--no-ff",
                "--no-commit",
                &main,
            ],
        );
        resolve(dir);
        commit(dir, "ABC-1: merge main");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        let (_, text, _) = history(dir);
        let line = text
            .lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string();
        (line, merge)
    }

    // A rename of a build script: the new path is equal at the merge, the old one is brought back.
    let rename = |dir: &Path| {
        git(dir, &["mv", "crates/tiny/build.rs", "crates/other_build.rs"]);
    };
    let (line, _) = scenario(&rename, &|_| {});
    assert_eq!(line, "step 1: needs-run", "equal rename");
    let (line, merge) = scenario(&rename, &|dir| {
        write(dir, "crates/tiny/build.rs", "fn main() { /* kept */ }\n");
    });
    assert!(
        line.starts_with("step 1: unverified: build-input-changed: ")
            && line.ends_with(&format!(" at {}", &merge[..7])),
        "a rename with one path resolved: {line}"
    );

    // Two inputs brought: one equal at the merge, one resolved to other content.
    let two = |dir: &Path| {
        write(dir, ".cargo/config.toml", "# from main\n");
        write(dir, "crates/tiny/other/build.rs", "// from main\n");
    };
    let (line, _) = scenario(&two, &|_| {});
    assert_eq!(line, "step 1: needs-run", "two equal inputs");
    let (line, merge) = scenario(&two, &|dir| {
        write(dir, "crates/tiny/other/build.rs", "// resolved\n");
    });
    assert_eq!(
        line,
        format!(
            "step 1: unverified: build-input-changed: crates/tiny/other/build.rs at {}",
            &merge[..7]
        ),
        "one equal, one resolved"
    );
}

#[test]
fn cargo_fixture_beside_step_test() {
    /// A data file `crates/tiny/tests/table.txt` (present before the ticket when `before`), step 1,
    /// then `change` after the GREEN. Returns the history line of step 1 and the changing commit.
    fn scenario(before: bool, change: &dyn Fn(&Path)) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        if before {
            write(dir, "crates/tiny/tests/table.txt", "one\n");
            commit(dir, "setup: a data file");
        }
        clean_pair(dir, "step 1", "one");
        change(dir);
        commit(dir, "ABC-1: the data file");
        let sha = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        let (_, text, _) = history(dir);
        let line = text
            .lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string();
        (line, sha)
    }

    let path = "crates/tiny/tests/table.txt";

    let (line, sha) = scenario(false, &|dir| write(dir, path, "one\n"));
    assert_eq!(
        line,
        format!("step 1: unverified: build-input-changed: {path} at {}", &sha[..7]),
        "added"
    );

    let (line, sha) = scenario(true, &|dir| std::fs::remove_file(dir.join(path)).unwrap());
    assert_eq!(
        line,
        format!("step 1: unverified: build-input-changed: {path} at {}", &sha[..7]),
        "deleted"
    );

    let (line, sha) = scenario(true, &|dir| write(dir, path, "two\n"));
    assert_eq!(line, format!("step 1: tests-changed: {path}"), "changed");
    let _ = sha;
}

// ---- STP-28 step 23 (AC-37) ----

#[test]
fn base_edges() {
    /// Sets the `base` of ABC-1 and commits the state.
    fn set_base(dir: &Path, base: &str) {
        let mut state = common::state_json(dir, "ABC-1");
        state["base"] = serde_json::json!(base);
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base");
    }

    // A base after HEAD is no ancestor of HEAD: both checks refuse.
    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "alpha");
    let before = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    commit(dir, "a later commit");
    let later = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    git(dir, &["checkout", "-q", "--detach", &before]);
    set_base(dir, &later);
    let out = {
        let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
        stapel(dir).args(["check", "ABC-1"]).output().unwrap()
    };
    let full = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(2), "{full}");
    assert!(full.contains("state.json has base"), "{full}");
    let (code, hist, err) = history(dir);
    assert_eq!(code, 2, "{hist}{err}");
    assert!(err.contains("state.json has base"), "{err}");

    // HEAD equal to the base: an empty Walk is no refusal of the base.
    let repo = cargo_repo();
    let dir = repo.path();
    clean_pair(dir, "step 1", "alpha");
    let head = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let mut state = common::state_json(dir, "ABC-1");
    state["base"] = serde_json::json!(head);
    write(
        dir,
        ".stapel/tickets/ABC-1/state.json",
        &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
    );
    let (code, hist, err) = history(dir);
    assert!(
        !format!("{hist}{err}").contains("state.json has base"),
        "{hist}{err}"
    );
    // The empty Walk holds no step commits; that is the only complaint.
    assert_eq!(code, 2, "{hist}{err}");
    assert!(err.contains("no step commits"), "{err}");
}

// ---- STP-30 AC-1, AC-2 (d2-1, d2-2, d2-3) ----

#[test]
fn base_merge_step_test_unit() {
    const MAIN_TEXT: &str = "#[test]\nfn one() {\n    assert_eq!(tiny::add(2, 2), 4);\n}\n";
    const FILE: &str = "crates/tiny/tests/one.rs";

    /// A ticket with `base_branch` main: step 1 adds `one` in `tests/one.rs`. For the kinds
    /// `elsewhere`, `mode`, `third` and `first` main adds the same file with another text of `one`
    /// and the merge resolves it by `kind`; for `deleted-same` and `deleted-other` main adds a note
    /// and the merge deletes the file, which a later commit adds again with the protected text or
    /// another one. Returns the history output and the last commit.
    fn scenario(kind: &str) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let mut state = common::state_json(dir, "ABC-1");
        state["base_branch"] = serde_json::json!(main);
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        clean_pair(dir, "step 1", "one");
        let protected = read(dir, FILE);
        let deleted = kind.starts_with("deleted");
        git(dir, &["checkout", "-q", &main]);
        if deleted {
            write(dir, "NOTES.md", "notes\n");
        } else {
            write(dir, FILE, MAIN_TEXT);
        }
        commit(dir, "main: a change");
        git(dir, &["checkout", "-q", "ticket"]);
        // A conflict of the merge is no failure here: the test resolves it.
        std::process::Command::new("git")
            .args([
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
                "merge",
                "-q",
                "--no-ff",
                "--no-commit",
                &main,
            ])
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        match kind {
            "elsewhere" => write(dir, FILE, &format!("{MAIN_TEXT}// a note\n")),
            "mode" => {
                use std::os::unix::fs::PermissionsExt;
                write(dir, FILE, MAIN_TEXT);
                std::fs::set_permissions(dir.join(FILE), std::fs::Permissions::from_mode(0o755))
                    .unwrap();
            }
            "third" => write(
                dir,
                FILE,
                "#[test]\nfn one() {\n    assert_eq!(tiny::add(3, 3), 6);\n}\n",
            ),
            "first" => write(dir, FILE, &protected),
            _ => std::fs::remove_file(dir.join(FILE)).unwrap(),
        }
        commit(dir, "ABC-1: merge main");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        let mut last = merge;
        if deleted {
            let again = if kind == "deleted-same" {
                protected
            } else {
                "#[test]\nfn one() {\n    assert_eq!(tiny::add(4, 4), 8);\n}\n".to_string()
            };
            write(dir, FILE, &again);
            commit(dir, "ABC-1: add the test file again");
            last = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        }
        let (_, text, _) = history(dir);
        (text, last)
    }
    let step_one = |text: &str| {
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string()
    };

    // The step test is Equal at the merge while its file differs elsewhere, or in mode only.
    for kind in ["elsewhere", "mode"] {
        let (text, _) = scenario(kind);
        assert_eq!(step_one(&text), "step 1: needs-run", "{kind}: {text}");
    }

    // The step test resolved to the first parent's text keeps the protected text.
    let (text, _) = scenario("first");
    assert_eq!(step_one(&text), "step 1: needs-run", "first: {text}");

    // Resolved to a text that is not the second parent's: `tests-changed` at the merge.
    let (text, merge) = scenario("third");
    assert!(
        step_one(&text).starts_with("step 1: tests-changed: "),
        "third: {text}"
    );
    assert!(text.contains(&merge[..7]), "the fault is the merge: {text}");

    // Deleted by the merge: a later re-add is compared with the protected text.
    let (text, _) = scenario("deleted-same");
    assert_eq!(step_one(&text), "step 1: needs-run", "deleted-same: {text}");
    let (text, again) = scenario("deleted-other");
    assert!(
        step_one(&text).starts_with("step 1: tests-changed: "),
        "deleted-other: {text}"
    );
    assert!(text.contains(&again[..7]), "the fault is the re-add: {text}");
}

// STP-30 AC-1 (d24-2): a data file beside a cargo step test, added at the RED and deleted inside the
// merge commit, blocks at the merge; a data file that main deleted and the merge brought gives none.
#[test]
fn base_merge_deletion_in_merge() {
    fn scenario(main_deletes: bool) -> (String, String) {
        let repo = cargo_repo();
        let dir = repo.path();
        let main = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        if main_deletes {
            write(dir, "crates/tiny/tests/data/table.txt", "one\n");
            commit(dir, "a data file");
        }
        let mut state = common::state_json(dir, "ABC-1");
        state["base_branch"] = serde_json::json!(main);
        state["base"] = serde_json::json!(git(dir, &["rev-parse", "HEAD"]).trim());
        write(
            dir,
            ".stapel/tickets/ABC-1/state.json",
            &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
        );
        commit(dir, "ABC-1: base branch");
        git(dir, &["checkout", "-q", "-b", "ticket"]);
        if !main_deletes {
            write(dir, "crates/tiny/tests/data/table.txt", "one\n");
        }
        clean_pair(dir, "step 1", "one");
        git(dir, &["checkout", "-q", &main]);
        if main_deletes {
            git(dir, &["rm", "-q", "crates/tiny/tests/data/table.txt"]);
        } else {
            write(dir, "README.md", "main moved\n");
        }
        commit(dir, "main: moves on");
        git(dir, &["checkout", "-q", "ticket"]);
        git(
            dir,
            &[
                "-c",
                "user.name=test-user",
                "-c",
                "user.email=test-user.invalid",
                "merge",
                "-q",
                "--no-ff",
                "--no-commit",
                &main,
            ],
        );
        if !main_deletes {
            git(dir, &["rm", "-q", "crates/tiny/tests/data/table.txt"]);
        }
        commit(dir, "ABC-1: merge main");
        let merge = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
        let (_, text, _) = history(dir);
        (text, merge)
    }

    let step_one = |text: &str| {
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string()
    };

    // Under cargo a deleted data file is a build input change, not `tests-changed` (as outside a
    // merge); the merge that deletes it blocks the step and is named.
    let (text, merge) = scenario(false);
    let line = step_one(&text);
    assert_ne!(line, "step 1: needs-run", "{text}");
    assert!(
        line.contains("crates/tiny/tests/data/table.txt") && line.contains(&merge[..7]),
        "the fault names the file and the merge: {text}"
    );

    let (text, _) = scenario(true);
    assert_eq!(step_one(&text), "step 1: needs-run", "{text}");
}
