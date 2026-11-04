//! STP-27 AC-15 and AC-16: the trial merge of parallel tickets and the stops it writes.

mod common;

use common::{Pair, git_in, set_section, set_spec_time, stapel, two_ticket_worktrees};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Writes `text` to `rel` in `dir` and commits it with git, outside any hook.
fn write_commit(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "-m", "wip"]);
}

/// The base text of a numbered file with line `n` replaced.
fn lines_with(total: u32, n: u32, text: &str) -> String {
    (1..=total)
        .map(|i| {
            if i == n {
                format!("{text}\n")
            } else {
                format!("line {i}\n")
            }
        })
        .collect()
}

/// The stop file of the pair `later` and `earlier` for `reason`.
fn stop_path(pair: &Pair, later: &str, earlier: &str) -> PathBuf {
    pair.repo
        .path()
        .join(format!(".git/stapel/stops/{later}/{earlier}-conflict.json"))
}

/// Runs the post-tool-use hook for a commit command in `dir`; the hook's additional context.
fn hook(dir: &Path, env: &[(&str, String)]) -> String {
    let payload = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "cwd": dir,
        "tool_input": { "command": "git commit -m wip" },
    });
    let mut cmd = stapel(dir);
    cmd.args(["hook", "post-tool-use"])
        .write_stdin(payload.to_string());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.assert().success().get_output().stdout.clone();
    if out.is_empty() {
        return String::new();
    }
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Both tickets commit `rel`; returns the hook's text in `b`.
fn both_change(pair: &Pair, rel: &str, a: &str, b: &str, env: &[(&str, String)]) -> String {
    write_commit(&pair.a, rel, a);
    write_commit(&pair.b, rel, b);
    hook(&pair.b, env)
}

fn conflict_line() -> &'static str {
    "stapel: ABC-2 conflicts with ABC-1 in src/shared.txt; ABC-2 waits for ABC-1"
}

#[test]
fn conflict_stops_the_later() {
    // The same line of one file: the stop of the later ticket and the line.
    let pair = two_ticket_worktrees();
    let said = both_change(
        &pair,
        "src/shared.txt",
        &lines_with(10, 2, "from a"),
        &lines_with(10, 2, "from b"),
        &[],
    );
    assert!(said.contains(conflict_line()), "{said}");
    let stop: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(stop_path(&pair, "ABC-2", "ABC-1")).unwrap())
            .unwrap();
    assert_eq!(stop["kind"], "merge");
    assert_eq!(stop["files"], serde_json::json!(["src/shared.txt"]));
    assert!(stop["at"].as_str().unwrap().ends_with('Z'));
    assert!(!stop_path(&pair, "ABC-1", "ABC-2").exists());
    // The earlier ticket's commit gives the same stop and the same line.
    assert!(hook(&pair.a, &[]).contains(conflict_line()));

    // A change in two places of main.rs: no stop.
    let pair = two_ticket_worktrees();
    let said = both_change(
        &pair,
        "src/main.rs",
        &lines_with(20, 1, "from a"),
        &lines_with(20, 20, "from b"),
        &[],
    );
    assert!(!said.contains("conflicts"), "{said}");
    assert!(!stop_path(&pair, "ABC-2", "ABC-1").exists());

    // Both append a line to the ledger: the union merge gives no stop.
    let pair = two_ticket_worktrees();
    let said = both_change(
        &pair,
        ".stapel/process-ledger.jsonl",
        "{\"n\":0}\n{\"n\":1}\n",
        "{\"n\":0}\n{\"n\":2}\n",
        &[],
    );
    assert!(!said.contains("conflicts"), "{said}");
    assert!(!stop_path(&pair, "ABC-2", "ABC-1").exists());

    // One line of Cargo.lock in both: a stop.
    let pair = two_ticket_worktrees();
    let lock = |v: &str| format!("# lock\nname = \"x\"\nversion = \"{v}\"\nchecksum = \"c\"\n");
    let said = both_change(&pair, "Cargo.lock", &lock("2"), &lock("3"), &[]);
    assert!(
        said.contains("stapel: ABC-2 conflicts with ABC-1 in Cargo.lock; ABC-2 waits for ABC-1"),
        "{said}"
    );
    assert!(stop_path(&pair, "ABC-2", "ABC-1").is_file());

    // GIT_DIR and GIT_INDEX_FILE in the hook's environment: the same result.
    let pair = two_ticket_worktrees();
    write_commit(&pair.a, "src/shared.txt", &lines_with(10, 2, "from a"));
    write_commit(&pair.b, "src/shared.txt", &lines_with(10, 2, "from b"));
    let git_dir = git_in(&pair.b, &["rev-parse", "--absolute-git-dir"])
        .trim()
        .to_string();
    let env = [
        ("GIT_DIR", git_dir.clone()),
        ("GIT_INDEX_FILE", format!("{git_dir}/index")),
    ];
    let said = hook(&pair.b, &env);
    assert!(said.contains(conflict_line()), "{said}");
    assert!(stop_path(&pair, "ABC-2", "ABC-1").is_file());

    // A ticket without spec time: no trial merge.
    let pair = two_ticket_worktrees();
    set_spec_time(&pair.b, "ABC-2", None);
    let said = both_change(
        &pair,
        "src/shared.txt",
        &lines_with(10, 2, "from a"),
        &lines_with(10, 2, "from b"),
        &[],
    );
    assert!(!said.contains("conflicts"), "{said}");
    assert!(!stop_path(&pair, "ABC-2", "ABC-1").exists());
    assert!(!stop_path(&pair, "ABC-1", "ABC-2").exists());

    // Order: the later spec time is the later ticket.
    let pair = two_ticket_worktrees();
    set_spec_time(&pair.a, "ABC-1", Some("2026-01-01T11:00:00Z"));
    set_spec_time(&pair.b, "ABC-2", Some("2026-01-01T10:00:00Z"));
    let said = both_change(
        &pair,
        "src/shared.txt",
        &lines_with(10, 2, "from a"),
        &lines_with(10, 2, "from b"),
        &[],
    );
    assert!(
        said.contains(
            "stapel: ABC-1 conflicts with ABC-2 in src/shared.txt; ABC-1 waits for ABC-2"
        ),
        "{said}"
    );
    assert!(stop_path(&pair, "ABC-1", "ABC-2").is_file());
    assert!(!stop_path(&pair, "ABC-2", "ABC-1").exists());

    // The same second: the higher key is the later ticket.
    let pair = two_ticket_worktrees();
    set_spec_time(&pair.a, "ABC-1", Some("2026-01-01T10:00:00Z"));
    set_spec_time(&pair.b, "ABC-2", Some("2026-01-01T10:00:00Z"));
    let said = both_change(
        &pair,
        "src/shared.txt",
        &lines_with(10, 2, "from a"),
        &lines_with(10, 2, "from b"),
        &[],
    );
    assert!(said.contains(conflict_line()), "{said}");
    assert!(stop_path(&pair, "ABC-2", "ABC-1").is_file());
}

/// `stapel next` in `dir`: its stdout.
fn next(dir: &Path, path_env: Option<&str>) -> String {
    let mut cmd = stapel(dir);
    cmd.arg("next");
    if let Some(path) = path_env {
        cmd.env("PATH", path);
    }
    let out = cmd.assert().success().get_output().stdout.clone();
    String::from_utf8_lossy(&out).into_owned()
}

fn conflicting_commits(pair: &Pair) {
    write_commit(&pair.a, "src/shared.txt", &lines_with(10, 2, "from a"));
    write_commit(&pair.b, "src/shared.txt", &lines_with(10, 2, "from b"));
}

#[test]
fn next_runs_the_trial_merge() {
    // A conflicting commit made outside the hook: `next` of either ticket writes the stop.
    for run_in_a in [true, false] {
        let pair = two_ticket_worktrees();
        conflicting_commits(&pair);
        assert!(!stop_path(&pair, "ABC-2", "ABC-1").exists());
        next(if run_in_a { &pair.a } else { &pair.b }, None);
        let stop: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(stop_path(&pair, "ABC-2", "ABC-1")).unwrap(),
        )
        .unwrap();
        assert_eq!(stop["kind"], "merge");
        assert_eq!(stop["files"], serde_json::json!(["src/shared.txt"]));
    }

    // A clean pair: no stop file.
    let pair = two_ticket_worktrees();
    write_commit(&pair.a, "src/main.rs", &lines_with(20, 1, "from a"));
    write_commit(&pair.b, "src/main.rs", &lines_with(20, 20, "from b"));
    next(&pair.b, None);
    next(&pair.a, None);
    let stops = pair.repo.path().join(".git/stapel/stops");
    let files = |dir: &Path| -> usize {
        std::fs::read_dir(dir)
            .map(|d| d.flatten().count())
            .unwrap_or(0)
    };
    assert_eq!(files(&stops.join("ABC-1")), 0);
    assert_eq!(files(&stops.join("ABC-2")), 0);

    // The conflict undone by a new commit: the conflict stop is removed.
    let pair = two_ticket_worktrees();
    conflicting_commits(&pair);
    next(&pair.b, None);
    assert!(stop_path(&pair, "ABC-2", "ABC-1").is_file());
    write_commit(&pair.b, "src/shared.txt", &lines_with(10, 2, "from a"));
    next(&pair.b, None);
    assert!(!stop_path(&pair, "ABC-2", "ABC-1").exists());

    // A stand-in git that exits 128 on merge-tree: the fact, and no stop changed.
    let pair = two_ticket_worktrees();
    conflicting_commits(&pair);
    next(&pair.b, None);
    let before = std::fs::read(stop_path(&pair, "ABC-2", "ABC-1")).unwrap();
    // The undo makes the pair clean; the stand-in must keep the stop.
    write_commit(&pair.b, "src/shared.txt", &lines_with(10, 2, "from a"));
    let real = String::from_utf8(
        std::process::Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let bin = tempfile::tempdir().unwrap();
    let script = bin.path().join("git");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do\n  if [ \"$a\" = merge-tree ]; then\n    \
             echo 'fatal: stand-in refuses merge-tree' >&2\n    exit 128\n  fi\ndone\n\
             exec {} \"$@\"\n",
            real.trim()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    let said = next(&pair.b, Some(&path));
    assert!(
        said.contains("trial merge not run: fatal: stand-in refuses merge-tree"),
        "{said}"
    );
    assert_eq!(
        std::fs::read(stop_path(&pair, "ABC-2", "ABC-1")).unwrap(),
        before
    );
}

// ---- STP-27 AC-17 and AC-18: a stopped ticket waits; the base branch is merged before close ----

/// Writes the five sections of `key` in `dir`, confirms them, sets `[check]` and commits a Plan step
/// closed by a word commit: the next action is then `run-check`.
fn build_ready(dir: &Path, key: &str) {
    set_section(dir, key, "Description", "### Frame\n\nThe problem.");
    set_section(dir, key, "Spec", "The spec.");
    set_section(dir, key, "Design", "The design.");
    set_section(dir, key, "Proof", "The proof.");
    set_section(dir, key, "Plan", "Route: light, auto.\n\n1. One.");
    let toml = dir.join(".stapel/stapel.toml");
    let mut text = std::fs::read_to_string(&toml).unwrap();
    text.push_str("\n[check]\nrunner = \"cargo\"\ntimeout_secs = 900\n");
    std::fs::write(&toml, text).unwrap();
    for section in ["spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", section]).assert().success();
    }
    write_commit(dir, "src/note.txt", "note\n");
    git_in(
        dir,
        &["commit", "-q", "--allow-empty", "-m", &format!("{key} step 1 skip: nothing")],
    );
}

const RUN_CHECK_B: &str = "next: run-check — stapel check ABC-2";

/// The stop file `ABC-2/ABC-1-dependency.json` with `body`.
fn put_dependency_stop(pair: &Pair, body: &str) -> PathBuf {
    let path = pair
        .repo
        .path()
        .join(".git/stapel/stops/ABC-2/ABC-1-dependency.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, body).unwrap();
    path
}

fn merge_stop_body() -> String {
    serde_json::json!({
        "kind": "merge",
        "files": ["src/shared.txt"],
        "at": "2026-01-01T10:00:00Z",
    })
    .to_string()
}

fn spec_stop_body() -> String {
    serde_json::json!({"kind": "spec", "files": [], "at": "2026-01-01T10:00:00Z"}).to_string()
}

/// The pre-tool-use hook for a Write of `rel` in `dir`: the assertion on its answer.
fn write_hook(dir: &Path, rel: &str) -> assert_cmd::assert::Assert {
    let payload = serde_json::json!({
        "tool_name": "Write",
        "cwd": dir,
        "permission_mode": "auto",
        "tool_input": { "file_path": dir.join(rel), "content": "x" },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .assert()
}

/// Gives `key` in `dir` a Spec whose confirmation is fresh (`fresh`) or stale.
fn spec_confirmation(dir: &Path, key: &str, fresh: bool) {
    set_section(dir, key, "Spec", "The spec.");
    let digest = Sha256::digest(b"The spec.");
    let hash: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let path = dir.join(format!(".stapel/tickets/{key}/state.json"));
    let mut state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    state["confirmations"] = serde_json::json!([{
        "section": "spec",
        "by": "test-user",
        "at": "2026-01-01T10:00:00Z",
        "hash": if fresh { format!("sha256:{hash}") } else { "sha256:old".to_string() },
        "normal_form": 1,
    }]);
    std::fs::write(&path, state.to_string()).unwrap();
}

const WAIT_LINE: &str = "blocked: ABC-2 waits for ABC-1 (";

#[test]
fn stopped_ticket_waits() {
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let b = &pair.b;

    // Without a stop: no wait, and a code write passes.
    let said = next(b, None);
    assert!(said.contains(RUN_CHECK_B), "{said}");
    assert!(!said.contains("waits for"), "{said}");
    write_hook(b, "crates/x.rs").success();

    // A `merge` stop, the earlier ticket not closed on the base branch: the fact and the action.
    put_dependency_stop(&pair, &merge_stop_body());
    let said = next(b, None);
    assert!(said.contains("blocked: ABC-2 waits for ABC-1 (dependency)"), "{said}");
    assert!(said.lines().any(|l| l == "next: wait — ABC-1"), "{said}");
    assert!(!said.contains("run-check"), "{said}");
    let json = {
        let out = stapel(b).args(["next", "--json"]).assert().success();
        String::from_utf8(out.get_output().stdout.clone()).unwrap()
    };
    assert!(json.contains("\"action\":\"wait\""), "{json}");

    // The guard denies a code write and keeps `.stapel/` and `docs/` open.
    write_hook(b, "crates/x.rs")
        .code(2)
        .stderr(predicates::str::contains("ABC-2 waits for ABC-1"));
    write_hook(b, ".stapel/note.md").success();
    write_hook(b, "docs/note.md").success();

    // A stop file that is not JSON, and one over 4 KiB: the same wait, the earlier key from the name.
    let big = serde_json::json!({
        "kind": "merge",
        "files": ["x".repeat(5 * 1024)],
        "at": "2026-01-01T10:00:00Z",
    })
    .to_string();
    for body in ["{".to_string(), big] {
        put_dependency_stop(&pair, &body);
        let said = next(b, None);
        assert!(
            said.contains("blocked: ABC-2 waits for ABC-1 (unreadable stop file)"),
            "{said}"
        );
        assert!(said.lines().any(|l| l == "next: wait — ABC-1"), "{said}");
        write_hook(b, "crates/x.rs").code(2);
    }

    // A real conflict stop and a `spec` dependency stop of one pair, the earlier spec fresh: the
    // conflict still waits.
    std::fs::remove_file(pair.repo.path().join(".git/stapel/stops/ABC-2/ABC-1-dependency.json"))
        .unwrap();
    spec_confirmation(&pair.a, "ABC-1", true);
    conflicting_commits(&pair);
    put_dependency_stop(&pair, &spec_stop_body());
    let said = next(b, None);
    assert!(stop_path(&pair, "ABC-2", "ABC-1").is_file(), "{said}");
    assert!(
        said.contains("blocked: ABC-2 waits for ABC-1 (conflict in src/shared.txt)"),
        "{said}"
    );
    assert_eq!(said.matches(WAIT_LINE).count(), 1, "{said}");
    assert!(said.lines().any(|l| l == "next: wait — ABC-1"), "{said}");
    write_hook(b, "crates/x.rs").code(2);
}

#[test]
fn spec_kind_waits_for_the_spec() {
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let b = &pair.b;
    put_dependency_stop(&pair, &spec_stop_body());

    // The earlier spec is fresh: no wait.
    spec_confirmation(&pair.a, "ABC-1", true);
    let said = next(b, None);
    assert!(said.contains(RUN_CHECK_B), "{said}");
    assert!(!said.contains("waits for"), "{said}");
    write_hook(b, "crates/x.rs").success();

    // The earlier spec is stale: the wait.
    spec_confirmation(&pair.a, "ABC-1", false);
    let said = next(b, None);
    assert!(said.contains(WAIT_LINE), "{said}");
    assert!(said.lines().any(|l| l == "next: wait — ABC-1"), "{said}");
    write_hook(b, "crates/x.rs").code(2);
    write_hook(b, ".stapel/note.md").success();

    // The earlier ticket has no spec confirmation: the wait.
    set_spec_time(&pair.a, "ABC-1", None);
    let said = next(b, None);
    assert!(said.contains(WAIT_LINE), "{said}");

    // Fresh again: no wait.
    spec_confirmation(&pair.a, "ABC-1", true);
    let said = next(b, None);
    assert!(!said.contains("waits for"), "{said}");
}

/// The branch checked out in the main checkout.
fn base_name(pair: &Pair) -> String {
    git_in(pair.repo.path(), &["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string()
}

/// Commits `rel` with `text` in the main checkout, adding only that file.
fn commit_on_base(pair: &Pair, rel: &str, text: &str) {
    let dir = pair.repo.path();
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    git_in(dir, &["add", rel]);
    git_in(dir, &["commit", "-q", "-m", "base work"]);
}

#[test]
fn merge_main_before_close() {
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let b = &pair.b;
    let base = base_name(&pair);
    let stop = put_dependency_stop(&pair, &merge_stop_body());

    // The base branch gains the earlier ticket's close: `merge-main`, no wait, before the check.
    let closed = serde_json::json!({
        "key": "ABC-1",
        "closed": {"by": "test-user", "at": "2026-01-02T10:00:00Z", "reason": "done"},
    })
    .to_string();
    commit_on_base(&pair, ".stapel/tickets/ABC-1/state.json", &closed);
    let said = next(b, None);
    assert!(
        said.lines().any(|l| l == format!("next: merge-main — git merge {base}")),
        "{said}"
    );
    assert!(!said.contains("waits for"), "{said}");
    assert!(!said.contains("run-check"), "{said}");

    // `close` is refused with the text of AC-18.
    stapel(b)
        .args(["close", "ABC-2", "--reason", "done"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(format!(
            "close needs {base} merged: run git merge {base} and stapel check ABC-2 first"
        )));

    // After the merge: `run-check`, and the stop file is gone.
    git_in(b, &["merge", "-q", "--no-edit", &base]);
    let said = next(b, None);
    assert!(said.contains(RUN_CHECK_B), "{said}");
    assert!(!said.contains("merge-main"), "{said}");
    assert!(!stop.exists(), "the stop of a closed earlier ticket stays");

    // The base branch gains only a `docs/` commit: no `merge-main`.
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    commit_on_base(&pair, "docs/note.md", "a note\n");
    let said = next(&pair.b, None);
    assert!(said.contains(RUN_CHECK_B), "{said}");
    assert!(!said.contains("merge-main"), "{said}");
}
