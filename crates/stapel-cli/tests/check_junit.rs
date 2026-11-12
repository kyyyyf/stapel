//! STP-28: `stapel check` under `[check] runner = "junit"`: step tests and the per-file run.

mod common;

use common::{JunitRepo, git_in, junit_repo, set_section, stapel};
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

// AC-42: a GREEN that edits a file in the folder of a step test's file, which matches a test glob and
// is no test file, reads `tests-changed`; the same file in another folder is free.
#[test]
fn fixture_beside_a_test_is_protected() {
    let case = |edit: &str| {
        let repo = junit_repo();
        let dir = repo.dir();
        write(dir, "__tests__/data.json", "{\"a\": 1}\n");
        write(dir, "other/data.test.json", "{\"a\": 1}\n");
        commit(dir, "ABC-1: fixtures");
        write(
            dir,
            "__tests__/f.test.ts",
            "it(\"fixed\", () => { /* needs src/f.ts */ });\n",
        );
        commit(dir, "ABC-1 step 1 RED: fixed");
        write(dir, "src/f.ts", "export const f = 1;\n");
        write(dir, edit, "{\"a\": 2}\n");
        commit(dir, "ABC-1 step 1 GREEN: fixed");
        let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("{edit}: no step 1:\n{text}"))
            .to_string()
    };
    assert_eq!(
        case("__tests__/data.json"),
        "step 1: tests-changed: __tests__/data.json"
    );
    assert_eq!(case("other/data.test.json"), "step 1: needs-run");
}

// AC-42: a fixture a GREEN adds beside a step test's file, or a later commit deletes, reads
// `tests-changed`, though the RED left it alone.
#[test]
fn fixture_added_or_deleted_is_protected() {
    let case = |fixture_before: bool, green: &dyn Fn(&Path)| {
        let repo = junit_repo();
        let dir = repo.dir();
        if fixture_before {
            write(dir, "__tests__/data.json", "{\"a\": 1}\n");
            commit(dir, "ABC-1: fixture");
        }
        write(
            dir,
            "__tests__/f.test.ts",
            "it(\"fixed\", () => { /* needs src/f.ts */ });\n",
        );
        commit(dir, "ABC-1 step 1 RED: fixed");
        write(dir, "src/f.ts", "export const f = 1;\n");
        green(dir);
        commit(dir, "ABC-1 step 1 GREEN: fixed");
        let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string()
    };
    assert_eq!(
        case(false, &|dir| write(dir, "__tests__/data.json", "{\"a\": 2}\n")),
        "step 1: tests-changed: __tests__/data.json",
        "added by the GREEN"
    );
    assert_eq!(
        case(true, &|dir| std::fs::remove_file(dir.join("__tests__/data.json")).unwrap()),
        "step 1: tests-changed: __tests__/data.json",
        "deleted after the RED"
    );
}

// AC-27: a RED that only adds a test call, or only reformats the rest of the file, leaves the rest
// of that file unprotected: a GREEN that edits a helper reads `needs-run`.
#[test]
fn same_file_helper_is_not_protected() {
    let case = |reformat: bool| {
        let repo = junit_repo();
        let dir = repo.dir();
        if reformat {
            write(
                dir,
                "__tests__/base.test.ts",
                "const  helper = 1 ;\nit(\"base\", () => {});\nit(\"keeps\", () => { /* needs src/index.ts */ });\nit(\"more\", () => { /* needs src/m.ts */ });\n",
            );
        } else {
            write(
                dir,
                "__tests__/base.test.ts",
                "const helper = 1;\nit(\"base\", () => {});\nit(\"keeps\", () => { /* needs src/index.ts */ });\nit(\"more\", () => { /* needs src/m.ts */ });\n",
            );
        }
        commit(dir, "ABC-1 step 1 RED: more");
        write(dir, "src/m.ts", "export const m = 1;\n");
        write(
            dir,
            "__tests__/base.test.ts",
            "const helper = 2;\nit(\"base\", () => {});\nit(\"keeps\", () => { /* needs src/index.ts */ });\nit(\"more\", () => { /* needs src/m.ts */ });\n",
        );
        commit(dir, "ABC-1 step 1 GREEN: more");
        let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string()
    };
    assert_eq!(case(false), "step 1: needs-run", "only a test call added");
    assert_eq!(case(true), "step 1: needs-run", "spaces and `;` only");
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

// AC-11: a title with an edge space matches the report's name once both are trimmed.
#[test]
fn step_name_with_edge_space() {
    let repo = junit_repo();
    let dir = repo.dir();
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"a \", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: a");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: a");
    let (code, text) = check(&repo);
    assert_eq!(code, 0, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
}

// AC-19: exit 1 with only skipped cases, and exit 1 with one suite of zero cases, are unverified.
#[test]
fn exit_code_edge_cases() {
    const SKIPPED: &str = "<testsuites><testsuite name=\"s\"><testcase classname=\"\" name=\" one\"><skipped/></testcase></testsuite></testsuites>\n";
    const EMPTY: &str = "<testsuites><testsuite name=\"s\"></testsuite></testsuites>\n";
    for (label, report) in [("skipped", SKIPPED), ("empty suite", EMPTY)] {
        let (code, text) = run_case(report, 1, PASSED, 0);
        assert_eq!(code, 1, "{label}: {text}");
        assert_eq!(outcome(&text, "step 1"), "unverified", "{label}: {text}");
        let line = text.lines().find(|l| l.starts_with("step 1: ")).unwrap();
        assert!(line.contains("exit code 1"), "{label}: {line}");
    }
}

// AC-15: a load failure with exit 0 is unverified (AC-19 first); one beside normal cases is read as
// normal cases.
#[test]
fn load_failure_edge_cases() {
    let (code, text) = run_case(NOT_LOADED, 0, PASSED, 0);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    let line = text.lines().find(|l| l.starts_with("step 1: ")).unwrap();
    assert!(line.contains("exit code 0"), "{line}");
    // A failed load case beside a normal failed case: not a load failure, the test case fails.
    const MIXED: &str = "<testsuites><testsuite name=\"s\"><testcase classname=\"Test suite failed to run\" name=\"Test suite failed to run\"><failure message=\"x\"/></testcase><testcase classname=\"\" name=\" one\"><failure message=\"y\"/></testcase></testsuite></testsuites>\n";
    let (code, text) = run_case(MIXED, 1, PASSED, 0);
    assert_eq!(code, 0, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert!(text.contains("RED 1 assert, 0 compile"), "{text}");
}

// AC-22: `.only` in the second test file of a step at GREEN makes the whole step unverified.
#[test]
fn only_call_in_another_file() {
    let repo = junit_repo();
    let dir = repo.dir();
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    );
    write(
        dir,
        "__tests__/y.test.ts",
        "it(\"two\", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one and two");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    write(
        dir,
        "__tests__/y.test.ts",
        "it(\"two\", () => { /* needs src/ok.ts */ });\nit.only(\"three\", () => {});\n",
    );
    commit(dir, "ABC-1 step 1 GREEN: one and two");
    let (code, text) = check(&repo);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    let line = text.lines().find(|l| l.starts_with("step 1: ")).unwrap();
    assert!(
        line.contains("__tests__/y.test.ts") && line.contains(".only") && line.contains("GREEN"),
        "{line}"
    );
    assert!(
        repo.runs().iter().all(|l| !l.contains("__tests__/")),
        "no command runs for the step: {:?}",
        repo.runs()
    );
}

// AC-21: the size of a report is read from its metadata before the file is read.
#[test]
fn report_size_checked_before_read() {
    let big = "truncate -s 17825792 junit.xml\nchmod 000 junit.xml\nexit 0\n";
    let (code, text) = fault_case("sh {script} {file}", big, None, 60);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    let line = text.lines().find(|l| l.starts_with("step 1: ")).unwrap();
    assert!(line.contains("16 MiB"), "{line}");
}

// STP-30 AC-13: the size bound of a regular report holds: exactly 16 MiB is read, one byte more is
// unverified as over 16 MiB.
#[test]
fn report_size_bound() {
    let body = |total: u64| {
        format!(
            "printf '%s' '{}' > junit.xml\nn=$(wc -c < junit.xml)\nhead -c $(({total} - n)) /dev/zero | tr '\\0' ' ' >> junit.xml\nexit 0\n",
            PASSED_ONE.trim()
        )
    };
    let limit: u64 = 16 * 1024 * 1024;

    let (_, text) = fault_case("sh {script} {file}", &body(limit), None, 60);
    let line = text.lines().find(|l| l.starts_with("step 1: ")).unwrap();
    assert!(!line.contains("16 MiB"), "exactly 16 MiB is read: {line}");
    assert!(!line.contains("unverified"), "exactly 16 MiB is read: {line}");

    let (code, text) = fault_case("sh {script} {file}", &body(limit + 1), None, 60);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    let line = text.lines().find(|l| l.starts_with("step 1: ")).unwrap();
    assert!(line.contains("16 MiB"), "{line}");
}

/// A change of the configuration: the `timeout_secs` line gets the value `n`.
fn bump_config(dir: &Path, n: u32) {
    let path = dir.join(".stapel/stapel.toml");
    let text: String = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|l| {
            if l.starts_with("timeout_secs = ") {
                format!("timeout_secs = {n}\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    std::fs::write(path, text).unwrap();
}

/// A passing step `number` in the junit repository: `__tests__/<file>.test.ts` and `src/<name>.ts`.
fn add_step(dir: &Path, number: u32, file: &str, name: &str) {
    write(
        dir,
        &format!("__tests__/{file}.test.ts"),
        &format!("it(\"{name}\", () => {{ /* needs src/{name}.ts */ }});\n"),
    );
    commit(dir, &format!("ABC-1 step {number} RED: {name}"));
    write(dir, &format!("src/{name}.ts"), "export const v = 1;\n");
    commit(dir, &format!("ABC-1 step {number} GREEN: {name}"));
}

// AC-25: a change of `.stapel/stapel.toml` after the first RED blocks every step whose RED lies before
// it, wherever the change falls; a change at or before the first RED blocks no step.
#[test]
fn config_change_after_first_red() {
    // Before the first RED, in its own commit: no block.
    let repo = junit_repo();
    bump_config(repo.dir(), 61);
    commit(repo.dir(), "ABC-1: a configuration change");
    add_step(repo.dir(), 1, "x", "one");
    let (code, text) = check(&repo);
    assert_eq!(code, 0, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");

    // In the first RED commit itself: no block.
    let repo = junit_repo();
    bump_config(repo.dir(), 62);
    add_step(repo.dir(), 1, "x", "one");
    let (code, text) = check(&repo);
    assert_eq!(code, 0, "{text}");
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");

    // After the last GREEN: the step reads `build-input-changed`, no command runs.
    let repo = junit_repo();
    add_step(repo.dir(), 1, "x", "one");
    bump_config(repo.dir(), 63);
    commit(repo.dir(), "ABC-1: a configuration change");
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

    // Between two steps: the first reads `build-input-changed`, the second, whose RED lies after the
    // change, passes.
    let repo = junit_repo();
    add_step(repo.dir(), 1, "x", "one");
    bump_config(repo.dir(), 64);
    commit(repo.dir(), "ABC-1: a configuration change");
    let sha = git_in(repo.dir(), &["rev-parse", "HEAD"]);
    add_step(repo.dir(), 2, "y", "two");
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
    assert_eq!(outcome(&text, "step 2"), "pass", "{text}");
    let runs = repo.runs();
    assert!(runs.iter().all(|l| !l.ends_with("x.test.ts")), "{runs:?}");
    assert!(runs.iter().any(|l| l.ends_with("y.test.ts")), "{runs:?}");
}

// AC-41: a block of a junit step names the blocking change as the fault commit, in the commit hook
// and in `stapel next`, not an earlier change that blocks no step.
#[test]
fn fault_commit_is_the_blocking_change() {
    let repo = junit_repo();
    let dir = repo.dir();
    // A configuration change before the first RED blocks no step.
    bump_config(dir, 61);
    commit(dir, "ABC-1: a configuration change");
    set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | WHEN a thing happens THE command does it. | `x::one` |",
    );
    set_section(dir, "ABC-1", "Design", "The design.");
    set_section(dir, "ABC-1", "Proof", "The proof.");
    set_section(dir, "ABC-1", "Plan", "Route: full, auto.\n\n1. One (AC-1).\n2. Two (AC-1).");
    for stage in ["description", "spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", stage]).assert().success();
    }
    commit(dir, "ABC-1: the ticket is confirmed");
    add_step(dir, 1, "x", "one");
    write(
        dir,
        "__tests__/y.test.ts",
        "it(\"two\", () => { /* needs src/two.ts */ });\n",
    );
    commit(dir, "ABC-1 step 2 RED: two");
    write(dir, "src/two.ts", "export const two = 2;\n");
    write(dir, "package.json", "{\"name\": \"x\"}\n");
    commit(dir, "ABC-1 step 2 GREEN: two");
    let head = git_in(dir, &["rev-parse", "HEAD"]).trim()[..7].to_string();

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
        context.lines().any(|l| l
            == format!(
                "step 2: unverified: build-input-changed: package.json at {head}"
            )),
        "{context}"
    );
    assert!(
        context.lines().any(|l| l
            == format!("stapel: {head} is HEAD; fix it with git commit --amend (step 2)")),
        "{context}"
    );

    let out = stapel(dir).arg("next").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let want = format!("blocked: step 2: unverified at {head} — build-input-changed: package.json");
    assert!(text.lines().any(|l| l.starts_with(&want)), "{want}\n{text}");
}

// AC-39, AC-24: the window of an input change. A plain input blocks the step unless it changes after
// the step's GREEN; the script that `command` starts blocks it only between its RED and GREEN.
#[test]
fn input_window_edges() {
    use std::os::unix::fs::PermissionsExt;
    // One step; `touch` changes the file under test at `when`: before the RED, between the RED and
    // the GREEN, in the GREEN or after the GREEN (a change inside the RED is `red-changes-code`, not
    // a window case). The exit code and the line of step 1.
    let case = |script: bool, when: &str| -> (i32, String) {
        let repo = junit_repo();
        let dir = repo.dir();
        if script {
            let stand_in = repo_stand_in(dir);
            write(
                dir,
                "run.sh",
                &format!("#!/bin/sh\nexec sh '{stand_in}' \"$@\"\n"),
            );
            let mode = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(dir.join("run.sh"), mode).unwrap();
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
            commit(dir, "ABC-1: a repository script");
        }
        let touch = |dir: &Path| {
            if script {
                let path = dir.join("run.sh");
                let text = std::fs::read_to_string(&path).unwrap();
                std::fs::write(&path, format!("{text}# touched\n")).unwrap();
            } else {
                write(dir, "package.json", "{\"name\": \"x\"}\n");
            }
        };
        if when == "before" {
            touch(dir);
            commit(dir, "ABC-1: a change");
        }
        write(
            dir,
            "__tests__/x.test.ts",
            "it(\"one\", () => { /* needs src/ok.ts */ });\n",
        );
        commit(dir, "ABC-1 step 1 RED: one");
        if when == "between" {
            touch(dir);
            commit(dir, "ABC-1: a change");
        }
        write(dir, "src/ok.ts", "export const ok = 1;\n");
        if when == "green" {
            touch(dir);
        }
        commit(dir, "ABC-1 step 1 GREEN: one");
        if when == "after" {
            touch(dir);
            commit(dir, "ABC-1: a change");
        }
        let (code, text) = check(&repo);
        (code, step_one(&text).to_string())
    };
    let name = |script: bool| if script { "run.sh" } else { "package.json" };
    for (script, when, blocked) in [
        (false, "before", true),
        (false, "between", true),
        (false, "green", true),
        (false, "after", false),
        (true, "before", false),
        (true, "between", true),
        (true, "green", true),
        (true, "after", false),
    ] {
        let (code, line) = case(script, when);
        let what = format!("{} {when}: {line}", name(script));
        if blocked {
            assert_eq!(code, 1, "{what}");
            let want = format!("step 1: unverified: build-input-changed: {} at ", name(script));
            assert!(line.starts_with(&want), "{what}");
        } else {
            assert_eq!(code, 0, "{what}");
            assert!(line.starts_with("step 1: pass"), "{what}");
        }
    }
}

// AC-43: a merge of the base branch that brings a build input unchanged blocks nothing; resolved to
// other content, or in a ticket without `base_branch`, it blocks.
#[test]
fn merge_of_the_base_branch_blocks_nothing() {
    /// Steps 1 and 2 around a merge of main that brings `jest.config.js`; returns the output of
    /// the check and the merge commit.
    fn scenario(base_branch: bool, resolved: bool) -> (String, String) {
        let repo = junit_repo();
        let dir = repo.dir();
        let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        if base_branch {
            let path = dir.join(".stapel/tickets/ABC-1/state.json");
            let mut state: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            state["base_branch"] = serde_json::json!(main);
            std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
            commit(dir, "ABC-1: base branch");
        }
        git_in(dir, &["checkout", "-q", "-b", "ticket"]);
        write(
            dir,
            "__tests__/x.test.ts",
            "it(\"one\", () => { /* needs src/ok.ts */ });\n",
        );
        commit(dir, "ABC-1 step 1 RED: one");
        write(dir, "src/ok.ts", "export const ok = 1;\n");
        commit(dir, "ABC-1 step 1 GREEN: one");
        git_in(dir, &["checkout", "-q", &main]);
        write(dir, "jest.config.js", "module.exports = {};\n");
        commit(dir, "main: a jest config");
        git_in(dir, &["checkout", "-q", "ticket"]);
        git_in(dir, &["merge", "-q", "--no-ff", "--no-commit", &main]);
        if resolved {
            write(dir, "jest.config.js", "module.exports = { bail: 1 };\n");
        }
        commit(dir, "ABC-1: merge main");
        let merge = git_in(dir, &["rev-parse", "HEAD"]).trim().to_string();
        write(
            dir,
            "__tests__/y.test.ts",
            "it(\"two\", () => { /* needs src/two.ts */ });\n",
        );
        commit(dir, "ABC-1 step 2 RED: two");
        write(dir, "src/two.ts", "export const two = 2;\n");
        commit(dir, "ABC-1 step 2 GREEN: two");
        let (_, text) = check(&repo);
        (text, merge)
    }

    let (text, _) = scenario(true, false);
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    assert_eq!(outcome(&text, "step 2"), "pass", "{text}");
    assert!(!text.contains("build-input-changed"), "{text}");

    for (base_branch, resolved) in [(true, true), (false, false)] {
        let (text, merge) = scenario(base_branch, resolved);
        assert_eq!(
            text.lines().find(|l| l.starts_with("step 2: ")).unwrap(),
            format!(
                "step 2: unverified: build-input-changed: jest.config.js at {}",
                &merge[..7]
            ),
            "base_branch {base_branch}, resolved {resolved}: {text}"
        );
    }
}

// STP-30 AC-1, AC-2: a fixture `a b.json` that a merge of the base branch brings into the folder of a
// step test reads `tests-changed` only when the merge resolves it to other text.
#[test]
fn base_merge_fixture_is_no_tests_change() {
    let scenario = |resolved: bool| {
        let repo = junit_repo();
        let dir = repo.dir();
        let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let path = dir.join(".stapel/tickets/ABC-1/state.json");
        let mut state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        state["base_branch"] = serde_json::json!(main);
        std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
        commit(dir, "ABC-1: base branch");
        git_in(dir, &["checkout", "-q", "-b", "ticket"]);
        write(
            dir,
            "__tests__/f.test.ts",
            "it(\"fixed\", () => { /* needs src/f.ts */ });\n",
        );
        commit(dir, "ABC-1 step 1 RED: fixed");
        write(dir, "src/f.ts", "export const f = 1;\n");
        commit(dir, "ABC-1 step 1 GREEN: fixed");
        git_in(dir, &["checkout", "-q", &main]);
        write(dir, "__tests__/a b.json", "{\"a\": 1}\n");
        commit(dir, "main: a fixture");
        git_in(dir, &["checkout", "-q", "ticket"]);
        git_in(dir, &["merge", "-q", "--no-ff", "--no-commit", &main]);
        if resolved {
            write(dir, "__tests__/a b.json", "{\"a\": 2}\n");
        }
        commit(dir, "ABC-1: merge main");
        let merge = git_in(dir, &["rev-parse", "HEAD"]).trim().to_string();
        let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
        let line = text
            .lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string();
        (line, merge, text)
    };
    let (line, _, text) = scenario(false);
    assert_eq!(line, "step 1: needs-run", "{text}");
    let (line, merge, text) = scenario(true);
    assert_eq!(line, "step 1: tests-changed: __tests__/a b.json", "{text}");
    assert!(text.contains(&merge[..7]), "the fault is the merge: {text}");
}

// STP-30 AC-1 (CR1-E3): a fixture present at the RED, deleted on main and brought by a merge of the
// base branch is absent at the merge and at its second parent: no `tests-changed`.
#[test]
fn base_merge_deletes_fixture() {
    let repo = junit_repo();
    let dir = repo.dir();
    let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    let path = dir.join(".stapel/tickets/ABC-1/state.json");
    let mut state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    state["base_branch"] = serde_json::json!(main);
    std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
    write(dir, "__tests__/a b.json", "{\"a\": 1}\n");
    commit(dir, "ABC-1: base branch with a fixture");
    git_in(dir, &["checkout", "-q", "-b", "ticket"]);
    write(
        dir,
        "__tests__/f.test.ts",
        "it(\"fixed\", () => { /* needs src/f.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: fixed");
    write(dir, "src/f.ts", "export const f = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: fixed");
    git_in(dir, &["checkout", "-q", &main]);
    git_in(dir, &["rm", "-q", "__tests__/a b.json"]);
    commit(dir, "main: the fixture is gone");
    git_in(dir, &["checkout", "-q", "ticket"]);
    git_in(dir, &["merge", "-q", "--no-ff", "-m", "ABC-1: merge main", &main]);
    let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
    let line = text
        .lines()
        .find(|l| l.starts_with("step 1: "))
        .unwrap_or_else(|| panic!("no step 1:\n{text}"))
        .to_string();
    assert_eq!(line, "step 1: needs-run", "{text}");
}

// STP-30 AC-1, AC-2 (CR1-E1): a fixture `x[1].json` is a path, not a pattern: a merge that resolves
// it to a third text blocks, a merge that brings it equal passes.
#[test]
fn base_merge_glob_names() {
    let scenario = |resolved: bool| {
        let repo = junit_repo();
        let dir = repo.dir();
        let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let path = dir.join(".stapel/tickets/ABC-1/state.json");
        let mut state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        state["base_branch"] = serde_json::json!(main);
        std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
        commit(dir, "ABC-1: base branch");
        git_in(dir, &["checkout", "-q", "-b", "ticket"]);
        write(
            dir,
            "__tests__/f.test.ts",
            "it(\"fixed\", () => { /* needs src/f.ts */ });\n",
        );
        commit(dir, "ABC-1 step 1 RED: fixed");
        write(dir, "src/f.ts", "export const f = 1;\n");
        commit(dir, "ABC-1 step 1 GREEN: fixed");
        git_in(dir, &["checkout", "-q", &main]);
        write(dir, "__tests__/x[1].json", "{\"a\": 1}\n");
        commit(dir, "main: a fixture");
        git_in(dir, &["checkout", "-q", "ticket"]);
        git_in(dir, &["merge", "-q", "--no-ff", "--no-commit", &main]);
        if resolved {
            write(dir, "__tests__/x[1].json", "{\"a\": 3}\n");
        }
        commit(dir, "ABC-1: merge main");
        let merge = git_in(dir, &["rev-parse", "HEAD"]).trim().to_string();
        let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
        let line = text
            .lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string();
        (line, merge, text)
    };
    let (line, _, text) = scenario(false);
    assert_eq!(line, "step 1: needs-run", "{text}");
    let (line, merge, text) = scenario(true);
    assert_eq!(line, "step 1: tests-changed: __tests__/x[1].json", "{text}");
    assert!(text.contains(&merge[..7]), "the fault is the merge: {text}");
}

// STP-30 AC-1, AC-6 (d1-1, d1-3): a mode change with an equal blob at a merge of the base branch
// blocks for a junit input and passes for a fixture beside a test.
#[test]
fn base_merge_mode_change_blocks() {
    fn scenario(path: &str, chmod: bool) -> (String, String) {
        let repo = junit_repo();
        let dir = repo.dir();
        let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let state_path = dir.join(".stapel/tickets/ABC-1/state.json");
        let mut state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        state["base_branch"] = serde_json::json!(main);
        std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
        commit(dir, "ABC-1: base branch");
        git_in(dir, &["checkout", "-q", "-b", "ticket"]);
        write(
            dir,
            "__tests__/x.test.ts",
            "it(\"one\", () => { /* needs src/ok.ts */ });\n",
        );
        commit(dir, "ABC-1 step 1 RED: one");
        write(dir, "src/ok.ts", "export const ok = 1;\n");
        commit(dir, "ABC-1 step 1 GREEN: one");
        git_in(dir, &["checkout", "-q", &main]);
        write(dir, path, "{\"a\": 1}\n");
        commit(dir, "main: a file");
        git_in(dir, &["checkout", "-q", "ticket"]);
        git_in(dir, &["merge", "-q", "--no-ff", "--no-commit", &main]);
        if chmod {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join(path), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        commit(dir, "ABC-1: merge main");
        let merge = git_in(dir, &["rev-parse", "HEAD"]).trim().to_string();
        write(
            dir,
            "__tests__/y.test.ts",
            "it(\"two\", () => { /* needs src/two.ts */ });\n",
        );
        commit(dir, "ABC-1 step 2 RED: two");
        write(dir, "src/two.ts", "export const two = 2;\n");
        commit(dir, "ABC-1 step 2 GREEN: two");
        let (_, text) = check(&repo);
        (text, merge)
    }

    // Control: an equal mode passes for the input.
    let (text, _) = scenario("package.json", false);
    assert_eq!(outcome(&text, "step 2"), "pass", "{text}");

    // A junit input with `chmod +x` and an equal blob.
    let (text, merge) = scenario("package.json", true);
    assert_eq!(
        text.lines().find(|l| l.starts_with("step 2: ")).unwrap(),
        format!(
            "step 2: unverified: build-input-changed: package.json at {}",
            &merge[..7]
        ),
        "{text}"
    );

    // A fixture beside a test with only `chmod +x` passes.
    let (text, _) = scenario("__tests__/a b.json", true);
    assert_eq!(outcome(&text, "step 2"), "pass", "{text}");
    assert!(!text.contains("build-input-changed"), "{text}");
}

// STP-30 AC-3 (CR1-E2): under junit the mode of a fixture is not part of its value: a fixture that a
// Base merge brings with equal content but another mode gives no `tests-changed` for step 1.
#[test]
fn base_merge_fixture_mode_only() {
    fn scenario(path: &str) -> (String, String) {
        let repo = junit_repo();
        let dir = repo.dir();
        let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let state_path = dir.join(".stapel/tickets/ABC-1/state.json");
        let mut state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
        state["base_branch"] = serde_json::json!(main);
        std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
        commit(dir, "ABC-1: base branch");
        git_in(dir, &["checkout", "-q", "-b", "ticket"]);
        write(
            dir,
            "__tests__/f.test.ts",
            "it(\"fixed\", () => { /* needs src/f.ts */ });\n",
        );
        commit(dir, "ABC-1 step 1 RED: fixed");
        write(dir, "src/f.ts", "export const f = 1;\n");
        commit(dir, "ABC-1 step 1 GREEN: fixed");
        git_in(dir, &["checkout", "-q", &main]);
        write(dir, path, "{\"a\": 1}\n");
        commit(dir, "main: a fixture");
        git_in(dir, &["checkout", "-q", "ticket"]);
        git_in(dir, &["merge", "-q", "--no-ff", "--no-commit", &main]);
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join(path), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        commit(dir, "ABC-1: merge main");
        let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
        let line = text
            .lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string();
        (line, text)
    }
    for path in ["__tests__/a b.json", "__tests__/data/c.json"] {
        let (line, text) = scenario(path);
        assert_eq!(line, "step 1: needs-run", "{path}: {text}");
    }
}

// STP-30 AC-16: after an equal merge, an edit of the fixture is compared with its value at the merge
// and reads `tests-changed`.
#[test]
fn base_merge_then_edit_is_tests_change() {
    let repo = junit_repo();
    let dir = repo.dir();
    let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    let path = dir.join(".stapel/tickets/ABC-1/state.json");
    let mut state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    state["base_branch"] = serde_json::json!(main);
    std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
    commit(dir, "ABC-1: base branch");
    git_in(dir, &["checkout", "-q", "-b", "ticket"]);
    write(
        dir,
        "__tests__/f.test.ts",
        "it(\"fixed\", () => { /* needs src/f.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: fixed");
    write(dir, "src/f.ts", "export const f = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: fixed");
    git_in(dir, &["checkout", "-q", &main]);
    write(dir, "__tests__/a b.json", "{\"a\": 1}\n");
    commit(dir, "main: a fixture");
    git_in(dir, &["checkout", "-q", "ticket"]);
    git_in(dir, &["merge", "-q", "--no-ff", "--no-commit", &main]);
    commit(dir, "ABC-1: merge main");
    let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
    assert!(text.contains("step 1: needs-run"), "equal merge: {text}");
    write(dir, "__tests__/a b.json", "{\"a\": 3}\n");
    commit(dir, "ABC-1: edit the fixture");
    let edit = git_in(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
    assert!(
        text.contains("step 1: tests-changed: __tests__/a b.json"),
        "{text}"
    );
    assert!(text.contains(&edit[..7]), "the fault is the edit: {text}");
}

// AC-21, AC-34: a report that is a FIFO, a folder or a symbolic link is unverified with its kind in
// the reason, for a Per-file run and for the suite; no read blocks.
#[test]
fn report_must_be_a_regular_file() {
    // The writer of the FIFO case ends by itself, so a reader that does open the FIFO is not stuck.
    let kinds = [
        (
            "a FIFO",
            "mkfifo junit.xml\n(timeout 10 sh -c ': > junit.xml' >/dev/null 2>&1 &)\nexit 0\n",
        ),
        ("a folder", "mkdir junit.xml\nexit 0\n"),
        (
            "a symbolic link",
            "echo '<testsuites/>' > real.xml\nln -s real.xml junit.xml\nexit 0\n",
        ),
    ];
    for (kind, body) in kinds {
        let (code, text) = fault_case("sh {script} {file}", body, None, 60);
        assert_eq!(code, 1, "{kind}: {text}");
        assert_eq!(outcome(&text, "step 1"), "unverified", "{kind}: {text}");
        let line = text.lines().find(|l| l.starts_with("step 1: ")).unwrap();
        assert!(line.contains(kind), "per-file {kind}: {line}");

        let repo = suite_repo(body, None, 60);
        let (code, text) = check(&repo);
        assert_eq!(code, 1, "{kind}: {text}");
        assert_eq!(outcome(&text, "suite"), "unverified", "{kind}: {text}");
        assert!(suite_line(&text).contains(kind), "suite {kind}: {text}");
    }
}

// STP-30 AC-12: a report that is a link, a folder, a FIFO or a socket is not opened; the reason
// names that kind once.
#[test]
fn report_kinds_are_named() {
    let kinds = [
        (
            "a FIFO",
            "mkfifo junit.xml\n(timeout 10 sh -c ': > junit.xml' >/dev/null 2>&1 &)\nexit 0\n",
        ),
        ("a folder", "mkdir junit.xml\nexit 0\n"),
        (
            "a symbolic link",
            "echo '<testsuites/>' > real.xml\nln -s real.xml junit.xml\nexit 0\n",
        ),
        (
            "a socket",
            "python3 -c \"import socket; socket.socket(socket.AF_UNIX).bind('junit.xml')\"\nexit 0\n",
        ),
    ];
    for (kind, body) in kinds {
        let (code, text) = fault_case("sh {script} {file}", body, None, 60);
        assert_eq!(code, 1, "{kind}: {text}");
        assert_eq!(outcome(&text, "step 1"), "unverified", "{kind}: {text}");
        let line = text.lines().find(|l| l.starts_with("step 1: ")).unwrap();
        assert_eq!(line.matches(kind).count(), 1, "per-file {kind}: {line}");

        let repo = suite_repo(body, None, 60);
        let (code, text) = check(&repo);
        assert_eq!(code, 1, "{kind}: {text}");
        assert_eq!(outcome(&text, "suite"), "unverified", "{kind}: {text}");
        assert_eq!(suite_line(&text).matches(kind).count(), 1, "suite {kind}: {text}");
    }
}

// STP-28 step 23 (AC-41): a `stapel.toml` change after step 2's RED is the fault commit of step 2
// in the hook and in `next`, though HEAD lies later.
#[test]
fn config_fault_commit() {
    let repo = junit_repo();
    let dir = repo.dir();
    set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | WHEN a thing happens THE command does it. | `x::one` |",
    );
    set_section(dir, "ABC-1", "Design", "The design.");
    set_section(dir, "ABC-1", "Proof", "The proof.");
    set_section(dir, "ABC-1", "Plan", "Route: full, auto.\n\n1. One (AC-1).\n2. Two (AC-1).");
    for stage in ["description", "spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", stage]).assert().success();
    }
    commit(dir, "ABC-1: the ticket is confirmed");
    add_step(dir, 1, "x", "one");
    write(
        dir,
        "__tests__/y.test.ts",
        "it(\"two\", () => { /* needs src/two.ts */ });\n",
    );
    commit(dir, "ABC-1 step 2 RED: two");
    bump_config(dir, 61);
    commit(dir, "ABC-1: a configuration change");
    let fault = git_in(dir, &["rev-parse", "HEAD"]).trim()[..7].to_string();
    write(dir, "src/two.ts", "export const two = 2;\n");
    commit(dir, "ABC-1 step 2 GREEN: two");
    let head = git_in(dir, &["rev-parse", "HEAD"]).trim()[..7].to_string();
    assert_ne!(fault, head);

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
        context.lines().any(|l| l
            == format!(
                "step 2: unverified: build-input-changed: .stapel/stapel.toml at {fault}"
            )),
        "{context}"
    );
    assert!(
        context.contains(&format!("{fault} is the fault commit"))
            || context.contains(&format!("stapel: {fault}")),
        "{context}"
    );

    let out = stapel(dir).arg("next").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let want = format!(
        "blocked: step 2: unverified at {fault} — build-input-changed: .stapel/stapel.toml"
    );
    assert!(text.lines().any(|l| l.starts_with(&want)), "{want}\n{text}");
}

/// A junit repository whose `command` starts `word(root)` and whose committed `run.sh` runs the
/// stand-in and logs its version (`v1`) to `script.log` of the tools folder; `a/keep` makes `a/..`
/// resolve. With `later` a commit after the step changes the log word to `v2`, so the text of HEAD
/// (the working tree) differs from the text at the step's commits.
fn absolute_script_repo(word: impl Fn(&Path) -> String, later: bool) -> JunitRepo {
    use std::os::unix::fs::PermissionsExt;
    let repo = junit_repo();
    let dir = repo.dir();
    let stand_in = repo_stand_in(dir);
    let log = repo.tools.path().join("script.log");
    let script = |v: &str| {
        format!(
            "#!/bin/sh\necho {v} >> '{}'\nexec sh '{stand_in}' \"$@\"\n",
            log.display()
        )
    };
    write(dir, "a/keep", "x\n");
    write(dir, ".gitignore", "node_modules/\nlate.sh\n");
    write(dir, "run.sh", &script("v1"));
    std::fs::set_permissions(dir.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = dir.join(".stapel/stapel.toml");
    let text: String = std::fs::read_to_string(&config)
        .unwrap()
        .lines()
        .map(|l| {
            if l.starts_with("command = ") {
                format!("command = \"{} {{file}}\"\n", word(dir))
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    std::fs::write(&config, text).unwrap();
    commit(dir, "setup: an absolute script");
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one");
    if later {
        write(dir, "run.sh", &script("v2"));
        commit(dir, "ABC-1: a later script text");
    }
    repo
}

/// The versions the script logged, in order.
fn script_log(repo: &JunitRepo) -> Vec<String> {
    std::fs::read_to_string(repo.tools.path().join("script.log"))
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect()
}

// AC-6: an absolute script under the root runs as committed; other absolute paths run as given.
#[test]
fn absolute_script_runs_at_step_commit() {
    // Mapped: the text at each commit runs, `v1` at RED and GREEN, `v2` at HEAD (once per run).
    let repo = absolute_script_repo(|d| format!("{}/run.sh", d.display()), true);
    let (_, text) = check(&repo);
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    let mut log = script_log(&repo);
    log.sort();
    assert_eq!(log, ["v1", "v1", "v2", "v2"], "{text}");
    // Not mapped: a `..` part and an empty part run the working-tree text, `v2`, at every commit.
    let words: [fn(&Path) -> String; 2] = [
        |d| format!("{}/a/../run.sh", d.display()),
        |d| format!("{}//run.sh", d.display()),
    ];
    for word in words {
        let repo = absolute_script_repo(word, true);
        let (_, text) = check(&repo);
        assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
        let log = script_log(&repo);
        assert!(!log.is_empty() && log.iter().all(|v| v == "v2"), "{log:?}");
    }
    // A path outside the root runs as given.
    let repo = absolute_script_repo(|d| format!("/bin/sh {}", repo_stand_in(d)), true);
    let (_, text) = check(&repo);
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
}

// AC-18: an absolute script under the root that is untracked cannot start: unverified.
#[test]
fn absolute_script_missing_at_commit() {
    use std::os::unix::fs::PermissionsExt;
    let repo = absolute_script_repo(|d| format!("{}/late.sh", d.display()), false);
    let dir = repo.dir();
    let stand_in = repo.tools.path().join("stand-in.sh").display().to_string();
    // Untracked and ignored, so absent at every commit, present in the working tree.
    write(dir, "late.sh", &format!("#!/bin/sh\nexec sh '{stand_in}' \"$@\"\n"));
    std::fs::set_permissions(dir.join("late.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let (code, text) = check(&repo);
    assert_eq!(code, 1, "{text}");
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    assert!(step_one(&text).contains("did not start"), "{text}");
    assert!(repo.runs().is_empty(), "{:?}", repo.runs());
}

// STP-30 AC-10: a `stapel.toml` that exists and fails to load stops `check --history`, `lint` and
// `next` with the load fault; none judges a step. An absent file keeps today's message.
#[test]
fn broken_config_is_refused() {
    let broken: [(&str, &str, &str); 2] = [
        ("a syntax error", "[check\nrunner = junit\n", "stapel.toml"),
        ("an unknown runner", "", "runner"),
    ];
    for (what, body, fault) in broken {
        let repo = junit_repo();
        let dir = repo.dir();
        let path = dir.join(".stapel/stapel.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("runner = \"junit\""), "{text}");
        std::fs::remove_file(&path).unwrap();
        match what {
            "an unknown runner" => std::fs::write(
                &path,
                text.replace("runner = \"junit\"", "runner = \"gradle\""),
            )
            .unwrap(),
            _ => std::fs::write(&path, body).unwrap(),
        }
        for (args, want) in [
            (vec!["check", "--history"], 2),
            (vec!["lint"], 2),
            (vec!["next"], 1),
        ] {
            let (code, text) = stdout(dir, &args);
            assert_eq!(code, want, "{what}: {args:?}: {text}");
            assert!(text.contains(fault), "{what}: {args:?}: {text}");
            assert!(
                !text.contains("run stapel init"),
                "{what}: {args:?} reads it as absent: {text}"
            );
            assert!(!text.contains("step 1"), "{what}: {args:?}: {text}");
        }
    }
    // A folder in place of the file, and an absent file, keep today's refusal.
    let repo = junit_repo();
    let dir = repo.dir();
    std::fs::remove_file(dir.join(".stapel/stapel.toml")).unwrap();
    std::fs::create_dir(dir.join(".stapel/stapel.toml")).unwrap();
    for args in [vec!["check", "--history"], vec!["lint"]] {
        let (code, text) = stdout(dir, &args);
        assert_eq!(code, 2, "a folder: {args:?}: {text}");
        assert!(
            text.contains("no .stapel/stapel.toml here or above"),
            "a folder: {args:?}: {text}"
        );
    }
    let (code, text) = stdout(dir, &["next"]);
    assert_eq!(code, 1, "a folder: next: {text}");
    assert!(
        text.contains("no .stapel/stapel.toml here or above"),
        "a folder: next: {text}"
    );
    let repo = junit_repo();
    let dir = repo.dir();
    std::fs::remove_file(dir.join(".stapel/stapel.toml")).unwrap();
    for args in [vec!["check", "--history"], vec!["lint"]] {
        let (code, text) = stdout(dir, &args);
        assert_eq!(code, 2, "{args:?}: {text}");
        assert!(text.contains("no .stapel/stapel.toml here or above"), "{args:?}: {text}");
    }
}

// STP-30 AC-1, AC-2 (d2-1, d2-2, d2-3): a step test call at a merge of the base branch is compared
// by its value, and by its mode only for a file unit; a deletion by the merge keeps the protected text.
#[test]
fn base_merge_step_test_unit() {
    const FILE: &str = "__tests__/f.test.ts";
    const MAIN_TEXT: &str = "it(\"fixed\", () => { /* needs src/f.ts, main */ });\n";

    /// A ticket with `base_branch` main: step 1 adds the call `fixed`. For `equal`, `mode`, `third`
    /// and `first` main adds the same file with another call text and the merge resolves it by
    /// `kind`; for `deleted-same` and `deleted-other` main adds a note and the merge deletes the file,
    /// which a later commit adds again with the protected text or another one. Returns the history
    /// output and the last commit.
    fn scenario(kind: &str) -> (String, String) {
        let repo = junit_repo();
        let dir = repo.dir();
        let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_string();
        let path = dir.join(".stapel/tickets/ABC-1/state.json");
        let mut state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        state["base_branch"] = serde_json::json!(main);
        std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
        commit(dir, "ABC-1: base branch");
        git_in(dir, &["checkout", "-q", "-b", "ticket"]);
        let protected = "it(\"fixed\", () => { /* needs src/f.ts */ });\n";
        write(dir, FILE, protected);
        commit(dir, "ABC-1 step 1 RED: fixed");
        write(dir, "src/f.ts", "export const f = 1;\n");
        commit(dir, "ABC-1 step 1 GREEN: fixed");
        let deleted = kind.starts_with("deleted");
        git_in(dir, &["checkout", "-q", &main]);
        if deleted {
            write(dir, "NOTES.md", "notes\n");
        } else {
            write(dir, FILE, MAIN_TEXT);
        }
        commit(dir, "main: a change");
        git_in(dir, &["checkout", "-q", "ticket"]);
        // A conflict of the merge is no failure here: the test resolves it.
        std::process::Command::new("git")
            .args(["merge", "-q", "--no-ff", "--no-commit", &main])
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        match kind {
            "equal" => write(dir, FILE, MAIN_TEXT),
            "mode" => {
                use std::os::unix::fs::PermissionsExt;
                write(dir, FILE, MAIN_TEXT);
                std::fs::set_permissions(dir.join(FILE), std::fs::Permissions::from_mode(0o755))
                    .unwrap();
            }
            "third" => write(
                dir,
                FILE,
                "it(\"fixed\", () => { /* needs src/f.ts, third */ });\n",
            ),
            "first" => write(dir, FILE, protected),
            _ => std::fs::remove_file(dir.join(FILE)).unwrap(),
        }
        commit(dir, "ABC-1: merge main");
        let mut last = git_in(dir, &["rev-parse", "HEAD"]).trim().to_string();
        if deleted {
            let again = if kind == "deleted-same" {
                protected
            } else {
                "it(\"fixed\", () => { /* needs src/f.ts, again */ });\n"
            };
            write(dir, FILE, again);
            commit(dir, "ABC-1: add the test file again");
            last = git_in(dir, &["rev-parse", "HEAD"]).trim().to_string();
        }
        let (_, text) = stdout(dir, &["check", "ABC-1", "--history"]);
        (text, last)
    }
    let step_one = |text: &str| {
        text.lines()
            .find(|l| l.starts_with("step 1: "))
            .unwrap_or_else(|| panic!("no step 1:\n{text}"))
            .to_string()
    };

    // Equal to the second parent, and a mode change with an equal blob: no `tests-changed`.
    for kind in ["equal", "mode"] {
        let (text, _) = scenario(kind);
        assert_eq!(step_one(&text), "step 1: needs-run", "{kind}: {text}");
    }

    // The first parent's text keeps the protected text.
    let (text, _) = scenario("first");
    assert_eq!(step_one(&text), "step 1: needs-run", "first: {text}");

    // A text that is not the second parent's: `tests-changed` at the merge.
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

// STP-30 d5-2: script edges of an absolute first word under the root.
#[test]
fn absolute_script_edges() {
    use std::os::unix::fs::PermissionsExt;
    let exec = |p: &Path| std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();

    // Present at HEAD, absent at the RED and the GREEN: the steps cannot start.
    let repo = absolute_script_repo(|d| format!("{}/late.sh", d.display()), false);
    let dir = repo.dir();
    let stand_in = repo.tools.path().join("stand-in.sh").display().to_string();
    write(dir, ".gitignore", "node_modules/\n");
    write(dir, "late.sh", &format!("#!/bin/sh\nexec sh '{stand_in}' \"$@\"\n"));
    exec(&dir.join("late.sh"));
    commit(dir, "ABC-1: the script arrives after the step");
    let (_, text) = check(&repo);
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    assert!(step_one(&text).contains("did not start"), "{text}");

    // Present at the step, deleted at HEAD: the HEAD run of the step and the suite run cannot start.
    let repo = absolute_script_repo(|d| format!("{}/run.sh", d.display()), false);
    let dir = repo.dir();
    std::fs::remove_file(dir.join("run.sh")).unwrap();
    commit(dir, "ABC-1: the script is deleted");
    let (_, text) = check(&repo);
    assert_eq!(outcome(&text, "step 1"), "unverified", "{text}");
    assert_eq!(outcome(&text, "suite"), "unverified", "{text}");
    assert!(suite_line(&text).contains("did not start"), "{text}");

    // A sibling whose name starts with the root runs as given, not at the step's commit.
    let repo = absolute_script_repo(|d| format!("{}2/run.sh", d.display()), true);
    let dir = repo.dir();
    let sibling = std::path::PathBuf::from(format!("{}2", dir.display()));
    let log = repo.tools.path().join("script.log");
    std::fs::create_dir_all(&sibling).unwrap();
    std::fs::write(
        sibling.join("run.sh"),
        format!(
            "#!/bin/sh\necho sibling >> '{}'\nexec sh '{}' \"$@\"\n",
            log.display(),
            repo.tools.path().join("stand-in.sh").display()
        ),
    )
    .unwrap();
    exec(&sibling.join("run.sh"));
    let (_, text) = check(&repo);
    let _ = std::fs::remove_dir_all(&sibling);
    assert_eq!(outcome(&text, "step 1"), "pass", "{text}");
    let seen = script_log(&repo);
    assert!(!seen.is_empty() && seen.iter().all(|v| v == "sibling"), "{seen:?}");
}

// STP-30 AC-10 (d8-1, d8-2): an unreadable file and a validation error give the load fault; a
// dangling link and an absent file give today's refusal; `next` judges no step in any of them.
#[test]
fn broken_config_kinds() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    const REFUSAL: &str = "no .stapel/stapel.toml here or above";
    for what in ["unreadable", "validation", "dangling", "absent"] {
        let repo = junit_repo();
        let dir = repo.dir();
        let path = dir.join(".stapel/stapel.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("command = "), "{text}");
        let fault = match what {
            "unreadable" => {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
                "stapel.toml"
            }
            "validation" => {
                // The line that starts with `command = `, not a commented `# command = …` line.
                let at = text.find("\ncommand = ").unwrap() + 1;
                let end = at + text[at..].find('\n').unwrap();
                let mut changed = text.clone();
                changed.replace_range(at..end, "command = \"sh run.sh\"");
                std::fs::write(&path, changed).unwrap();
                "{file}"
            }
            "dangling" => {
                std::fs::remove_file(&path).unwrap();
                symlink(dir.join("nowhere.toml"), &path).unwrap();
                REFUSAL
            }
            _ => {
                std::fs::remove_file(&path).unwrap();
                REFUSAL
            }
        };
        for (args, want) in [
            (vec!["check", "--history"], 2),
            (vec!["lint"], 2),
            (vec!["next"], 1),
        ] {
            let (code, out) = stdout(dir, &args);
            assert_eq!(code, want, "{what}: {args:?}: {out}");
            assert!(out.contains(fault), "{what}: {args:?}: {out}");
            if what == "unreadable" || what == "validation" {
                assert!(!out.contains(REFUSAL), "{what}: {args:?}: {out}");
            }
            assert!(!out.contains("step 1"), "{what}: {args:?}: {out}");
            assert!(!out.contains("pass"), "{what}: {args:?}: {out}");
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).ok();
    }
}
