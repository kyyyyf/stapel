//! STP-28: `stapel lint [KEY]`: a test a ticket names does not exist, a step test no ticket names.

mod common;

use common::{git_config, git_in, junit_repo, stapel, stapel_repo};
use std::path::Path;

fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn commit(dir: &Path, message: &str) {
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "--allow-empty", "-m", message]);
}

/// A ticket folder by hand: the text and a `state.json` without a base, closed or open.
fn ticket(dir: &Path, key: &str, text: &str, closed: bool) {
    write(dir, &format!(".stapel/tickets/{key}/ticket.md"), text);
    let mut state = serde_json::json!({
        "schema_version": 1,
        "key": key,
        "title": "t",
        "confirmations": [],
    });
    if closed {
        state["closed"] = serde_json::json!({
            "by": "test-user",
            "at": "2026-01-01T00:00:00Z",
            "reason": "done",
        });
    }
    write(
        dir,
        &format!(".stapel/tickets/{key}/state.json"),
        &state.to_string(),
    );
}

/// The exit code, the lines of stdout and the text of stderr of `stapel lint <args>`.
fn lint(dir: &Path, args: &[&str]) -> (i32, Vec<String>, String) {
    let out = stapel(dir).arg("lint").args(args).output().unwrap();
    let lines = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(String::from)
        .collect();
    (
        out.status.code().unwrap_or(-1),
        lines,
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// A stapel repository with a git identity and the test file `crates/stapel-core/tests/x.rs`
/// holding the tests `names`, not yet committed.
fn cargo_repo(names: &[&str]) -> tempfile::TempDir {
    let repo = stapel_repo();
    let dir = repo.path();
    git_config(dir, "user.email", "test-user.invalid");
    write(dir, "crates/stapel-core/tests/x.rs", &x_source(names));
    repo
}

fn x_source(names: &[&str]) -> String {
    names
        .iter()
        .map(|n| format!("#[test]\nfn {n}() {{}}\n\n"))
        .collect()
}

// AC-29: a Checked test that does not exist at HEAD is reported.
#[test]
fn names_a_missing_test() {
    let repo = cargo_repo(&["here"]);
    let dir = repo.path();
    ticket(
        dir,
        "ABC-1",
        "# ABC-1\n\n## Proof\n\n| AC-1 | one | `x::here`, `x::gone` |\n",
        true,
    );
    ticket(
        dir,
        "ABC-2",
        "# ABC-2\n\n## Spec\n\n| AC-1 | rows of an open ticket are plans | `x::row_gone` |\n\n\
         ## Plan\n\n1. First (tests `x::here`, `x::gone_one`): `a.rs`.\n\
         2. Second (test `x::gone_two`): `b.rs`.\n",
        false,
    );
    commit(dir, "base");
    commit(dir, "ABC-2 step 1 RED: first");
    commit(dir, "ABC-2 step 1 GREEN: first");
    commit(dir, "ABC-2 step 2 RED: second");

    let (code, lines, err) = lint(dir, &["ABC-1"]);
    assert_eq!(code, 1, "{lines:?} {err}");
    assert_eq!(lines, ["ABC-1 names x::gone, which does not exist"]);

    // Step 1 has a GREEN, step 2 has none, and the rows of an open ticket are plans.
    let (code, lines, err) = lint(dir, &["ABC-2"]);
    assert_eq!(code, 1, "{lines:?} {err}");
    assert_eq!(lines, ["ABC-2 names x::gone_one, which does not exist"]);
}

// AC-30, AC-31: a step test of the ticket's RED commits that exists and that no ticket names in a
// table row is reported; the three cargo forms of a name count.
#[test]
fn step_test_named_by_no_ticket() {
    let repo = cargo_repo(&[]);
    let dir = repo.path();
    ticket(dir, "ABC-1", "# ABC-1\n", false);
    ticket(dir, "ABC-2", "# ABC-2\n", false);
    commit(dir, "base");
    write(
        dir,
        "crates/stapel-core/tests/x.rs",
        &x_source(&["by_stem", "by_suffix", "by_dir", "by_plan", "nobody"]),
    );
    commit(dir, "ABC-1 step 1 RED: five tests");
    // Named in a table row by the ticket (`x::t`, `stapel-core/x::t`), by another ticket
    // (`core::x::t`); a Plan step is no table row.
    ticket(
        dir,
        "ABC-1",
        "# ABC-1\n\n## Spec\n\n| AC-1 | a | `x::by_stem`, `stapel-core/x::by_dir` |\n\n\
         ## Plan\n\n1. Five (tests `x::by_plan`): `x.rs`.\n",
        false,
    );
    ticket(
        dir,
        "ABC-2",
        "# ABC-2\n\n| AC-1 | b | `core::x::by_suffix` |\n",
        false,
    );

    let (code, lines, err) = lint(dir, &["ABC-1"]);
    assert_eq!(code, 1, "{lines:?} {err}");
    assert_eq!(
        lines,
        [
            "stapel-core/x::by_plan is a step test of ABC-1 and no ticket names it",
            "stapel-core/x::nobody is a step test of ABC-1 and no ticket names it",
        ]
    );

    // Naming the rest in a row of any ticket makes it clean.
    ticket(
        dir,
        "ABC-2",
        "# ABC-2\n\n| AC-1 | b | `core::x::by_suffix`, `x::by_plan`, `x::nobody` |\n",
        false,
    );
    let (code, lines, err) = lint(dir, &["ABC-1"]);
    assert_eq!(
        (code, lines),
        (0, vec!["lint: ABC-1 clean".to_string()]),
        "{err}"
    );
}

// AC-31: the junit name is `<test file>::<name>`.
#[test]
fn lint_under_junit() {
    let repo = junit_repo();
    let dir = repo.dir();
    ticket(dir, "ABC-1", "# ABC-1\n", false);
    write(
        dir,
        "src/a.test.ts",
        "describe(\"a\", () => {});\nit(\"a b\", () => {});\n",
    );
    commit(dir, "ABC-1 step 1 RED: a b");

    let (code, lines, err) = lint(dir, &["ABC-1"]);
    assert_eq!(code, 1, "{lines:?} {err}");
    assert_eq!(
        lines,
        ["src/a.test.ts::a b is a step test of ABC-1 and no ticket names it"]
    );

    ticket(
        dir,
        "ABC-1",
        "# ABC-1\n\n| AC-1 | x | `src/a.test.ts::a b` |\n",
        false,
    );
    let (code, lines, err) = lint(dir, &["ABC-1"]);
    assert_eq!(
        (code, lines),
        (0, vec!["lint: ABC-1 clean".to_string()]),
        "{err}"
    );

    // A closed ticket's Named tests are all checked, by file and by name.
    ticket(
        dir,
        "ABC-1",
        "# ABC-1\n\n| AC-1 | x | `src/a.test.ts::a b`, `src/missing.test.ts::x`, \
         `src/a.test.ts::gone` |\n",
        true,
    );
    let (code, lines, err) = lint(dir, &["ABC-1"]);
    assert_eq!(code, 1, "{lines:?} {err}");
    assert_eq!(
        lines,
        [
            "ABC-1 names src/a.test.ts::gone, which does not exist",
            "ABC-1 names src/missing.test.ts::x, which does not exist",
        ]
    );
}

// AC-28, AC-32: the ticket, the order of the report and the exit codes.
#[test]
fn lint_exit_codes_and_order() {
    // No ticket at all: a refusal.
    let empty = stapel_repo();
    let (code, lines, err) = lint(empty.path(), &[]);
    assert_eq!(code, 2, "{lines:?} {err}");
    assert!(lines.is_empty(), "{lines:?}");
    assert!(err.contains("no open ticket"), "{err}");

    let repo = cargo_repo(&["named_ok"]);
    let dir = repo.path();
    ticket(
        dir,
        "ABC-3",
        "# ABC-3\n\n| AC-1 | c | `x::named_ok` |\n",
        true,
    );
    ticket(dir, "ABC-2", "# ABC-2\n", false);
    ticket(
        dir,
        "ABC-4",
        "# ABC-4\n\n## Plan\n\n1. Planned (test `x::planned_gone`): `x.rs`.\n",
        false,
    );
    commit(dir, "base");
    ticket(
        dir,
        "ABC-1",
        "# ABC-1\n\n| AC-1 | a | `x::zzz_gone`, `x::aaa_gone`, `x::named_ok` |\n",
        true,
    );
    write(
        dir,
        "crates/stapel-core/tests/x.rs",
        &x_source(&["named_ok", "b_unnamed", "a_unnamed"]),
    );
    commit(dir, "ABC-1 step 1 RED: tests");
    commit(dir, "ABC-4 step 1 RED: planned");
    commit(dir, "ABC-4 step 1 GREEN: planned");
    let main = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"]);

    // Two missing, then two unnamed, each sorted by name.
    let (code, lines, err) = lint(dir, &["ABC-1"]);
    assert_eq!(code, 1, "{lines:?} {err}");
    assert_eq!(
        lines,
        [
            "ABC-1 names x::aaa_gone, which does not exist",
            "ABC-1 names x::zzz_gone, which does not exist",
            "stapel-core/x::a_unnamed is a step test of ABC-1 and no ticket names it",
            "stapel-core/x::b_unnamed is a step test of ABC-1 and no ticket names it",
        ]
    );

    // A test named in a Plan step only, once the step has a GREEN.
    let (code, lines, err) = lint(dir, &["ABC-4"]);
    assert_eq!(code, 1, "{lines:?} {err}");
    assert_eq!(lines, ["ABC-4 names x::planned_gone, which does not exist"]);

    // Clean: exit 0.
    let (code, lines, err) = lint(dir, &["ABC-3"]);
    assert_eq!(
        (code, lines),
        (0, vec!["lint: ABC-3 clean".to_string()]),
        "{err}"
    );

    // Unknown key, and two open tickets on the base branch without a key: refusals.
    let (code, lines, err) = lint(dir, &["ABC-9"]);
    assert_eq!(code, 2, "{lines:?} {err}");
    assert!(lines.is_empty() && err.contains("ABC-9"), "{lines:?} {err}");
    let (code, lines, err) = lint(dir, &[]);
    assert_eq!(code, 2, "{lines:?} {err}");
    assert!(lines.is_empty() && err.contains("ABC-2"), "{lines:?} {err}");

    // On the branch ABC-2 the ticket is the branch ticket.
    git_in(dir, &["checkout", "-q", "-b", "ABC-2"]);
    let (code, lines, err) = lint(dir, &[]);
    assert_eq!(
        (code, lines),
        (0, vec!["lint: ABC-2 clean".to_string()]),
        "{err}"
    );
    let (code, lines, err) = lint(dir, &["ABC-9"]);
    assert_eq!(code, 2, "{lines:?} {err}");
    git_in(dir, &["checkout", "-q", main.trim()]);
}
