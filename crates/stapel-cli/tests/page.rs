//! STP-32: `stapel page [KEY]` writes the page of a ticket as a file.

mod common;

use common::{
    git_in, read, repo_with_ticket, set_section, stapel, state_json, two_ticket_worktrees,
};
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
         - proof: new, 1 line\n- plan: new, 1 line\n\n## Criteria\n```diff\n The spec.\n```\n"
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

/// Inserts the template's `description` entry as the first `[[sections]]` entry.
fn add_description_section(dir: &Path) {
    let path = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let at = text.find("[[sections]]").unwrap();
    let entry = "[[sections]]\nid = \"description\"\ntitle = \"Description\"\nowner = \"product\"\n\n";
    std::fs::write(&path, format!("{}{entry}{}", &text[..at], &text[at..])).unwrap();
}

const DESCRIPTION: &str =
    "Source: the chat.\n\n### Frame\n\nframe one\n\nframe two\n\n### Decisions\n\n- d1\n- d2";
const SPEC: &str = "- term: old\n| AC-1 | first | `t::a` |\n| AC-2 | second | `t::b` |";

fn golden(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name),
    )
    .unwrap()
}

fn new_ticket_repo() -> tempfile::TempDir {
    let repo = repo_with_ticket();
    let dir = repo.path();
    ignore_in_gitignore(dir);
    add_description_section(dir);
    set_section(dir, "ABC-1", "Description", DESCRIPTION);
    set_section(dir, "ABC-1", "Spec", SPEC);
    repo
}

#[test]
fn layout_of_a_new_ticket() {
    let repo = new_ticket_repo();
    let dir = repo.path();
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    assert_eq!(read(dir, PAGE), golden("page_new.md"));
}

#[test]
fn marks_since_the_last_confirmation() {
    let repo = new_ticket_repo();
    let dir = repo.path();
    for id in ["description", "spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", "ABC-1", id]).assert().success();
    }
    set_section(
        dir,
        "ABC-1",
        "Description",
        "Source: the chat.\n\n### Frame\n\nframe one\n\nframe 2\n\n### Decisions\n\n- d1",
    );
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "- term: new\n| AC-1 | first | `t::a` |\n| AC-2 | second | `t::c` |\n\
         | AC-3 | third | `t::d` |",
    );
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    assert_eq!(read(dir, PAGE), golden("page_changed.md"));
}

#[test]
fn fence_longer_than_any_backtick_run() {
    let repo = new_ticket_repo();
    let dir = repo.path();
    set_section(
        dir,
        "ABC-1",
        "Description",
        "Source: the chat.\n\n### Frame\n\nred \x1b[31mtext\tend\n\n### Decisions\n\n- d1",
    );
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| AC-1 | has ``` three | `t::a` |\n| AC-2 | has ````` five | `t::b` |",
    );
    let path = dir.join(".stapel/tickets/ABC-1/state.json");
    let mut state = state_json(dir, "ABC-1");
    state["title"] = serde_json::json!("Ti\ttle");
    std::fs::write(path, state.to_string()).unwrap();
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    let page = read(dir, PAGE);
    assert!(page.starts_with("# ABC-1 — Ti tle\n"), "{page}");
    assert!(
        page.contains(
            "## Criteria\n``````diff\n | AC-1 | has ``` three | `t::a` |\n \
             | AC-2 | has ````` five | `t::b` |\n``````\n"
        ),
        "{page}"
    );
    assert!(page.contains("## Frame\n```diff\n Source: the chat.\n"), "{page}");
    assert!(page.contains("\n red  [31mtext end\n"), "{page}");
    assert!(!page.contains('\x1b') && !page.contains('\t'), "{page}");
}

const MISSING: &str = "the page of ABC-1 is missing; run stapel page ABC-1 and read \
     .stapel/tickets/ABC-1/page.md";
const STALE: &str = "the page of ABC-1 is stale; run stapel page ABC-1 and read \
     .stapel/tickets/ABC-1/page.md again";

#[test]
fn guard_refuses_without_the_page() {
    let repo = new_ticket_repo();
    let dir = repo.path();
    assert!(!dir.join(PAGE).exists());
    common::hook_bash(dir, "stapel ok ABC-1", "auto")
        .code(2)
        .stderr(predicates::str::contains(MISSING));
    // No grant was written.
    let written = std::fs::read_dir(common::grants_dir(dir))
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(written, 0);

    // With the page the guard asks.
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    common::hook_bash(dir, "stapel ok ABC-1", "auto").success();
}

#[test]
fn guard_refuses_a_stale_page() {
    let repo = new_ticket_repo();
    let dir = repo.path();
    let denied = || {
        common::hook_bash(dir, "stapel ok ABC-1", "auto")
            .code(2)
            .stderr(predicates::str::contains(STALE));
    };

    // A Spec edit after the page.
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    common::hook_bash(dir, "stapel ok ABC-1", "auto").success();
    set_section(dir, "ABC-1", "Spec", "| AC-1 | edited | `t::a` |");
    denied();

    // A hand edit of the page: one byte added.
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    common::hook_bash(dir, "stapel ok ABC-1", "auto").success();
    let mut bytes = std::fs::read(dir.join(PAGE)).unwrap();
    bytes.push(b'\n');
    std::fs::write(dir.join(PAGE), bytes).unwrap();
    denied();

    // Changed line endings, and an empty file.
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    let text = read(dir, PAGE).replace('\n', "\r\n");
    std::fs::write(dir.join(PAGE), text).unwrap();
    denied();
    std::fs::write(dir.join(PAGE), "").unwrap();
    denied();

    // A section confirmed in the terminal after the page.
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    stapel(dir).args(["ok", "ABC-1", "spec"]).assert().success();
    denied();
}

#[test]
fn grant_binds_the_page() {
    let repo = new_ticket_repo();
    let dir = repo.path();
    let state = dir.join(".stapel/tickets/ABC-1/state.json");
    let decisions = dir.join(".stapel/tickets/ABC-1/decisions.jsonl");
    let files = || {
        (
            std::fs::read(&state).unwrap_or_default(),
            std::fs::read(&decisions).unwrap_or_default(),
        )
    };
    let grant_fails = |replaced: &str| {
        let before = files();
        stapel(dir)
            .args(["ok", "ABC-1", "--grant", &common::token_of(replaced)])
            .env("CLAUDECODE", "1")
            .assert()
            .code(1)
            .stderr(predicates::str::contains("the grant does not match"));
        assert_eq!(files(), before);
    };

    // The page edited by hand after the dialog.
    let (_, replaced) = common::ask(dir, "stapel ok ABC-1");
    let mut bytes = std::fs::read(dir.join(PAGE)).unwrap();
    bytes.extend_from_slice(b"edited\n");
    std::fs::write(dir.join(PAGE), bytes).unwrap();
    grant_fails(&replaced);

    // The page removed after the dialog.
    let (_, replaced) = common::ask(dir, "stapel ok ABC-1");
    std::fs::remove_file(dir.join(PAGE)).unwrap();
    grant_fails(&replaced);

    // A Decisions edit and the page written again after the dialog.
    let (_, replaced) = common::ask(dir, "stapel ok ABC-1");
    set_section(
        dir,
        "ABC-1",
        "Description",
        "Source: the chat.\n\n### Frame\n\nframe one\n\nframe two\n\n### Decisions\n\n- d1\n- d3",
    );
    stapel(dir).args(["page", "ABC-1"]).assert().success();
    grant_fails(&replaced);
}
