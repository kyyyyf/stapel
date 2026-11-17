//! STP-32: `stapel page [KEY]` writes the page of a ticket as a file.

mod common;

use common::{git_in, read, repo_with_ticket, stapel, state_json, two_ticket_worktrees};
use sha2::{Digest, Sha256};
use std::path::Path;

const IGNORE: &str = "/.stapel/tickets/*/page.md";
const PAGE: &str = ".stapel/tickets/ABC-1/page.md";
const NOT_IGNORED: &str = ".stapel/tickets/ABC-1/page.md is not ignored by git; add \
     /.stapel/tickets/*/page.md to the ignore lines or run stapel init";

fn ignore_in_gitignore(dir: &Path) {
    let path = dir.join(".gitignore");
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!("{IGNORE}\n"));
    std::fs::write(path, text).unwrap();
}

fn stdout_of(dir: &Path, args: &[&str]) -> String {
    let out = stapel(dir).args(args).assert().success().get_output().clone();
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn writes_the_page() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    ignore_in_gitignore(dir);
    let out = stdout_of(dir, &["page", "ABC-1"]);
    let bytes = std::fs::read(dir.join(PAGE)).unwrap();
    let hex: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(out, format!("page: {PAGE} (sha256:{})\n", &hex[..12]));
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        "# ABC-1 — First\n\n## Sections\n- spec: new, 1 line\n- design: new, 1 line\n\
         - proof: new, 1 line\n- plan: new, 1 line\n"
    );

    // Run again with no edit, and with no KEY: the same bytes.
    let again = stdout_of(dir, &["page"]);
    assert_eq!(again, out);
    assert_eq!(std::fs::read(dir.join(PAGE)).unwrap(), bytes);
}

#[test]
fn nothing_waits() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    ignore_in_gitignore(dir);
    stapel(dir).args(["ok", "ABC-1", "spec"]).assert().success();
    stapel(dir).args(["ok", "ABC-1", "design"]).assert().success();
    stapel(dir).args(["ok", "ABC-1", "proof"]).assert().success();
    stapel(dir).args(["ok", "ABC-1", "plan"]).assert().success();
    assert_eq!(
        stdout_of(dir, &["page", "ABC-1"]),
        "nothing to confirm: ABC-1\n"
    );
    assert!(!dir.join(PAGE).exists());
}

#[test]
fn refusals_as_ok() {
    // An unknown key: exit 1, no file.
    let repo = repo_with_ticket();
    let dir = repo.path();
    ignore_in_gitignore(dir);
    stapel(dir)
        .args(["page", "ABC-9"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("no ticket ABC-9"));
    assert!(!dir.join(PAGE).exists());

    // A closed ticket: the text of `stapel ok ABC-1`.
    let mut state = state_json(dir, "ABC-1");
    state["closed"] = serde_json::json!({"by": "x", "at": "2026-10-02T00:00:00Z", "reason": "done"});
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/state.json"),
        state.to_string(),
    )
    .unwrap();
    let ok = stapel(dir).args(["ok", "ABC-1"]).assert().code(1).get_output().clone();
    let page = stapel(dir).args(["page", "ABC-1"]).assert().code(1).get_output().clone();
    assert!(String::from_utf8_lossy(&page.stderr).contains("was closed by x"));
    assert_eq!(page.stderr, ok.stderr);
    assert!(!dir.join(PAGE).exists());

    // A foreign key in a ticket worktree: the code and text of `stapel ok ABC-1`.
    let pair = two_ticket_worktrees();
    let b = &pair.b;
    ignore_in_gitignore(b);
    // The worktree of ABC-2 also holds the folder of ABC-1, as a worktree made after it does.
    std::fs::create_dir_all(b.join(".stapel/tickets/ABC-1")).unwrap();
    for file in ["ticket.md", "state.json"] {
        std::fs::copy(
            pair.a.join(".stapel/tickets/ABC-1").join(file),
            b.join(".stapel/tickets/ABC-1").join(file),
        )
        .unwrap();
    }
    let ok = stapel(b).args(["ok", "ABC-1"]).assert().code(2).get_output().clone();
    let page = stapel(b).args(["page", "ABC-1"]).assert().code(2).get_output().clone();
    assert!(
        String::from_utf8_lossy(&page.stderr)
            .contains("ABC-1 is not the ticket of branch ABC-2; run it in .claude/worktrees/ABC-1")
    );
    assert_eq!(page.stderr, ok.stderr);
    assert!(!b.join(".stapel/tickets/ABC-1/page.md").exists());
    assert!(!b.join(".stapel/tickets/ABC-2/page.md").exists());
}

#[test]
fn refuses_when_not_ignored() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    // No ignore line at all: exit 1, the AC-1 text, no file. `init` may write the line; drop it.
    let gitignore: String = read(dir, ".gitignore")
        .lines()
        .filter(|l| !l.contains("page.md"))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(dir.join(".gitignore"), gitignore).unwrap();
    stapel(dir)
        .args(["page", "ABC-1"])
        .assert()
        .code(1)
        .stderr(format!("stapel: {NOT_IGNORED}\n"));
    assert!(!dir.join(PAGE).exists());

    // The line in .git/info/exclude only: written.
    let exclude = dir.join(".git/info/exclude");
    let mut text = std::fs::read_to_string(&exclude).unwrap_or_default();
    text.push_str(&format!("{IGNORE}\n"));
    std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
    std::fs::write(&exclude, text).unwrap();
    assert!(!git_in(dir, &["status", "--porcelain", "-uall"]).contains("page.md"));
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    assert!(dir.join(PAGE).is_file());
}
