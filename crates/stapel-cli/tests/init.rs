//! STP-1: `stapel init`.

mod common;

use common::{bare_dir, git_repo, read, stapel};
use predicates::prelude::PredicateBooleanExt;
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
        .stdout(contains("created: .gitignore"))
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
        "/target\n/.stapel/index/\n/.claude/worktrees/\n/.stapel/tickets/*/page.md\n"
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
        .success()
        .stdout(contains("created: .gitignore\n"))
        .stdout(contains("created: .gitattributes\n"))
        .stdout(contains("appended: .gitattributes").not());
    let attrs = read(empty.path(), ".gitattributes");
    assert_eq!(
        attrs,
        ".stapel/process-ledger.jsonl merge=union\n.stapel/sessions.jsonl merge=union\n"
    );
}

// STP-27 AC-1, D1-3: edges of the union lines.
#[test]
fn union_lines_edges() {
    let ledger = ".stapel/process-ledger.jsonl merge=union";
    let sessions = ".stapel/sessions.jsonl merge=union";

    // One of the two lines present: only the other is appended.
    for (have, want) in [(ledger, sessions), (sessions, ledger)] {
        let repo = git_repo();
        let dir = repo.path();
        std::fs::write(dir.join(".gitattributes"), format!("{have}\n")).unwrap();
        stapel(dir)
            .args(["init", "--prefix", "ABC"])
            .assert()
            .success();
        assert_eq!(read(dir, ".gitattributes"), format!("{have}\n{want}\n"));
    }

    // A file without a final newline gets a newline, then the lines.
    let repo = git_repo();
    let dir = repo.path();
    std::fs::write(dir.join(".gitattributes"), "*.png binary").unwrap();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    assert_eq!(
        read(dir, ".gitattributes"),
        format!("*.png binary\n{ledger}\n{sessions}\n")
    );

    // A second run after a first run created both files changes nothing.
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success()
        .stdout(contains("created: .gitattributes\n"));
    let ignore = read(dir, ".gitignore");
    let attrs = read(dir, ".gitattributes");
    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("created: .gitattributes").not())
        .stdout(contains("appended: .gitattributes").not());
    assert_eq!(read(dir, ".gitignore"), ignore);
    assert_eq!(read(dir, ".gitattributes"), attrs);

    // A non-UTF-8 .gitattributes is refused and kept.
    let repo = git_repo();
    let dir = repo.path();
    std::fs::write(dir.join(".gitattributes"), b"\xff\xfe\n").unwrap();
    let out = stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        std::fs::read(dir.join(".gitattributes")).unwrap(),
        b"\xff\xfe\n"
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
    // STP-31 AC-10: the template is a junit one.
    assert!(text.contains("runner = \"junit\""), "{text}");
    assert!(text.contains("{file}"), "{text}");
    assert!(text.contains("# report"), "{text}");
    assert!(text.contains("# tests"), "{text}");
    let at = text.find("# [check]").unwrap();
    let uncommented: String = format!(
        "{}{}",
        &text[..at],
        text[at..]
            .lines()
            .map(|l| format!("{}\n", l.strip_prefix("# ").unwrap_or(l)))
            .collect::<String>()
    );
    let check = Config::parse(&uncommented)
        .unwrap_or_else(|e| panic!("{e}: {uncommented}"))
        .check
        .expect("the uncommented template has a [check]");
    assert_eq!(check.runner, "junit");
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

// STP-27 AC-25 (D17-7): an existing `stapel.toml` whose `[roles]` table lacks the row of an
// installed agent gets the row; every other line stays; a second run changes nothing.
#[test]
fn roles_row_added() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let row = "stapel-depends = \"depends\"";
    let starter = read(dir, ".stapel/stapel.toml");
    assert!(starter.lines().any(|l| l.trim() == row), "{starter}");
    let old: String = starter
        .lines()
        .filter(|l| l.trim() != row)
        .map(|l| format!("{l}\n"))
        .collect::<String>()
        .replace(
            "stapel-reviewer = \"reviewer\"\n",
            "stapel-reviewer = \"reviewer\"\nmy-agent = \"research\" # hand edit\n",
        )
        + "\n# hand edit at the end\n";
    std::fs::write(dir.join(".stapel/stapel.toml"), &old).unwrap();

    stapel(dir).arg("init").assert().success();

    let new = read(dir, ".stapel/stapel.toml");
    Config::parse(&new).unwrap();
    let lines: Vec<&str> = new.lines().collect();
    assert_eq!(
        lines.iter().filter(|l| l.trim() == row).count(),
        1,
        "{new}"
    );
    let at = lines.iter().position(|l| l.trim() == "[roles]").unwrap();
    let next_table = lines[at + 1..]
        .iter()
        .position(|l| l.starts_with('['))
        .map_or(lines.len(), |p| at + 1 + p);
    let row_at = lines.iter().position(|l| l.trim() == row).unwrap();
    assert!(at < row_at && row_at < next_table, "row outside [roles]: {new}");
    let without: String = lines
        .iter()
        .filter(|l| l.trim() != row)
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(without, old, "another line changed: {new}");

    let before = common::snapshot(dir);
    std::thread::sleep(std::time::Duration::from_millis(50));
    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("already set up"));
    assert_eq!(common::snapshot(dir), before);
}

// STP-32 AC-5: the page file is ignored in both modes, and in this repository.
#[test]
fn page_is_ignored_in_both_modes() {
    const LINE: &str = "/.stapel/tickets/*/page.md";
    for (args, file) in [
        (vec!["init", "--prefix", "ABC"], ".gitignore"),
        (vec!["init", "--local", "--prefix", "ABC"], ".git/info/exclude"),
    ] {
        let repo = git_repo();
        let dir = repo.path();
        stapel(dir).args(&args).assert().success();
        let lines = read(dir, file);
        assert!(lines.lines().any(|l| l == LINE), "{file}: {lines}");
        std::fs::create_dir_all(dir.join(".stapel/tickets/ABC-1")).unwrap();
        std::fs::write(dir.join(".stapel/tickets/ABC-1/page.md"), "# page\n").unwrap();
        let status = common::git_in(dir, &["status", "--porcelain", "-uall"]);
        assert!(!status.contains("page.md"), "{status}");
    }
    let own = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.gitignore")).unwrap();
    assert!(own.lines().any(|l| l == LINE), "{own}");
}
