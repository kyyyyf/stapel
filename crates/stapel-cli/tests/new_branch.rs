//! STP-27: `stapel new` with worktrees and branches.

mod common;

use common::{stapel, stapel_repo, state_json};
use predicates::str::contains;
use std::path::Path;

fn git_out(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit_all(dir: &Path) {
    git(dir, &["config", "user.email", "test-user"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "base"]);
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn ticket_file(dir: &Path, key: &str) {
    let folder = dir.join(".stapel/tickets").join(key);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("ticket.md"), format!("# {key}\n")).unwrap();
}

#[test]
fn key_above_every_worktree_and_branch() {
    let repo = stapel_repo();
    let dir = repo.path();
    let outside = tempfile::tempdir().unwrap();
    let out = outside.path();
    git(dir, &["config", "user.email", "test-user"]);
    ticket_file(dir, "ABC-1");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "base"]);

    // A worktree with ABC-4 on disk only.
    let wt4 = out.join("wt4");
    git(dir, &["worktree", "add", "-q", wt4.to_str().unwrap(), "-b", "side4"]);
    ticket_file(&wt4, "ABC-4");

    // A branch whose tree has ABC-6.
    let wt6 = out.join("wt6");
    git(dir, &["worktree", "add", "-q", wt6.to_str().unwrap(), "-b", "side6"]);
    ticket_file(&wt6, "ABC-6");
    // A file named like a key is no ticket folder and does not count.
    std::fs::write(wt6.join(".stapel/tickets/ABC-30.md"), "note\n").unwrap();
    std::fs::write(wt6.join(".stapel/tickets/ABC-30"), "note\n").unwrap();
    git(&wt6,&["add", "-A"]);
    git(&wt6, &["commit", "-q", "-m", "six"]);
    git(dir, &["worktree", "remove", "--force", wt6.to_str().unwrap()]);

    // A branch ABC-9 with no folder, and a remote-tracking branch origin/ABC-11.
    git(dir, &["branch", "ABC-9"]);
    git(dir, &["update-ref", "refs/remotes/origin/ABC-11", "HEAD"]);

    // A prunable worktree: the folder is gone, its branch ABC-12 remains.
    let wt12 = out.join("wt12");
    git(dir, &["worktree", "add", "-q", wt12.to_str().unwrap(), "-b", "ABC-12"]);
    ticket_file(&wt12, "ABC-14");
    std::fs::remove_dir_all(&wt12).unwrap();

    stapel(dir)
        .args(["new", "Next ticket"])
        .assert()
        .success()
        .stdout(contains("created: ABC-13"));
}

#[test]
fn new_creates_branch_and_worktree() {
    for base_branch in ["main", "master"] {
        let repo = stapel_repo();
        let dir = repo.path();
        git(dir, &["branch", "-m", base_branch]);
        commit_all(dir);
        let head = git_out(dir, &["rev-parse", "HEAD"]);
        let short = git_out(dir, &["rev-parse", "--short", "HEAD"]);

        stapel(dir)
            .args(["new", "First ticket"])
            .assert()
            .success()
            .stdout(contains("created: ABC-1"))
            .stdout(contains(format!("branch: ABC-1 from {base_branch} at {short}")))
            .stdout(contains("worktree: .claude/worktrees/ABC-1"));

        assert_eq!(git_out(dir, &["rev-parse", "ABC-1"]), head);
        let wt = dir.join(".claude/worktrees/ABC-1");
        assert!(wt.join(".stapel/tickets/ABC-1/ticket.md").is_file());
        assert!(!dir.join(".stapel/tickets/ABC-1").exists());
        let state = state_json(&wt, "ABC-1");
        assert_eq!(state["base"], head.as_str());
        assert_eq!(state["base_branch"], base_branch);
        assert_eq!(git_out(&wt, &["rev-parse", "--abbrev-ref", "HEAD"]), "ABC-1");
    }
}

#[test]
fn two_new_at_once() {
    let repo = stapel_repo();
    let dir = repo.path().to_path_buf();
    commit_all(&dir);

    // While another process holds new.lock, `new` waits and makes nothing.
    let lock_dir = dir.join(".git/stapel");
    std::fs::create_dir_all(&lock_dir).unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_dir.join("new.lock"))
        .unwrap();
    held.lock().unwrap();
    let waiter = {
        let dir = dir.clone();
        std::thread::spawn(move || stapel(&dir).args(["new", "Waiting"]).output().unwrap())
    };
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert!(!waiter.is_finished(), "new ran while new.lock was held");
    assert!(!dir.join(".claude/worktrees/ABC-1").exists());
    drop(held);
    let out = waiter.join().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    git(&dir, &["rev-parse", "--verify", "-q", "ABC-1"]);

    let barrier =std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|i| {
            let dir = dir.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                stapel(&dir).args(["new", &format!("Ticket {i}")]).output().unwrap()
            })
        })
        .collect();
    let mut keys = Vec::new();
    for h in handles {
        let out = h.join().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success(), "{stdout} {}", String::from_utf8_lossy(&out.stderr));
        let line = stdout.lines().find(|l| l.starts_with("created: ")).unwrap().to_string();
        keys.push(line["created: ".len()..].to_string());
    }
    assert_ne!(keys[0], keys[1]);
    for key in &keys {
        git(&dir, &["rev-parse", "--verify", "-q", key]);
        assert!(dir.join(".claude/worktrees").join(key).join(".stapel/tickets").join(key).is_dir());
    }
}

#[test]
fn unborn_head_and_ticket_branch() {
    // No commit: the ticket stays in this checkout.
    let repo = stapel_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["new", "First ticket"])
        .assert()
        .success()
        .stdout(contains("created: ABC-1"))
        .stdout(contains("note: no commit yet; the ticket stays in this checkout"));
    assert!(dir.join(".stapel/tickets/ABC-1/ticket.md").is_file());
    assert!(!dir.join(".claude/worktrees/ABC-1").exists());

    // Inside a ticket worktree: refused, no branch made.
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    std::fs::write(dir.join("docs/a.md"), "a\n").unwrap();
    commit_all(dir);
    let branch = git_out(dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    stapel(dir).args(["new", "Second ticket"]).assert().success();
    let wt = dir.join(".claude/worktrees/ABC-2");
    let branches_before = git_out(dir, &["branch", "--list"]);
    stapel(&wt)
        .args(["new", "Third ticket"])
        .assert()
        .code(2)
        .stderr(contains("stapel new runs on the base branch, not on the ticket branch ABC-2"));
    assert_eq!(git_out(dir, &["branch", "--list"]), branches_before);
    assert!(!branches_before.contains("ABC-3"));

    // A change outside machine files: refused.
    std::fs::write(dir.join("docs/a.md"), "changed\n").unwrap();
    stapel(dir)
        .args(["new", "Fourth ticket"])
        .assert()
        .code(2)
        .stderr(contains("commit the changes of docs/a.md first; the new worktree starts at HEAD"));
    std::fs::write(dir.join("docs/a.md"), "a\n").unwrap();

    // A changed state.json only: allowed.
    let state = dir.join(".stapel/tickets/ABC-1/state.json");
    let text = std::fs::read_to_string(&state).unwrap();
    std::fs::write(&state, format!("{text} ")).unwrap();
    stapel(dir).args(["new", "Fifth ticket"]).assert().success();
    let _ = branch;
}

/// One checkout with two open tickets ABC-1 and ABC-2, committed, on a base branch.
fn two_tickets_in_one_checkout() -> tempfile::TempDir {
    let repo = stapel_repo();
    let dir = repo.path();
    stapel(dir).args(["new", "First"]).assert().success();
    stapel(dir).args(["new", "Second"]).assert().success();
    for key in ["ABC-1", "ABC-2"] {
        common::set_section(dir, key, "Spec", "The spec.");
    }
    commit_all(dir);
    repo
}

#[test]
fn branch_names_the_ticket() {
    let repo = two_tickets_in_one_checkout();
    let dir = repo.path();
    let base = git_out(dir, &["rev-parse", "--abbrev-ref", "HEAD"]);

    // On the base branch: refused as before.
    stapel(dir)
        .arg("status")
        .assert()
        .code(1)
        .stderr(contains("several open tickets, name one: ABC-1, ABC-2"));

    // A branch with no folder: the old rule.
    git(dir, &["checkout", "-q", "-b", "ABC-7"]);
    stapel(dir)
        .arg("next")
        .assert()
        .code(2)
        .stderr(contains("several open tickets, name one: ABC-1, ABC-2"));

    for branch in ["ABC-2", "abc-2"] {
        git(dir, &["checkout", "-q", &base]);
        git(dir, &["checkout", "-q", "-b", branch]);
        stapel(dir).arg("status").assert().success().stdout(contains("ABC-2"));
        stapel(dir).arg("next").assert().success().stdout(contains("ABC-2"));
        stapel(dir)
            .args(["ok", "spec"])
            .assert()
            .success()
            .stdout(contains("confirmed: ABC-2 spec"));
        let out = stapel(dir)
            .args(["review", "start", "--round", "1"])
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!err.contains("several open tickets"), "{branch}: {err}");
        git(dir, &["checkout", "-q", "-f", &base]);
        git(dir, &["branch", "-q", "-D", branch]);
    }

    // A closed branch ticket: `next` still takes it.
    git(dir, &["checkout", "-q", "-b", "ABC-2"]);
    let mut state = state_json(dir, "ABC-2");
    state["closed"] =
        serde_json::json!({"by": "test-user", "at": "2026-10-19T10:00:00Z", "reason": "done"});
    std::fs::write(
        dir.join(".stapel/tickets/ABC-2/state.json"),
        serde_json::to_string_pretty(&state).unwrap(),
    )
    .unwrap();
    stapel(dir)
        .arg("next")
        .assert()
        .success()
        .stdout(contains("ABC-2"))
        .stdout(contains("next: closed"));
}

#[test]
fn key_must_match_the_branch() {
    let repo = two_tickets_in_one_checkout();
    let dir = repo.path();
    let base = git_out(dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git(dir, &["checkout", "-q", "-b", "ABC-2"]);

    let text = format!(
        "ABC-1 is not the ticket of branch ABC-2; run it in .claude/worktrees/ABC-1"
    );
    for args in [
        vec!["check", "ABC-1"],
        vec!["ok", "ABC-1", "spec"],
        vec!["close", "ABC-1", "--reason", "done"],
    ] {
        stapel(dir)
            .args(&args)
            .assert()
            .code(2)
            .stderr(contains(format!("stapel: {text}")));
    }

    // A key typed in lower case: the refusal names the folder's key.
    stapel(dir)
        .args(["check", "abc-1"])
        .assert()
        .code(2)
        .stderr(contains(format!("stapel: {text}")));

    // The branch ticket itself is not refused for that reason.
    let out = stapel(dir).args(["check", "ABC-2"]).output().unwrap();
    assert!(!String::from_utf8_lossy(&out.stderr).contains("is not the ticket of branch"));

    // On the base branch: no refusal.
    git(dir, &["checkout", "-q", &base]);
    stapel(dir).args(["ok", "ABC-1", "spec"]).assert().success();
    let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
    assert!(!String::from_utf8_lossy(&out.stderr).contains("is not the ticket of branch"));
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .assert()
        .success();
}
