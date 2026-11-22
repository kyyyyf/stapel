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
