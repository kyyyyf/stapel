//! STP-27 AC-15 and AC-16: the trial merge of parallel tickets and the stops it writes.

mod common;

use common::{Pair, git_in, set_spec_time, stapel, two_ticket_worktrees};
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
