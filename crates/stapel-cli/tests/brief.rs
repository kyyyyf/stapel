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
    2. Two. Names AC-2 and\n   AC-3 to AC-4.\n\
    3. Three.";

fn golden() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN)).unwrap()
}

/// A ticket ABC-1 with a Frame (with Prior art and Size paragraphs) and a Plan of three steps.
fn fixture() -> tempfile::TempDir {
    let repo = repo_with_ticket();
    set_section(repo.path(), "ABC-1", "Description", FRAME);
    set_section(repo.path(), "ABC-1", "Plan", PLAN);
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
    let text = brief(dir, "1");
    assert!(
        text.ends_with("## Step\n1. One. Names AC-1\n   and goes on.\n"),
        "{text}"
    );
    assert!(!text.contains("paragraph"), "{text}");
}

// AC-3: a Frame that is empty after the drops, and a Frame with no heading after it.
#[test]
fn frame_edges() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "1. One.");
    let only_dropped = "### Frame\n\n\
        **Prior art.** Old tools.\n\n\
        **Size.** About ten lines.\n\n\
        ### Decisions\n\nNot in the brief.";
    set_section(dir, "ABC-1", "Description", only_dropped);
    let text = brief(dir, "1");
    assert!(!text.contains("## Frame"), "{text}");
    assert!(text.ends_with("\n## Step\n1. One.\n"), "{text}");

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

    // `ticket.md` that is not UTF-8.
    std::fs::write(
        tickets.join("ABC-1/ticket.md"),
        [0x23, 0x20, 0xff, 0xfe, 0x0a],
    )
    .unwrap();
    refuses(dir, "ABC-1");
}
