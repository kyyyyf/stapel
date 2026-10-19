//! STP-20 AC-1, AC-3 to AC-9: `stapel next`, the ticket choice and the rules before the build.

mod common;

use common::{bare_dir, repo_with_ticket, set_section, stapel, state_json};
use predicates::str::contains;
use std::path::Path;

const TICKET: &str = ".stapel/tickets/ABC-1/ticket.md";

/// The standard output of `stapel next` with `args`, after the exit code `code` is checked.
fn output(dir: &Path, args: &[&str], code: i32) -> String {
    let out = stapel(dir)
        .arg("next")
        .args(args)
        .assert()
        .code(code)
        .get_output()
        .stdout
        .clone();
    String::from_utf8(out).unwrap()
}

/// The action line, `next: …`, of a run that exits 0.
fn action(dir: &Path) -> String {
    action_of(dir, &[])
}

fn action_of(dir: &Path, args: &[&str]) -> String {
    let text = output(dir, args, 0);
    let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("next: ")).collect();
    assert_eq!(lines.len(), 1, "one action line in: {text}");
    lines[0].to_string()
}

/// The fact lines, `blocked: …`, of a run that exits 0.
fn facts(dir: &Path) -> Vec<String> {
    output(dir, &[], 0)
        .lines()
        .filter(|l| l.starts_with("blocked: "))
        .map(String::from)
        .collect()
}

fn state_path(dir: &Path) -> std::path::PathBuf {
    dir.join(".stapel/tickets/ABC-1/state.json")
}

fn write_state(dir: &Path, v: &serde_json::Value) {
    std::fs::write(state_path(dir), v.to_string()).unwrap();
}

fn set_frame(dir: &Path) {
    set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
}

fn set_plan(dir: &Path, body: &str) {
    set_section(dir, "ABC-1", "Plan", body);
}

/// A ticket whose Description has a Frame and whose five sections are written; nothing is confirmed.
fn written() -> tempfile::TempDir {
    let repo = repo_with_ticket();
    set_frame(repo.path());
    set_plan(repo.path(), "Route: full, auto.\n\n1. One.\n2. Two.");
    repo
}

fn confirm_all(dir: &Path) {
    for s in ["spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", s]).assert().success();
    }
}

fn add_finding(dir: &Path, stage: &str) {
    let line = format!("{{\"v\":1,\"stage\":\"{stage}\"}}\n");
    std::fs::write(dir.join(".stapel/tickets/ABC-1/findings.jsonl"), line).unwrap();
}

fn close(dir: &Path) {
    let mut state = state_json(dir, "ABC-1");
    state["closed"] =
        serde_json::json!({"by": "test-user", "at": "2026-10-19T10:00:00Z", "reason": "done"});
    write_state(dir, &state);
}

// AC-1: the ticket line, the fact lines in configured order, one action line.
#[test]
fn prints_facts_then_one_action() {
    let repo = written();
    let dir = repo.path();
    for s in ["spec", "design", "proof"] {
        stapel(dir).args(["ok", s]).assert().success();
    }
    let mut state = state_json(dir, "ABC-1");
    state["confirmations"][0]["normal_form"] = serde_json::json!(0);
    write_state(dir, &state);
    let text = output(dir, &[], 0);
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/next_text.txt"),
    )
    .unwrap();
    assert_eq!(text, golden);
}

// AC-3
#[test]
fn no_open_ticket_names_new() {
    let repo = common::stapel_repo();
    let dir = repo.path();
    let expected = "next: new-ticket — stapel new \"<title>\"\n";
    assert_eq!(output(dir, &[], 0), expected);
    stapel(dir).args(["new", "First"]).assert().success();
    close(dir);
    assert_eq!(output(dir, &[], 0), expected);
    // An explicit key still names the closed ticket.
    assert!(output(dir, &["ABC-1"], 0).contains("next: closed"));
}

// AC-4, wrong call: exit 2, the reason on stderr, nothing on stdout.
#[test]
fn refuses_a_wrong_call() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args(["next", "NOPE-1"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(contains("stapel: no ticket NOPE-1 in .stapel/tickets"));
    stapel(dir)
        .args(["next", "--jsonx"])
        .assert()
        .code(2)
        .stdout("");
    stapel(dir).args(["new", "Second"]).assert().success();
    stapel(dir)
        .arg("next")
        .assert()
        .code(2)
        .stdout("")
        .stderr(contains(
            "stapel: several open tickets, name one: ABC-1, ABC-2",
        ));

    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".stapel/tickets/abc-1")).unwrap();
    if !dir.join(".stapel/tickets/ABC-1/ticket.md").exists() {
        return; // case-insensitive file system: the two names are one folder
    }
    stapel(dir)
        .args(["next", "abc-1"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(contains(
            "stapel: abc-1 matches several folders that differ only in case",
        ));
}

// AC-4, unreadable: exit 1, the reason on stderr, nothing on stdout.
#[test]
fn refuses_an_unreadable_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let fails = |args: &[&str]| {
        stapel(dir)
            .arg("next")
            .args(args)
            .assert()
            .code(1)
            .stdout("")
            .stderr(contains("stapel: "));
    };

    std::fs::write(
        state_path(dir),
        r#"{"key":"ABC-1","build":{"allowed":false}}"#,
    )
    .unwrap();
    fails(&["ABC-1"]);
    std::fs::write(state_path(dir), "{ broken").unwrap();
    fails(&["ABC-1"]);

    let repo = repo_with_ticket();
    let dir = repo.path();
    let md = dir.join(TICKET);
    std::fs::remove_file(&md).unwrap();
    std::fs::create_dir(&md).unwrap();
    stapel(dir)
        .args(["next", "ABC-1"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("stapel: "));

    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::write(dir.join(".stapel/stapel.toml"), "not [valid").unwrap();
    stapel(dir)
        .arg("next")
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("stapel: .stapel/stapel.toml"));

    let outside = bare_dir();
    stapel(outside.path())
        .arg("next")
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("stapel: no .stapel/stapel.toml here or above"));
}

// AC-5
#[test]
fn closed_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    close(dir);
    let text = output(dir, &["abc-1"], 0);
    assert_eq!(text, "ticket: ABC-1 — First\nnext: closed\n");
}

// AC-6
#[test]
fn frame_first() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let expected = format!("next: write-frame — {TICKET}");
    assert_eq!(action(dir), expected, "TODO");
    set_section(dir, "ABC-1", "Description", "");
    assert_eq!(action(dir), expected, "empty");
    set_section(dir, "ABC-1", "Description", "Only a story.");
    assert_eq!(action(dir), expected, "no Frame");
    set_section(dir, "ABC-1", "Description", "#### Frame\n\nThe problem.");
    assert_eq!(action(dir), expected, "level four");
    set_frame(dir);
    assert_eq!(action(dir), format!("next: write-section Plan — {TICKET}"));
}

// AC-7: the first confirmed section that is not written, in configured order.
#[test]
fn sections_in_order() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_frame(dir);
    set_plan(dir, "1. One.");
    set_section(dir, "ABC-1", "Spec", "TODO");
    assert_eq!(action(dir), format!("next: write-section Spec — {TICKET}"));
    set_section(dir, "ABC-1", "Spec", "The spec.");

    let path = dir.join(TICKET);
    let original = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, original.replace("## Design\n", "## Designs\n")).unwrap();
    assert_eq!(
        action(dir),
        format!("next: write-section Design — {TICKET}")
    );

    std::fs::write(&path, format!("{original}\n## Proof\n\nagain\n")).unwrap();
    assert_eq!(action(dir), format!("next: write-section Proof — {TICKET}"));
    assert!(facts(dir).contains(&"blocked: Proof is duplicated".to_string()));

    // Review and Summary are generated sections: they do not ask to be written here.
    std::fs::write(&path, &original).unwrap();
    assert!(original.contains("## Review\n\nTODO"));
    assert!(!action(dir).starts_with("next: write-section"));
}

// AC-7: the Plan counts as written only when it has steps, each label once.
#[test]
fn plan_structure() {
    let repo = written();
    let dir = repo.path();
    let write_plan = format!("next: write-section Plan — {TICKET}");

    set_plan(dir, "Only prose, no steps.");
    assert_eq!(action(dir), write_plan);
    assert!(facts(dir).contains(&"blocked: the Plan has no steps".to_string()));

    set_plan(dir, "1. One.\n2. Two.\n2. Two again.");
    assert_eq!(action(dir), write_plan);
    assert!(facts(dir).contains(&"blocked: the Plan names step 2 twice".to_string()));

    // Indented, a letter after the dot and an uppercase suffix are not steps.
    for body in ["  1. One.", "1.x One.", "1C. One.", "1.One", "ab. One."] {
        set_plan(dir, body);
        assert_eq!(action(dir), write_plan, "{body}");
        assert!(
            facts(dir).contains(&"blocked: the Plan has no steps".to_string()),
            "{body}"
        );
    }

    set_plan(dir, "12b. A step with a letter.");
    assert_eq!(action(dir), format!("next: spec-review — {TICKET}"));
    assert!(!facts(dir).iter().any(|f| f.contains("Plan")));
}

// AC-8
#[test]
fn spec_review_until_a_fact() {
    let spec_review = format!("next: spec-review — {TICKET}");
    let repo = written();
    let dir = repo.path();
    assert_eq!(action(dir), spec_review);

    add_finding(dir, "step-drift");
    assert_eq!(action(dir), spec_review);

    add_finding(dir, "spec-review");
    assert_eq!(action(dir), "next: confirm — stapel ok ABC-1");

    // A line that is not JSON is skipped.
    let repo = written();
    let dir = repo.path();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/findings.jsonl"),
        "not json\n",
    )
    .unwrap();
    assert_eq!(action(dir), spec_review);

    // One earlier confirmation, of any section, ends the rule.
    let repo = written();
    let dir = repo.path();
    stapel(dir).args(["ok", "plan"]).assert().success();
    assert_eq!(action(dir), "next: confirm — stapel ok ABC-1");
}

// AC-8: a light route has no spec review.
#[test]
fn light_route_skips_spec_review() {
    let repo = written();
    let dir = repo.path();
    let spec_review = format!("next: spec-review — {TICKET}");
    let confirm = "next: confirm — stapel ok ABC-1";
    for (line, light) in [
        ("Route: light, pinned.", true),
        ("Route: LIGHT", true),
        ("Route: lightweight", false),
        ("Route: light-ish", false),
        ("Route: full, auto.", false),
        ("Routes: light", false),
        ("no route line", false),
    ] {
        set_plan(dir, &format!("{line}\n\n1. One."));
        let expected = if light { confirm } else { spec_review.as_str() };
        assert_eq!(action(dir), expected, "{line}");
    }
}

// AC-9
#[test]
fn confirm_names_each_section() {
    let repo = written();
    let dir = repo.path();
    stapel(dir).args(["ok", "spec"]).assert().success();
    stapel(dir).args(["ok", "design"]).assert().success();
    let mut state = state_json(dir, "ABC-1");
    state["confirmations"].as_array_mut().unwrap().remove(0);
    write_state(dir, &state);
    set_section(dir, "ABC-1", "Design", "The design, edited.");

    let lines = facts(dir);
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert_eq!(lines[0], "blocked: spec not confirmed");
    assert!(
        lines[1].starts_with(
            "blocked: design confirmation is stale: changed since confirmed by test-user at "
        ),
        "{}",
        lines[1]
    );
    assert_eq!(lines[2], "blocked: proof not confirmed");
    assert_eq!(lines[3], "blocked: plan not confirmed");
    assert_eq!(action(dir), "next: confirm — stapel ok ABC-1");

    // Dependencies and a missing section are stale too, in the words of `stapel status`.
    let repo = written();
    let dir = repo.path();
    confirm_all(dir);
    set_section(dir, "ABC-1", "Spec", "The spec, edited.");
    let lines = facts(dir);
    assert!(lines[0].starts_with("blocked: spec confirmation is stale: changed since confirmed"));
    assert!(lines.contains(
        &"blocked: design confirmation is stale: depends on spec, which changed".to_string()
    ));
    assert_eq!(action(dir), "next: confirm — stapel ok ABC-1");
}

// AC-7, coverage (step 1c): a label with a letter is named with its letter.
#[test]
fn plan_labels_with_letters() {
    let repo = written();
    let dir = repo.path();
    set_plan(dir, "Route: full, auto.\n\n12b. One.\n12b. Again.");
    assert_eq!(action(dir), format!("next: write-section Plan — {TICKET}"));
    assert!(facts(dir).contains(&"blocked: the Plan names step 12b twice".to_string()));
}

// AC-7, coverage (step 1c): Review and Summary are generated and do not hold the build.
#[test]
fn generated_sections_wait() {
    let repo = written();
    let dir = repo.path();
    confirm_all(dir);
    let text = std::fs::read_to_string(dir.join(TICKET)).unwrap();
    assert!(text.contains("## Review\n\nTODO"));
    assert!(text.contains("## Summary\n\nTODO"));
    assert_eq!(action(dir), format!("next: build-step step 1 — {TICKET}"));
    assert!(facts(dir).is_empty());
}

// AC-8, coverage (step 1c): the first word of the Route line, with trailing punctuation removed.
#[test]
fn route_word_edges() {
    let repo = written();
    let dir = repo.path();
    let spec_review = format!("next: spec-review — {TICKET}");
    let confirm = "next: confirm — stapel ok ABC-1";
    for (line, light) in [
        ("Route: light.", true),
        ("Route: light;", true),
        ("Route: light!", false),
        ("Route: light-ish", false),
    ] {
        set_plan(dir, &format!("{line}\n\n1. One."));
        let expected = if light { confirm } else { spec_review.as_str() };
        assert_eq!(action(dir), expected, "{line}");
    }
}

// AC-9, coverage (step 1c): one fact per stale reason, the changed one first.
#[test]
fn stale_reasons_each_a_fact() {
    let repo = written();
    let dir = repo.path();
    stapel(dir).args(["ok", "design"]).assert().success();
    set_section(dir, "ABC-1", "Design", "The design, edited.");
    set_section(dir, "ABC-1", "Spec", "The spec, edited.");
    let lines = facts(dir);
    let design: Vec<&String> = lines
        .iter()
        .filter(|l| l.starts_with("blocked: design "))
        .collect();
    assert_eq!(design.len(), 2, "{lines:?}");
    assert!(
        design[0].starts_with(
            "blocked: design confirmation is stale: changed since confirmed by test-user at "
        ),
        "{}",
        design[0]
    );
    assert_eq!(
        design[1],
        "blocked: design confirmation is stale: depends on spec, which changed"
    );
    assert_eq!(action(dir), "next: confirm — stapel ok ABC-1");
}

// AC-7 and AC-9, coverage (step 1c): a lowercase key names the ticket, the output uses the folder key.
#[test]
fn folder_key_in_outputs() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    assert_eq!(
        action_of(dir, &["abc-1"]),
        format!("next: write-frame — {TICKET}")
    );

    let repo = written();
    let dir = repo.path();
    add_finding(dir, "spec-review");
    assert_eq!(action_of(dir, &["abc-1"]), "next: confirm — stapel ok ABC-1");
    assert!(output(dir, &["abc-1"], 0).starts_with("ticket: ABC-1 — "));
}

// AC-10: the history is read only when no rule before the build applies.
#[test]
fn history_read_only_when_needed() {
    fn commit(dir: &Path, subject: &str) {
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-q", "--allow-empty", "-m", subject],
        ] {
            let out = std::process::Command::new("git")
                .args(&args)
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
        }
    }
    fn shallow_clone(origin: &Path, away: &Path) -> std::path::PathBuf {
        let out = std::process::Command::new("git")
            .args(["clone", "-q", "--depth", "1"])
            .arg(format!("file://{}", origin.display()))
            .arg("clone")
            .current_dir(away)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(out.status.success());
        away.join("clone")
    }
    let bad_base = |dir: &Path| {
        let mut state = state_json(dir, "ABC-1");
        state["base"] = serde_json::json!("0".repeat(40));
        write_state(dir, &state);
    };
    let confirm = "next: confirm — stapel ok ABC-1";

    // A shallow clone at the confirm stage: the action is printed.
    let repo = written();
    let dir = repo.path();
    add_finding(dir, "spec-review");
    commit(dir, "initial");
    commit(dir, "second");
    let away = tempfile::tempdir().unwrap();
    let clone = shallow_clone(dir, away.path());
    assert_eq!(action(&clone), confirm);

    // The same clone with every section confirmed: the shallow refusal.
    let repo = written();
    let dir = repo.path();
    confirm_all(dir);
    commit(dir, "initial");
    commit(dir, "second");
    let away = tempfile::tempdir().unwrap();
    let clone = shallow_clone(dir, away.path());
    stapel(&clone)
        .arg("next")
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("stapel: this is a shallow clone"));

    // A base off the history at the confirm stage: the action is printed.
    let repo = written();
    let dir = repo.path();
    add_finding(dir, "spec-review");
    commit(dir, "initial");
    bad_base(dir);
    assert_eq!(action(dir), confirm);

    // With every section confirmed: the refusal of `stapel check`.
    let repo = written();
    let dir = repo.path();
    confirm_all(dir);
    commit(dir, "initial");
    bad_base(dir);
    stapel(dir)
        .arg("next")
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("stapel: state.json has base"));
}

// AC-11: a step commit whose label the Plan does not name.
#[test]
fn step_outside_the_plan() {
    fn commit(dir: &Path, files: &[(&str, &str)], subject: &str) {
        for (rel, text) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-q", "--allow-empty", "-m", subject],
        ] {
            let out = std::process::Command::new("git")
                .args(&args)
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
        }
    }
    let repo = written();
    let dir = repo.path();
    set_plan(
        dir,
        "Route: full, auto.\n\n1. One.\n2. Two.\n3. Three.\n4. Four.",
    );
    confirm_all(dir);
    commit(dir, &[], "initial");
    let test = "#[test]\nfn five() {\n    assert!(true);\n}\n";
    commit(
        dir,
        &[("crates/tiny/tests/five.rs", test)],
        "ABC-1 step 5 RED: five",
    );
    assert!(facts(dir).contains(&"blocked: history has step 5, the Plan does not".to_string()));
    assert_eq!(action(dir), format!("next: write-section Plan — {TICKET}"));
}

// AC-12: a history outcome that is a fault blocks the build.
#[test]
fn history_faults_block_the_build() {
    fn commit(dir: &Path, files: &[(&str, &str)], subject: &str) -> String {
        for (rel, text) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-q", "--allow-empty", "-m", subject],
        ] {
            let out = std::process::Command::new("git")
                .args(&args)
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
        }
        let out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim()[..7].to_string()
    }
    fn ready() -> tempfile::TempDir {
        let repo = written();
        confirm_all(repo.path());
        commit(repo.path(), &[], "initial");
        repo
    }
    fn test_file(n: &str) -> (String, &'static str) {
        (
            format!("crates/tiny/tests/t{n}.rs"),
            "#[test]\nfn t() {\n    assert!(true);\n}\n",
        )
    }
    let fix = "next: fix-history step 1 — stapel check --history ABC-1".to_string();
    let lib = |text: &str| ("crates/tiny/src/lib.rs".to_string(), text.to_string());

    // A RED that changes code, with its GREEN after it.
    let repo = ready();
    let dir = repo.path();
    let (path, text) = test_file("1");
    let (lib_path, lib_text) = lib("// red\n");
    let red = commit(
        dir,
        &[(&path, text), (&lib_path, &lib_text)],
        "ABC-1 step 1 RED: test and code",
    );
    commit(
        dir,
        &[(&lib_path, "// green\n")],
        "ABC-1 step 1 GREEN: code",
    );
    let lines = facts(dir);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with(&format!("blocked: step 1: red-changes-code at {red} — ")),
        "{}",
        lines[0]
    );
    assert_eq!(action(dir), fix);

    // A GREEN without a RED.
    let repo = ready();
    let dir = repo.path();
    let green = commit(
        dir,
        &[("crates/tiny/src/lib.rs", "// green\n")],
        "ABC-1 step 1 GREEN: code",
    );
    assert_eq!(
        facts(dir),
        vec![format!(
            "blocked: step 1: unpaired at {green} — no RED commit"
        )]
    );
    assert_eq!(action(dir), fix);

    // Two faults: two facts in history order, the action names the first.
    let repo = ready();
    let dir = repo.path();
    let (path, text) = test_file("1");
    let red = commit(
        dir,
        &[(&path, text), (&lib_path, &lib_text)],
        "ABC-1 step 1 RED: test and code",
    );
    commit(
        dir,
        &[(&lib_path, "// green\n")],
        "ABC-1 step 1 GREEN: code",
    );
    let green = commit(dir, &[(&lib_path, "// two\n")], "ABC-1 step 2 GREEN: code");
    let lines = facts(dir);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].starts_with(&format!("blocked: step 1: red-changes-code at {red} — ")),
        "{}",
        lines[0]
    );
    assert_eq!(
        lines[1],
        format!("blocked: step 2: unpaired at {green} — no RED commit")
    );
    assert_eq!(action(dir), fix);

    // Interleaved steps are `unverified`, which `next` leaves to the full check.
    let repo = ready();
    let dir = repo.path();
    let (path1, text) = test_file("1");
    commit(dir, &[(&path1, text)], "ABC-1 step 1 RED: one");
    let (path2, text) = test_file("2");
    commit(dir, &[(&path2, text)], "ABC-1 step 2 RED: two");
    commit(dir, &[(&lib_path, "// one\n")], "ABC-1 step 1 GREEN: one");
    commit(dir, &[(&lib_path, "// two\n")], "ABC-1 step 2 GREEN: two");
    assert!(facts(dir).is_empty(), "{:?}", facts(dir));
}

// AC-12: an open RED is judged alone; its own `unpaired` is no fault.
#[test]
fn open_red_is_judged_alone() {
    fn commit(dir: &Path, files: &[(&str, &str)], subject: &str) -> String {
        for (rel, text) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-q", "--allow-empty", "-m", subject],
        ] {
            let out = std::process::Command::new("git")
                .args(&args)
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
        }
        let out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim()[..7].to_string()
    }
    fn ready() -> tempfile::TempDir {
        let repo = written();
        confirm_all(repo.path());
        commit(repo.path(), &[], "initial");
        repo
    }
    let test = "#[test]\nfn t() {\n    assert!(true);\n}\n";
    let fix = |label: &str| format!("next: fix-history {label} — stapel check --history ABC-1");

    // Step 1 RED only, which changes code.
    let repo = ready();
    let dir = repo.path();
    let red = commit(
        dir,
        &[
            ("crates/tiny/tests/t1.rs", test),
            ("crates/tiny/src/lib.rs", "// code\n"),
        ],
        "ABC-1 step 1 RED: test and code",
    );
    let lines = facts(dir);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with(&format!("blocked: step 1: red-changes-code at {red}")),
        "{}",
        lines[0]
    );
    assert_eq!(action(dir), fix("step 1"));

    // Step 1 RED only, with no test.
    let repo = ready();
    let dir = repo.path();
    let red = commit(dir, &[], "ABC-1 step 1 RED: no test");
    assert!(
        facts(dir)
            .iter()
            .any(|f| f.starts_with(&format!("blocked: step 1: no-tests at {red}"))),
        "{:?}",
        facts(dir)
    );
    assert_eq!(action(dir), fix("step 1"));

    // Step 2 RED after step 1 RED with no GREEN: step 1 is a fault, step 2 is the open RED.
    let repo = ready();
    let dir = repo.path();
    commit(
        dir,
        &[("crates/tiny/tests/t1.rs", test)],
        "ABC-1 step 1 RED: one",
    );
    let second = commit(
        dir,
        &[("crates/tiny/tests/t2.rs", test)],
        "ABC-1 step 2 RED: two",
    );
    assert_eq!(
        facts(dir),
        vec![format!(
            "blocked: step 1: unpaired at {second} — no GREEN commit"
        )]
    );
    assert_eq!(action(dir), fix("step 1"));

    // Step 2 RED with no test while step 1 has no commit.
    let repo = ready();
    let dir = repo.path();
    let red = commit(dir, &[], "ABC-1 step 2 RED: no test");
    assert!(
        facts(dir)
            .iter()
            .any(|f| f.starts_with(&format!("blocked: step 2: no-tests at {red}"))),
        "{:?}",
        facts(dir)
    );
    assert_eq!(action(dir), fix("step 2"));
}

// AC-13: a failing current check blocks the build.
#[test]
fn failing_check_blocks_the_build() {
    fn commit(dir: &Path, subject: &str) -> String {
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-q", "--allow-empty", "-m", subject],
        ] {
            let out = std::process::Command::new("git")
                .args(&args)
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
        }
        let out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }
    let repo = written();
    let dir = repo.path();
    let toml = dir.join(".stapel/stapel.toml");
    let mut text = std::fs::read_to_string(&toml).unwrap();
    text.push_str("\n[check]\nrunner = \"cargo\"\ntimeout_secs = 900\n");
    std::fs::write(&toml, text).unwrap();
    confirm_all(dir);
    let head = commit(dir, "initial");
    let record = serde_json::json!({
        "v": 1, "kind": "check", "id": "c1", "at": "2026-10-19T10:00:00Z",
        "ticket": "ABC-1", "head": head, "result": "fail", "steps": [],
    });
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/runs.jsonl"),
        format!("{record}\n"),
    )
    .unwrap();
    assert_eq!(
        facts(dir),
        vec![format!("blocked: check fails at {}", &head[..7])]
    );
    assert_eq!(
        action(dir),
        "next: fix-check — .stapel/tickets/ABC-1/runs.jsonl"
    );

    // After a new commit the record is stale and does not block.
    std::fs::write(dir.join("note.txt"), "a later change\n").unwrap();
    commit(dir, "ABC-1: a later commit");
    assert!(facts(dir).is_empty(), "{:?}", facts(dir));
    assert_eq!(action(dir), format!("next: build-step step 1 — {TICKET}"));
}

// AC-14: the first Plan step that is not done is the one to build.
#[test]
fn first_open_step() {
    fn commit(dir: &Path, files: &[(&str, &str)], subject: &str) {
        for (rel, text) in files {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-q", "--allow-empty", "-m", subject],
        ] {
            let out = std::process::Command::new("git")
                .args(&args)
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
        }
    }
    fn ready() -> tempfile::TempDir {
        let repo = written();
        confirm_all(repo.path());
        commit(repo.path(), &[], "initial");
        repo
    }
    let test = "#[test]\nfn t() {\n    assert!(true);\n}\n";
    let red = |dir: &Path, n: &str| {
        commit(
            dir,
            &[(&format!("crates/tiny/tests/t{n}.rs"), test)],
            &format!("ABC-1 step {n} RED: test"),
        )
    };
    let green = |dir: &Path, n: &str| {
        commit(
            dir,
            &[("crates/tiny/src/lib.rs", &format!("// {n}\n"))],
            &format!("ABC-1 step {n} GREEN: code"),
        )
    };

    let repo = ready();
    assert_eq!(
        action(repo.path()),
        format!("next: build-step step 1 — {TICKET}")
    );

    let repo = ready();
    red(repo.path(), "1");
    assert_eq!(
        action(repo.path()),
        format!("next: green-step step 1 — {TICKET}")
    );
    assert!(facts(repo.path()).is_empty());

    green(repo.path(), "1");
    assert_eq!(
        action(repo.path()),
        format!("next: build-step step 2 — {TICKET}")
    );

    let repo = ready();
    red(repo.path(), "2");
    green(repo.path(), "2");
    assert_eq!(
        action(repo.path()),
        format!("next: build-step step 1 — {TICKET}")
    );
}

// AC-15: a coverage or cleanup commit marks a Plan step done.
#[test]
fn coverage_and_cleanup_steps_are_done() {
    fn commit(dir: &Path, subject: &str) {
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-q", "--allow-empty", "-m", subject],
        ] {
            let out = std::process::Command::new("git")
                .args(&args)
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
        }
    }
    // The action after `subject` is committed on a Plan with `steps`.
    let after = |steps: &str, subject: &str| {
        let repo = written();
        let dir = repo.path();
        set_plan(dir, &format!("Route: full, auto.\n\n{steps}"));
        confirm_all(dir);
        commit(dir, "initial");
        commit(dir, subject);
        action(dir)
    };
    let build = |label: &str| format!("next: build-step {label} — {TICKET}");

    assert_eq!(
        after("1c. A.\n2. B.", "ABC-1 step 1c coverage: more tests"),
        build("step 2")
    );
    assert_eq!(
        after("4. A.\n5. B.", "ABC-1 step 4 cleanup: dead code"),
        build("step 5")
    );
    assert_eq!(
        after("2. A.\n3. B.", "ABC-1 step 2 notes"),
        build("step 2"),
        "no colon"
    );
    assert_eq!(
        after("2. A.\n3. B.", "ABC-1 step 2: notes"),
        build("step 2"),
        "no word"
    );
}
