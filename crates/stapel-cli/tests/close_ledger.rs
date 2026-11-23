//! STP-37 step 11: `stapel close` writes the `catches` and `finding` records of the ledger.

mod common;

use common::{confirm_sections, read, repo_with_ticket, stapel, state_json};
use serde_json::{Value, json};
use std::path::Path;

const FINDINGS: &str = ".stapel/tickets/ABC-1/findings.jsonl";
const LEDGER: &str = ".stapel/process-ledger.jsonl";

fn finding(id: &str, stage: &str, round: Value, severity: &str, fate: &str) -> Value {
    json!({
        "id": id,
        "ticket": "ABC-1",
        "stage": stage,
        "round": round,
        "role": "reviewer",
        "severity": severity,
        "promise": "inside",
        "claim": format!("claim of {id}"),
        "catchable_at": "code",
        "fate": fate,
    })
}

fn triage(id: &str, fate: &str, verdict: Option<&str>) -> Value {
    let mut v = json!({"kind": "triage", "id": id, "ticket": "ABC-1", "fate": fate});
    if let Some(verdict) = verdict {
        v["verdict"] = json!(verdict);
    }
    v
}

fn put(dir: &Path, rel: &str, lines: &[Value]) {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(dir.join(rel), text).unwrap();
}

fn close(dir: &Path) -> std::process::Output {
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn ledger_lines(dir: &Path) -> Vec<Value> {
    if !dir.join(LEDGER).exists() {
        return Vec::new();
    }
    read(dir, LEDGER)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn is_open(dir: &Path) -> bool {
    state_json(dir, "ABC-1").get("closed").is_none()
}

// AC-16: the counts, the deferred records and the printed line.
#[test]
fn writes_catches_and_deferred() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm_sections(dir, "ABC-1");
    let old = json!({"v": 1, "at": "2026-01-01", "kind": "decision", "ticket": "ABC-1", "note": "old"});
    put(dir, LEDGER, &[old.clone()]);
    fn ruled(mut f: Value, rule: &str) -> Value {
        f["rule"] = json!(rule);
        f
    }
    let long = {
        let mut f = ruled(finding("F10", "code-review", json!(1), "HIGH", "defer"), "r-a");
        f["claim"] = json!("x".repeat(600));
        f
    };
    let mut outside = finding("F6", "code-review", json!(1), "HIGH", "open");
    outside["promise"] = json!("outside");
    put(
        dir,
        FINDINGS,
        &[
            ruled(finding("F1", "code-review", json!(1), "HIGH", "patch"), "r-b"),
            ruled(finding("F2", "code-review", json!(1), "MEDIUM", "patch"), "r-a"),
            ruled(finding("F3", "code-review", json!(1), "LOW", "patch"), "r-a"),
            ruled(finding("F4", "code-review", json!(1), "HIGH", "patch"), "r-a"),
            triage("F4", "patch", Some("low")),
            ruled(finding("F5", "code-review", json!(1), "HIGH", "patch"), "r-a"),
            triage("F5", "patch", Some("false")),
            outside,
            ruled(finding("F7", "code-review", json!(1), "HIGH", "no-change"), "r-a"),
            ruled(finding("F8", "spec-review", json!(1), "HIGH", "patch"), "a;b=c"),
            finding("F9", "spec-review", json!(1), "MEDIUM", "patch"),
            long,
            ruled(finding("F11", "spec-review", json!("2"), "MEDIUM", "open"), "r-b"),
            triage("F11", "defer", None),
            ruled(finding("F12", "code-review", json!(1), "HIGH", "defer"), "r-a"),
            triage("F12", "defer", Some("false")),
            ruled(finding("F13", "code-review", json!(1), "HIGH", "defer"), "r-a"),
            triage("F13", "defer", Some("low")),
        ],
    );
    let out = close(dir);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(stdout.lines().any(|l| l == "ledger: 4 records"), "{stdout}");
    assert!(stdout.contains("closed: ABC-1"), "{stdout}");
    let lines = ledger_lines(dir);
    assert_eq!(lines.len(), 5, "{lines:?}");
    assert_eq!(lines[0], old);
    let kinds: Vec<&str> = lines[1..].iter().map(|l| l["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["catches", "finding", "finding", "finding"]);
    for l in &lines[1..] {
        assert_eq!(l["v"], 1);
        assert_eq!(l["ticket"], "ABC-1");
        assert!(l["at"].as_str().unwrap().len() == 10, "{l}");
    }
    assert_eq!(
        lines[1]["note"],
        "At the close: counted findings: code-review/r-a=2; code-review/r-b=1; \
         spec-review/a_b_c=1; spec-review/none=1; spec-review/r-b=1"
    );
    let cut = "x".repeat(500);
    assert_eq!(
        lines[2]["note"],
        format!("At the close, deferred: F10 HIGH code-review round 1: {cut}")
    );
    assert_eq!(
        lines[3]["note"],
        "At the close, deferred: F11 MEDIUM spec-review round 2: claim of F11"
    );
    assert_eq!(
        lines[4]["note"],
        "At the close, deferred: F13 LOW code-review round 1: claim of F13"
    );
    assert!(!read(dir, LEDGER).contains("F12"));
    assert!(state_json(dir, "ABC-1").get("closed").is_some());
}

// AC-16: no findings, one record.
#[test]
fn no_findings() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm_sections(dir, "ABC-1");
    let out = close(dir);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(stdout.lines().any(|l| l == "ledger: 1 records"), "{stdout}");
    let lines = ledger_lines(dir);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["kind"], "catches");
    assert_eq!(lines[0]["note"], "At the close: counted findings: none");
}

// AC-17: a refused close writes nothing; the later close writes.
#[test]
fn a_refused_close_writes_nothing() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(
        dir,
        FINDINGS,
        &[finding("F1", "code-review", json!(1), "HIGH", "patch")],
    );
    // A section waits for `stapel ok`.
    stapel(dir).args(["ok", "ABC-1", "spec"]).assert().success();
    let out = close(dir);
    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("wait for stapel ok"), "{}", text(&out.stderr));
    assert!(is_open(dir) && !dir.join(LEDGER).exists());
    confirm_sections(dir, "ABC-1");
    // A missing reason.
    let out = stapel(dir)
        .args(["close", "ABC-1", "--reason", " "])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(is_open(dir) && !dir.join(LEDGER).exists());
    // A check that has not passed.
    let toml = read(dir, ".stapel/stapel.toml");
    std::fs::write(
        dir.join(".stapel/stapel.toml"),
        format!("{toml}\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n"),
    )
    .unwrap();
    let out = close(dir);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("current passing check"),
        "{}",
        text(&out.stderr)
    );
    assert!(is_open(dir) && !dir.join(LEDGER).exists());
    // The fix: the close writes.
    std::fs::write(dir.join(".stapel/stapel.toml"), toml).unwrap();
    let out = close(dir);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let lines = ledger_lines(dir);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(
        lines[0]["note"],
        "At the close: counted findings: code-review/none=1"
    );
}

// AC-17: a malformed journal line or a failed append refuses; the later close writes.
#[test]
fn bad_journals_refuse() {
    use std::os::unix::fs::PermissionsExt;
    let repo = repo_with_ticket();
    let dir = repo.path();
    confirm_sections(dir, "ABC-1");
    // A malformed line of findings.jsonl.
    let good = finding("F1", "code-review", json!(1), "HIGH", "patch");
    std::fs::write(dir.join(FINDINGS), format!("{good}\nnot json\n")).unwrap();
    let out = close(dir);
    assert!(!out.status.success());
    let err = text(&out.stderr);
    assert!(err.contains("findings.jsonl:2"), "{err}");
    assert!(is_open(dir) && !dir.join(LEDGER).exists());
    put(dir, FINDINGS, &[good]);
    // A malformed line of the ledger.
    std::fs::write(dir.join(LEDGER), "broken\n").unwrap();
    let out = close(dir);
    assert!(!out.status.success());
    let err = text(&out.stderr);
    assert!(err.contains("process-ledger.jsonl:1"), "{err}");
    assert!(is_open(dir));
    assert_eq!(read(dir, LEDGER), "broken\n");
    std::fs::remove_file(dir.join(LEDGER)).unwrap();
    // A ledger file that cannot be written: the grant is spent.
    let old = json!({"v": 1, "at": "2026-01-01", "kind": "decision", "ticket": "ABC-1", "note": "old"});
    put(dir, LEDGER, &[old]);
    let before = read(dir, LEDGER);
    std::fs::set_permissions(dir.join(LEDGER), std::fs::Permissions::from_mode(0o444)).unwrap();
    let (_, replaced) = common::ask(dir, "stapel close --reason done");
    let token = common::token_of(&replaced);
    let with_grant = |token: &str| {
        stapel(dir)
            .args(["close", "ABC-1", "--reason", "done", "--grant", token])
            .env("CLAUDECODE", "1")
            .output()
            .unwrap()
    };
    let out = with_grant(&token);
    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("process-ledger.jsonl"), "{}", text(&out.stderr));
    assert!(is_open(dir));
    assert_eq!(read(dir, LEDGER), before);
    std::fs::set_permissions(dir.join(LEDGER), std::fs::Permissions::from_mode(0o644)).unwrap();
    let out = with_grant(&token);
    assert!(!out.status.success(), "the grant is spent");
    assert!(is_open(dir) && read(dir, LEDGER) == before);
    let (_, replaced) = common::ask(dir, "stapel close --reason done");
    let out = with_grant(&common::token_of(&replaced));
    assert!(out.status.success(), "{}", text(&out.stderr));
    let lines = ledger_lines(dir);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(
        lines[1]["note"],
        "At the close: counted findings: code-review/none=1"
    );
}
