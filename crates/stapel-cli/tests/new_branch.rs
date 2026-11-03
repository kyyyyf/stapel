//! STP-27: `stapel new` with worktrees and branches.

mod common;

use common::{stapel, stapel_repo};
use predicates::str::contains;
use std::path::Path;

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
    git(&wt6, &["add", "-A"]);
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
