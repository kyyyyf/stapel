//! STP-3 AC-5: append-only journals (`decisions.jsonl`, `tokens.jsonl`).

mod common;

use common::{repo_with_ticket, stapel};
use predicates::str::contains;
use serde_json::json;
use stapel_core::journal::{Entry, MAX_LINE, append, read};
use std::path::Path;

fn problems(entries: &[(usize, Entry)]) -> Vec<usize> {
    entries
        .iter()
        .filter(|(_, e)| matches!(e, Entry::Problem(_)))
        .map(|(n, _)| *n)
        .collect()
}

#[test]
fn corrupt_line_is_reported_and_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tokens.jsonl");
    let long = format!("{{\"v\":1,\"note\":\"{}\"}}", "x".repeat(70 * 1024));
    let original =
        format!("{{\"v\":1,\"id\":\"a\"}}\n{{broken\n{long}\n\n{{\"v\":1,\"id\":\"b\"}}\n");
    std::fs::write(&path, &original).unwrap();

    let entries = read(&path).unwrap();
    assert_eq!(problems(&entries), [2, 3]);
    assert_eq!(
        entries
            .iter()
            .filter(|(_, e)| matches!(e, Entry::V1(_)))
            .count(),
        2
    );

    append(&path, &json!({"v": 1, "id": "c"})).unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.starts_with(&original), "existing lines were changed");
    let too_long = json!({"v": 1, "note": "y".repeat(MAX_LINE)});
    assert!(append(&path, &too_long).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), after);
}

#[test]
fn missing_newline_is_repaired_on_append() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("decisions.jsonl");
    std::fs::write(&path, "{\"v\":1,\"id\":\"half").unwrap();
    append(&path, &json!({"v": 1, "id": "next"})).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text, "{\"v\":1,\"id\":\"half\n{\"v\":1,\"id\":\"next\"}\n");
    let entries = read(&path).unwrap();
    assert_eq!(problems(&entries), [1]);
    assert!(matches!(entries[1].1, Entry::V1(_)));
}

#[test]
fn legacy_lines_are_read() {
    // The hand-written journals of STP-1, STP-2 and STP-3, as they are in this repository.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for key in ["STP-1", "STP-2", "STP-3"] {
        let path = root.join(".stapel/tickets").join(key).join("tokens.jsonl");
        let text = std::fs::read_to_string(&path).unwrap();
        let lines = text
            .lines()
            .filter(|l| {
                !l.is_empty()
                    && serde_json::from_str::<serde_json::Value>(l)
                        .is_ok_and(|v| v.get("v").is_none())
            })
            .count();
        let entries = read(&path).unwrap();
        assert!(
            problems(&entries).is_empty(),
            "{key}: {:?}",
            problems(&entries)
        );
        let legacy = entries
            .iter()
            .filter(|(_, e)| matches!(e, Entry::Legacy(_)))
            .count();
        assert_eq!(legacy, lines, "{key}");
    }
    assert!(read(&root.join("no/such/file.jsonl")).unwrap().is_empty());
}

#[test]
fn concurrent_appends_keep_every_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tokens.jsonl");
    let threads: Vec<_> = (0..10)
        .map(|t| {
            let path = path.clone();
            std::thread::spawn(move || {
                for i in 0..200 {
                    let note = format!("thread {t} line {i} — non-ASCII ü ✓ {}", "z".repeat(500));
                    append(
                        &path,
                        &json!({"v": 1, "id": format!("{t}-{i}"), "note": note}),
                    )
                    .unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let entries = read(&path).unwrap();
    assert_eq!(entries.len(), 2000);
    assert!(problems(&entries).is_empty());
}

#[test]
fn status_warns_without_failing() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::write(dir.join(".stapel/tickets/ABC-1/tokens.jsonl"), "{broken\n").unwrap();
    stapel(dir)
        .args(["status", "ABC-1"])
        .assert()
        .success()
        .stdout(contains(
            "warning: journal line .stapel/tickets/ABC-1/tokens.jsonl:1 cannot be read",
        ));
}
