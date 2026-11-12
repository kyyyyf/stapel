//! STP-31: `stapel export`.

mod common;

use common::{bare_dir, git_repo, stapel};
use std::collections::BTreeMap;
use std::path::Path;
use tempfile::TempDir;

/// Runs git in `dir` with the identity of commit number `n`; returns stdout.
fn git_n(dir: &Path, n: u32, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", format!("author-{n}"))
        .env("GIT_AUTHOR_EMAIL", format!("a{n}@example.invalid"))
        .env("GIT_AUTHOR_DATE", format!("2026-01-{n:02}T10:00:00+03:00"))
        .env("GIT_COMMITTER_NAME", format!("committer-{n}"))
        .env("GIT_COMMITTER_EMAIL", format!("c{n}@example.invalid"))
        .env(
            "GIT_COMMITTER_DATE",
            format!("2026-02-{n:02}T11:00:00-05:00"),
        )
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn put(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn make_executable(dir: &Path, rel: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(rel);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Writes the files and commits them as commit number `n` with the message `msg`, kept verbatim.
fn commit(dir: &Path, n: u32, files: &[(&str, &str)], msg: &str) {
    for (rel, text) in files {
        put(dir, rel, text);
    }
    git_n(dir, n, &["add", "-A"]);
    git_n(dir, n, &["commit", "-q", "--cleanup=verbatim", "-m", msg]);
}

/// A repository on `main` with one base commit, then the branch `ABC-1` checked out.
fn base_repo() -> TempDir {
    let repo = git_repo();
    let dir = repo.path();
    git_n(dir, 1, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    put(dir, "tool.sh", "echo 1\n");
    make_executable(dir, "tool.sh");
    commit(
        dir,
        1,
        &[
            ("src/a.txt", "a1\n"),
            ("src/with space.txt", "space\n"),
            (".claude/settings.json", "main-settings\n"),
        ],
        "base",
    );
    git_n(dir, 1, &["checkout", "-q", "-b", "ABC-1"]);
    repo
}

/// The ticket branch of the Rewrite case: RED, a merge of `main`, GREEN and a close commit.
fn rewrite_repo() -> TempDir {
    let repo = base_repo();
    let dir = repo.path();
    commit(
        dir,
        2,
        &[
            ("src/red.txt", "red\n"),
            (".stapel/tickets/ABC-1/state.json", "{\"key\":\"ABC-1\"}\n"),
            (".claude/agents/stapel-builder.md", "agent 1\n"),
            (".claude/skills/stapel/SKILL.md", "skill 1\n"),
            (".claude/settings.json", "ticket-settings\n"),
        ],
        "RED",
    );
    git_n(dir, 3, &["checkout", "-q", "main"]);
    commit(dir, 3, &[("src/main_only.txt", "main\n")], "main work");
    git_n(dir, 4, &["checkout", "-q", "ABC-1"]);
    git_n(
        dir,
        4,
        &["merge", "-q", "--no-ff", "--cleanup=verbatim", "-m", "merge main", "main"],
    );
    commit(
        dir,
        5,
        &[
            ("src/green.txt", "green\n"),
            ("tool.sh", "echo 2\n"),
            (".stapel/tickets/ABC-1/ticket.md", "ticket\n"),
            (".claude/agents/stapel-builder.md", "agent 2\n"),
            (".claude/skills/stapel/SKILL.md", "skill 2\n"),
        ],
        "GREEN\n\nbody \u{1}\u{7} end\n",
    );
    commit(
        dir,
        6,
        &[
            (".stapel/tickets/ABC-1/state.json", "{\"closed\":true}\n"),
            (".claude/agents/stapel-builder.md", "agent 3\n"),
        ],
        "close",
    );
    repo
}

fn export(dir: &Path, onto: &str, tmp: &Path) {
    stapel(dir)
        .env("TMPDIR", tmp)
        .args(["export", "ABC-1", "--onto", onto])
        .assert()
        .success();
}

fn rev(dir: &Path, name: &str) -> String {
    git_n(dir, 1, &["rev-parse", name]).trim().to_string()
}

/// `(subject, commit)` of every commit of `range`, oldest first.
fn commits(dir: &Path, range: &str) -> Vec<(String, String)> {
    git_n(
        dir,
        1,
        &["log", "--topo-order", "--reverse", "--format=%s %H", range],
    )
    .lines()
    .map(|l| {
        let (s, h) = l.rsplit_once(' ').unwrap();
        (s.to_string(), h.to_string())
    })
    .collect()
}

/// The tree entries of a commit: path to `mode object`.
fn entries(dir: &Path, commit: &str) -> BTreeMap<String, String> {
    let out = std::process::Command::new("git")
        .args(["ls-tree", "-r", "-z", commit])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    out.stdout
        .split(|b| *b == 0)
        .filter(|e| !e.is_empty())
        .map(|e| {
            let text = String::from_utf8_lossy(e).into_owned();
            let (meta, path) = text.split_once('\t').unwrap();
            let mut parts = meta.split(' ');
            let mode = parts.next().unwrap();
            let _kind = parts.next();
            let object = parts.next().unwrap();
            (path.to_string(), format!("{mode} {object}"))
        })
        .collect()
}

fn is_process_path(path: &str) -> bool {
    path.starts_with(".stapel/")
        || path.starts_with(".claude/skills/stapel/")
        || path == ".claude/settings.json"
        || (path.starts_with(".claude/agents/stapel-")
            && path.ends_with(".md")
            && !path[".claude/agents/".len()..].contains('/'))
}

/// A commit without its tree and parent lines: author, committer and message.
fn identity_and_message(dir: &Path, commit: &str) -> String {
    git_n(dir, 1, &["cat-file", "commit", commit])
        .lines()
        .filter(|l| !l.starts_with("tree ") && !l.starts_with("parent "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parents(dir: &Path, commit: &str) -> Vec<String> {
    git_n(dir, 1, &["rev-list", "--parents", "-n", "1", commit])
        .split_whitespace()
        .skip(1)
        .map(String::from)
        .collect()
}

// STP-31 AC-15: each commit of the range gets its Clean tree, mapped parents, and the same author,
// committer, dates and message.
#[test]
fn rewrites_the_range() {
    let repo = rewrite_repo();
    let dir = repo.path();
    let tmp = bare_dir();
    export(dir, "main", tmp.path());

    let old = commits(dir, "main..ABC-1");
    let new = commits(dir, "main..export/ABC-1");
    let subjects = |v: &[(String, String)]| v.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>();
    assert_eq!(subjects(&old), ["RED", "merge main", "GREEN", "close"]);
    assert_eq!(subjects(&new), ["RED", "merge main", "GREEN"]);

    let main_settings = entries(dir, "main")[".claude/settings.json"].clone();
    for (subject, new_commit) in &new {
        let (_, old_commit) = old.iter().find(|(s, _)| s == subject).unwrap();
        let mut want: BTreeMap<String, String> = entries(dir, old_commit)
            .into_iter()
            .filter(|(p, _)| !is_process_path(p))
            .collect();
        want.insert(".claude/settings.json".to_string(), main_settings.clone());
        assert_eq!(entries(dir, new_commit), want, "{subject}");
        assert_eq!(
            identity_and_message(dir, new_commit),
            identity_and_message(dir, old_commit),
            "{subject}"
        );
    }
    let tree = entries(dir, "export/ABC-1");
    assert!(tree.contains_key("src/with space.txt"));
    assert!(tree["tool.sh"].starts_with("100755 "), "{tree:?}");
    assert!(tree.keys().all(|p| !p.starts_with(".stapel")), "{tree:?}");

    let by = |s: &str| new.iter().find(|(x, _)| x == s).unwrap().1.clone();
    assert_eq!(parents(dir, &by("RED")), [rev(dir, "main~1")]);
    assert_eq!(
        parents(dir, &by("merge main")),
        [by("RED"), rev(dir, "main")]
    );
    assert_eq!(parents(dir, &by("GREEN")), [by("merge main")]);
}

// STP-31 AC-15: a second run gives the same tip; a new commit lands on the old tip.
#[test]
fn export_is_repeatable() {
    let repo = rewrite_repo();
    let dir = repo.path();
    let tmp = bare_dir();
    export(dir, "main", tmp.path());
    let first = rev(dir, "export/ABC-1");
    export(dir, "main", tmp.path());
    assert_eq!(rev(dir, "export/ABC-1"), first);

    commit(dir, 7, &[("src/more.txt", "more\n")], "more");
    export(dir, "main", tmp.path());
    let second = rev(dir, "export/ABC-1");
    assert_ne!(second, first);
    assert_eq!(parents(dir, &second), [first]);
}

// STP-31 AC-16: a commit that changes only process files is dropped, its child takes the parent;
// a merge that brings only process files is dropped; a merge of two code lines stays.
#[test]
fn drops_empty_commits() {
    let repo = git_repo();
    let dir = repo.path();
    let tmp = bare_dir();
    git_n(dir, 1, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    commit(dir, 1, &[("src/a.txt", "a1\n")], "base");
    git_n(dir, 1, &["branch", "base"]);
    git_n(dir, 1, &["branch", "side"]);
    // Local main is ahead of `base` by a commit that changes only `.stapel/`.
    commit(dir, 2, &[(".stapel/process-ledger.jsonl", "{}\n")], "ledger");
    git_n(dir, 3, &["checkout", "-q", "-b", "ABC-1", "base"]);
    commit(dir, 3, &[(".stapel/tickets/ABC-1/state.json", "{}\n")], "new");
    git_n(
        dir,
        4,
        &["merge", "-q", "--no-ff", "-m", "merge main", "main"],
    );
    commit(
        dir,
        5,
        &[("src/red.txt", "red\n"), (".stapel/tickets/ABC-1/ticket.md", "t\n")],
        "RED",
    );
    commit(dir, 6, &[("src/green.txt", "green\n")], "GREEN");
    commit(
        dir,
        7,
        &[(".stapel/tickets/ABC-1/state.json", "{\"closed\":true}\n")],
        "close",
    );
    git_n(dir, 8, &["checkout", "-q", "side"]);
    commit(dir, 8, &[("src/side.txt", "side\n")], "side work");
    git_n(dir, 9, &["checkout", "-q", "ABC-1"]);
    git_n(
        dir,
        9,
        &["merge", "-q", "--no-ff", "-m", "merge side", "side"],
    );

    export(dir, "base", tmp.path());

    let new = commits(dir, "base..export/ABC-1");
    let subjects: Vec<_> = new.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(subjects, ["RED", "GREEN", "side work", "merge side"]);
    let by = |s: &str| new.iter().find(|(x, _)| x == s).unwrap().1.clone();
    assert_eq!(parents(dir, &by("RED")), [rev(dir, "base")]);
    assert_eq!(
        parents(dir, &by("merge side")),
        [by("GREEN"), by("side work")]
    );
}

/// A repository with `stapel init --prefix ABC` committed on `main`.
fn ticket_repo() -> TempDir {
    let repo = common::stapel_repo();
    let dir = repo.path();
    git_n(dir, 1, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    commit(dir, 1, &[("src/a.txt", "a1\n")], "base");
    repo
}

/// A branch `name` from `main` with a Branch state that names `base_branch` (when given) and one
/// commit `subject`; the checkout returns to `main`.
fn ticket_branch(dir: &Path, name: &str, base_branch: Option<&str>, subject: &str) {
    git_n(dir, 2, &["checkout", "-q", "-b", name, "main"]);
    let state = match base_branch {
        Some(b) => format!("{{\"key\":\"{name}\",\"base_branch\":\"{b}\"}}\n"),
        None => format!("{{\"key\":\"{name}\"}}\n"),
    };
    commit(
        dir,
        2,
        &[
            (&format!(".stapel/tickets/{name}/state.json"), &state),
            (&format!("src/{name}.txt"), "code\n"),
        ],
        subject,
    );
    git_n(dir, 2, &["checkout", "-q", "main"]);
}

fn export_refs(dir: &Path) -> String {
    git_n(dir, 1, &["for-each-ref", "refs/heads/export"])
}

// STP-31 AC-12: the key comes from the argument or the current branch, the ticket is read from its
// branch, and each refusal exits 1 without an `export/` ref.
#[test]
fn export_key_and_refusals() {
    let repo = ticket_repo();
    let dir = repo.path();
    ticket_branch(dir, "ABC-1", Some("main"), "red");

    // The worktree on branch ABC-1, no KEY.
    git_n(dir, 1, &["checkout", "-q", "ABC-1"]);
    stapel(dir).args(["export", "--onto", "main"]).assert().success();
    assert_eq!(commits(dir, "main..export/ABC-1").len(), 1);
    git_n(dir, 1, &["update-ref", "-d", "refs/heads/export/ABC-1"]);

    // The main checkout, before the merge of ABC-1: the ticket is read from its branch.
    git_n(dir, 1, &["checkout", "-q", "main"]);
    stapel(dir)
        .args(["export", "ABC-1", "--onto", "main"])
        .assert()
        .success();
    assert_eq!(commits(dir, "main..export/ABC-1").len(), 1);

    // On branch export/ABC-1 without KEY, and an option in place of a key.
    git_n(dir, 1, &["checkout", "-q", "export/ABC-1"]);
    let before = export_refs(dir);
    stapel(dir)
        .args(["export", "--onto", "main"])
        .assert()
        .code(1);
    stapel(dir).args(["export", "-x"]).assert().code(1);
    assert_eq!(export_refs(dir), before);
    git_n(dir, 1, &["checkout", "-q", "main"]);
    git_n(dir, 1, &["update-ref", "-d", "refs/heads/export/ABC-1"]);

    // Detached HEAD without KEY.
    git_n(dir, 1, &["checkout", "-q", "--detach"]);
    stapel(dir)
        .args(["export", "--onto", "main"])
        .assert()
        .code(1);
    git_n(dir, 1, &["checkout", "-q", "main"]);

    // No branch, a branch without a Branch state, a key outside the pattern.
    git_n(dir, 1, &["branch", "ABC-2", "main"]);
    ticket_branch(dir, "XYZ-1", Some("main"), "red");
    for key in ["ABC-9", "ABC-2", "XYZ-1"] {
        stapel(dir)
            .args(["export", key, "--onto", "main"])
            .assert()
            .code(1);
    }
    assert_eq!(export_refs(dir), "");
}

// STP-31 AC-13: without `--onto` the Onto ref is the upstream of the base branch; no upstream or no
// `base_branch` exits 1 and names `--onto`.
#[test]
fn onto_defaults_to_the_upstream() {
    let repo = ticket_repo();
    let dir = repo.path();
    let remote = bare_dir();
    git_n(remote.path(), 1, &["init", "-q", "--bare"]);
    let url = remote.path().display().to_string();
    git_n(dir, 1, &["remote", "add", "origin", &url]);
    // The state after `push -u origin main`, made without the push.
    git_n(dir, 1, &["config", "branch.main.remote", "origin"]);
    git_n(dir, 1, &["config", "branch.main.merge", "refs/heads/main"]);
    git_n(dir, 1, &["update-ref", "refs/remotes/origin/main", "main"]);
    // A commit on main that is not on the remote: it stays in the range of an upstream Onto ref.
    commit(dir, 3, &[("src/local.txt", "local\n")], "local only");
    ticket_branch(dir, "ABC-1", Some("main"), "red");
    git_n(dir, 1, &["branch", "side", "main"]);
    ticket_branch(dir, "ABC-2", Some("side"), "red");
    ticket_branch(dir, "ABC-3", None, "red");

    stapel(dir).args(["export", "ABC-1"]).assert().success();
    let subjects: Vec<_> = commits(dir, "origin/main..export/ABC-1")
        .into_iter()
        .map(|(s, _)| s)
        .collect();
    assert_eq!(subjects, ["local only", "red"]);

    for key in ["ABC-2", "ABC-3"] {
        let out = stapel(dir).args(["export", key]).assert().code(1);
        let err = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
        assert!(err.contains("--onto"), "{key}: {err}");
    }
    assert!(!export_refs(dir).contains("export/ABC-2"));
    assert!(!export_refs(dir).contains("export/ABC-3"));
}
