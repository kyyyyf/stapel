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
