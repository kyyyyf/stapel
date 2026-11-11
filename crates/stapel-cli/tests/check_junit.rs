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

const NOT_LOADED: &str = include_str!("../../stapel-core/tests/data/junit/b2.xml");
const NO_SUITE: &str = include_str!("../../stapel-core/tests/data/junit/none.xml");
const PASSED: &str = "<testsuites><testsuite name=\"s\"><testcase classname=\"\" name=\" one\"/></testsuite></testsuites>\n";
const FAILED: &str = "<testsuites><testsuite name=\"s\"><testcase classname=\"\" name=\" one\"><failure message=\"x\"/></testcase></testsuite></testsuites>\n";
const TWO_SUITES: &str = "<testsuites><testsuite name=\"a\"><testcase classname=\"\" name=\" one\"/></testsuite><testsuite name=\"b\"><testcase classname=\"\" name=\" one\"/></testsuite></testsuites>\n";
const TWO_CASES: &str = "<testsuites><testsuite name=\"s\"><testcase classname=\"\" name=\" one\"/><testcase classname=\"\" name=\" one\"/></testsuite></testsuites>\n";
const OTHER_CASE: &str = "<testsuites><testsuite name=\"s\"><testcase classname=\"\" name=\" other\"/></testsuite></testsuites>\n";

/// A junit ticket with one step `__tests__/x.test.ts::one` whose Per-file run writes `red` and exits
/// with `red_exit` while `src/ok.ts` is missing, and writes `green` and exits with `green_exit` after
/// the GREEN commit adds it. Returns the exit code and the text of `stapel check ABC-1`.
fn run_case(red: &str, red_exit: i32, green: &str, green_exit: i32) -> (i32, String) {
    let repo = junit_repo();
    let dir = repo.dir();
    let tools = repo.tools.path();
    write_tool(tools, "red.xml", red);
    write_tool(tools, "green.xml", green);
    let script = tools.join("shaped.sh");
    std::fs::write(
        &script,
        format!(
            "if [ -e src/ok.ts ]; then cat '{t}/green.xml' > junit.xml; exit {g}; fi\ncat '{t}/red.xml' > junit.xml\nexit {r}\n",
            t = tools.display(),
            g = green_exit,
            r = red_exit
        ),
    )
    .unwrap();
    let path = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let text: String = text
        .lines()
        .map(|l| {
            if l.starts_with("command = ") {
                format!("command = \"sh {} {{file}}\"\n", script.display())
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    std::fs::write(&path, text).unwrap();
    commit(dir, "ABC-1: a shaped run");
    write(dir, "__tests__/x.test.ts", "it(\"one\", () => {});\n");
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one");
    check(&repo)
}

fn write_tool(tools: &Path, name: &str, text: &str) {
    std::fs::write(tools.join(name), text).unwrap();
}

// AC-15: a file that does not load fails its step tests as a compile error.
#[test]
fn file_that_does_not_load_fails() {
    // The real b2.xml shape: two cases of classname `Test suite failed to run`, exit 1.
    let (code, text) = run_case(NOT_LOADED, 1, PASSED, 0);
    assert_eq!(code, 0, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert!(text.contains("RED 0 assert, 1 compile"), "{text}");
    // The same at GREEN.
    let (code, text) = run_case(NOT_LOADED, 1, NOT_LOADED, 1);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "not-green", "{text}");
    assert!(text.contains("__tests__/x.test.ts::one"), "{text}");
}

// AC-16, AC-17, AC-18: a report of the wrong shape is unverified, with the reason.
#[test]
fn report_shape_is_unverified() {
    let step = |text: &str| -> String {
        assert_eq!(outcome(text, "step 1"), "unverified", "{text}");
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap()
            .to_string()
    };
    // No suite (the real none.xml), whatever the exit code.
    for exit in [1, 0] {
        let (code, text) = run_case(NO_SUITE, exit, PASSED, 0);
        assert_eq!(code, 1, "{text}");
        assert!(
            step(&text).contains("holds no test suite"),
            "exit {exit}: {text}"
        );
    }
    // Two suites: the count is in the reason.
    let (_, text) = run_case(TWO_SUITES, 0, PASSED, 0);
    assert!(step(&text).contains("2 test suites"), "{text}");
    // A name with two test cases, and a name with none: the name and the count.
    let (_, text) = run_case(TWO_CASES, 0, PASSED, 0);
    let line = step(&text);
    assert!(
        line.contains("__tests__/x.test.ts") && line.contains("one") && line.contains("2 test cases"),
        "{text}"
    );
    let (_, text) = run_case(OTHER_CASE, 0, PASSED, 0);
    let line = step(&text);
    assert!(
        line.contains("one") && line.contains("0 test cases"),
        "{text}"
    );
}

// AC-19: the exit code must agree with the failures of the report.
#[test]
fn exit_code_must_agree() {
    let step = |text: &str| -> String {
        assert_eq!(outcome(text, "step 1"), "unverified", "{text}");
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap()
            .to_string()
    };
    // Exit 0 with a failed test case.
    let (_, text) = run_case(FAILED, 1, FAILED, 0);
    let line = step(&text);
    assert!(
        line.contains("exit code 0") && line.contains("1 failed test case"),
        "{text}"
    );
    // Exit 1 with test cases none of which failed.
    let (_, text) = run_case(PASSED, 1, PASSED, 0);
    let line = step(&text);
    assert!(
        line.contains("exit code 1") && line.contains("0 failed test cases"),
        "{text}"
    );
}

/// A junit ticket with one step `__tests__/x.test.ts::one` (RED and GREEN commits) whose command is
/// `command` with `{script}` replaced by a shell script holding `script`. `old` is a report committed
/// before the step; `timeout` replaces `timeout_secs`. Returns the exit code and the text of check.
fn fault_case(command: &str, script: &str, old: Option<&str>, timeout: u32) -> (i32, String) {
    let repo = junit_repo();
    let dir = repo.dir();
    let path = repo.tools.path().join("fault.sh");
    std::fs::write(&path, script).unwrap();
    let config = dir.join(".stapel/stapel.toml");
    let text: String = std::fs::read_to_string(&config)
        .unwrap()
        .lines()
        .map(|l| {
            if l.starts_with("command = ") {
                format!(
                    "command = \"{}\"\n",
                    command.replace("{script}", &path.display().to_string())
                )
            } else if l.starts_with("timeout_secs") {
                format!("timeout_secs = {timeout}\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    std::fs::write(&config, text).unwrap();
    if let Some(old) = old {
        write(dir, "junit.xml", old);
    }
    commit(dir, "ABC-1: a faulty run");
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one");
    check(&repo)
}

// AC-20, AC-21: a run that cannot start, times out, dies, or leaves no usable report is unverified.
#[test]
fn run_faults_are_unverified() {
    let reason = |(code, text): (i32, String), needle: &str| {
        assert_eq!(code, 1, "{text}");
        assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
        let line = text.lines().find(|l| l.starts_with("step 1: ")).unwrap();
        assert!(line.contains(needle), "wanted {needle:?} in {line:?}");
    };
    let sh = "sh {script} {file}";
    // No program.
    reason(
        fault_case("/nonexistent/prog {file}", "", None, 60),
        "did not start",
    );
    // A sleeping stand-in meets the 10 s limit.
    reason(fault_case(sh, "sleep 60\n", None, 10), "time limit of 10 s");
    // A signal with no report, and with a readable one-suite report: the signal is the reason.
    reason(fault_case(sh, "kill -9 $$\n", None, 60), "signal");
    let good = format!("printf '%s' '{}' > junit.xml\nkill -9 $$\n", PASSED.trim());
    reason(fault_case(sh, &good, None, 60), "signal");
    // No report; an old committed report is removed, so it is no report either.
    reason(fault_case(sh, "exit 0\n", None, 60), "no report");
    reason(fault_case(sh, "exit 0\n", Some(PASSED), 60), "no report");
    // 17 MiB.
    let big = "head -c 17825792 /dev/zero | tr '\\0' ' ' > junit.xml\nexit 0\n";
    reason(fault_case(sh, big, None, 60), "16 MiB");
    // A refused report.
    reason(
        fault_case(sh, "echo '<testsuites>' > junit.xml\nexit 0\n", None, 60),
        "report of",
    );
}

// AC-22: `.only` in a test file of the step at RED or GREEN: unverified, no command runs for it.
#[test]
fn only_call_is_unverified() {
    let only = |repo: &JunitRepo, text: &str, word: &str| {
        let (code, out) = check(repo);
        assert_eq!(code, 1, "{out}");
        assert_eq!(outcome(&out, "step 1"), "unverified", "{out}");
        let line = out.lines().find(|l| l.starts_with("step 1: ")).unwrap();
        assert!(
            line.contains("__tests__/x.test.ts") && line.contains(".only") && line.contains(word),
            "{text}: {line}"
        );
        assert!(
            repo.runs()
                .iter()
                .all(|l| !l.ends_with("__tests__/x.test.ts")),
            "no command runs for the file: {:?}",
            repo.runs()
        );
    };
    // `test.only` at RED.
    let repo = junit_repo();
    let dir = repo.dir();
    write(
        dir,
        "__tests__/x.test.ts",
        "test.only(\"one\", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one");
    only(&repo, "test.only at RED", "RED");
    // `describe.only` added at GREEN.
    let repo = junit_repo();
    let dir = repo.dir();
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok.ts */ });\ndescribe.only(\"g\", () => {});\n",
    );
    commit(dir, "ABC-1 step 1 GREEN: one");
    only(&repo, "describe.only at GREEN", "GREEN");
}

/// A junit repository with one step `__tests__/x.test.ts::one`. `setup` runs before the first commit
/// that names the ticket; `green` adds files to the GREEN commit.
fn step_repo(setup: impl Fn(&Path), green: impl Fn(&Path)) -> JunitRepo {
    let repo = junit_repo();
    let dir = repo.dir();
    setup(dir);
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    green(dir);
    commit(dir, "ABC-1 step 1 GREEN: one");
    repo
}

/// The line of `step 1` in the text of a check.
fn step_one(text: &str) -> &str {
    text.lines()
        .find(|l| l.starts_with("step 1: "))
        .unwrap_or_else(|| panic!("no step 1 line:\n{text}"))
}

// AC-23: no `node_modules` in the working tree: the steps that run are unverified with the reason.
#[test]
fn node_modules_missing_is_unverified() {
    let repo = step_repo(|_| {}, |_| {});
    std::fs::remove_dir_all(repo.dir().join("node_modules")).unwrap();
    let (code, text) = check(&repo);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    assert!(
        step_one(&text).ends_with("unverified: node_modules is missing in the working tree"),
        "{text}"
    );
    assert!(repo.runs().is_empty(), "{:?}", repo.runs());
}

// AC-24: a GREEN that changes `package.json`: `build-input-changed`, no command runs.
#[test]
fn build_input_change_is_unverified() {
    let repo = step_repo(
        |_| {},
        |dir| write(dir, "package.json", "{\"name\": \"x\"}\n"),
    );
    let sha = git_in(repo.dir(), &["rev-parse", "HEAD"]);
    let (code, text) = check(&repo);
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        step_one(&text),
        format!(
            "step 1: unverified: build-input-changed: package.json at {}",
            &sha.trim()[..7]
        ),
        "{text}"
    );
    assert!(repo.runs().is_empty(), "{:?}", repo.runs());
}

// AC-24: a merge counts by its first-parent diff; the step before it passes, the one after reads
// `build-input-changed` at the merge.
#[test]
fn merge_brings_a_build_input() {
    let repo = junit_repo();
    let dir = repo.dir();
    let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git_in(dir, &["checkout", "-q", "-b", "side"]);
    write(dir, "jest.config.js", "module.exports = {};\n");
    commit(dir, "main: a jest config");
    git_in(dir, &["checkout", "-q", main.trim()]);
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one");
    git_in(
        dir,
        &["merge", "-q", "--no-ff", "-m", "ABC-1: merge main", "side"],
    );
    let merge = git_in(dir, &["rev-parse", "HEAD"]);
    write(
        dir,
        "__tests__/y.test.ts",
        "it(\"two\", () => { /* needs src/two.ts */ });\n",
    );
    commit(dir, "ABC-1 step 2 RED: two");
    write(dir, "src/two.ts", "export const two = 2;\n");
    commit(dir, "ABC-1 step 2 GREEN: two");
    let (code, text) = check(&repo);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert_eq!(
        text.lines().find(|l| l.starts_with("step 2: ")).unwrap(),
        format!(
            "step 2: unverified: build-input-changed: jest.config.js at {}",
            &merge.trim()[..7]
        ),
        "{text}"
    );
    assert!(
        repo.runs().iter().all(|l| !l.ends_with("y.test.ts")),
        "{:?}",
        repo.runs()
    );
}

/// The path of the stand-in that `junit_repo` configured, read from its `command`.
fn repo_stand_in(dir: &Path) -> String {
    let text = std::fs::read_to_string(dir.join(".stapel/stapel.toml")).unwrap();
    let line = text.lines().find(|l| l.starts_with("command = ")).unwrap();
    line.trim_start_matches("command = \"sh ")
        .trim_end_matches(" {file}\"")
        .to_string()
}

// AC-25: a GREEN that changes `.stapel/stapel.toml`, or the repository script that `command` starts.
#[test]
fn config_change_is_unverified() {
    // The configuration.
    let repo = step_repo(
        |_| {},
        |dir| {
            let path = dir.join(".stapel/stapel.toml");
            let text = std::fs::read_to_string(&path).unwrap();
            std::fs::write(&path, text.replace("timeout_secs = 60", "timeout_secs = 61")).unwrap();
        },
    );
    let sha = git_in(repo.dir(), &["rev-parse", "HEAD"]);
    let (code, text) = check(&repo);
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        step_one(&text),
        format!(
            "step 1: unverified: build-input-changed: .stapel/stapel.toml at {}",
            &sha.trim()[..7]
        ),
        "{text}"
    );
    assert!(repo.runs().is_empty(), "{:?}", repo.runs());
    // A script of the repository.
    let repo = step_repo(
        |dir| {
            let stand_in = repo_stand_in(dir);
            write(dir, "run.sh", &format!("exec sh '{stand_in}' \"$1\"\n"));
            let path = dir.join(".stapel/stapel.toml");
            let text: String = std::fs::read_to_string(&path)
                .unwrap()
                .lines()
                .map(|l| {
                    if l.starts_with("command = ") {
                        "command = \"./run.sh {file}\"\n".to_string()
                    } else {
                        format!("{l}\n")
                    }
                })
                .collect();
            std::fs::write(&path, text).unwrap();
            commit(dir, "setup: a repository script");
        },
        |dir| write(dir, "run.sh", "exit 0\n"),
    );
    let sha = git_in(repo.dir(), &["rev-parse", "HEAD"]);
    let (code, text) = check(&repo);
    assert_eq!(code, 1, "{text}");
    assert_eq!(
        step_one(&text),
        format!(
            "step 1: unverified: build-input-changed: run.sh at {}",
            &sha.trim()[..7]
        ),
        "{text}"
    );
    assert!(repo.runs().is_empty(), "{:?}", repo.runs());
}

// AC-36: under junit a path of `docs/` that matches a test or input glob makes the check stale.
#[test]
fn docs_path_matching_a_glob_is_stale() {
    let line_after = |rel: &str| {
        let repo = step_repo(|_| {}, |_| {});
        let (code, text) = check(&repo);
        assert_eq!(code, 0, "{text}");
        let pass = format!(
            "check: pass at {}",
            &git_in(repo.dir(), &["rev-parse", "HEAD"]).trim()[..7]
        );
        write(repo.dir(), rel, "x\n");
        commit(repo.dir(), "docs: a change");
        let (_, status) = stdout(repo.dir(), &["status", "ABC-1"]);
        let line = status
            .lines()
            .find(|l| l.starts_with("check: "))
            .unwrap_or("(no check line)")
            .to_string();
        (pass, line)
    };
    let (pass, line) = line_after("docs/a.test.ts");
    assert!(
        line.starts_with("check: stale") && line.contains("docs/a.test.ts"),
        "{pass} / {line}"
    );
    let (pass, line) = line_after("docs/site/package.json");
    assert!(
        line.starts_with("check: stale") && line.contains("docs/site/package.json"),
        "{pass} / {line}"
    );
    let (pass, line) = line_after("docs/a.md");
    assert_eq!(line, pass);
}

/// A junit ticket with one passing step `__tests__/x.test.ts::one` whose Per-file run (one argument)
/// passes after the GREEN. The suite run (no argument) logs `args=0` to `suite.log` and then runs
/// `body`. `old` is a report committed before the step; `timeout` replaces `timeout_secs`.
fn suite_repo(body: &str, old: Option<&str>, timeout: u32) -> JunitRepo {
    let repo = junit_repo();
    let dir = repo.dir();
    let t = repo.tools.path();
    std::fs::write(t.join("red.xml"), FAILED_ONE).unwrap();
    std::fs::write(t.join("green.xml"), PASSED_ONE).unwrap();
    std::fs::write(
        t.join("suite.sh"),
        format!(
            "if [ \"$#\" = 1 ]; then\n  if [ -e src/ok.ts ]; then cat '{t}/green.xml' > junit.xml; exit 0; fi\n  cat '{t}/red.xml' > junit.xml\n  exit 1\nfi\necho \"args=$#\" >> '{t}/suite.log'\n{body}\n",
            t = t.display()
        ),
    )
    .unwrap();
    let config = dir.join(".stapel/stapel.toml");
    let text: String = std::fs::read_to_string(&config)
        .unwrap()
        .lines()
        .map(|l| {
            if l.starts_with("command = ") {
                format!("command = \"sh {} {{file}}\"\n", t.join("suite.sh").display())
            } else if l.starts_with("timeout_secs") {
                format!("timeout_secs = {timeout}\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    std::fs::write(&config, text).unwrap();
    if let Some(old) = old {
        write(dir, "junit.xml", old);
    }
    commit(dir, "ABC-1: a suite command");
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one");
    repo
}

const PASSED_ONE: &str = "<testsuites><testsuite name=\"s\"><testcase classname=\"\" name=\" one\"/></testsuite></testsuites>\n";
const FAILED_ONE: &str = "<testsuites><testsuite name=\"s\"><testcase classname=\"\" name=\" one\"><failure message=\"x\"/></testcase></testsuite></testsuites>\n";

/// The text of the `suite:` line of a check.
fn suite_line(text: &str) -> &str {
    text.lines()
        .find(|l| l.starts_with("suite: "))
        .unwrap_or_else(|| panic!("no suite line:\n{text}"))
}

/// A shell body that writes `xml` as the report and exits with `exit`.
fn writes(xml: &str, exit: i32) -> String {
    format!("printf '%s' '{xml}' > junit.xml\nexit {exit}")
}

// AC-26, AC-34: the HEAD suite runs the command once without `{file}`; faults are unverified.
#[test]
fn suite_runs_the_whole_command() {
    let case = |(a, b, c): (u32, u32, u32), exit: i32| {
        let cases = |n: u32, tail: &str| {
            (0..n)
                .map(|i| format!("<testcase classname=\"\" name=\" t{i}{tail}\"/>"))
                .collect::<String>()
        };
        let failure = "<testcase classname=\"\" name=\" bad\"><failure message=\"x\"/></testcase>";
        let skipped = "<testcase classname=\"\" name=\" skip\"><skipped/></testcase>";
        let xml = format!(
            "<testsuites><testsuite name=\"s\">{}{}{}{}</testsuite></testsuites>",
            cases(a, ""),
            if b > 0 { failure } else { "" },
            skipped.repeat(c as usize),
            ""
        );
        let repo = suite_repo(&writes(&xml, exit), None, 60);
        let (code, text) = check(&repo);
        let log = std::fs::read_to_string(repo.tools.path().join("suite.log")).unwrap_or_default();
        (repo, code, text, log)
    };
    // A pass: the command ran once with no argument; the counts are in the detail.
    let (_repo, code, text, log) = case((2, 0, 1), 0);
    assert_eq!(code, 0, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert_eq!(outcome(&text, "suite"), "pass", "{text}");
    assert!(
        suite_line(&text).contains("2 passed, 0 failed, 1 ignored"),
        "{text}"
    );
    assert_eq!(log, "args=0\n", "one run, `{{file}}` left out");
    // A failing case.
    let (_repo, code, text, _) = case((1, 1, 0), 1);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert_eq!(outcome(&text, "suite"), "fail", "{text}");
    assert!(suite_line(&text).contains("1 failed"), "{text}");

    let unverified = |body: &str, old: Option<&str>, timeout: u32, needle: &str| {
        let repo = suite_repo(body, old, timeout);
        let (code, text) = check(&repo);
        assert_eq!(code, 1, "{text}");
        assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
        assert_eq!(outcome(&text, "suite"), "unverified", "{text}");
        assert!(
            suite_line(&text).contains(needle),
            "wanted {needle:?}: {text}"
        );
    };
    // A committed passing report and a stand-in that writes none: the old report is removed.
    unverified("exit 0", Some(PASSED_ONE), 60, "no report");
    unverified("exit 0", None, 60, "no report");
    // A timeout and a signal.
    unverified("sleep 60", None, 10, "time limit of 10 s");
    unverified("kill -9 $$", None, 60, "signal");
    // A report over 16 MiB and one the reader refuses.
    unverified(
        "head -c 17825792 /dev/zero | tr '\\0' ' ' > junit.xml\nexit 0",
        None,
        60,
        "16 MiB",
    );
    unverified(
        "echo '<testsuites>' > junit.xml\nexit 0",
        None,
        60,
        "report",
    );
}

// AC-26: every test suite of the suite report is summed.
#[test]
fn suite_sums_every_suite() {
    let two = |second: &str| {
        format!(
            "<testsuites><testsuite name=\"a\"><testcase classname=\"\" name=\" a1\"/><testcase classname=\"\" name=\" a2\"/></testsuite><testsuite name=\"b\"><testcase classname=\"\" name=\" b1\"/>{second}</testsuite></testsuites>"
        )
    };
    let ok = "<testcase classname=\"\" name=\" b2\"/><testcase classname=\"\" name=\" b3\"/>";
    let repo = suite_repo(&writes(&two(ok), 0), None, 60);
    let (code, text) = check(&repo);
    assert_eq!(code, 0, "{text}");
    assert_eq!(outcome(&text, "suite"), "pass", "{text}");
    assert!(
        suite_line(&text).contains("5 passed, 0 failed, 0 ignored"),
        "{text}"
    );
    let bad = "<testcase classname=\"\" name=\" b2\"><failure message=\"x\"/></testcase><testcase classname=\"\" name=\" b3\"/>";
    let repo = suite_repo(&writes(&two(bad), 1), None, 60);
    let (code, text) = check(&repo);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "suite"), "fail", "{text}");
    assert!(
        suite_line(&text).contains("4 passed, 1 failed, 0 ignored"),
        "{text}"
    );
}

// AC-35: a junit record has `"runner": "junit"` and no cargo version; a passing one is current.
#[test]
fn junit_record_is_current() {
    let xml = "<testsuites><testsuite name=\"s\"><testcase classname=\"\" name=\" a\"/><testcase classname=\"\" name=\" b\"/></testsuite></testsuites>";
    let repo = suite_repo(&writes(xml, 0), None, 60);
    let dir = repo.dir();
    let (code, text) = check(&repo);
    assert_eq!(code, 0, "{text}");
    let runs = std::fs::read_to_string(dir.join(".stapel/tickets/ABC-1/runs.jsonl")).unwrap();
    let record: serde_json::Value =
        serde_json::from_str(runs.lines().last().expect("a record")).unwrap();
    assert_eq!(record["runner"], "junit", "{record}");
    assert!(record.get("cargo").is_none(), "{record}");
    assert_eq!(record["result"], "pass", "{record}");
    assert_eq!(record["suite"]["outcome"], "pass", "{record}");
    assert_eq!(record["suite"]["passed"], 2, "{record}");
    let pass = format!(
        "check: pass at {}",
        &git_in(dir, &["rev-parse", "HEAD"]).trim()[..7]
    );
    let (_, status) = stdout(dir, &["status", "ABC-1"]);
    assert!(status.lines().any(|l| l == pass), "{pass}\n{status}");
    let (_, next) = stdout(dir, &["next", "ABC-1"]);
    assert!(!next.contains("check: stale"), "{next}");
    assert!(!next.contains("stapel check"), "{next}");
}

/// Commits one test file with one call and a source file that is a GREEN of the same step.
fn pair(dir: &Path, step: u32, file: &str, call: &str) {
    write(dir, &format!("__tests__/{file}.test.ts"), call);
    commit(dir, &format!("ABC-1 step {step} RED: {file}"));
    write(dir, &format!("src/{file}.ts"), "export const x = 1;\n");
    commit(dir, &format!("ABC-1 step {step} GREEN: {file}"));
}

// AC-13, AC-14: the history outcomes of STP-4 and STP-14 under junit; `tests-changed` protects the
// text of a step test call, so a later RED supersedes it and a `spec change:` retires it.
#[test]
fn history_outcomes_under_junit() {
    let repo = junit_repo();
    let dir = repo.dir();
    // Step 1: a GREEN with no RED.
    commit(dir, "ABC-1 step 1 GREEN: nothing before it");
    // Step 2: two REDs.
    write(dir, "__tests__/two.test.ts", "it(\"two\", () => {});\n");
    commit(dir, "ABC-1 step 2 RED: two");
    write(dir, "__tests__/two.test.ts", "it(\"two\", () => { /* again */ });\n");
    commit(dir, "ABC-1 step 2 RED: two again");
    commit(dir, "ABC-1 step 2 GREEN: two");
    // Step 3: the RED changes code.
    write(dir, "src/lib.ts", "export const lib = 1;\n");
    write(dir, "__tests__/code.test.ts", "it(\"code\", () => {});\n");
    commit(dir, "ABC-1 step 3 RED: test and code");
    commit(dir, "ABC-1 step 3 GREEN: green");
    // Step 4: a README next to the test is no code.
    write(dir, "README.md", "notes\n");
    write(dir, "__tests__/doc.test.ts", "it(\"doc\", () => {});\n");
    commit(dir, "ABC-1 step 4 RED: test and a README");
    write(dir, "src/doc.ts", "export const doc = 1;\n");
    commit(dir, "ABC-1 step 4 GREEN: doc");
    // Step 5: the GREEN edits the step test call.
    pair(dir, 5, "five", "it(\"five\", () => { /* needs src/five.ts */ });\n");
    write(
        dir,
        "__tests__/five.test.ts",
        "it(\"five\", () => { /* needs src/five.ts, nothing */ });\n",
    );
    commit(dir, "ABC-1: edits the test of step 5");
    // Steps 6 and 7: the RED of step 7 edits the test of step 6.
    pair(dir, 6, "six", "it(\"six\", () => {});\n");
    write(dir, "__tests__/six.test.ts", "it(\"six\", () => { /* more */ });\n");
    commit(dir, "ABC-1 step 7 RED: six, changed");
    commit(dir, "ABC-1 step 7 GREEN: six");
    // Step 8: a spec change deletes the test.
    pair(dir, 8, "eight", "it(\"eight\", () => {});\n");
    write(dir, "__tests__/eight.test.ts", "// gone\n");
    commit(dir, "ABC-1: spec change: drop eight");
    // Steps 9 and 10: the RED of step 10 lies between the RED and the GREEN of step 9.
    write(dir, "__tests__/nine.test.ts", "it(\"nine\", () => {});\n");
    commit(dir, "ABC-1 step 9 RED: nine");
    write(dir, "__tests__/ten.test.ts", "it(\"ten\", () => {});\n");
    commit(dir, "ABC-1 step 10 RED: ten");
    write(dir, "src/nine.ts", "export const nine = 1;\n");
    commit(dir, "ABC-1 step 9 GREEN: nine");
    write(dir, "src/ten.ts", "export const ten = 1;\n");
    commit(dir, "ABC-1 step 10 GREEN: ten");

    let (code, text) = stdout(dir, &["check", "ABC-1", "--history"]);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unpaired", "{text}");
    assert_eq!(outcome(&text, "step 2"), "duplicate", "{text}");
    assert_eq!(outcome(&text, "step 3"), "red-changes-code", "{text}");
    assert!(text.contains("step 3: red-changes-code: src/lib.ts"), "{text}");
    assert_eq!(outcome(&text, "step 4"), "needs-run", "{text}");
    assert_eq!(outcome(&text, "step 5"), "tests-changed", "{text}");
    assert!(
        text.contains("step 5: tests-changed: __tests__/five.test.ts::five"),
        "{text}"
    );
    assert_eq!(outcome(&text, "step 6"), "needs-run", "{text}");
    assert!(
        text.contains("  superseded by step 7: __tests__/six.test.ts::six"),
        "{text}"
    );
    assert_eq!(outcome(&text, "step 7"), "needs-run", "{text}");
    assert_eq!(outcome(&text, "step 8"), "retired", "{text}");
    assert!(
        text.contains("  retired by "),
        "the spec change retires the test: {text}"
    );
    assert_eq!(outcome(&text, "step 9"), "unverified", "{text}");
    assert!(text.contains("interleaved: step 10"), "{text}");
    assert_eq!(outcome(&text, "step 10"), "needs-run", "{text}");
}

/// A junit ticket whose step 1 RED adds `__tests__/h.test.ts` (a helper line and one test call) and
/// the fixture `__tests__/data.json`; `green` is the GREEN commit's change.
fn protected_repo(green: impl Fn(&Path)) -> JunitRepo {
    let repo = junit_repo();
    let dir = repo.dir();
    write(
        dir,
        "__tests__/h.test.ts",
        "const helper = 1;\nit(\"holds\", () => { /* needs src/h.ts */ });\n",
    );
    write(dir, "__tests__/data.json", "{\"a\": 1}\n");
    commit(dir, "ABC-1 step 1 RED: holds");
    write(dir, "src/h.ts", "export const h = 1;\n");
    green(dir);
    commit(dir, "ABC-1 step 1 GREEN: holds");
    repo
}

// AC-27: `tests-changed` protects the rest of a test file the RED changed and its other files that
// match a test glob; a file the RED left alone stays free.
#[test]
fn tests_changed_under_junit() {
    let case = |name: &str, green: &dyn Fn(&Path)| {
        let repo = protected_repo(green);
        let (_, text) = stdout(repo.dir(), &["check", "ABC-1", "--history"]);
        let line = text
            .lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("{name}: no step 1:\n{text}"))
            .to_string();
        line
    };
    let line = case("a helper line", &|dir| {
        write(
            dir,
            "__tests__/h.test.ts",
            "const helper = 2;\nit(\"holds\", () => { /* needs src/h.ts */ });\n",
        );
    });
    assert_eq!(
        line, "step 1: tests-changed: __tests__/h.test.ts",
        "a helper line"
    );
    let line = case("the fixture", &|dir| {
        write(dir, "__tests__/data.json", "{\"a\": 2}\n");
    });
    assert_eq!(
        line, "step 1: tests-changed: __tests__/data.json",
        "the fixture"
    );
    let line = case("a file the RED left alone", &|dir| {
        write(
            dir,
            "__tests__/base.test.ts",
            "const helper = 5;\nit(\"base\", () => {});\nit(\"keeps\", () => { /* needs src/index.ts */ });\n",
        );
    });
    assert_eq!(line, "step 1: needs-run", "a file the RED left alone");
}

// AC-27: the commit hook prints the `tests-changed` fault of a GREEN that edits a helper line.
#[test]
fn hook_faults_under_junit() {
    let repo = protected_repo(|dir| {
        write(
            dir,
            "__tests__/h.test.ts",
            "const helper = 2;\nit(\"holds\", () => { /* needs src/h.ts */ });\n",
        );
    });
    let dir = repo.dir();
    let payload = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git commit -m x", "description": "test" },
        "tool_response": { "stdout": "", "stderr": "" },
        "cwd": dir,
    });
    let out = stapel(dir)
        .args(["hook", "post-tool-use"])
        .write_stdin(payload.to_string())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(out.status.code(), Some(0), "{text}");
    let answer: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("stdout is not one JSON object ({e}):\n{text}"));
    let context = answer["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or_else(|| panic!("no additionalContext:\n{text}"));
    assert!(
        context
            .lines()
            .any(|l| l == "step 1: tests-changed: __tests__/h.test.ts"),
        "{context}"
    );
}
