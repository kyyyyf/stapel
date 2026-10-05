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
