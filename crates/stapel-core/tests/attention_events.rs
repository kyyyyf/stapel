//! The events reader of a main transcript (STP-29, step 1): questions and dialogs.

use serde_json::{json, Value};
use stapel_core::attention::{read_events, EventKind};
use std::path::PathBuf;

fn ts(secs: u64) -> String {
    // 2026-11-11T10:00:00Z plus `secs`, as Claude Code writes it, with milliseconds.
    let total = 10 * 3600 + secs;
    format!(
        "2026-11-11T{:02}:{:02}:{:02}.000Z",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

fn call_line(secs: u64, id: &str, name: &str, input: Value) -> Value {
    json!({
        "type": "assistant",
        "sessionId": "s1",
        "timestamp": ts(secs),
        "message": {"role": "assistant", "content": [
            {"type": "tool_use", "id": id, "name": name, "input": input}
        ]}
    })
}

fn result_line(secs: u64, id: &str, source: Option<Value>, is_error: bool) -> Value {
    let mut line = json!({
        "type": "user",
        "sessionId": "s1",
        "timestamp": ts(secs),
        "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": id, "content": "ok", "is_error": is_error}
        ]}
    });
    if let Some(source) = source {
        line["permissionDecision"] = json!({ "source": source });
    }
    line
}

fn write(dir: &tempfile::TempDir, lines: &[Value]) -> PathBuf {
    let path = dir.path().join("s1.jsonl");
    let mut text = String::new();
    for line in lines {
        text.push_str(&line.to_string());
        text.push('\n');
    }
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn question_counts_its_questions() {
    let dir = tempfile::tempdir().unwrap();
    let three = json!({"questions": [{"question": "a"}, {"question": "b"}, {"question": "c"}]});
    let path = write(
        &dir,
        &[
            call_line(0, "q1", "AskUserQuestion", three),
            result_line(90, "q1", None, false),
        ],
    );
    let events = read_events(&path).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Question(3));
    assert_eq!(events[0].wait_ms, 90_000);

    // `questions` absent, a string, an empty array, an object: count 1.
    let inputs = [
        json!({}),
        json!({"questions": "what?"}),
        json!({"questions": []}),
        json!({"questions": {"a": 1}}),
    ];
    for input in inputs {
        let path = write(
            &dir,
            &[
                call_line(0, "q1", "AskUserQuestion", input.clone()),
                result_line(5, "q1", None, false),
            ],
        );
        let events = read_events(&path).unwrap();
        assert_eq!(events.len(), 1, "input {input}");
        assert_eq!(events[0].kind, EventKind::Question(1), "input {input}");
        assert_eq!(events[0].wait_ms, 5_000);
    }

    // A rejected question is still an event.
    let path = write(
        &dir,
        &[
            call_line(0, "q1", "AskUserQuestion", json!({"questions": [{"question": "a"}, {"question": "b"}]})),
            result_line(30, "q1", None, true),
        ],
    );
    let events = read_events(&path).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Question(2));
    assert_eq!(events[0].wait_ms, 30_000);
}

#[test]
fn dialog_by_permission_source() {
    let dir = tempfile::tempdir().unwrap();
    for source in ["user_temporary", "user_permanent"] {
        let path = write(
            &dir,
            &[
                call_line(0, "t1", "Bash", json!({"command": "ls"})),
                result_line(45, "t1", Some(json!(source)), false),
            ],
        );
        let events = read_events(&path).unwrap();
        assert_eq!(events.len(), 1, "source {source}");
        assert_eq!(events[0].kind, EventKind::Dialog, "source {source}");
        assert_eq!(events[0].wait_ms, 45_000);
    }

    // `config`, `hook`, no decision, a number: no event.
    let others = [Some(json!("config")), Some(json!("hook")), None, Some(json!(7))];
    for source in others {
        let path = write(
            &dir,
            &[
                call_line(0, "t1", "Bash", json!({"command": "ls"})),
                result_line(45, "t1", source.clone(), false),
            ],
        );
        assert!(read_events(&path).unwrap().is_empty(), "source {source:?}");
    }

    // A failed call with a person's source is an event.
    let path = write(
        &dir,
        &[
            call_line(0, "t1", "Bash", json!({"command": "ls"})),
            result_line(10, "t1", Some(json!("user_temporary")), true),
        ],
    );
    assert_eq!(read_events(&path).unwrap().len(), 1);

    // The call streamed on two lines: the wait runs from the first.
    let path = write(
        &dir,
        &[
            call_line(0, "t1", "Bash", json!({"command": "ls"})),
            call_line(20, "t1", "Bash", json!({"command": "ls"})),
            result_line(60, "t1", Some(json!("user_temporary")), false),
        ],
    );
    let events = read_events(&path).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Dialog);
    assert_eq!(events[0].wait_ms, 60_000);
}
