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

#[test]
fn import_needs_a_user_name() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &format!("{}\n", base()));
    let text = "stapel: git config user.name is not set; set it to the name confirmations are recorded under";

    for value in [None, Some(""), Some("   ")] {
        match value {
            None => {
                let status = std::process::Command::new("git")
                    .args(["config", "--unset", "user.name"])
                    .current_dir(dir)
                    .status()
                    .unwrap();
                assert!(status.success());
            }
            Some(v) => common::git_config(dir, "user.name", v),
        }
        let before = common::snapshot(dir);
        let out = stapel(dir)
            .args(["findings", "import", "in.jsonl"])
            .assert()
            .code(1)
            .get_output()
            .clone();
        assert!(out.stdout.is_empty());
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(err.contains(text), "{value:?}: {err}");
        assert_eq!(common::snapshot(dir), before, "{value:?}");
        assert!(!dir.join(TARGET).exists());
    }
}

#[test]
fn guard_allow_output_is_empty() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "f.jsonl", &format!("{}\n", base()));
    let command = "stapel findings import f.jsonl ABC-1";

    for mode in ["default", "bypassPermissions", ""] {
        let out = common::hook_bash(dir, command, mode)
            .success()
            .get_output()
            .clone();
        assert!(out.stdout.is_empty(), "{mode}: {:?}", out.stdout);
        assert!(out.stderr.is_empty(), "{mode}: {:?}", out.stderr);
    }

    // No permission mode field at all.
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
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
}

#[test]
fn importer_escapes_the_user_name() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let name = "a \"q\" é\\";
    common::git_config(dir, "user.name", name);
    put(dir, "in.jsonl", &format!("{}\n", base()));
    stapel(dir)
        .env("CLAUDECODE", "1")
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success();
    let text = common::read(dir, TARGET);
    let line = text.lines().last().unwrap();
    let v: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(v["imported_by"]["user"], name, "{line}");
    assert_eq!(v["imported_by"]["by"], "agent", "{line}");
}

#[test]
fn ledger_kind_words() {
    let repo = repo_with_ticket();
    let dir = repo.path();

    // A wrong word is denied by the hook with the reason of the command, in the modes with a dialog.
    for mode in ["default", "auto"] {
        let out = common::hook_bash(dir, "stapel ledger add --kind Decision --note x", mode)
            .get_output()
            .clone();
        assert!(!out.status.success(), "{mode}");
        assert!(out.stdout.is_empty(), "{mode}: {:?}", out.stdout);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("--kind must be one of finding, catches, decision"),
            "{mode}: {err}"
        );
    }

    // Two `--kind` words: the dialog stays, also in modes `default` and `auto`.
    for mode in ["default", "auto"] {
        let out = common::hook_bash(
            dir,
            "stapel ledger add --kind finding --kind decision --note x",
            mode,
        )
        .success()
        .get_output()
        .clone();
        let v: Value = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("{mode}: {e}: {:?}", out.stdout));
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecision"], "ask",
            "{mode}: {v}"
        );
    }
    assert!(ledger_text(dir).is_none());
}

#[test]
fn ledger_importer_for_catches() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let name = "a \"q\" \\";
    common::git_config(dir, "user.name", name);
    stapel(dir)
        .env("CLAUDECODE", "1")
        .args(["ledger", "add", "--kind", "catches", "--note", "x"])
        .assert()
        .success()
        .stdout("recorded: catches for ABC-1\n");
    let text = ledger_text(dir).unwrap();
    let line = text.lines().last().unwrap();
    let v: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(v["imported_by"]["by"], "agent", "{line}");
    assert_eq!(v["imported_by"]["user"], name, "{line}");

    // No user name: the AC-3 text, exit 1, nothing more appended.
    unset_user_name(dir);
    stapel(dir)
        .env("CLAUDECODE", "1")
        .args(["ledger", "add", "--kind", "catches", "--note", "y"])
        .assert()
        .code(1)
        .stdout("")
        .stderr("stapel: git config user.name is not set; set it to the name confirmations are recorded under\n");
    assert_eq!(ledger_text(dir).unwrap(), text);
}

/// Imports `record` as the only line of `in.jsonl`; asserts exit 2, nothing appended and the exact
/// stderr.
fn import_refused(dir: &Path, record: &Value, reason: &str) {
    put(dir, "in.jsonl", &format!("{record}\n"));
    let before = common::snapshot(dir);
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(format!("stapel: in.jsonl:1: {reason}\n"));
    assert_eq!(common::snapshot(dir), before, "{record}");
    assert!(!dir.join(TARGET).exists());
}

#[test]
fn import_checks_the_fate() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let reason = "fate: must be one of open, patch, bad_plan, intent_gap, defer, no-change";
    for fate in [json!("fixed"), json!("Open"), json!(""), json!(5)] {
        let mut v = base();
        v["fate"] = fate;
        import_refused(dir, &v, reason);
    }
    let mut v = base();
    v.as_object_mut().unwrap().remove("fate");
    import_refused(dir, &v, "fate: missing");

    let mut lines = String::new();
    for (i, fate) in ["open", "patch", "bad_plan", "intent_gap", "defer", "no-change"]
        .iter()
        .enumerate()
    {
        let mut v = base();
        v["id"] = json!(format!("F-{i}"));
        v["fate"] = json!(fate);
        lines.push_str(&format!("{v}\n"));
    }
    put(dir, "in.jsonl", &lines);
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 6 findings into ABC-1\n");
}

#[test]
fn import_checks_the_triage_fields() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let set = |field: &str, value: Value| {
        let mut v = base();
        v[field] = value;
        v
    };
    import_refused(
        dir,
        &set("verdict", json!("wrong")),
        "verdict: must be one of high, medium, low, false, maybe-false",
    );
    for verdict in ["false", "maybe-false"] {
        import_refused(
            dir,
            &set("verdict", json!(verdict)),
            &format!("evidence: missing; the verdict {verdict} needs it"),
        );
    }
    import_refused(
        dir,
        &set("evidence", json!("")),
        "evidence: must be a non-empty string",
    );
    import_refused(
        dir,
        &set("note", json!(3)),
        "note: must be a non-empty string",
    );
    import_refused(
        dir,
        &set("root", json!("a b")),
        "root: must be 1 to 64 characters with no whitespace or control character",
    );
    import_refused(
        dir,
        &set("root", json!("")),
        "root: must be 1 to 64 characters with no whitespace or control character",
    );
    import_refused(
        dir,
        &set("kind", json!("triage")),
        "kind: a finding has no kind; triage records go through stapel findings triage",
    );

    let mut ok = base();
    ok["verdict"] = json!("false");
    ok["evidence"] = json!("the code does it");
    ok["root"] = json!("R-1");
    ok["note"] = json!("n");
    put(dir, "in.jsonl", &format!("{ok}\n"));
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
}
