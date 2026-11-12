//! STP-31: `stapel init --local`.

mod common;

use common::{git_config, git_in, git_repo, read, snapshot, stapel};
use predicates::str::contains;
use stapel_core::config::{Config, History};

fn history_of(dir: &std::path::Path) -> Option<History> {
    Config::parse(&read(dir, ".stapel/stapel.toml"))
        .unwrap()
        .repository
        .map(|r| r.history)
}

// STP-31 AC-1: the starter `stapel.toml` records the local mode.
#[test]
fn local_mode_is_recorded() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    let text = read(dir, ".stapel/stapel.toml");
    assert!(
        text.contains("[repository]\nhistory = \"local\"\n"),
        "{text}"
    );
    assert_eq!(history_of(dir), Some(History::Local));
}

// STP-31 AC-2: later runs, with or without the flag, keep the mode and change nothing.
#[test]
fn second_run_keeps_local_mode() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    let before = snapshot(dir);
    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("already set up"));
    assert_eq!(snapshot(dir), before);
    stapel(dir)
        .args(["init", "--local"])
        .assert()
        .success()
        .stdout(contains("already set up"));
    assert_eq!(snapshot(dir), before);
    assert_eq!(history_of(dir), Some(History::Local));
}

// STP-31 AC-3: `--local` on a shared `stapel.toml` is refused and writes nothing.
#[test]
fn local_flag_refused_on_a_shared_config() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let before = snapshot(dir);
    stapel(dir)
        .args(["init", "--local"])
        .assert()
        .code(1)
        .stderr(contains(
            ".stapel/stapel.toml is in shared mode; --local applies only to the first stapel init. \
             To switch, add [repository] with history = \"local\" to it, remove stapel's lines from \
             .gitignore and .gitattributes, and run stapel init",
        ));
    assert_eq!(snapshot(dir), before);
}

// STP-31 AC-5: the ignore and merge lines go to the git folder; `.gitignore` and `.gitattributes`
// stay as they are.
#[test]
fn lines_go_to_the_git_folder() {
    let repo = git_repo();
    let dir = repo.path();
    std::fs::write(dir.join(".gitignore"), "node_modules/\n").unwrap();
    git_in(dir, &["add", ".gitignore"]);
    git_in(
        dir,
        &[
            "-c",
            "user.email=a@b.c",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "ignore",
        ],
    );
    // The exclude file has no final newline and the info folder is missing.
    let _ = std::fs::remove_dir_all(dir.join(".git/info"));
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success()
        .stdout(contains(".git/info/exclude"))
        .stdout(contains(".git/info/attributes"));
    assert_eq!(read(dir, ".gitignore"), "node_modules/\n");
    assert!(!dir.join(".gitattributes").exists());
    let exclude = read(dir, ".git/info/exclude");
    assert!(exclude.contains("/.stapel/index/\n"), "{exclude}");
    assert!(exclude.contains("/.claude/worktrees/\n"), "{exclude}");
    let attributes = read(dir, ".git/info/attributes");
    assert!(
        attributes.contains(".stapel/process-ledger.jsonl merge=union\n"),
        "{attributes}"
    );
    assert!(
        attributes.contains(".stapel/sessions.jsonl merge=union\n"),
        "{attributes}"
    );
    let status = git_in(dir, &["status", "--porcelain"]);
    assert!(!status.contains(".gitignore"), "{status}");
    assert!(!status.contains(".gitattributes"), "{status}");

    // A second run adds no line twice.
    let before = (exclude.clone(), attributes.clone());
    stapel(dir).arg("init").assert().success();
    assert_eq!(
        (
            read(dir, ".git/info/exclude"),
            read(dir, ".git/info/attributes")
        ),
        before
    );

    // An exclude file without a final newline gets a newline first.
    std::fs::write(dir.join(".git/info/exclude"), "keep-me").unwrap();
    stapel(dir).arg("init").assert().success();
    let exclude = read(dir, ".git/info/exclude");
    assert!(
        exclude.starts_with("keep-me\n/.stapel/index/\n"),
        "{exclude}"
    );

    // A linked worktree uses the shared files.
    git_in(dir, &["add", "-A"]);
    git_in(
        dir,
        &[
            "-c",
            "user.email=a@b.c",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "first",
        ],
    );
    let linked = tempfile::tempdir().unwrap();
    let wt = linked.path().join("wt");
    git_in(dir, &["worktree", "add", "-q", wt.to_str().unwrap()]);
    let shared = read(dir, ".git/info/exclude");
    stapel(&wt).arg("init").assert().success();
    assert_eq!(read(dir, ".git/info/exclude"), shared);
    assert!(!wt.join(".gitattributes").exists());
}

// STP-31 AC-6: a file in the git folder that is not UTF-8 is refused and nothing is written.
#[test]
fn refuses_a_non_utf8_exclude_file() {
    for name in ["exclude", "attributes"] {
        let repo = git_repo();
        let dir = repo.path();
        stapel(dir)
            .args(["init", "--local", "--prefix", "ABC"])
            .assert()
            .success();
        let file = dir.join(".git/info").join(name);
        std::fs::write(&file, b"abc\xff\n").unwrap();
        let _ = std::fs::remove_file(dir.join(".stapel/allowlist.toml"));
        let before = snapshot(dir);
        stapel(dir)
            .arg("init")
            .assert()
            .code(1)
            .stderr(contains(format!(".git/info/{name}")));
        assert_eq!(snapshot(dir), before);
        assert_eq!(std::fs::read(&file).unwrap(), b"abc\xff\n");
    }
}

fn commit_all(dir: &std::path::Path, message: &str) -> String {
    git_in(dir, &["add", "-A"]);
    git_in(
        dir,
        &[
            "-c",
            "user.email=a@b.c",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            message,
        ],
    );
    git_in(dir, &["rev-parse", "HEAD"]).trim().to_string()
}

fn make_local(dir: &std::path::Path) {
    let path = dir.join(".stapel/stapel.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n[repository]\nhistory = \"local\"\n");
    std::fs::write(&path, text).unwrap();
}

// STP-31 AC-7: a stapel pre-push inside the work tree goes to the exclude file.
#[test]
fn pre_push_in_a_tracked_hooks_folder() {
    // A relative hooks folder with a tracked hook.
    let repo = git_repo();
    let dir = repo.path();
    git_config(dir, "core.hooksPath", ".husky");
    std::fs::create_dir_all(dir.join(".husky")).unwrap();
    std::fs::write(dir.join(".husky/pre-commit"), "#!/bin/sh\nexit 0\n").unwrap();
    commit_all(dir, "husky");
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    assert!(dir.join(".husky/pre-push").exists());
    let exclude = read(dir, ".git/info/exclude");
    assert!(exclude.contains("/.husky/pre-push\n"), "{exclude}");
    let status = git_in(dir, &["status", "--porcelain", "--", ".husky"]);
    assert_eq!(status, "");
    // A second run adds nothing.
    stapel(dir).arg("init").assert().success();
    assert_eq!(read(dir, ".git/info/exclude"), exclude);

    // An absolute hooks folder inside the work tree.
    let repo = git_repo();
    let dir = repo.path();
    let hooks = dir.canonicalize().unwrap().join("hooks-here");
    git_config(dir, "core.hooksPath", hooks.to_str().unwrap());
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    assert!(hooks.join("pre-push").exists());
    let exclude = read(dir, ".git/info/exclude");
    assert!(exclude.contains("/hooks-here/pre-push\n"), "{exclude}");

    // The default folder inside the git folder: no line.
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    let exclude = read(dir, ".git/info/exclude");
    assert!(!exclude.contains("pre-push"), "{exclude}");

    // A hooks folder outside the work tree: the current warning, no line.
    let repo = git_repo();
    let dir = repo.path();
    let outside = tempfile::tempdir().unwrap();
    git_config(dir, "core.hooksPath", outside.path().to_str().unwrap());
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success()
        .stderr(contains("is outside the repository"));
    let exclude = read(dir, ".git/info/exclude");
    assert!(!exclude.contains("pre-push"), "{exclude}");

    // A hook that is not stapel's stays and gets no line.
    let repo = git_repo();
    let dir = repo.path();
    git_config(dir, "core.hooksPath", ".husky");
    std::fs::create_dir_all(dir.join(".husky")).unwrap();
    std::fs::write(dir.join(".husky/pre-push"), "#!/bin/sh\nexit 0\n").unwrap();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success()
        .stderr(contains("not from stapel"));
    let exclude = read(dir, ".git/info/exclude");
    assert!(!exclude.contains("pre-push"), "{exclude}");

    // A stapel hook from a shared install, then the local mode: the line is added.
    let repo = git_repo();
    let dir = repo.path();
    git_config(dir, "core.hooksPath", ".husky");
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    assert!(dir.join(".husky/pre-push").exists());
    make_local(dir);
    stapel(dir).arg("init").assert().success();
    let exclude = read(dir, ".git/info/exclude");
    assert!(exclude.contains("/.husky/pre-push\n"), "{exclude}");
}

fn run_hook(dir: &std::path::Path, input: &str, agent: bool) -> std::process::Output {
    use std::io::Write;
    let mut cmd = std::process::Command::new(dir.join(".git/hooks/pre-push"));
    cmd.current_dir(dir)
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if agent {
        cmd.env("CLAUDECODE", "1");
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

// STP-31 AC-8: the local pre-push refuses new commits that carry `.stapel/`.
#[test]
fn pre_push_refuses_process_files() {
    let repo = git_repo();
    let dir = repo.path();
    std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    let plain = commit_all(dir, "plain");
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    let with = commit_all(dir, "process files");
    git_in(dir, &["rm", "-r", "-q", "--cached", ".stapel"]);
    git_in(
        dir,
        &[
            "-c",
            "user.email=a@b.c",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "drop process files",
        ],
    );
    let without = git_in(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let zero = "0".repeat(40);
    let line = |sha: &str| format!("refs/heads/feat {sha} refs/heads/feat {zero}\n");
    let code = |out: &std::process::Output| out.status.code();

    let out = run_hook(dir, &line(&plain), false);
    assert_eq!(code(&out), Some(0));
    let out = run_hook(dir, &line(&with), false);
    assert_eq!(code(&out), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.contains(
            "stapel: refs/heads/feat carries .stapel/; push the branch that stapel export writes"
        ),
        "{stderr}"
    );
    // A tip that removes `.stapel/` on top of a commit that has it, neither on a remote.
    assert_eq!(code(&run_hook(dir, &line(&without), false)), Some(1));
    // Two lines, the second carrying `.stapel/`.
    let two = format!("{}{}", line(&plain), line(&with));
    assert_eq!(code(&run_hook(dir, &two, false)), Some(1));
    // A line of three words is skipped; an all-zero local sha; empty input.
    let three = format!("refs/heads/feat {with} refs/heads/feat\n");
    assert_eq!(code(&run_hook(dir, &three, false)), Some(0));
    let deleted = format!("(delete) {zero} refs/heads/feat {with}\n");
    assert_eq!(code(&run_hook(dir, &deleted, false)), Some(0));
    assert_eq!(code(&run_hook(dir, "", false)), Some(0));
    // The agent refusal comes first.
    let out = run_hook(dir, &line(&plain), true);
    assert_eq!(code(&out), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("agent session"));
    // The same history already on the remote's ref.
    git_in(dir, &["update-ref", "refs/remotes/origin/feat", &without]);
    assert_eq!(code(&run_hook(dir, &line(&without), false)), Some(0));
}

// STP-31 AC-9: a stapel pre-push gets the text of the run's mode; a foreign one stays.
#[test]
fn pre_push_text_follows_the_mode() {
    use std::os::unix::fs::PermissionsExt;
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let hook = dir.join(".git/hooks/pre-push");
    let shared_text = std::fs::read_to_string(&hook).unwrap();

    // To the local mode: the hook is updated and stays executable.
    make_local(dir);
    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("updated: .git/hooks/pre-push (pre-push"));
    let local_text = std::fs::read_to_string(&hook).unwrap();
    assert_ne!(local_text, shared_text);
    assert_ne!(hook.metadata().unwrap().permissions().mode() & 0o111, 0);
    // A second run changes nothing.
    stapel(dir).arg("init").assert().success();
    assert_eq!(std::fs::read_to_string(&hook).unwrap(), local_text);

    // Back to the shared mode: the hook is updated again.
    let config = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        &config,
        text.replace("history = \"local\"", "history = \"shared\""),
    )
    .unwrap();
    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("updated: .git/hooks/pre-push (pre-push"));
    assert_eq!(std::fs::read_to_string(&hook).unwrap(), shared_text);
    assert_ne!(hook.metadata().unwrap().permissions().mode() & 0o111, 0);

    // A foreign hook is kept with the warning.
    let repo = git_repo();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".git/hooks")).unwrap();
    std::fs::write(dir.join(".git/hooks/pre-push"), "#!/bin/sh\nexit 0\n").unwrap();
    stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success()
        .stderr(contains("not from stapel"));
    assert_eq!(read(dir, ".git/hooks/pre-push"), "#!/bin/sh\nexit 0\n");
}

// STP-31 AC-11: in the local mode `init` ends with the checklist, on every run; shared mode prints none.
#[test]
fn prints_the_checklist() {
    let golden = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/init_local_checklist.txt"),
    )
    .unwrap();
    let repo = git_repo();
    let dir = repo.path();
    let first = stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .output()
        .unwrap();
    assert!(first.status.success());
    let out = String::from_utf8(first.stdout).unwrap();
    assert!(out.ends_with(&golden), "{out}");
    let second = stapel(dir).arg("init").output().unwrap();
    assert!(second.status.success());
    let out = String::from_utf8(second.stdout).unwrap();
    assert!(out.contains("already set up"), "{out}");
    assert!(out.ends_with(&golden), "{out}");

    let repo = git_repo();
    let dir = repo.path();
    let shared = stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .output()
        .unwrap();
    assert!(shared.status.success());
    let out = String::from_utf8(shared.stdout).unwrap();
    assert!(!out.contains("checklist:"), "{out}");
}

// STP-31 AC-18: the person's whole path on a foreign repository (the pilot's shape): no
// `Cargo.toml`, hooks in a tracked `.husky/` folder, a script that writes a JUnit report.
#[test]
fn foreign_repository_end_to_end() {
    fn put(dir: &std::path::Path, rel: &str, text: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    // The pre-push hook run as git runs it: in the repository, without the agent variables, with
    // the input lines on stdin. Returns (exit code, stderr).
    fn pre_push(dir: &std::path::Path, input: &str) -> (Option<i32>, String) {
        use std::io::Write;
        let mut child = std::process::Command::new(dir.join(".husky/pre-push"))
            .current_dir(dir)
            .env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    // The awk program of the stand-in: one passing test case per `it("title"` line.
    const AWK: &str = r#"
BEGIN { out = "junit.xml"; print "<?xml version=\"1.0\"?>\n<testsuites>\n<testsuite name=\"s\">" > out }
/(^|[^.A-Za-z_])it\("/ {
  line = $0
  match(line, /\("[^"]*"/)
  title = substr(line, RSTART + 2, RLENGTH - 3)
  failed = 0
  if (match(line, /needs [^ ]+/)) {
    path = substr(line, RSTART + 6, RLENGTH - 6)
    if (system("test -e " path) != 0) failed = 1
  }
  body = ""
  if (failed) { body = "<failure message=\"missing\"/>"; bad = 1 }
  printf "<testcase classname=\"\" name=\" %s\">%s</testcase>\n", title, body > out
}
END { print "</testsuite>\n</testsuites>" > out; exit bad }
"#;

    let repo = git_repo();
    let tools = common::bare_dir();
    let remote = common::bare_dir();
    let dir = repo.path();
    git_in(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    git_config(dir, "user.email", "test-user.invalid");
    git_in(remote.path(), &["init", "-q", "--bare"]);
    git_config(dir, "remote.origin.url", &remote.path().display().to_string());
    git_config(dir, "remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*");

    // The foreign repository: no `Cargo.toml`, a tracked `.husky/`, an ignored `node_modules`.
    git_config(dir, "core.hooksPath", ".husky");
    put(dir, ".husky/pre-commit", "#!/bin/sh\nexit 0\n");
    put(dir, ".gitignore", "node_modules/\n");
    put(dir, "src/index.ts", "export const one = 1;\n");
    put(dir, "__tests__/base.test.ts", "it(\"base\", () => {});\n");
    put(dir, "node_modules/marker", "x\n");
    let base = commit_all(dir, "initial");
    // The person pushed `main` once: the remote-tracking ref and the upstream.
    git_in(dir, &["update-ref", "refs/remotes/origin/main", &base]);
    git_config(dir, "branch.main.remote", "origin");
    git_config(dir, "branch.main.merge", "refs/heads/main");

    // The stand-in for the test runner, outside the repository.
    let script = tools.path().join("stand-in.sh");
    std::fs::write(
        &script,
        format!(
            "[ -L node_modules ] && [ -e node_modules/marker ] || exit 4\n\
             if [ \"$#\" = 0 ]; then set -- __tests__/*.test.ts; exec awk '{AWK}' \"$@\"; fi\n\
             exec awk '{AWK}' \"$1\"\n"
        ),
    )
    .unwrap();

    // init --local: the starter has the junit template, the lines go to the git folder.
    let out = stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(out.contains("pre-push"), "{out}");
    let toml = common::read(dir, ".stapel/stapel.toml");
    assert!(toml.contains("history = \"local\""), "{toml}");
    assert!(toml.contains("junit"), "{toml}");
    assert!(!dir.join(".gitattributes").exists());
    assert_eq!(common::read(dir, ".gitignore"), "node_modules/\n");
    let exclude = common::read(dir, ".git/info/exclude");
    assert!(exclude.contains("/.husky/pre-push\n"), "{exclude}");
    assert!(dir.join(".husky/pre-push").exists());

    // The `[check]` command is filled; the installed files are committed.
    let mut toml = toml;
    toml.push_str(&format!(
        "\n[check]\nrunner = \"junit\"\ncommand = \"sh {} {{file}}\"\nreport = \"junit.xml\"\ntimeout_secs = 60\n",
        script.display()
    ));
    put(dir, ".stapel/stapel.toml", &toml);
    let installed = commit_all(dir, "install stapel");
    let tracked = git_in(dir, &["ls-files"]);
    assert!(!tracked.contains(".husky/pre-push"), "{tracked}");
    assert!(tracked.contains(".stapel/stapel.toml"), "{tracked}");
    assert!(
        git_in(dir, &["status", "--porcelain"]).is_empty(),
        "{}",
        git_in(dir, &["status", "--porcelain"])
    );

    // The session start prints `stapel next`; the guard denies a push.
    let out = stapel(dir)
        .args(["hook", "session-start"])
        .write_stdin("{\"hook_event_name\": \"SessionStart\"}")
        .assert()
        .code(0);
    let out = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(out.contains("stapel next:"), "{out}");
    let payload = serde_json::json!({
        "session_id": "test",
        "hook_event_name": "PreToolUse",
        "cwd": dir,
        "tool_name": "Bash",
        "tool_input": { "command": "git push origin main" },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .assert()
        .code(2);

    // The ticket, RED and GREEN in its worktree.
    stapel(dir).args(["new", "First"]).assert().success();
    let wt = dir.join(".claude/worktrees/ABC-1");
    assert!(wt.is_dir());
    git_in(&wt, &["add", "-A"]);
    git_in(&wt, &["commit", "-q", "--allow-empty", "-m", "ABC-1: ticket"]);
    // The person's install in the worktree (the folder is ignored).
    put(&wt, "node_modules/marker", "x\n");
    put(
        &wt,
        "__tests__/smoke.test.ts",
        "it(\"works\", () => { /* needs src/smoke.ts */ });\n",
    );
    commit_all(&wt, "ABC-1 step 1 RED: smoke");
    put(&wt, "src/smoke.ts", "export const smoke = 1;\n");
    commit_all(&wt, "ABC-1 step 1 GREEN: smoke");
    let out = stapel(&wt).args(["check", "ABC-1"]).output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("step 1: pass"), "{text}");

    // The pre-push refuses `main`, which carries `.stapel/`, and the ticket branch.
    let zero = "0".repeat(40);
    let main = git_in(dir, &["rev-parse", "main"]).trim().to_string();
    assert_eq!(main, installed);
    let (code, err) = pre_push(dir, &format!("refs/heads/main {main} refs/heads/main {base}\n"));
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("stapel export"), "{err}");

    // export: the upstream of the base branch is the default Onto ref.
    let out = stapel(dir)
        .args(["export", "ABC-1"])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(out.contains("onto refs/remotes/origin/main"), "{out}");
    assert!(out.contains("push: git push origin export/ABC-1:ABC-1"), "{out}");
    let tree = git_in(dir, &["ls-tree", "-r", "--name-only", "export/ABC-1"]);
    for path in tree.lines() {
        assert!(
            !path.starts_with(".stapel/") && !path.starts_with(".claude/"),
            "{path}"
        );
    }
    assert!(tree.contains("src/smoke.ts"), "{tree}");
    assert!(tree.contains("__tests__/smoke.test.ts"), "{tree}");

    // The export branch passes the pre-push.
    let tip = git_in(dir, &["rev-parse", "export/ABC-1"]).trim().to_string();
    let (code, err) = pre_push(
        dir,
        &format!("refs/heads/export/ABC-1 {tip} refs/heads/ABC-1 {zero}\n"),
    );
    assert_eq!(code, Some(0), "{err}");

    // No tracked file of the main checkout changed.
    assert_eq!(git_in(dir, &["rev-parse", "HEAD"]).trim(), installed);
    let status = git_in(dir, &["status", "--porcelain", "--untracked-files=no"]);
    assert_eq!(status, "", "{status}");
}
