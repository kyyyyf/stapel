//! STP-37 AC-9 and AC-12: a `history:` line lets a commit change a protected step test.

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

fn commit(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    let out = std::process::Command::new("git")
        .args(["commit", "-q", "--allow-empty", "-m", message])
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
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A stapel repository with a one-crate cargo workspace and ticket ABC-1, all committed.
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
    write(
        dir,
        "crates/tiny/src/lib.rs",
        "pub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n",
    );
    write(dir, ".gitignore", "/target\n");
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    stapel(dir).args(["new", "First"]).assert().success();
    commit(dir, "initial");
    repo
}

/// The RED of `label`: a new test file `<name>.rs` with the test `<name>`.
fn red(dir: &Path, label: &str, name: &str) {
    write(
        dir,
        &format!("crates/tiny/tests/{name}.rs"),
        &format!("#[test]\nfn {name}() {{\n    assert_eq!(tiny::{name}(), 1);\n}}\n"),
    );
    commit(dir, &format!("ABC-1 {label} RED: {name} is missing"));
}

/// The code a test needs.
fn code(dir: &Path, name: &str) {
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{lib}\npub fn {name}() -> u32 {{\n    1\n}}\n"),
    );
}

/// Changes the text of the test `<name>`.
fn edit_test(dir: &Path, name: &str) {
    let rel = format!("crates/tiny/tests/{name}.rs");
    let old = read(dir, &rel);
    let from = format!("assert_eq!(tiny::{name}(), 1);");
    assert!(old.contains(&from), "{rel}");
    write(
        dir,
        &rel,
        &old.replacen(&from, &format!("assert_eq!(tiny::{name}(), 1 + 0);"), 1),
    );
}

/// A GREEN that adds the code and changes its own test, with `message` after the subject.
fn green_editing(dir: &Path, label: &str, name: &str, message: &str) {
    code(dir, name);
    edit_test(dir, name);
    commit(dir, &format!("ABC-1 {label} GREEN: {name}{message}"));
}

/// `stapel check ABC-1 --history` (`full` adds nothing: the history outcomes do not run cargo).
fn history(dir: &Path) -> String {
    let out = stapel(dir)
        .args(["check", "ABC-1", "--history"])
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The outcome printed for a label.
fn outcome<'a>(text: &'a str, label: &str) -> &'a str {
    let prefix = format!("{label}: ");
    text.lines()
        .find_map(|l| l.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("no line for {label}:\n{text}"))
        .split([' ', ':'])
        .next()
        .unwrap()
}

/// The notes after the line of a step.
fn notes<'a>(text: &'a str, label: &str) -> Vec<&'a str> {
    let prefix = format!("{label}: ");
    let mut lines = text.lines().skip_while(|l| !l.starts_with(&prefix));
    lines.next();
    lines
        .take_while(|l| l.starts_with("  "))
        .map(str::trim)
        .collect()
}

#[test]
fn green_may_change_its_test() {
    let repo = cargo_repo();
    let dir = repo.path();
    // Step 1: its GREEN changes its test and says why.
    red(dir, "step 1", "one");
    green_editing(dir, "step 1", "one", "\n\nhistory: the first assert was too strict");
    // Step 2: the same without the line.
    red(dir, "step 2", "two");
    green_editing(dir, "step 2", "two", "");
    // Step 3: a later fix commit with a history line, outside any step, changes the test.
    red(dir, "step 3", "three");
    code(dir, "three");
    commit(dir, "ABC-1 step 3 GREEN: three");
    edit_test(dir, "three");
    commit(dir, "ABC-1: fix the test of step 3\n\nhistory: the value was spelled badly");
    let fix = git(dir, &["rev-parse", "--short=7", "HEAD"]);
    // Step 4: a history commit after the next step changes the test of step 4.
    red(dir, "step 4", "four");
    code(dir, "four");
    commit(dir, "ABC-1 step 4 GREEN: four");
    red(dir, "step 5", "five");
    code(dir, "five");
    commit(dir, "ABC-1 step 5 GREEN: five");
    edit_test(dir, "four");
    commit(dir, "ABC-1: fix the test of step 4\n\nhistory: no reason to keep the old text");

    let out = history(dir);
    assert_ne!(outcome(&out, "step 1"), "tests-changed", "{out}");
    assert_eq!(
        notes(&out, "step 1"),
        ["history by step 1: the first assert was too strict"],
        "{out}"
    );
    assert_eq!(outcome(&out, "step 2"), "tests-changed", "{out}");
    assert!(notes(&out, "step 2").is_empty(), "{out}");
    assert_ne!(outcome(&out, "step 3"), "tests-changed", "{out}");
    assert_eq!(
        notes(&out, "step 3"),
        [format!(
            "history by {}: the value was spelled badly",
            fix.trim()
        )],
        "{out}"
    );
    assert_ne!(outcome(&out, "step 4"), "tests-changed", "{out}");
    assert!(
        notes(&out, "step 4")
            .iter()
            .any(|n| n.starts_with("history by ")
                && n.ends_with(": no reason to keep the old text")),
        "{out}"
    );
}

#[test]
fn an_empty_reason_counts_as_none() {
    let repo = cargo_repo();
    let dir = repo.path();
    // A valid history commit next to each case: excused.
    red(dir, "step 1", "valid");
    green_editing(dir, "step 1", "valid", "\n\nhistory: a good reason");
    // No reason, and a reason of spaces.
    red(dir, "step 2", "bare");
    green_editing(dir, "step 2", "bare", "\n\nhistory:");
    red(dir, "step 3", "blank");
    green_editing(dir, "step 3", "blank", "\n\nhistory:    x\n\nhistory:   ");
    // Another case of the key.
    red(dir, "step 4", "upper");
    green_editing(dir, "step 4", "upper", "\n\nHistory: a reason");
    // A RED with a history line changes the test of step 5: superseded, not a history note.
    red(dir, "step 5", "five");
    code(dir, "five");
    commit(dir, "ABC-1 step 5 GREEN: five");
    edit_test(dir, "five");
    write(
        dir,
        "crates/tiny/tests/six.rs",
        "#[test]\nfn six() {\n    assert_eq!(tiny::six(), 1);\n}\n",
    );
    commit(
        dir,
        "ABC-1 step 6 RED: six is missing\n\nhistory: the RED may not carry it",
    );
    code(dir, "six");
    commit(dir, "ABC-1 step 6 GREEN: six");
    // The line in the subject counts.
    red(dir, "step 7", "seven");
    code(dir, "seven");
    commit(dir, "ABC-1 step 7 GREEN: seven");
    edit_test(dir, "seven");
    commit(dir, "history: seven was wrong");
    // Two lines: the first reason counts.
    red(dir, "step 8", "eight");
    green_editing(
        dir,
        "step 8",
        "eight",
        "\n\nhistory: the first reason\nhistory: the second reason",
    );

    let out = history(dir);
    assert_ne!(outcome(&out, "step 1"), "tests-changed", "{out}");
    assert_eq!(
        notes(&out, "step 1"),
        ["history by step 1: a good reason"],
        "{out}"
    );
    for label in ["step 2", "step 4"] {
        assert_eq!(outcome(&out, label), "tests-changed", "{label}\n{out}");
        assert!(
            !notes(&out, label).iter().any(|n| n.starts_with("history by")),
            "{label}\n{out}"
        );
    }
    // `history:    x` followed by an empty line: the first line with a reason counts.
    assert_ne!(outcome(&out, "step 3"), "tests-changed", "{out}");
    assert_eq!(notes(&out, "step 3"), ["history by step 3: x"], "{out}");
    assert!(
        notes(&out, "step 5")
            .iter()
            .any(|n| n.starts_with("superseded by step 6")),
        "{out}"
    );
    assert!(
        !notes(&out, "step 5")
            .iter()
            .any(|n| n.starts_with("history by")),
        "{out}"
    );
    assert!(
        notes(&out, "step 7")
            .iter()
            .any(|n| n.starts_with("history by ") && n.ends_with(": seven was wrong")),
        "{out}"
    );
    assert_eq!(
        notes(&out, "step 8"),
        ["history by step 8: the first reason"],
        "{out}"
    );
}
