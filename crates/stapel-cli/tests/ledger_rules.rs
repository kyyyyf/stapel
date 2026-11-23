//! STP-38: `stapel ledger rules`. Step 6: the counts per rule and ticket.

mod common;

use common::{repo_with_ticket, snapshot, stapel};
use serde_json::{Value, json};
use std::path::Path;

fn put(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn lines(items: &[Value]) -> String {
    items.iter().map(|l| format!("{l}\n")).collect()
}

/// A finding of `key`; `extra` fields replace or add to the base ones.
fn finding(key: &str, id: &str, severity: &str, extra: Value) -> Value {
    let mut v = json!({
        "id": id,
        "ticket": key,
        "stage": "code-review",
        "round": 1,
        "role": "reviewer",
        "severity": severity,
        "promise": "inside",
        "claim": "a claim",
        "catchable_at": "code",
        "fate": "open",
    });
    for (k, value) in extra.as_object().unwrap() {
        v[k] = value.clone();
    }
    v
}

fn rules_output(dir: &Path) -> String {
    let out = stapel(dir).args(["ledger", "rules"]).assert().success();
    let out = out.get_output();
    assert_eq!(String::from_utf8_lossy(&out.stderr), "");
    String::from_utf8(out.stdout.clone()).unwrap()
}

#[test]
fn counts_per_rule_and_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file = |key: &str| format!(".stapel/tickets/{key}/findings.jsonl");

    put(
        dir,
        &file("ABC-1"),
        &lines(&[
            finding("ABC-1", "F-1", "HIGH", json!({})),
            finding("ABC-1", "F-2", "MEDIUM", json!({"rule": ""})),
            finding("ABC-1", "F-3", "HIGH", json!({"rule": 7})),
            finding("ABC-1", "F-4", "LOW", json!({"rule": "rule 3"})),
        ]),
    );
    put(
        dir,
        &file("ABC-2"),
        &lines(&[
            finding("ABC-2", "F-1", "HIGH", json!({"rule": "rule 3"})),
            finding("ABC-2", "F-2", "HIGH", json!({"rule": " rule 3 "})),
            finding(
                "ABC-2",
                "F-3",
                "HIGH",
                json!({"rule": "rule 9", "promise": "outside"}),
            ),
            finding(
                "ABC-2",
                "F-4",
                "HIGH",
                json!({"rule": "rule 5", "verdict": "false", "evidence": "the code does it"}),
            ),
            finding(
                "ABC-2",
                "F-5",
                "HIGH",
                json!({"rule": "rule 6", "fate": "no-change"}),
            ),
            finding("ABC-2", "F-6", "HIGH", json!({"rule": "rule 7"})),
            finding("ABC-2", "F-7", "LOW", json!({"rule": "rule 8"})),
            json!({"kind": "triage", "id": "F-6", "ticket": "ABC-2", "fate": "open", "verdict": "low"}),
            json!({"kind": "triage", "id": "F-7", "ticket": "ABC-2", "fate": "open", "verdict": "high"}),
        ]),
    );
    put(
        dir,
        &file("ABC-3"),
        &lines(&[
            finding("ABC-3", "F-1", "MEDIUM", json!({"rule": "rule 3"})),
            finding("ABC-3", "F-2", "HIGH", json!({"rule": "Rule 3"})),
        ]),
    );

    let before = snapshot(dir);
    assert_eq!(rules_output(dir), include_str!("golden/ledger_rules.txt"));
    assert_eq!(snapshot(dir), before, "ledger rules writes nothing");

    // Seven tickets with findings: the newest five get a column, `total` counts all seven.
    for n in 1..=7 {
        let key = format!("ABC-{n}");
        put(
            dir,
            &file(&key),
            &lines(&[finding(&key, "F-1", "HIGH", json!({"rule": "r"}))]),
        );
    }
    assert_eq!(
        rules_output(dir),
        "rule  ABC-3  ABC-4  ABC-5  ABC-6  ABC-7  total\n\
         r         1      1      1      1      1      7\n"
    );
}

#[test]
fn bad_lines_and_last() {
    fn words(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .map(|l| l.split_whitespace().map(String::from).collect())
            .collect()
    }
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file = ".stapel/tickets/ABC-1/findings.jsonl";

    // No ticket with findings.
    assert_eq!(rules_output(dir), "rules: no findings\n");

    // A broken line is skipped, with one notice.
    let mut text = lines(&[finding("ABC-1", "F-1", "HIGH", json!({"rule": "r"}))]);
    text.push_str("{not json\n");
    put(dir, file, &text);
    let out = stapel(dir).args(["ledger", "rules"]).assert().success();
    let out = out.get_output();
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "notice: 1 lines of findings.jsonl skipped\n"
    );
    assert_eq!(
        words(&String::from_utf8_lossy(&out.stdout))[1],
        ["r", "1", "1"]
    );

    // `--last` that is not a whole number of 1 or more.
    for bad in ["0", "x", "-1", "1.5", ""] {
        stapel(dir)
            .args(["ledger", "rules", "--last", bad])
            .assert()
            .code(2);
    }
}

#[test]
fn marks_quiet_rules() {
    // The rows as words, so the test does not depend on padding.
    fn words(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .map(|l| l.split_whitespace().map(String::from).collect())
            .collect()
    }
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file = |key: &str| format!(".stapel/tickets/{key}/findings.jsonl");
    let one = |key: &str, rule: &str, severity: &str| {
        lines(&[finding(key, "F-1", severity, json!({"rule": rule}))])
    };
    // a: counted only in the oldest of four tickets. b: only LOW findings.
    // c: counted in the third. none: no count.
    put(dir, &file("ABC-1"), &one("ABC-1", "a", "HIGH"));
    put(dir, &file("ABC-2"), &one("ABC-2", "b", "LOW"));
    put(dir, &file("ABC-3"), &one("ABC-3", "c", "MEDIUM"));
    put(dir, &file("ABC-4"), &one("ABC-4", "", "LOW"));

    let all = words(&rules_output(dir));
    assert_eq!(all[0], ["rule", "ABC-1", "ABC-2", "ABC-3", "ABC-4", "total"]);
    assert_eq!(all[1], ["a", "1", "0", "0", "0", "1", "quiet"]);
    assert_eq!(all[2], ["b", "0", "0", "0", "0", "0", "quiet"]);
    assert_eq!(all[3], ["c", "0", "0", "1", "0", "1"]);
    assert_eq!(all[4], ["none", "0", "0", "0", "0", "0"]);
    assert_eq!(all.len(), 5);

    // `--last 2` shows two columns: no row is marked.
    let out = stapel(dir)
        .args(["ledger", "rules", "--last", "2"])
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!text.contains("quiet"), "{text}");

    // Two tickets: no row is marked.
    std::fs::remove_dir_all(dir.join(".stapel/tickets/ABC-3")).unwrap();
    std::fs::remove_dir_all(dir.join(".stapel/tickets/ABC-4")).unwrap();
    assert!(!rules_output(dir).contains("quiet"));
}

// STP-38 AC-8: the columns come in key-number order, then prefix order, not lexical order.
#[test]
fn orders_keys_by_number() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for key in ["ABC-10", "XYZ-9", "ABC-9"] {
        put(
            dir,
            &format!(".stapel/tickets/{key}/findings.jsonl"),
            &lines(&[finding(key, "F-1", "HIGH", json!({"rule": "r"}))]),
        );
    }
    let text = rules_output(dir);
    let head: Vec<&str> = text.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(head, ["rule", "ABC-9", "XYZ-9", "ABC-10", "total"], "{text}");
}
