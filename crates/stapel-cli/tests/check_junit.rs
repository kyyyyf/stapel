//! STP-28: `stapel check` under `[check] runner = "junit"`: step tests and the per-file run.

mod common;

use common::{JunitRepo, git_in, junit_repo, stapel};
use std::path::Path;

fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn commit(dir: &Path, message: &str) {
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "--allow-empty", "-m", message]);
}

fn stdout(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = stapel(dir).args(args).output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code().unwrap_or(-1), text)
}

/// One full `stapel check` at a time on the machine.
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn check(repo: &JunitRepo) -> (i32, String) {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    stdout(repo.dir(), &["check", "ABC-1"])
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

// AC-9: the step tests of a junit RED are its added or changed test calls.
#[test]
fn step_tests_from_the_red_commit() {
    let repo = junit_repo();
    let dir = repo.dir();
    // Adds `fresh`, retitles `keeps`, and changes a helper line; `base` keeps its text.
    write(
        dir,
        "__tests__/base.test.ts",
        "const helper = 2;\nit(\"base\", () => {});\nit(\"keeps going\", () => { /* needs src/index.ts */ });\nit(\"fresh\", () => { /* needs src/fresh.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: fresh and a new title");
    write(dir, "src/fresh.ts", "export const fresh = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: fresh");
    // Only a helper line changes.
    write(
        dir,
        "__tests__/base.test.ts",
        "const helper = 3;\nit(\"base\", () => {});\nit(\"keeps going\", () => { /* needs src/index.ts */ });\nit(\"fresh\", () => { /* needs src/fresh.ts */ });\n",
    );
    commit(dir, "ABC-1 step 2 RED: a helper line");
    commit(dir, "ABC-1 step 2 GREEN: nothing");

    let (code, text) = stdout(dir, &["check", "ABC-1", "--list"]);
    assert_eq!(code, 0, "{text}");
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|l| *l == "step 1").expect(&text);
    let step_one: Vec<&str> = lines[at..]
        .iter()
        .take_while(|l| **l == "step 1" || l.starts_with(' '))
        .copied()
        .collect();
    assert!(step_one.contains(&"  tests: 2"), "{text}");
    assert!(
        step_one.contains(&"    __tests__/base.test.ts::fresh"),
        "{text}"
    );
    assert!(
        step_one.contains(&"    __tests__/base.test.ts::keeps going"),
        "{text}"
    );
    assert!(
        !step_one.iter().any(|l| l.ends_with("::base")),
        "an unchanged test call is no step test: {text}"
    );
    let at = lines.iter().position(|l| *l == "step 2").expect(&text);
    assert!(lines[at..].contains(&"  tests: 0"), "{text}");

    let (code, text) = stdout(dir, &["check", "ABC-1", "--history"]);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 2"), "no-tests", "{text}");
}

// AC-10, AC-11: RED fails, GREEN and HEAD pass; one run per file and commit.
#[test]
fn red_to_green_passes() {
    let repo = junit_repo();
    let dir = repo.dir();
    write(
        dir,
        "__tests__/my file.test.ts",
        "it(\"adds\", () => { /* needs src/add.ts */ });\nit(\"doubles\", () => { /* needs src/double.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: add and double");
    write(dir, "src/add.ts", "export const add = 1;\n");
    write(dir, "src/double.ts", "export const double = 2;\n");
    commit(dir, "ABC-1 step 1 GREEN: add and double");

    let (_, text) = check(&repo);
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert!(
        text.lines()
            .any(|l| l.starts_with("step 1: pass") && l.contains("2 tests")),
        "{text}"
    );
    // The stand-in saw one word for the path, `PATH`, `HOME`, the jest-junit variable and the
    // `node_modules` link, or it would have left no report; it ran once per commit.
    let runs: Vec<String> = repo
        .runs()
        .into_iter()
        .filter(|l| l.ends_with(" __tests__/my file.test.ts"))
        .collect();
    assert_eq!(runs.len(), 3, "{:?}", repo.runs());
    let head = git_in(dir, &["rev-parse", "HEAD"]);
    assert!(runs[2].starts_with(head.trim()), "{runs:?}");
    assert!(
        git_in(dir, &["status", "--porcelain"])
            .lines()
            .all(|l| l.contains(".stapel/tickets/")),
        "the check leaves the tree alone"
    );
}

// AC-11, AC-12, AC-33: not-green, no-red, ignored at RED, and removed before any HEAD run.
#[test]
fn not_green_and_removed() {
    let repo = junit_repo();
    let dir = repo.dir();
    // Step 1: GREEN still failing; a later commit repairs it.
    write(
        dir,
        "__tests__/green.test.ts",
        "it(\"needs code\", () => { /* needs src/later.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: needs code");
    commit(dir, "ABC-1 step 1 GREEN: forgot the code");
    write(dir, "src/later.ts", "export const later = 1;\n");
    commit(dir, "ABC-1: the code, late");
    // Step 2: passes at RED.
    write(
        dir,
        "__tests__/steady.test.ts",
        "it(\"steady\", () => {});\n",
    );
    commit(dir, "ABC-1 step 2 RED: a test that passes");
    commit(dir, "ABC-1 step 2 GREEN: nothing to do");
    // Step 3: skipped at RED.
    write(
        dir,
        "__tests__/parked.test.ts",
        "it.skip(\"parked\", () => {});\n",
    );
    commit(dir, "ABC-1 step 3 RED: a parked test");
    commit(dir, "ABC-1 step 3 GREEN: nothing");
    // Step 4: the test call is deleted later.
    write(
        dir,
        "__tests__/gone.test.ts",
        "it(\"gone\", () => { /* needs src/gone.ts */ });\n",
    );
    commit(dir, "ABC-1 step 4 RED: gone");
    write(dir, "src/gone.ts", "export const gone = 1;\n");
    commit(dir, "ABC-1 step 4 GREEN: gone");
    write(dir, "__tests__/gone.test.ts", "// no test left\n");
    commit(dir, "ABC-1: the test call is deleted");
    // Step 5: the whole file is deleted later.
    write(
        dir,
        "__tests__/vanish.test.ts",
        "it(\"vanish\", () => { /* needs src/vanish.ts */ });\n",
    );
    commit(dir, "ABC-1 step 5 RED: vanish");
    write(dir, "src/vanish.ts", "export const vanish = 1;\n");
    commit(dir, "ABC-1 step 5 GREEN: vanish");
    std::fs::remove_file(dir.join("__tests__/vanish.test.ts")).unwrap();
    commit(dir, "ABC-1: the file is deleted");

    let (code, text) = check(&repo);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "not-green", "{text}");
    assert!(text.contains("__tests__/green.test.ts::needs code"), "{text}");
    assert_eq!(outcome(&text, "step 2"), "no-red", "{text}");
    assert!(text.contains("__tests__/steady.test.ts::steady"), "{text}");
    assert_eq!(outcome(&text, "step 3"), "unverified", "{text}");
    assert!(text.contains("ignored at RED"), "{text}");
    assert_eq!(outcome(&text, "step 4"), "removed", "{text}");
    assert_eq!(outcome(&text, "step 5"), "removed", "{text}");
    let head = git_in(dir, &["rev-parse", "HEAD"]);
    let runs = repo.runs();
    for file in ["gone", "vanish"] {
        let path = format!("__tests__/{file}.test.ts");
        let of_file: Vec<&String> = runs.iter().filter(|l| l.ends_with(&path)).collect();
        assert_eq!(of_file.len(), 2, "RED and GREEN only: {runs:?}");
        assert!(
            of_file.iter().all(|l| !l.starts_with(head.trim())),
            "no run at HEAD for {path}: {runs:?}"
        );
    }
}
