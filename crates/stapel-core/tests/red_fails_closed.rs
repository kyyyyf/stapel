//! STP-13 AC-8 (D4-1): a git error while the files named by manifests are listed counts as code.

use stapel_core::outcomes::red_changes_code;
use std::path::Path;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "test-user")
        .env("GIT_AUTHOR_EMAIL", "test-user.invalid")
        .env("GIT_COMMITTER_NAME", "test-user")
        .env("GIT_COMMITTER_EMAIL", "test-user.invalid")
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

fn repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    let dir = repo.path();
    git(dir, &["init", "-q"]);
    write(
        dir,
        "Cargo.toml",
        "[package]\nname = \"t\"\nversion = \"0.1.0\"\n\n[lib]\npath = \"crates/t/tests/gen.rs\"\n",
    );
    write(dir, "crates/t/tests/gen.rs", "fn one() {}\n");
    write(dir, "docs/a.txt", "a\n");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "base"]);
    write(dir, "crates/t/tests/gen.rs", "fn two() {}\n");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "red"]);
    repo
}

#[test]
fn unreadable_tree_counts_as_code() {
    let repo = repo();
    let dir = repo.path();
    // A different file that is no code by STP-4 and that no manifest names.
    write(dir, "crates/t/tests/other.rs", "fn x() {}\n");
    git(dir, &["add", "crates/t/tests/other.rs"]);
    git(dir, &["commit", "-q", "-m", "red two"]);
    let red2 = git(dir, &["rev-parse", "HEAD"]);
    // An unchanged subtree whose object is gone: the diff skips it, a recursive listing fails.
    let docs = git(dir, &["rev-parse", "HEAD:docs"]);
    let docs = docs.trim();
    let object = dir.join(".git/objects").join(&docs[..2]).join(&docs[2..]);
    assert!(object.exists(), "{object:?}");
    std::fs::remove_file(&object).unwrap();
    let result = red_changes_code(dir, red2.trim());
    assert_eq!(result.as_ref().ok(), Some(&true), "{result:?}");
}
