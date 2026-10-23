//! STP-23: triage of findings. Step 1: `stapel findings import` needs no dialog.

mod common;

use common::{repo_with_ticket, stapel};
use serde_json::{Value, json};
use std::path::Path;

const TARGET: &str = ".stapel/tickets/ABC-1/findings.jsonl";

fn base() -> Value {
    json!({
        "id": "F-1",
        "ticket": "ABC-1",
        "stage": "code-review",
        "round": 1,
        "role": "reviewer",
        "severity": "HIGH",
        "promise": "inside",
        "claim": "a claim",
        "catchable_at": "code",
        "fate": "patch",
    })
}

fn put(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A line with the importer insert before its last `}`, trailing whitespace removed.
fn with_insert(line: &str, importer: &str) -> String {
    let t = line.trim_end();
    let body = t.strip_suffix('}').unwrap();
    format!("{body},\"imported_by\":{importer}}}")
}

#[test]
fn import_in_an_agent_shell() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let importer = r#"{"by":"agent","user":"test-user"}"#;

    // A line of 64 KiB less 10 bytes grows past the limit with the insert: nothing is appended.
    let mut long = base();
    long["id"] = json!("F-9");
    let short_len = long.to_string().len();
    long["claim"] = json!("x".repeat(65536 - 10 - short_len + "a claim".len()));
    let long_line = long.to_string();
    assert_eq!(long_line.len(), 65536 - 10);
    put(dir, "long.jsonl", &format!("{long_line}\n"));
    let out = stapel(dir)
        .env("CLAUDECODE", "1")
        .args(["findings", "import", "long.jsonl"])
        .assert()
        .code(2)
        .get_output()
        .clone();
    assert!(out.stdout.is_empty());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.contains("stapel: long.jsonl:1: longer than 64 KiB"),
        "{err}"
    );
    assert!(!dir.join(TARGET).exists());

    // `--grant` is not an option of the command.
    put(dir, "in.jsonl", &format!("{}\n", base()));
    stapel(dir)
        .env("CLAUDECODE", "1")
        .args(["findings", "import", "in.jsonl", "--grant", "x"])
        .assert()
        .code(2);
    assert!(!dir.join(TARGET).exists());

    // Two lines, odd spacing and key order kept, in an agent shell with no grant.
    let one = r#"{"id": "F-1", "note": "n", "ticket": "ABC-1", "stage": "spec-review", "round": "2", "role": "external", "severity": "LOW", "promise": "outside", "claim": "first", "catchable_at": "spec", "fate": "patch"  }"#;
    let two = {
        let mut v = base();
        v["id"] = json!("F-2");
        v.to_string()
    };
    put(dir, "in.jsonl", &format!("{one}\n\n{two}   \n"));
    stapel(dir)
        .env("CLAUDECODE", "1")
        .args(["findings", "import", "in.jsonl", "ABC-1"])
        .assert()
        .success()
        .stdout("imported: 2 findings into ABC-1\n");
    let expect = format!(
        "{}\n{}\n",
        with_insert(one, importer),
        with_insert(&two, importer)
    );
    assert_eq!(common::read(dir, TARGET), expect);
}

#[test]
fn guard_allows_import_without_a_dialog() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "f.jsonl", &format!("{}\n", base()));
    let command = "stapel findings import f.jsonl ABC-1";

    for mode in ["default", "bypassPermissions", ""] {
        let out = common::hook_bash(dir, command, mode)
            .success()
            .get_output()
            .clone();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(!text.contains("ask"), "{mode}: {text}");
    }

    // No permission mode at all.
    let payload = json!({
        "tool_name": "Bash",
        "cwd": dir,
        "tool_input": { "command": command, "description": "test" },
    });
    let out = stapel(dir)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(!String::from_utf8_lossy(&out.stdout).contains("ask"));

    // An allow rule changes nothing.
    put(
        dir,
        ".claude/settings.local.json",
        r#"{"permissions":{"allow":["Bash(stapel findings:*)"]}}"#,
    );
    let out = common::hook_bash(dir, command, "default")
        .success()
        .get_output()
        .clone();
    assert!(!String::from_utf8_lossy(&out.stdout).contains("ask"));

    assert!(!common::grants_dir(dir).exists());
}

#[test]
fn importer_recorded() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &format!("{}\n", base()));

    // Neither agent variable: a person.
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success();
    let text = common::read(dir, TARGET);
    assert!(
        text.trim_end()
            .ends_with(r#","imported_by":{"by":"person","user":"test-user"}}"#),
        "{text}"
    );

    // An empty agent variable still means an agent.
    let mut second = base();
    second["id"] = json!("F-2");
    put(dir, "in2.jsonl", &format!("{second}\n"));
    stapel(dir)
        .env("CLAUDE_CODE_ENTRYPOINT", "")
        .args(["findings", "import", "in2.jsonl"])
        .assert()
        .success();
    let text = common::read(dir, TARGET);
    let last = text.lines().last().unwrap();
    assert!(
        last.ends_with(r#","imported_by":{"by":"agent","user":"test-user"}}"#),
        "{last}"
    );

    // A record that brings `imported_by` is refused, and nothing is appended.
    let before = common::read(dir, TARGET);
    let mut third = base();
    third["id"] = json!("F-3");
    let ok = third.to_string();
    third["id"] = json!("F-4");
    third["imported_by"] = json!({"by": "person", "user": "someone"});
    put(dir, "in3.jsonl", &format!("{ok}\n{third}\n"));
    let out = stapel(dir)
        .args(["findings", "import", "in3.jsonl"])
        .assert()
        .code(2)
        .get_output()
        .clone();
    assert!(out.stdout.is_empty());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.contains("stapel: in3.jsonl:2: imported_by: written by stapel, not by the file"),
        "{err}"
    );
    assert_eq!(common::read(dir, TARGET), before);
}

const LEDGER: &str = ".stapel/process-ledger.jsonl";

const DECISION_REFUSAL: &str = "stapel: only a person writes ledger decisions: this shell belongs to an agent (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); run the command as one plain command so that Claude Code shows the permission dialog, or, from a Zed task started from a Claude Code shell, run it with env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT\n";

fn ledger_text(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(LEDGER)).ok()
}

fn unset_user_name(dir: &Path) {
    let status = std::process::Command::new("git")
        .args(["config", "--unset", "user.name"])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn ledger_records_without_a_dialog() {
    let repo = repo_with_ticket();
    let dir = repo.path();

    // The guard allows both kinds in every permission mode, writes no grant.
    for command in [
        "stapel ledger add --kind finding --note x",
        "stapel ledger add --kind=catches --note x",
        "stapel ledger add --note x --kind finding ABC-1",
    ] {
        for mode in ["default", "bypassPermissions", "dontAsk", "auto", ""] {
            let out = common::hook_bash(dir, command, mode)
                .success()
                .get_output()
                .clone();
            let text = String::from_utf8_lossy(&out.stdout);
            assert!(text.is_empty(), "{command} {mode}: {text}");
        }
    }
    assert!(!common::grants_dir(dir).exists());

    // In an agent shell without a grant the command appends, `imported_by` last.
    stapel(dir)
        .env("CLAUDECODE", "1")
        .args(["ledger", "add", "--kind", "finding", "--note", "first"])
        .assert()
        .success()
        .stdout("recorded: finding for ABC-1\n");
    stapel(dir)
        .env("CLAUDE_CODE_ENTRYPOINT", "cli")
        .args(["ledger", "add", "--kind=catches", "--note=second"])
        .assert()
        .success()
        .stdout("recorded: catches for ABC-1\n");
    stapel(dir)
        .args(["ledger", "add", "--kind", "finding", "--note", "third"])
        .assert()
        .success();
    let text = ledger_text(dir).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    for (line, kind, note, by) in [
        (lines[0], "finding", "first", "agent"),
        (lines[1], "catches", "second", "agent"),
        (lines[2], "finding", "third", "person"),
    ] {
        let tail = format!(
            r#""kind":"{kind}","ticket":"ABC-1","note":"{note}","imported_by":{{"by":"{by}","user":"test-user"}}}}"#
        );
        assert!(line.ends_with(&tail), "{line}");
    }
}

#[test]
fn ledger_decision_keeps_its_dialog() {
    let repo = repo_with_ticket();
    let dir = repo.path();

    // One plain `decision` asks; the same with another case or a second `--kind` is not allowed.
    for command in [
        "stapel ledger add --kind decision --note x",
        "stapel ledger add --kind=decision --note x",
        "stapel ledger add --kind finding --kind decision --note x",
    ] {
        let (reason, _) = common::ask(dir, command);
        assert!(reason.contains("add a ledger record"), "{command}: {reason}");
    }
    for mode in ["default", "bypassPermissions", "dontAsk", ""] {
        let out = common::hook_bash(dir, "stapel ledger add --kind Decision --note x", mode)
            .get_output()
            .clone();
        assert!(!out.stdout.is_empty() || !out.status.success(), "{mode}");
    }

    // In an agent shell without a grant: the refusal of decisions, nothing written.
    for var in ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"] {
        stapel(dir)
            .env(var, "1")
            .args(["ledger", "add", "--kind", "decision", "--note", "x"])
            .assert()
            .code(1)
            .stdout("")
            .stderr(DECISION_REFUSAL);
    }
    assert!(ledger_text(dir).is_none());

    // Through the grant: no importer, and `user.name` is not read.
    unset_user_name(dir);
    let (_, replaced) = common::ask(dir, "stapel ledger add --kind decision --note x");
    let token = common::token_of(&replaced);
    stapel(dir)
        .env("CLAUDECODE", "1")
        .args([
            "ledger", "add", "--kind", "decision", "--note", "x", "ABC-1", "--grant", &token,
        ])
        .assert()
        .success()
        .stdout("recorded: decision for ABC-1\n");
    let text = ledger_text(dir).unwrap();
    assert!(!text.contains("imported_by"), "{text}");
    assert!(
        text.trim_end()
            .ends_with(r#""kind":"decision","ticket":"ABC-1","note":"x"}"#),
        "{text}"
    );
}

#[test]
fn ledger_needs_a_user_name() {
    let text = "stapel: git config user.name is not set; set it to the name confirmations are recorded under\n";
    let repo = repo_with_ticket();
    let dir = repo.path();
    let run = || {
        stapel(dir)
            .args(["ledger", "add", "--kind", "finding", "--note", "x"])
            .assert()
            .code(1)
            .stdout("")
            .stderr(text)
    };

    unset_user_name(dir);
    run();
    assert!(ledger_text(dir).is_none());

    common::git_config(dir, "user.name", "  ");
    run();
    assert!(ledger_text(dir).is_none());
}
