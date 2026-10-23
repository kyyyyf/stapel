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
