//! STP-14 AC-11 to AC-16 and AC-18: `stapel hook post-tool-use`, the history check after an
//! agent's commit.

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
/// committed. The hook runs no cargo, so there is no lock file.
fn fixture() -> TempDir {
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

/// Appends a library function.
fn add_fn(dir: &Path, name: &str) {
    let lib = read(dir, "crates/tiny/src/lib.rs");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &format!("{lib}\npub fn {name}() -> u32 {{\n    1\n}}\n"),
    );
}

/// A correct RED for `label`: it adds the test `name` in `crates/tiny/tests/<name>.rs`.
fn red(dir: &Path, label: &str, name: &str) {
    add_test(dir, name, name, &format!("assert_eq!(tiny::{name}(), 1);"));
    commit(dir, &format!("ABC-1 {label} RED: {name} is missing"));
}

/// A GREEN for `label` that adds the library function `name`.
fn green(dir: &Path, label: &str, name: &str) {
    add_fn(dir, name);
    commit(dir, &format!("ABC-1 {label} GREEN: {name}"));
}

/// A clean pair for `label`: the RED adds a test that fails, the GREEN adds the function.
fn clean_pair(dir: &Path, label: &str, name: &str) {
    red(dir, label, name);
    green(dir, label, name);
}

/// Two REDs, then their GREENs: the first step reads `interleaved`.
fn interleaved_pairs(dir: &Path, first: &str, second: &str) {
    red(dir, first, "outer");
    red(dir, second, "inner");
    green(dir, first, "outer");
    green(dir, second, "inner");
}

/// The first seven characters of the id of `rev`.
fn sha(dir: &Path, rev: &str) -> String {
    git(dir, &["rev-parse", rev]).trim()[..7].to_string()
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

/// A PostToolUse payload of a Bash call; `cwd` only when given.
fn payload(cwd: Option<&Path>, command: &str) -> String {
    let mut v = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": command, "description": "test" },
        "tool_response": { "stdout": "", "stderr": "" },
    });
    if let Some(cwd) = cwd {
        v["cwd"] = serde_json::json!(cwd);
    }
    v.to_string()
}

/// `stapel hook post-tool-use` in the working directory `wd`, with `CLAUDE_PROJECT_DIR` only when
/// given: exit code, standard output and standard error.
fn run_hook(wd: &Path, project: Option<&Path>, stdin: &str) -> (i32, String, String) {
    let mut cmd = stapel(wd);
    cmd.args(["hook", "post-tool-use"])
        .write_stdin(stdin.to_string());
    if let Some(project) = project {
        cmd.env("CLAUDE_PROJECT_DIR", project);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The hook after `git commit -m x` in `dir`.
fn hook(dir: &Path) -> (i32, String, String) {
    run_hook(dir, None, &payload(Some(dir), "git commit -m x"))
}

/// The hook exits 0 and prints nothing.
fn assert_silent(result: (i32, String, String), what: &str) {
    let (code, out, err) = result;
    assert_eq!(code, 0, "{what}: {out}{err}");
    assert_eq!(out, "", "{what}");
    assert_eq!(err, "", "{what}");
}

/// The hook exits 2 with nothing on stdout; returns stderr.
fn assert_fault(result: (i32, String, String), what: &str) -> String {
    let (code, out, err) = result;
    assert_eq!(code, 2, "{what}: {out}{err}");
    assert_eq!(out, "", "{what}");
    assert_eq!(
        err.lines().last(),
        Some("details: stapel check ABC-1 --history"),
        "{what}:\n{err}"
    );
    err
}

/// The advice lines of the hook's output.
fn advice(err: &str) -> Vec<String> {
    err.lines()
        .filter(|l| l.starts_with("stapel: "))
        .map(String::from)
        .collect()
}

fn amend(sha: &str, labels: &str) -> String {
    format!("stapel: {sha} is HEAD; fix it with git commit --amend ({labels})")
}

fn ask(sha: &str, labels: &str) -> String {
    format!(
        "stapel: {sha} is below HEAD or an amend cannot fix it; stop and ask the human ({labels})"
    )
}

/// A ticket whose step 2 has a RED without a test, below a GREEN at `HEAD`: a hook fault.
fn failing() -> TempDir {
    let repo = fixture();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    commit(dir, "ABC-1 step 2 RED: no test");
    commit(dir, "ABC-1 step 2 GREEN: code");
    repo
}

/// A ticket with one clean pair.
fn clean() -> TempDir {
    let repo = fixture();
    clean_pair(repo.path(), "step 1", "one");
    repo
}

/// Marks the ticket ABC-1 closed and commits it.
fn close(dir: &Path) {
    let mut state = common::state_json(dir, "ABC-1");
    state["closed"] =
        serde_json::json!({"by": "test-user", "at": "2026-01-02T00:00:00Z", "reason": "done"});
    write(
        dir,
        ".stapel/tickets/ABC-1/state.json",
        &format!("{}\n", serde_json::to_string_pretty(&state).unwrap()),
    );
    commit(dir, "ABC-1: close");
}

// ---- AC-11 ----

#[test]
fn failing_history_exits_2_with_advice() {
    let repo = fixture();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    red(dir, "step 2", "two");
    add_test(dir, "two", "two_again", "assert!(true);");
    commit(dir, "ABC-1 step 2 RED: two again");
    let second = sha(dir, "HEAD");
    green(dir, "step 2", "two");
    commit(dir, "ABC-1 step 3 RED: no test");
    let err = assert_fault(hook(dir), "golden");
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/hook_post_fail.txt"),
    )
    .unwrap();
    assert_eq!(without_ids(&err), golden, "actual:\n{err}");
    assert_eq!(
        advice(&err),
        vec![ask(&second, "step 2"), amend(&sha(dir, "HEAD"), "step 3")]
    );
}

#[test]
fn advice_follows_the_fault_commit() {
    /// A fresh ticket built by `build`, which returns the advice lines the hook must print.
    fn case(what: &str, build: impl FnOnce(&Path) -> Vec<String>) -> String {
        let repo = fixture();
        let dir = repo.path();
        let want = build(dir);
        let err = assert_fault(hook(dir), what);
        assert_eq!(advice(&err), want, "{what}:\n{err}");
        err
    }
    fn has(err: &str, line: &str, what: &str) {
        assert!(
            err.lines().any(|l| l == line),
            "{what}: no {line:?}:\n{err}"
        );
    }

    // `unpaired: no RED commit`: the step's first GREEN.
    let err = case("no RED at HEAD", |dir| {
        green(dir, "step 1", "one");
        vec![ask(&sha(dir, "HEAD"), "step 1")]
    });
    has(&err, "step 1: unpaired: no RED commit", "no RED at HEAD");
    case("no RED below HEAD", |dir| {
        green(dir, "step 1", "one");
        let fault = sha(dir, "HEAD");
        clean_pair(dir, "step 2", "two");
        vec![ask(&fault, "step 1")]
    });

    // `unpaired: the GREEN commit comes before the RED commit`: the step's first RED.
    let err = case("GREEN before RED, RED at HEAD", |dir| {
        green(dir, "step 1", "one");
        red(dir, "step 1", "one");
        vec![ask(&sha(dir, "HEAD"), "step 1")]
    });
    has(
        &err,
        "step 1: unpaired: the GREEN commit comes before the RED commit",
        "GREEN before RED",
    );
    case("GREEN before RED, RED below HEAD", |dir| {
        green(dir, "step 1", "one");
        red(dir, "step 1", "one");
        let fault = sha(dir, "HEAD");
        clean_pair(dir, "step 2", "two");
        vec![ask(&fault, "step 1")]
    });

    // `unpaired: no GREEN commit`, closed by another step's RED: that RED.
    let err = case("no GREEN, closed at HEAD", |dir| {
        red(dir, "step 1", "one");
        red(dir, "step 2", "two");
        vec![ask(&sha(dir, "HEAD"), "step 1")]
    });
    has(&err, "step 1: unpaired: no GREEN commit", "no GREEN");
    assert!(!err.contains("step 2:"), "an open RED is no fault:\n{err}");
    case("no GREEN, closed below HEAD", |dir| {
        red(dir, "step 1", "one");
        red(dir, "step 2", "two");
        let fault = sha(dir, "HEAD");
        green(dir, "step 2", "two");
        vec![ask(&fault, "step 1")]
    });

    // `duplicate`: the step's second RED or GREEN, whichever comes first.
    let err = case("duplicate at HEAD", |dir| {
        clean_pair(dir, "step 1", "one");
        add_test(dir, "one", "one_again", "assert!(true);");
        commit(dir, "ABC-1 step 1 RED: one again");
        vec![ask(&sha(dir, "HEAD"), "step 1")]
    });
    has(
        &err,
        "step 1: duplicate: 2 RED and 1 GREEN commits",
        "duplicate",
    );
    case("duplicate below HEAD", |dir| {
        red(dir, "step 1", "one");
        add_test(dir, "one", "one_again", "assert!(true);");
        commit(dir, "ABC-1 step 1 RED: one again");
        let fault = sha(dir, "HEAD");
        green(dir, "step 1", "one");
        vec![ask(&fault, "step 1")]
    });

    // `no-tests`: the RED; at HEAD only as an open RED.
    let err = case("no-tests at HEAD", |dir| {
        commit(dir, "ABC-1 step 1 RED: no test");
        vec![amend(&sha(dir, "HEAD"), "step 1")]
    });
    has(
        &err,
        "step 1: no-tests: the RED commit adds or changes no test",
        "no-tests",
    );
    case("no-tests below HEAD", |dir| {
        commit(dir, "ABC-1 step 1 RED: no test");
        let fault = sha(dir, "HEAD");
        commit(dir, "ABC-1 step 1 GREEN: code");
        vec![ask(&fault, "step 1")]
    });

    // `red-changes-code`: the RED; seen after the GREEN at HEAD it names the RED, not HEAD.
    case("red-changes-code at HEAD", |dir| {
        add_test(dir, "one", "one", "assert_eq!(tiny::one(), 1);");
        add_fn(dir, "one");
        commit(dir, "ABC-1 step 1 RED: test and code");
        vec![amend(&sha(dir, "HEAD"), "step 1")]
    });
    let err = case("red-changes-code below the GREEN at HEAD", |dir| {
        add_test(dir, "one", "one", "assert_eq!(tiny::one(), 1);");
        add_fn(dir, "one");
        commit(dir, "ABC-1 step 1 RED: test and code");
        let fault = sha(dir, "HEAD");
        commit(dir, "ABC-1 step 1 GREEN: green");
        vec![ask(&fault, "step 1")]
    });
    has(
        &err,
        "step 1: red-changes-code: crates/tiny/src/lib.rs",
        "red-changes-code",
    );

    // `tests-changed`: where the walk stops; another step's GREEN at HEAD allows an amend.
    let err = case("tests-changed by a GREEN at HEAD", |dir| {
        clean_pair(dir, "step 1", "one");
        red(dir, "step 2", "two");
        add_fn(dir, "two");
        edit(
            dir,
            "crates/tiny/tests/one.rs",
            "assert_eq!(tiny::one(), 1);",
            "assert_eq!(tiny::one(), 1 + 0);",
        );
        commit(dir, "ABC-1 step 2 GREEN: two, and edits the test of step 1");
        vec![amend(&sha(dir, "HEAD"), "step 1")]
    });
    has(
        &err,
        "step 1: tests-changed: tiny/one::one",
        "tests-changed",
    );
    case("tests-changed below HEAD", |dir| {
        clean_pair(dir, "step 1", "one");
        edit(
            dir,
            "crates/tiny/tests/one.rs",
            "assert_eq!(tiny::one(), 1);",
            "assert_eq!(tiny::one(), 1 + 0);",
        );
        commit(dir, "ABC-1: edits the test of step 1");
        let fault = sha(dir, "HEAD");
        clean_pair(dir, "step 2", "two");
        vec![ask(&fault, "step 1")]
    });
    {
        // Another step's RED that changes the test is no fault.
        let repo = fixture();
        let dir = repo.path();
        clean_pair(dir, "step 1", "one");
        add_test(dir, "two", "two", "assert_eq!(tiny::two(), 1);");
        edit(
            dir,
            "crates/tiny/tests/one.rs",
            "assert_eq!(tiny::one(), 1);",
            "assert_eq!(tiny::one(), 1 + 0);",
        );
        commit(
            dir,
            "ABC-1 step 2 RED: two, and a new form of the test of step 1",
        );
        assert_silent(hook(dir), "a RED changes another step's test");
    }

    // `unverified: interleaved`: the other step's RED inside the pair, never HEAD.
    let err = case("interleaved", |dir| {
        interleaved_pairs(dir, "step 1", "step 2");
        vec![ask(&sha(dir, "HEAD~2"), "step 1")]
    });
    has(
        &err,
        "step 1: unverified: interleaved: step 2",
        "interleaved",
    );

    // `unverified: build-input-changed`: the changing commit, one line for every step it blocks.
    let lib_table = |dir: &Path| {
        edit(
            dir,
            "crates/tiny/Cargo.toml",
            "edition = \"2021\"\n",
            "edition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n",
        );
        commit(dir, "ABC-1: a lib table");
    };
    let err = case("build-input-changed at HEAD", |dir| {
        clean_pair(dir, "step 1", "one");
        clean_pair(dir, "step 2", "two");
        lib_table(dir);
        vec![amend(&sha(dir, "HEAD"), "step 1, step 2")]
    });
    has(
        &err,
        &format!(
            "step 2: unverified: build-input-changed: crates/tiny/Cargo.toml at {}",
            &advice(&err)[0][8..15]
        ),
        "build-input-changed",
    );
    case("build-input-changed below HEAD", |dir| {
        clean_pair(dir, "step 1", "one");
        clean_pair(dir, "step 2", "two");
        lib_table(dir);
        let fault = sha(dir, "HEAD");
        clean_pair(dir, "step 3", "three");
        vec![ask(&fault, "step 1, step 2, step 3")]
    });

    // `duplicate` and `tests-changed` on one commit at HEAD: one of them allows no amend.
    case("duplicate and tests-changed on HEAD", |dir| {
        clean_pair(dir, "step 1", "one");
        clean_pair(dir, "step 2", "two");
        edit(
            dir,
            "crates/tiny/tests/one.rs",
            "assert_eq!(tiny::one(), 1);",
            "assert_eq!(tiny::one(), 1 + 0);",
        );
        commit(
            dir,
            "ABC-1 step 2 GREEN: again, and edits the test of step 1",
        );
        vec![ask(&sha(dir, "HEAD"), "step 1, step 2")]
    });
}

// ---- AC-12 ----

#[test]
fn silent_without_faults() {
    let repo = fixture();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    clean_pair(dir, "step 2", "two");
    assert_silent(hook(dir), "clean ticket");
}

#[test]
fn silent_after_an_open_red() {
    let repo = fixture();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    red(dir, "step 2", "two");
    assert_silent(hook(dir), "an open RED at HEAD");
    commit(dir, "ABC-1: notes");
    assert_silent(hook(dir), "an open RED under a notes commit");

    // A correct RED followed by another step's RED is no longer open.
    let repo = fixture();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    red(dir, "step 2", "two");
    red(dir, "step 3", "three");
    let err = assert_fault(hook(dir), "RED then another RED");
    assert!(
        err.lines()
            .any(|l| l == "step 2: unpaired: no GREEN commit"),
        "{err}"
    );

    // A step with two REDs, the second at HEAD.
    let repo = fixture();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    red(dir, "step 2", "two");
    add_test(dir, "two", "two_again", "assert!(true);");
    commit(dir, "ABC-1 step 2 RED: two again");
    assert_fault(hook(dir), "two REDs");

    // A step with a GREEN, then a RED at HEAD.
    let repo = fixture();
    let dir = repo.path();
    clean_pair(dir, "step 1", "one");
    green(dir, "step 2", "two");
    red(dir, "step 2", "two");
    assert_fault(hook(dir), "GREEN then RED");
}

// ---- AC-18 ----

#[test]
fn open_red_is_checked() {
    /// Step 1 is a clean pair; `make_red` commits step 2's RED; the hook must print `line`.
    fn case(what: &str, setup: impl Fn(&Path), make_red: impl Fn(&Path), line: &str) {
        let repo = fixture();
        let dir = repo.path();
        setup(dir);
        clean_pair(dir, "step 1", "one");
        make_red(dir);
        let red_sha = sha(dir, "HEAD");
        let err = assert_fault(hook(dir), what);
        assert!(
            err.lines().any(|l| l == line),
            "{what}: no {line:?}:\n{err}"
        );
        assert!(!err.contains("step 1:"), "{what}:\n{err}");
        assert_eq!(advice(&err), vec![amend(&red_sha, "step 2")], "{what}");
        // The history check still reads the open RED as unpaired.
        let out = stapel(dir)
            .args(["check", "ABC-1", "--history"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            text.lines()
                .any(|l| l == "step 2: unpaired: no GREEN commit"),
            "{what}:\n{text}"
        );
        // Under a notes commit the RED is no longer HEAD.
        commit(dir, "ABC-1: notes");
        let err = assert_fault(hook(dir), what);
        assert!(
            err.lines().any(|l| l == line),
            "{what}: no {line:?}:\n{err}"
        );
        assert_eq!(advice(&err), vec![ask(&red_sha, "step 2")], "{what}");
    }

    case(
        "no test",
        |_| {},
        |dir| commit(dir, "ABC-1 step 2 RED: no test"),
        "step 2: no-tests: the RED commit adds or changes no test",
    );
    case(
        "a source file",
        |_| {},
        |dir| {
            add_test(dir, "two", "two", "assert_eq!(tiny::two(), 1);");
            add_fn(dir, "two");
            commit(dir, "ABC-1 step 2 RED: test and code");
        },
        "step 2: red-changes-code: crates/tiny/src/lib.rs",
    );
    case(
        "an included file",
        |dir| {
            write(
                dir,
                "crates/tiny/src/seed.rs",
                "pub const SEED: &str = include_str!(\"../tests/data/seed.txt\");\n",
            );
            write(dir, "crates/tiny/tests/data/seed.txt", "seed\n");
            commit(dir, "setup: a source file includes test data");
        },
        |dir| {
            add_test(dir, "two", "two", "assert_eq!(tiny::two(), 1);");
            write(dir, "crates/tiny/tests/data/seed.txt", "changed\n");
            commit(dir, "ABC-1 step 2 RED: test and included data");
        },
        "step 2: red-changes-code: crates/tiny/tests/data/seed.txt",
    );
}

// ---- AC-13 ----

#[test]
fn silent_cases() {
    let repo = failing();
    let dir = repo.path();
    // The fixture fails when the hook runs.
    let err = assert_fault(hook(dir), "control");
    assert!(
        err.lines()
            .any(|l| l == "step 2: no-tests: the RED commit adds or changes no test"),
        "{err}"
    );

    let edit_tool = serde_json::json!({
        "tool_name": "Edit",
        "cwd": dir,
        "tool_input": { "command": "git commit -m x", "file_path": "x" },
    })
    .to_string();
    assert_silent(run_hook(dir, None, &edit_tool), "tool Edit");
    assert_silent(
        run_hook(dir, None, &payload(Some(dir), "git status")),
        "git status",
    );
    assert_silent(run_hook(dir, None, "not json"), "not json");
    assert_silent(run_hook(dir, None, ""), "empty stdin");
    let no_tool = serde_json::json!({
        "cwd": dir,
        "tool_input": { "command": "git commit -m x" },
    })
    .to_string();
    assert_silent(run_hook(dir, None, &no_tool), "no tool_name");
    let no_command = serde_json::json!({
        "tool_name": "Bash",
        "cwd": dir,
        "tool_input": { "description": "x" },
    })
    .to_string();
    assert_silent(run_hook(dir, None, &no_command), "no command");
    assert_silent(
        run_hook(dir, None, &payload(Some(dir), "")),
        "empty command",
    );
    let unreadable = format!("git commit -m x; echo {}", "a".repeat(70 * 1024));
    assert_silent(
        run_hook(dir, None, &payload(Some(dir), &unreadable)),
        "unreadable command",
    );

    // A shallow clone of the failing history.
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
    assert_silent(hook(&clone), "shallow clone");

    // The subject of HEAD names no ticket, or a key without a folder.
    commit(dir, "notes without a key");
    assert_silent(hook(dir), "subject without a key");
    commit(dir, "XYZ-9: another ticket");
    assert_silent(hook(dir), "key of no folder");

    // A closed failing ticket.
    let repo = failing();
    let dir = repo.path();
    close(dir);
    assert_silent(hook(dir), "closed ticket");

    // A ticket without step commits.
    let repo = fixture();
    let dir = repo.path();
    commit(dir, "ABC-1: notes");
    assert_silent(hook(dir), "no steps");

    // An unborn HEAD.
    let repo = common::git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    assert_silent(hook(dir), "unborn HEAD");
}

// ---- AC-14 ----

#[test]
fn silent_during_a_rebase() {
    let repo = failing();
    let dir = repo.path();
    assert_fault(hook(dir), "control");
    let head = git(dir, &["rev-parse", "HEAD"]);
    for (name, is_dir) in [
        ("rebase-merge", true),
        ("rebase-apply", true),
        ("MERGE_HEAD", false),
        ("CHERRY_PICK_HEAD", false),
        ("REVERT_HEAD", false),
    ] {
        let path = dir.join(".git").join(name);
        if is_dir {
            std::fs::create_dir_all(&path).unwrap();
        } else {
            std::fs::write(&path, &head).unwrap();
        }
        assert_silent(hook(dir), name);
        if is_dir {
            std::fs::remove_dir_all(&path).unwrap();
        } else {
            std::fs::remove_file(&path).unwrap();
        }
    }
    assert_fault(hook(dir), "after the states are gone");

    // A rebase in a linked worktree, with `cwd` there.
    let away = tempfile::tempdir().unwrap();
    let wt = away.path().join("wt");
    git(
        dir,
        &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "wt"],
    );
    let wt_payload = payload(Some(&wt), "git commit -m x");
    assert_fault(run_hook(dir, None, &wt_payload), "worktree control");
    let git_dir = git(&wt, &["rev-parse", "--absolute-git-dir"]);
    std::fs::create_dir_all(Path::new(git_dir.trim()).join("rebase-merge")).unwrap();
    assert_silent(run_hook(dir, None, &wt_payload), "rebase in a worktree");
}

// ---- AC-15 ----

#[test]
fn commit_command_forms() {
    let repo = failing();
    let dir = repo.path();
    let heredoc = "git commit -m \"$(cat <<'EOF'\nABC-1 step 3 RED: a test\n\n\
                   Co-Authored-By: someone <someone@example.invalid>\nEOF\n)\"";
    let runs = [
        "git commit -m x",
        "git commit -a -m x -m y",
        heredoc,
        "git commit --amend --no-edit",
        "git -C . commit -m x",
        "cd . && git commit -m x",
        "bash -c 'git commit -m x'",
        "git -c user.name=a commit -m x",
    ];
    for command in runs {
        assert_fault(run_hook(dir, None, &payload(Some(dir), command)), command);
    }
    let skips = [
        "git log --grep commit",
        "echo git commit",
        "git commit-tree HEAD^{tree} -m x",
        "git status",
    ];
    for command in skips {
        assert_silent(run_hook(dir, None, &payload(Some(dir), command)), command);
    }
}

// ---- AC-16 ----

#[test]
fn reads_the_hook_repository() {
    let bad = failing();
    let good = clean();
    let other = clean();
    let (bad, good, other) = (bad.path(), good.path(), other.path());
    let commit_cmd = "git commit -m x";

    // `cwd` comes first.
    assert_fault(
        run_hook(other, Some(good), &payload(Some(bad), commit_cmd)),
        "cwd failing",
    );
    assert_silent(
        run_hook(bad, Some(bad), &payload(Some(good), commit_cmd)),
        "cwd clean, the others failing",
    );
    // Without `cwd`, `CLAUDE_PROJECT_DIR`.
    assert_fault(
        run_hook(good, Some(bad), &payload(None, commit_cmd)),
        "CLAUDE_PROJECT_DIR failing",
    );
    assert_silent(
        run_hook(bad, Some(good), &payload(None, commit_cmd)),
        "CLAUDE_PROJECT_DIR clean, the working directory failing",
    );
    // Without both, the working directory.
    assert_fault(
        run_hook(bad, None, &payload(None, commit_cmd)),
        "working directory failing",
    );
    assert_silent(
        run_hook(good, None, &payload(None, commit_cmd)),
        "working directory clean",
    );

    // A linked worktree whose HEAD fails while the main checkout is clean.
    let away = tempfile::tempdir().unwrap();
    let wt = away.path().join("wt");
    git(
        good,
        &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "wt"],
    );
    commit(&wt, "ABC-1 step 2 RED: no test");
    commit(&wt, "ABC-1 step 2 GREEN: code");
    assert_silent(hook(good), "main checkout clean");
    assert_fault(
        run_hook(good, Some(good), &payload(Some(&wt), commit_cmd)),
        "cwd in a failing worktree",
    );

    // `cwd` outside any stapel repository: no fallback.
    let outside = common::bare_dir();
    assert_silent(
        run_hook(bad, Some(bad), &payload(Some(outside.path()), commit_cmd)),
        "cwd outside",
    );
}
