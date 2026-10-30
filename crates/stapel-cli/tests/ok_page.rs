//! STP-25: the dialog page of `stapel ok KEY` and its section lines.

mod common;

use common::{ask, repo_with_ticket, set_section, stapel, state_json};
use std::path::Path;

const ANSWER: &str = "Answer Yes or No; do not choose \"don't ask again\".";

fn confirm(dir: &Path, section: &str) {
    stapel(dir)
        .args(["ok", "ABC-1", section])
        .assert()
        .success();
}

#[test]
fn section_lines() {
    // Spec changed, design stale by its dependency, proof and plan never confirmed.
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm(dir, "spec");
    confirm(dir, "design");
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert_eq!(
        reason,
        format!(
            "stapel: confirm 4 waiting sections of ABC-1; runs: stapel\n\
             spec: 2 lines changed (+1 -1)\n\
             design: unchanged; depends on spec, which changed\n\
             proof: new, 1 line\n\
             plan: new, 1 line\n\
             Criteria:\n\
             - The spec.\n\
             + The spec, revised.\n\
             {ANSWER}"
        )
    );

    // A confirmation without stored text shows the section as new.
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "line one\nline two\nline three");
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    let path = dir.join(".stapel/tickets/ABC-1/state.json");
    let mut state = state_json(dir, "ABC-1");
    for c in state["confirmations"].as_array_mut().unwrap() {
        if c["section"] == "plan" {
            c.as_object_mut().unwrap().remove("text");
        }
    }
    std::fs::write(path, state.to_string()).unwrap();
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("\nplan: new, 3 lines; depends on spec, which changed\n"),
        "{reason}"
    );

    // A re-wrapped Spec paragraph: four changed lines, and no Criteria part.
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Spec", "keep one\nkeep two");
    confirm(dir, "spec");
    set_section(dir, "ABC-1", "Spec", "keep one keep\ntwo");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("\nspec: 4 lines changed (+2 -2)\n"),
        "{reason}"
    );
    assert!(!reason.contains("Criteria:"), "{reason}");
    assert!(
        reason.starts_with("stapel: confirm 4 waiting sections of ABC-1; runs: stapel\n"),
        "{reason}"
    );
    assert!(reason.ends_with(&format!("\n{ANSWER}")), "{reason}");
}

#[test]
fn criteria_new_and_changed() {
    let spec = "- term: old\n\
                | № | Criterion | Test |\n\
                |---|---|---|\n\
                | AC-1 | first | `t::a` |\n\
                | AC-2 | second | `t::b` |";
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Spec", spec);
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!(
            "\nCriteria:\n  | AC-1 | first | `t::a` |\n  | AC-2 | second | `t::b` |\n\
             spec, other lines: +1 -0\n{ANSWER}"
        )),
        "{reason}"
    );

    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    let changed = "- term: new\n\
                   | № | Criterion | Test |\n\
                   |---|---|---|\n\
                   | AC-1 | first | `t::a` |\n\
                   | AC-2 | second | `t::c` |\n\
                   | AC-3 | third | `t::d` |";
    set_section(dir, "ABC-1", "Spec", changed);
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!(
            "\nCriteria:\n- | AC-2 | second | `t::b` |\n+ | AC-2 | second | `t::c` |\n\
             + | AC-3 | third | `t::d` |\nspec, other lines: +1 -1\n{ANSWER}"
        )),
        "{reason}"
    );

    // The Spec does not wait: no Criteria part.
    confirm(dir, "spec");
    set_section(dir, "ABC-1", "Design", "The design, revised.");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(!reason.contains("Criteria:"), "{reason}");

    // A blank line and a re-wrap among the other lines: logical lines are the same, so no
    // count line and no Criteria part (+0 -0).
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "intro one\nintro two\n\n| AC-1 | first | `t::a` |",
    );
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "intro one intro\ntwo\n\n\n  | AC-1 | first | `t::a` |",
    );
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(!reason.contains("Criteria:"), "{reason}");
    assert!(!reason.contains("other lines"), "{reason}");

    // A removed row: shown with `- `, and no count line.
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "intro one intro two\n\n| AC-1 | first | `t::a` |\n| AC-2 | second | `t::b` |",
    );
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "intro one intro two\n\n| AC-1 | first | `t::a` |",
    );
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!(
            "\nCriteria:\n- | AC-2 | second | `t::b` |\n{ANSWER}"
        )),
        "{reason}"
    );

    // A confirmation without stored text: every row and the other lines as new.
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Spec", "intro\n\n| AC-1 | first | `t::a` |");
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "intro\n\n| AC-1 | first | `t::a` |\n| AC-2 | second | `t::b` |",
    );
    let path = dir.join(".stapel/tickets/ABC-1/state.json");
    let mut state = state_json(dir, "ABC-1");
    for c in state["confirmations"].as_array_mut().unwrap() {
        if c["section"] == "spec" {
            c.as_object_mut().unwrap().remove("text");
        }
    }
    std::fs::write(path, state.to_string()).unwrap();
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!(
            "\nCriteria:\n  | AC-1 | first | `t::a` |\n  | AC-2 | second | `t::b` |\n\
             spec, other lines: +1 -0\n{ANSWER}"
        )),
        "{reason}"
    );

    // A row inside a fence is not a row; an indented row is.
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "```\n| AC-1 | fenced | `t::a` |\n```\n\n   | AC-2 | real | `t::b` |",
    );
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!(
            "\nCriteria:\n  | AC-2 | real | `t::b` |\nspec, other lines: +1 -0\n{ANSWER}"
        )),
        "{reason}"
    );

    // Every row went: the removed rows are shown with `- `.
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(
        dir,
        "ABC-1",
        "Spec",
        "intro\n\n| AC-1 | first | `t::a` |\n| AC-2 | second | `t::b` |",
    );
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    set_section(dir, "ABC-1", "Spec", "intro");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!(
            "\nCriteria:\n- | AC-1 | first | `t::a` |\n- | AC-2 | second | `t::b` |\n{ANSWER}"
        )),
        "{reason}"
    );
}

#[test]
fn criteria_without_rows() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Spec", "first line\nwrapped here\n\nsecond paragraph");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!(
            "\nCriteria:\n  first line wrapped here\n  second paragraph\n{ANSWER}"
        )),
        "{reason}"
    );
    assert!(!reason.contains("other lines"), "{reason}");

    // An `AC-` line without ` |` after the number is not a row.
    set_section(dir, "ABC-1", "Spec", "| AC-1x | no row\n\nsecond");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!("\nCriteria:\n  | AC-1x | no row\n  second\n{ANSWER}")),
        "{reason}"
    );
    assert!(!reason.contains("other lines"), "{reason}");

    let repo = repo_with_ticket();
    let dir = repo.path();
    let page = |text: &str| {
        set_section(dir, "ABC-1", "Spec", text);
        ask(dir, "stapel ok ABC-1").0
    };

    // A heading is a line of its own; four spaces of indent make it plain text.
    let reason = page("para\n   # h\nafter\n    # x\nend");
    assert!(
        reason.contains(&format!(
            "\nCriteria:\n  para\n  # h\n  after # x end\n{ANSWER}"
        )),
        "{reason}"
    );

    // No heading inside a fence or an HTML comment.
    let reason = page("para\n```\n# x\n```\nafter");
    assert!(
        reason.contains(&format!("\nCriteria:\n  para ``` # x ``` after\n{ANSWER}")),
        "{reason}"
    );
    let reason = page("para\n<!--\n# x\n-->\nafter");
    assert!(
        reason.contains(&format!("\nCriteria:\n  para <!-- # x --> after\n{ANSWER}")),
        "{reason}"
    );

    // List items start a line; `1.x` does not.
    let reason = page("intro\n- a\n- b\n* c\n1. d\n12. e\n2.x tail");
    assert!(
        reason.contains(&format!(
            "\nCriteria:\n  intro\n  - a\n  - b\n  * c\n  1. d\n  12. e 2.x tail\n{ANSWER}"
        )),
        "{reason}"
    );

    // The diff form: only inserted and deleted logical lines, no count line.
    set_section(dir, "ABC-1", "Spec", "one\n\ntwo");
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    let reason = page("one\n\nthree");
    assert!(
        reason.contains(&format!("\nCriteria:\n- two\n+ three\n{ANSWER}")),
        "{reason}"
    );
    assert!(!reason.contains("other lines"), "{reason}");
}

/// Inserts the template's `description` entry as the first `[[sections]]` entry.
fn add_description_section(dir: &Path) {
    let path = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let at = text.find("[[sections]]").unwrap();
    let entry = "[[sections]]\nid = \"description\"\ntitle = \"Description\"\nowner = \"product\"\n\n";
    std::fs::write(&path, format!("{}{entry}{}", &text[..at], &text[at..])).unwrap();
}

#[test]
fn frame_and_decisions_new_and_changed() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    add_description_section(dir);
    let body = "Source: the chat.\n\n### Frame\n\n**Problem.** It hurts\nover two lines.\n\n\
                - first item\n- second item\n\n### Decisions\n\n- decide one\n- decide two";
    set_section(dir, "ABC-1", "Description", body);
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!(
            "\nFrame:\n  Source: the chat.\n  ### Frame\n  **Problem.** It hurts over two lines.\n  \
             - first item\n  - second item\nCriteria:\n  The spec.\n\
             Decisions:\n  - decide one\n  - decide two\n{ANSWER}"
        )),
        "{reason}"
    );

    // After a confirmation, one Decisions item added: only that item, no Frame part.
    for id in ["description", "spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    set_section(
        dir,
        "ABC-1",
        "Description",
        &format!("{body}\n- decide three"),
    );
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!("\nDecisions:\n+ - decide three\n{ANSWER}")),
        "{reason}"
    );
    assert!(!reason.contains("Frame:"), "{reason}");

    // `### decisions` is no heading: its lines stay in the Frame part.
    let repo = repo_with_ticket();
    let dir = repo.path();
    add_description_section(dir);
    set_section(
        dir,
        "ABC-1",
        "Description",
        "Source.\n\n### decisions\n\n- x\n\n### Decisions\n\n- d",
    );
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains(&format!(
            "\nFrame:\n  Source.\n  ### decisions\n  - x\nCriteria:\n  The spec.\n\
             Decisions:\n  - d\n{ANSWER}"
        )),
        "{reason}"
    );
}

#[test]
fn description_without_headings() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    add_description_section(dir);

    // No `### Frame`: every line outside `### Decisions` is the Frame part.
    set_section(
        dir,
        "ABC-1",
        "Description",
        "Plain paragraph\nwrapped.\n\n- an item\n\n### Decisions\n\n- d1",
    );
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("\nFrame:\n  Plain paragraph wrapped.\n  - an item\nCriteria:\n"),
        "{reason}"
    );
    assert!(
        reason.contains(&format!("\nDecisions:\n  - d1\n{ANSWER}")),
        "{reason}"
    );

    // No `### Decisions`: all of it is the Frame part, and no Decisions label.
    set_section(
        dir,
        "ABC-1",
        "Description",
        "Plain paragraph.\n\n### Frame\n\nframe text",
    );
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("\nFrame:\n  Plain paragraph.\n  ### Frame\n  frame text\nCriteria:\n"),
        "{reason}"
    );
    assert!(!reason.contains("Decisions:"), "{reason}");
}

/// Drops every `depends_on` line of `.stapel/stapel.toml`, so a changed Spec makes no other
/// section stale, and adds the `description` section.
fn independent_sections(dir: &Path) {
    let path = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| !l.starts_with("depends_on"))
        .collect();
    std::fs::write(&path, kept.join("\n") + "\n").unwrap();
    add_description_section(dir);
}

/// `- <prefix>01` to `- <prefix>NN`, one list item per line.
fn items(prefix: &str, n: usize) -> String {
    (1..=n)
        .map(|i| format!("- {prefix}{i:02}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn dialog_limit_and_cut() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    independent_sections(dir);
    for id in ["design", "proof", "plan"] {
        confirm(dir, id);
    }
    let long = format!("| AC-1 | {}é{} | `t::a` |", "a".repeat(87), "b".repeat(200));
    let mut spec = vec![long, "| AC-2 | red\x1b[31m and\ttab | `t::b` |".to_string()];
    spec.extend((3..=10).map(|i| format!("| AC-{i} | row {i} | `t::{i}` |")));
    set_section(dir, "ABC-1", "Spec", &spec.join("\n"));
    let description = format!(
        "{}\n\n### Decisions\n\n{}",
        items("frame ", 12),
        items("decide ", 3)
    );
    set_section(dir, "ABC-1", "Description", &description);
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/ok_page_dialog.txt"),
    )
    .unwrap();
    assert_eq!(reason, golden.trim_end_matches('\n'));
    assert_eq!(reason.lines().count(), 22);
    assert!(reason.lines().all(|l| l.chars().count() <= 100), "{reason}");
}

#[test]
fn dialog_exactly_at_the_limit() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    independent_sections(dir);
    for id in ["design", "proof", "plan"] {
        confirm(dir, id);
    }
    let overflow =
        "… 1 more line (frame 0, criteria 1, decisions 0); all of them: stapel status ABC-1";

    // Frame label and 15 lines, Criteria label and one line: exactly 22 lines, no overflow line.
    set_section(dir, "ABC-1", "Description", &items("f", 15));
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert_eq!(reason.lines().count(), 22, "{reason}");
    assert!(!reason.contains("more line"), "{reason}");
    assert!(
        reason.ends_with(&format!("\n  - f15\nCriteria:\n  The spec.\n{ANSWER}")),
        "{reason}"
    );

    // One more Frame line: the Criteria part has no room, its one line is counted.
    set_section(dir, "ABC-1", "Description", &items("f", 16));
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert_eq!(reason.lines().count(), 22, "{reason}");
    assert!(
        reason.ends_with(&format!("\n  - f16\n{overflow}\n{ANSWER}")),
        "{reason}"
    );
    assert!(!reason.contains("Criteria:"), "{reason}");

    // A Decisions label with no room for a line is left out; its two lines are counted.
    confirm(dir, "spec");
    let description = format!("{}\n\n### Decisions\n\n{}", items("f", 17), items("d", 2));
    set_section(dir, "ABC-1", "Description", &description);
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert_eq!(reason.lines().count(), 22, "{reason}");
    assert!(
        reason.ends_with(&format!(
            "\n  - f17\n… 2 more lines (frame 0, criteria 0, decisions 2); all of them: stapel \
             status ABC-1\n{ANSWER}"
        )),
        "{reason}"
    );
    assert!(!reason.contains("Decisions:"), "{reason}");
}

#[test]
fn many_waiting_sections() {
    let repo = common::stapel_repo();
    let dir = repo.path();
    let path = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let from = text.find("[[sections]]").unwrap();
    let to = text.find("# Role -> model").unwrap();
    let sections: String = (1..=12)
        .map(|i| format!("[[sections]]\nid = \"s{i}\"\ntitle = \"S{i}\"\nowner = \"product\"\n\n"))
        .collect();
    let rest = text[to..].replace(
        "requires = [\"spec\", \"design\", \"proof\"]",
        "requires = [\"s1\"]",
    );
    std::fs::write(&path, format!("{}{sections}{rest}", &text[..from])).unwrap();
    stapel(dir).args(["new", "First"]).assert().success();
    for i in 1..=12 {
        set_section(dir, "ABC-1", &format!("S{i}"), &format!("text {i}"));
    }

    let (reason, _) = ask(dir, "stapel ok ABC-1");
    let lines: Vec<&str> = reason.lines().collect();
    assert_eq!(lines.len(), 13, "{reason}");
    assert_eq!(
        lines[0],
        "stapel: confirm 12 waiting sections of ABC-1; runs: stapel"
    );
    assert_eq!(lines[1], "s1: new, 1 line");
    assert_eq!(lines[10], "s10: new, 1 line");
    assert_eq!(lines[11], "… 2 more waiting sections: s11, s12");
    assert_eq!(lines[12], ANSWER);

    // Eleven waiting sections: one folded.
    confirm(dir, "s12");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    let lines: Vec<&str> = reason.lines().collect();
    assert_eq!(lines.len(), 13, "{reason}");
    assert_eq!(
        lines[0],
        "stapel: confirm 11 waiting sections of ABC-1; runs: stapel"
    );
    assert_eq!(lines[11], "… 1 more waiting section: s11");
}

#[test]
fn terminal_prints_the_whole_page() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    independent_sections(dir);
    let long = format!("| AC-1 | {}é{} | `t::a` |", "a".repeat(87), "b".repeat(200));
    let mut spec = vec![long, "| AC-2 | red\x1b[31m and\ttab | `t::b` |".to_string()];
    spec.extend((3..=10).map(|i| format!("| AC-{i} | row {i} | `t::{i}` |")));
    set_section(dir, "ABC-1", "Spec", &spec.join("\n"));
    let description = format!(
        "{}\n\n### Decisions\n\n{}",
        items("frame ", 12),
        items("decide ", 3)
    );
    set_section(dir, "ABC-1", "Description", &description);
    let out = stapel(dir).args(["ok", "ABC-1"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // Each hash is `sha256:` and hex digits up to the closing bracket.
    let normalized: String = stdout
        .lines()
        .map(|l| match (l.starts_with("confirmed:"), l.find("sha256:")) {
            (true, Some(i)) => format!("{}sha256:HASH)\n", &l[..i]),
            _ => format!("{l}\n"),
        })
        .collect();
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/ok_page_terminal.txt"),
    )
    .unwrap();
    assert_eq!(normalized, golden);
}
