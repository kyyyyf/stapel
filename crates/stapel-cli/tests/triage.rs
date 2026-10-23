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

#[test]
fn import_root_and_type_bounds() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let set = |field: &str, value: Value| {
        let mut v = base();
        v[field] = value;
        v
    };
    let root = "must be 1 to 64 characters with no whitespace or control character";
    for bad in ["a".repeat(65), "a\tb".to_string(), "a\nb".to_string()] {
        import_refused(dir, &set("root", json!(bad)), &format!("root: {root}"));
    }
    let verdict = "verdict: must be one of high, medium, low, false, maybe-false";
    import_refused(dir, &set("verdict", json!(5)), verdict);
    import_refused(dir, &set("verdict", Value::Null), verdict);
    let mut v = set("verdict", json!("false"));
    v["evidence"] = json!(5);
    import_refused(dir, &v, "evidence: must be a non-empty string");
    let kind = "kind: a finding has no kind; triage records go through stapel findings triage";
    import_refused(dir, &set("kind", Value::Null), kind);
    import_refused(dir, &set("kind", json!("")), kind);

    put(dir, "in.jsonl", &format!("{}\n", set("root", json!("a".repeat(64)))));
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
}

// ---- Step 3: stapel findings triage ----

/// Writes `records` as the lines of the ticket's findings.jsonl.
fn seed(dir: &Path, records: &[Value]) {
    let text: String = records.iter().map(|r| format!("{r}\n")).collect();
    put(dir, TARGET, &text);
}

fn finding(id: &str) -> Value {
    let mut v = base();
    v["id"] = json!(id);
    v
}

#[test]
fn triage_appends_records() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let importer = r#"{"by":"agent","user":"test-user"}"#;
    seed(dir, &[finding("F-1"), finding("F-2")]);
    let before = common::read(dir, TARGET);

    let one =
        r#"{ "id": "F-1", "ticket": "ABC-1", "verdict": "high", "root": "R-1", "fate": "patch" }"#;
    let two = r#"{"id":"F-2","ticket":"ABC-1","fate":"no-change","verdict":"false","evidence":"the code does it","note":"n"}   "#;
    put(dir, "t.jsonl", &format!("{one}\n\n{two}\n"));
    stapel(dir)
        .env("CLAUDECODE", "1")
        .args(["findings", "triage", "t.jsonl", "ABC-1"])
        .assert()
        .success()
        .stdout("triaged: 2 findings of ABC-1\n");
    let expect = |line: &str| {
        let t = line.trim_end();
        let rest = t.strip_prefix('{').unwrap().strip_suffix('}').unwrap();
        format!("{{\"kind\":\"triage\",{rest},\"imported_by\":{importer}}}\n")
    };
    assert_eq!(
        common::read(dir, TARGET),
        format!("{before}{}{}", expect(one), expect(two))
    );

    // Without KEY: the one open ticket; a person's shell records `person`.
    put(
        dir,
        "t2.jsonl",
        r#"{"id":"F-1","ticket":"ABC-1","fate":"defer"}
"#,
    );
    stapel(dir)
        .args(["findings", "triage", "t2.jsonl"])
        .assert()
        .success()
        .stdout("triaged: 1 findings of ABC-1\n");
    let text = common::read(dir, TARGET);
    assert!(
        text.ends_with(
            "{\"kind\":\"triage\",\"id\":\"F-1\",\"ticket\":\"ABC-1\",\"fate\":\"defer\",\"imported_by\":{\"by\":\"person\",\"user\":\"test-user\"}}\n"
        ),
        "{text}"
    );
}

#[test]
fn triage_refusals() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut outside = finding("F-3");
    outside["promise"] = json!("outside");
    outside["fate"] = json!("open");
    let mut late = finding("F-4");
    late["round"] = json!("2");
    late["fate"] = json!("open");
    let mut gap = finding("F-5");
    gap["fate"] = json!("intent_gap");
    let mut inside_low = finding("F-6");
    inside_low["severity"] = json!("LOW");
    inside_low["fate"] = json!("open");
    let mut triage_only = json!({"kind": "triage", "id": "F-7", "ticket": "ABC-1", "fate": "open"});
    triage_only["imported_by"] = json!({"by": "person", "user": "x"});
    seed(
        dir,
        &[
            finding("F-1"),
            finding("F-2"),
            outside,
            late,
            gap,
            inside_low,
            triage_only,
        ],
    );

    let refused = |text: &str, reason: &str| {
        put(dir, "t.jsonl", text);
        let before = common::snapshot(dir);
        stapel(dir)
            .args(["findings", "triage", "t.jsonl"])
            .assert()
            .code(2)
            .stdout("")
            .stderr(format!("stapel: t.jsonl:{reason}\n"));
        assert_eq!(common::snapshot(dir), before, "{text}");
    };
    let line = |fields: &[(&str, Value)]| {
        let mut v = json!({"id": "F-1", "ticket": "ABC-1", "fate": "patch"});
        for (k, x) in fields {
            if x.is_null() {
                v.as_object_mut().unwrap().remove(*k);
            } else {
                v[*k] = x.clone();
            }
        }
        format!("{v}\n")
    };

    refused(&line(&[("id", Value::Null)]), "1: id: missing");
    refused(&line(&[("ticket", Value::Null)]), "1: ticket: missing");
    refused(&line(&[("fate", Value::Null)]), "1: fate: missing");
    refused(
        &line(&[("fate", json!("fixed"))]),
        "1: fate: must be one of open, patch, bad_plan, intent_gap, defer, no-change",
    );
    refused(
        &line(&[("ticket", json!("XYZ-2"))]),
        "1: ticket: must be ABC-1",
    );
    refused(
        &line(&[("severity", json!("LOW"))]),
        "1: severity: not a triage field",
    );
    refused(
        &line(&[("kind", json!("triage"))]),
        "1: kind: not a triage field",
    );
    refused(
        &line(&[("verdict", json!("false"))]),
        "1: evidence: missing; the verdict false needs it",
    );
    refused(
        &line(&[("root", json!("a b"))]),
        "1: root: must be 1 to 64 characters with no whitespace or control character",
    );
    refused(
        &line(&[("id", json!("F-9"))]),
        "1: id F-9 is not a finding in .stapel/tickets/ABC-1/findings.jsonl",
    );
    refused(
        &line(&[("id", json!("F-7"))]),
        "1: id F-7 is not a finding in .stapel/tickets/ABC-1/findings.jsonl",
    );
    refused(
        &format!("{}{}", line(&[]), line(&[("fate", json!("defer"))])),
        "2: id F-1 repeats line 1",
    );
    refused(
        &format!("{}{}", line(&[]), line(&[("id", json!("F-9"))])),
        "2: id F-9 is not a finding in .stapel/tickets/ABC-1/findings.jsonl",
    );
    refused("\n  \n", " no records");

    // A finding for the person needs the person's words.
    let note = "note: missing; a finding for the person needs the person's answer";
    for id in ["F-3", "F-4", "F-5"] {
        refused(&line(&[("id", json!(id))]), &format!("1: {note}"));
    }
    // A finding that is not the person's, or one with a note, is accepted.
    put(
        dir,
        "ok.jsonl",
        &format!(
            "{}{}{}",
            line(&[("id", json!("F-6"))]),
            line(&[("id", json!("F-3")), ("note", json!("the person said so"))]),
            line(&[("id", json!("F-2"))])
        ),
    );
    stapel(dir)
        .args(["findings", "triage", "ok.jsonl"])
        .assert()
        .success()
        .stdout("triaged: 3 findings of ABC-1\n");

    // More than 100 records.
    let names: Vec<String> = (0..101).map(|i| format!("M-{i}")).collect();
    let all: Vec<Value> = names.iter().map(|id| finding(id)).collect();
    seed(dir, &all);
    let many: String = names
        .iter()
        .map(|id| line(&[("id", json!(id))]))
        .collect();
    put(dir, "many.jsonl", &many);
    let before = common::snapshot(dir);
    stapel(dir)
        .args(["findings", "triage", "many.jsonl"])
        .assert()
        .code(2)
        .stdout("")
        .stderr("stapel: many.jsonl:101: more than 100 records; split the file\n");
    assert_eq!(common::snapshot(dir), before);

    // No findings.jsonl: every id is not a finding.
    std::fs::remove_file(dir.join(TARGET)).unwrap();
    refused(
        &line(&[]),
        "1: id F-1 is not a finding in .stapel/tickets/ABC-1/findings.jsonl",
    );

    // No user name: exit 1, nothing appended.
    seed(dir, &[finding("F-1")]);
    unset_user_name(dir);
    put(dir, "t.jsonl", &line(&[]));
    let before = common::snapshot(dir);
    stapel(dir)
        .args(["findings", "triage", "t.jsonl"])
        .assert()
        .code(1)
        .stdout("")
        .stderr("stapel: git config user.name is not set; set it to the name confirmations are recorded under\n");
    assert_eq!(common::snapshot(dir), before);
}

#[test]
fn triage_on_a_closed_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    seed(dir, &[finding("F-1")]);
    stapel(dir)
        .args(["close", "--reason", "done"])
        .assert()
        .success();
    put(
        dir,
        "t.jsonl",
        r#"{"id":"F-1","ticket":"ABC-1","fate":"defer"}
"#,
    );
    stapel(dir)
        .args(["findings", "triage", "t.jsonl", "ABC-1"])
        .assert()
        .success()
        .stdout("triaged: 1 findings of ABC-1\n");
    let text = common::read(dir, TARGET);
    assert_eq!(text.lines().count(), 2, "{text}");
    assert!(
        text.lines()
            .last()
            .unwrap()
            .starts_with(r#"{"kind":"triage","#)
    );

    // A legacy ticket: the refusal of `findings import`.
    put(
        dir,
        ".stapel/tickets/ABC-9/state.json",
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    );
    put(
        dir,
        "t9.jsonl",
        r#"{"id":"F-1","ticket":"ABC-9","fate":"defer"}
"#,
    );
    let before = common::snapshot(dir);
    stapel(dir)
        .args(["findings", "triage", "t9.jsonl", "ABC-9"])
        .assert()
        .code(2)
        .stdout("")
        .stderr(
            "stapel: ticket ABC-9 has a legacy (STP-1) state.json; its journal is kept by hand\n",
        );
    assert_eq!(common::snapshot(dir), before);
}

// ---- Step 3c: coverage after the drift review ----

#[test]
fn triage_note_reads_the_current_state() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let by = json!({"by": "person", "user": "x"});
    let mut f3 = finding("F-3");
    f3["promise"] = json!("outside");
    f3["fate"] = json!("open");
    let mut f4 = finding("F-4");
    f4["fate"] = json!("open");
    let mut f5 = finding("F-5");
    f5["promise"] = json!("outside");
    f5["fate"] = json!("open");
    let moved = json!({"kind": "triage", "id": "F-4", "ticket": "ABC-1", "fate": "intent_gap", "imported_by": by});
    let false_verdict = json!({"kind": "triage", "id": "F-5", "ticket": "ABC-1", "verdict": "false", "evidence": "e", "fate": "open", "imported_by": by});
    seed(dir, &[f3, f4, f5, moved, false_verdict]);

    let line = |id: &str, extra: &str| {
        format!("{{\"id\":\"{id}\",\"ticket\":\"ABC-1\",\"fate\":\"defer\"{extra}}}\n")
    };
    let run = |text: &str| {
        put(dir, "t.jsonl", text);
        stapel(dir)
            .args(["findings", "triage", "t.jsonl"])
            .assert()
    };

    // F-3 outside and open: a note is needed; after a record with a note it is not.
    run(&line("F-3", "")).code(2);
    run(&line("F-3", ",\"note\":\"the person said so\""))
        .success()
        .stdout("triaged: 1 findings of ABC-1\n");
    run(&line("F-3", ""))
        .success()
        .stdout("triaged: 1 findings of ABC-1\n");

    // F-4 was moved to intent_gap by an earlier record: the person's finding again.
    run(&line("F-4", ""))
        .code(2)
        .stdout("")
        .stderr("stapel: t.jsonl:1: note: missing; a finding for the person needs the person's answer\n");

    // F-5 is false by an earlier verdict: no note needed.
    run(&line("F-5", ""))
        .success()
        .stdout("triaged: 1 findings of ABC-1\n");
}

#[test]
fn triage_line_rules() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    seed(dir, &[finding("F-1")]);
    let ok = r#"{"id":"F-1","ticket":"ABC-1","fate":"defer"}"#;

    let refused = |text: &str, reason: &str| {
        put(dir, "t.jsonl", text);
        let before = common::snapshot(dir);
        stapel(dir)
            .args(["findings", "triage", "t.jsonl"])
            .assert()
            .code(2)
            .stdout("")
            .stderr(format!("stapel: t.jsonl:{reason}\n"));
        assert_eq!(common::snapshot(dir), before, "{text}");
    };

    refused(
        r#"{"id":"F-1","ticket":"ABC-1","fate":"defer","imported_by":{"by":"person","user":"x"}}
"#,
        "1: imported_by: written by stapel, not by the file",
    );
    let id_rule = "1: id: must be 1 to 64 characters with no whitespace or control character";
    refused(
        &format!(
            "{{\"id\":\"{}\",\"ticket\":\"ABC-1\",\"fate\":\"defer\"}}\n",
            "a".repeat(65)
        ),
        id_rule,
    );
    refused(
        "{\"id\":\"F 1\",\"ticket\":\"ABC-1\",\"fate\":\"defer\"}\n",
        id_rule,
    );

    // A line of 64 KiB less 10 bytes grows past the limit with the insert.
    let short = r#"{"id":"F-1","ticket":"ABC-1","fate":"defer","note":""}"#;
    let long = format!(
        "{}{}\"}}",
        &short[..short.len() - 2],
        "x".repeat(65536 - 10 - short.len())
    );
    assert_eq!(long.len(), 65536 - 10);
    refused(&format!("{long}\n"), "1: longer than 64 KiB");

    // A file over 256 KiB.
    let big = format!("{ok}\n").repeat(6000);
    assert!(big.len() > 256 * 1024);
    refused(&big, " more than 256 KiB; split the file");

    // A damaged line in findings.jsonl: the refusal of `findings import`.
    put(dir, TARGET, &format!("{}\nnot json\n", finding("F-1")));
    put(dir, "t.jsonl", &format!("{ok}\n"));
    let before = common::snapshot(dir);
    let out = stapel(dir)
        .args(["findings", "triage", "t.jsonl"])
        .assert()
        .code(2)
        .stdout("")
        .get_output()
        .clone();
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.starts_with("stapel: .stapel/tickets/ABC-1/findings.jsonl:2: "),
        "{err}"
    );
    assert_eq!(common::snapshot(dir), before);
}

#[test]
fn guard_allows_triage() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "f.jsonl", "{}\n");
    let command = "stapel findings triage f.jsonl ABC-1";

    for mode in ["default", "bypassPermissions", ""] {
        common::hook_bash(dir, command, mode)
            .success()
            .stdout("")
            .stderr("");
    }

    let payload = json!({
        "tool_name": "Bash",
        "cwd": dir,
        "tool_input": { "command": command, "description": "test" },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .assert()
        .success()
        .stdout("")
        .stderr("");
    assert!(!common::grants_dir(dir).exists());
}

// ---- Step 4: the current state and the actions `triage` and `ask-human` of `next` ----

/// A ticket with a Frame, a Spec, a Plan and the confirmations of all four sections.
fn past_spec() -> tempfile::TempDir {
    let repo = repo_with_ticket();
    let dir = repo.path();
    common::set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    common::set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | WHEN a thing happens THE command does it. | `next::t_one` |",
    );
    common::set_section(
        dir,
        "ABC-1",
        "Plan",
        "Route: full, auto.\n\n1. One (AC-1).\n2. Two (AC-1).",
    );
    for s in ["spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", s]).assert().success();
    }
    repo
}

/// Appends `line` to the ticket's findings.jsonl.
fn append(dir: &Path, line: &str) {
    let mut text = common::read(dir, TARGET);
    text.push_str(line);
    text.push('\n');
    put(dir, TARGET, &text);
}

fn open_finding(id: &str) -> Value {
    let mut v = finding(id);
    v["fate"] = json!("open");
    v
}

fn triage_line(id: &str, fields: Value) -> String {
    let mut v = json!({ "kind": "triage", "id": id, "ticket": "ABC-1" });
    for (k, f) in fields.as_object().unwrap() {
        v[k] = f.clone();
    }
    v.to_string()
}

/// The `blocked: finding` lines and the action line of `stapel next`.
fn next_lines(dir: &Path) -> (Vec<String>, String) {
    let out = stapel(dir)
        .args(["next", "ABC-1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let facts = text
        .lines()
        .filter(|l| l.starts_with("blocked: finding ") || l.starts_with("blocked: and "))
        .map(String::from)
        .collect();
    let actions: Vec<&str> = text.lines().filter(|l| l.starts_with("next: ")).collect();
    assert_eq!(actions.len(), 1, "one action line in: {text}");
    (facts, actions[0].to_string())
}

fn assert_no_line(dir: &Path) {
    let (facts, action) = next_lines(dir);
    assert!(facts.is_empty(), "{facts:?}");
    assert!(
        !action.starts_with("next: triage") && !action.starts_with("next: ask-human"),
        "{action}"
    );
}

#[test]
fn next_asks_for_triage() {
    let repo = past_spec();
    let dir = repo.path();
    seed(dir, &[open_finding("F-1")]);
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-1 (HIGH, inside, code-review round 1): open"]
    );
    assert_eq!(
        action,
        "next: triage — .stapel/tickets/ABC-1/findings.jsonl"
    );

    // Twelve open MEDIUM: ten lines and the count of the rest.
    let many: Vec<Value> = (1..=12)
        .map(|i| {
            let mut v = open_finding(&format!("M-{i}"));
            v["severity"] = json!("MEDIUM");
            v
        })
        .collect();
    seed(dir, &many);
    let (facts, action) = next_lines(dir);
    assert_eq!(facts.len(), 11, "{facts:?}");
    assert_eq!(
        facts[0],
        "blocked: finding M-1 (MEDIUM, inside, code-review round 1): open"
    );
    assert_eq!(
        facts[9],
        "blocked: finding M-10 (MEDIUM, inside, code-review round 1): open"
    );
    assert_eq!(facts[10], "blocked: and 2 more");
    assert_eq!(
        action,
        "next: triage — .stapel/tickets/ABC-1/findings.jsonl"
    );

    // A closed ticket stays closed.
    let mut state = common::state_json(dir, "ABC-1");
    state["closed"] = json!({"by": "test-user", "at": "2026-10-19T10:00:00Z", "reason": "done"});
    put(dir, ".stapel/tickets/ABC-1/state.json", &state.to_string());
    let (facts, action) = next_lines(dir);
    assert!(facts.is_empty(), "{facts:?}");
    assert_eq!(action, "next: closed");
}

#[test]
fn next_triage_before_confirm() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    common::set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    common::set_section(
        dir,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | WHEN a thing happens THE command does it. | `next::t_one` |",
    );
    common::set_section(
        dir,
        "ABC-1",
        "Plan",
        "Route: full, auto.\n\n1. One (AC-1).\n2. Two (AC-1).",
    );
    let mut f = open_finding("F-1");
    f["stage"] = json!("spec-review");
    seed(dir, &[f]);
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-1 (HIGH, inside, spec-review round 1): open"]
    );
    assert_eq!(
        action,
        "next: triage — .stapel/tickets/ABC-1/findings.jsonl"
    );

    append(dir, &triage_line("F-1", json!({"fate": "patch"})));
    let (facts, action) = next_lines(dir);
    assert!(facts.is_empty(), "{facts:?}");
    assert_eq!(action, "next: confirm — stapel ok ABC-1");
}

#[test]
fn next_asks_the_human() {
    let repo = past_spec();
    let dir = repo.path();
    let target = "next: ask-human — .stapel/tickets/ABC-1/findings.jsonl";

    // An open outside LOW.
    let mut f = open_finding("F-1");
    f["severity"] = json!("LOW");
    f["promise"] = json!("outside");
    seed(dir, &[f]);
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-1 (LOW, outside, code-review round 1): open"]
    );
    assert_eq!(action, target);

    // A fate intent_gap.
    let mut f = finding("F-1");
    f["severity"] = json!("LOW");
    f["fate"] = json!("intent_gap");
    seed(dir, &[f]);
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-1 (LOW, inside, code-review round 1): intent_gap"]
    );
    assert_eq!(action, target);

    // An open HIGH of code-review round 2, and of round "2".
    for round in [json!(2), json!("2")] {
        let mut f = open_finding("F-1");
        f["round"] = round;
        seed(dir, &[f]);
        let (_, action) = next_lines(dir);
        assert_eq!(action, target);
    }

    // A step-drift finding of round "step-2" is not a person's finding.
    let mut f = open_finding("F-1");
    f["stage"] = json!("step-drift");
    f["round"] = json!("step-2");
    seed(dir, &[f]);
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-1 (HIGH, inside, step-drift round step-2): open"]
    );
    assert_eq!(
        action,
        "next: triage — .stapel/tickets/ABC-1/findings.jsonl"
    );

    // One person's finding among others: the lines of all, the action ask-human.
    let mut p = open_finding("F-2");
    p["round"] = json!(2);
    seed(dir, &[open_finding("F-1"), p]);
    let (facts, action) = next_lines(dir);
    assert_eq!(facts.len(), 2, "{facts:?}");
    assert_eq!(action, target);
}

#[test]
fn current_state_of_a_finding() {
    let repo = past_spec();
    let dir = repo.path();
    seed(dir, &[open_finding("F-1")]);
    let (facts, _) = next_lines(dir);
    assert_eq!(facts.len(), 1);

    append(
        dir,
        &triage_line("F-1", json!({"fate": "open", "verdict": "low"})),
    );
    assert_no_line(dir);

    append(
        dir,
        &triage_line("F-1", json!({"fate": "open", "verdict": "medium"})),
    );
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-1 (MEDIUM, inside, code-review round 1): open"]
    );
    assert_eq!(
        action,
        "next: triage — .stapel/tickets/ABC-1/findings.jsonl"
    );

    append(
        dir,
        &triage_line("F-1", json!({"fate": "open", "verdict": "false"})),
    );
    assert_no_line(dir);

    // A fate open with no verdict keeps the last verdict.
    append(dir, &triage_line("F-1", json!({"fate": "open"})));
    assert_no_line(dir);

    // Written by hand: two finding records of F-4, the last one counts; a line that is not JSON
    // is skipped; a fate that is not text and a severity that is not text give no line.
    let mut low = open_finding("F-4");
    low["severity"] = json!("LOW");
    let mut f5 = open_finding("F-5");
    f5["fate"] = json!(["x"]);
    let mut f6 = open_finding("F-6");
    f6["severity"] = json!(3);
    seed(dir, &[low, f5, f6]);
    append(dir, "this is not json");
    assert_no_line(dir);
    append(dir, &open_finding("F-4").to_string());
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-4 (HIGH, inside, code-review round 1): open"]
    );
    assert_eq!(
        action,
        "next: triage — .stapel/tickets/ABC-1/findings.jsonl"
    );
}

#[test]
fn legacy_records_do_not_block() {
    let repo = past_spec();
    let dir = repo.path();
    let mut old = finding("F-1");
    old["fate"] = json!("fixed: done");
    seed(dir, &[old, open_finding("F-2")]);
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-2 (HIGH, inside, code-review round 1): open"]
    );
    assert_eq!(
        action,
        "next: triage — .stapel/tickets/ABC-1/findings.jsonl"
    );
}

// ---- Step 4b: coverage after the drift review of step 4 ----

#[test]
fn next_triage_among_the_actions() {
    fn commit(dir: &Path, files: &[(&str, &str)], subject: &str) {
        for (rel, text) in files {
            put(dir, rel, text);
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
    let line = "blocked: finding F-1 (HIGH, inside, code-review round 1): open";
    let triage = "next: triage — .stapel/tickets/ABC-1/findings.jsonl";

    // No Frame: write-frame, no finding line.
    let repo = repo_with_ticket();
    let dir = repo.path();
    seed(dir, &[open_finding("F-1")]);
    let (facts, action) = next_lines(dir);
    assert!(facts.is_empty(), "{facts:?}");
    assert_eq!(action, "next: write-frame — .stapel/tickets/ABC-1/ticket.md");

    // A Frame, the Plan not written: write-section.
    common::set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    let (facts, action) = next_lines(dir);
    assert!(facts.is_empty(), "{facts:?}");
    assert_eq!(
        action,
        "next: write-section Plan — .stapel/tickets/ABC-1/ticket.md"
    );

    // A confirmed ticket with step 1 built as RED only: triage, not green-step.
    let repo = past_spec();
    let dir = repo.path();
    commit(dir, &[], "initial");
    commit(
        dir,
        &[(
            "crates/tiny/tests/t1.rs",
            "#[test]\nfn t() {\n    assert!(true);\n}\n",
        )],
        "ABC-1 step 1 RED: one",
    );
    let (facts, action) = next_lines(dir);
    assert!(facts.is_empty(), "{facts:?}");
    assert!(action.starts_with("next: green-step step 1"), "{action}");
    seed(dir, &[open_finding("F-1")]);
    let (facts, action) = next_lines(dir);
    assert_eq!(facts, [line]);
    assert_eq!(action, triage);
}

#[test]
fn next_cap_of_ten() {
    let triage = "next: triage — .stapel/tickets/ABC-1/findings.jsonl";
    let ask = "next: ask-human — .stapel/tickets/ABC-1/findings.jsonl";
    let many = |n: usize| -> Vec<Value> {
        (1..=n)
            .map(|i| open_finding(&format!("F-{i}")))
            .collect()
    };
    let repo = past_spec();
    let dir = repo.path();

    // Ten: ten lines, no `and` line.
    seed(dir, &many(10));
    let (facts, action) = next_lines(dir);
    assert_eq!(facts.len(), 10, "{facts:?}");
    assert!(facts.iter().all(|l| l.starts_with("blocked: finding ")));
    assert_eq!(action, triage);

    // Eleven: ten lines and the count of the rest.
    seed(dir, &many(11));
    let (facts, action) = next_lines(dir);
    assert_eq!(facts.len(), 11, "{facts:?}");
    assert_eq!(
        facts[9],
        "blocked: finding F-10 (HIGH, inside, code-review round 1): open"
    );
    assert_eq!(facts[10], "blocked: and 1 more");
    assert_eq!(action, triage);

    // Eleven, one of them outside: ask-human with the same lines.
    let mut all = many(11);
    all[10]["promise"] = json!("outside");
    seed(dir, &all);
    let (facts, action) = next_lines(dir);
    assert_eq!(facts.len(), 11, "{facts:?}");
    assert_eq!(
        facts[9],
        "blocked: finding F-10 (HIGH, inside, code-review round 1): open"
    );
    assert_eq!(facts[10], "blocked: and 1 more");
    assert_eq!(action, ask);
}

#[test]
fn next_round_forms() {
    let triage = "next: triage — .stapel/tickets/ABC-1/findings.jsonl";
    let ask = "next: ask-human — .stapel/tickets/ABC-1/findings.jsonl";
    let repo = past_spec();
    let dir = repo.path();

    // A round string that is not a whole number is not a person's finding.
    let mut f = open_finding("F-1");
    f["round"] = json!("2a");
    seed(dir, &[f]);
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-1 (HIGH, inside, code-review round 2a): open"]
    );
    assert_eq!(action, triage);

    // A round 2.0 is the whole round 2: the person, the line shows `round 2.0`.
    let mut f = open_finding("F-1");
    f["round"] = json!(2.0);
    seed(dir, &[f]);
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-1 (HIGH, inside, code-review round 2.0): open"]
    );
    assert_eq!(action, ask);

    // An open HIGH of spec-review round 2: the person.
    let mut f = open_finding("F-1");
    f["stage"] = json!("spec-review");
    f["round"] = json!(2);
    seed(dir, &[f]);
    let (facts, action) = next_lines(dir);
    assert_eq!(
        facts,
        ["blocked: finding F-1 (HIGH, inside, spec-review round 2): open"]
    );
    assert_eq!(action, ask);
}

// ---- Step 5: `stapel brief` shows the current state of a finding ----

#[test]
fn brief_shows_the_current_fate() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    common::set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    common::set_section(dir, "ABC-1", "Plan", "1. One (F-1, F-2, F-3, F-4).");
    seed(dir, &[open_finding("F-1"), finding("F-2"), finding("F-3"), finding("F-4")]);
    append(dir, &triage_line("F-1", json!({"verdict": "low", "fate": "defer"})));
    append(
        dir,
        &triage_line(
            "F-2",
            json!({"verdict": "false", "evidence": "e", "fate": "no-change"}),
        ),
    );
    // A later verdict that is not false hides the earlier false one.
    append(
        dir,
        &triage_line(
            "F-3",
            json!({"verdict": "false", "evidence": "e", "fate": "no-change"}),
        ),
    );
    append(dir, &triage_line("F-3", json!({"verdict": "medium", "fate": "patch"})));
    append(
        dir,
        &triage_line("F-4", json!({"verdict": "maybe-false", "fate": "defer"})),
    );
    let out = stapel(dir)
        .args(["brief", "ABC-1", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.ends_with(
            "## Findings\n\
             - F-1 (LOW): a claim Fate: defer\n\
             - F-2 (HIGH): a claim Fate: no-change Verdict: false\n\
             - F-3 (MEDIUM): a claim Fate: patch\n\
             - F-4 (HIGH): a claim Fate: defer Verdict: maybe-false\n"
        ),
        "in: {text}"
    );
}

#[test]
fn brief_reads_old_and_later_records() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    common::set_section(dir, "ABC-1", "Description", "### Frame\n\nThe problem.");
    common::set_section(dir, "ABC-1", "Plan", "1. One (F-1, F-2, F-3).");
    let mut f1 = finding("F-1");
    f1["fate"] = json!("fixed: done");
    let mut f3_low = finding("F-3");
    f3_low["severity"] = json!("LOW");
    f3_low["claim"] = json!("the first claim");
    let mut f3_high = finding("F-3");
    f3_high["claim"] = json!("the second claim");
    seed(dir, &[f1, finding("F-2"), f3_low, f3_high]);
    append(
        dir,
        &triage_line(
            "F-2",
            json!({"verdict": "false", "evidence": "e", "fate": "no-change"}),
        ),
    );
    append(dir, &triage_line("F-2", json!({"fate": "defer"})));
    let out = stapel(dir)
        .args(["brief", "ABC-1", "1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.ends_with(
            "## Findings\n\
             - F-1 (HIGH): a claim Fate: fixed: done\n\
             - F-2 (HIGH): a claim Fate: defer Verdict: false\n\
             - F-3 (HIGH): the second claim Fate: patch\n"
        ),
        "in: {text}"
    );
}
