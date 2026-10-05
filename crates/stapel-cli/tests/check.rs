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

/// `stapel check ABC-1`: exit code and output.
fn check(dir: &Path) -> (i32, String) {
    let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
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
    let body = format!(
        "std::fs::write({:?}, \"ran\").unwrap();",
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
    assert!(!marker.exists(), "a step test ran:\n{text}");
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
    fix_triple(dir);
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
    let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/check_report.txt"),
    )
    .unwrap();
    assert_eq!(without_ids(&text), golden, "actual:\n{text}");
}
