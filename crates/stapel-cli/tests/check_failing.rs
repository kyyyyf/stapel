//! STP-42: `stapel check` names the failing tests of a failing suite.

mod common;

use common::{read, stapel};
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

/// A repository with a one-crate workspace, ticket ABC-1 and one passing step, all committed.
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

/// A `[check] cargo` stand-in: a whole-suite run (`--workspace`) prints the file `canned` and exits
/// with the code in the file `code`; every other run goes to the fake cargo.
fn canned_cargo(scripts: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(scripts.join("real")).unwrap();
    let real = common::fake_cargo(&scripts.join("real"));
    let path = scripts.join("cargo");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\ncase \"$*\" in\n*--workspace*)\n  cat {dir}/canned\n  exit $(cat {dir}/code)\n  ;;\nesac\nexec {real} \"$@\"\n",
            dir = scripts.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

/// Sets `[check] cargo` to `cargo` and commits it.
fn configure(dir: &Path, cargo: &str) {
    let text = read(dir, ".stapel/stapel.toml");
    let text: String = text
        .lines()
        .map(|l| {
            if l.starts_with("timeout_secs = ") {
                format!("{l}\ncargo = \"{cargo}\"\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: the check keys");
}

/// Runs `stapel check ABC-1 --level head` with the suite printing `output` and exiting `code`;
/// returns the lines of the output and the last record.
fn check_with(
    dir: &Path,
    scripts: &Path,
    output: &str,
    code: i32,
) -> (Vec<String>, serde_json::Value) {
    std::fs::write(scripts.join("canned"), output).unwrap();
    std::fs::write(scripts.join("code"), code.to_string()).unwrap();
    let out = stapel(dir)
        .args(["check", "ABC-1", "--level", "head"])
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    assert!(
        lines.iter().any(|l| l.starts_with("suite: ")),
        "no suite line: {text}"
    );
    (lines, last_record(dir))
}

/// The line after the suite line, if any.
fn after_suite(lines: &[String]) -> Option<&str> {
    let at = lines.iter().position(|l| l.starts_with("suite: "))?;
    lines.get(at + 1).map(String::as_str)
}

fn failing_lines(lines: &[String]) -> usize {
    lines.iter().filter(|l| l.starts_with("failing:")).count()
}

// AC-8: the failing names of a failing cargo suite, with the target; a doc test after its
// `Doc-tests` line; at most 20 distinct names and the rest counted; no line and no key otherwise.
#[test]
fn cargo_names_failing_tests() {
    let repo = repo_with_a_step();
    let dir = repo.path();
    let scripts = tempfile::tempdir().unwrap();
    configure(dir, &canned_cargo(scripts.path()));
    let s = scripts.path();

    // 3 failing tests in two targets.
    let two_targets = "\
   Compiling tiny v0.1.0
     Running unittests src/lib.rs (target/debug/deps/tiny-0123abcd)

running 2 tests
test alpha::first ... FAILED
test alpha::fine ... ok

failures:
    alpha::first

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/basic.rs (target/debug/deps/basic-4567ef01)

running 2 tests
test beta::second ... FAILED
test beta::third ... FAILED

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
    let (lines, record) = check_with(dir, s, two_targets, 101);
    assert_eq!(
        after_suite(&lines),
        Some("failing: src/lib.rs alpha::first, tests/basic.rs beta::second, tests/basic.rs beta::third"),
        "{lines:?}"
    );
    assert_eq!(
        record["suite"]["failing"],
        serde_json::json!([
            "src/lib.rs alpha::first",
            "tests/basic.rs beta::second",
            "tests/basic.rs beta::third"
        ]),
        "{record}"
    );
    assert_eq!(record["suite"]["failing_more"], 0, "{record}");
    assert_eq!(record["suite"]["failed"], 3, "{record}");

    // A failing doc test after a `Doc-tests tiny` line.
    let doc = "\
     Running unittests src/lib.rs (target/debug/deps/tiny-0123abcd)

running 1 test
test fine ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests tiny

running 1 test
test src/lib.rs - one (line 3) ... FAILED

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
    let (lines, record) = check_with(dir, s, doc, 101);
    assert_eq!(
        after_suite(&lines),
        Some("failing: doc tiny src/lib.rs - one (line 3)"),
        "{lines:?}"
    );
    assert_eq!(
        record["suite"]["failing"],
        serde_json::json!(["doc tiny src/lib.rs - one (line 3)"]),
        "{record}"
    );
    assert_eq!(record["suite"]["failing_more"], 0, "{record}");

    // 25 failing: the first 20 and `and 5 more`.
    let mut many = String::from(
        "     Running unittests src/lib.rs (target/debug/deps/tiny-0123abcd)\n\nrunning 25 tests\n",
    );
    for i in 1..=25 {
        many.push_str(&format!("test t{i:02} ... FAILED\n"));
    }
    many.push_str("\ntest result: FAILED. 0 passed; 25 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n");
    let first_20: Vec<String> = (1..=20).map(|i| format!("src/lib.rs t{i:02}")).collect();
    let (lines, record) = check_with(dir, s, &many, 101);
    assert_eq!(
        after_suite(&lines),
        Some(format!("failing: {} and 5 more", first_20.join(", ")).as_str()),
        "{lines:?}"
    );
    assert_eq!(record["suite"]["failing"], serde_json::json!(first_20), "{record}");
    assert_eq!(record["suite"]["failing_more"], 5, "{record}");

    // One name printed twice is one name.
    let twice = "\
     Running unittests src/lib.rs (target/debug/deps/tiny-0123abcd)

running 1 test
test same ... FAILED

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/tiny-0123abcd)

running 1 test
test same ... FAILED
";
    let (lines, record) = check_with(dir, s, twice, 101);
    assert_eq!(after_suite(&lines), Some("failing: src/lib.rs same"), "{lines:?}");
    assert_eq!(record["suite"]["failing"], serde_json::json!(["src/lib.rs same"]), "{record}");
    assert_eq!(record["suite"]["failing_more"], 0, "{record}");

    // A compile failure names no test: no line, no key.
    let broken = "error: could not compile `tiny` (lib) due to 1 previous error\n";
    let (lines, record) = check_with(dir, s, broken, 101);
    assert_eq!(failing_lines(&lines), 0, "{lines:?}");
    assert!(record["suite"].get("failing").is_none(), "{record}");
    assert!(record["suite"].get("failing_more").is_none(), "{record}");

    // A passing suite: no line, no key.
    let passing = "\
     Running unittests src/lib.rs (target/debug/deps/tiny-0123abcd)

running 1 test
test fine ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";
    let (lines, record) = check_with(dir, s, passing, 0);
    assert_eq!(failing_lines(&lines), 0, "{lines:?}");
    assert_eq!(record["suite"]["outcome"], "pass", "{record}");
    assert!(record["suite"].get("failing").is_none(), "{record}");
    assert!(record["suite"].get("failing_more").is_none(), "{record}");
}
