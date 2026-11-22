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

// AC-10: the RED run of a step whose test a history commit changed takes the test file of the newest
// history commit on the RED commit's code.
#[test]
fn a_weakened_test_is_no_red() {
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
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    stapel(dir).args(["new", "First"]).assert().success();
    let scripts = TempDir::new().unwrap();
    let fake = common::fake_cargo(scripts.path());
    assert!(std::fs::metadata(&fake).unwrap().permissions().mode() & 0o100 != 0);
    let toml: String = read(dir, ".stapel/stapel.toml")
        .lines()
        .map(|l| {
            if l.starts_with("timeout_secs = ") {
                format!("timeout_secs = 3600\ncargo = \"{fake}\"\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    write(dir, ".stapel/stapel.toml", &toml);
    commit(dir, "initial");

    let test = |marker: &str, extra: &str| {
        format!("#[test]\nfn t() {{\n    // fake: {marker}\n{extra}}}\n")
    };
    let add = |dir: &Path, text: &str| {
        let lib = read(dir, "crates/tiny/src/lib.rs");
        write(dir, "crates/tiny/src/lib.rs", &format!("{lib}{text}\n"));
    };
    // Step 1: the history GREEN weakens the test: it passes on the RED code.
    write(dir, "crates/tiny/tests/s1.rs", &test("needs pub fn two", ""));
    commit(dir, "ABC-1 step 1 RED: two is missing");
    add(dir, "pub fn two() {}");
    write(dir, "crates/tiny/tests/s1.rs", &test("needs pub fn one", "    let _weaker = 1;\n"));
    commit(dir, "ABC-1 step 1 GREEN: two\n\nhistory: the first check was too strict");
    // Step 2: the history GREEN fixes a typo of the test; the test still fails on the RED code.
    write(dir, "crates/tiny/tests/s2.rs", &test("needs pub fn three", ""));
    commit(dir, "ABC-1 step 2 RED: three is missing");
    add(dir, "pub fn three() {}");
    write(
        dir,
        "crates/tiny/tests/s2.rs",
        &test("needs pub fn three", "    let _fixed = 1;\n"),
    );
    commit(dir, "ABC-1 step 2 GREEN: three\n\nhistory: a typo in a comment");
    // Step 3: the history GREEN's test does not compile on the RED code.
    write(dir, "crates/tiny/tests/s3.rs", &test("needs pub fn four", ""));
    commit(dir, "ABC-1 step 3 RED: four is missing");
    add(dir, "pub fn four() {}");
    write(dir, "crates/tiny/tests/s3.rs", &test("item pub fn four", "    let _item = 1;\n"));
    commit(dir, "ABC-1 step 3 GREEN: four\n\nhistory: the test names the item");

    let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(outcome(&text, "step 1"), "no-red", "{text}");
    assert_eq!(outcome(&text, "step 2"), "pass", "{text}");
    assert_eq!(outcome(&text, "step 3"), "pass", "{text}");
}

// AC-11 (cargo): a history commit that changes a build input blocks no step; a later commit without
// the line still blocks, and the text of the history commit is what a later change is compared with.
#[test]
fn build_input_with_a_reason() {
    let helper = |text: &str| format!("pub fn shared() -> u32 {{\n    {text}\n}}\n");
    let outcomes = |tail: &dyn Fn(&Path)| {
        let repo = cargo_repo();
        let dir = repo.path();
        red(dir, "step 1", "one");
        code(dir, "one");
        commit(dir, "ABC-1 step 1 GREEN: one");
        write(dir, "crates/tiny/tests/common/mod.rs", &helper("1"));
        commit(dir, "ABC-1: a shared helper\n\nhistory: the tests share one helper");
        red(dir, "step 2", "two");
        code(dir, "two");
        commit(dir, "ABC-1 step 2 GREEN: two");
        tail(dir);
        let text = history(dir);
        (text, git(dir, &["rev-parse", "--short=7", "HEAD"]))
    };
    // Only the history commit: both steps wait for a run.
    let (text, _) = outcomes(&|_| {});
    assert_eq!(outcome(&text, "step 1"), "needs-run", "{text}");
    assert_eq!(outcome(&text, "step 2"), "needs-run", "{text}");
    // A later commit without the line changes a manifest: it blocks, and it is the one named.
    let (text, sha) = outcomes(&|dir| {
        let manifest = read(dir, "crates/tiny/Cargo.toml");
        write(
            dir,
            "crates/tiny/Cargo.toml",
            &format!("{manifest}\n[lib]\nname = \"tiny\"\n"),
        );
        commit(dir, "ABC-1: a lib table");
    });
    for label in ["step 1", "step 2"] {
        assert_eq!(outcome(&text, label), "unverified", "{label}\n{text}");
        assert!(
            text.contains(&format!(
                "{label}: unverified: build-input-changed: crates/tiny/Cargo.toml at {}",
                sha.trim()
            )),
            "{label}\n{text}"
        );
    }
    // A later commit without the line changes the helper again: it blocks.
    let (text, sha) = outcomes(&|dir| {
        write(dir, "crates/tiny/tests/common/mod.rs", &helper("2"));
        commit(dir, "ABC-1: the helper changes again");
    });
    assert_eq!(outcome(&text, "step 2"), "unverified", "{text}");
    assert!(
        text.contains(&format!(
            "step 2: unverified: build-input-changed: crates/tiny/tests/common/mod.rs at {}",
            sha.trim()
        )),
        "{text}"
    );
}

// AC-11 (junit): history commits that change an input, the configuration and the command's script
// block no step and no run, in both modes; a later commit without the line still blocks.
#[test]
fn junit_input_with_a_reason() {
    use std::os::unix::fs::PermissionsExt;
    let setup = || {
        let repo = common::junit_repo();
        let dir = repo.dir();
        let toml = read(dir, ".stapel/stapel.toml");
        let stand_in = toml
            .lines()
            .find(|l| l.starts_with("command = "))
            .unwrap()
            .trim_start_matches("command = \"sh ")
            .trim_end_matches(" {file}\"")
            .to_string();
        write(dir, "run.sh", &format!("exec sh '{stand_in}' \"$1\"\n"));
        std::fs::set_permissions(dir.join("run.sh"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let toml: String = toml
            .lines()
            .map(|l| {
                if l.starts_with("command = ") {
                    "command = \"./run.sh {file}\"\ninputs = [\"jest.config.js\"]\n".to_string()
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        write(dir, ".stapel/stapel.toml", &toml);
        commit(dir, "setup: a repository script and an input");
        repo
    };
    let step = |dir: &Path, n: u32, name: &str, between: &dyn Fn(&Path)| {
        write(
            dir,
            &format!("__tests__/{name}.test.ts"),
            &format!("it(\"{name}\", () => {{ /* needs src/{name}.ts */ }});\n"),
        );
        commit(dir, &format!("ABC-1 step {n} RED: {name}"));
        between(dir);
        write(dir, &format!("src/{name}.ts"), "export const v = 1;\n");
        commit(dir, &format!("ABC-1 step {n} GREEN: {name}"));
    };
    let say = "\n\nhistory: this change was meant";
    let both = |text: &str| {
        for label in ["step 1", "step 2"] {
            assert_ne!(outcome(text, label), "unverified", "{label}\n{text}");
            assert!(!text.contains("build-input-changed"), "{text}");
        }
    };

    // History commits: an input, the configuration and the script. No step is blocked.
    let repo = setup();
    let dir = repo.dir();
    step(dir, 1, "one", &|dir| {
        write(dir, "jest.config.js", "module.exports = {};\n");
        commit(dir, &format!("ABC-1: a jest config{say}"));
        let toml = read(dir, ".stapel/stapel.toml");
        write(
            dir,
            ".stapel/stapel.toml",
            &toml.replace("timeout_secs = 60", "timeout_secs = 61"),
        );
        commit(dir, &format!("ABC-1: a longer timeout{say}"));
    });
    step(dir, 2, "two", &|dir| {
        let script = read(dir, "run.sh");
        write(dir, "run.sh", &format!("# the same call\n{script}"));
        commit(dir, &format!("ABC-1: a comment in the script{say}"));
    });
    let out = history(dir);
    both(&out);
    assert_eq!(outcome(&out, "step 1"), "needs-run", "{out}");
    assert_eq!(outcome(&out, "step 2"), "needs-run", "{out}");
    let full = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
    let full = format!(
        "{}{}",
        String::from_utf8_lossy(&full.stdout),
        String::from_utf8_lossy(&full.stderr)
    );
    both(&full);
    assert_eq!(outcome(&full, "step 1"), "pass", "{full}");
    assert_eq!(outcome(&full, "step 2"), "pass", "{full}");

    // A later commit without the line changes the input again before the GREEN: it blocks.
    let repo = setup();
    let dir = repo.dir();
    step(dir, 1, "one", &|dir| {
        write(dir, "jest.config.js", "module.exports = {};\n");
        commit(dir, &format!("ABC-1: a jest config{say}"));
        write(dir, "jest.config.js", "module.exports = { bail: 1 };\n");
        commit(dir, "ABC-1: a stricter jest config");
    });
    let sha = git(dir, &["log", "--format=%h", "-n", "1", "--grep", "stricter"]);
    let out = history(dir);
    assert!(
        out.contains(&format!(
            "step 1: unverified: build-input-changed: jest.config.js at {}",
            &sha.trim()[..7]
        )),
        "{out}"
    );
}

// AC-13, AC-14: the report lists the history commits oldest first, before the suite line (after the
// steps in `--history` mode); the record has `history` with full ids, reasons cut to 200 characters
// (not bytes), control bytes replaced in the printed line, and past 20 entries `history_truncated`.
#[test]
fn the_record_lists_the_reasons() {
    let repo = common::junit_repo();
    let dir = repo.dir();
    write(dir, "__tests__/one.test.ts", "it(\"one\", () => { /* needs src/one.ts */ });\n");
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/one.ts", "export const v = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one\n\nhistory: the first reason");
    let first = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let long: String = "ж".repeat(300);
    commit(dir, &format!("ABC-1: a long note\n\nhistory: {long}"));
    let second = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    commit(dir, "ABC-1: a plain note without a reason");
    commit(dir, "ABC-1: control bytes\n\nhistory: bad\x1b[31m text");
    let third = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let ids = [&first, &second, &third];
    let lines_of = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|l| l.starts_with("history: "))
            .map(String::from)
            .collect()
    };
    let position = |text: &str, prefix: &str| {
        text.lines()
            .position(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("no {prefix}\n{text}"))
    };
    let check = |args: &[&str]| {
        let out = stapel(dir).args(args).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let expected = |long_chars: usize| -> Vec<String> {
        vec![
            format!("history: {} the first reason", &first[..7]),
            format!("history: {} {}", &second[..7], "ж".repeat(long_chars)),
            format!("history: {} bad [31m text", &third[..7]),
        ]
    };
    let same = |text: &str| {
        assert!(!text.contains('\x1b'), "{text:?}");
        let got = lines_of(text);
        assert_eq!(got.len(), 3, "{text}");
        assert_eq!(got[0], expected(0)[0], "{text}");
        assert!(got[1].starts_with(&expected(200)[1]), "{text}");
        assert_eq!(got[2], expected(0)[2], "{text}");
    };

    // The full check: after the step lines, before the suite line.
    let full = check(&["check", "ABC-1"]);
    same(&full);
    let (step, hist, suite) = (
        position(&full, "step 1: "),
        position(&full, "history: "),
        position(&full, "suite: "),
    );
    assert!(step < hist && hist < suite, "{full}");
    // `--history`: after the step lines, before the result line.
    let only = check(&["check", "ABC-1", "--history"]);
    same(&only);
    let (step, hist, result) = (
        position(&only, "step 1: "),
        position(&only, "history: "),
        position(&only, "result: "),
    );
    assert!(step < hist && hist < result, "{only}");

    let last = |dir: &Path| -> serde_json::Value {
        let text = read(dir, ".stapel/tickets/ABC-1/runs.jsonl");
        let line = text
            .lines()
            .rfind(|l| l.contains("\"kind\":\"check\""))
            .unwrap();
        serde_json::from_str(line).unwrap()
    };
    let record = last(dir);
    let entries = record["history"].as_array().expect("history in the record");
    assert_eq!(entries.len(), 3, "{record}");
    for (entry, id) in entries.iter().zip(ids) {
        assert_eq!(entry["commit"], id.as_str(), "{record}");
        assert_eq!(entry.as_object().unwrap().len(), 2, "{record}");
    }
    assert_eq!(entries[0]["reason"], "the first reason", "{record}");
    assert_eq!(entries[1]["reason"], "ж".repeat(200).as_str(), "{record}");
    assert!(record.get("history_truncated").is_none(), "{record}");

    // Past 20 entries the rest are left out.
    for n in 0..18 {
        commit(dir, &format!("ABC-1: note {n}\n\nhistory: reason {n}"));
    }
    check(&["check", "ABC-1"]);
    let record = last(dir);
    let entries = record["history"].as_array().expect("history in the record");
    assert_eq!(entries.len(), 20, "{record}");
    assert_eq!(entries[0]["commit"], first.as_str(), "{record}");
    assert_eq!(record["history_truncated"], true, "{record}");
}

// AC-10 (review round 1, XR1-1, CR1-1): a step cached by an earlier passing check runs its RED again
// when a later history commit changed the text of its test.
#[test]
fn a_cached_step_runs_its_red_again() {
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
    let scripts = TempDir::new().unwrap();
    let fake = common::fake_cargo(scripts.path());
    let toml: String = read(dir, ".stapel/stapel.toml")
        .lines()
        .map(|l| {
            if l.starts_with("timeout_secs = ") {
                format!("timeout_secs = 3600\ncargo = \"{fake}\"\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    write(dir, ".stapel/stapel.toml", &toml);
    commit(dir, "initial");

    let test = |marker: &str, extra: &str| {
        format!("#[test]\nfn t() {{\n    // fake: {marker}\n{extra}}}\n")
    };
    write(dir, "crates/tiny/tests/s1.rs", &test("needs pub fn two", ""));
    commit(dir, "ABC-1 step 1 RED: two is missing");
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(dir, "crates/tiny/src/lib.rs", &format!("{lib}pub fn two() {{}}\n"));
    commit(dir, "ABC-1 step 1 GREEN: two");

    let run = |dir: &Path| {
        let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };
    let first = run(dir);
    assert_eq!(outcome(&first, "step 1"), "pass", "{first}");

    // A later fix commit weakens step 1's test: it passes on the RED code.
    write(
        dir,
        "crates/tiny/tests/s1.rs",
        &test("needs pub fn one", "    let _weaker = 1;\n"),
    );
    commit(
        dir,
        "ABC-1 fix: weaker test of step 1\n\nhistory: the first check was too strict",
    );
    let second = run(dir);
    assert_eq!(outcome(&second, "step 1"), "no-red", "{second}");
}

// AC-10 (review round 1, XR1-3): a test file shared with a later step's test: the RED run takes only
// the step's own tests from the history commit, so the later test that needs later code stays out.
// AC-9 (CR1-5): the note `history by <label>: <reason>` shows in a full check and under junit; a
// history commit that deletes a test is judged as today; a history commit that weakens the test of
// step 1 in a file shared with step 2 splices only that test into the RED run, so step 1 is `no-red`.
#[test]
fn the_note_in_full_and_junit() {
    let report = |dir: &Path| {
        let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };

    // The note in a full check, junit.
    let repo = common::junit_repo();
    let dir = repo.dir();
    write(
        dir,
        "__tests__/one.test.ts",
        "it(\"one\", () => { /* needs src/one.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/one.ts", "export const v = 1;\n");
    write(
        dir,
        "__tests__/one.test.ts",
        "it(\"one\", () => { /* note needs src/one.ts */ });\n",
    );
    commit(
        dir,
        "ABC-1 step 1 GREEN: one\n\nhistory: the comment was reworded",
    );
    let full = report(dir);
    assert_eq!(outcome(&full, "step 1"), "pass", "{full}");
    assert_eq!(
        notes(&full, "step 1"),
        ["history by step 1: the comment was reworded"],
        "{full}"
    );
    let out = history(dir);
    assert_eq!(
        notes(&out, "step 1"),
        ["history by step 1: the comment was reworded"],
        "{out}"
    );

    // A history commit that deletes a test is judged as today: no note.
    let repo = common::junit_repo();
    let dir = repo.dir();
    write(
        dir,
        "__tests__/two.test.ts",
        "it(\"two\", () => { /* needs src/two.ts */ });\nit(\"extra\", () => {});\n",
    );
    commit(dir, "ABC-1 step 1 RED: two");
    write(dir, "src/two.ts", "export const v = 2;\n");
    commit(dir, "ABC-1 step 1 GREEN: two");
    write(
        dir,
        "__tests__/two.test.ts",
        "it(\"two\", () => { /* needs src/two.ts */ });\n",
    );
    commit(dir, "ABC-1: drop a test\n\nhistory: the extra test was noise");
    let out = history(dir);
    assert!(notes(&out, "step 1").is_empty(), "{out}");
    let full = report(dir);
    assert!(notes(&full, "step 1").is_empty(), "{full}");
    assert!(!full.contains("history by "), "{full}");

    // A shared test file with two tests of two steps: the history commit weakens step 1's test.
    let repo = common::junit_repo();
    let dir = repo.dir();
    let file = "__tests__/shared.test.ts";
    let first = "it(\"a\", () => { /* needs src/one.ts */ });\n";
    let second = "it(\"b\", () => { /* needs src/two.ts */ });\n";
    write(dir, file, first);
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/one.ts", "export const v = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one");
    write(dir, file, &format!("{first}{second}"));
    commit(dir, "ABC-1 step 2 RED: two");
    write(dir, "src/two.ts", "export const v = 2;\n");
    commit(dir, "ABC-1 step 2 GREEN: two");
    write(dir, file, &format!("it(\"a\", () => {{}});\n{second}"));
    commit(
        dir,
        "ABC-1 fix: weaker test of step 1\n\nhistory: the first check was too strict",
    );
    let full = report(dir);
    assert_eq!(outcome(&full, "step 1"), "no-red", "{full}");
    assert_ne!(outcome(&full, "step 2"), "no-red", "{full}");
}

#[test]
fn later_tests_stay_out_of_the_red_run() {
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
    let scripts = TempDir::new().unwrap();
    let fake = common::fake_cargo(scripts.path());
    let toml: String = read(dir, ".stapel/stapel.toml")
        .lines()
        .map(|l| {
            if l.starts_with("timeout_secs = ") {
                format!("timeout_secs = 3600\ncargo = \"{fake}\"\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    write(dir, ".stapel/stapel.toml", &toml);
    commit(dir, "initial");

    let test = |name: &str, marker: &str, extra: &str| {
        format!("#[test]\nfn {name}() {{\n    // fake: {marker}\n{extra}}}\n")
    };
    let add = |dir: &Path, text: &str| {
        let lib = read(dir, "crates/tiny/src/lib.rs");
        write(dir, "crates/tiny/src/lib.rs", &format!("{lib}{text}\n"));
    };
    let file = "crates/tiny/tests/shared.rs";
    let first = test("a", "needs pub fn two", "");
    // Step 1.
    write(dir, file, &first);
    commit(dir, "ABC-1 step 1 RED: two is missing");
    add(dir, "pub fn two() {}");
    commit(dir, "ABC-1 step 1 GREEN: two");
    // Step 2: its test, in the same file, needs code that the RED of step 1 lacks.
    write(
        dir,
        file,
        &format!("{first}\n{}", test("b", "item pub fn later", "")),
    );
    commit(dir, "ABC-1 step 2 RED: later is missing");
    add(dir, "pub fn later() {}");
    commit(dir, "ABC-1 step 2 GREEN: later");
    // A history commit weakens the test of step 1 only.
    write(
        dir,
        file,
        &format!(
            "{}\n{}",
            test("a", "needs pub fn one", "    let _weaker = 1;\n"),
            test("b", "item pub fn later", "")
        ),
    );
    commit(
        dir,
        "ABC-1 fix: weaker test of step 1\n\nhistory: the first check was too strict",
    );

    let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(outcome(&text, "step 1"), "no-red", "{text}");
}
