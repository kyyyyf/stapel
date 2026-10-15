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
    assert_eq!(
        c[2]["depends_on"],
        serde_json::json!({"spec": c[0]["hash"]})
    );
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
    let a = normalize(&stdout_of(
        stapel(dir).args(["ok", "ABC-1"]).assert().success(),
    ));

    // Scenario B: never confirmed (one line), normal form only.
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm(dir, "design");
    edit_state(dir, |s| s["confirmations"][0]["normal_form"] = 0.into());
    let b = normalize(&stdout_of(
        stapel(dir).args(["ok", "ABC-1"]).assert().success(),
    ));

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
        .stderr(predicates::str::contains(
            "section spec (## Spec) is missing",
        ));
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
    stapel(dir)
        .args(["ok", "nope"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "\"nope\" is neither a section nor a ticket; sections: spec, design, proof, plan; \
             open tickets: ABC-1",
        ));

    // A closed ticket key: the refusal of `prepare`.
    let repo = repo_with_ticket();
    let dir = repo.path();
    edit_state(dir, |s| {
        s["closed"] =
            serde_json::json!({"by": "x", "at": "2026-10-02T00:00:00Z", "reason": "done"});
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
    let out = normalize(&stdout_of(
        stapel(dir).args(["ok", "spec"]).assert().success(),
    ));
    assert_eq!(
        out,
        "== spec: new, 1 line\nconfirmed: ABC-1 spec (sha256:HASH)\n"
    );
    // A fresh section is confirmed again.
    let out = normalize(&stdout_of(
        stapel(dir).args(["ok", "spec"]).assert().success(),
    ));
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
        s["closed"] =
            serde_json::json!({"by": "x", "at": "2026-10-02T00:00:00Z", "reason": "done"});
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
        stapel(dir)
            .args(["ok", "ABC-1", "design"])
            .assert()
            .success(),
    ));
    assert_eq!(
        out,
        "== design: unchanged; confirmed under normal form 0, depends on spec, which changed\n\
         confirmed: ABC-1 design (sha256:HASH)\n"
    );
}

#[test]
fn dialog_lists_every_waiting_section() {
    use common::{ask, grants_dir, hook_bash, read};
    const DENY: i32 = 2;
    fn short(dir: &Path, title: &str) -> String {
        let text = read(dir, ".stapel/tickets/ABC-1/ticket.md");
        let sections = stapel_core::ticket::parse(&text);
        let body = stapel_core::ticket::find(&sections, title).found().unwrap();
        stapel_core::hash::section_hash(body)[.."sha256:".len() + 12].to_string()
    }
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm(dir, "spec");
    confirm(dir, "design");
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");

    // Spec changed (+1 -1); design stale only by its dependency; proof and plan never confirmed.
    let (spec, design) = (short(dir, "Spec"), short(dir, "Design"));
    let (proof, plan) = (short(dir, "Proof"), short(dir, "Plan"));
    let expected = [
        format!("spec ({spec}…): +1 -1 lines since last confirmation"),
        format!(
            "design ({design}…): +0 -0 lines since last confirmation; depends on spec, which changed"
        ),
        format!("proof ({proof}…): new, 1 line"),
        format!("plan ({plan}…): new, 1 line"),
    ];
    for (command, program) in [
        ("stapel ok ABC-1", "stapel"),
        ("stapel ok abc-1", "stapel"),
        (
            "cargo run -q -p stapel-cli -- ok ABC-1",
            "cargo run -q -p stapel-cli --",
        ),
    ] {
        let (reason, replaced) = ask(dir, command);
        assert!(reason.contains("ABC-1"), "{reason}");
        let mut at = 0;
        for line in &expected {
            let i = reason[at..]
                .find(line.as_str())
                .unwrap_or_else(|| panic!("{line:?} not in order in {reason}"));
            at += i + line.len();
        }
        assert!(
            replaced.starts_with(&format!("{program} ok ABC-1 --grant ")),
            "{replaced}"
        );
        assert_eq!(common::token_of(&replaced).len(), 32);
    }
    assert_eq!(
        std::fs::read_dir(grants_dir(dir)).unwrap().count(),
        3,
        "one grant file per dialog"
    );

    // Modes without a dialog deny, and write no grant.
    let before = std::fs::read_dir(grants_dir(dir)).unwrap().count();
    for mode in ["bypassPermissions", "dontAsk", ""] {
        hook_bash(dir, "stapel ok ABC-1", mode)
            .code(DENY)
            .stderr(predicates::str::contains("dialog"));
    }
    assert_eq!(std::fs::read_dir(grants_dir(dir)).unwrap().count(), before);

    // An allow rule that would skip the dialog denies.
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    std::fs::write(
        dir.join(".claude/settings.local.json"),
        r#"{"permissions":{"allow":["Bash(stapel ok:*)"]}}"#,
    )
    .unwrap();
    hook_bash(dir, "stapel ok ABC-1", "auto")
        .code(DENY)
        .stderr(predicates::str::contains("allow rule"));
    assert_eq!(std::fs::read_dir(grants_dir(dir)).unwrap().count(), before);
}

fn ok_key_grant(dir: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    stapel(dir)
        .arg("ok")
        .args(args)
        .env("CLAUDECODE", "1")
        .assert()
}

#[test]
fn key_form_grant_names_every_section() {
    use common::{ask, grants_dir, token_of};
    // Each case: the dialog for the key form, a change after it, then the grant is refused.
    type Change = fn(&Path);
    let changes: [(&str, Change); 3] = [
        ("a waiting section is edited", |d| {
            set_section(d, "ABC-1", "Design", "Rewritten while the dialog was open.");
        }),
        ("a dependency is edited, so it waits too", |d| {
            set_section(d, "ABC-1", "Spec", "Rewritten while the dialog was open.");
        }),
        ("a waiting section is confirmed", |d| confirm(d, "design")),
    ];
    for (what, change) in changes {
        let repo = repo_with_ticket();
        let dir = repo.path();
        confirm(dir, "spec");
        let (_, replaced) = ask(dir, "stapel ok ABC-1");
        change(dir);
        let before = (confirmations(dir).len(), decisions(dir).len());
        ok_key_grant(dir, &["ABC-1", "--grant", &token_of(&replaced)])
            .code(1)
            .stderr(predicates::str::contains("the grant does not match"));
        assert_eq!(
            (confirmations(dir).len(), decisions(dir).len()),
            before,
            "{what}"
        );
    }

    // A grant of the section form does not confirm the key form, nor the reverse.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let (_, section_grant) = ask(dir, "stapel ok spec");
    ok_key_grant(dir, &["ABC-1", "--grant", &token_of(&section_grant)])
        .code(1)
        .stderr(predicates::str::contains("the grant does not match"));
    let (_, key_grant) = ask(dir, "stapel ok ABC-1");
    ok_key_grant(dir, &["ABC-1", "spec", "--grant", &token_of(&key_grant)])
        .code(1)
        .stderr(predicates::str::contains("the grant does not match"));
    assert!(confirmations(dir).is_empty());

    // An expired grant.
    let (_, replaced) = ask(dir, "stapel ok ABC-1");
    for entry in std::fs::read_dir(grants_dir(dir)).unwrap() {
        std::fs::write(entry.unwrap().path(), r#"{"expires_at":1}"#).unwrap();
    }
    ok_key_grant(dir, &["ABC-1", "--grant", &token_of(&replaced)])
        .code(1)
        .stderr(predicates::str::contains("expired"));
    assert!(confirmations(dir).is_empty());
    assert!(decisions(dir).is_empty());
}

#[test]
fn key_form_agent_shell_refused() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let before = snapshot(dir);
    for (var, key) in [("CLAUDECODE", "ABC-1"), ("CLAUDE_CODE_ENTRYPOINT", "abc-1")] {
        stapel(dir)
            .args(["ok", key])
            .env(var, "1")
            .assert()
            .code(1)
            .stderr(predicates::str::contains("only a person confirms"))
            .stderr(predicates::str::contains("stapel ok KEY"));
    }
    assert_eq!(snapshot(dir), before);
}

#[test]
fn key_form_grant_confirms_once() {
    use common::{ask, grants_dir, token_of};
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm(dir, "spec");
    confirm(dir, "design");
    set_section(dir, "ABC-1", "Design", "The design, revised.");
    let (_, replaced) = ask(dir, "stapel ok ABC-1");
    let token = token_of(&replaced);

    let out = ok_key_grant(dir, &["ABC-1", "--grant", &token]).success();
    assert_eq!(normalize(&stdout_of(out)), golden("ok_key_form.txt"));
    let c = confirmations(dir);
    assert_eq!(c.len(), 5);
    for new in &c[2..] {
        assert_eq!(new["via"], "grant");
    }
    let d = decisions(dir);
    assert_eq!(d.len(), 5);
    for record in &d[2..] {
        assert_eq!(record["via"], "grant");
    }
    assert_eq!(std::fs::read_dir(grants_dir(dir)).unwrap().count(), 0);

    // The same token again: nothing waits, and the grant is gone.
    ok_key_grant(dir, &["ABC-1", "--grant", &token])
        .code(1)
        .stderr(predicates::str::contains("the grant does not match"));
    assert_eq!(confirmations(dir).len(), 5);
    assert_eq!(decisions(dir).len(), 5);
}

#[test]
fn guard_denies_when_nothing_waits() {
    use common::{grants_dir, hook_bash};
    const DENY: i32 = 2;
    let repo = repo_with_ticket();
    let dir = repo.path();
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    for command in ["stapel ok ABC-1", "stapel ok abc-1"] {
        hook_bash(dir, command, "auto")
            .code(DENY)
            .stderr(predicates::str::contains("nothing to confirm: ABC-1"));
    }
    assert!(!grants_dir(dir).exists() || std::fs::read_dir(grants_dir(dir)).unwrap().count() == 0);
}

#[test]
fn dialog_shows_every_stale_reason() {
    use common::ask;
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm(dir, "spec");
    confirm(dir, "design");
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    set_section(dir, "ABC-1", "Design", "The design, revised.");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("+1 -1 lines since last confirmation; depends on spec, which changed"),
        "{reason}"
    );
}

#[test]
fn key_form_grant_binds_a_generated_dependency() {
    use common::{ask, token_of};
    let repo = repo_with_ticket();
    let dir = repo.path();
    let path = dir.join(".stapel/stapel.toml");
    let toml = std::fs::read_to_string(&path).unwrap();
    let plan = "id = \"plan\"\ntitle = \"Plan\"\nowner = \"engineer\"\ndepends_on = [\"spec\", \"design\"]";
    assert!(toml.contains(plan));
    std::fs::write(
        &path,
        toml.replace(
            plan,
            &plan.replace("\"design\"]", "\"design\", \"review\"]"),
        ),
    )
    .unwrap();
    let (reason, replaced) = ask(dir, "stapel ok ABC-1");
    assert!(reason.contains("plan ("), "{reason}");
    set_section(dir, "ABC-1", "Review", "A review, edited after the dialog.");
    let before = (confirmations(dir).len(), decisions(dir).len());
    ok_key_grant(dir, &["ABC-1", "--grant", &token_of(&replaced)])
        .code(1)
        .stderr(predicates::str::contains("the grant does not match"));
    assert_eq!(
        (confirmations(dir).len(), decisions(dir).len()),
        before,
        "nothing is recorded"
    );
}

#[test]
fn new_with_reasons_in_terminal_and_dialog() {
    use common::ask;
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Plan", "line one\nline two\nline three");
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    set_section(dir, "ABC-1", "Spec", "The spec, revised.");
    edit_state(dir, |s| {
        let plan = s["confirmations"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["section"] == "plan")
            .unwrap();
        plan.as_object_mut().unwrap().remove("text");
    });
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("new, 3 lines; depends on spec, which changed"),
        "{reason}"
    );
    let out = stdout_of(stapel(dir).args(["ok", "ABC-1"]).assert().success());
    assert!(
        out.contains("== plan: new, 3 lines; depends on spec, which changed\n"),
        "{out}"
    );
}

#[test]
fn new_never_shows_the_text_change() {
    use common::ask;
    for with_dependency in [false, true] {
        let repo = repo_with_ticket();
        let dir = repo.path();
        set_section(dir, "ABC-1", "Plan", "only line");
        for id in ["spec", "design", "proof", "plan"] {
            confirm(dir, id);
        }
        edit_state(dir, |s| {
            let plan = s["confirmations"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|c| c["section"] == "plan")
                .unwrap();
            plan.as_object_mut().unwrap().remove("text");
        });
        set_section(dir, "ABC-1", "Plan", "changed line");
        if with_dependency {
            set_section(dir, "ABC-1", "Spec", "The spec, revised.");
        }
        let (reason, _) = ask(dir, "stapel ok ABC-1");
        let out = stdout_of(stapel(dir).args(["ok", "ABC-1"]).assert().success());
        let plan_line = out.lines().find(|l| l.starts_with("== plan:")).unwrap();
        assert!(!plan_line.contains("changed since"), "{out}");
        let plan_part = &reason[reason.find("plan (").unwrap()..];
        assert!(!plan_part.contains("changed since"), "{reason}");
        if with_dependency {
            assert!(
                reason.contains("new, 1 line; depends on spec, which changed"),
                "{reason}"
            );
            assert!(
                out.contains("== plan: new, 1 line; depends on spec, which changed\n"),
                "{out}"
            );
        } else {
            assert!(reason.contains("new, 1 line; runs:"), "{reason}");
            assert!(!reason.contains("depends on"), "{reason}");
            assert!(out.contains("== plan: new, 1 line\n"), "{out}");
        }
    }
}

#[test]
fn key_form_blank_user_name_refused() {
    for waiting in [true, false] {
        let repo = repo_with_ticket();
        let dir = repo.path();
        if !waiting {
            for id in ["spec", "design", "proof", "plan"] {
                confirm(dir, id);
            }
        }
        git_config(dir, "user.name", "   ");
        let before = snapshot(dir);
        stapel(dir)
            .args(["ok", "ABC-1"])
            .assert()
            .code(1)
            .stderr(predicates::str::contains("user.name"));
        assert_eq!(snapshot(dir), before, "waiting: {waiting}");
    }
}

#[test]
fn unresolved_argument_in_agent_shell_refused() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let before = snapshot(dir);
    stapel(dir)
        .args(["ok", "nope"])
        .env("CLAUDECODE", "1")
        .assert()
        .code(1)
        .stderr(predicates::str::contains("only a person confirms"))
        .stderr(predicates::str::contains("neither a section nor a ticket").not());
    assert_eq!(snapshot(dir), before);
}

#[test]
fn section_form_grant_differs_by_action_only() {
    use common::{ask, token_of};
    let repo = repo_with_ticket();
    let dir = repo.path();
    for id in ["spec", "design", "proof", "plan"] {
        confirm(dir, id);
    }
    // Only spec waits: its confirmation is gone, no hash changed.
    edit_state(dir, |s| {
        s["confirmations"]
            .as_array_mut()
            .unwrap()
            .retain(|c| c["section"] != "spec");
    });
    let (_, section_grant) = ask(dir, "stapel ok spec");
    let (reason, key_grant) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("confirm 1 waiting section of ABC-1"),
        "{reason}"
    );
    let before = (confirmations(dir).len(), decisions(dir).len());
    ok_key_grant(dir, &["ABC-1", "--grant", &token_of(&section_grant)])
        .code(1)
        .stderr(predicates::str::contains("the grant does not match"));
    ok_key_grant(dir, &["ABC-1", "spec", "--grant", &token_of(&key_grant)])
        .code(1)
        .stderr(predicates::str::contains("the grant does not match"));
    assert_eq!((confirmations(dir).len(), decisions(dir).len()), before);
}

#[test]
fn dialog_counts_added_and_removed_apart() {
    use common::ask;
    let repo = repo_with_ticket();
    let dir = repo.path();
    set_section(dir, "ABC-1", "Spec", "keep\nold");
    confirm(dir, "spec");
    set_section(dir, "ABC-1", "Spec", "keep\nnew one\nnew two");
    let (reason, _) = ask(dir, "stapel ok ABC-1");
    assert!(
        reason.contains("+2 -1 lines since last confirmation"),
        "{reason}"
    );
}
