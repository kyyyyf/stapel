//! STP-24: `stapel review start KEY --round N` and its refusals (step 1).

mod common;

use common::{repo_with_ticket, snapshot, stapel, state_json};
use predicates::str::contains;
use std::path::Path;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(["-c", "user.email=none", "-c", "user.name=test-user"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8(out.stdout).unwrap()
}

fn write_state(dir: &Path, key: &str, v: &serde_json::Value) {
    std::fs::write(
        dir.join(format!(".stapel/tickets/{key}/state.json")),
        serde_json::to_string_pretty(v).unwrap(),
    )
    .unwrap();
}

/// A ticket ABC-1 with a base commit and one later commit; the tree is clean.
fn built() -> tempfile::TempDir {
    let repo = repo_with_ticket();
    let dir = repo.path();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "initial"]);
    let base = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let mut state = state_json(dir, "ABC-1");
    state["base"] = serde_json::json!(base);
    write_state(dir, "ABC-1", &state);
    std::fs::write(dir.join("code.txt"), "one\n").unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "ABC-1 step 1 GREEN: code"]);
    repo
}

fn findings(dir: &Path, lines: &[&str]) {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(dir.join(".stapel/tickets/ABC-1/findings.jsonl"), text).unwrap();
}

const ROUND_ONE: &str = r#"{"v":1,"stage":"code-review","round":"1","id":"R-1"}"#;

fn start(dir: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    stapel(dir).args(["review", "start"]).args(args).assert()
}

/// The command refuses with `code` and `text` after `stapel: `, and writes nothing.
fn refused(dir: &Path, args: &[&str], code: i32, text: &str) {
    let before = snapshot(dir);
    start(dir, args)
        .code(code)
        .stderr(format!("stapel: {text}\n"));
    assert_eq!(snapshot(dir), before, "{text}: files changed");
    no_review_folder(dir);
}

fn no_review_folder(dir: &Path) {
    assert!(
        !dir.join(".stapel/scratch/review").exists(),
        "a review folder was written"
    );
}

fn close(dir: &Path) {
    let mut state = state_json(dir, "ABC-1");
    state["closed"] =
        serde_json::json!({"by": "test-user", "at": "2026-10-19T10:00:00Z", "reason": "done"});
    write_state(dir, "ABC-1", &state);
}

#[test]
fn refuses_tickets() {
    let repo = built();
    let dir = repo.path();
    refused(
        dir,
        &["ABC-9", "--round", "1"],
        2,
        "no ticket ABC-9 in .stapel/tickets",
    );

    // The key is case-insensitive.
    start(dir, &["abc-1", "--round", "1"]).code(0);
    std::fs::remove_dir_all(dir.join(".stapel/scratch")).unwrap();

    let mut state = state_json(dir, "ABC-1");
    close(dir);
    refused(
        dir,
        &["ABC-1", "--round", "1"],
        2,
        "ticket ABC-1 is closed; it has no review round",
    );

    state.as_object_mut().unwrap().remove("schema_version");
    write_state(dir, "ABC-1", &state);
    refused(
        dir,
        &["ABC-1", "--round", "1"],
        1,
        "ticket ABC-1 is in the STP-1 format; it has no review pack",
    );
}

#[test]
fn refuses_a_dirty_tree() {
    let repo = built();
    let dir = repo.path();
    let run = |dir: &Path| start(dir, &["ABC-1", "--round", "1"]);
    let dirty =
        |path: &str| format!("uncommitted changes: {path}; commit them before a review round");
    let args = ["ABC-1", "--round", "1"];

    std::fs::write(dir.join("stray.txt"), "x").unwrap();
    refused(dir, &args, 2, &dirty("stray.txt"));
    std::fs::remove_file(dir.join("stray.txt")).unwrap();

    std::fs::write(dir.join("code.txt"), "two\n").unwrap();
    git(dir, &["add", "code.txt"]);
    refused(dir, &args, 2, &dirty("code.txt"));
    git(dir, &["restore", "--staged", "--worktree", "code.txt"]);

    let ticket = dir.join(".stapel/tickets/ABC-1/ticket.md");
    let text = std::fs::read_to_string(&ticket).unwrap();
    std::fs::write(&ticket, format!("{text}\nmore\n")).unwrap();
    refused(dir, &args, 2, &dirty(".stapel/tickets/ABC-1/ticket.md"));
    git(dir, &["restore", ".stapel/tickets/ABC-1/ticket.md"]);

    // Seven dirty paths: the text names the first five.
    for n in 1..=7 {
        std::fs::write(dir.join(format!("d{n}.txt")), "x").unwrap();
    }
    refused(
        dir,
        &args,
        2,
        &dirty("d1.txt, d2.txt, d3.txt, d4.txt, d5.txt"),
    );
    for n in 1..=7 {
        std::fs::remove_file(dir.join(format!("d{n}.txt"))).unwrap();
    }

    // An ignored file and a machine file do not count.
    std::fs::write(dir.join(".git/info/exclude"), "ignored.txt\n").unwrap();
    std::fs::write(dir.join("ignored.txt"), "x").unwrap();
    findings(dir, &[ROUND_ONE]);
    run(dir).code(0);
}

#[test]
fn refuses_bad_rounds() {
    let repo = built();
    let dir = repo.path();
    for round in ["0", "3"] {
        refused(
            dir,
            &["ABC-1", "--round", round],
            2,
            "--round must be 1 or 2; after round 2 the person decides",
        );
    }
    for args in [
        vec!["ABC-1", "--round", "x"],
        vec!["ABC-1", "--round", "-1"],
        vec!["ABC-1"],
    ] {
        start(dir, &args).code(2).stderr(contains("error:"));
    }
    no_review_folder(dir);
}

#[test]
fn refuses_a_bad_base() {
    let repo = built();
    let dir = repo.path();
    let good = state_json(dir, "ABC-1");

    let mut state = good.clone();
    state.as_object_mut().unwrap().remove("base");
    write_state(dir, "ABC-1", &state);
    refused(
        dir,
        &["ABC-1", "--round", "1"],
        1,
        "state.json of ABC-1 has no base commit",
    );

    let bad = |base: &str| {
        format!(
            "state.json has base {base}, which is not a full commit id on the first-parent history of HEAD"
        )
    };
    let mut state = good.clone();
    let unknown = "0".repeat(40);
    state["base"] = serde_json::json!(unknown);
    write_state(dir, "ABC-1", &state);
    refused(dir, &["ABC-1", "--round", "1"], 1, &bad(&unknown));

    // A base that exists but lies off the first-parent history: the second parent of a merge.
    let first = good["base"].as_str().unwrap().to_string();
    let tree = git(dir, &["rev-parse", &format!("{first}^{{tree}}")])
        .trim()
        .to_string();
    let side = git(dir, &["commit-tree", &tree, "-p", &first, "-m", "side"])
        .trim()
        .to_string();
    git(dir, &["merge", "-q", "--no-ff", "-m", "merge", &side]);
    let mut state = good;
    state["base"] = serde_json::json!(side);
    write_state(dir, "ABC-1", &state);
    refused(dir, &["ABC-1", "--round", "1"], 1, &bad(&side));
}

#[test]
fn round_two_needs_round_one() {
    let repo = built();
    let dir = repo.path();
    let text = "stapel: round 2 needs the code review findings of round 1 in .stapel/tickets/ABC-1/findings.jsonl";
    let refuse = |dir: &Path| {
        start(dir, &["ABC-1", "--round", "2"])
            .code(2)
            .stderr(contains(text));
        no_review_folder(dir);
    };
    refuse(dir);
    findings(
        dir,
        &[
            r#"{"v":1,"stage":"spec-review","round":"1","id":"S-1"}"#,
            r#"{"v":1,"stage":"step-drift","round":"1","id":"D-1"}"#,
        ],
    );
    refuse(dir);
    findings(
        dir,
        &[r#"{"v":1,"stage":"code-review","round":"2","id":"R-1"}"#],
    );
    refuse(dir);
    findings(dir, &[ROUND_ONE]);
    start(dir, &["ABC-1", "--round", "2"]).code(0);
}

#[test]
fn refusal_order() {
    let repo = built();
    let dir = repo.path();
    let good = state_json(dir, "ABC-1");
    std::fs::write(dir.join("stray.txt"), "x").unwrap();

    close(dir);
    refused(
        dir,
        &["ABC-1", "--round", "3"],
        2,
        "ticket ABC-1 is closed; it has no review round",
    );

    // Legacy and closed in one state.json, with a dirty tree and a bad round: legacy wins.
    let mut legacy = state_json(dir, "ABC-1");
    legacy.as_object_mut().unwrap().remove("schema_version");
    write_state(dir, "ABC-1", &legacy);
    refused(
        dir,
        &["ABC-1", "--round", "3"],
        1,
        "ticket ABC-1 is in the STP-1 format; it has no review pack",
    );

    std::fs::remove_file(dir.join("stray.txt")).unwrap();
    let mut state = good;
    let unknown = "0".repeat(40);
    state["base"] = serde_json::json!(unknown);
    write_state(dir, "ABC-1", &state);
    refused(
        dir,
        &["ABC-1", "--round", "2"],
        1,
        &format!(
            "state.json has base {unknown}, which is not a full commit id on the first-parent history of HEAD"
        ),
    );
}

#[test]
fn refusal_order_of_the_later_cases() {
    let repo = built();
    let dir = repo.path();
    let good = state_json(dir, "ABC-1");
    let dirty = "uncommitted changes: stray.txt; commit them before a review round";

    // An unknown key wins over a bad round.
    refused(
        dir,
        &["ABC-9", "--round", "3"],
        2,
        "no ticket ABC-9 in .stapel/tickets",
    );

    // A bad round wins over a dirty tree.
    std::fs::write(dir.join("stray.txt"), "x").unwrap();
    refused(
        dir,
        &["ABC-1", "--round", "3"],
        2,
        "--round must be 1 or 2; after round 2 the person decides",
    );

    // A dirty tree wins over a missing base and over a bad base.
    let mut no_base = good.clone();
    no_base.as_object_mut().unwrap().remove("base");
    write_state(dir, "ABC-1", &no_base);
    refused(dir, &["ABC-1", "--round", "1"], 2, dirty);

    let mut bad_base = good;
    bad_base["base"] = serde_json::json!("0".repeat(40));
    write_state(dir, "ABC-1", &bad_base);
    refused(dir, &["ABC-1", "--round", "1"], 2, dirty);
}

fn tree(dir: &Path, round: u32) -> std::path::PathBuf {
    dir.join(format!(".stapel/scratch/review/ABC-1-r{round}/tree"))
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

#[test]
fn copies_head() {
    use std::os::unix::fs::PermissionsExt;
    let repo = built();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join("dir")).unwrap();
    std::fs::write(dir.join("a b.txt"), "spaced\n").unwrap();
    std::fs::write(dir.join("dir/é.rs"), "fn main() {}\n").unwrap();
    std::fs::write(dir.join("run.sh"), "#!/bin/sh\necho hi\n").unwrap();
    std::fs::set_permissions(dir.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(dir.join("empty"), "").unwrap();
    std::fs::write(dir.join("gone.txt"), "x\n").unwrap();
    commit_all(dir, "more files");
    git(dir, &["rm", "-q", "gone.txt"]);
    git(dir, &["commit", "-q", "-m", "delete"]);

    start(dir, &["ABC-1", "--round", "1"]).code(0);
    let t = tree(dir, 1);
    assert_eq!(std::fs::read(t.join("a b.txt")).unwrap(), b"spaced\n");
    assert_eq!(std::fs::read(t.join("dir/é.rs")).unwrap(), b"fn main() {}\n");
    assert_eq!(std::fs::read(t.join("empty")).unwrap(), b"");
    assert_eq!(std::fs::read(t.join("code.txt")).unwrap(), b"one\n");
    let mode = std::fs::metadata(t.join("run.sh")).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0o111, "the executable bit");
    assert!(!t.join(".git").exists());
    assert!(!t.join("gone.txt").exists());
}

#[test]
fn copy_ignores_autocrlf() {
    let repo = built();
    let dir = repo.path();
    common::git_config(dir, "core.autocrlf", "true");
    std::fs::write(dir.join("lf.txt"), "one\ntwo\n").unwrap();
    git(dir, &["-c", "core.autocrlf=false", "add", "-A"]);
    git(dir, &["commit", "-q", "-m", "lf"]);

    start(dir, &["ABC-1", "--round", "1"]).code(0);
    assert_eq!(
        std::fs::read(tree(dir, 1).join("lf.txt")).unwrap(),
        b"one\ntwo\n"
    );
}

#[test]
fn rerun_replaces_the_copy() {
    let repo = built();
    let dir = repo.path();
    start(dir, &["ABC-1", "--round", "1"]).code(0);
    std::fs::write(tree(dir, 1).join("stray.txt"), "x").unwrap();
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    commit_all(dir, "new");

    start(dir, &["ABC-1", "--round", "1"]).code(0);
    assert!(!tree(dir, 1).join("stray.txt").exists());
    assert_eq!(std::fs::read(tree(dir, 1).join("new.txt")).unwrap(), b"new\n");
}

#[test]
fn copy_keeps_the_index() {
    let repo = built();
    let dir = repo.path();
    let index = || std::fs::read(dir.join(".git/index")).unwrap();
    let state = || {
        (
            git(dir, &["status", "--porcelain"]),
            git(dir, &["rev-parse", "HEAD"]),
        )
    };
    let (before_index, before_state) = (index(), state());

    start(dir, &["ABC-1", "--round", "1"]).code(0);
    assert!(tree(dir, 1).join("code.txt").exists(), "the copy");
    assert!(index() == before_index, ".git/index changed");
    assert_eq!(state(), before_state);
    assert!(!dir.join(".stapel/scratch/review/ABC-1-r1/index").exists());
    assert!(!tree(dir, 1).join("index").exists());
    assert_eq!(
        std::fs::read(dir.join(".stapel/scratch/review/.gitignore")).unwrap(),
        b"*\n"
    );
    assert!(
        !dir.join(".stapel/scratch/review/ABC-1-r1/.gitignore").exists(),
        "the round folder has a .gitignore of its own"
    );
}

#[test]
fn copy_needs_no_tar() {
    let repo = built();
    let dir = repo.path();
    let git_path = String::from_utf8(
        std::process::Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let only_git = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(git_path.trim(), only_git.path().join("git")).unwrap();

    stapel(dir)
        .env("PATH", only_git.path())
        .args(["review", "start", "ABC-1", "--round", "1"])
        .assert()
        .code(0);
    assert_eq!(std::fs::read(tree(dir, 1).join("code.txt")).unwrap(), b"one\n");
}

#[test]
fn removes_old_copies() {
    let repo = built();
    let dir = repo.path();
    for title in ["Second", "Third"] {
        stapel(dir).args(["new", title]).assert().success();
    }
    let mut state = state_json(dir, "ABC-2");
    state["closed"] =
        serde_json::json!({"by": "test-user", "at": "2026-10-19T10:00:00Z", "reason": "done"});
    write_state(dir, "ABC-2", &state);
    commit_all(dir, "more tickets");
    findings(dir, &[ROUND_ONE]);

    let review = dir.join(".stapel/scratch/review");
    for name in ["ABC-1-r1", "ABC-2-r1", "ABC-3-r1", "ABC-2-rx", "notes"] {
        std::fs::create_dir_all(review.join(name)).unwrap();
        std::fs::write(review.join(name).join("file"), "x").unwrap();
    }
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("keep"), "x").unwrap();
    std::os::unix::fs::symlink(outside.path(), review.join("ABC-2-r2")).unwrap();
    std::fs::write(review.join(".gitignore"), "*\n").unwrap();

    start(dir, &["ABC-1", "--round", "2"]).code(0);
    assert!(review.join("ABC-1-r2/tree/code.txt").exists());
    assert!(!review.join("ABC-1-r1").exists(), "another round of the ticket");
    assert!(!review.join("ABC-2-r1").exists(), "a closed ticket");
    assert!(
        review.join("ABC-2-r2").symlink_metadata().is_err(),
        "the symlink"
    );
    assert!(outside.path().join("keep").exists(), "the symlink target");
    assert!(review.join("ABC-3-r1/file").exists(), "an open ticket");
    assert!(review.join("ABC-2-rx/file").exists(), "not a round name");
    assert!(review.join("notes/file").exists(), "another name");
}

#[test]
fn copy_follows_attributes() {
    let repo = built();
    let dir = repo.path();
    std::fs::write(dir.join(".gitattributes"), "*.txt text eol=crlf\n").unwrap();
    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    commit_all(dir, "attributes");
    assert_eq!(
        git(dir, &["cat-file", "blob", "HEAD:a.txt"]),
        "one\ntwo\n",
        "the blob has LF endings"
    );

    start(dir, &["ABC-1", "--round", "1"]).code(0);
    assert_eq!(
        std::fs::read(tree(dir, 1).join("a.txt")).unwrap(),
        b"one\r\ntwo\r\n"
    );
}

#[test]
fn copy_keeps_plain_modes() {
    use std::os::unix::fs::PermissionsExt;
    let repo = built();
    let dir = repo.path();
    std::fs::write(dir.join("plain.txt"), "plain\n").unwrap();
    std::fs::set_permissions(dir.join("plain.txt"), std::fs::Permissions::from_mode(0o644))
        .unwrap();
    std::os::unix::fs::symlink("code.txt", dir.join("link")).unwrap();
    commit_all(dir, "plain and link");
    let refs_before = git(dir, &["for-each-ref"]);

    start(dir, &["ABC-1", "--round", "1"]).code(0);
    let t = tree(dir, 1);
    let mode = std::fs::metadata(t.join("plain.txt")).unwrap().permissions().mode();
    assert_eq!(mode & 0o111, 0, "a plain file stays non-executable");
    let link = t.join("link");
    assert!(
        link.symlink_metadata().unwrap().file_type().is_symlink(),
        "a symlink stays a symlink"
    );
    assert_eq!(std::fs::read_link(&link).unwrap(), Path::new("code.txt"));
    assert_eq!(git(dir, &["for-each-ref"]), refs_before, "the refs");
}

#[test]
fn removes_old_copies_edges() {
    let repo = built();
    let dir = repo.path();
    stapel(dir).args(["new", "Second"]).assert().success();
    let odd = dir.join(".stapel/tickets/AB-r1-1");
    std::fs::create_dir_all(&odd).unwrap();
    for entry in std::fs::read_dir(dir.join(".stapel/tickets/ABC-2")).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, odd.join(path.file_name().unwrap())).unwrap();
    }
    let mut state = state_json(dir, "AB-r1-1");
    state["key"] = serde_json::json!("AB-r1-1");
    write_state(dir, "AB-r1-1", &state);
    commit_all(dir, "more tickets");

    let review = dir.join(".stapel/scratch/review");
    for name in ["ABC-1-r02", "AB-r1-1", "ABC-1-r1"] {
        std::fs::create_dir_all(review.join(name)).unwrap();
        std::fs::write(review.join(name).join("file"), "x").unwrap();
    }
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("keep"), "x").unwrap();
    std::os::unix::fs::symlink(outside.path(), review.join("ABC-1-r3")).unwrap();
    std::fs::write(review.join(".gitignore"), "*\n").unwrap();

    start(dir, &["ABC-1", "--round", "1"]).code(0);
    assert!(review.join("ABC-1-r1/tree/code.txt").exists(), "the current round");
    assert!(!review.join("ABC-1-r1/file").exists(), "the current folder is replaced");
    assert!(
        review.join("ABC-1-r3").symlink_metadata().is_err(),
        "the symlink"
    );
    assert!(outside.path().join("keep").exists(), "the symlink target");
    assert!(!review.join("ABC-1-r02").exists(), "a whole number");
    assert!(review.join("AB-r1-1/file").exists(), "an open ticket with -r in its key");
}

const SPEC_TWO: &str = "| № | Criterion | Test |\n|---|---|---|\n\
| AC-1 | first criterion | `t::a` |\n| AC-2 | second criterion | `t::b` |";

/// Like `built()`, with a Spec of two criteria and an extra tracked `files` before the base.
fn built_with_spec(files: &[(&str, &str)]) -> tempfile::TempDir {
    let repo = repo_with_ticket();
    let dir = repo.path();
    common::set_section(dir, "ABC-1", "Spec", SPEC_TWO);
    for (name, text) in files {
        std::fs::write(dir.join(name), text).unwrap();
    }
    commit_all(dir, "initial");
    let base = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let mut state = state_json(dir, "ABC-1");
    state["base"] = serde_json::json!(base);
    write_state(dir, "ABC-1", &state);
    std::fs::write(dir.join("code.txt"), "one\n").unwrap();
    commit_all(dir, "ABC-1 step 1 GREEN: code");
    repo
}

/// `pack.md` of a round with the base and HEAD commits replaced by `<base>` and `<head>`.
fn pack_of(dir: &Path, round: u32) -> String {
    let base = state_json(dir, "ABC-1")["base"].as_str().unwrap().to_string();
    let head = git(dir, &["rev-parse", "HEAD"]).trim().to_string();
    let text = std::fs::read_to_string(
        dir.join(format!(".stapel/scratch/review/ABC-1-r{round}/pack.md")),
    )
    .unwrap();
    text.replace(&base, "<base>").replace(&head, "<head>")
}

fn golden(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(name);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn pack_of_round_one() {
    let repo = built_with_spec(&[]);
    let dir = repo.path();
    start(dir, &["ABC-1", "--round", "1"]).code(0);
    assert_eq!(pack_of(dir, 1), golden("review_pack_r1.md"));
}

#[test]
fn diff_file() {
    let repo = built_with_spec(&[("old.txt", "a\nb\nc\nd\n")]);
    let dir = repo.path();
    git(dir, &["mv", "old.txt", "é.txt"]);
    commit_all(dir, "rename");
    for (key, value) in [
        ("diff.external", "echo external"),
        ("color.diff", "always"),
        ("diff.noprefix", "true"),
        ("diff.renames", "false"),
        ("core.quotepath", "true"),
    ] {
        common::git_config(dir, key, value);
    }
    start(dir, &["ABC-1", "--round", "1"]).code(0);
    for key in [
        "diff.external",
        "color.diff",
        "diff.noprefix",
        "diff.renames",
        "core.quotepath",
    ] {
        git(dir, &["config", "--unset", key]);
    }
    let base = state_json(dir, "ABC-1")["base"].as_str().unwrap().to_string();
    let range = format!("{base}..HEAD");
    let expected = git(
        dir,
        &[
            "-c",
            "core.quotepath=false",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            &range,
        ],
    );
    assert!(expected.contains("rename to é.txt"), "{expected}");
    let written =
        std::fs::read_to_string(dir.join(".stapel/scratch/review/ABC-1-r1/diff.patch")).unwrap();
    assert_eq!(written, expected);
}

#[test]
fn pack_of_round_two() {
    let repo = built_with_spec(&[]);
    let dir = repo.path();
    findings(
        dir,
        &[
            r#"{"v":1,"stage":"code-review","round":"1","id":"R-1","severity":"HIGH","claim":"first claim"}"#,
            r#"{"v":1,"stage":"code-review","round":"1","id":"R-2","severity":"LOW","claim":"second claim"}"#,
            r#"{"v":1,"stage":"code-review","round":"1","id":"R-3","severity":"HIGH","claim":"third claim"}"#,
            r#"{"v":1,"stage":"step-drift","round":"1","id":"R-4","severity":"HIGH","claim":"fourth claim"}"#,
            r#"{"v":1,"kind":"triage","id":"R-1","fate":"patch","verdict":"medium"}"#,
            r#"{"v":1,"kind":"triage","id":"R-2","fate":"defer"}"#,
            r#"{"v":1,"kind":"triage","id":"R-3","fate":"patch"}"#,
            r#"{"v":1,"kind":"triage","id":"R-4","fate":"patch"}"#,
            r#"{"v":1,"kind":"triage","id":"R-3","fate":"patch","verdict":"false"}"#,
        ],
    );
    start(dir, &["ABC-1", "--round", "2"]).code(0);
    assert_eq!(pack_of(dir, 2), golden("review_pack_r2.md"));

    findings(
        dir,
        &[
            r#"{"v":1,"stage":"code-review","round":"1","id":"R-2","severity":"LOW","claim":"second claim"}"#,
            r#"{"v":1,"kind":"triage","id":"R-2","fate":"defer"}"#,
        ],
    );
    start(dir, &["ABC-1", "--round", "2"]).code(0);
    let pack = pack_of(dir, 2);
    assert!(pack.contains("\n- none\n"), "{pack}");
    assert!(!pack.contains("R-2"), "{pack}");
}
