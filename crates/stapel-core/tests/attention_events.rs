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

fn typed_line(secs: u64, content: Value) -> Value {
    json!({
        "type": "user",
        "sessionId": "s1",
        "timestamp": ts(secs),
        "message": {"role": "user", "content": content}
    })
}

fn text_line(secs: u64) -> Value {
    json!({
        "type": "assistant",
        "sessionId": "s1",
        "timestamp": ts(secs),
        "message": {"role": "assistant", "content": [{"type": "text", "text": "answer"}]}
    })
}

#[test]
fn typed_message_and_its_wait() {
    let dir = tempfile::tempdir().unwrap();
    let mut flagged = Vec::new();
    for (i, flag) in ["isMeta", "isSidechain", "isCompactSummary"].iter().enumerate() {
        let mut line = typed_line(50 + i as u64, json!("flagged"));
        line[*flag] = json!(true);
        flagged.push(line);
    }
    let mut lines = vec![
        // The first line of the file: wait 0.
        typed_line(5, json!("first")),
        text_line(10),
        typed_line(25, json!("second")),
        // A content of one `text` item; two typed lines in a row share the `assistant` line.
        typed_line(40, json!([{"type": "text", "text": "third"}])),
    ];
    lines.extend(flagged);
    lines.push(typed_line(60, json!("<command-name>/clear</command-name>")));
    lines.push(typed_line(61, json!("[Request interrupted by user]")));
    lines.push(typed_line(62, json!([{"type": "text", "text": "<local-command-stdout>"}])));
    lines.push(result_line(63, "unknown", None, false));
    let path = write(&dir, &lines);
    let events = read_events(&path).unwrap();
    let got: Vec<(EventKind, u64)> = events.iter().map(|e| (e.kind, e.wait_ms)).collect();
    assert_eq!(
        got,
        vec![
            (EventKind::Typed, 0),
            (EventKind::Typed, 15_000),
            (EventKind::Typed, 30_000),
        ]
    );
}

#[test]
fn broken_lines_are_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s1.jsonl");
    let mut no_time = typed_line(1, json!("no time"));
    no_time.as_object_mut().unwrap().remove("timestamp");
    let mut number_time = typed_line(2, json!("number time"));
    number_time["timestamp"] = json!(5);
    let mut big = call_line(3, "big", "Bash", json!({"command": "ls"}));
    big["padding"] = json!("x".repeat(16 * 1024 * 1024 + 1));
    let lines: Vec<String> = vec![
        "not json".to_string(),
        "[1, 2]".to_string(),
        String::new(),
        no_time.to_string(),
        number_time.to_string(),
        big.to_string(),
        // The call is only on the oversized line, which is skipped.
        result_line(10, "big", Some(json!("user_temporary")), false).to_string(),
        // The call is on a later line.
        result_line(20, "late", Some(json!("user_temporary")), false).to_string(),
        call_line(30, "late", "Bash", json!({"command": "ls"})).to_string(),
        call_line(100, "q1", "AskUserQuestion", json!({})).to_string(),
        result_line(130, "q1", None, false).to_string(),
        typed_line(140, json!("hello")).to_string(),
    ];
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let events = read_events(&path).unwrap();
    let got: Vec<(EventKind, u64)> = events.iter().map(|e| (e.kind, e.wait_ms)).collect();
    assert_eq!(
        got,
        vec![(EventKind::Question(1), 30_000), (EventKind::Typed, 40_000)]
    );

    // An empty file: no events, no error.
    std::fs::write(&path, "").unwrap();
    assert!(read_events(&path).unwrap().is_empty());
}
