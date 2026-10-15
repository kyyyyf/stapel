//! STP-17 AC-1 to AC-11: `stapel ok KEY`, the change block and the argument rule.

mod common;

use common::{git_config, repo_with_ticket, set_section, snapshot, stapel, state_json};
use predicates::prelude::PredicateBooleanExt;
use std::path::Path;

fn golden(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name),
    )
    .unwrap()
}

/// Replaces every `sha256:` plus 12 hex digits with `sha256:HASH`.
fn normalize(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("sha256:") {
        let (head, tail) = rest.split_at(i + "sha256:".len());
        out.push_str(head);
        let hex = tail.bytes().take_while(u8::is_ascii_hexdigit).count();
        if hex >= 12 {
            out.push_str("HASH");
            rest = &tail[12..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn stdout_of(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}

fn confirm(dir: &Path, section: &str) {
    stapel(dir)
        .args(["ok", "ABC-1", section])
        .assert()
        .success();
}

fn confirmations(dir: &Path) -> Vec<serde_json::Value> {
    state_json(dir, "ABC-1")["confirmations"]
        .as_array()
        .unwrap()
        .clone()
}

fn decisions(dir: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join(".stapel/tickets/ABC-1/decisions.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn edit_state(dir: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let path = dir.join(".stapel/tickets/ABC-1/state.json");
    let mut state = state_json(dir, "ABC-1");
    edit(&mut state);
    std::fs::write(path, state.to_string()).unwrap();
}

#[test]
fn key_form_confirms_every_waiting_section() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm(dir, "spec");
    confirm(dir, "design");
    set_section(dir, "ABC-1", "Design", "The design, revised.");
    git_config(dir, "user.name", "Some Person");
    let before_decisions = decisions(dir).len();
    assert_eq!(before_decisions, 2);

    let out = stapel(dir).args(["ok", "ABC-1"]).assert().success();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert_eq!(
        stderr.matches("goes into the tracked").count(),
        1,
        "{stderr}"
    );
    assert_eq!(normalize(&stdout_of(out)), golden("ok_key_form.txt"));

    let c = confirmations(dir);
    assert_eq!(c.len(), 5);
    let sections: Vec<&str> = c.iter().map(|c| c["section"].as_str().unwrap()).collect();
    assert_eq!(sections, ["spec", "design", "design", "proof", "plan"]);
    for new in &c[2..] {
        assert_eq!(new["by"], "Some Person");
        assert_eq!(new["via"], "terminal");
        assert_eq!(new["normal_form"], 1);
        assert!(new["text"].is_string(), "{new}");
        assert!(new["at"].as_str().unwrap().ends_with('Z'));
    }
    assert_eq!(c[2]["depends_on"], serde_json::json!({"spec": c[0]["hash"]}));
    assert_eq!(c[2]["text"], "The design, revised.");
    assert!(c[1].get("text").is_none(), "older text kept: {}", c[1]);
    assert_eq!(c[0]["text"], "The spec.", "the fresh spec is untouched");

    let d = decisions(dir);
    assert_eq!(d.len(), 5, "one record per section, none for spec");
    for (record, confirmation) in d[2..].iter().zip(&c[2..]) {
        assert_eq!(record["action"], "ok");
        assert_eq!(record["by"], "Some Person");
        assert_eq!(record["section"], confirmation["section"]);
        assert_eq!(record["hash"], confirmation["hash"]);
        assert_eq!(record["via"], "terminal");
    }
}

#[test]
fn change_block_for_each_kind() {
    // Scenario A: changed, dependency only, changed and dependency, no stored text.
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "line one\nline two\nline three");
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    set_section(dir, "ABC-1", "Proof", "The proof, revised.");
    edit_state(dir, |s| {
        let plan = s["confirmations"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["section"] == "plan")
            .unwrap();
        plan.as_object_mut().unwrap().remove("text");
    });
    let a = normalize(&stdout_of(stapel(dir).args(["ok", "ABC-1"]).assert().success()));

    // Scenario B: never confirmed (one line), normal form only.
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm(dir, "design");
    edit_state(dir, |s| s["confirmations"][0]["normal_form"] = 0.into());
    let b = normalize(&stdout_of(stapel(dir).args(["ok", "ABC-1"]).assert().success()));

    assert_eq!(format!("{a}--\n{b}"), golden("ok_change_blocks.txt"));
}

#[test]
fn key_form_refuses_all_when_one_fails() {
    // (mutation of ticket.md, state before the key form, reason)
    type Mutation = fn(&str) -> String;
    let cases: [(Mutation, bool, &str); 3] = [
        (
            |t| format!("{t}\n## Design\n\nagain\n"),
            false,
            "appears more than once",
        ),
        (
            |t| t.replace("## Plan\n", "## Roadmap\n"),
            false,
            "section plan (## Plan) is missing",
        ),
        // A section confirmed before and now missing is waiting.
        (
            |t| t.replace("## Plan\n", "## Roadmap\n"),
            true,
            "section plan (## Plan) is missing",
        ),
    ];
    for (mutate, confirmed_before, reason) in cases {
        let repo = repo_with_ticket();
        let dir = repo.path();
        if confirmed_before {
            for id in ["spec", "design", "proof", "plan"] {
                confirm(dir, id);
            }
        }
        let path = dir.join(".stapel/tickets/ABC-1/ticket.md");
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, mutate(&text)).unwrap();
        let before = snapshot(dir);
        stapel(dir)
            .args(["ok", "ABC-1"])
            .assert()
            .code(1)
            .stderr(predicates::str::contains(reason));
        assert_eq!(snapshot(dir), before, "{reason}: files changed");
    }

    // A missing section that others depend on: the first waiting section in order is refused.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let path = dir.join(".stapel/tickets/ABC-1/ticket.md");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("## Spec\n", "## Specification\n")).unwrap();
    let before = snapshot(dir);
    stapel(dir)
        .args(["ok", "ABC-1"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("section spec (## Spec) is missing"));
    assert_eq!(snapshot(dir), before);
}

#[test]
fn key_form_with_nothing_waiting() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    let before = snapshot(dir);
    for key in ["ABC-1", "abc-1"] {
        let out = stdout_of(stapel(dir).args(["ok", key]).assert().success());
        assert_eq!(out, "nothing to confirm: ABC-1\n");
    }
    assert_eq!(snapshot(dir), before);
}

#[test]
fn one_argument_is_section_then_key() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    // A section id, any case: the section form.
    for arg in ["spec", "SPEC"] {
        let out = normalize(&stdout_of(stapel(dir).args(["ok", arg]).assert().success()));
        assert!(
            out.ends_with("confirmed: ABC-1 spec (sha256:HASH)\n") && !out.contains("to confirm"),
            "{arg}: {out}"
        );
    }
    // A key, any case: the key form.
    let out = stdout_of(stapel(dir).args(["ok", "abc-1"]).assert().success());
    assert!(
        out.starts_with("to confirm: ABC-1 design, proof, plan\n"),
        "{out}"
    );
    let out = stdout_of(stapel(dir).args(["ok", "ABC-1"]).assert().success());
    assert_eq!(out, "nothing to confirm: ABC-1\n");

    // A generated section: today's refusal.
    stapel(dir)
        .args(["ok", "review"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("cannot be confirmed"))
        .stderr(predicates::str::contains("neither").not());

    // Neither a section nor a ticket.
    stapel(dir).args(["ok", "nope"]).assert().code(1).stderr(
        predicates::str::contains(
            "\"nope\" is neither a section nor a ticket; sections: spec, design, proof, plan; \
             open tickets: ABC-1",
        ),
    );

    // A closed ticket key: the refusal of `prepare`.
    let repo = repo_with_ticket();
    let dir = repo.path();
    edit_state(dir, |s| {
        s["closed"] = serde_json::json!({"by": "x", "at": "2026-10-02T00:00:00Z", "reason": "done"});
    });
    stapel(dir)
        .args(["ok", "ABC-1"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("was closed by x"));

    // A legacy ticket key.
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".stapel/tickets/ABC-9")).unwrap();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-9/state.json"),
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    )
    .unwrap();
    stapel(dir)
        .args(["ok", "abc-9"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("legacy"));

    // Two folders that differ only in case: the refusal of `resolve`.
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".stapel/tickets/abc-1")).unwrap();
    std::fs::write(
        dir.join(".stapel/tickets/abc-1/state.json"),
        std::fs::read(dir.join(".stapel/tickets/ABC-1/state.json")).unwrap(),
    )
    .unwrap();
    stapel(dir)
        .args(["ok", "ABC-1"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("differ only in case"));
}

#[test]
fn section_form_prints_change_block() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let out = normalize(&stdout_of(stapel(dir).args(["ok", "spec"]).assert().success()));
    assert_eq!(
        out,
        "== spec: new, 1 line\nconfirmed: ABC-1 spec (sha256:HASH)\n"
    );
    // A fresh section is confirmed again.
    let out = normalize(&stdout_of(stapel(dir).args(["ok", "spec"]).assert().success()));
    assert_eq!(
        out,
        "== spec: unchanged\nconfirmed: ABC-1 spec (sha256:HASH)\n"
    );
    assert_eq!(confirmations(dir).len(), 2);
    // A changed section prints its diff, with the key given.
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    let out = normalize(&stdout_of(
        stapel(dir).args(["ok", "ABC-1", "spec"]).assert().success(),
    ));
    assert_eq!(
        out,
        "== spec: changed since the last confirmation\n--- confirmed\n+++ current\n@@ -1 +1 @@\n\
         -The spec.\n+The spec, revised.\nconfirmed: ABC-1 spec (sha256:HASH)\n"
    );
}

#[test]
fn section_id_wins_over_a_ticket_folder() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    // A legacy ticket folder named like a section id is not an open ticket.
    std::fs::create_dir_all(dir.join(".stapel/tickets/spec")).unwrap();
    std::fs::write(
        dir.join(".stapel/tickets/spec/state.json"),
        r#"{"key":"spec","build":{"allowed":false}}"#,
    )
    .unwrap();
    for arg in ["spec", "SPEC"] {
        let out = normalize(&stdout_of(stapel(dir).args(["ok", arg]).assert().success()));
        assert_eq!(
            out,
            "== spec: ".to_string()
                + if arg == "spec" {
                    "new, 1 line"
                } else {
                    "unchanged"
                }
                + "\nconfirmed: ABC-1 spec (sha256:HASH)\n",
            "{arg}"
        );
    }
    assert_eq!(confirmations(dir).len(), 2);
}

#[test]
fn refusal_lists_only_open_tickets() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    // A legacy ticket is not listed as open.
    std::fs::create_dir_all(dir.join(".stapel/tickets/ABC-9")).unwrap();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-9/state.json"),
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    )
    .unwrap();
    stapel(dir)
        .args(["ok", "nope"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("open tickets: ABC-1\n"));
    // A closed ticket is not listed either: `none`.
    edit_state(dir, |s| {
        s["closed"] = serde_json::json!({"by": "x", "at": "2026-10-02T00:00:00Z", "reason": "done"});
    });
    stapel(dir)
        .args(["ok", "nope"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("open tickets: none\n"));
}

#[test]
fn change_block_with_two_reasons() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm(dir, "spec");
    confirm(dir, "design");
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    edit_state(dir, |s| {
        let design = s["confirmations"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["section"] == "design")
            .unwrap();
        design["normal_form"] = 0.into();
    });
    let out = normalize(&stdout_of(
        stapel(dir).args(["ok", "ABC-1", "design"]).assert().success(),
    ));
    assert_eq!(
        out,
        "== design: unchanged; confirmed under normal form 0, depends on spec, which changed\n\
         confirmed: ABC-1 design (sha256:HASH)\n"
    );
}
