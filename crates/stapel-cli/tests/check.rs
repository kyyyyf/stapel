//! STP-4: `stapel check`, the RED to GREEN check of a ticket's steps.

mod common;

use common::{read, stapel};
use predicates::str::contains;
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

fn list(dir: &Path) -> String {
    let out = stapel(dir)
        .args(["check", "ABC-1", "--list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(out).unwrap()
}

/// The lines of the `--list` output under one label.
fn step_block(text: &str, label: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if !line.starts_with(' ') {
            inside = line == label;
            continue;
        }
        if inside {
            out.push(line.trim().to_string());
        }
    }
    out
}

// STP-4 AC-1: steps are paired by label, in history order; merged side commits are not read.
#[test]
fn list_pairs_red_and_green_by_label() {
    let repo = cargo_repo();
    let dir = repo.path();
    let basic = read(dir, "crates/tiny/tests/basic.rs");
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        &format!("{basic}\n#[test]\nfn first() {{\n    assert_eq!(tiny::add(2, 2), 4);\n}}\n"),
    );
    commit(dir, "ABC-1 step 1 RED: first test");
    commit(dir, "ABC-1 step 1 GREEN: first code");
    let basic = read(dir, "crates/tiny/tests/basic.rs");
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        &format!("{basic}\n#[test]\nfn second() {{\n    assert_eq!(tiny::add(3, 3), 6);\n}}\n"),
    );
    commit(dir, "ABC-1 code review round 1 RED: second test");
    commit(dir, "ABC-1 code review round 1 GREEN: second code");

    let main = git(dir, &["branch", "--show-current"]).trim().to_string();
    git(dir, &["checkout", "-q", "-b", "side"]);
    write(dir, "crates/tiny/tests/side.rs", "#[test]\nfn side() {}\n");
    commit(dir, "ABC-1 step 9 RED: on a side branch");
    git(dir, &["checkout", "-q", &main]);
    git(
        dir,
        &[
            "-c",
            "user.email=test-user.invalid",
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "merge side",
            "side",
        ],
    );

    let text = list(dir);
    let labels: Vec<&str> = text.lines().filter(|l| !l.starts_with(' ')).collect();
    assert_eq!(
        labels,
        ["ticket: ABC-1", "step 1", "code review round 1"],
        "{text}"
    );
    let first = step_block(&text, "step 1");
    assert!(
        first
            .iter()
            .any(|l| l.starts_with("red: ") && l.ends_with("ABC-1 step 1 RED: first test")),
        "{text}"
    );
    assert!(
        first
            .iter()
            .any(|l| l.starts_with("green: ") && l.ends_with("ABC-1 step 1 GREEN: first code")),
        "{text}"
    );
    assert!(first.contains(&"tiny/basic::first".to_string()), "{text}");
    assert!(
        step_block(&text, "code review round 1").contains(&"tiny/basic::second".to_string()),
        "{text}"
    );
    assert!(!text.contains("step 9"), "{text}");
}

// STP-4 AC-1: only `<KEY> <label> RED: ` and `… GREEN: ` subjects of this key are step commits.
#[test]
fn list_ignores_other_keys_and_prose() {
    let repo = cargo_repo();
    let dir = repo.path();
    let basic = read(dir, "crates/tiny/tests/basic.rs");
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        &format!("{basic}\n#[test]\nfn real() {{}}\n"),
    );
    commit(dir, "ABC-1 step 1 RED: real");
    for subject in [
        "ABC-10 step 1 RED: another ticket",
        "ABC-1: notes about step 2 RED: prose",
        "ABC-1 RED: no label",
        "ABC-1  GREEN: blank label",
        "abc-1 step 3 RED: lower case key",
        "ABC-1 step 1 green: lower case marker",
    ] {
        commit(dir, subject);
    }
    commit(dir, "ABC-1 step 1 GREEN: fix RED: notes");

    let text = list(dir);
    let labels: Vec<&str> = text.lines().filter(|l| !l.starts_with(' ')).collect();
    assert_eq!(labels, ["ticket: ABC-1", "step 1"], "{text}");
    let block = step_block(&text, "step 1");
    assert_eq!(
        block.iter().filter(|l| l.starts_with("red: ")).count(),
        1,
        "{text}"
    );
    let greens: Vec<&String> = block.iter().filter(|l| l.starts_with("green: ")).collect();
    assert_eq!(greens.len(), 1, "{text}");
    assert!(
        greens[0].ends_with("ABC-1 step 1 GREEN: fix RED: notes"),
        "{text}"
    );
}

// STP-4 AC-1: step tests are those whose text the RED adds or changes; comments, whitespace and
// helpers do not count, and literals are kept byte for byte.
#[test]
fn list_finds_added_and_changed_tests() {
    let repo = cargo_repo();
    let dir = repo.path();
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn starts() {\n    assert_eq!(tiny::add(1, 1), 2);\n}\n\n\
         #[test]\nfn same() {\n    assert_eq!(tiny::add(0, 0), 0);\n}\n\n\
         #[test]\nfn changed() {\n    assert_eq!(tiny::add(1, 2), 3);\n}\n\n\
         #[test]\nfn literal() {\n    let s = \"a } // b\";\n    assert_eq!(s.len(), 8);\n}\n",
    );
    commit(dir, "ABC-1: more tests before the ticket's steps");
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        "// A comment that changes nothing.\n#[test]\nfn starts() {\n    assert_eq!(tiny::add(1, 1), 2); // why\n}\n\n\
         #[test]\nfn same() {\n        assert_eq!( tiny::add(0, 0),\n 0 );\n}\n\n\
         #[test]\nfn changed() {\n    assert_eq!(tiny::add(1, 2), 4);\n}\n\n\
         #[test]\nfn literal() {\n    let s = \"a  } // b\";\n    assert_eq!(s.len(), 8);\n}\n\n\
         fn helper() -> u32 {\n    7\n}\n\n\
         #[test]\nfn added() {\n    assert_eq!(helper(), 7);\n}\n\n\
         mod nested {\n    #[test]\n    fn inner() {}\n}\n",
    );
    commit(dir, "ABC-1 step 1 RED: added and changed");
    let text = list(dir);
    let tests: Vec<String> = step_block(&text, "step 1")
        .into_iter()
        .filter(|l| l.starts_with("tiny/"))
        .collect();
    assert_eq!(
        tests,
        [
            "tiny/basic::added",
            "tiny/basic::changed",
            "tiny/basic::literal"
        ],
        "{text}"
    );
}

// STP-4 AC-1: a renamed test file keeps its tests; only a changed text makes a step test.
#[test]
fn list_keeps_renamed_and_reformatted_tests() {
    let repo = cargo_repo();
    let dir = repo.path();
    // A file large enough that git's rename detection (50 % similarity) sees the move.
    let mut big = read(dir, "crates/tiny/tests/basic.rs");
    for n in 0..12 {
        big.push_str(&format!(
            "\n#[test]\nfn kept_{n}() {{\n    assert_eq!(tiny::add({n}, 0), {n});\n}}\n"
        ));
    }
    write(dir, "crates/tiny/tests/basic.rs", &big);
    commit(dir, "ABC-1: tests before the ticket's steps");
    git(
        dir,
        &[
            "mv",
            "crates/tiny/tests/basic.rs",
            "crates/tiny/tests/moved.rs",
        ],
    );
    let moved = read(dir, "crates/tiny/tests/moved.rs");
    write(
        dir,
        "crates/tiny/tests/moved.rs",
        &format!("{moved}\n#[test]\nfn fresh() {{\n    assert_eq!(tiny::add(5, 5), 10);\n}}\n"),
    );
    commit(dir, "ABC-1 step 1 RED: rename and add");
    let text = list(dir);
    let tests: Vec<String> = step_block(&text, "step 1")
        .into_iter()
        .filter(|l| l.starts_with("tiny/"))
        .collect();
    assert_eq!(tests, ["tiny/moved::fresh"], "{text}");
}

/// Commit ids replaced by `<sha>`, so golden files hold no ids.
fn without_ids(text: &str) -> String {
    text.lines()
        .map(|line| format!("{}\n", line_without_ids(line)))
        .collect()
}

fn line_without_ids(text: &str) -> String {
    text.split(' ')
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
        .collect::<Vec<_>>()
        .join(" ")
}

// STP-4 AC-1: the layout of `--list`; it works without `[check]` and refuses with exit 2 for an
// unknown key or a ticket without step commits.
#[test]
fn list_matches_golden() {
    let repo = cargo_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["check", "ABC-1", "--list"])
        .assert()
        .code(2)
        .stderr(contains("no step commits"));
    stapel(dir)
        .args(["check", "ABC-7", "--list"])
        .assert()
        .code(2);

    let basic = read(dir, "crates/tiny/tests/basic.rs");
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        &format!("{basic}\n#[test]\nfn adds_two() {{\n    assert_eq!(tiny::add(2, 2), 4);\n}}\n"),
    );
    commit(dir, "ABC-1 step 1 RED: first test");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{LIB}\npub fn one() -> u32 {{\n    1\n}}\n"),
    );
    commit(dir, "ABC-1 step 1 GREEN: first code");
    let basic = read(dir, "crates/tiny/tests/basic.rs");
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        &format!("{basic}\n#[test]\nfn one() {{\n    assert_eq!(tiny::one(), 1);\n}}\n"),
    );
    commit(dir, "ABC-1 step 2 RED: second test");

    // Without [check] the list still works.
    let config = read(dir, ".stapel/stapel.toml");
    let without: String = config
        .lines()
        .filter(|l| {
            !(l.starts_with("[check]") || l.starts_with("runner") || l.starts_with("timeout_secs"))
        })
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(dir.join(".stapel/stapel.toml"), without).unwrap();

    let text = list(dir);
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/check_list.txt"),
    )
    .unwrap();
    assert_eq!(without_ids(&text), golden, "actual:\n{text}");
}

// ---- STP-4 AC-2: outcomes from history alone ----

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

#[test]
fn outcome_unpaired_and_duplicate() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "one", "assert!(true);");
    commit(dir, "ABC-1 step 1 RED: only a red");
    commit(dir, "ABC-1 step 2 GREEN: only a green");
    commit(dir, "ABC-1 step 3 GREEN: green first");
    add_test(dir, "basic", "three", "assert!(true);");
    commit(dir, "ABC-1 step 3 RED: then red");
    add_test(dir, "basic", "four", "assert!(true);");
    commit(dir, "ABC-1 step 4 RED: red");
    add_test(dir, "basic", "four_b", "assert!(true);");
    commit(dir, "ABC-1 step 4 RED: red again");
    commit(dir, "ABC-1 step 4 GREEN: green");
    add_test(dir, "basic", "five", "assert!(true);");
    commit(dir, "ABC-1 step 5 RED: red");
    commit(dir, "ABC-1 step 5 GREEN: green");
    commit(dir, "ABC-1 step 5 GREEN: green again");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    for (label, want) in [
        ("step 1", "unpaired"),
        ("step 2", "unpaired"),
        ("step 3", "unpaired"),
        ("step 4", "duplicate"),
        ("step 5", "duplicate"),
    ] {
        assert_eq!(outcome(&text, label), want, "{label}\n{text}");
    }
}

#[test]
fn outcome_no_tests_for_delete_only_red() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "doomed", "assert!(true);");
    write(dir, "crates/tiny/tests/golden/out.txt", "old\n");
    write(
        dir,
        "crates/tiny/tests/common/mod.rs",
        "pub fn helper() -> u32 {\n    1\n}\n",
    );
    commit(dir, "ABC-1: tests before the steps");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn doomed() {\n    assert!(true);\n}\n",
        "",
    );
    commit(dir, "ABC-1 step 1 RED: delete a test");
    commit(dir, "ABC-1 step 1 GREEN: code");
    write(dir, "crates/tiny/tests/golden/out.txt", "new\n");
    commit(dir, "ABC-1 step 2 RED: golden file only");
    commit(dir, "ABC-1 step 2 GREEN: code");
    edit(dir, "crates/tiny/tests/common/mod.rs", "1", "2");
    commit(dir, "ABC-1 step 3 RED: helper only");
    commit(dir, "ABC-1 step 3 GREEN: code");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    for label in ["step 1", "step 2", "step 3"] {
        assert_eq!(outcome(&text, label), "no-tests", "{label}\n{text}");
    }
}

#[test]
fn outcome_red_changes_code() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "a", "assert_eq!(tiny::add(1, 2), 3);");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{LIB}\npub fn more() {{}}\n"),
    );
    commit(dir, "ABC-1 step 1 RED: test and code");
    commit(dir, "ABC-1 step 1 GREEN: code");
    add_test(dir, "basic", "b", "assert!(true);");
    write(dir, "docs/impl.rs", "pub fn hidden() {}\n");
    commit(dir, "ABC-1 step 2 RED: code under docs");
    commit(dir, "ABC-1 step 2 GREEN: code");
    add_test(dir, "basic", "c", "assert!(true);");
    write(dir, "crates/tiny/build.rs", "fn main() {}\n");
    commit(dir, "ABC-1 step 3 RED: a build script");
    commit(dir, "ABC-1 step 3 GREEN: code");
    add_test(dir, "basic", "d", "assert!(true);");
    edit(
        dir,
        "crates/tiny/Cargo.toml",
        "edition = \"2021\"\n",
        "edition = \"2021\"\nbuild = \"build.rs\"\n",
    );
    commit(dir, "ABC-1 step 4 RED: a manifest change");
    commit(dir, "ABC-1 step 4 GREEN: code");
    add_test(dir, "basic", "e", "assert!(true);");
    write(dir, "docs/notes.md", "Notes.\n");
    edit(
        dir,
        "crates/tiny/Cargo.toml",
        "build = \"build.rs\"\n",
        "build = \"build.rs\"\n\n[dev-dependencies]\n",
    );
    commit(
        dir,
        "ABC-1 step 5 RED: notes and an empty dev-dependencies table",
    );
    commit(dir, "ABC-1 step 5 GREEN: code");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    for label in ["step 1", "step 2", "step 3", "step 4"] {
        assert_eq!(outcome(&text, label), "red-changes-code", "{label}\n{text}");
    }
    assert!(text.contains("crates/tiny/src/lib.rs"), "{text}");
    assert!(text.contains("docs/impl.rs"), "{text}");
    assert_ne!(outcome(&text, "step 5"), "red-changes-code", "{text}");
}

#[test]
fn outcome_tests_changed_after_red() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "weakened", "assert_eq!(tiny::add(2, 2), 5);");
    commit(dir, "ABC-1 step 1 RED: a test");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "assert_eq!(tiny::add(2, 2), 5);",
        "assert!(true);",
    );
    commit(dir, "ABC-1 step 1 GREEN: weakens the test");

    write(dir, "crates/tiny/tests/golden/out.txt", "expected\n");
    add_test(
        dir,
        "golden",
        "reads_golden",
        "assert_eq!(include_str!(\"golden/out.txt\"), \"expected\\n\");",
    );
    commit(dir, "ABC-1 step 2 RED: a golden file");
    commit(dir, "ABC-1 step 2 GREEN: code");
    write(
        dir,
        "crates/tiny/tests/golden/out.txt",
        "whatever the code prints\n",
    );
    commit(dir, "ABC-1: fix the golden");

    write(
        dir,
        "crates/tiny/tests/common/mod.rs",
        "pub fn expected() -> u32 {\n    7\n}\n",
    );
    add_test(
        dir,
        "helper",
        "uses_helper",
        "assert_eq!(tiny::add(3, 4), common::expected());",
    );
    edit(
        dir,
        "crates/tiny/tests/helper.rs",
        "\n#[test]",
        "mod common;\n\n#[test]",
    );
    commit(dir, "ABC-1 step 3 RED: a helper");
    commit(dir, "ABC-1 step 3 GREEN: code");
    edit(dir, "crates/tiny/tests/common/mod.rs", "7", "8");
    commit(dir, "ABC-1 step 3 follow-up: the helper changes");

    add_test(dir, "basic", "spaces", "assert_eq!(\"a b\".len(), 3);");
    commit(dir, "ABC-1 step 4 RED: a literal");
    edit(dir, "crates/tiny/tests/basic.rs", "\"a b\"", "\"ab\"");
    commit(dir, "ABC-1 step 4 GREEN: the literal changes");

    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    for label in ["step 1", "step 2", "step 3", "step 4"] {
        assert_eq!(outcome(&text, label), "tests-changed", "{label}\n{text}");
    }
}

#[test]
fn superseded_tests_are_shown() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "evolves", "assert_eq!(tiny::add(1, 1), 3);");
    commit(dir, "ABC-1 step 1 RED: first version");
    commit(dir, "ABC-1 step 1 GREEN: code");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "assert_eq!(tiny::add(1, 1), 3);",
        "assert_eq!(tiny::add(1, 1), 4);",
    );
    commit(dir, "ABC-1 step 2 RED: second version");
    commit(dir, "ABC-1 step 2 GREEN: code");
    let (_, text) = check(dir);
    assert_ne!(outcome(&text, "step 1"), "tests-changed", "{text}");
    assert!(
        text.lines()
            .any(|l| l.trim() == "superseded by step 2: tiny/basic::evolves"),
        "{text}"
    );
}

#[test]
fn retired_tests_need_a_spec_change() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "kept", "assert!(true);");
    add_test(dir, "basic", "wrong_rule", "assert!(true);");
    commit(dir, "ABC-1 step 1 RED: two tests");
    commit(dir, "ABC-1 step 1 GREEN: code");
    add_test(dir, "basic", "only_one", "assert!(true);");
    commit(dir, "ABC-1 step 2 RED: one test");
    commit(dir, "ABC-1 step 2 GREEN: code");
    add_test(dir, "basic", "silently_gone", "assert!(true);");
    commit(dir, "ABC-1 step 3 RED: one test");
    commit(dir, "ABC-1 step 3 GREEN: code");
    add_test(dir, "basic", "new_rule", "assert!(true);");
    commit(dir, "ABC-1 step 4 RED: the new rule");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn wrong_rule() {\n    assert!(true);\n}\n",
        "",
    );
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn only_one() {\n    assert!(true);\n}\n",
        "",
    );
    commit(
        dir,
        "ABC-1 step 4 GREEN: code\n\nspec change: AC-1, the old rules were wrong",
    );
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn silently_gone() {\n    assert!(true);\n}\n",
        "",
    );
    commit(dir, "ABC-1: tidy up");
    let (_, text) = check(dir);
    assert!(
        text.lines()
            .any(|l| l.trim() == "retired by step 4: tiny/basic::wrong_rule"),
        "{text}"
    );
    assert_eq!(outcome(&text, "step 2"), "retired", "{text}");
    assert!(
        !text.contains("retired by step 4: tiny/basic::silently_gone"),
        "{text}"
    );
    assert!(!text.contains("retired by ABC-1: tidy up"), "{text}");
}

#[test]
fn outcome_order_and_static_outcomes() {
    let repo = cargo_repo();
    let dir = repo.path();
    let marker = dir.join("a-step-test-ran");
    // A step test that would leave a marker if it ran: static outcomes run nothing.
    // Each run appends the commit it ran at; the suite runs at HEAD, so only HEAD may appear.
    let body = format!(
        "let h = std::process::Command::new(\"git\").args([\"rev-parse\", \"HEAD\"]).output().unwrap().stdout;\n    \
         use std::io::Write;\n    \
         std::fs::OpenOptions::new().create(true).append(true).open({:?}).unwrap().write_all(&h).unwrap();",
        marker.display().to_string()
    );
    add_test(dir, "basic", "leaves_a_marker", &body);
    commit(dir, "ABC-1 step 1 RED: two reds and no green");
    add_test(dir, "basic", "leaves_a_marker_too", &body);
    commit(dir, "ABC-1 step 1 RED: again");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{LIB}\npub fn x() {{}}\n"),
    );
    commit(dir, "ABC-1 step 2 RED: code and no test");
    commit(dir, "ABC-1 step 2 GREEN: code");
    add_test(dir, "basic", "marker_and_code", &body);
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{LIB}\npub fn y() {{}}\n"),
    );
    commit(dir, "ABC-1 step 3 RED: test and code");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "fn marker_and_code() {\n",
        "fn marker_and_code() {\n    let _changed = 1;\n",
    );
    commit(dir, "ABC-1 step 3 GREEN: also changes the test");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unpaired", "{text}");
    assert_eq!(outcome(&text, "step 2"), "no-tests", "{text}");
    assert_eq!(outcome(&text, "step 3"), "red-changes-code", "{text}");
    let head = git(dir, &["rev-parse", "HEAD"]);
    let ran = std::fs::read_to_string(&marker).unwrap_or_default();
    assert!(
        ran.lines().all(|l| l == head.trim()),
        "a step test ran at a step commit:\n{ran}\n{text}"
    );
}
// ---- STP-4 step 3 drift review D3-4 ----

// A rename below git's similarity threshold is an added file: all its tests are step tests.
#[test]
fn list_counts_a_dissimilar_rename_as_added() {
    let repo = cargo_repo();
    let dir = repo.path();
    git(
        dir,
        &[
            "mv",
            "crates/tiny/tests/basic.rs",
            "crates/tiny/tests/other.rs",
        ],
    );
    add_test(dir, "other", "new_one", "assert_eq!(tiny::add(4, 4), 8);");
    add_test(dir, "other", "new_two", "assert_eq!(tiny::add(5, 4), 9);");
    commit(dir, "ABC-1 step 1 RED: rename and rewrite");
    let text = list(dir);
    let tests: Vec<String> = step_block(&text, "step 1")
        .into_iter()
        .filter(|l| l.starts_with("tiny/"))
        .collect();
    assert_eq!(
        tests,
        [
            "tiny/other::new_one",
            "tiny/other::new_two",
            "tiny/other::starts"
        ],
        "{text}"
    );
}

// The first commit of a repository can be a RED commit; `--list` ignores a dirty tree.
#[test]
fn list_reads_a_root_commit_and_ignores_a_dirty_tree() {
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
    write(dir, "crates/tiny/src/lib.rs", LIB);
    add_test(dir, "basic", "first", "assert!(true);");
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    stapel(dir).args(["new", "First"]).assert().success();
    commit(dir, "ABC-1 step 1 RED: everything at once");
    write(dir, "crates/tiny/src/lib.rs", "uncommitted change\n");
    let text = list(dir);
    assert!(
        step_block(&text, "step 1").contains(&"tiny/basic::first".to_string()),
        "{text}"
    );
}

// ---- STP-4 step 4 drift review ----

// D4-1: only a commit of the ticket (its subject starts with the key) retires a test.
#[test]
fn retirement_needs_a_ticket_commit() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "foreign", "assert!(true);");
    add_test(dir, "basic", "ours", "assert!(true);");
    commit(dir, "ABC-1 step 1 RED: two tests");
    commit(dir, "ABC-1 step 1 GREEN: code");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn foreign() {\n    assert!(true);\n}\n",
        "",
    );
    commit(dir, "XYZ-9: other work\n\nspec change: not this ticket's");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn ours() {\n    assert!(true);\n}\n",
        "",
    );
    commit(
        dir,
        "ABC-1: drop a rule\n\nspec change: AC-1, the rule was wrong",
    );
    let (_, text) = check(dir);
    assert!(
        !text
            .lines()
            .any(|l| l.contains("retired by") && l.contains("tiny/basic::foreign")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.contains("retired by") && l.contains("tiny/basic::ours")),
        "{text}"
    );
}

// D4-2: a test deleted and added again in a weaker form is still `tests-changed`.
#[test]
fn deleted_and_readded_tests_stay_protected() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "phoenix", "assert_eq!(tiny::add(2, 2), 4);");
    commit(dir, "ABC-1 step 1 RED: a test");
    commit(dir, "ABC-1 step 1 GREEN: code");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn phoenix() {\n    assert_eq!(tiny::add(2, 2), 4);\n}\n",
        "",
    );
    commit(dir, "ABC-1: remove it");
    add_test(dir, "basic", "phoenix", "assert!(true);");
    commit(dir, "ABC-1: bring it back weaker");
    let (_, text) = check(dir);
    assert_eq!(outcome(&text, "step 1"), "tests-changed", "{text}");
}

// D4-7: the paths a RED may change, and at most 20 named code paths.
#[test]
fn red_paths_allowed_and_capped() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "allowed", "assert!(true);");
    write(dir, "README.md", "Read me.\n");
    write(dir, ".stapel/notes.txt", "notes\n");
    write(dir, "docs/plan.txt", "plan\n");
    write(dir, "crates/tiny/tests/golden/new.txt", "golden\n");
    edit(
        dir,
        "crates/tiny/Cargo.toml",
        "edition = \"2021\"\n",
        "edition = \"2021\"\n\n[target.'cfg(unix)'.dev-dependencies]\n",
    );
    commit(dir, "ABC-1 step 1 RED: only allowed paths");
    commit(dir, "ABC-1 step 1 GREEN: code");
    add_test(dir, "basic", "many", "assert!(true);");
    for n in 0..25 {
        write(dir, &format!("crates/tiny/src/m{n:02}.rs"), "\n");
    }
    commit(dir, "ABC-1 step 2 RED: many code paths");
    commit(dir, "ABC-1 step 2 GREEN: code");
    let (_, text) = check(dir);
    assert_ne!(outcome(&text, "step 1"), "red-changes-code", "{text}");
    assert_eq!(outcome(&text, "step 2"), "red-changes-code", "{text}");
    assert!(text.contains("crates/tiny/src/m19.rs and 5 more"), "{text}");
    assert!(!text.contains("m20.rs"), "{text}");
}

// ---- STP-4 AC-3 and AC-4: runs in a worktree ----

/// Sets `[check] timeout_secs` and commits it.
fn set_timeout(dir: &Path, secs: u32) {
    edit(
        dir,
        ".stapel/stapel.toml",
        "timeout_secs = 900",
        &format!("timeout_secs = {secs}"),
    );
    commit(dir, "ABC-1: a shorter time limit");
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

#[test]
fn outcome_pass_with_assert_and_compile_reds() {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");
    add_test(dir, "basic", "halves", "assert_eq!(tiny::half(8), 4);");
    commit(dir, "ABC-1 step 2 RED: half is missing");
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{lib}\npub fn half(x: u32) -> u32 {{\n    x / 2\n}}\n"),
    );
    commit(dir, "ABC-1 step 2 GREEN: half");
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert_eq!(outcome(&text, "step 2"), "pass", "{text}");
    assert!(
        text.lines()
            .any(|l| l.starts_with("step 1: pass") && l.contains("1 assert")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("step 2: pass") && l.contains("1 compile")),
        "{text}"
    );
    assert!(text.lines().any(|l| l.starts_with("suite: pass")), "{text}");
    assert!(text.lines().any(|l| l == "result: pass"), "{text}");
}

#[test]
fn outcome_no_red() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(
        dir,
        "basic",
        "already_true",
        "assert_eq!(tiny::add(1, 1), 2);",
    );
    commit(dir, "ABC-1 step 1 RED: a test that passes from the start");
    commit(dir, "ABC-1 step 1 GREEN: nothing to do");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "no-red", "{text}");
    assert!(text.contains("tiny/basic::already_true"), "{text}");
}

#[test]
fn outcome_not_green() {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    edit(dir, "crates/tiny/src/lib.rs", "x * 2", "x * 4");
    commit(dir, "ABC-1 step 1 GREEN: still wrong");
    edit(dir, "crates/tiny/src/lib.rs", "x * 4", "x * 3");
    commit(dir, "ABC-1: fixed later");
    add_test(dir, "basic", "halves", "assert_eq!(tiny::half(8), 4);");
    commit(dir, "ABC-1 step 2 RED: half is missing");
    write(dir, "crates/tiny/src/lib.rs", "this does not compile\n");
    commit(dir, "ABC-1 step 2 GREEN: a broken library");
    let lib = format!(
        "{LIB}\npub fn triple(x: u32) -> u32 {{\n    x * 3\n}}\n\npub fn half(x: u32) -> u32 {{\n    x / 2\n}}\n"
    );
    write(dir, "crates/tiny/src/lib.rs", &lib);
    commit(dir, "ABC-1: the library repaired");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "not-green", "{text}");
    assert_eq!(outcome(&text, "step 2"), "not-green", "{text}");
}

#[test]
fn outcome_removed_or_ignored_at_head() {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(
        dir,
        "basic",
        "gone_later",
        "assert_eq!(tiny::triple(1), 3);",
    );
    commit(dir, "ABC-1 step 1 RED: a test");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: code");
    add_test(
        dir,
        "basic",
        "ignored_later",
        "assert_eq!(tiny::half(2), 1);",
    );
    commit(dir, "ABC-1 step 2 RED: a test");
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{lib}\npub fn half(x: u32) -> u32 {{\n    x / 2\n}}\n"),
    );
    commit(dir, "ABC-1 step 2 GREEN: code");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn gone_later() {\n    assert_eq!(tiny::triple(1), 3);\n}\n",
        "",
    );
    commit(dir, "ABC-1: delete a test without a spec change");
    edit(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn ignored_later()",
        "#[test]\n#[ignore]\nfn ignored_later()",
    );
    add_test(dir, "basic", "third", "assert_eq!(tiny::quarter(8), 2);");
    commit(
        dir,
        "ABC-1 step 3 RED: ignores the step 2 test and adds one",
    );
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{lib}\npub fn quarter(x: u32) -> u32 {{\n    x / 4\n}}\n"),
    );
    commit(dir, "ABC-1 step 3 GREEN: code");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "removed", "{text}");
    assert_eq!(outcome(&text, "step 2"), "removed", "{text}");
}

#[test]
fn outcome_unverified_on_timeout_and_stale_lock() {
    let repo = cargo_repo();
    let dir = repo.path();
    set_timeout(dir, 10);
    // The test starts a child that would outlive it; the whole process group is killed.
    add_test(
        dir,
        "slow",
        "hangs",
        "std::process::Command::new(\"sleep\").arg(\"301\").spawn().unwrap();\n    std::thread::sleep(std::time::Duration::from_secs(120));",
    );
    commit(dir, "ABC-1 step 1 RED: a test that hangs");
    commit(dir, "ABC-1 step 1 GREEN: code");
    edit(
        dir,
        "crates/tiny/Cargo.toml",
        "version = \"0.1.0\"",
        "version = \"0.2.0\"",
    );
    commit(dir, "ABC-1: a version bump without the lock file");
    add_test(
        dir,
        "basic",
        "after_bump",
        "assert_eq!(tiny::add(1, 1), 3);",
    );
    commit(dir, "ABC-1 step 2 RED: a test");
    commit(dir, "ABC-1 step 2 GREEN: code");
    let started = std::time::Instant::now();
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(110),
        "{text}"
    );
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    assert!(
        text.lines()
            .any(|l| l.starts_with("step 1: unverified") && l.contains("time")),
        "{text}"
    );
    assert_eq!(outcome(&text, "step 2"), "unverified", "{text}");
    assert!(
        text.lines()
            .any(|l| l.starts_with("step 2: unverified") && l.contains("Cargo.lock")),
        "{text}"
    );
    let left = std::process::Command::new("pgrep")
        .args(["-f", "sleep 301"])
        .output()
        .unwrap();
    assert!(
        left.stdout.is_empty(),
        "a child survived: {}",
        String::from_utf8_lossy(&left.stdout)
    );
}

#[test]
fn outcome_unverified_on_ignored_or_unmatched_test() {
    let repo = cargo_repo();
    let dir = repo.path();
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        &format!(
            "{}\n#[test]\n#[ignore]\nfn skipped() {{\n    assert!(false);\n}}\n",
            read(dir, "crates/tiny/tests/basic.rs")
        ),
    );
    commit(dir, "ABC-1 step 1 RED: an ignored test");
    commit(dir, "ABC-1 step 1 GREEN: code");
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        &format!(
            "{}\n#[test]\n#[cfg(any())]\nfn never_built() {{\n    assert!(false);\n}}\n",
            read(dir, "crates/tiny/tests/basic.rs")
        ),
    );
    commit(dir, "ABC-1 step 2 RED: a test that is not compiled");
    commit(dir, "ABC-1 step 2 GREEN: code");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    assert!(
        text.lines()
            .any(|l| l.starts_with("step 1: unverified") && l.contains("ignored")),
        "{text}"
    );
    assert_eq!(outcome(&text, "step 2"), "unverified", "{text}");
}

#[test]
fn cargo_error_is_not_a_red() {
    let repo = cargo_repo();
    let dir = repo.path();
    write(dir, "crates/tiny/src/lib.rs", "not rust at all\n");
    commit(dir, "ABC-1: the library is broken before the step");
    add_test(dir, "basic", "needs_lib", "assert_eq!(tiny::add(2, 2), 4);");
    commit(
        dir,
        "ABC-1 step 1 RED: fails only because the library does not build",
    );
    write(dir, "crates/tiny/src/lib.rs", LIB);
    commit(dir, "ABC-1 step 1 GREEN: the library builds");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
}

#[test]
fn child_output_cannot_fake_a_result() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(
        dir,
        "basic",
        "fake",
        "std::process::Command::new(\"sh\").args([\"-c\", \"echo 'test fake ... ok'\"]).status().unwrap();\n    assert_eq!(tiny::add(1, 1), 2);",
    );
    commit(dir, "ABC-1 step 1 RED: a test that prints a result line");
    commit(dir, "ABC-1 step 1 GREEN: code");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
}

proptest::proptest! {
    #[test]
    fn output_parser_never_panics(out in "\\PC{0,400}", n in 0usize..4, exit in proptest::option::of(-2i32..300)) {
        let names: Vec<String> = (0..n).map(|i| format!("t{i}")).collect();
        let started = std::time::Instant::now();
        let _ = stapel_core::runner::parse_run(&out, &names, exit, "basic", true);
        let _ = stapel_core::runner::parse_run(&out, &names, exit, "basic", false);
        proptest::prop_assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }
}

#[test]
fn runs_in_a_worktree_and_leaves_the_tree_alone() {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");
    // A leftover worktree folder from a killed check.
    let common = dir.join(".git/stapel");
    write(&common, "check-worktree/junk.txt", "left over\n");
    let machine = |p: &str| p.contains("runs.jsonl");
    let before: Vec<_> = common::snapshot(dir)
        .into_iter()
        .filter(|(p, _, _)| !machine(p))
        .collect();
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    let after: Vec<_> = common::snapshot(dir)
        .into_iter()
        .filter(|(p, _, _)| !machine(p))
        .collect();
    assert_eq!(before, after, "the working tree changed");
    assert!(!common.join("check-worktree").exists());
    let worktrees = git(dir, &["worktree", "list"]);
    assert_eq!(worktrees.lines().count(), 1, "{worktrees}");
}

#[test]
fn refuses_a_dirty_tree() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "x", "assert!(false);");
    commit(dir, "ABC-1 step 1 RED: x");
    commit(dir, "ABC-1 step 1 GREEN: x");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{LIB}\n// uncommitted\n"),
    );
    let (code, text) = check(dir);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("crates/tiny/src/lib.rs"), "{text}");
    git(dir, &["checkout", "--", "crates/tiny/src/lib.rs"]);
    write(dir, "crates/tiny/tests/new_file.rs", "\n");
    let (code, text) = check(dir);
    assert_eq!(code, 2, "{text}");
    std::fs::remove_file(dir.join("crates/tiny/tests/new_file.rs")).unwrap();
    // A machine file does not make the tree dirty.
    let state = dir.join(".stapel/tickets/ABC-1/state.json");
    let text_state = std::fs::read_to_string(&state).unwrap();
    std::fs::write(&state, format!("{text_state}\n")).unwrap();
    let (code, text) = check(dir);
    assert_ne!(code, 2, "{text}");
}

#[test]
fn refuses_ticket_without_steps() {
    let repo = cargo_repo();
    let dir = repo.path();
    let (code, text) = check(dir);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("no step commits"), "{text}");
    stapel(dir).args(["check", "ABC-7"]).assert().code(2);
}

#[test]
fn refuses_without_configuration() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "x", "assert!(false);");
    commit(dir, "ABC-1 step 1 RED: x");
    commit(dir, "ABC-1 step 1 GREEN: x");
    let config = read(dir, ".stapel/stapel.toml");
    let without: String = config
        .lines()
        .filter(|l| {
            !(l.starts_with("[check]") || l.starts_with("runner") || l.starts_with("timeout_secs"))
        })
        .map(|l| format!("{l}\n"))
        .collect();
    write(dir, ".stapel/stapel.toml", &without);
    commit(dir, "ABC-1: no check section");
    let (code, text) = check(dir);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("check is not configured"), "{text}");
}

#[test]
fn refuses_a_second_check_and_takes_over_a_dead_lock() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "x", "assert_eq!(tiny::add(1, 1), 3);");
    commit(dir, "ABC-1 step 1 RED: x");
    commit(dir, "ABC-1 step 1 GREEN: x");
    let lock_path = dir.join(".git/stapel/check.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();
    held.try_lock().unwrap();
    let (code, text) = check(dir);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("another stapel check"), "{text}");
    held.unlock().unwrap();
    drop(held);
    std::fs::write(&lock_path, "999999999\n").unwrap();
    let (code, text) = check(dir);
    assert_ne!(code, 2, "{text}");
    assert!(text.contains("999999999"), "{text}");
}

#[test]
fn suite_failure_fails_the_check() {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "other", "unrelated_failure", "assert!(false);");
    commit(dir, "ABC-1: a failing test outside the steps");
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert!(text.lines().any(|l| l.starts_with("suite: fail")), "{text}");
    assert!(text.lines().any(|l| l == "result: fail"), "{text}");
}

#[test]
fn report_matches_golden() {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");
    add_test(
        dir,
        "basic",
        "already_true",
        "assert_eq!(tiny::add(1, 1), 2);",
    );
    commit(dir, "ABC-1 step 2 RED: passes from the start");
    commit(dir, "ABC-1 step 2 GREEN: nothing");
    let out = full_check(dir);
    let text = String::from_utf8(out.stdout).unwrap();
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/check_report.txt"),
    )
    .unwrap();
    assert_eq!(
        without_cargo_line(&without_ids(&text)),
        golden,
        "actual:\n{text}"
    );
}

// ---- STP-4 AC-5: run records ----

fn run_records(dir: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join(".stapel/tickets/ABC-1/runs.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["kind"] == "check")
        .collect()
}

#[test]
fn appends_a_run_record() {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    let head = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let records = run_records(dir);
    assert_eq!(records.len(), 1, "{records:?}");
    let r = &records[0];
    assert_eq!(r["v"], 1);
    assert!(r["id"].as_str().unwrap().starts_with("c-"), "{r}");
    assert!(r["at"].as_str().unwrap().ends_with('Z'), "{r}");
    assert_eq!(r["ticket"], "ABC-1");
    assert_eq!(r["head"], head.as_str());
    assert!(r["tool"].as_str().unwrap().starts_with("stapel "), "{r}");
    assert_eq!(r["result"], "pass");
    let step = &r["steps"][0];
    assert_eq!(step["label"], "step 1");
    assert_eq!(step["outcome"], "pass");
    assert_eq!(step["tests"], 1);
    assert_eq!(step["red"].as_str().unwrap().len(), 40, "{step}");
    assert_eq!(step["green"].as_str().unwrap().len(), 40, "{step}");
    assert_eq!(r["suite"]["outcome"], "pass");
    assert_eq!(r["suite"]["passed"], 2);
    assert_eq!(r["suite"]["failed"], 0);

    // A second check appends a second record.
    let (code, _) = check(dir);
    assert_eq!(code, 0);
    let records = run_records(dir);
    assert_eq!(records.len(), 2);
    assert_ne!(records[0]["id"], records[1]["id"]);
}

#[test]
fn record_caps_long_lists() {
    let repo = cargo_repo();
    let dir = repo.path();
    // 250 static steps (a RED without a GREEN), one with a 300-byte label.
    for n in 0..250 {
        add_test(dir, "many", &format!("t{n}"), "assert!(false);");
        let label = if n == 0 {
            "x".repeat(300)
        } else {
            format!("step {n}")
        };
        commit(dir, &format!("ABC-1 {label} RED: test {n}"));
    }
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    let r = &run_records(dir)[0];
    assert_eq!(
        r["steps"].as_array().unwrap().len(),
        200,
        "{}",
        r["steps"].as_array().unwrap().len()
    );
    assert_eq!(r["steps_truncated"], true);
    assert!(r["steps"][0]["label"].as_str().unwrap().len() <= 200);
    assert!(r.to_string().len() < 64 * 1024);
}

#[test]
fn record_names_at_most_twenty() {
    let repo = cargo_repo();
    let dir = repo.path();
    for n in 0..25 {
        add_test(
            dir,
            "wide",
            &format!("w{n:02}"),
            "assert_eq!(tiny::add(1, 1), 3);",
        );
    }
    commit(dir, "ABC-1 step 1 RED: 25 tests");
    let wide = read(dir, "crates/tiny/tests/wide.rs");
    write(
        dir,
        "crates/tiny/tests/wide.rs",
        &wide.replace("assert_eq!(tiny::add(1, 1), 3);", "assert!(true);"),
    );
    commit(dir, "ABC-1 step 1 GREEN: weakens them all");
    let (_, text) = check(dir);
    let r = &run_records(dir)[0];
    let step = &r["steps"][0];
    assert_eq!(step["outcome"], "tests-changed", "{text}");
    assert_eq!(step["names"].as_array().unwrap().len(), 20, "{step}");
}

#[test]
fn other_run_lines_are_ignored() {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");
    let path = dir.join(".stapel/tickets/ABC-1/runs.jsonl");
    let before = "{\"v\": 1, \"step\": \"7\", \"result\": \"pass\"}\nnot json\n";
    std::fs::write(&path, before).unwrap();
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.starts_with(before), "{after}");
    assert_eq!(run_records(dir).len(), 1);
}

// ---- STP-4 step 5 drift review ----

// D5-2: a grandchild in its own session that keeps the output open does not hang the check.
#[test]
fn detached_grandchild_does_not_hang_the_check() {
    let repo = cargo_repo();
    let dir = repo.path();
    set_timeout(dir, 10);
    add_test(
        dir,
        "basic",
        "detaches",
        "std::process::Command::new(\"sh\").args([\"-c\", \"setsid sleep 302 &\"]).status().unwrap();\n    assert_eq!(tiny::add(1, 1), 3);",
    );
    commit(dir, "ABC-1 step 1 RED: a test that leaves a detached child");
    commit(dir, "ABC-1 step 1 GREEN: code");
    let started = std::time::Instant::now();
    // The check is stopped after 90 s, so a hang fails this test instead of blocking it.
    let out = {
        let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
        stapel(dir)
            .args(["check", "ABC-1"])
            .env("CARGO_BUILD_JOBS", "2")
            .timeout(std::time::Duration::from_secs(90))
            .output()
    };
    let took = started.elapsed();
    // The detached child is not this check's to kill (Guarantees); the test ends it.
    let _ = std::process::Command::new("pkill")
        .args(["-x", "-f", "sleep 302"])
        .status();
    let out = out.unwrap_or_else(|e| panic!("the check did not finish: {e}"));
    assert!(
        out.status.code().is_some(),
        "the check was stopped after {took:?}"
    );
    assert!(took < std::time::Duration::from_secs(90), "took {took:?}");
}
#[test]
fn refusal_appends_no_record() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "x", "assert!(false);");
    commit(dir, "ABC-1 step 1 RED: x");
    commit(dir, "ABC-1 step 1 GREEN: x");
    write(dir, "crates/tiny/src/lib.rs", "dirty\n");
    let (code, _) = check(dir);
    assert_eq!(code, 2);
    assert!(!dir.join(".stapel/tickets/ABC-1/runs.jsonl").exists());
}

// ---- STP-4 AC-6: status, close and the dialog ----

fn status_check_line(dir: &Path) -> String {
    let out = stapel(dir).args(["status", "ABC-1"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| l.starts_with("check: "))
        .unwrap_or("(no check line)")
        .to_string()
}

/// A repository whose ticket has one passing step.
fn passing_repo() -> TempDir {
    let repo = cargo_repo();
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");
    repo
}

fn short_head(dir: &Path) -> String {
    git(dir, &["rev-parse", "--short=7", "HEAD"])
        .trim()
        .to_string()
}

#[test]
fn status_shows_the_check_line() {
    let repo = passing_repo();
    let dir = repo.path();
    assert_eq!(status_check_line(dir), "check: none");
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    assert_eq!(
        status_check_line(dir),
        format!("check: pass at {}", short_head(dir))
    );

    add_test(dir, "basic", "breaks", "assert_eq!(tiny::add(1, 1), 3);");
    commit(dir, "ABC-1: a failing test outside the steps");
    let line = status_check_line(dir);
    assert!(
        line.starts_with("check: stale") && line.contains("crates/tiny/tests/basic.rs"),
        "{line}"
    );
    let (code, _) = check(dir);
    assert_eq!(code, 1);
    assert_eq!(
        status_check_line(dir),
        format!("check: fail at {}", short_head(dir))
    );

    let config = read(dir, ".stapel/stapel.toml");
    let without: String = config
        .lines()
        .filter(|l| {
            !(l.starts_with("[check]") || l.starts_with("runner") || l.starts_with("timeout_secs"))
        })
        .map(|l| format!("{l}\n"))
        .collect();
    write(dir, ".stapel/stapel.toml", &without);
    assert_eq!(status_check_line(dir), "check: not configured");
}

#[test]
fn machine_files_keep_a_check_current() {
    let repo = passing_repo();
    let dir = repo.path();
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    let pass = format!("check: pass at {}", short_head(dir));
    commit(dir, "ABC-1: the check record");
    let state = dir.join(".stapel/tickets/ABC-1/state.json");
    let text_state = std::fs::read_to_string(&state).unwrap();
    std::fs::write(&state, format!("{text_state}\n")).unwrap();
    write(dir, ".stapel/tickets/ABC-1/tokens.jsonl", "{\"v\":1}\n");
    assert_eq!(status_check_line(dir), pass);
    write(
        dir,
        ".stapel/tickets/ABC-1/notes.txt",
        "not a machine file\n",
    );
    assert!(
        status_check_line(dir).starts_with("check: stale"),
        "{}",
        status_check_line(dir)
    );
}

#[test]
fn unknown_head_is_stale() {
    let repo = passing_repo();
    let dir = repo.path();
    let record = format!(
        "{{\"v\":1,\"kind\":\"check\",\"id\":\"c-000000000000\",\"at\":\"2026-01-01T00:00:00Z\",\"ticket\":\"ABC-1\",\"head\":\"{}\",\"result\":\"pass\",\"steps\":[]}}\n",
        "0".repeat(40)
    );
    write(dir, ".stapel/tickets/ABC-1/runs.jsonl", &record);
    let line = status_check_line(dir);
    assert!(
        line.starts_with("check: stale") && line.contains("unknown commit"),
        "{line}"
    );
}

#[test]
fn close_needs_a_current_passing_check() {
    let repo = passing_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .assert()
        .code(1)
        .stderr(contains("stapel check"))
        .stderr(contains("check: none"));
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{}\n// later\n", read(dir, "crates/tiny/src/lib.rs")),
    );
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .assert()
        .code(1)
        .stderr(contains("check: stale"));
    git(dir, &["checkout", "--", "crates/tiny/src/lib.rs"]);
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .assert()
        .success()
        .stdout(contains("closed: ABC-1"));
}

#[test]
fn close_dialog_shows_the_check() {
    let repo = passing_repo();
    let dir = repo.path();
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    let (reason, _) = common::ask(dir, "stapel close ABC-1 --reason done");
    assert!(
        reason.contains(&format!("check: pass at {}", short_head(dir))),
        "{reason}"
    );

    let plain = common::stapel_repo();
    let pdir = plain.path();
    stapel(pdir).args(["new", "Plain"]).assert().success();
    let (reason, _) = common::ask(pdir, "stapel close ABC-1 --reason done");
    assert!(reason.contains("check: not configured"), "{reason}");
}

// ---- STP-4 step 6 drift review ----

// D6-1, D6-2: many steps with long reasons still give one record within the journal's line limit.
#[test]
fn record_fits_the_journal_line() {
    let repo = cargo_repo();
    let dir = repo.path();
    for n in 0..70 {
        add_test(dir, "fit", &format!("f{n}"), "assert!(false);");
        for k in 0..20 {
            write(
                dir,
                &format!(
                    "crates/tiny/src/a_rather_long_module_name_for_step_{n:03}_and_path_{k:02}.rs"
                ),
                "\n",
            );
        }
        commit(dir, &format!("ABC-1 step {n} RED: test and code"));
        commit(dir, &format!("ABC-1 step {n} GREEN: code"));
    }
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    let records = run_records(dir);
    assert_eq!(
        records.len(),
        1,
        "no record: {}",
        text.lines().last().unwrap_or("")
    );
    let r = &records[0];
    assert!(r.to_string().len() < 64 * 1024);
    assert_eq!(
        r["steps_truncated"],
        true,
        "{}",
        r["steps"].as_array().unwrap().len()
    );
}

// ---- STP-4 step 7 drift review ----

// D7-3: a record counts only with a full commit id; a ref name such as `HEAD` is not one.
#[test]
fn a_record_needs_a_full_commit_id() {
    let repo = passing_repo();
    let dir = repo.path();
    write(
        dir,
        ".stapel/tickets/ABC-1/runs.jsonl",
        "{\"v\":1,\"kind\":\"check\",\"id\":\"c-000000000000\",\"at\":\"2026-01-01T00:00:00Z\",\"ticket\":\"ABC-1\",\"head\":\"HEAD\",\"result\":\"pass\",\"steps\":[]}\n",
    );
    let line = status_check_line(dir);
    assert!(!line.starts_with("check: pass"), "{line}");
}

// D7-2: a current but failing check does not let a ticket close.
#[test]
fn close_refuses_a_failing_check() {
    let repo = passing_repo();
    let dir = repo.path();
    add_test(dir, "other", "fails", "assert!(false);");
    commit(dir, "ABC-1: a failing test outside the steps");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .assert()
        .code(1)
        .stderr(contains("check: fail at"));
}

// ---- STP-4 code review round 1 ----

// F-1: a squashed or amended history with the same tree is stale.
#[test]
fn rewritten_history_is_stale() {
    let repo = passing_repo();
    let dir = repo.path();
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    commit(dir, "ABC-1: a commit to squash");
    git(dir, &["reset", "-q", "--soft", "HEAD~3"]);
    commit(dir, "ABC-1: everything squashed");
    let line = status_check_line(dir);
    assert!(line.starts_with("check: stale"), "{line}");
}

// F-2: a committed rename into a machine file still names the code it removed.
#[test]
fn a_rename_after_the_check_is_stale() {
    let repo = passing_repo();
    let dir = repo.path();
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    git(
        dir,
        &[
            "mv",
            "crates/tiny/src/lib.rs",
            ".stapel/tickets/ABC-1/tokens.jsonl",
        ],
    );
    commit(dir, "ABC-1: code moved into a machine file");
    let line = status_check_line(dir);
    assert!(
        line.starts_with("check: stale") && line.contains("crates/tiny/src/lib.rs"),
        "{line}"
    );
}

// E-3: a golden file the RED only reads is protected too.
#[test]
fn untouched_golden_files_are_protected() {
    let repo = cargo_repo();
    let dir = repo.path();
    write(dir, "crates/tiny/tests/golden/out.txt", "four\n");
    commit(dir, "ABC-1: a golden file before the steps");
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
        &format!("{lib}\npub fn render() -> &'static str {{\n    \"five\\n\"\n}}\n"),
    );
    write(dir, "crates/tiny/tests/golden/out.txt", "five\n");
    commit(
        dir,
        "ABC-1 step 1 GREEN: code, and the golden file follows it",
    );
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "tests-changed", "{text}");
}

// E-9: bytes inside a commit body cannot add a step commit.
#[test]
fn commit_bodies_cannot_inject_steps() {
    let repo = cargo_repo();
    let dir = repo.path();
    add_test(dir, "basic", "real", "assert!(true);");
    commit(dir, "ABC-1 step 1 RED: real");
    let head = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    commit(
        dir,
        &format!("ABC-1: notes\n\nbody\x1e{head}\x1fABC-1 step 9 RED: injected\x1fbody"),
    );
    let text = list(dir);
    assert!(!text.contains("step 9"), "{text}");
}

// F-5: a test that passes at RED in either run is no RED.
#[test]
fn a_single_pass_at_red_is_no_red() {
    let repo = cargo_repo();
    let dir = repo.path();
    let counter = dir.join("runs-of-the-flaky-test");
    let body = format!(
        "let first = !std::path::Path::new({0:?}).exists();\n    std::fs::write({0:?}, \"x\").unwrap();\n    assert!(first);",
        counter.display().to_string()
    );
    add_test(dir, "basic", "flaky", &body);
    commit(dir, "ABC-1 step 1 RED: passes once, then fails");
    commit(dir, "ABC-1 step 1 GREEN: code");
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "no-red", "{text}");
}

// E-6: a later RED that rewrites an earlier step's golden file is named in the report.
#[test]
fn superseded_golden_files_are_shown() {
    let repo = cargo_repo();
    let dir = repo.path();
    write(dir, "crates/tiny/tests/golden/out.txt", "one\n");
    add_test(
        dir,
        "golden",
        "first",
        "assert_eq!(include_str!(\"golden/out.txt\"), tiny::out());",
    );
    commit(dir, "ABC-1 step 1 RED: a golden file");
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{lib}\npub fn out() -> &'static str {{\n    \"one\\n\"\n}}\n"),
    );
    commit(dir, "ABC-1 step 1 GREEN: code");
    write(dir, "crates/tiny/tests/golden/out.txt", "two\n");
    add_test(
        dir,
        "basic",
        "second",
        "assert_eq!(tiny::out(), \"two\\n\");",
    );
    commit(dir, "ABC-1 step 2 RED: the golden file changes");
    edit(dir, "crates/tiny/src/lib.rs", "\"one\\n\"", "\"two\\n\"");
    commit(dir, "ABC-1 step 2 GREEN: code");
    let (_, text) = check(dir);
    assert!(
        text.lines()
            .any(|l| l.trim() == "superseded by step 2: crates/tiny/tests/golden/out.txt"),
        "{text}"
    );
}

// E-11: a check does not prune other worktrees, even when their folders are missing.
#[test]
fn other_worktrees_survive_a_check() {
    let repo = passing_repo();
    let dir = repo.path();
    let away = tempfile::tempdir().unwrap();
    let other = away.path().join("other");
    git(
        dir,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            &other.display().to_string(),
            "HEAD",
        ],
    );
    std::fs::remove_dir_all(&other).unwrap();
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    let list = git(dir, &["worktree", "list"]);
    assert!(list.contains("other"), "{list}");
}

// E-12: a SHA-256 repository gets a current check.
#[test]
fn sha256_repositories_work() {
    let dir = tempfile::tempdir().unwrap();
    let init = std::process::Command::new("git")
        .args(["init", "-q", "--object-format=sha256"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(init.success());
    common::git_config(dir.path(), "user.name", "test-user");
    let repo = fill_cargo_repo(dir);
    let dir = repo.path();
    with_wrong_triple(dir);
    add_test(dir, "basic", "triples", "assert_eq!(tiny::triple(2), 6);");
    commit(dir, "ABC-1 step 1 RED: triple is wrong");
    fix_triple(dir);
    commit(dir, "ABC-1 step 1 GREEN: triple fixed");
    let (code, text) = check(dir);
    assert_eq!(code, 0, "{text}");
    assert!(
        status_check_line(dir).starts_with("check: pass at"),
        "{}",
        status_check_line(dir)
    );
}

// E-14: a shallow clone is refused.
#[test]
fn shallow_clones_are_refused() {
    let repo = passing_repo();
    let dir = repo.path();
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
    let (code, text) = check(&clone);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("shallow"), "{text}");
}
// E-8: paths are given to git literally, so glob characters in a file name do not hide a change.
#[test]
fn glob_characters_in_paths_are_literal() {
    let repo = cargo_repo();
    let dir = repo.path();
    write(dir, "crates/tiny/tests/golden/out[1].txt", "expected\n");
    add_test(
        dir,
        "golden",
        "reads",
        "assert_eq!(include_str!(\"golden/out[1].txt\"), tiny::shown());",
    );
    commit(dir, "ABC-1 step 1 RED: a golden file with brackets");
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{lib}\npub fn shown() -> &'static str {{\n    \"other\\n\"\n}}\n"),
    );
    write(dir, "crates/tiny/tests/golden/out[1].txt", "other\n");
    commit(
        dir,
        "ABC-1 step 1 GREEN: code, and the golden file follows it",
    );
    let (code, text) = check(dir);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "tests-changed", "{text}");
}

// ---- STP-6 AC-8 to AC-11: the environment and the cargo program ----

/// The report with its `cargo: <path> (<version>)` line (STP-6 AC-9) replaced by a fixed one.
fn without_cargo_line(text: &str) -> String {
    text.lines()
        .map(|l| {
            if l.starts_with("cargo: ") {
                "cargo: <cargo>\n".to_string()
            } else {
                format!("{l}\n")
            }
        })
        .collect()
}

/// An executable shell script `name` in `dir`.
fn script(dir: &Path, name: &str, body: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

/// The first `cargo` in an absolute entry of this test's `PATH`.
fn real_cargo() -> String {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .filter(|p| p.is_absolute())
        .map(|p| p.join("cargo"))
        .find(|p| p.is_file())
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// Sets `[check] cargo` and commits the change.
fn set_cargo_key(dir: &Path, cargo: &str) {
    let text = read(dir, ".stapel/stapel.toml");
    let text: String = text
        .lines()
        .filter(|l| !l.starts_with("cargo = "))
        .map(|l| format!("{l}\n"))
        .collect();
    let text = text.replacen(
        "timeout_secs = 900\n",
        &format!("timeout_secs = 900\ncargo = \"{cargo}\"\n"),
        1,
    );
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: the cargo key");
}

fn cargo_line(text: &str) -> String {
    text.lines()
        .find(|l| l.starts_with("cargo: "))
        .unwrap_or("(no cargo line)")
        .to_string()
}

#[test]
fn cargo_runs_with_the_allow_list_only() {
    let repo = passing_repo();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let dumps = scripts.path().join("dumps");
    std::fs::create_dir(&dumps).unwrap();
    let fake = script(
        scripts.path(),
        "cargo",
        &format!(
            "env > {}/env.$$\nexec {} \"$@\"",
            dumps.display(),
            real_cargo()
        ),
    );
    set_cargo_key(dir, &fake);
    let out = {
        let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
        stapel(dir)
            .args(["check", "ABC-1"])
            .env("CARGO_BUILD_JOBS", "2")
            .env("RUSTC_WRAPPER", "/nonexistent")
            .env("RUSTFLAGS", "--cfg=fake")
            .env("RUSTC", "/nonexistent")
            .env("CARGO_ALIAS_TEST", "x")
            .env("STAPEL_TEST_SECRET", "1")
            .output()
            .unwrap()
    };
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    let allowed = [
        "HOME",
        "USER",
        "PATH",
        "LANG",
        "TMPDIR",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "CARGO_BUILD_JOBS",
        "CARGO_TARGET_DIR",
        "CARGO_TERM_COLOR",
        // set by the shell itself
        "PWD",
        "OLDPWD",
        "SHLVL",
        "_",
    ];
    let mut seen = 0;
    for entry in std::fs::read_dir(&dumps).unwrap() {
        let dump = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        seen += 1;
        let names: Vec<&str> = dump
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, _)| k))
            .collect();
        for n in &names {
            assert!(allowed.contains(n), "{n} reached cargo: {dump}");
        }
        for must in ["PATH", "CARGO_TARGET_DIR", "CARGO_TERM_COLOR"] {
            assert!(names.contains(&must), "{must} is missing: {dump}");
        }
        assert!(
            dump.lines().any(|l| l == "CARGO_BUILD_JOBS=2"),
            "CARGO_BUILD_JOBS did not reach cargo: {dump}"
        );
    }
    assert!(seen >= 3, "cargo ran {seen} times");
}

#[test]
fn cargo_runs_see_every_allowed_variable_and_one_cargo() {
    let repo = passing_repo();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let dumps = scripts.path().join("dumps");
    std::fs::create_dir(&dumps).unwrap();
    // The program that a `PATH` search would find after the real cargo; it must never run.
    let late = scripts.path().join("late");
    std::fs::create_dir(&late).unwrap();
    script(
        &late,
        "cargo",
        &format!("echo wrong > {}/late.$$\nexit 1", dumps.display()),
    );
    let fake = script(
        scripts.path(),
        "cargo",
        &format!(
            "env > {d}/env.$$\nprintf '%s\\n' \"$*\" > {d}/argv.$$\nexec {r} \"$@\"",
            d = dumps.display(),
            r = real_cargo()
        ),
    );
    set_cargo_key(dir, &fake);
    // Every allow-listed name is set in the caller; the toolchain names keep the values of this
    // process so that the real cargo still works, the others get known values.
    let mut sent: Vec<(&str, String)> = Vec::new();
    for name in ["HOME", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN"] {
        if let Some(v) = std::env::var_os(name) {
            sent.push((name, v.to_string_lossy().into_owned()));
        }
    }
    if !sent.iter().any(|(n, _)| *n == "HOME") {
        sent.push(("HOME", scripts.path().to_string_lossy().into_owned()));
    }
    let path = format!("{}:{}", std::env::var("PATH").unwrap(), late.display());
    sent.push(("PATH", path));
    sent.push(("USER", "stapel-test-user".into()));
    sent.push(("LANG", "C".into()));
    sent.push(("TMPDIR", scripts.path().to_string_lossy().into_owned()));
    sent.push(("CARGO_BUILD_JOBS", "2".into()));
    let out = {
        let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
        let mut cmd = stapel(dir);
        cmd.args(["check", "ABC-1"])
            .env("RUSTC_WRAPPER", "/nonexistent")
            .env("RUSTFLAGS", "--cfg=fake")
            .env("RUSTC", "/nonexistent")
            .env("CARGO_ALIAS_TEST", "x")
            .env("STAPEL_TEST_SECRET", "1");
        for (n, v) in &sent {
            cmd.env(n, v);
        }
        cmd.output().unwrap()
    };
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    // set by the shell itself
    let shell = ["PWD", "OLDPWD", "SHLVL", "_"];
    let mut expected: Vec<&str> = sent.iter().map(|(n, _)| *n).collect();
    expected.extend(["CARGO_TARGET_DIR", "CARGO_TERM_COLOR"]);
    expected.sort_unstable();
    let mut seen = 0;
    let mut versions = 0;
    let mut tests = 0;
    for entry in std::fs::read_dir(&dumps).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(!file.starts_with("late."), "a PATH cargo ran: {body}");
        if file.starts_with("argv.") {
            if body.trim() == "--version" {
                versions += 1;
            } else if body.starts_with("test ") {
                tests += 1;
            } else {
                panic!("an unexpected cargo run: {body}");
            }
            continue;
        }
        seen += 1;
        let mut names: Vec<&str> = body
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, _)| k))
            .filter(|n| !shell.contains(n))
            .collect();
        names.sort_unstable();
        assert_eq!(names, expected, "the environment of a run: {body}");
        for (n, v) in &sent {
            assert!(
                body.lines().any(|l| l == format!("{n}={v}")),
                "{n} did not arrive with {v}: {body}"
            );
        }
    }
    assert!(seen >= 3, "cargo ran {seen} times");
    // Every run, the `--version` run included, went through the configured program: one version
    // run, and one test run for each of the RED, GREEN and HEAD commits.
    assert_eq!(versions, 1, "version runs");
    assert_eq!(tests, seen - 1, "test runs of {seen} runs");
}

#[test]
fn cargo_path_and_version_are_reported() {
    let repo = passing_repo();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let real = real_cargo();
    let versioned = |print: &str| {
        script(
            scripts.path(),
            "cargo",
            &format!("if [ \"$1\" = --version ]; then {print}; exit 0; fi\nexec {real} \"$@\""),
        )
    };
    let reported = |dir: &Path| {
        let (code, text) = check(dir);
        assert_eq!(code, 0, "{text}");
        let records = run_records(dir);
        let last = records.last().unwrap()["cargo"]
            .as_str()
            .unwrap_or("(no cargo field)")
            .to_string();
        (cargo_line(&text), last)
    };

    // The key names the program; the first line of `--version` is the version.
    let fake = versioned("printf 'cargo 9.9.9 (fake)\\nsecond line\\n'");
    set_cargo_key(dir, &fake);
    let (line, field) = reported(dir);
    assert_eq!(line, format!("cargo: {fake} (cargo 9.9.9 (fake))"));
    assert_eq!(field, format!("{fake} (cargo 9.9.9 (fake))"));

    // A long, non-UTF-8 line is read lossily and cut to 200 bytes.
    let long = "a".repeat(300);
    versioned(&format!("printf '\\377{long}\\n'"));
    let (line, field) = reported(dir);
    let version = field.strip_prefix(&format!("{fake} (")).unwrap();
    let version = version.strip_suffix(')').unwrap();
    assert_eq!(version.len(), 200, "{field}");
    assert_eq!(version.chars().next(), Some('\u{FFFD}'));
    assert!(version.chars().skip(1).all(|c| c == 'a'), "{field}");
    assert_eq!(line, format!("cargo: {field}"));

    // An empty version reads `?`.
    versioned("printf ''");
    let (line, field) = reported(dir);
    assert_eq!(line, format!("cargo: {fake} (?)"));
    assert_eq!(field, format!("{fake} (?)"));

    // Without the key: the first `cargo` in an absolute `PATH` entry.
    let text = read(dir, ".stapel/stapel.toml");
    write(
        dir,
        ".stapel/stapel.toml",
        &text
            .lines()
            .filter(|l| !l.starts_with("cargo = "))
            .map(|l| format!("{l}\n"))
            .collect::<String>(),
    );
    commit(dir, "ABC-1: no cargo key");
    let (line, field) = reported(dir);
    assert!(
        line.starts_with(&format!("cargo: {real} (cargo ")) && line.ends_with(')'),
        "{line}"
    );
    assert_eq!(line, format!("cargo: {field}"));
}

#[test]
fn refuses_without_a_working_cargo() {
    let repo = passing_repo();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let refused = |dir: &Path, what: &str| {
        let (code, text) = check(dir);
        assert_eq!(code, 2, "{what}: {text}");
        assert!(text.contains("cargo"), "{what}: {text}");
        assert!(
            !dir.join(".stapel/tickets/ABC-1/runs.jsonl").exists(),
            "{what}: a record was written"
        );
    };

    // `cargo --version` fails.
    let broken = script(scripts.path(), "broken", "echo broken >&2\nexit 1");
    set_cargo_key(dir, &broken);
    refused(dir, "version fails");

    // `cargo --version` runs past `timeout_secs`.
    let slow = script(scripts.path(), "slow", "exec sleep 60");
    set_cargo_key(dir, &slow);
    set_timeout(dir, 10);
    let started = std::time::Instant::now();
    refused(dir, "version times out");
    assert!(started.elapsed().as_secs() < 50, "the limit did not apply");

    // No `cargo` in an absolute `PATH` entry; a relative entry does not count.
    let text = read(dir, ".stapel/stapel.toml");
    write(
        dir,
        ".stapel/stapel.toml",
        &text
            .lines()
            .filter(|l| !l.starts_with("cargo = "))
            .map(|l| format!("{l}\n"))
            .collect::<String>(),
    );
    // `bin/cargo` in the repository is reachable only through a relative `PATH` entry.
    std::fs::create_dir(dir.join("bin")).unwrap();
    script(&dir.join("bin"), "cargo", "exec true");
    commit(dir, "ABC-1: a cargo in a relative folder");
    let git_bin = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .find(|p| p.join("git").is_file())
        .unwrap();
    let only_git = scripts.path().join("only-git");
    std::fs::create_dir(&only_git).unwrap();
    std::os::unix::fs::symlink(git_bin.join("git"), only_git.join("git")).unwrap();
    let out = {
        let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
        stapel(dir)
            .args(["check", "ABC-1"])
            .env("PATH", format!("bin:{}", only_git.display()))
            .output()
            .unwrap()
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(text.contains("cargo"), "{text}");
    assert!(!dir.join(".stapel/tickets/ABC-1/runs.jsonl").exists());
}

#[test]
fn cargo_key_is_validated_through_the_cli() {
    let repo = passing_repo();
    let dir = repo.path();
    // A program inside the repository.
    std::fs::create_dir(dir.join("tools")).unwrap();
    let inside = script(&dir.join("tools"), "cargo", "exec true");
    for (what, value) in [
        ("inside the repository", inside.as_str()),
        ("relative", "cargo"),
    ] {
        set_cargo_key(dir, value);
        let (code, text) = check(dir);
        assert_eq!(code, 2, "{what}: {text}");
        assert!(text.contains("check.cargo"), "{what}: {text}");
        assert!(
            !dir.join(".stapel/tickets/ABC-1/runs.jsonl").exists(),
            "{what}: a record was written"
        );
    }
}

#[test]
fn cargo_key_is_checked_again_before_the_run() {
    use std::os::unix::fs::PermissionsExt;
    let repo = passing_repo();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    // The key names a link to a program outside the repository; the link is valid at load.
    let outside = script(scripts.path(), "outside", "exec true");
    let link = scripts.path().join("link");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    std::fs::create_dir(dir.join("tools")).unwrap();
    let inside = script(&dir.join("tools"), "cargo", "exec true");
    set_cargo_key(dir, link.to_str().unwrap());
    // `git worktree add` runs this script after the key is loaded: the link then points into
    // the repository.
    let after_checkout = dir.join(".git").join("hooks").join("post-checkout");
    std::fs::write(
        &after_checkout,
        format!("#!/bin/sh\nln -sfn '{inside}' '{}'\n", link.display()),
    )
    .unwrap();
    std::fs::set_permissions(&after_checkout, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (code, text) = check(dir);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("check.cargo"), "{text}");
    assert!(text.contains("inside the repository"), "{text}");
    assert!(
        !dir.join(".stapel/tickets/ABC-1/runs.jsonl").exists(),
        "a record was written"
    );
}

#[test]
fn path_cargo_must_be_executable() {
    let repo = passing_repo();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let real = real_cargo();
    // A `cargo` that is not executable comes first in `PATH`; the real one comes later.
    let plain = scripts.path().join("plain");
    std::fs::create_dir(&plain).unwrap();
    std::fs::write(plain.join("cargo"), "not a program\n").unwrap();
    let path = format!("{}:{}", plain.display(), std::env::var("PATH").unwrap());
    let out = {
        let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
        stapel(dir)
            .args(["check", "ABC-1"])
            .env("CARGO_BUILD_JOBS", "2")
            .env("PATH", path)
            .output()
            .unwrap()
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        cargo_line(&text).starts_with(&format!("cargo: {real} (cargo ")),
        "{text}"
    );
}

/// `stapel check ABC-1` with extra environment variables: exit code and output.
fn check_env(dir: &Path, envs: &[(&str, &Path)]) -> (i32, String) {
    let _one = CARGO.lock().unwrap_or_else(|e| e.into_inner());
    let mut cmd = stapel(dir);
    cmd.args(["check", "ABC-1"]).env("CARGO_BUILD_JOBS", "2");
    for (n, v) in envs {
        cmd.env(n, v);
    }
    let out = cmd.output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
}

/// A cargo that records the folder of every run in `dumps`, then runs the real cargo.
fn recording_cargo(scripts: &Path, dumps: &Path) -> String {
    std::fs::create_dir_all(dumps).unwrap();
    script(
        scripts,
        "cargo",
        &format!(
            "pwd -P > {d}/pwd.$$\nexec {r} \"$@\"",
            d = dumps.display(),
            r = real_cargo()
        ),
    )
}

#[test]
fn worktree_is_outside_the_repository() {
    let repo = passing_repo();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let dumps = scripts.path().join("dumps");
    let fake = recording_cargo(scripts.path(), &dumps);
    set_cargo_key(dir, &fake);
    let tmp = tempfile::tempdir().unwrap();
    let tmp_real = tmp.path().canonicalize().unwrap();
    let repo_real = dir.canonicalize().unwrap();
    let (code, text) = check_env(dir, &[("TMPDIR", tmp.path())]);
    assert_eq!(code, 0, "{text}");
    let mut seen = 0;
    let mut folders = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(&dumps).unwrap() {
        let body = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        let folder = Path::new(body.trim()).to_path_buf();
        assert_eq!(folder.parent(), Some(tmp_real.as_path()), "{folder:?}");
        assert!(!folder.starts_with(&repo_real), "{folder:?}");
        folders.insert(folder);
        seen += 1;
    }
    assert!(seen >= 3, "cargo ran {seen} times");
    assert_eq!(folders.len(), 1, "one worktree folder: {folders:?}");
    // Removed when done; the lock and the target folder stay under `.git/stapel/`.
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    assert!(dir.join(".git/stapel/check.lock").exists());
    assert!(dir.join(".git/stapel/check-target").is_dir());
    assert_eq!(git(dir, &["worktree", "list"]).lines().count(), 1);
    // A leftover folder of an earlier run stays; the next check does not break on it.
    std::fs::create_dir(tmp.path().join("stapel-check-leftover")).unwrap();
    let (code, text) = check_env(dir, &[("TMPDIR", tmp.path())]);
    assert_eq!(code, 0, "{text}");
    assert!(tmp.path().join("stapel-check-leftover").is_dir());
}

#[test]
fn config_above_the_worktree_is_unverified() {
    let repo = passing_repo();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let dumps = scripts.path().join("dumps");
    let fake = recording_cargo(scripts.path(), &dumps);
    set_cargo_key(dir, &fake);
    for file in [
        ".cargo/config",
        ".cargo/config.toml",
        "rust-toolchain",
        "rust-toolchain.toml",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), file, "");
        let held = tmp.path().canonicalize().unwrap().join(file);
        let (code, text) = check_env(dir, &[("TMPDIR", tmp.path())]);
        assert_eq!(code, 1, "{file}: {text}");
        let want = format!("build-input-outside: {}", held.display());
        assert!(
            text.contains(&format!("step 1: unverified: {want}")),
            "{file}: {text}"
        );
        assert!(
            text.contains(&format!("suite: unverified: {want}")),
            "{file}: {text}"
        );
        // Found before `cargo --version` runs.
        assert_eq!(std::fs::read_dir(&dumps).unwrap().count(), 0, "{file}");
    }
    // The configuration of `CARGO_HOME` is not promised and does not count.
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), ".cargo/config.toml", "");
    let home = tmp.path().join(".cargo");
    let (code, text) = check_env(dir, &[("TMPDIR", tmp.path()), ("CARGO_HOME", &home)]);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn refuses_a_tmpdir_inside_the_repository() {
    let repo = passing_repo();
    let dir = repo.path();
    let runs = dir.join(".stapel/tickets/ABC-1/runs.jsonl");
    let inside = dir.join("scratch");
    std::fs::create_dir(&inside).unwrap();
    let away = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(&inside, away.path().join("link")).unwrap();
    let missing = away.path().join("missing");
    let cases: [(&str, &Path); 3] = [
        ("inside", &inside),
        ("link", &away.path().join("link")),
        ("missing", &missing),
    ];
    for (what, tmp) in cases {
        let (code, text) = check_env(dir, &[("TMPDIR", tmp)]);
        assert_eq!(code, 2, "{what}: {text}");
        assert!(text.contains("TMPDIR"), "{what}: {text}");
        assert!(!runs.exists(), "{what}: a record was written");
    }
    // A relative `TMPDIR` is resolved against the current folder, here the repository.
    let (code, text) = check_env(dir, &[("TMPDIR", Path::new("scratch"))]);
    assert_eq!(code, 2, "relative: {text}");
    assert!(!runs.exists(), "relative: a record was written");
}

#[test]
fn history_outcomes_come_before_build_input_outside() {
    let repo = passing_repo();
    let dir = repo.path();
    add_test(dir, "basic", "lonely", "assert!(true);");
    commit(dir, "ABC-1 step 2 RED: only a red");
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "rust-toolchain.toml", "");
    let (code, text) = check_env(dir, &[("TMPDIR", tmp.path())]);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 2"), "unpaired", "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    assert!(
        text.contains("step 1: unverified: build-input-outside:"),
        "{text}"
    );
}

#[test]
fn unwritable_tmpdir_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let repo = passing_repo();
    let dir = repo.path();
    let runs = dir.join(".stapel/tickets/ABC-1/runs.jsonl");
    let tmp = tempfile::tempdir().unwrap();
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    let writable = std::fs::File::create(tmp.path().join("probe")).is_ok();
    if !writable {
        let (code, text) = check_env(dir, &[("TMPDIR", tmp.path())]);
        assert_eq!(code, 2, "{text}");
        assert!(text.contains("TMPDIR"), "{text}");
        assert!(!runs.exists(), "a record was written");
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn cargo_home_cannot_hide_a_config_above() {
    let repo = passing_repo();
    let dir = repo.path();
    // `CARGO_HOME` is the `TMPDIR` itself, with a planted configuration in it.
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), ".cargo/config.toml", "");
    let home = tmp.path().canonicalize().unwrap();
    let (code, text) = check_env(dir, &[("TMPDIR", tmp.path()), ("CARGO_HOME", &home)]);
    assert_eq!(code, 1, "same folder: {text}");
    let want = format!("build-input-outside: {}", home.display());
    assert!(
        text.contains(&format!("step 1: unverified: {want}")),
        "{text}"
    );
    // `CARGO_HOME` is a folder above `TMPDIR` and holds a `rust-toolchain.toml`.
    let up = tempfile::tempdir().unwrap();
    write(up.path(), "rust-toolchain.toml", "");
    let inner = up.path().join("inner");
    std::fs::create_dir(&inner).unwrap();
    let home = up.path().canonicalize().unwrap();
    let (code, text) = check_env(dir, &[("TMPDIR", &inner), ("CARGO_HOME", &home)]);
    assert_eq!(code, 1, "ancestor: {text}");
    let want = format!("build-input-outside: {}", home.display());
    assert!(
        text.contains(&format!("step 1: unverified: {want}")),
        "{text}"
    );
    // `rust-toolchain` directly in a `.cargo` folder that is `CARGO_HOME`'s parent still counts.
    let top = tempfile::tempdir().unwrap();
    write(top.path(), "rust-toolchain", "");
    write(top.path(), ".cargo/config.toml", "");
    let home = top.path().join(".cargo");
    let (code, text) = check_env(dir, &[("TMPDIR", top.path()), ("CARGO_HOME", &home)]);
    assert_eq!(code, 1, "toolchain: {text}");
    let held = top.path().canonicalize().unwrap().join("rust-toolchain");
    assert!(
        text.contains(&format!("build-input-outside: {}", held.display())),
        "{text}"
    );
}

#[test]
fn relative_tmpdir_is_resolved() {
    let repo = passing_repo();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    let dumps = scripts.path().join("dumps");
    let fake = recording_cargo(scripts.path(), &dumps);
    set_cargo_key(dir, &fake);
    let sibling = tempfile::tempdir_in(dir.parent().unwrap()).unwrap();
    let rel = Path::new("..").join(sibling.path().file_name().unwrap());
    let (code, text) = check_env(dir, &[("TMPDIR", &rel)]);
    assert_eq!(code, 0, "{text}");
    let real = sibling.path().canonicalize().unwrap();
    let mut seen = 0;
    for entry in std::fs::read_dir(&dumps).unwrap() {
        let body = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        assert_eq!(Path::new(body.trim()).parent(), Some(real.as_path()));
        seen += 1;
    }
    assert!(seen >= 1, "cargo did not run");
}

#[test]
fn stale_check_worktrees_are_pruned() {
    let repo = passing_repo();
    let dir = repo.path();
    let tmp = tempfile::tempdir().unwrap();
    let stale = tmp.path().join("stapel-check-crashed");
    let other = tmp.path().join("other-gone");
    let live = tmp.path().join("live-one");
    for p in [&stale, &other, &live] {
        git(
            dir,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                p.to_str().unwrap(),
                "HEAD",
            ],
        );
    }
    std::fs::remove_dir_all(&stale).unwrap();
    std::fs::remove_dir_all(&other).unwrap();
    let (code, text) = check_env(dir, &[("TMPDIR", tmp.path())]);
    assert_eq!(code, 0, "{text}");
    let list = git(dir, &["worktree", "list", "--porcelain"]);
    assert!(!list.contains("stapel-check-crashed"), "{list}");
    // Other worktrees are not touched, a missing folder or not.
    assert!(list.contains("other-gone"), "{list}");
    assert!(list.contains("live-one"), "{list}");
}

#[test]
fn unreadable_ancestor_counts_as_found() {
    use std::os::unix::fs::PermissionsExt;
    let repo = passing_repo();
    let dir = repo.path();
    let top = tempfile::tempdir().unwrap();
    let cargo = top.path().join(".cargo");
    std::fs::create_dir(&cargo).unwrap();
    let inner = top.path().join("inner");
    std::fs::create_dir(&inner).unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o000)).unwrap();
    let readable = std::fs::symlink_metadata(cargo.join("config"))
        .map_or_else(|e| e.kind() == std::io::ErrorKind::NotFound, |_| true);
    if !readable {
        let (code, text) = check_env(dir, &[("TMPDIR", &inner)]);
        assert_eq!(code, 1, "{text}");
        assert!(
            text.contains("step 1: unverified: build-input-outside:"),
            "{text}"
        );
    }
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
}
