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
        || path.starts_with(".claude/skills/stapel-elaborate/")
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

// STP-31 AC-16: a commit is compared with the Clean tree of its parent, also outside the range; a
// merge whose parents collapse is dropped only when its Clean tree equals that parent's.
#[test]
fn drops_against_clean_parent_trees() {
    let repo = git_repo();
    let dir = repo.path();
    let tmp = bare_dir();
    git_n(dir, 1, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    commit(
        dir,
        1,
        &[("src/a.txt", "a1\n"), (".claude/settings.json", "s0\n")],
        "base",
    );
    git_n(dir, 1, &["branch", "onto"]);
    // The Onto ref changed settings.json since the merge base.
    git_n(dir, 2, &["checkout", "-q", "onto"]);
    commit(dir, 2, &[(".claude/settings.json", "s1\n")], "onto settings");
    // Local main is ahead of the base by a commit that changes only `.stapel/`.
    git_n(dir, 3, &["checkout", "-q", "-b", "side", "main"]);
    commit(dir, 3, &[(".stapel/process-ledger.jsonl", "{}\n")], "ledger");
    git_n(dir, 3, &["checkout", "-q", "-b", "ABC-1", "main"]);
    commit(
        dir,
        4,
        &[
            (".claude/settings.json", "s2\n"),
            (".stapel/tickets/ABC-1/state.json", "{}\n"),
        ],
        "settings only",
    );
    // The merge brings only `.stapel/` from side, and its own change of a code file.
    git_n(
        dir,
        5,
        &["merge", "-q", "--no-ff", "--no-commit", "side"],
    );
    commit(dir, 5, &[("src/evil.txt", "evil\n")], "merge side");

    export(dir, "onto", tmp.path());

    let new = commits(dir, "onto..export/ABC-1");
    let subjects: Vec<_> = new.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(subjects, ["merge side"]);
    assert_eq!(parents(dir, &new[0].1), [rev(dir, "main")]);
    assert!(entries(dir, &new[0].1).contains_key("src/evil.txt"));
}

// STP-31 AC-16 (ER2-4): the merge that `git pull` made on local main is dropped, its children take
// the later parent; a merge of two code lines stays.
#[test]
fn drops_pull_merges() {
    let repo = git_repo();
    let dir = repo.path();
    let tmp = bare_dir();
    git_n(dir, 1, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    commit(dir, 1, &[("src/a.txt", "a1\n")], "base");
    git_n(dir, 1, &["branch", "up"]);
    // Local main holds the install commit, which changes only `.stapel/`.
    commit(dir, 2, &[(".stapel/process-ledger.jsonl", "{}\n")], "install");
    // The origin gets a new commit; the Onto ref sits on it.
    git_n(dir, 3, &["checkout", "-q", "up"]);
    commit(dir, 3, &[("src/origin.txt", "origin\n")], "origin work");
    git_n(dir, 3, &["branch", "onto"]);
    git_n(dir, 3, &["branch", "side"]);
    // The pull: a merge of the new origin commit into local main.
    git_n(dir, 4, &["checkout", "-q", "main"]);
    git_n(dir, 4, &["merge", "-q", "--no-ff", "-m", "pull", "up"]);
    git_n(dir, 5, &["checkout", "-q", "-b", "ABC-1", "main"]);
    commit(
        dir,
        5,
        &[("src/red.txt", "red\n"), (".stapel/tickets/ABC-1/state.json", "{}\n")],
        "RED",
    );
    git_n(dir, 6,&["checkout", "-q", "side"]);
    commit(dir, 6, &[("src/side.txt", "side\n")], "side work");
    git_n(dir, 7, &["checkout", "-q", "ABC-1"]);
    git_n(
        dir,
        7,
        &["merge", "-q", "--no-ff", "-m", "merge side", "side"],
    );

    export(dir, "onto", tmp.path());

    let new = commits(dir, "onto..export/ABC-1");
    let subjects: Vec<_> = new.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(subjects, ["RED", "side work", "merge side"]);
    let by = |s: &str| new.iter().find(|(x, _)| x == s).unwrap().1.clone();
    assert_eq!(parents(dir, &by("RED")), [rev(dir, "onto")]);
    assert_eq!(
        parents(dir, &by("merge side")),
        [by("RED"), by("side work")]
    );
}

/// A repository with `stapel init --prefix ABC` committed on `main`.
fn ticket_repo() -> TempDir {
    let repo = common::stapel_repo();
    let dir = repo.path();
    git_n(dir, 1, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    // The files that `init` wrote stay in the work tree but out of `main`, as in the pilot where
    // the Onto ref never holds `.stapel/`; ticket data is committed on the ticket branches only.
    let untracked = git_n(dir, 1, &["ls-files", "--others", "--exclude-standard"]);
    let exclude = dir.join(".git/info/exclude");
    std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
    let lines: String = untracked.lines().map(|l| format!("/{l}\n")).collect();
    std::fs::write(&exclude, lines).unwrap();
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

// STP-31 AC-12 (R1-3): without `stapel.toml` the key must still be a valid branch name.
#[test]
fn key_must_be_a_branch_name() {
    let repo = git_repo();
    let dir = repo.path();
    commit(dir, 1, &[("src/a.txt", "a1\n")], "base");
    let before = git_n(dir, 1, &["for-each-ref"]);
    for key in ["a..b", "a b", "x.lock"] {
        let out = stapel(dir).args(["export", key]).assert().code(1);
        let err = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
        assert!(err.contains(key), "{key}: {err}");
        assert!(err.contains("valid branch name"), "{key}: {err}");
    }
    assert_eq!(git_n(dir, 1, &["for-each-ref"]), before);
}

// STP-31 AC-13 (drift-2): the upstream is that of exactly `base_branch`, not of a branch that
// starts with its name, and an empty `base_branch` is no branch.
#[test]
fn default_upstream_is_exact() {
    let repo = ticket_repo();
    let dir = repo.path();
    let remote = bare_dir();
    git_n(remote.path(), 1, &["init", "-q", "--bare"]);
    let url = remote.path().display().to_string();
    git_n(dir, 1, &["remote", "add", "origin", &url]);
    git_n(dir, 1, &["config", "branch.main.remote", "origin"]);
    git_n(dir, 1, &["config", "branch.main.merge", "refs/heads/main"]);
    git_n(dir, 1, &["update-ref", "refs/remotes/origin/main", "main"]);
    git_n(dir, 1, &["branch", "feature/x", "main"]);
    git_n(dir, 1, &["config", "branch.feature/x.remote", "origin"]);
    git_n(dir, 1, &["config", "branch.feature/x.merge", "refs/heads/feature/x"]);
    git_n(dir, 1, &["update-ref", "refs/remotes/origin/feature/x", "main"]);
    ticket_branch(dir, "ABC-1", Some("feature"), "red");
    ticket_branch(dir, "ABC-2", Some(""), "red");

    for key in ["ABC-1", "ABC-2"] {
        let out = stapel(dir).args(["export", key]).assert().code(1);
        let err = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
        assert!(err.contains("--onto"), "{key}: {err}");
    }
    assert_eq!(export_refs(dir), "");
}

/// Runs `stapel export ABC-1 <args>` expecting exit 1; returns stderr.
fn refused(dir: &Path, args: &[&str]) -> String {
    let out = stapel(dir)
        .args(["export", "ABC-1"])
        .args(args)
        .assert()
        .code(1);
    String::from_utf8_lossy(&out.get_output().stderr).into_owned()
}

/// A root commit with the empty tree, on no branch yet: returns its id.
fn root_commit(dir: &Path) -> String {
    git_n(
        dir,
        7,
        &["commit-tree", "4b825dc642cb6eb9a060e54bf8d69288fbee4904", "-m", "root"],
    )
    .trim()
    .to_string()
}

// STP-31 AC-14: each refusal exits 1, names its cause and changes no ref.
#[test]
fn export_refusals() {
    let repo = ticket_repo();
    let dir = repo.path();
    ticket_branch(dir, "ABC-1", Some("main"), "red");
    let refs = |dir: &Path| git_n(dir, 1, &["for-each-ref"]);
    let before = refs(dir);

    for onto in ["nosuch", "-x"] {
        let err = refused(dir, &["--onto", onto]);
        assert!(err.contains("no commit"), "{onto}: {err}");
    }

    // An unrelated root commit.
    let root = root_commit(dir);
    git_n(dir, 1, &["branch", "other", &root]);
    let err = refused(dir, &["--onto", "other"]);
    assert!(err.contains("no common commit"), "{err}");

    // An Onto ref whose tree has `.stapel/`.
    git_n(dir, 1, &["checkout", "-q", "-b", "tracked", "main"]);
    commit(dir, 3, &[(".stapel/x", "x\n")], "tracks the process");
    git_n(dir, 1, &["checkout", "-q", "main"]);
    let err = refused(dir, &["--onto", "tracked"]);
    assert!(err.contains(".stapel"), "{err}");

    // An empty range: the Onto ref holds every commit of the branch and has no `.stapel/`.
    git_n(dir, 1, &["checkout", "-q", "-b", "after", "ABC-1"]);
    git_n(dir, 1, &["rm", "-rq", ".stapel"]);
    git_n(dir, 6, &["commit", "-q", "-m", "drops the process files"]);
    git_n(dir, 1, &["checkout", "-q", "main"]);
    let err = refused(dir, &["--onto", "after"]);
    assert!(err.contains("empty"), "{err}");

    // A branch whose only commits touch `.stapel/`.
    git_n(dir, 1, &["checkout", "-q", "-b", "ABC-3", "main"]);
    commit(
        dir,
        4,
        &[(".stapel/tickets/ABC-3/state.json", "{\"base_branch\":\"main\"}\n")],
        "process only",
    );
    git_n(dir, 1, &["checkout", "-q", "main"]);
    let out = stapel(dir)
        .args(["export", "ABC-3", "--onto", "main"])
        .assert()
        .code(1);
    let err = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(err.contains("dropped"), "{err}");

    // An orphan branch merged into the ticket branch.
    ticket_branch(dir, "ABC-2", Some("main"), "red");
    git_n(dir, 1, &["branch", "orphan", &root]);
    git_n(dir, 1, &["checkout", "-q", "ABC-2"]);
    git_n(
        dir,
        5,
        &["merge", "-q", "--no-ff", "--allow-unrelated-histories", "-m", "merge orphan", "orphan"],
    );
    git_n(dir, 1, &["checkout", "-q", "main"]);
    let out = stapel(dir)
        .args(["export", "ABC-2", "--onto", "main"])
        .assert()
        .code(1);
    let err = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(err.contains("no parents"), "{err}");

    // None of the refusals above changed a ref.
    let after = refs(dir);
    let new: Vec<_> = after.lines().filter(|l| !before.contains(l)).collect();
    assert!(
        new.iter().all(|l| !l.contains("export/")),
        "{new:?}"
    );

    // `export/ABC-1` checked out in a worktree.
    let trees = TempDir::new().unwrap();
    let path = trees.path().join("wt");
    git_n(
        dir,
        1,
        &["worktree", "add", "-q", "-b", "export/ABC-1", path.to_str().unwrap(), "main"],
    );
    let tip = rev(dir, "export/ABC-1");
    let err = refused(dir, &["--onto", "main"]);
    assert!(err.contains("checked out"), "{err}");
    assert_eq!(rev(dir, "export/ABC-1"), tip);
}

// STP-31 AC-17: the output is the golden file; the remote of the push line is the longest remote
// name that starts the rest of the Onto ref; an Onto commit id gives one line.
#[test]
fn export_output() {
    let repo = ticket_repo();
    let dir = repo.path();
    ticket_branch(dir, "ABC-1", Some("main"), "red");
    git_n(dir, 1, &["checkout", "-q", "ABC-1"]);
    commit(dir, 3, &[(".stapel/close.txt", "closed\n")], "close");
    git_n(dir, 1, &["checkout", "-q", "main"]);
    git_n(dir, 1, &["remote", "add", "origin", "/nonexistent/origin"]);
    git_n(dir, 1, &["update-ref", "refs/remotes/origin/main", "main"]);

    let run = |onto: &str| {
        let out = stapel(dir)
            .args(["export", "ABC-1", "--onto", onto])
            .assert()
            .success();
        String::from_utf8_lossy(&out.get_output().stdout).into_owned()
    };

    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/export_output.txt"),
    )
    .unwrap();
    let out = run("origin/main");
    let tip = rev(dir, "export/ABC-1");
    assert_eq!(out, golden.replace("<tip>", &tip));

    // Remotes `my` and `my/fork`: the longest name wins. Newer git refuses `git remote add my/fork`
    // while `my` exists, so the second remote is set by its config key.
    git_n(dir, 1, &["remote", "add", "my", "/nonexistent/my"]);
    git_n(dir, 1, &["config", "remote.my/fork.url", "/nonexistent/fork"]);
    git_n(dir, 1, &["update-ref", "refs/remotes/my/fork/release/1", "main"]);
    let out = run("my/fork/release/1");
    let lines: Vec<_> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert!(lines[0].ends_with("onto refs/remotes/my/fork/release/1"), "{out}");
    assert_eq!(lines[1], "push: git push my/fork export/ABC-1:ABC-1");

    // A commit id has no full name: one line.
    let main = rev(dir, "main");
    let out = run(&main);
    assert_eq!(out.lines().count(), 1, "{out}");
    assert!(out.ends_with(&format!("onto {main}\n")), "{out}");
}

// STP-31 AC-19: an export and a refusal leave the work tree, the index and `HEAD` as they were, no
// temporary folder stays, and `update-ref` is the last git call.
#[test]
fn export_leaves_the_checkout() {
    let repo = ticket_repo();
    let dir = repo.path();
    commit(dir, 1, &[("src/b.txt", "b1\n")], "second file");
    ticket_branch(dir, "ABC-1", Some("main"), "red");
    // A staged change and an unstaged change.
    put(dir, "src/a.txt", "staged\n");
    git_n(dir, 1, &["add", "src/a.txt"]);
    put(dir, "src/b.txt", "unstaged\n");
    let tmp = bare_dir();
    let trace_dir = bare_dir();

    let state = |dir: &Path| {
        (
            git_n(dir, 1, &["status", "--porcelain"]),
            std::fs::read(dir.join(".git/index")).unwrap(),
            rev(dir, "HEAD"),
            std::fs::read_to_string(dir.join("src/a.txt")).unwrap(),
            std::fs::read_to_string(dir.join("src/b.txt")).unwrap(),
        )
    };
    let leftovers = |tmp: &Path| {
        std::fs::read_dir(tmp)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("stapel-export-"))
            .collect::<Vec<_>>()
    };
    // The git calls of one run, in order.
    let trace_of = |args: &[&str], name: &str, code: i32| {
        let trace = trace_dir.path().join(name);
        stapel(dir)
            .env("TMPDIR", tmp.path())
            .env("GIT_TRACE", &trace)
            .args(args)
            .assert()
            .code(code);
        std::fs::read_to_string(&trace)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once(" git ").map(|(_, call)| call.to_string()))
            .collect::<Vec<_>>()
    };

    let before = state(dir);
    assert!(before.0.contains("M  src/a.txt"), "{}", before.0);
    assert!(before.0.contains(" M src/b.txt"), "{}", before.0);

    // A refusal: no ref, no change, no folder, no `update-ref`.
    let calls = trace_of(&["export", "ABC-1", "--onto", "nosuch"], "refusal", 1);
    assert!(!calls.iter().any(|c| c.contains("update-ref")), "{calls:?}");
    assert_eq!(export_refs(dir), "");
    assert!(state(dir) == before);
    assert_eq!(leftovers(tmp.path()), Vec::<String>::new());

    // An export: the ref is written by the last git call.
    let calls = trace_of(&["export", "ABC-1", "--onto", "main"], "export", 0);
    let last = calls.last().expect("no git call traced");
    assert!(last.contains("update-ref"), "{last}\n{calls:?}");
    assert_eq!(
        calls.iter().filter(|c| c.contains("update-ref")).count(),
        1,
        "{calls:?}"
    );
    assert!(!export_refs(dir).is_empty());
    assert!(state(dir) == before);
    assert_eq!(leftovers(tmp.path()), Vec::<String>::new());

    // A remote Onto ref: the remote list is read before the ref is written.
    git_n(dir, 1, &["remote", "add", "origin", "/nonexistent/origin"]);
    git_n(dir, 1, &["update-ref", "refs/remotes/origin/main", "main"]);
    let calls = trace_of(&["export", "ABC-1", "--onto", "origin/main"], "remote", 0);
    let last = calls.last().expect("no git call traced");
    assert!(last.contains("update-ref"), "{last}\n{calls:?}");
    assert!(calls.iter().any(|c| c.contains("remote")), "{calls:?}");
    assert!(state(dir) == before);
    assert_eq!(leftovers(tmp.path()), Vec::<String>::new());
}

// STP-31 AC-21: from a subfolder or a linked worktree the process paths and the `.stapel` check are
// read from the top of the tree, and `export/ABC-1` is the same as from the root.
#[test]
fn export_from_a_subfolder() {
    let repo = rewrite_repo();
    let dir = repo.path();
    let tmp = bare_dir();
    let run_in = |cwd: &Path| {
        stapel(cwd)
            .env("TMPDIR", tmp.path())
            .args(["export", "ABC-1", "--onto", "main"])
            .assert()
            .success();
    };

    run_in(dir);
    let from_root = rev(dir, "export/ABC-1");
    git_n(dir, 1, &["update-ref", "-d", "refs/heads/export/ABC-1"]);

    run_in(&dir.join("src"));
    assert_eq!(rev(dir, "export/ABC-1"), from_root);
    // Process paths are as in `main`: only its settings file.
    let kept: Vec<_> = entries(dir, "export/ABC-1")
        .into_keys()
        .filter(|p| is_process_path(p))
        .collect();
    assert_eq!(kept, [".claude/settings.json"]);
    git_n(dir, 1, &["update-ref", "-d", "refs/heads/export/ABC-1"]);

    // A linked worktree of the ticket, run from its subfolder.
    let linked = bare_dir();
    let linked_dir = linked.path().join("wt");
    git_n(
        dir,
        1,
        &["worktree", "add", "-q", "--detach", linked_dir.to_str().unwrap(), "ABC-1"],
    );
    run_in(&linked_dir.join("src"));
    assert_eq!(rev(dir, "export/ABC-1"), from_root);
    git_n(dir, 1, &["update-ref", "-d", "refs/heads/export/ABC-1"]);

    // The `.stapel` check of the Onto tree is read from the top too.
    git_n(dir, 3, &["checkout", "-q", "main"]);
    commit(dir, 3, &[(".stapel/x.txt", "x\n")], "main with process files");
    git_n(dir, 3, &["checkout", "-q", "ABC-1"]);
    let before = export_refs(dir);
    let err = stapel(&dir.join("src"))
        .env("TMPDIR", tmp.path())
        .args(["export", "ABC-1", "--onto", "main"])
        .assert()
        .code(1);
    let err = String::from_utf8_lossy(&err.get_output().stderr).into_owned();
    assert!(err.contains(".stapel"), "{err}");
    assert_eq!(export_refs(dir), before);
}

// STP-31 AC-15 (R1-2, ER1-4): the author and committer lines, the `encoding` header and the message
// bytes are kept exactly; only `gpgsig` is dropped.
#[test]
fn keeps_identity_and_encoding() {
    use std::io::Write;

    fn raw_commit(dir: &Path, commit: &str) -> Vec<u8> {
        let out = std::process::Command::new("git")
            .args(["cat-file", "commit", commit])
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(out.status.success());
        out.stdout
    }

    fn write_commit(dir: &Path, raw: &[u8]) -> String {
        let mut child = std::process::Command::new("git")
            .args(["hash-object", "-t", "commit", "-w", "--stdin"])
            .current_dir(dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(raw).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// The lines of a raw commit without `tree`, `parent` and the `gpgsig` block.
    fn kept_lines(raw: &[u8]) -> Vec<u8> {
        let split = raw.windows(2).position(|w| w == b"\n\n").unwrap();
        let mut out = Vec::new();
        let mut in_sig = false;
        for line in raw[..split].split(|b| *b == b'\n') {
            if in_sig && line.starts_with(b" ") {
                continue;
            }
            in_sig = line.starts_with(b"gpgsig ");
            if in_sig || line.starts_with(b"tree ") || line.starts_with(b"parent ") {
                continue;
            }
            out.extend_from_slice(line);
            out.push(b'\n');
        }
        out.extend_from_slice(&raw[split..]);
        out
    }

    let repo = base_repo();
    let dir = repo.path();
    let tmp = bare_dir();
    commit(
        dir,
        2,
        &[
            ("src/one.txt", "1\n"),
            (".stapel/tickets/ABC-1/state.json", "{\"key\":\"ABC-1\"}\n"),
        ],
        "one",
    );
    commit(dir, 3, &[("src/two.txt", "2\n")], "two");
    commit(dir, 4, &[("src/three.txt", "3\n")], "three");
    commit(dir, 5, &[("src/four.txt", "4\n")], "four");

    // Rewrite the four commits by hand, with identities and bytes `commit-tree` would change.
    let originals: Vec<String> = commits(dir, "main..ABC-1").into_iter().map(|(_, h)| h).collect();
    let mut parent = rev(dir, "main");
    let mut sources: Vec<String> = Vec::new();
    for (i, original) in originals.iter().enumerate() {
        let tree = git_n(dir, 1, &["rev-parse", &format!("{original}^{{tree}}")]);
        let mut raw: Vec<u8> = format!("tree {}\nparent {parent}\n", tree.trim()).into_bytes();
        match i {
            0 => raw.extend_from_slice(
                b"author John Smith Jr. <js@example.invalid> 1767261600 +0300\n\
                  committer \"Q\" <q@x> 1767265200 -0500\n\nJr. line\n",
            ),
            1 => raw.extend_from_slice(
                b"author \"Q\" <q@x> 1767261600 +0000\n\
                  committer John Smith Jr. <js@example.invalid> 1767265200 +0000\n\nquoted\n",
            ),
            2 => {
                raw.extend_from_slice(
                    b"author Andr\xe9 <a@example.invalid> 1767261600 +0100\n\
                      committer Andr\xe9 <a@example.invalid> 1767261600 +0100\n\
                      encoding ISO-8859-1\n\ncaf\xe9 \xff\n",
                );
            }
            _ => raw.extend_from_slice(
                b"author signer <s@example.invalid> 1767261600 +0000\n\
                  committer signer <s@example.invalid> 1767261600 +0000\n\
                  gpgsig -----BEGIN PGP SIGNATURE-----\n \n abc\n -----END PGP SIGNATURE-----\n\
                  \nsigned\n",
            ),
        }
        parent = write_commit(dir, &raw);
        sources.push(parent.clone());
    }
    git_n(dir, 1, &["update-ref", "refs/heads/ABC-1", &parent]);

    export(dir, "main", tmp.path());
    let exported: Vec<String> = commits(dir, "main..export/ABC-1").into_iter().map(|(_, h)| h).collect();
    assert_eq!(exported.len(), sources.len());
    for (new, old) in exported.iter().zip(&sources) {
        let new_raw = raw_commit(dir, new);
        assert_eq!(
            String::from_utf8_lossy(&kept_lines(&new_raw)),
            String::from_utf8_lossy(&kept_lines(&raw_commit(dir, old))),
        );
        assert_eq!(kept_lines(&new_raw), kept_lines(&raw_commit(dir, old)));
        assert!(!new_raw.windows(7).any(|w| w == b"gpgsig "));
    }
}

// STP-31 AC-15, AC-13 (CR2-2, CR2-3, ER2-6): a commit stored with a malformed ident is copied, a
// `mergetag` and a `gpgsig-sha256` header are dropped, and a base branch named `-x` is refused
// naming `--onto`.
#[test]
fn copies_odd_commits_and_base_names() {
    use std::io::Write;

    fn raw_commit(dir: &Path, commit: &str) -> Vec<u8> {
        let out = std::process::Command::new("git")
            .args(["cat-file", "commit", commit])
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(out.status.success());
        out.stdout
    }

    fn write_literally(dir: &Path, raw: &[u8]) -> String {
        let mut child = std::process::Command::new("git")
            .args(["hash-object", "-t", "commit", "-w", "--literally", "--stdin"])
            .current_dir(dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(raw).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// The raw commit without its `tree` and `parent` lines.
    fn after_parents(raw: &[u8]) -> Vec<u8> {
        let mut rest = raw;
        while rest.starts_with(b"tree ") || rest.starts_with(b"parent ") {
            let eol = rest.iter().position(|b| *b == b'\n').unwrap();
            rest = &rest[eol + 1..];
        }
        rest.to_vec()
    }

    let repo = base_repo();
    let dir = repo.path();
    let tmp = bare_dir();
    commit(
        dir,
        2,
        &[
            ("src/one.txt", "1\n"),
            (".stapel/tickets/ABC-1/state.json", "{\"key\":\"ABC-1\"}\n"),
        ],
        "one",
    );
    commit(dir, 3, &[("src/two.txt", "2\n")], "two");
    commit(dir, 4, &[("src/three.txt", "3\n")], "three");

    let originals: Vec<String> = commits(dir, "main..ABC-1").into_iter().map(|(_, h)| h).collect();
    let bodies: [(&[u8], &[u8]); 3] = [
        (
            b"author <e@example.invalid> 1767261600 +0000\n\
              committer c <c@example.invalid> 1767261600 +0000\n",
            b"\nempty name\n",
        ),
        (
            b"author a <a@example.invalid> 1767261600 +0000\n\
              committer c <c@example.invalid> 1767261600 +0000\n\
              mergetag object 0123456789012345678901234567890123456789\n \
              type commit\n tag v1\n \n note\n",
            b"\ntagged\n",
        ),
        (
            b"author a <a@example.invalid> 1767261600 +0000\n\
              committer c <c@example.invalid> 1767261600 +0000\n\
              gpgsig-sha256 -----BEGIN PGP SIGNATURE-----\n \n abc\n -----END PGP SIGNATURE-----\n",
            b"\nsigned\n",
        ),
    ];
    let mut parent = rev(dir, "main");
    let mut sources: Vec<String> = Vec::new();
    for (original, (head, message)) in originals.iter().zip(bodies) {
        let tree = git_n(dir, 1, &["rev-parse", &format!("{original}^{{tree}}")]);
        let mut raw: Vec<u8> = format!("tree {}\nparent {parent}\n", tree.trim()).into_bytes();
        raw.extend_from_slice(head);
        raw.extend_from_slice(message);
        parent = write_literally(dir, &raw);
        sources.push(parent.clone());
    }
    git_n(dir, 1, &["update-ref", "refs/heads/ABC-1", &parent]);

    export(dir, "main", tmp.path());
    let exported: Vec<String> = commits(dir, "main..export/ABC-1").into_iter().map(|(_, h)| h).collect();
    assert_eq!(exported.len(), 3);
    let expected: [Vec<u8>; 3] = [
        [bodies[0].0, bodies[0].1].concat(),
        b"author a <a@example.invalid> 1767261600 +0000\n\
          committer c <c@example.invalid> 1767261600 +0000\n\ntagged\n"
            .to_vec(),
        b"author a <a@example.invalid> 1767261600 +0000\n\
          committer c <c@example.invalid> 1767261600 +0000\n\nsigned\n"
            .to_vec(),
    ];
    for (new, want) in exported.iter().zip(&expected) {
        assert_eq!(
            String::from_utf8_lossy(&after_parents(&raw_commit(dir, new))),
            String::from_utf8_lossy(want)
        );
    }
    assert_eq!(sources.len(), 3);

    // A base branch that reads as an option is refused naming `--onto`.
    let repo = ticket_repo();
    let dir = repo.path();
    ticket_branch(dir, "ABC-3", Some("-x"), "red");
    let out = stapel(dir).args(["export", "ABC-3"]).assert().code(1);
    let err = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(err.contains("--onto"), "{err}");
    assert_eq!(export_refs(dir), "");
}

// STP-31 AC-16 (R3-2): a pull merge that also changes code (an own change or a conflict resolution) is
// kept with both parents and the change, whether the later parent is the second or the first parent.
#[test]
fn keeps_pull_merges_with_own_changes() {
    // `later_first`: the pull merge has the origin commit as its first parent.
    let run = |later_first: bool| {
        let repo = git_repo();
        let dir = repo.path();
        let tmp = bare_dir();
        git_n(dir, 1, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        commit(dir, 1, &[("src/a.txt", "a1\n")], "base");
        git_n(dir, 1, &["branch", "root"]);
        git_n(dir, 1, &["branch", "up"]);
        // Local main holds the install commit, which changes only `.stapel/`.
        commit(dir, 2, &[(".stapel/process-ledger.jsonl", "{}\n")], "install");
        git_n(dir, 3, &["checkout", "-q", "up"]);
        commit(dir, 3, &[("src/origin.txt", "origin\n")], "origin work");
        git_n(dir, 3, &["branch", "onto"]);
        // The pull merge, with its own change in `src/x.txt`.
        let (into, from) = if later_first { ("up", "main") } else { ("main", "up") };
        git_n(dir, 4, &["checkout", "-q", into]);
        git_n(dir, 4, &["merge", "-q", "--no-ff", "--no-commit", from]);
        put(dir, "src/x.txt", "resolved\n");
        git_n(dir, 4, &["add", "-A"]);
        git_n(dir, 4, &["commit", "-q", "-m", "pull"]);
        git_n(dir, 5, &["checkout", "-q", "-b", "ABC-1"]);
        commit(
            dir,
            5,
            &[("src/red.txt", "red\n"), (".stapel/tickets/ABC-1/state.json", "{}\n")],
            "RED",
        );

        export(dir, "onto", tmp.path());

        let new = commits(dir, "onto..export/ABC-1");
        let subjects: Vec<_> = new.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(subjects, ["pull", "RED"], "later_first={later_first}");
        let by = |s: &str| new.iter().find(|(x, _)| x == s).unwrap().1.clone();
        // The install commit is empty after the clean, so it maps to the base commit.
        let want = if later_first {
            [rev(dir, "onto"), rev(dir, "root")]
        } else {
            [rev(dir, "root"), rev(dir, "onto")]
        };
        assert_eq!(parents(dir, &by("pull")), want, "later_first={later_first}");
        assert_eq!(parents(dir, &by("RED")), [by("pull")]);
        let merge = entries(dir, &by("pull"));
        assert!(merge.contains_key("src/x.txt"), "later_first={later_first}");
        assert!(!entries(dir, &rev(dir, "onto")).contains_key("src/x.txt"));
        assert!(!merge.keys().any(|p| p.starts_with(".stapel/")));
    };
    run(false);
    run(true);
}

// STP-34 AC-14: the folder of the elaboration skill is a process path; a person's own skill folder is not.
#[test]
fn removes_the_elaboration_skill() {
    let repo = base_repo();
    let dir = repo.path();
    commit(
        dir,
        2,
        &[
            ("src/red.txt", "red\n"),
            (".stapel/tickets/ABC-1/state.json", "{\"key\":\"ABC-1\"}\n"),
            (".claude/skills/stapel-elaborate/SKILL.md", "ticket skill\n"),
            (".claude/skills/stapel-foo/SKILL.md", "foo\n"),
        ],
        "RED",
    );
    git_n(dir, 3, &["checkout", "-q", "main"]);
    commit(
        dir,
        3,
        &[(".claude/skills/stapel-elaborate/SKILL.md", "onto skill\n")],
        "onto skill",
    );
    git_n(dir, 4, &["checkout", "-q", "ABC-1"]);
    let tmp = bare_dir();
    export(dir, "main", tmp.path());

    let onto = entries(dir, "main");
    let elaborate = ".claude/skills/stapel-elaborate/SKILL.md";
    let new = commits(dir, "main..export/ABC-1");
    assert_eq!(new.len(), 1);
    let first = entries(dir, &new[0].1);
    assert_eq!(first.get(elaborate), onto.get(elaborate), "{first:?}");
    assert!(first.contains_key(".claude/skills/stapel-foo/SKILL.md"), "{first:?}");
    let tree = entries(dir, "export/ABC-1");
    assert_eq!(tree.get(elaborate), onto.get(elaborate), "{tree:?}");
    assert!(tree.contains_key(".claude/skills/stapel-foo/SKILL.md"));
}

// STP-38 AC-7: `export` writes `merge-request.md` with the title, the Summary body as it is and the
// deferred findings that are not false; a second export rewrites it; the guard denies an agent write.
#[test]
fn writes_the_merge_request() {
    let repo = ticket_repo();
    let dir = repo.path();
    ticket_branch(dir, "ABC-1", Some("main"), "red");
    git_n(dir, 1, &["checkout", "-q", "ABC-1"]);
    let folder = ".stapel/tickets/ABC-1";
    put(
        dir,
        &format!("{folder}/state.json"),
        "{\"schema_version\":1,\"key\":\"ABC-1\",\"title\":\"Merge text\",\"base_branch\":\"main\"}\n",
    );
    let summary = "Line one.\n\nLine two with `code`.\n";
    put(
        dir,
        &format!("{folder}/ticket.md"),
        &format!("# ABC-1 \u{2014} Merge text\n\n## Spec\n\nThe spec.\n\n## Summary\n\n{summary}"),
    );
    let finding = |id: &str, severity: &str, stage: &str, round: u32, claim: &str, fate: &str| {
        serde_json::json!({
            "id": id, "ticket": "ABC-1", "stage": stage, "round": round, "role": "reviewer",
            "severity": severity, "promise": "inside", "claim": claim,
            "catchable_at": "code", "fate": fate,
        })
        .to_string()
    };
    let triage = |id: &str, fate: &str, verdict: &str| {
        serde_json::json!({
            "kind": "triage", "id": id, "ticket": "ABC-1", "fate": fate, "verdict": verdict,
        })
        .to_string()
    };
    let lines = [
        finding("F1", "MEDIUM", "code-review", 1, "first claim", "defer"),
        finding("F2", "HIGH", "code-review", 1, "second claim", "defer"),
        finding("F3", "LOW", "spec-review", 2, "third claim", "defer"),
        finding("F4", "HIGH", "code-review", 1, "fourth claim", "patch"),
        triage("F2", "defer", "false"),
        triage("F3", "defer", "high"),
    ];
    let jsonl = |lines: &[String]| lines.iter().map(|l| format!("{l}\n")).collect::<String>();
    put(dir, &format!("{folder}/findings.jsonl"), &jsonl(&lines));

    let tmp = bare_dir();
    let run = |dir: &Path| {
        stapel(dir)
            .env("TMPDIR", tmp.path())
            .args(["export", "ABC-1", "--onto", "main"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone()
    };
    let out = String::from_utf8_lossy(&run(dir)).into_owned();
    let path = format!("{folder}/merge-request.md");
    assert!(
        out.lines().any(|l| l == format!("merge request: {path}")),
        "{out}"
    );
    let text = common::read(dir, &path);
    assert!(text.starts_with("# ABC-1 \u{2014} Merge text\n"), "{text}");
    let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("no {needle:?} in {text}"));
    assert!(at(summary) < at("## Deferred findings\n"), "{text}");
    let tail = &text[at("## Deferred findings\n")..];
    let items: Vec<&str> = tail.lines().filter(|l| l.starts_with("- ")).collect();
    assert_eq!(
        items,
        [
            "- F1 MEDIUM code-review round 1: first claim",
            "- F3 HIGH spec-review round 2: third claim",
        ],
        "{text}"
    );

    // A second export rewrites the file; with no deferred finding it holds `none`.
    put(
        dir,
        &format!("{folder}/findings.jsonl"),
        &jsonl(&[finding("F4", "HIGH", "code-review", 1, "fourth claim", "patch")]),
    );
    run(dir);
    let text = common::read(dir, &path);
    let tail = &text[text.find("## Deferred findings\n").unwrap()..];
    assert!(tail.lines().any(|l| l == "none"), "{text}");
    assert!(!text.contains("first claim"), "{text}");

    // The guard denies an agent write of the file.
    let payload = serde_json::json!({
        "session_id": "test",
        "hook_event_name": "PreToolUse",
        "cwd": dir,
        "tool_name": "Write",
        "tool_input": { "file_path": dir.join(&path).display().to_string(), "content": "x" },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .assert()
        .code(2)
        .stderr(predicates::str::contains("only stapel"));
}

// STP-38 AC-7: from a checkout without the ticket's state.json the export writes no merge-request.md
// and prints no `merge request:` line.
#[test]
fn without_state_json_no_merge_request() {
    let repo = ticket_repo();
    let dir = repo.path();
    ticket_branch(dir, "ABC-1", Some("main"), "red");
    let tmp = bare_dir();
    let out = stapel(dir)
        .env("TMPDIR", tmp.path())
        .args(["export", "ABC-1", "--onto", "main"])
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(!text.contains("merge request:"), "{text}");
    assert!(!dir.join(".stapel/tickets/ABC-1").exists());
}
