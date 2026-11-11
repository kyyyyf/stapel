//! STP-29: the hooks `stop` and `session-end` append one attention record per call.

mod common;

use common::{repo_with_ticket, stapel};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const S: &str = "claude-sonnet-5-5";

fn records(dir: &Path, key: &str) -> Vec<Value> {
    std::fs::read_to_string(dir.join(format!(".stapel/tickets/{key}/tokens.jsonl")))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn tokens_path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!(".stapel/tickets/{key}/tokens.jsonl"))
}

fn attention_records(rs: &[Value]) -> Vec<&Value> {
    rs.iter().filter(|r| r["kind"] == "attention").collect()
}

fn time(sec: u32) -> String {
    format!("2026-01-01T11:{:02}:{:02}.000Z", sec / 60, sec % 60)
}

/// A main-transcript line of the assistant: one message with a tool call (or none), final or not.
fn assistant(sec: u32, id: &str, call: Option<(&str, &str, Value)>, last: bool) -> Value {
    let content = match call {
        Some((call_id, name, input)) => json!([{"type": "tool_use", "id": call_id, "name": name, "input": input}]),
        None => json!([{"type": "text", "text": "done"}]),
    };
    let mut message = json!({
        "id": id, "model": S, "role": "assistant", "content": content,
        "usage": {"input_tokens": 1, "output_tokens": 2, "cache_read_input_tokens": 3, "cache_creation_input_tokens": 4},
    });
    if last {
        message["stop_reason"] = json!("end_turn");
    }
    json!({"type": "assistant", "timestamp": time(sec), "sessionId": "s-hook", "isSidechain": false, "message": message})
}

fn result(sec: u32, call_id: &str, source: &str) -> Value {
    json!({
        "type": "user", "timestamp": time(sec), "sessionId": "s-hook", "isSidechain": false,
        "permissionDecision": {"decision": "allow", "source": source},
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": call_id, "content": "ok"}]},
    })
}

fn typed(sec: u32, text: &str) -> Value {
    json!({
        "type": "user", "timestamp": time(sec), "sessionId": "s-hook", "isSidechain": false,
        "message": {"role": "user", "content": text},
    })
}

fn write(dir: &Path, name: &str, lines: &[Value]) -> PathBuf {
    let mut text = String::from("{\"type\":\"user\",\"timestamp\":\"2026-01-01T11:00:00.000Z\",\"sessionId\":\"s-hook\"}\n");
    for line in lines {
        text.push_str(&line.to_string());
        text.push('\n');
    }
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

/// A typed message at 11:00:01, a dialog (15 s), a question of 2 (60 s) and a second dialog (10 s).
/// The last message has no final line, so the last event comes after the last complete message.
fn events_transcript(dir: &Path, with_last_message: bool) -> PathBuf {
    let mut lines = vec![
        typed(1, "hello"),
        assistant(5, "m1", Some(("d1", "Bash", json!({"command": "ls"}))), true),
        result(20, "d1", "user_temporary"),
        assistant(25, "m2", Some(("q1", "AskUserQuestion", json!({"questions": [{"question": "a"}, {"question": "b"}]}))), true),
        result(85, "q1", "user_temporary"),
        assistant(90, "m3", Some(("d2", "Bash", json!({"command": "pwd"}))), true),
        result(100, "d2", "user_permanent"),
    ];
    if with_last_message {
        lines.push(assistant(110, "m4", None, false));
    }
    write(dir, "main.jsonl", &lines)
}

fn hook(repo: &Path, elsewhere: &Path, name: &str, path: &Path) {
    let input = json!({
        "hook_event_name": if name == "stop" { "Stop" } else { "SessionEnd" },
        "session_id": "s-hook",
        "cwd": elsewhere,
        "transcript_path": path,
    });
    stapel(elsewhere)
        .env("CLAUDE_PROJECT_DIR", repo)
        .args(["hook", name])
        .write_stdin(input.to_string())
        .assert()
        .success()
        .stdout("");
}

/// A planted token record of the transcript `s-hook` for model `S`, made `age_secs` ago.
fn planted_token(to: &str, age_secs: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    json!({
        "v": 1, "id": "t-planted", "at": stapel_core::time::rfc3339(now - age_secs), "ticket": "ABC-1",
        "role": "orchestrator", "model": S, "source": "measured", "transcript": "s-hook", "input": 1,
        "from": "2026-01-01T11:00:05.000Z", "to": to, "messages": 1,
    })
    .to_string()
        + "\n"
}

/// The numbers of `events_transcript`.
fn check_numbers(r: &Value) {
    assert_eq!(r["v"], 1, "{r}");
    assert!(r["id"].as_str().unwrap().starts_with("a-"), "{r}");
    assert_eq!(r["ticket"], "ABC-1", "{r}");
    assert_eq!(r["transcript"], "s-hook", "{r}");
    assert_eq!(r["questions"], 2, "{r}");
    assert_eq!(r["dialogs"], 2, "{r}");
    assert_eq!(r["typed"], 1, "{r}");
    assert_eq!(r["waited_ms"], 85_000, "{r}");
    assert_eq!(r["attention_ms"], 85_000, "{r}");
    assert_eq!(r["long_waits"], 0, "{r}");
    assert_eq!(r["from"], "2026-01-01T11:00:01.000Z", "{r}");
    assert_eq!(r["to"], "2026-01-01T11:01:40.000Z", "{r}");
    for absent in ["role", "model", "source"] {
        assert!(r.get(absent).is_none(), "{r}");
    }
}

#[test]
fn stop_appends_one_record() {
    let elsewhere = tempfile::tempdir().unwrap();
    let dir = elsewhere.path();
    let path = events_transcript(dir, true);

    // No record yet: the token record and one attention record; the last event follows the last complete message.
    let repo = repo_with_ticket();
    hook(repo.path(), dir, "stop", &path);
    let rs = records(repo.path(), "ABC-1");
    assert_eq!(rs.len(), 2, "{rs:?}");
    assert_eq!(rs.iter().filter(|r| r["role"] == "orchestrator").count(), 1, "{rs:?}");
    let attention = attention_records(&rs);
    assert_eq!(attention.len(), 1, "{rs:?}");
    assert_eq!(attention[0]["hook"], "stop", "{}", attention[0]);
    check_numbers(attention[0]);

    // A record of the transcript 9 minutes old: nothing.
    let repo = repo_with_ticket();
    let file = tokens_path(repo.path(), "ABC-1");
    let text = planted_token("2026-01-01T11:00:10.000Z", 9 * 60);
    std::fs::write(&file, &text).unwrap();
    hook(repo.path(), dir, "stop", &path);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);

    // No event: the token record only.
    let quiet = write(dir, "quiet.jsonl", &[assistant(5, "m1", None, true)]);
    let repo = repo_with_ticket();
    hook(repo.path(), dir, "stop", &quiet);
    let rs = records(repo.path(), "ABC-1");
    assert_eq!(rs.len(), 1, "{rs:?}");
    assert!(attention_records(&rs).is_empty(), "{rs:?}");

    // No complete message and one event: an attention record alone.
    let one = write(dir, "one.jsonl", &[typed(1, "hello")]);
    let repo = repo_with_ticket();
    hook(repo.path(), dir, "stop", &one);
    let rs = records(repo.path(), "ABC-1");
    assert_eq!(rs.len(), 1, "{rs:?}");
    assert_eq!(rs[0]["kind"], "attention", "{}", rs[0]);
    assert_eq!(rs[0]["hook"], "stop", "{}", rs[0]);
    assert_eq!(rs[0]["typed"], 1, "{}", rs[0]);
    assert_eq!(rs[0]["questions"], 0, "{}", rs[0]);
    assert_eq!(rs[0]["dialogs"], 0, "{}", rs[0]);
    assert_eq!(rs[0]["from"], "2026-01-01T11:00:01.000Z", "{}", rs[0]);
    assert_eq!(rs[0]["to"], "2026-01-01T11:00:01.000Z", "{}", rs[0]);
}

#[test]
fn session_end_appends_the_rest() {
    let elsewhere = tempfile::tempdir().unwrap();
    let dir = elsewhere.path();

    // A token record 1 minute old and new events: the hook appends the rest, with its attention record.
    let path = events_transcript(dir, true);
    let repo = repo_with_ticket();
    let file = tokens_path(repo.path(), "ABC-1");
    let text = planted_token("2026-01-01T11:00:10.000Z", 60);
    std::fs::write(&file, &text).unwrap();
    hook(repo.path(), dir, "session-end", &path);
    let rs = records(repo.path(), "ABC-1");
    assert_eq!(rs.len(), 3, "{rs:?}");
    assert_eq!(rs[0]["id"], "t-planted", "{rs:?}");
    assert_eq!(rs.iter().filter(|r| r["hook"] == "session-end" && r["role"] == "orchestrator").count(), 1, "{rs:?}");
    let attention = attention_records(&rs);
    assert_eq!(attention.len(), 1, "{rs:?}");
    assert_eq!(attention[0]["hook"], "session-end", "{}", attention[0]);
    check_numbers(attention[0]);

    // Events and no new message: an attention record alone.
    let path = events_transcript(dir, false);
    let repo = repo_with_ticket();
    let file = tokens_path(repo.path(), "ABC-1");
    std::fs::write(&file, planted_token("2026-01-01T11:01:30.000Z", 60)).unwrap();
    hook(repo.path(), dir, "session-end", &path);
    let rs = records(repo.path(), "ABC-1");
    assert_eq!(rs.len(), 2, "{rs:?}");
    assert_eq!(rs[1]["kind"], "attention", "{}", rs[1]);
    assert_eq!(rs[1]["hook"], "session-end", "{}", rs[1]);
    check_numbers(&rs[1]);
}

#[test]
fn subagent_stop_has_no_attention() {
    let elsewhere = tempfile::tempdir().unwrap();
    let dir = elsewhere.path();

    // A subagent transcript: a call and its result of source `user_temporary`, every line with an agent id.
    let mut lines = vec![
        assistant(5, "m1", Some(("d1", "Bash", json!({"command": "ls"}))), true),
        result(20, "d1", "user_temporary"),
        assistant(25, "m2", None, true),
    ];
    for line in &mut lines {
        line["agentId"] = json!("agent-hook");
        line["isSidechain"] = json!(true);
    }
    let mut text = String::new();
    for line in &lines {
        text.push_str(&line.to_string());
        text.push('\n');
    }
    let path = dir.join("sub.jsonl");
    std::fs::write(&path, text).unwrap();

    // The subagent-stop hook: the token records, no attention record.
    let repo = repo_with_ticket();
    let input = json!({
        "hook_event_name": "SubagentStop",
        "session_id": "s-hook",
        "cwd": dir,
        "agent_type": "stapel-builder",
        "agent_transcript_path": path,
    });
    stapel(dir)
        .env("CLAUDE_PROJECT_DIR", repo.path())
        .args(["hook", "subagent-stop"])
        .write_stdin(input.to_string())
        .assert()
        .success()
        .stdout("");
    let rs = records(repo.path(), "ABC-1");
    assert!(!rs.is_empty(), "{rs:?}");
    assert!(rs.iter().all(|r| r["hook"] == "subagent-stop"), "{rs:?}");
    assert!(attention_records(&rs).is_empty(), "{rs:?}");

    // The session-end hook given the same subagent transcript (an identity with an agent part): no attention record.
    let repo = repo_with_ticket();
    hook(repo.path(), dir, "session-end", &path);
    let rs = records(repo.path(), "ABC-1");
    assert!(!rs.is_empty(), "{rs:?}");
    assert!(attention_records(&rs).is_empty(), "{rs:?}");
}
