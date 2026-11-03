//! STP-1: `stapel init`.

mod common;

use common::{bare_dir, git_repo, read, stapel};
use predicates::str::contains;
use stapel_core::config::Config;

fn key_of(dir: &std::path::Path) -> String {
    Config::parse(&read(dir, ".stapel/stapel.toml"))
        .unwrap()
        .tickets
        .key
}

// STP-25 AC-1: the template has the `description` section first, and `build.requires` names it.
#[test]
fn template_has_the_description_section() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let config = Config::parse(&read(dir, ".stapel/stapel.toml")).unwrap();
    let first = &config.sections[0];
    assert_eq!(first.id, "description");
    assert_eq!(first.title, "Description");
    assert_eq!(first.owner, "product");
    assert!(first.depends_on.is_empty());
    for s in &config.sections {
        assert!(
            !s.depends_on.iter().any(|d| d == "description"),
            "{} depends on description",
            s.id
        );
    }
    assert_eq!(
        config.build.requires,
        ["description", "spec", "design", "proof"]
    );
}

// STP-25 AC-1 (E-1, E-2): the template as shipped gives one `## Description` and a dialog line for it.
#[test]
fn new_ticket_has_one_description() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    stapel(dir).args(["new", "First"]).assert().success();
    let ticket = read(dir, ".stapel/tickets/ABC-1/ticket.md");
    assert_eq!(
        ticket.lines().filter(|l| *l == "## Description").count(),
        1,
        "{ticket}"
    );
    let (reason, _) = common::ask(dir, "stapel ok ABC-1");
    assert!(reason.contains("\ndescription: new, "), "{reason}");
}

// AC-2
#[test]
fn creates_layout_in_empty_repo() {
    let repo = git_repo();
    let dir = repo.path();

    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success()
        .stdout(contains("created: .stapel/stapel.toml"))
        .stdout(contains("created: .stapel/allowlist.toml"))
        .stdout(contains("created: .stapel/tickets/.gitkeep"))
        .stdout(contains("appended: .gitignore"))
        .stdout(contains("created: .claude/settings.json"));

    assert!(dir.join(".stapel/stapel.toml").is_file());
    assert!(dir.join(".stapel/allowlist.toml").is_file());
    assert!(dir.join(".stapel/tickets").is_dir());
    assert!(
        read(dir, ".gitignore")
            .lines()
            .any(|l| l == "/.stapel/index/")
    );
}

// AC-2: the repository root is found from a subdirectory.
#[test]
fn creates_layout_at_repo_root_from_subdir() {
    let repo = git_repo();
    let sub = repo.path().join("src/deep");
    std::fs::create_dir_all(&sub).unwrap();

    stapel(&sub)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();

    assert!(repo.path().join(".stapel/stapel.toml").is_file());
    assert!(!sub.join(".stapel").exists());
}

// AC-2: an existing .gitignore keeps its lines.
#[test]
fn appends_to_existing_gitignore() {
    let repo = git_repo();
    std::fs::write(repo.path().join(".gitignore"), "/target").unwrap();

    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();

    assert_eq!(
        read(repo.path(), ".gitignore"),
        "/target\n/.stapel/index/\n/.claude/worktrees/\n"
    );
}

// STP-27 AC-1: worktrees are ignored and both journals merge as union; a second run adds none.
#[test]
fn worktrees_ignored_and_journals_union() {
    let repo = git_repo();
    let dir = repo.path();
    std::fs::write(dir.join(".gitattributes"), "*.png binary").unwrap();

    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();

    let count = |text: &str, line: &str| text.lines().filter(|l| *l == line).count();
    let ignore = read(dir, ".gitignore");
    let attrs = read(dir, ".gitattributes");
    assert_eq!(count(&ignore, "/.claude/worktrees/"), 1, "{ignore}");
    assert!(attrs.starts_with("*.png binary\n"), "{attrs}");
    for line in [
        ".stapel/process-ledger.jsonl merge=union",
        ".stapel/sessions.jsonl merge=union",
    ] {
        assert_eq!(count(&attrs, line), 1, "{attrs}");
    }

    stapel(dir).arg("init").assert().success();
    assert_eq!(read(dir, ".gitignore"), ignore);
    assert_eq!(read(dir, ".gitattributes"), attrs);

    let empty = git_repo();
    stapel(empty.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let attrs = read(empty.path(), ".gitattributes");
    assert_eq!(
        attrs,
        ".stapel/process-ledger.jsonl merge=union\n.stapel/sessions.jsonl merge=union\n"
    );
}

// AC-11
#[test]
fn refuses_outside_git() {
    let dir = bare_dir();

    stapel(dir.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .code(1)
        .stderr(contains("not a git repository"));

    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

// AC-12
#[test]
fn prefix_from_flag() {
    let repo = git_repo();
    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    assert_eq!(key_of(repo.path()), "ABC-{n}");
}

// AC-12
#[test]
fn prefix_from_prompt() {
    let repo = git_repo();
    stapel(repo.path())
        .arg("init")
        .env("STAPEL_ASSUME_TTY", "1")
        .write_stdin("QA\n")
        .assert()
        .success()
        .stdout(contains("Ticket key prefix"));
    assert_eq!(key_of(repo.path()), "QA-{n}");
}

// AC-12
#[test]
fn rejects_bad_prefix() {
    for bad in ["A", "ABCDEFGHI", "ab", "A1", "АБ", ""] {
        let repo = git_repo();
        stapel(repo.path())
            .args(["init", "--prefix", bad])
            .assert()
            .code(1)
            .stderr(contains("prefix"));
        assert!(!repo.path().join(".stapel").exists(), "created for {bad:?}");
    }
}

// AC-12
#[test]
fn refuses_without_prefix_noninteractive() {
    let repo = git_repo();
    stapel(repo.path())
        .arg("init")
        .assert()
        .code(1)
        .stderr(contains("--prefix"));
    assert!(!repo.path().join(".stapel").exists());
}

// AC-4, R-3; AC-12: the second run does not ask for a prefix.
#[test]
fn second_run_changes_nothing() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let before = common::snapshot(dir);
    std::thread::sleep(std::time::Duration::from_millis(50));

    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("already set up"));

    assert_eq!(common::snapshot(dir), before);
}

// AC-5
#[test]
fn keeps_user_edited_config() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();

    let edited = read(dir, ".stapel/stapel.toml").replace("claude-haiku-4-5", "my-cheap-model")
        + "\n# hand edit\n";
    std::fs::write(dir.join(".stapel/stapel.toml"), &edited).unwrap();
    std::fs::remove_file(dir.join(".stapel/allowlist.toml")).unwrap();

    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("created: .stapel/allowlist.toml"));

    assert_eq!(read(dir, ".stapel/stapel.toml"), edited);
    assert!(dir.join(".stapel/allowlist.toml").is_file());
}

// AC-5: a prefix given again does not rewrite an existing config.
#[test]
fn second_run_with_other_prefix_keeps_key() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();

    stapel(dir)
        .args(["init", "--prefix", "XYZ"])
        .assert()
        .success()
        .stdout(contains("ABC-{n}"));

    assert_eq!(key_of(dir), "ABC-{n}");
}

// AC-5 (D-5): a hand-edited config that no longer parses is reported and left alone.
#[test]
fn refuses_unparsable_config() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    std::fs::write(dir.join(".stapel/stapel.toml"), "[tickets]\n").unwrap();
    std::fs::remove_file(dir.join(".stapel/allowlist.toml")).unwrap();

    stapel(dir)
        .arg("init")
        .assert()
        .code(1)
        .stderr(contains(".stapel/stapel.toml"));

    assert_eq!(read(dir, ".stapel/stapel.toml"), "[tickets]\n");
    assert!(!dir.join(".stapel/allowlist.toml").exists());
}

// AC-5 (F-6): a .gitignore that is not UTF-8 is never rewritten, and nothing else is created.
#[test]
fn keeps_non_utf8_gitignore() {
    let repo = git_repo();
    let dir = repo.path();
    let original = b"\xff\xfe# keep\n".to_vec();
    std::fs::write(dir.join(".gitignore"), &original).unwrap();

    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .code(1)
        .stderr(contains(".gitignore"));

    assert_eq!(std::fs::read(dir.join(".gitignore")).unwrap(), original);
    assert!(!dir.join(".stapel").exists());
}

// D2-4: a stapel.toml that is not UTF-8 is refused and left alone.
#[test]
fn refuses_non_utf8_config() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    std::fs::write(dir.join(".stapel/stapel.toml"), b"\xff\xfe").unwrap();

    stapel(dir)
        .arg("init")
        .assert()
        .code(1)
        .stderr(contains(".stapel/stapel.toml"));
    assert_eq!(
        std::fs::read(dir.join(".stapel/stapel.toml")).unwrap(),
        b"\xff\xfe"
    );
}

// STP-4 AC-7: a cargo repository gets a `[check]` section.
#[test]
fn writes_check_for_cargo_repositories() {
    let repo = git_repo();
    let dir = repo.path();
    std::fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let config = Config::parse(&read(dir, ".stapel/stapel.toml")).unwrap();
    assert_eq!(config.check.unwrap().runner, "cargo");
}

// STP-4 AC-7: another repository gets a commented example and no gate.
#[test]
fn writes_commented_check_otherwise() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let text = read(dir, ".stapel/stapel.toml");
    assert!(Config::parse(&text).unwrap().check.is_none());
    assert!(text.lines().any(|l| l.starts_with("# [check]")), "{text}");
    assert!(!text.lines().any(|l| l.trim() == "[check]"), "{text}");
}

// STP-26 AC-12: a new `stapel.toml` holds the `[roles]` table with the four default rows.
#[test]
fn template_has_the_roles_table() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let text = read(dir, ".stapel/stapel.toml");
    Config::parse(&text).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = lines
        .iter()
        .position(|l| *l == "[roles]")
        .unwrap_or_else(|| panic!("no [roles] table: {text}"));
    for row in [
        "stapel-builder = \"builder\"",
        "stapel-drift = \"drift\"",
        "stapel-reviewer = \"reviewer\"",
        "stapel-spec-reviewer = \"reviewer\"",
    ] {
        assert!(lines[at..].contains(&row), "{row} missing: {text}");
    }
}
