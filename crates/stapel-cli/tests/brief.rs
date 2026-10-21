//! STP-21 AC-1 to AC-4: `stapel brief KEY STEP`, the header, the Frame and the Step.

mod common;

use common::{hook_bash, repo_with_ticket, set_section, snapshot, stapel};
use std::path::Path;

const GOLDEN: &str = "tests/golden/brief_text.txt";

const FRAME: &str = "### Frame\n\n\
    **Problem.** The thing.\n\n\
    **Prior art.** Old tools.\nSecond line.\n\n\
    **Size.** About ten lines.\nSecond line.\n\n\
    **Promise.** One item.\n\n\
    ### Decisions\n\n\
    - not in the brief.";

const PLAN: &str = "Route: full, auto.\n\n\
    1. One. Names AC-1.\n\
    2. Two. Names AC-2 and\n   AC-3 to AC-4. Reads R1-1.\n\
    3. Three.";

const SPEC: &str = "Terms:\n\n\
    - **Widget**: a part that holds a gear.\n\
    - **Gear**: a wheel.\n\
    - **Unused**: named by no row.\n\n\
    | № | Criterion | Test |\n|---|---|---|\n\
    | AC-1 | One. | `brief::t1` |\n\
    | AC-2 | Two uses a widget. | `brief::t2`, `brief::t3` |\n\
    | AC-3 | Three. | `brief::t3` |\n\
    | AC-4 | Four. | `brief::t4` |";

const TEST_PLAN: &str = "Light tests.\n\n\
    | Category | Cases | Tests |\n|---|---|---|\n\
    | Main | one | `brief::t1` |\n\
    | Two | a case | `brief::t2` |\n\
    | Coverage (step 2) | more | `brief::zz` |\n\n\
    ### Inputs\n\n\
    | Input | Test |\n|---|---|\n\
    | a | `brief::t3` |\n\
    | b | `brief::t1` |\n\n\
    ### External states\n\n\
    | System | Handling | Test |\n|---|---|---|\n\
    | os | x | `brief::t4` |\n\
    | git | y | — |";

/// Adds a `## Test plan` section with `body` before the Design of ticket ABC-1.
fn set_test_plan(dir: &Path, body: &str) {
    let path = dir.join(".stapel/tickets/ABC-1/ticket.md");
    let text = std::fs::read_to_string(&path).unwrap();
    let text = match text.find("## Test plan\n") {
        Some(_) => panic!("a Test plan exists"),
        None => text.replacen(
            "## Design\n",
            &format!("## Test plan\n\n{body}\n\n## Design\n"),
            1,
        ),
    };
    std::fs::write(&path, text).unwrap();
}

fn golden() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN)).unwrap()
}

/// A ticket ABC-1 with a Frame (with Prior art and Size paragraphs) and a Plan of three steps.
fn fixture() -> tempfile::TempDir {
    let repo = repo_with_ticket();
    set_section(repo.path(), "ABC-1", "Description", FRAME);
    set_section(repo.path(), "ABC-1", "Plan", PLAN);
    set_section(repo.path(), "ABC-1", "Spec", SPEC);
    set_test_plan(repo.path(), TEST_PLAN);
    std::fs::write(
        repo.path().join(".stapel/tickets/ABC-1/findings.jsonl"),
        "{\"id\":\"R1-1\",\"severity\":\"HIGH\",\"claim\":\"A claim.\",\"fate\":\"fixed\"}\n",
    )
    .unwrap();
    repo
}

/// The standard output of `stapel brief ABC-1 <step>` that exits 0 and has the `# Brief:` line.
fn brief(dir: &Path, step: &str) -> String {
    let out = stapel(dir)
        .args(["brief", "ABC-1", step])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.starts_with(&format!("# Brief: ABC-1 step {step}\n")),
        "no brief line in: {text}"
    );
    text
}

// AC-1, AC-2, AC-3, AC-4
#[test]
fn prints_the_brief() {
    let repo = fixture();
    let text = brief(repo.path(), "2");
    assert_eq!(text, golden());
}

// AC-5, AC-18: named criteria, ranges, order, whole words, and the `tests:` line.
#[test]
fn criteria_and_ranges() {
    /// The brief of step 1 of a Plan `plan` over a Spec of five rows and `AC-10`.
    fn with_plan(dir: &Path, plan: &str) -> String {
        let spec = "| № | Criterion | Test |\n|---|---|---|\n\
            | AC-1 | One. | `brief::a` |\n\
            | AC-2 | Two. | `brief::b`, `brief::c` |\n\
            | AC-3 | Three. | `brief::c` |\n\
            | AC-4 | Four. | `brief::d` |\n\
            | AC-5 | Five. | `brief::e` |\n\
            | AC-10 | Ten. | `brief::j` |";
        set_section(dir, "ABC-1", "Spec", spec);
        set_section(dir, "ABC-1", "Plan", plan);
        brief(dir, "1")
    }
    let repo = repo_with_ticket();
    let dir = repo.path();
    let head = "| № | Criterion | Test |\n|---|---|---|\n";

    // A name and a range.
    let text = with_plan(dir, "1. One. AC-1, AC-3 to AC-5.");
    assert!(
        text.contains(&format!(
            "\n## Criteria\n{head}| AC-1 | One. | `brief::a` |\n\
             | AC-3 | Three. | `brief::c` |\n| AC-4 | Four. | `brief::d` |\n\
             | AC-5 | Five. | `brief::e` |\n"
        )),
        "{text}"
    );
    assert!(!text.contains("| AC-2 |"), "{text}");
    assert!(
        text.contains("base: none\ntests: brief::a, brief::c, brief::d, brief::e\n"),
        "{text}"
    );

    // A name twice: one row. A step test of two rows: listed once.
    let text = with_plan(dir, "1. One. AC-3 and AC-2 and AC-3 again.");
    assert_eq!(text.matches("| AC-3 |").count(), 1, "{text}");
    assert!(
        text.contains("| AC-2 | Two. | `brief::b`, `brief::c` |\n| AC-3 | Three. | `brief::c` |\n"),
        "{text}"
    );
    assert!(text.contains("\ntests: brief::b, brief::c\n"), "{text}");

    // Spec order, not the order of the step.
    let text = with_plan(dir, "1. One. AC-5 then AC-1.");
    let one = text.find("| AC-1 |").expect("no AC-1");
    let five = text.find("| AC-5 |").expect("no AC-5");
    assert!(one < five, "{text}");
    assert!(text.contains("\ntests: brief::a, brief::e\n"), "{text}");

    // `AC-10` is not `AC-1`.
    let text = with_plan(dir, "1. One. AC-10.");
    assert!(text.contains("| AC-10 |"), "{text}");
    assert!(!text.contains("| AC-1 |"), "{text}");
    assert!(text.contains("\ntests: brief::j\n"), "{text}");

    // A range broken after `to` with an indented next line.
    let text = with_plan(dir, "1. One. AC-2 to\n   AC-3.");
    assert!(
        text.contains("| AC-2 | Two. | `brief::b`, `brief::c` |\n| AC-3 | Three. | `brief::c` |\n"),
        "{text}"
    );
    assert!(!text.contains("| AC-4 |"), "{text}");

    // The part comes after the Step.
    let step = text.find("\n## Step\n").expect("no Step");
    let criteria = text.find("\n## Criteria\n").expect("no Criteria");
    assert!(step < criteria, "{text}");
}

// AC-12: a named criterion without a row, and a range with n > m.
#[test]
fn unknown_criterion() {
    fn refuses(dir: &Path, plan: &str, message: &str) {
        set_section(dir, "ABC-1", "Plan", plan);
        let out = stapel(dir)
            .args(["brief", "ABC-1", "1"])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "", "{plan}");
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            format!("{message}\n"),
            "{plan}"
        );
    }
    let repo = fixture();
    let dir = repo.path();
    refuses(
        dir,
        "1. One. AC-1 and AC-9.",
        "stapel: step 1 names AC-9, which the Spec does not have",
    );
    // The first id with no row, in the order of the step.
    refuses(
        dir,
        "1. One. AC-8 and AC-9.",
        "stapel: step 1 names AC-8, which the Spec does not have",
    );
    refuses(
        dir,
        "1. One. AC-4 to AC-3.",
        "stapel: step 1 names AC-4 to AC-3, which is not a range",
    );
}

// AC-5: the step tests come from the last cell of a row only.
#[test]
fn tests_from_the_last_cell() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let spec = "| № | Criterion | Test |\n|---|---|---|\n\
        | AC-1 | Calls `x::y` first. | `brief::a` |";
    set_section(dir, "ABC-1", "Spec", spec);
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");
    let text = brief(dir, "1");
    assert!(text.contains("\ntests: brief::a\n"), "{text}");
    assert!(
        text.contains("| AC-1 | Calls `x::y` first. | `brief::a` |\n"),
        "{text}"
    );
    assert!(!text.contains("tests: x::y"), "{text}");
    assert!(!text.contains("brief::a, x::y"), "{text}");
}

// AC-12: the refusal names the first unknown id in the order of the step.
#[test]
fn unknown_ids_in_step_order() {
    fn refuses(dir: &Path, plan: &str, message: &str) {
        set_section(dir, "ABC-1", "Plan", plan);
        let out = stapel(dir)
            .args(["brief", "ABC-1", "1"])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "", "{plan}");
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            format!("{message}\n"),
            "{plan}"
        );
    }
    let repo = repo_with_ticket();
    let dir = repo.path();

    // Neither row exists: the first in the step, not the smallest.
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | One. | `brief::a` |",
    );
    refuses(
        dir,
        "1. One. AC-9 and AC-8.",
        "stapel: step 1 names AC-9, which the Spec does not have",
    );

    // A row missing inside a range.
    let mut spec = String::from("| № | Criterion | Test |\n|---|---|---|\n");
    for n in (1..=9).filter(|n| *n != 6) {
        spec.push_str(&format!("| AC-{n} | Row. | `brief::t{n}` |\n"));
    }
    set_section(dir, "ABC-1", "Spec", spec.trim_end());
    refuses(
        dir,
        "1. One. AC-1 to AC-9.",
        "stapel: step 1 names AC-6, which the Spec does not have",
    );

    // `AC-10` is not `AC-1`: the row of AC-1 does not stand for it.
    refuses(
        dir,
        "1. One. AC-10.",
        "stapel: step 1 names AC-10, which the Spec does not have",
    );
}

// AC-5: the header row and separator come from the table that holds the criterion rows.
#[test]
fn header_of_the_criteria_table() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let spec = "| a | b |\n|---|---|\n| 1 | 2 |\n\n\
        | № | Criterion | Test |\n|---|---|---|\n\
        | AC-1 | One. | `brief::a` |";
    set_section(dir, "ABC-1", "Spec", spec);
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");
    let text = brief(dir, "1");
    assert!(
        text.contains(
            "\n## Criteria\n| № | Criterion | Test |\n|---|---|---|\n| AC-1 | One. | `brief::a` |\n"
        ),
        "{text}"
    );
    assert!(!text.contains("| a | b |"), "{text}");
}

// AC-12: with an unknown id and a bad range, the refusal is for the first in the step.
#[test]
fn refusals_in_step_order() {
    fn refuses(dir: &Path, plan: &str, message: &str) {
        set_section(dir, "ABC-1", "Plan", plan);
        let out = stapel(dir)
            .args(["brief", "ABC-1", "1"])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "", "{plan}");
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            format!("{message}\n"),
            "{plan}"
        );
    }
    let repo = fixture();
    let dir = repo.path();
    refuses(
        dir,
        "1. One. AC-9 and AC-4 to AC-3.",
        "stapel: step 1 names AC-9, which the Spec does not have",
    );
    refuses(
        dir,
        "1. One. AC-4 to AC-3 and AC-9.",
        "stapel: step 1 names AC-4 to AC-3, which is not a range",
    );
}

// AC-1: the command writes no file.
#[test]
fn writes_nothing() {
    let repo = fixture();
    let dir = repo.path();
    let before = snapshot(dir);
    brief(dir, "2");
    assert_eq!(snapshot(dir), before);
}

// AC-1: a closed ticket gives its brief too.
#[test]
fn closed_ticket() {
    let repo = fixture();
    let dir = repo.path();
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .assert()
        .success();
    assert_eq!(brief(dir, "2"), golden());
}

// AC-1: the guard lets the command run, and it runs with CLAUDECODE set.
#[test]
fn runs_in_an_agent_shell() {
    let repo = fixture();
    let dir = repo.path();
    hook_bash(dir, "stapel brief ABC-1 2", "auto").success();
    let out = stapel(dir)
        .env("CLAUDECODE", "1")
        .args(["brief", "ABC-1", "2"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("# Brief: ABC-1 step 2\n"), "{text}");
    assert_eq!(text, golden());
}

// AC-3: the Frame without its Prior art and Size paragraphs.
#[test]
fn frame_without_prior_art_and_size() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", PLAN);
    set_section(dir, "ABC-1", "Spec", SPEC);
    let frame = "### Frame\n\n\
        **Prior art** without the dot.\n\n\
        **Prior art.** Dropped.\nSecond line.\n\n\
        Text that mentions **Size.** later.\n\n\
        **Size.** Dropped.\nSecond line.\n\n\
        **Last.** Kept.\n\n\
        ### Other\n\nAfter the Frame.";
    set_section(dir, "ABC-1", "Description", frame);
    let text = brief(dir, "1");
    let expected = "## Frame\n\
        **Prior art** without the dot.\n\n\
        Text that mentions **Size.** later.\n\n\
        **Last.** Kept.\n\n\
        ## Step\n";
    assert!(text.contains(expected), "{text}");
    // The Spec of this test holds AC-1, so the Criteria part follows the Step.
    assert!(
        text.contains("\n## Criteria\n| № | Criterion | Test |\n"),
        "{text}"
    );
    assert!(!text.contains("Dropped"), "{text}");
    assert!(!text.contains("After the Frame"), "{text}");

    // No `### Frame`: no part, the Step stays.
    set_section(dir, "ABC-1", "Description", "No frame here.");
    let text = brief(dir, "1");
    assert!(!text.contains("## Frame"), "{text}");
    assert!(text.contains("## Step\n1. One. Names AC-1.\n"), "{text}");
}

// AC-1: a lettered step, and a KEY in another case than the folder key.
#[test]
fn lettered_step_and_folder_key() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(
        dir,
        "ABC-1",
        "Plan",
        "1. One.\n2c. Two c. Names AC-1.\n3. Three.",
    );
    set_section(dir, "ABC-1", "Spec", SPEC);
    set_section(
        dir,
        "ABC-1",
        "Description",
        "### Frame\n\n**Problem.** The thing.",
    );
    let out = stapel(dir)
        .args(["brief", "abc-1", "2c"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.starts_with(
            "# Brief: ABC-1 step 2c\nticket: .stapel/tickets/ABC-1/ticket.md\nbase: none\n"
        ),
        "{text}"
    );
    assert!(text.contains("## Step\n2c. Two c. Names AC-1.\n"), "{text}");
    assert!(!text.contains("abc-1"), "{text}");
    assert!(text.contains("\ntests: brief::t1\n"), "{text}");
}

// AC-2: the base of state.json is printed as written.
#[test]
fn base_of_state_json() {
    let repo = fixture();
    let dir = repo.path();
    let path = dir.join(".stapel/tickets/ABC-1/state.json");
    let mut state = common::state_json(dir, "ABC-1");
    let sha = "0123456789abcdef0123456789abcdef01234567";
    state["base"] = serde_json::Value::String(sha.to_string());
    std::fs::write(&path, serde_json::to_string_pretty(&state).unwrap()).unwrap();
    let text = brief(dir, "2");
    assert!(
        text.starts_with(&format!(
            "# Brief: ABC-1 step 2\nticket: .stapel/tickets/ABC-1/ticket.md\nbase: {sha}\n"
        )),
        "{text}"
    );
}

// AC-4: the step item ends at a blank line.
#[test]
fn step_item_ends_at_a_blank_line() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(
        dir,
        "ABC-1",
        "Plan",
        "1. One. Names AC-1\n   and goes on.\n\nA paragraph after the blank line.\n\n2. Two.",
    );
    set_section(dir, "ABC-1", "Spec", SPEC);
    let text = brief(dir, "1");
    assert!(
        text.contains("## Step\n1. One. Names AC-1\n   and goes on.\n\n## Criteria\n"),
        "{text}"
    );
    assert!(!text.contains("paragraph"), "{text}");
}

// AC-3: a Frame that is empty after the drops, and a Frame with no heading after it.
#[test]
fn frame_edges() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "Terms:\n\n- **Widget**: a part.\n\n\
         | № | Criterion | Test |\n|---|---|---|\n| AC-1 | A widget. | `brief::a` |",
    );
    let only_dropped = "### Frame\n\n\
        **Prior art.** Old tools.\n\n\
        **Size.** About ten lines.\n\n\
        ### Decisions\n\nNot in the brief.";
    set_section(dir, "ABC-1", "Description", only_dropped);
    let text = brief(dir, "1");
    assert!(!text.contains("## Frame"), "{text}");
    assert!(
        text.starts_with("# Brief: ABC-1 step 1\nticket: .stapel/tickets/ABC-1/ticket.md\nbase: none\ntests: brief::a\n\n## Step\n1. One. AC-1.\n\n## Criteria\n"),
        "{text}"
    );
    assert!(
        text.ends_with("\n## Terms\n- **Widget**: a part.\n"),
        "{text}"
    );

    // No heading after the Frame: it runs to the end of the Description.
    set_section(
        dir,
        "ABC-1",
        "Description",
        "### Frame\n\n**Problem.** The thing.\n\n**Size.** Dropped.\n\n**Promise.** Last one.",
    );
    let text = brief(dir, "1");
    assert!(
        text.contains("## Frame\n**Problem.** The thing.\n\n**Promise.** Last one.\n\n## Step\n"),
        "{text}"
    );
    assert!(!text.contains("Dropped"), "{text}");
}

// AC-11: a step that is not in the Plan, or is in it twice.
#[test]
fn step_not_in_the_plan() {
    fn refuses(dir: &Path, step: &str, message: &str) {
        let out = stapel(dir)
            .args(["brief", "ABC-1", step])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "");
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            format!("{message}\n")
        );
    }
    let no_step = "stapel: the Plan of ABC-1 has no step 3";

    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One.\n2. Two.");
    refuses(dir, "3", no_step);

    // A Plan that is `TODO`.
    set_section(dir, "ABC-1", "Plan", "TODO");
    refuses(dir, "3", no_step);

    // Two lines `2.`.
    set_section(dir, "ABC-1", "Plan", "1. One.\n2. Two.\n2. Two again.");
    refuses(dir, "2", "stapel: the Plan of ABC-1 names step 2 twice");

    // A `stapel.toml` without the `plan` section.
    let config = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    let start = text
        .find("[[sections]]\nid = \"plan\"")
        .expect("no plan section");
    let end = text[start + 1..]
        .find("[[sections]]")
        .map(|i| start + 1 + i)
        .unwrap_or(text.len());
    let without = format!("{}{}", &text[..start], &text[end..]);
    assert!(!without.contains("id = \"plan\""));
    std::fs::write(&config, without).unwrap();
    refuses(dir, "3", no_step);
}

// AC-10: a legacy ticket, and unreadable `state.json` or `ticket.md`, exit 1.
#[test]
fn refuses_an_unreadable_ticket() {
    fn refuses(dir: &Path, key: &str) {
        let out = stapel(dir)
            .args(["brief", key, "1"])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "", "{key}");
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(err.starts_with("stapel: "), "{key}: {err}");
        assert_eq!(err.lines().count(), 1, "{key}: {err}");
    }
    let repo = fixture();
    let dir = repo.path();
    let tickets = dir.join(".stapel/tickets");

    // `state.json` that is not JSON.
    std::fs::create_dir_all(tickets.join("ABC-8")).unwrap();
    std::fs::write(tickets.join("ABC-8/state.json"), "not json").unwrap();
    refuses(dir, "ABC-8");

    // A legacy ticket.
    std::fs::create_dir_all(tickets.join("ABC-9")).unwrap();
    std::fs::write(
        tickets.join("ABC-9/state.json"),
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    )
    .unwrap();
    refuses(dir, "ABC-9");

    // `findings.jsonl` that is a directory, and one that is not UTF-8.
    let findings = tickets.join("ABC-1/findings.jsonl");
    std::fs::remove_file(&findings).unwrap();
    std::fs::create_dir(&findings).unwrap();
    refuses(dir, "ABC-1");
    std::fs::remove_dir(&findings).unwrap();
    std::fs::write(&findings, [0x7b, 0xff, 0xfe, 0x0a]).unwrap();
    refuses(dir, "ABC-1");
    std::fs::remove_file(&findings).unwrap();

    // `ticket.md` that is not UTF-8.
    std::fs::write(
        tickets.join("ABC-1/ticket.md"),
        [0x23, 0x20, 0xff, 0xfe, 0x0a],
    )
    .unwrap();
    refuses(dir, "ABC-1");
}

// AC-10: a wrong `STEP`, an unknown or ambiguous `KEY` and a missing argument exit 2.
#[test]
fn refuses_a_wrong_call() {
    fn refuses(dir: &Path, args: &[&str], message: &str) {
        let out = stapel(dir)
            .arg("brief")
            .args(args)
            .assert()
            .code(2)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "", "{args:?}");
        let err = String::from_utf8(out.stderr).unwrap();
        if message.is_empty() {
            assert!(!err.is_empty(), "{args:?}");
        } else {
            assert_eq!(err, format!("{message}\n"), "{args:?}");
        }
    }
    let repo = fixture();
    let dir = repo.path();

    for step in ["x", "c2", "2cc", "2C", "step 2"] {
        refuses(
            dir,
            &["ABC-1", step],
            &format!(
                "stapel: STEP must be digits followed by at most one lowercase letter, \
                 e.g. 2 or 2c: {step}"
            ),
        );
    }

    refuses(
        dir,
        &["NOPE-1", "1"],
        "stapel: no ticket NOPE-1 in .stapel/tickets",
    );

    // Two folders that differ only in case.
    let tickets = dir.join(".stapel/tickets");
    std::fs::create_dir_all(tickets.join("ABC-7")).unwrap();
    std::fs::create_dir_all(tickets.join("abc-7")).unwrap();
    let out = stapel(dir)
        .args(["brief", "ABC-7", "1"])
        .assert()
        .code(2)
        .get_output()
        .clone();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "");
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.starts_with("stapel: ABC-7 matches several folders that differ only in case: "),
        "{err}"
    );

    // A missing argument is a clap usage error.
    refuses(dir, &["ABC-1"], "");
    refuses(dir, &[], "");
}

// AC-6: the terms the criteria use, and the terms those terms use.
#[test]
fn terms_used_and_their_terms() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");
    /// The part `## Terms` of step 1 over a Spec of `list` (the Terms list), a row of `row` and
    /// `after` (lines after the table); `None` when the brief has no such part.
    fn terms(dir: &Path, list: &str, row: &str, after: &str) -> Option<String> {
        let spec = format!(
            "Terms:\n\n{list}\n\n| № | Criterion | Test |\n|---|---|---|\n\
             | AC-1 | {row} | `brief::a` |\n\n{after}"
        );
        set_section(dir, "ABC-1", "Spec", spec.trim_end());
        let text = brief(dir, "1");
        let (_, rest) = text.split_once("\n## Terms\n")?;
        Some(rest.split("\n## ").next().unwrap().to_string())
    }

    // Upper case in the row.
    assert_eq!(
        terms(dir, "- **Alpha**: a.\n- **Beta**: b.", "uses ALPHA", "").as_deref(),
        Some("- **Alpha**: a.\n")
    );

    // A plural with `s`, one with `es`; a term that no row uses stays out.
    assert_eq!(
        terms(
            dir,
            "- **Delta item**: d.\n- **Box**: b.\n- **Other**: o.",
            "uses delta items and boxes",
            ""
        )
        .as_deref(),
        Some("- **Delta item**: d.\n- **Box**: b.\n")
    );

    // A term used only by another term, two levels deep; Spec order, not the order of use.
    assert_eq!(
        terms(
            dir,
            "- **Deep**: d.\n- **Leaf**: needs a deep one.\n- **Top**: uses a leaf.",
            "uses top",
            ""
        )
        .as_deref(),
        Some("- **Deep**: d.\n- **Leaf**: needs a deep one.\n- **Top**: uses a leaf.\n")
    );

    // A name that starts a longer word is not used.
    assert_eq!(terms(dir, "- **Step**: s.", "Stepping on", ""), None);

    // Two terms on one line: the text of the first ends at the second name; each term of the
    // printed line counts as printed, so Gamma, which Beta uses, is added.
    assert_eq!(
        terms(
            dir,
            "- **Alpha**: a. **Beta**: uses gamma.\n- **Gamma**: g.",
            "uses alpha",
            ""
        )
        .as_deref(),
        Some("- **Alpha**: a. **Beta**: uses gamma.\n- **Gamma**: g.\n")
    );
    // Both on one line used: the line is printed once.
    assert_eq!(
        terms(
            dir,
            "- **Alpha**: a. **Beta**: b.\n- **Gamma**: g.",
            "uses alpha and beta",
            ""
        )
        .as_deref(),
        Some("- **Alpha**: a. **Beta**: b.\n")
    );

    // A `- **Promised:**` line after the table is no term.
    assert_eq!(
        terms(
            dir,
            "- **Alpha**: a.",
            "uses promised",
            "- **Promised:** promised things."
        ),
        None
    );
}

// AC-6: terms are matched against the criterion rows only.
#[test]
fn terms_from_the_rows_only() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");
    // Only the header row `| № | Criterion | Test |` holds the word `criterion`.
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "Terms:\n\n- **Criterion**: c.\n\n| № | Criterion | Test |\n|---|---|---|\n\
         | AC-1 | does a thing | `brief::a` |",
    );
    let text = brief(dir, "1");
    assert!(!text.contains("Criterion**"), "{text}");
    assert!(!text.contains("\n## Terms\n"), "{text}");
}

// AC-6: each term on a printed term line counts as printed.
#[test]
fn terms_on_a_shared_line() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "Terms:\n\n- **Alpha**: a. **Beta**: uses gamma.\n- **Gamma**: g.\n\n\
         | № | Criterion | Test |\n|---|---|---|\n| AC-1 | uses alpha | `brief::a` |",
    );
    let text = brief(dir, "1");
    let (_, rest) = text.split_once("\n## Terms\n").expect(&text);
    assert_eq!(
        rest.split("\n## ").next().unwrap(),
        "- **Alpha**: a. **Beta**: uses gamma.\n- **Gamma**: g.\n"
    );
    assert_eq!(text.matches("**Beta**").count(), 1, "{text}");
}

// AC-14: a step that names no criterion and no finding.
#[test]
fn step_names_nothing() {
    fn refuses(dir: &Path, plan: &str) {
        set_section(dir, "ABC-1", "Plan", plan);
        let out = stapel(dir)
            .args(["brief", "ABC-1", "1"])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "", "{plan}");
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            "stapel: step 1 names no criterion and no finding\n",
            "{plan}"
        );
    }
    let repo = fixture();
    let dir = repo.path();

    // No id, and ids that are not criteria and have no recorded prefix (no `findings.jsonl`).
    refuses(dir, "1. One.");
    refuses(dir, "1. One. Reads D1-1 and UTF-8.");

    // A recorded prefix that is not the prefix of the word; lines that are no records.
    let file = dir.join(".stapel/tickets/ABC-1/findings.jsonl");
    std::fs::write(
        &file,
        "{\"id\":\"D2-1\"}\nnot json\n{\"x\":\"D1-1\"}\n{\"id\":5}\n",
    )
    .unwrap();
    refuses(dir, "1. One. Reads D1-1 and UTF-8.");

    // A named finding is enough: the step is not refused.
    set_section(dir, "ABC-1", "Plan", "1. One. Reads D2-1.");
    let text = brief(dir, "1");
    assert!(text.contains("\n## Step\n1. One. Reads D2-1.\n"), "{text}");
}

/// Writes `findings.jsonl` of ABC-1 and the Plan `1. <item>`, and gives the brief of step 1.
fn findings_brief(dir: &Path, records: &str, item: &str) -> String {
    std::fs::write(dir.join(".stapel/tickets/ABC-1/findings.jsonl"), records).unwrap();
    set_section(dir, "ABC-1", "Plan", &format!("1. {item}"));
    brief(dir, "1")
}

// AC-9: the last record of an id, `none` for a missing field, a range, words that are no findings.
#[test]
fn findings_last_record() {
    let repo = fixture();
    let dir = repo.path();
    let records = "{\"id\":\"D1-2\",\"severity\":\"LOW\",\"claim\":\"old.\",\"fate\":\"open\"}\n\
        not json\n\
        {\"id\":\"D1-5\",\"severity\":\"HIGH\",\"claim\":\"five.\",\"fate\":\"fixed\"}\n\
        {\"x\":\"D1-6\"}\n\
        {\"id\":\"D1-6\",\"severity\":\"MEDIUM\",\"claim\":\"six.\"}\n\
        {\"id\":\"D1-2\",\"severity\":\"HIGH\",\"claim\":\"new.\",\"fate\":\"fixed\"}\n\
        {\"id\":\"D1-7\",\"claim\":\"seven.\",\"fate\":\"open\"}\n";

    // Step order, each finding once, the last record, `none` for a missing field, a range.
    let text = findings_brief(
        dir,
        records,
        "One. Reads D1-2, then D1-5 to D1-6, D1-2 again, UTF-8 and STP-22, then D1-7.",
    );
    assert!(
        text.ends_with(
            "\n## Findings\n\
             - D1-2 (HIGH): new. Fate: fixed\n\
             - D1-5 (HIGH): five. Fate: fixed\n\
             - D1-6 (MEDIUM): six. Fate: none\n\
             - D1-7 (none): seven. Fate: open\n"
        ),
        "{text}"
    );
    assert_eq!(text.matches("D1-2 (").count(), 1, "{text}");
    assert!(!text.contains("UTF-8 ("), "{text}");
    assert!(!text.contains("STP-22 ("), "{text}");

    // The part follows `## External states`, the last part.
    let text = findings_brief(dir, records, "One. AC-4. Reads D1-5.");
    let at = |h: &str| text.find(h).unwrap_or_else(|| panic!("no {h} in {text}"));
    assert!(
        at("\n## External states\n") < at("\n## Findings\n"),
        "{text}"
    );
}

// AC-13: the first named finding without a well-formed record refuses.
#[test]
fn unknown_finding() {
    fn refuses(dir: &Path, records: &str, item: &str, id: &str) {
        std::fs::write(dir.join(".stapel/tickets/ABC-1/findings.jsonl"), records).unwrap();
        set_section(dir, "ABC-1", "Plan", &format!("1. {item}"));
        let out = stapel(dir)
            .args(["brief", "ABC-1", "1"])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "", "{item}");
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            format!("stapel: step 1 names {id}, which findings.jsonl does not have\n"),
            "{item}"
        );
    }
    let repo = fixture();
    let dir = repo.path();

    // A recorded prefix without the record; the first missing id in step order.
    let records = "{\"id\":\"D1-1\"}\n{\"id\":\"D1-2\"}\n";
    refuses(dir, records, "One. Reads D1-9.", "D1-9");
    refuses(dir, records, "One. Reads D1-2, D1-8 and D1-9.", "D1-8");
    refuses(dir, records, "One. Reads D1-1 to D1-3.", "D1-3");

    // The only line of the id is unreadable.
    let records = "{\"id\":\"D1-1\"}\n{\"id\":\"D1-9\"\n";
    refuses(dir, records, "One. Reads D1-9.", "D1-9");
}

// AC-18, AC-9: a step that names one finding only leaves out every other part.
#[test]
fn empty_parts_are_left_out() {
    let repo = fixture();
    let dir = repo.path();
    let text = findings_brief(
        dir,
        "{\"id\":\"D1-1\",\"severity\":\"LOW\",\"claim\":\"c.\",\"fate\":\"open\"}\n",
        "One. Reads D1-1.",
    );
    for part in ["Frame", "Step", "Findings"] {
        assert!(text.contains(&format!("\n## {part}\n")), "{text}");
    }
    assert!(!text.contains("\ntests:"), "{text}");
    for part in ["Criteria", "Terms", "Cases", "Inputs", "External states"] {
        assert!(!text.contains(&format!("## {part}")), "{text}");
    }
}

// AC-6: the Terms list ends at the criteria table.
#[test]
fn terms_list_ends_at_the_table() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");
    /// The brief of step 1 over a Spec of `list`, a row of `row` and `after`.
    fn brief_of(dir: &Path, list: &str, row: &str, after: &str) -> String {
        let spec = format!(
            "Terms:\n\n{list}\n\n| № | Criterion | Test |\n|---|---|---|\n\
             | AC-1 | {row} | `brief::a` |\n\n{after}"
        );
        set_section(dir, "ABC-1", "Spec", spec.trim_end());
        brief(dir, "1")
    }

    // A `- **Late**: x` line after the table holds no term, though a row uses `late`.
    let text = brief_of(dir, "- **Early**: e.", "uses late", "- **Late**: x");
    assert!(!text.contains("Late"), "{text}");
    assert!(!text.contains("\n## Terms\n"), "{text}");

    // A positive control: a term `Step` used as `step` is printed.
    let text = brief_of(dir, "- **Step**: s.", "does a step", "");
    assert!(text.contains("\n## Terms\n- **Step**: s.\n"), "{text}");
}

/// The brief of step `step` of a ticket whose Spec has one criterion with the test `brief::a`
/// and whose Test plan is `test_plan`.
fn plan_brief(test_plan: &str, step: &str) -> String {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | One. | `brief::a` |",
    );
    set_section(dir, "ABC-1", "Plan", &format!("{step}. One. AC-1."));
    set_test_plan(dir, test_plan);
    brief(dir, step)
}

// AC-7: rows of the cases table that name a step test or the step.
#[test]
fn cases_of_the_step() {
    let head = "| Category | Cases | Tests |\n|---|---|---|\n";
    let plan = format!(
        "{head}\
         | Both | x | `brief::a`, `brief::other` |\n\
         | Other | x | `brief::other` |\n\
         | Coverage (step 1) | x | — |\n\
         | Coverage (step 1c) | x | — |\n\
         | Coverage (step 11) | x | — |\n\
         | Longer | x | `brief::a_b` |\n\n\
         ### Inputs\n\n\
         | Input | Test |\n|---|---|\n| z | `brief::a` |"
    );
    let text = plan_brief(&plan, "1");
    let (_, rest) = text.split_once("\n## Cases\n").expect(&text);
    assert_eq!(
        rest.split("\n## ").next().unwrap(),
        format!(
            "{head}| Both | x | `brief::a`, `brief::other` |\n\
             | Coverage (step 1) | x | — |\n"
        )
    );
    // After the Criteria; the Inputs row is not in the cases table.
    let criteria = text.find("\n## Criteria\n").expect("no Criteria");
    assert!(criteria < text.find("\n## Cases\n").unwrap(), "{text}");
    assert!(!text.contains("| Input | Test |\n|---|---|\n| z | `brief::a` |\n\n## Cases"));

    // A lettered step takes its own label only.
    let text = plan_brief(&plan, "1c");
    let (_, rest) = text.split_once("\n## Cases\n").expect(&text);
    assert!(rest.contains("| Coverage (step 1c) | x | — |\n"), "{text}");
    assert!(!rest.contains("| Coverage (step 1) |"), "{text}");

    // Without a table: no part.
    let text = plan_brief("No tables here, step 1.", "1");
    assert!(!text.contains("## Cases"), "{text}");
}

// AC-8: rows of the Inputs and External states tables whose last cell names a step test.
#[test]
fn rows_of_the_step_tests() {
    let plan = "| Category | Cases | Tests |\n|---|---|---|\n| Main | x | `brief::a` |\n\n\
        ### Inputs\n\n\
        | Input | Note | Test |\n|---|---|---|\n\
        | one | x | `brief::a`, `brief::other` |\n\
        | two | x | `brief::other` |\n\
        | three | `brief::a` | `brief::other` |\n\
        | four | x | — |\n\
        | five | x | `brief::a_b` |\n\n\
        ### External states\n\n\
        | System | Test |\n|---|---|\n\
        | os | `brief::a` |\n\
        | git | — |\n\
        | cargo | `brief::other` |";
    let text = plan_brief(plan, "1");
    let part = |title: &str| {
        let (_, rest) = text.split_once(&format!("\n## {title}\n")).expect(&text);
        rest.split("\n## ").next().unwrap().to_string()
    };
    assert_eq!(
        part("Inputs"),
        "| Input | Note | Test |\n|---|---|---|\n| one | x | `brief::a`, `brief::other` |\n"
    );
    assert_eq!(
        part("External states"),
        "| System | Test |\n|---|---|\n| os | `brief::a` |\n"
    );
    // The order: Cases, Inputs, External states.
    let at = |title: &str| text.find(&format!("\n## {title}\n")).unwrap();
    assert!(at("Cases") < at("Inputs") && at("Inputs") < at("External states"));

    // A Test plan without the tables: no part.
    let text = plan_brief("Only words.", "1");
    assert!(!text.contains("## Inputs"), "{text}");
    assert!(!text.contains("## External states"), "{text}");

    // Tables with no row of the step: no part.
    let plan = "### Inputs\n\n| Input | Test |\n|---|---|\n| z | `brief::other` |";
    let text = plan_brief(plan, "1");
    assert!(!text.contains("## Inputs"), "{text}");
}

// AC-15: more than 1600 estimated tokens (bytes / 4, rounded up) refuses and names the part sizes.
#[test]
fn too_large() {
    /// The parts of a brief in print order: the lower-case name and the bytes of each. The header
    /// ends at its last line; each other part starts at its blank line.
    fn parts(text: &str) -> Vec<(String, usize)> {
        let mut starts = vec![0];
        starts.extend(text.match_indices("\n\n## ").map(|(i, _)| i + 1));
        starts.push(text.len());
        starts
            .windows(2)
            .enumerate()
            .map(|(k, w)| {
                let name = if k == 0 {
                    "header".to_string()
                } else {
                    text[w[0]..].lines().nth(1).unwrap()[3..].to_lowercase()
                };
                (name, w[1] - w[0])
            })
            .collect()
    }
    fn tokens(bytes: usize) -> usize {
        bytes.div_ceil(4)
    }
    fn run(dir: &Path) -> std::process::Output {
        stapel(dir)
            .args(["brief", "ABC-1", "2"])
            .assert()
            .get_output()
            .clone()
    }
    fn with_frame(dir: &Path, frame: &str) {
        set_section(
            dir,
            "ABC-1",
            "Description",
            &format!("### Frame\n\n{frame}"),
        );
    }

    let repo = fixture();
    let dir = repo.path();

    // A small run gives the parts and the total of the rest of the brief.
    with_frame(dir, "x");
    let small = String::from_utf8(run(dir).stdout).unwrap();
    let small_parts = parts(&small);
    assert_eq!(small_parts[1].0, "frame");

    // A Frame of 6 000 bytes: refused, with the size of every printed part.
    with_frame(dir, &"a".repeat(6000));
    let out = run(dir);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "");
    let mut big_parts = small_parts.clone();
    big_parts[1].1 = "\n## Frame\n".len() + 6000 + 1;
    let total: usize = big_parts.iter().map(|(_, n)| n).sum();
    let list: Vec<String> = big_parts
        .iter()
        .map(|(name, n)| format!("{name} {}", tokens(*n)))
        .collect();
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        format!(
            "stapel: the brief of step 2 has {} tokens, more than 1600: {}\n",
            tokens(total),
            list.join(", ")
        )
    );
    assert!(tokens(total) > 1600);

    // Exactly 1600 estimated tokens (6400 bytes) passes; 1601 (6401 bytes) refuses.
    let pad = 1 + 6400 - small.len();
    with_frame(dir, &"b".repeat(pad));
    let out = run(dir);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!((text.len(), tokens(text.len())), (6400, 1600));
    with_frame(dir, &"b".repeat(pad + 1));
    let out = run(dir);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.starts_with("stapel: the brief of step 2 has 1601 tokens, more than 1600: header "),
        "{stderr}"
    );
}

// AC-6 (D4D-2): the text of a term ends at the next name, also when that name is on the next line.
#[test]
fn term_text_ends_at_the_next_name() {
    /// The Terms part of step 1 for a row that `row` describes, over a list where Alpha's text
    /// runs onto the line of Beta, and only Beta's text names Delta.
    fn terms_for(dir: &Path, row: &str) -> String {
        let spec = format!(
            "Terms:\n\n- **Alpha**: a, with\n  more words. **Beta**: b names delta.\n\
             - **Delta**: d.\n\n| № | Criterion | Test |\n|---|---|---|\n\
             | AC-1 | {row} | `brief::a` |"
        );
        set_section(dir, "ABC-1", "Spec", &spec);
        let text = brief(dir, "1");
        match text.split_once("\n## Terms\n") {
            Some((_, rest)) => rest.split("\n## ").next().unwrap().to_string(),
            None => String::new(),
        }
    }
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");

    // Alpha's text runs onto the line of Beta, so the two share that line (AC-6, D4D-1): either
    // one used prints both, and Beta's text brings Delta.
    let all = "- **Alpha**: a, with\n  more words. **Beta**: b names delta.\n- **Delta**: d.\n";
    assert_eq!(terms_for(dir, "uses alpha"), all);
    assert_eq!(terms_for(dir, "uses beta"), all);

    // Two terms on one line: the line is printed once and Delta follows from the second text.
    let spec = "Terms:\n\n- **Alpha**: a. **Beta**: b names delta.\n- **Delta**: d.\n\n\
        | № | Criterion | Test |\n|---|---|---|\n| AC-1 | uses alpha | `brief::a` |";
    set_section(dir, "ABC-1", "Spec", spec);
    let text = brief(dir, "1");
    assert!(
        text.contains("\n## Terms\n- **Alpha**: a. **Beta**: b names delta.\n- **Delta**: d.\n"),
        "{text}"
    );
}

// AC-7 (D5-2): a step without criteria takes the cases rows of its label.
#[test]
fn cases_of_a_step_without_criteria() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let head = "| Category | Cases | Tests |\n|---|---|---|\n";
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/findings.jsonl"),
        "{\"id\":\"D1-1\",\"severity\":\"LOW\",\"claim\":\"c.\",\"fate\":\"open\"}\n",
    )
    .unwrap();
    set_section(dir, "ABC-1", "Plan", "1. One. Reads D1-1.");
    set_test_plan(
        dir,
        &format!(
            "{head}| Main | x | `brief::a` |\n| Coverage (step 1) | y | — |\n\
             | Coverage (step 2) | z | — |"
        ),
    );
    let text = brief(dir, "1");
    assert!(!text.contains("\ntests:"), "{text}");
    assert!(!text.contains("## Criteria"), "{text}");
    let (_, rest) = text.split_once("\n## Cases\n").expect(&text);
    assert_eq!(
        rest.split("\n## ").next().unwrap(),
        format!("{head}| Coverage (step 1) | y | — |\n")
    );
}

// AC-9, AC-14 (D6-1): without `findings.jsonl` there is no finding prefix.
#[test]
fn no_findings_file() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file = dir.join(".stapel/tickets/ABC-1/findings.jsonl");
    assert!(!file.exists());
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | One. | `brief::a` |",
    );

    // A step with a criterion prints, and `D1-1` is no finding.
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1, reads D1-1.");
    let text = brief(dir, "1");
    assert!(text.contains("\n## Criteria\n"), "{text}");
    assert!(!text.contains("## Findings"), "{text}");

    // A step that names `D1-1` only refuses as a step that names nothing.
    set_section(dir, "ABC-1", "Plan", "1. One. Reads D1-1.");
    let out = stapel(dir)
        .args(["brief", "ABC-1", "1"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "");
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "stapel: step 1 names no criterion and no finding\n"
    );
    assert!(!file.exists());
}

// AC-9 (D6-2): a finding prefix matches as a whole; `AC` is never a finding prefix.
#[test]
fn finding_prefixes_exact() {
    fn refuses_as_nothing(dir: &Path, records: &str, item: &str) {
        std::fs::write(dir.join(".stapel/tickets/ABC-1/findings.jsonl"), records).unwrap();
        set_section(dir, "ABC-1", "Plan", &format!("1. {item}"));
        let out = stapel(dir)
            .args(["brief", "ABC-1", "1"])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "", "{item}");
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            "stapel: step 1 names no criterion and no finding\n",
            "{item}"
        );
    }
    let repo = fixture();
    let dir = repo.path();
    let record = |id: &str| {
        format!("{{\"id\":\"{id}\",\"severity\":\"HIGH\",\"claim\":\"c.\",\"fate\":\"open\"}}\n")
    };

    // Only `R1-1` is recorded: `SR1-1` and `R-1` are no findings.
    refuses_as_nothing(dir, &record("R1-1"), "One. Reads SR1-1 and R-1.");
    let text = findings_brief(dir, &record("R1-1"), "One. Reads SR1-1, R-1 and R1-1.");
    assert!(
        text.ends_with("\n## Findings\n- R1-1 (HIGH): c. Fate: open\n"),
        "{text}"
    );
    assert_eq!(text.matches("(HIGH)").count(), 1, "{text}");

    // A word in brackets is named.
    let text = findings_brief(dir, &record("D1-2"), "One. See (D1-2, and more.");
    assert!(
        text.ends_with("\n## Findings\n- D1-2 (HIGH): c. Fate: open\n"),
        "{text}"
    );

    // A record `AC-1` does not make `AC-1` a finding: the Spec row is printed, no Findings part.
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | One. | `brief::a` |",
    );
    let text = findings_brief(dir, &record("AC-1"), "One. AC-1.");
    assert!(text.contains("\n## Criteria\n"), "{text}");
    assert!(!text.contains("## Findings"), "{text}");
}

// AC-9 (D6-3): a field that is not a string prints `none`; the last well-formed record counts.
#[test]
fn finding_fields_not_strings() {
    let repo = fixture();
    let dir = repo.path();

    // Numbers, null, arrays and objects are no strings.
    let text = findings_brief(
        dir,
        "{\"id\":\"D1-1\",\"severity\":3,\"claim\":\"c.\",\"fate\":\"open\"}\n\
         {\"id\":\"D1-2\",\"severity\":\"LOW\",\"claim\":null,\"fate\":[\"x\"]}\n\
         {\"id\":\"D1-3\",\"severity\":{\"a\":1},\"claim\":true,\"fate\":7}\n",
        "One. Reads D1-1 to D1-3.",
    );
    assert!(
        text.ends_with(
            "\n## Findings\n\
             - D1-1 (none): c. Fate: open\n\
             - D1-2 (LOW): none Fate: none\n\
             - D1-3 (none): none Fate: none\n"
        ),
        "{text}"
    );

    // An unreadable line after the last well-formed record of an id does not replace it.
    let text = findings_brief(
        dir,
        "{\"id\":\"D1-2\",\"severity\":\"LOW\",\"claim\":\"old.\",\"fate\":\"open\"}\n\
         {\"id\":\"D1-2\",\"severity\":\"HIGH\",\"claim\":\"new.\",\"fate\":\"fixed\"}\n\
         {\"id\":\"D1-2\",\"severity\":\"MEDIUM\",\"claim\":\"cut.\"\n",
        "One. Reads D1-2.",
    );
    assert!(
        text.ends_with("\n## Findings\n- D1-2 (HIGH): new. Fate: fixed\n"),
        "{text}"
    );
}

// AC-15 (D7-1): the size counts bytes, not characters.
#[test]
fn size_counts_bytes() {
    fn tokens(bytes: usize) -> usize {
        bytes.div_ceil(4)
    }
    fn run(dir: &Path) -> std::process::Output {
        stapel(dir)
            .args(["brief", "ABC-1", "2"])
            .assert()
            .get_output()
            .clone()
    }
    fn with_frame(dir: &Path, frame: &str) {
        set_section(
            dir,
            "ABC-1",
            "Description",
            &format!("### Frame\n\n{frame}"),
        );
    }
    let repo = fixture();
    let dir = repo.path();
    with_frame(dir, "x");
    let small = String::from_utf8(run(dir).stdout).unwrap();

    // 2 000 `é` and padding: exactly 6 400 bytes, 1600 tokens, though far fewer characters.
    let pad = 6400 + 1 - small.len() - 4000;
    let frame = format!("{}{}", "é".repeat(2000), "a".repeat(pad));
    with_frame(dir, &frame);
    let out = run(dir);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!((text.len(), tokens(text.len())), (6400, 1600));
    assert!(text.chars().count() < 6000);

    // One more `é` is two bytes: 6 402 bytes, 1601 tokens, refused with the size of the Frame.
    with_frame(dir, &format!("é{frame}"));
    let out = run(dir);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.starts_with("stapel: the brief of step 2 has 1601 tokens, more than 1600: header "),
        "{stderr}"
    );
    let frame_bytes = "\n## Frame\n".len() + frame.len() + 2 + 1;
    assert!(
        stderr.contains(&format!(", frame {},", tokens(frame_bytes))),
        "{stderr}"
    );
}

// AC-1, AC-2, AC-18 (DR-3): a ticket without a Frame, a Terms list or Test plan tables.
#[test]
fn missing_parts_of_the_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | does a step | `brief::a` |",
    );
    let check = |text: &str| {
        assert!(
            text.starts_with(
                "# Brief: ABC-1 step 1\nticket: .stapel/tickets/ABC-1/ticket.md\nbase: none\n\
                 tests: brief::a\n"
            ),
            "{text}"
        );
        assert!(text.contains("\n## Step\n1. One. AC-1.\n"), "{text}");
        assert!(text.contains("\n## Criteria\n"), "{text}");
        for part in [
            "Frame",
            "Terms",
            "Cases",
            "Inputs",
            "External states",
            "Findings",
        ] {
            assert!(!text.contains(&format!("## {part}")), "{text}");
        }
    };

    // No Test plan section at all.
    check(&brief(dir, "1"));

    // A Test plan of words only.
    set_test_plan(dir, "Only words, step 1.");
    check(&brief(dir, "1"));
}

// AC-18, AC-7, AC-8 (E-1): backticked test names in the step item are the step tests.
#[test]
fn explicit_step_tests() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | One. | `brief::a`, `brief::b` |",
    );
    let chead = "| Category | Cases | Tests |\n|---|---|---|\n";
    let ihead = "| Input | Test |\n|---|---|\n";
    set_test_plan(
        dir,
        &format!(
            "{chead}| Main | x | `brief::a` |\n| Other | x | `brief::b` |\n\
             | Both | x | `brief::a`, `brief::b` |\n\n\
             ### Inputs\n\n{ihead}| one | `brief::a` |\n| two | `brief::b` |"
        ),
    );
    let part = |text: &str, title: &str| {
        let (_, rest) = text
            .split_once(&format!("\n## {title}\n"))
            .unwrap_or_else(|| panic!("no {title} in {text}"));
        rest.split("\n## ").next().unwrap().to_string()
    };

    // The test `brief::a` only: the `tests:` line, the cases and the Inputs rows hold it only.
    set_section(
        dir,
        "ABC-1",
        "Plan",
        "1. One. AC-1, with the test `brief::a` only.",
    );
    let text = brief(dir, "1");
    assert!(text.contains("\ntests: brief::a\n"), "{text}");
    assert_eq!(
        part(&text, "Cases"),
        format!("{chead}| Main | x | `brief::a` |\n| Both | x | `brief::a`, `brief::b` |\n")
    );
    assert_eq!(
        part(&text, "Inputs"),
        format!("{ihead}| one | `brief::a` |\n")
    );
    assert!(
        part(&text, "Criteria").contains("| AC-1 | One. |"),
        "{text}"
    );

    // Step order, each name once; a backticked word without `::` is no test.
    set_section(
        dir,
        "ABC-1",
        "Plan",
        "1. One. AC-1: run `cargo test`, then `brief::b`, `brief::a` and `brief::b` again.",
    );
    let text = brief(dir, "1");
    assert!(text.contains("\ntests: brief::b, brief::a\n"), "{text}");

    // No backticked name: the tests of the criteria.
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1, run `cargo test`.");
    let text = brief(dir, "1");
    assert!(text.contains("\ntests: brief::a, brief::b\n"), "{text}");
}

// AC-13 (R-1): a finding range with n > m refuses, in step order with the unknown ids.
#[test]
fn finding_range_not_a_range() {
    fn refuses(dir: &Path, item: &str, reason: &str) {
        set_section(dir, "ABC-1", "Plan", &format!("1. {item}"));
        let out = stapel(dir)
            .args(["brief", "ABC-1", "1"])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "", "{item}");
        assert_eq!(
            String::from_utf8(out.stderr).unwrap(),
            format!("stapel: step 1 {reason}\n"),
            "{item}"
        );
    }
    let repo = fixture();
    let dir = repo.path();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/findings.jsonl"),
        "{\"id\":\"D1-5\"}\n{\"id\":\"D1-6\"}\n",
    )
    .unwrap();

    // A step that names the reversed range only.
    refuses(
        dir,
        "One. Reads D1-6 to D1-5.",
        "names D1-6 to D1-5, which is not a range",
    );

    // The first refusal in step order: an unknown id first, or the range first.
    refuses(
        dir,
        "One. Reads D1-9, then D1-6 to D1-5.",
        "names D1-9, which findings.jsonl does not have",
    );
    refuses(
        dir,
        "One. Reads D1-6 to D1-5, then D1-9.",
        "names D1-6 to D1-5, which is not a range",
    );

    // A range of one id and a range in order are no refusal.
    set_section(
        dir,
        "ABC-1",
        "Plan",
        "1. One. Reads D1-5 to D1-5 and D1-5 to D1-6.",
    );
    let text = brief(dir, "1");
    assert!(
        text.contains("\n## Findings\n- D1-5 (none): none Fate: none\n- D1-6 (none)"),
        "{text}"
    );
}

// AC-9 (D6-4): every run of whitespace in a field is one space, so a field stays on one line.
#[test]
fn finding_fields_on_one_line() {
    let repo = fixture();
    let dir = repo.path();
    let text = findings_brief(
        dir,
        "{\"id\":\"D1-1\",\"severity\":\"ME\\nDIUM\",\
         \"claim\":\"one\\n## Frame\\n\\n- D1-9 (HIGH): fake.\\t\\ttab  end\",\
         \"fate\":\"fix\\r\\n  ed\"}\n",
        "One. Reads D1-1.",
    );
    let tail = "\n## Findings\n\
        - D1-1 (ME DIUM): one ## Frame - D1-9 (HIGH): fake. tab end Fate: fix ed\n";
    assert!(text.ends_with(tail), "{text}");
    assert!(!text.contains("\n## Frame\n\n- D1-9"), "{text}");
}

// AC-7 (D5-1): the cases table is the first table before the first `###` heading.
#[test]
fn cases_table_before_the_subsections() {
    let text = plan_brief(
        "Light tests only.\n\n### Inputs\n\n| Input | Test |\n|---|---|\n| z | `brief::a` |",
        "1",
    );
    assert!(!text.contains("## Cases"), "{text}");
    assert!(
        text.contains("\n## Inputs\n| Input | Test |\n|---|---|\n| z | `brief::a` |\n"),
        "{text}"
    );

    // A table after the text but before the heading is the cases table.
    let text = plan_brief(
        "| Category | Cases | Tests |\n|---|---|---|\n| Main | x | `brief::a` |\n\n\
         ### Inputs\n\n| Input | Test |\n|---|---|\n| z | `brief::a` |",
        "1",
    );
    let (_, rest) = text.split_once("\n## Cases\n").expect(&text);
    assert_eq!(
        rest.split("\n## ").next().unwrap(),
        "| Category | Cases | Tests |\n|---|---|---|\n| Main | x | `brief::a` |\n"
    );
}

// AC-6 (D4D-1): a term whose text runs onto the line of the next name shares that line, and the
// sharing follows a chain of lines.
#[test]
fn terms_on_a_chain_of_lines() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One. AC-1.");
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "Terms:\n\n- **Alpha**: a. **Beta**: b, over\n  a second line\n  and more. \
         **Gamma**: g uses delta.\n- **Delta**: d.\n- **Unused**: u.\n\n\
         | № | Criterion | Test |\n|---|---|---|\n| AC-1 | uses alpha | `brief::a` |",
    );
    let text = brief(dir, "1");
    let (_, rest) = text.split_once("\n## Terms\n").expect(&text);
    assert_eq!(
        rest.split("\n## ").next().unwrap(),
        "- **Alpha**: a. **Beta**: b, over\n  a second line\n  and more. \
         **Gamma**: g uses delta.\n- **Delta**: d.\n"
    );
}
