//! STP-16: a closed output pipe ends a command without a panic, with its writes done.

mod common;

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Which output is a closed pipe in a run.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Open,
    StdoutClosed,
    StderrClosed,
}

/// An end before every message of `fixtures/main.jsonl`'s day: a fresh checkout gives the fixture a
/// new modification time, and `tokens import` refuses a recent file without `--until`.
const UNTIL: &str = "2026-01-02T00:00:00Z";

/// One command line with its environment and standard input.
#[derive(Clone)]
struct Case {
    name: &'static str,
    args: Vec<String>,
    stdin: String,
    env: Vec<(&'static str, String)>,
}

fn case(name: &'static str, args: &[&str]) -> Case {
    Case {
        name,
        args: args.iter().map(|a| a.to_string()).collect(),
        stdin: String::new(),
        env: Vec::new(),
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Runs `stapel` in `dir` with the environment of `common::stapel`. A closed output is the
/// write end of a pipe whose reader is dropped before the child starts, so every write fails.
fn run(dir: &Path, case: &Case, mode: Mode) -> Run {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("stapel"));
    cmd.args(&case.args)
        .current_dir(dir)
        .env(
            "GIT_CEILING_DIRECTORIES",
            std::env::temp_dir().canonicalize().unwrap(),
        )
        .env_remove("STAPEL_ASSUME_TTY")
        .env_remove("CLAUDE_PROJECT_DIR")
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .env_remove("CLAUDE_CODE_EXECPATH")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    for (k, v) in &case.env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::piped());
    match mode {
        Mode::Open => {
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        }
        Mode::StdoutClosed => {
            let (reader, writer) = std::io::pipe().unwrap();
            drop(reader);
            cmd.stdout(writer).stderr(Stdio::piped());
        }
        Mode::StderrClosed => {
            let (reader, writer) = std::io::pipe().unwrap();
            drop(reader);
            cmd.stdout(Stdio::piped()).stderr(writer);
        }
    }
    let mut child = cmd.spawn().unwrap();
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().unwrap();
        // The child may end before it reads its input.
        let _ = stdin.write_all(case.stdin.as_bytes());
    }
    // `cmd` holds the writer of the closed output; it must not outlive the spawn.
    drop(cmd);
    let out = child.wait_with_output().unwrap();
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// A copy of a fixture directory, made with `cp -a` after the fixture is built.
fn copy_of(dir: &Path) -> TempDir {
    let copy = tempfile::tempdir().unwrap();
    let status = Command::new("cp")
        .arg("-a")
        .arg(format!("{}/.", dir.display()))
        .arg(copy.path())
        .status()
        .unwrap();
    assert!(status.success());
    copy
}

/// The failure text when a run printed a panic message.
fn panic_failure(run: &Run, what: &str) -> Option<String> {
    [&run.stdout, &run.stderr]
        .into_iter()
        .find(|t| t.contains("panicked") || t.contains("Broken pipe"))
        .map(|t| format!("{what}: panic message in output:\n{t}"))
}

/// Runs `case` once open and once closed, each in its own copy of `fixture`; returns both
/// runs and both directories.
fn open_and_closed(fixture: &Path, case: &Case, closed: Mode) -> (Run, Run, TempDir, TempDir) {
    let open_dir = copy_of(fixture);
    let closed_dir = copy_of(fixture);
    let open = run(open_dir.path(), case, Mode::Open);
    let shut = run(closed_dir.path(), case, closed);
    (open, shut, open_dir, closed_dir)
}

/// Drops every `id` and `at` key at any depth.
fn strip(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("id");
            map.remove("at");
            map.values_mut().for_each(strip);
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        _ => {}
    }
}

/// Every file under `dir` but `.git/`: `.json` as a parsed value, `.jsonl` as parsed records
/// per line (ids and times dropped), other files as bytes.
fn tree(dir: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
                continue;
            }
            let rel = path.strip_prefix(root).unwrap().display().to_string();
            let bytes = std::fs::read(&path).unwrap();
            let text = match path.extension().and_then(|e| e.to_str()) {
                Some("json") => {
                    let mut v: Value = serde_json::from_slice(&bytes).unwrap();
                    strip(&mut v);
                    v.to_string()
                }
                Some("jsonl") => String::from_utf8(bytes)
                    .unwrap()
                    .lines()
                    .map(|l| {
                        let mut v: Value = serde_json::from_str(l).unwrap();
                        strip(&mut v);
                        v.to_string()
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => format!("{bytes:?}"),
            };
            out.insert(rel, text);
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn assert_same_tree(open: &Path, closed: &Path, what: &str) {
    let (a, b) = (tree(open), tree(closed));
    let names_a: Vec<_> = a.keys().collect();
    let names_b: Vec<_> = b.keys().collect();
    assert_eq!(names_a, names_b, "{what}: different files");
    for (name, text) in &a {
        assert_eq!(text, &b[name], "{what}: {name} differs");
    }
}

// ---- fixtures ----

fn fixture_path(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .display()
        .to_string()
}

fn hook_payload(dir: &Path, command: &str) -> String {
    serde_json::json!({
        "tool_name": "Bash",
        "cwd": dir,
        "permission_mode": "auto",
        "tool_input": { "command": command, "description": "test" },
    })
    .to_string()
}

/// The ticket repository, made stale: `spec` confirmed, then edited.
fn stale_repo() -> TempDir {
    let repo = common::repo_with_ticket();
    common::stapel(repo.path())
        .args(["ok", "spec"])
        .assert()
        .success();
    common::set_section(repo.path(), "ABC-1", "Spec", "The spec, edited.");
    repo
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
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
}

fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Commits everything with fixed author, committer and dates.
fn commit(dir: &Path, message: &str, n: u32) {
    let date = format!("2026-01-01T00:00:{n:02}Z");
    git(dir, &["add", "-A"]);
    let out = Command::new("git")
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

/// A one-crate cargo workspace with ticket ABC-1 and one RED/GREEN step, all committed.
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
        "pub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n\npub fn triple(x: u32) -> u32 {\n    x * 2\n}\n",
    );
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn starts() {\n    assert_eq!(tiny::add(1, 1), 2);\n}\n",
    );
    write(dir, ".gitignore", "/target\n");
    common::stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    common::stapel(dir)
        .args(["new", "First"])
        .assert()
        .success();
    let lock = Command::new("cargo")
        .args(["generate-lockfile", "--offline"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        lock.status.success(),
        "{}",
        String::from_utf8_lossy(&lock.stderr)
    );
    commit(dir, "initial", 0);
    let basic = std::fs::read_to_string(dir.join("crates/tiny/tests/basic.rs")).unwrap();
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        &format!("{basic}\n#[test]\nfn triples() {{\n    assert_eq!(tiny::triple(2), 6);\n}}\n"),
    );
    commit(dir, "ABC-1 step 1 RED: triple is wrong", 1);
    let lib = std::fs::read_to_string(dir.join("crates/tiny/src/lib.rs")).unwrap();
    write(
        dir,
        "crates/tiny/src/lib.rs",
        &lib.replace("x * 2", "x * 3"),
    );
    commit(dir, "ABC-1 step 1 GREEN: triple fixed", 2);
    repo
}

// ---- AC-1 ----

// STP-16 AC-1
#[test]
fn stdout_closed_keeps_exit_code() {
    let mut cases: Vec<(Case, TempDir)> = Vec::new();

    let r = common::git_repo();
    cases.push((case("init --prefix", &["init", "--prefix", "ABC"]), r));

    let r = common::git_repo();
    let mut c = case("init prompt", &["init"]);
    c.stdin = "STP\n".into();
    c.env.push(("STAPEL_ASSUME_TTY", "1".into()));
    cases.push((c, r));

    cases.push((case("new", &["new", "Second"]), common::stapel_repo()));
    cases.push((case("ok", &["ok", "spec"]), common::repo_with_ticket()));
    cases.push((case("status stale", &["status"]), stale_repo()));
    cases.push((
        case("close", &["close", "--reason", "done"]),
        common::repo_with_ticket(),
    ));

    let r = common::repo_with_ticket();
    common::stapel(r.path())
        .args([
            "tokens", "add", "--role", "research", "--model", "m-1", "--input", "1", "--output",
            "1",
        ])
        .assert()
        .success();
    cases.push((case("tokens", &["tokens"]), r));

    cases.push((
        case(
            "tokens add",
            &[
                "tokens", "add", "--role", "research", "--model", "m-1", "--input", "1",
                "--output", "1",
            ],
        ),
        common::repo_with_ticket(),
    ));
    let main = fixture_path("main.jsonl");
    cases.push((
        case(
            "tokens import",
            &[
                "tokens",
                "import",
                &main,
                "--role",
                "orchestrator",
                "--until",
                UNTIL,
            ],
        ),
        common::repo_with_ticket(),
    ));
    let session = fixture_path("session.jsonl");
    cases.push((
        case("tokens session", &["tokens", "session", &session]),
        common::repo_with_ticket(),
    ));
    cases.push((
        case("check --list", &["check", "ABC-1", "--list"]),
        cargo_repo(),
    ));

    let r = common::repo_with_ticket();
    let mut c = case("hook ask", &["hook", "pre-tool-use"]);
    c.stdin = hook_payload(r.path(), "stapel ok spec");
    cases.push((c, r));

    let mut failures = Vec::new();
    for (c, fixture) in &cases {
        let (open, shut, _a, _b) = open_and_closed(fixture.path(), c, Mode::StdoutClosed);
        if open.stdout.is_empty() {
            failures.push(format!("{}: the open run printed nothing", c.name));
        }
        if shut.code != open.code {
            failures.push(format!(
                "{}: exit code {} closed, {} open; stderr: {}",
                c.name, shut.code, open.code, shut.stderr
            ));
        }
        if shut.stderr.contains("panicked") || shut.stderr.contains("Broken pipe") {
            failures.push(format!("{}: panic message: {}", c.name, shut.stderr));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- AC-2 ----

// STP-16 AC-2
#[test]
fn stderr_closed_keeps_exit_code() {
    let mut cases: Vec<(Case, TempDir, i32)> = Vec::new();

    cases.push((
        case("unknown ticket", &["status", "NOPE-1"]),
        common::repo_with_ticket(),
        1,
    ));
    cases.push((case("empty title", &["new", ""]), common::stapel_repo(), 1));
    let missing = fixture_path("no-such-transcript.jsonl");
    cases.push((
        case(
            "missing transcript",
            &["tokens", "import", &missing, "--role", "orchestrator"],
        ),
        common::repo_with_ticket(),
        1,
    ));
    let r = common::repo_with_ticket();
    let mut c = case("hook deny", &["hook", "pre-tool-use"]);
    c.stdin = "not json".into();
    cases.push((c, r, 2));
    cases.push((
        case(
            "tokens add, role missing",
            &[
                "tokens", "add", "--role", "research", "--model", "m-1", "--input", "1",
                "--output", "1",
            ],
        ),
        common::repo_with_ticket(),
        0,
    ));

    let mut failures = Vec::new();
    for (c, fixture, want) in &cases {
        let (open, shut, _a, _b) = open_and_closed(fixture.path(), c, Mode::StderrClosed);
        if open.code != *want {
            failures.push(format!(
                "{}: open run gave {}, want {want}",
                c.name, open.code
            ));
        }
        if open.stderr.is_empty() {
            failures.push(format!("{}: the open run wrote nothing to stderr", c.name));
        }
        if shut.code != open.code {
            failures.push(format!(
                "{}: exit code {} closed, {} open; stdout: {}",
                c.name, shut.code, open.code, shut.stdout
            ));
        }
        if shut.stdout.contains("panicked") {
            failures.push(format!("{}: panic message: {}", c.name, shut.stdout));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- AC-3 ----

// STP-16 AC-3
#[test]
fn closed_output_keeps_every_write() {
    let main = fixture_path("main.jsonl");
    let session = fixture_path("session.jsonl");
    let add = [
        "tokens", "add", "--role", "research", "--model", "m-1", "--input", "1", "--output", "1",
    ];
    let mut cases: Vec<(Case, TempDir)> = Vec::new();
    cases.push((
        case("init --prefix", &["init", "--prefix", "ABC"]),
        common::git_repo(),
    ));
    let mut c = case("init prompt", &["init"]);
    c.stdin = "STP\n".into();
    c.env.push(("STAPEL_ASSUME_TTY", "1".into()));
    cases.push((c, common::git_repo()));
    cases.push((case("new", &["new", "Second"]), common::stapel_repo()));
    cases.push((case("ok", &["ok", "spec"]), common::repo_with_ticket()));
    cases.push((
        case("close", &["close", "--reason", "done"]),
        common::repo_with_ticket(),
    ));
    cases.push((case("tokens add", &add), common::repo_with_ticket()));
    cases.push((
        case(
            "tokens import",
            &[
                "tokens",
                "import",
                &main,
                "--role",
                "orchestrator",
                "--until",
                UNTIL,
            ],
        ),
        common::repo_with_ticket(),
    ));
    cases.push((
        case("tokens session", &["tokens", "session", &session]),
        common::repo_with_ticket(),
    ));

    let mut failures = Vec::new();
    for (c, fixture) in &cases {
        for mode in [Mode::StdoutClosed, Mode::StderrClosed] {
            let what = format!("{} with {mode:?}", c.name);
            let (open, shut, a, b) = open_and_closed(fixture.path(), c, mode);
            failures.extend(panic_failure(&shut, &what));
            if shut.code != open.code {
                failures.push(format!("{what}: exit code {} vs {}", shut.code, open.code));
            }
            if open.code != 0 {
                failures.push(format!("{what}: the open run failed: {}", open.stderr));
            }
            let (ta, tb) = (tree(a.path()), tree(b.path()));
            if ta == tree(fixture.path()) {
                failures.push(format!("{what}: the open run wrote nothing"));
            }
            if ta != tb {
                failures.push(format!("{what}: the trees differ"));
            }
        }
    }

    // The full `check`, once open and once closed, with two build jobs.
    let fixture = cargo_repo();
    let c = Case {
        env: vec![("CARGO_BUILD_JOBS", "2".into())],
        ..case("check", &["check", "ABC-1"])
    };
    let (open, shut, a, b) = open_and_closed(fixture.path(), &c, Mode::StdoutClosed);
    assert_eq!(open.code, 0, "{}{}", open.stdout, open.stderr);
    failures.extend(panic_failure(&shut, "check"));
    if shut.code != open.code {
        failures.push(format!("check: exit code {} vs {}", shut.code, open.code));
    }
    assert_same_tree(a.path(), b.path(), "check");

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- AC-4 ----

const PATTERNS: [&str; 11] = [
    "print!",
    "println!",
    "eprint!",
    "eprintln!",
    "dbg!",
    "write!",
    "writeln!",
    "stdout(",
    "stderr(",
    "Stdout",
    "Stderr",
];

/// `text` with `//` comments cut from `//` to the end of the line, where `//` is outside a
/// string literal. Handles `"..."` with escapes, raw strings and the char literal `'"'`.
fn without_comments(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '\'' && chars.get(i + 1) == Some(&'"') && chars.get(i + 2) == Some(&'\'') {
            out.extend(&chars[i..i + 3]);
            i += 3;
            continue;
        }
        if c == 'r'
            && !chars
                .get(i.wrapping_sub(1))
                .is_some_and(|p| p.is_alphanumeric() || *p == '_')
        {
            let mut j = i + 1;
            while chars.get(j) == Some(&'#') {
                j += 1;
            }
            if chars.get(j) == Some(&'"') {
                let hashes = j - i - 1;
                let close: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = chars[j + 1..].iter().collect();
                let end = rest.find(&close).map_or(chars.len(), |p| {
                    j + 1 + rest[..p].chars().count() + close.len()
                });
                out.extend(&chars[i..end]);
                i = end;
                continue;
            }
        }
        if c == '"' {
            out.push(c);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    out.push(chars[i]);
                    i += 1;
                }
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                out.push('"');
                i += 1;
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

// STP-16 AC-4
#[test]
fn no_print_macros_outside_out() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    files.sort();
    assert!(!files.is_empty());
    let mut hits = Vec::new();
    for file in files {
        if file.file_name().is_some_and(|n| n == "out.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&file).unwrap();
        for (n, line) in without_comments(&text).lines().enumerate() {
            if let Some(p) = PATTERNS.iter().find(|p| line.contains(**p)) {
                hits.push(format!(
                    "{}:{}: {p}",
                    file.strip_prefix(&src).unwrap().display(),
                    n + 1
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "output outside out.rs:\n{}",
        hits.join("\n")
    );
}
