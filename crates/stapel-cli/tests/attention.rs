//! STP-29: the hooks `stop` and `session-end` append one attention record per call.

mod common;

use common::{git_config, git_in, repo_with_ticket, stapel};
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

#[test]
fn repeated_call_adds_nothing() {
    let elsewhere = tempfile::tempdir().unwrap();
    let dir = elsewhere.path();
    let path = events_transcript(dir, true);
    let repo = repo_with_ticket();
    let file = tokens_path(repo.path(), "ABC-1");

    // The first call writes the token record and the attention record.
    hook(repo.path(), dir, "session-end", &path);
    let first = std::fs::read_to_string(&file).unwrap();
    assert_eq!(records(repo.path(), "ABC-1").len(), 2);

    // The same input again: no new event, so the file stays byte for byte.
    hook(repo.path(), dir, "session-end", &path);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), first);

    // A second checkout of the repository (a linked worktree without the first one's journal lines):
    // the first call in `a` records the events, the same transcript in `b` records none of them again.
    let pair = common::two_ticket_worktrees();
    hook(&pair.a, dir, "session-end", &path);
    assert_eq!(attention_records(&records(&pair.a, "ABC-1")).len(), 1);
    hook(&pair.b, dir, "session-end", &path);
    for key in ["ABC-1", "ABC-2"] {
        let rs = records(&pair.b, key);
        assert!(attention_records(&rs).is_empty(), "{key}: {rs:?}");
    }
    // An event added after the first call is recorded once, in the second checkout.
    let mut text = std::fs::read_to_string(&path).unwrap();
    text += &(typed(130, "later").to_string() + "\n");
    std::fs::write(&path, text).unwrap();
    hook(&pair.b, dir, "session-end", &path);
    hook(&pair.b, dir, "session-end", &path);
    let rs = records(&pair.b, "ABC-2");
    let attention = attention_records(&rs);
    assert_eq!(attention.len(), 1, "{rs:?}");
    assert_eq!(attention[0]["typed"], 1, "{}", attention[0]);
    assert_eq!(attention[0]["dialogs"], 0, "{}", attention[0]);
    assert_eq!(attention[0]["questions"], 0, "{}", attention[0]);
    assert_eq!(attention[0]["from"], "2026-01-01T11:02:10.000Z", "{}", attention[0]);
    assert_eq!(attention_records(&records(&pair.a, "ABC-1")).len(), 1);
}

/// A planted attention record of the transcript `transcript` with `to` at `to`.
fn planted_attention(transcript: &str, to: &str) -> String {
    json!({
        "v": 1, "id": "a-planted", "at": "2026-01-01T11:30:00.000Z", "ticket": "ABC-1",
        "kind": "attention", "hook": "session-end", "transcript": transcript,
        "from": "2026-01-01T11:00:01.000Z", "to": to,
        "questions": 0, "dialogs": 1, "typed": 1, "waited_ms": 15_000, "attention_ms": 15_000, "long_waits": 0,
    })
    .to_string()
        + "\n"
}

#[test]
fn cursor_across_tickets() {
    let elsewhere = tempfile::tempdir().unwrap();
    let dir = elsewhere.path();
    // Events at 11:00:01 (typed), 11:00:20 (dialog), 11:01:25 (2 questions), 11:01:40 (dialog).
    let path = events_transcript(dir, false);

    // The cursor is in the closed ticket ABC-1: at event 2, then between the call and the result of event 3.
    for cursor in ["2026-01-01T11:00:20.000Z", "2026-01-01T11:00:50.000Z"] {
        let repo = repo_with_ticket();
        stapel(repo.path()).args(["new", "Second"]).assert().success();
        common::confirm_sections(repo.path(), "ABC-1");
        let closed = tokens_path(repo.path(), "ABC-1");
        let noise = [
            // Not an attention cursor: a token record of the transcript, later than every event.
            planted_token("2026-01-01T11:01:40.000Z", 60).trim_end().to_string(),
            "not json".to_string(),
            // Another transcript's attention record, later than every event.
            planted_attention("s-other", "2026-01-01T11:05:00.000Z").trim_end().to_string(),
            // A record of this transcript whose `to` is no time: no cursor.
            planted_attention("s-hook", "yesterday").trim_end().to_string(),
        ];
        let text = planted_attention("s-hook", cursor) + &noise.join("\n") + "\n";
        std::fs::write(&closed, text).unwrap();
        stapel(repo.path()).args(["close", "ABC-1", "--reason", "merged"]).assert().success();

        hook(repo.path(), dir, "session-end", &path);
        let rs = records(repo.path(), "ABC-2");
        assert_eq!(rs.len(), 1, "{cursor}: {rs:?}");
        let r = &rs[0];
        assert_eq!(r["kind"], "attention", "{cursor}: {r}");
        assert_eq!(r["ticket"], "ABC-2", "{cursor}: {r}");
        assert_eq!(r["typed"], 0, "{cursor}: {r}");
        assert_eq!(r["questions"], 2, "{cursor}: {r}");
        assert_eq!(r["dialogs"], 1, "{cursor}: {r}");
        assert_eq!(r["waited_ms"], 70_000, "{cursor}: {r}");
        assert_eq!(r["from"], "2026-01-01T11:01:25.000Z", "{cursor}: {r}");
        assert_eq!(r["to"], "2026-01-01T11:01:40.000Z", "{cursor}: {r}");
    }

    // The cursor is in another checkout of the repository: the first call ran in `a`, the second in `b`.
    let pair = common::two_ticket_worktrees();
    hook(&pair.a, dir, "session-end", &path);
    assert_eq!(attention_records(&records(&pair.a, "ABC-1")).len(), 1);
    let mut text = std::fs::read_to_string(&path).unwrap();
    text += &(typed(130, "later").to_string() + "\n");
    std::fs::write(&path, text).unwrap();
    hook(&pair.b, dir, "session-end", &path);
    let rs = records(&pair.b, "ABC-2");
    let attention = attention_records(&rs);
    assert_eq!(attention.len(), 1, "{rs:?}");
    assert_eq!(attention[0]["typed"], 1, "{}", attention[0]);
    assert_eq!(attention[0]["dialogs"], 0, "{}", attention[0]);
    assert_eq!(attention[0]["questions"], 0, "{}", attention[0]);
    assert_eq!(attention[0]["from"], "2026-01-01T11:02:10.000Z", "{}", attention[0]);
    assert!(attention_records(&records(&pair.b, "ABC-1")).is_empty());
}

#[test]
fn events_split_by_worktree() {
    let elsewhere = tempfile::tempdir().unwrap();
    let dir = elsewhere.path();
    let repo = repo_with_ticket();
    let root = repo.path().display().to_string();
    let with_cwd = |mut line: Value, cwd: Value| {
        line["cwd"] = cwd;
        line
    };
    let tree = |key: &str| json!(format!("{root}/.claude/worktrees/{key}/crates"));

    // Events of the closed ticket ABC-1 come first, the events of the rest later.
    let lines = vec![
        with_cwd(typed(1, "hello"), tree("ABC-1")),
        assistant(5, "m1", Some(("d1", "Bash", json!({"command": "ls"}))), true),
        with_cwd(result(20, "d1", "user_temporary"), tree("ABC-1")),
        assistant(25, "m2", Some(("q1", "AskUserQuestion", json!({"questions": [{"question": "a"}, {"question": "b"}]}))), true),
        // The main checkout: no worktree ticket.
        with_cwd(result(85, "q1", "user_temporary"), json!(root)),
        assistant(90, "m3", Some(("d2", "Bash", json!({"command": "pwd"}))), true),
        // A worktree of a key with no ticket: no worktree ticket.
        with_cwd(result(100, "d2", "user_permanent"), tree("ZZZ-9")),
        // No `cwd` at all, then a `cwd` that is a number.
        typed(105, "no folder"),
        with_cwd(typed(106, "a number"), json!(7)),
    ];
    let path = write(dir, "split.jsonl", &lines);

    stapel(repo.path()).args(["new", "Second"]).assert().success();
    common::confirm_sections(repo.path(), "ABC-1");
    let closed = tokens_path(repo.path(), "ABC-1");
    std::fs::write(&closed, "").unwrap();
    stapel(repo.path()).args(["close", "ABC-1", "--reason", "merged"]).assert().success();

    hook(repo.path(), dir, "session-end", &path);
    let first_1 = std::fs::read_to_string(&closed).unwrap();
    let first_2 = std::fs::read_to_string(tokens_path(repo.path(), "ABC-2")).unwrap();

    // ABC-1: the events of its worktree.
    let rs = records(repo.path(), "ABC-1");
    let attention = attention_records(&rs);
    assert_eq!(attention.len(), 1, "{rs:?}");
    let r = attention[0];
    assert_eq!(r["ticket"], "ABC-1", "{r}");
    assert_eq!(r["typed"], 1, "{r}");
    assert_eq!(r["dialogs"], 1, "{r}");
    assert_eq!(r["questions"], 0, "{r}");
    assert_eq!(r["waited_ms"], 15_000, "{r}");
    assert_eq!(r["from"], "2026-01-01T11:00:01.000Z", "{r}");
    assert_eq!(r["to"], "2026-01-01T11:00:20.000Z", "{r}");

    // ABC-2, the target ticket: the rest.
    let rs = records(repo.path(), "ABC-2");
    let attention = attention_records(&rs);
    assert_eq!(attention.len(), 1, "{rs:?}");
    let r = attention[0];
    assert_eq!(r["ticket"], "ABC-2", "{r}");
    assert_eq!(r["typed"], 2, "{r}");
    assert_eq!(r["dialogs"], 1, "{r}");
    assert_eq!(r["questions"], 2, "{r}");
    assert_eq!(r["from"], "2026-01-01T11:01:25.000Z", "{r}");
    assert_eq!(r["to"], "2026-01-01T11:01:46.000Z", "{r}");

    // The same call again: the cursor is the latest `to` of the attention records, so nothing is added.
    hook(repo.path(), dir, "session-end", &path);
    assert_eq!(std::fs::read_to_string(&closed).unwrap(), first_1);
    assert_eq!(std::fs::read_to_string(tokens_path(repo.path(), "ABC-2")).unwrap(), first_2);
}

#[test]
fn report_shows_the_attention_lines() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for args in [
        vec!["--role", "builder", "--input", "10", "--output", "20", "--cache-read", "0"],
        vec!["--role", "author", "--estimate", "5000"],
        vec!["--role", "builder", "--estimate", "70"],
    ] {
        let mut a = vec!["tokens", "add", "ABC-1"];
        a.extend(args);
        stapel(dir).args(&a).assert().success();
    }
    let main = dir.join("main.jsonl");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/main.jsonl"),
        &main,
    )
    .unwrap();
    stapel(dir)
        .args(["tokens", "import"])
        .arg(&main)
        .args(["--role", "orchestrator", "--until", "2026-01-02T00:00:00Z"])
        .assert()
        .success();
    let record = |id: &str, from: &str, to: &str, n: [u64; 6]| {
        json!({
            "v": 1, "id": id, "at": "2026-01-01T13:00:00.000Z", "ticket": "ABC-1",
            "kind": "attention", "hook": "session-end", "transcript": "s-report",
            "from": from, "to": to,
            "dialogs": n[0], "questions": n[1], "typed": n[2],
            "waited_ms": n[3], "attention_ms": n[4], "long_waits": n[5],
        })
        .to_string()
            + "\n"
    };
    let path = tokens_path(dir, "ABC-1");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text += &record(
        "a-one", "2026-01-01T11:00:00.000Z", "2026-01-01T11:40:00.000Z",
        [2, 1, 3, 1_500_000, 1_200_000, 1],
    );
    text += &record(
        "a-two", "2026-01-01T11:50:00.000Z", "2026-01-01T12:05:00.000Z",
        [1, 2, 1, 3_900_000, 900_000, 2],
    );
    std::fs::write(&path, text).unwrap();

    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/tokens_attention.txt"),
    )
    .unwrap();
    for args in [vec!["tokens", "ABC-1"], vec!["tokens"]] {
        let out = stapel(dir).args(&args).assert().success().get_output().stdout.clone();
        assert_eq!(String::from_utf8(out).unwrap(), golden, "{args:?}");
    }
}

#[test]
fn report_without_attention_is_unchanged() {
    let golden = |name: &str| {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(name))
            .unwrap()
    };
    let repo = repo_with_ticket();
    let dir = repo.path();
    for args in [
        vec!["--role", "builder", "--input", "10", "--output", "20", "--cache-read", "0"],
        vec!["--role", "author", "--estimate", "5000"],
        vec!["--role", "builder", "--estimate", "70"],
    ] {
        let mut a = vec!["tokens", "add", "ABC-1"];
        a.extend(args);
        stapel(dir).args(&a).assert().success();
    }
    let main = dir.join("main.jsonl");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/main.jsonl"),
        &main,
    )
    .unwrap();
    stapel(dir)
        .args(["tokens", "import"])
        .arg(&main)
        .args(["--role", "orchestrator", "--until", "2026-01-02T00:00:00Z"])
        .assert()
        .success();

    // One ticket, no attention record: the report is the old golden, with neither new line.
    let out = stapel(dir).args(["tokens", "ABC-1"]).assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text, golden("tokens_report.txt"));
    assert!(!text.contains("attention:") && !text.contains("steps:"), "{text}");

    // The same journal among other tickets: the all-tickets golden, with neither new line.
    for title in ["Second", "Third", "Fourth"] {
        stapel(dir).args(["new", title]).assert().success();
    }
    std::fs::write(
        dir.join(".stapel/tickets/ABC-2/tokens.jsonl"),
        concat!(
            r#"{"ts":"2026-10-02T18:00:00Z","ticket":"ABC-2","role":"reviewer","model":"claude-sonnet-5-5","total_tokens":50786,"kind":"measured"}"#, "\n",
            r#"{"ts":"2026-10-02T19:00:00Z","ticket":"ABC-2","role":"builder","model":"claude-opus-5-5","input_tokens":900000,"output_tokens":90000,"kind":"estimate"}"#, "\n",
            r#"{"ts":"2026-10-05T07:00:00Z","ticket":"ABC-2","role":"author","model":"claude-opus-5-5","input_tokens":null,"output_tokens":null,"kind":"estimate"}"#, "\n",
        ),
    )
    .unwrap();
    stapel(dir)
        .args(["tokens", "add", "ABC-3", "--role", "builder", "--estimate", "1"])
        .assert()
        .success();
    let mut journal = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join(".stapel/tickets/ABC-3/tokens.jsonl"))
        .unwrap();
    std::io::Write::write_all(&mut journal, b"{broken\n").unwrap();
    let out = stapel(dir).arg("tokens").assert().code(1).get_output().stdout.clone();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text, golden("tokens_report_all.txt"));
    assert!(!text.contains("attention:") && !text.contains("steps:"), "{text}");
}

#[test]
fn report_counts_steps_after_a_merge() {
    // Replaces the journal of ABC-1 with the given lines and returns the report of `stapel tokens ABC-1`.
    let report = |dir: &Path, lines: &[Value]| -> String {
        let text: String = lines.iter().map(|l| l.to_string() + "\n").collect();
        std::fs::write(tokens_path(dir, "ABC-1"), text).unwrap();
        let out = stapel(dir).args(["tokens", "ABC-1"]).assert().success().get_output().stdout.clone();
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains("problem:"), "{text}");
        text
    };
    let record = |id: &str, from: &str, to: &str| {
        json!({
            "v": 1, "id": id, "at": "2026-01-01T13:00:00.000Z", "ticket": "ABC-1",
            "kind": "attention", "hook": "session-end", "transcript": "s-steps",
            "from": from, "to": to,
            "dialogs": 1, "questions": 2, "typed": 3,
            "waited_ms": 120_000, "attention_ms": 60_000, "long_waits": 0,
        })
    };
    let steps_line = |text: &str| -> String {
        text.lines().find(|l| l.starts_with("steps:")).unwrap_or_else(|| panic!("{text}")).to_string()
    };
    let two = |a: (&str, &str), b: (&str, &str)| [record("a-one", a.0, a.1), record("a-two", b.0, b.1)];

    // A git repository whose HEAD has no commit yet: the steps cannot be read.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let records = two(
        ("2026-01-01T11:00:00.000Z", "2026-01-01T11:20:00.000Z"),
        ("2026-01-01T11:30:00.000Z", "2026-01-01T11:59:00.000Z"),
    );
    assert_eq!(steps_line(&report(dir, &records)), "steps: —; wall time: 59 min");

    // No git repository at all.
    let bare = repo_with_ticket();
    std::fs::remove_dir_all(bare.path().join(".git")).unwrap();
    assert_eq!(steps_line(&report(bare.path(), &records)), "steps: —; wall time: 59 min");

    // A git repository with a commit and no step commit: zero steps.
    git_config(dir, "user.email", "test-user.invalid");
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "-m", "base"]);
    let main_branch = git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).trim().to_string();
    assert_eq!(steps_line(&report(dir, &records)), "steps: 0; wall time: 59 min");

    // Two step labels on a side branch, brought into HEAD by a merge commit; the first label has
    // both its RED and its GREEN commit, and a commit of another ticket does not count.
    git_in(dir, &["checkout", "-q", "-b", "side"]);
    for subject in ["ABC-1 1 RED: one", "ABC-1 1 GREEN: one", "ABC-1 2 RED: two", "ABC-2 3 RED: other"] {
        git_in(dir, &["commit", "-q", "--allow-empty", "-m", subject]);
    }
    git_in(dir, &["checkout", "-q", &main_branch]);
    git_in(dir, &["commit", "-q", "--allow-empty", "-m", "main moves on"]);
    git_in(dir, &["merge", "-q", "--no-ff", "-m", "Merge side", "side"]);
    let parents = git_in(dir, &["log", "-1", "--format=%p"]);
    assert_eq!(parents.split_whitespace().count(), 2, "{parents}");
    assert_eq!(steps_line(&report(dir, &records)), "steps: 2; wall time: 59 min");

    // Wall time from the earliest `from` to the latest `to`, whatever the order of the records.
    let records = two(
        ("2026-01-01T12:01:00.000Z", "2026-01-01T12:10:00.000Z"),
        ("2026-01-01T11:00:00.000Z", "2026-01-01T11:20:00.000Z"),
    );
    assert_eq!(steps_line(&report(dir, &records)), "steps: 2; wall time: 1 h 10 min");
    let records = two(
        ("2026-01-01T11:00:00.000Z", "2026-01-01T11:20:00.000Z"),
        ("2026-01-01T11:30:00.000Z", "2026-01-01T12:01:00.000Z"),
    );
    assert_eq!(steps_line(&report(dir, &records)), "steps: 2; wall time: 1 h 1 min");

    // D4-1: exactly two hours.
    let records = two(
        ("2026-01-01T11:00:00.000Z", "2026-01-01T11:20:00.000Z"),
        ("2026-01-01T11:30:00.000Z", "2026-01-01T13:00:00.000Z"),
    );
    assert_eq!(steps_line(&report(dir, &records)), "steps: 2; wall time: 2 h 0 min");

    // D4-2: absent numbers and string numbers count as 0, with no problem line.
    let odd = json!({
        "v": 1, "id": "a-odd", "at": "2026-01-01T13:00:00.000Z", "ticket": "ABC-1",
        "kind": "attention", "hook": "stop", "transcript": "s-steps",
        "from": "2026-01-01T11:05:00.000Z", "to": "2026-01-01T11:10:00.000Z",
        "dialogs": "5", "waited_ms": "9",
    });
    let text = report(dir, &[record("a-one", "2026-01-01T11:00:00.000Z", "2026-01-01T11:20:00.000Z"), odd]);
    assert!(
        text.contains("attention: 1 dialogs, 2 questions, 3 messages; 1 min attention, 2 min waited (0 over 10 min)"),
        "{text}"
    );
    assert_eq!(steps_line(&text), "steps: 2; wall time: 20 min");
}

/// One assistant message line with a model, a session, an optional agent and the four counts.
fn model_line(sec: u32, id: &str, model: &str, session: &str, agent: Option<&str>, n: [u64; 4]) -> Value {
    let mut line = json!({
        "type": "assistant", "timestamp": time(sec), "sessionId": session, "isSidechain": agent.is_some(),
        "message": {
            "id": id, "model": model, "role": "assistant", "content": [{"type": "text", "text": "done"}],
            "usage": {
                "input_tokens": n[0], "output_tokens": n[1],
                "cache_read_input_tokens": n[2], "cache_creation_input_tokens": n[3],
            },
            "stop_reason": "end_turn",
        },
    });
    if let Some(agent) = agent {
        line["agentId"] = json!(agent);
    }
    line
}

/// The main transcript of `events_transcript` with a message of a second model at 11:00:30; the
/// last message of the file has no final line when `with_last_message` is set.
fn measure_main(dir: &Path, with_last_message: bool) -> PathBuf {
    let h = "claude-haiku-4-5-20251001";
    let mut lines = vec![
        typed(1, "hello"),
        assistant(5, "m1", Some(("d1", "Bash", json!({"command": "ls"}))), true),
        result(20, "d1", "user_temporary"),
        assistant(25, "m2", Some(("q1", "AskUserQuestion", json!({"questions": [{"question": "a"}, {"question": "b"}]}))), true),
        model_line(30, "h1", h, "s-hook", None, [11, 22, 33, 44]),
        result(85, "q1", "user_temporary"),
        assistant(90, "m3", Some(("d2", "Bash", json!({"command": "pwd"}))), true),
        result(100, "d2", "user_permanent"),
    ];
    if with_last_message {
        lines.push(assistant(110, "m4", None, false));
    }
    write(dir, "main.jsonl", &lines)
}

/// `<dir>/s-hook/subagents` with two files of the session, one of another session and a note.
fn measure_subagents(dir: &Path) {
    let h = "claude-haiku-4-5-20251001";
    let folder = dir.join("s-hook/subagents");
    std::fs::create_dir_all(&folder).unwrap();
    let file = |name: &str, lines: Vec<Value>| {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(folder.join(name), text).unwrap();
    };
    file(
        "agent-a1.jsonl",
        vec![model_line(3000, "x1", S, "s-hook", Some("agent-a1"), [10, 20, 30, 40])],
    );
    file(
        "agent-a2.jsonl",
        vec![
            model_line(3001, "x2", S, "s-hook", Some("agent-a2"), [100, 200, 300, 400]),
            model_line(3002, "x3", h, "s-hook", Some("agent-a2"), [1000, 2000, 3000, 4000]),
        ],
    );
    file(
        "agent-b1.jsonl",
        vec![model_line(3003, "x4", S, "s-other", Some("agent-b1"), [7, 7, 7, 7])],
    );
    std::fs::write(folder.join("notes.txt"), "not a transcript\n").unwrap();
}

fn golden_measure() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/tokens_measure.txt")).unwrap()
}

#[test]
fn measure_prints_tokens_and_attention() {
    let dir = common::bare_dir();
    let dir = dir.path();
    let main = measure_main(dir, true);
    measure_subagents(dir);
    let before = common::snapshot(dir);
    let out = stapel(dir).args(["tokens", "measure"]).arg(&main).assert().success().get_output().clone();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), golden_measure());
    assert_eq!(out.stderr, b"");
    assert_eq!(common::snapshot(dir), before);

    // A relative path and a link to the file give the same output.
    stapel(dir)
        .args(["tokens", "measure", "main.jsonl"])
        .assert()
        .success()
        .stdout(golden_measure());
    let linked = tempfile::tempdir().unwrap();
    let link = linked.path().join("link.jsonl");
    std::os::unix::fs::symlink(&main, &link).unwrap();
    let text = stapel(dir).args(["tokens", "measure"]).arg(&link).assert().success().get_output().stdout.clone();
    assert!(String::from_utf8(text).unwrap().starts_with("transcript: s-hook\n"));

    // Without a subagent folder: the main rows only; a final last message has no `≥`.
    let alone = tempfile::tempdir().unwrap();
    let main = measure_main(alone.path(), false);
    let text = stapel(alone.path()).args(["tokens", "measure"]).arg(&main).assert().success().get_output().stdout.clone();
    let text = String::from_utf8(text).unwrap();
    assert!(!text.contains("subagents"), "{text}");
    assert!(!text.contains('≥'), "{text}");
    assert_eq!(text.lines().filter(|l| l.starts_with("main ")).count(), 2, "{text}");
    let sonnet = text.lines().find(|l| l.contains(S)).unwrap();
    let cells: Vec<&str> = sonnet.split_whitespace().collect();
    assert_eq!(cells, ["main", S, "3", "6", "9", "12"], "{text}");
    assert!(text.ends_with("wall time: 1 min\n"), "{text}");

    // One timed line: no time passes.
    let one = tempfile::tempdir().unwrap();
    let path = write(one.path(), "one.jsonl", &[]);
    let text = stapel(one.path()).args(["tokens", "measure"]).arg(&path).assert().success().get_output().stdout.clone();
    let text = String::from_utf8(text).unwrap();
    assert!(text.starts_with("transcript: s-hook\n"), "{text}");
    assert!(text.ends_with("wall time: 0 min\n"), "{text}");
}

#[test]
fn measure_outside_a_repository() {
    // A folder that is neither a stapel nor a git repository.
    let dir = common::bare_dir();
    let dir = dir.path();
    let main = measure_main(dir, true);
    measure_subagents(dir);
    let before = common::snapshot(dir);
    stapel(dir)
        .args(["tokens", "measure"])
        .arg(&main)
        .assert()
        .code(0)
        .stdout(golden_measure())
        .stderr("");
    assert_eq!(common::snapshot(dir), before);
}

#[test]
fn measure_refuses_bad_files() {
    use std::os::unix::fs::PermissionsExt;
    let dir = common::bare_dir();
    let dir = dir.path();
    let missing = dir.join("missing.jsonl");
    let folder = dir.join("folder");
    std::fs::create_dir(&folder).unwrap();
    let fifo = dir.join("fifo.jsonl");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let no_session = dir.join("no-session.jsonl");
    std::fs::write(&no_session, "{\"type\":\"user\",\"timestamp\":\"2026-01-01T11:00:00.000Z\"}\n").unwrap();
    let subagent = dir.join("subagent.jsonl");
    let line = model_line(5, "s1", S, "s-hook", Some("agent-a1"), [1, 2, 3, 4]);
    std::fs::write(&subagent, format!("{line}\n")).unwrap();
    let locked = dir.join("locked.jsonl");
    std::fs::write(&locked, "{\"sessionId\":\"s\"}\n").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let mut cases = vec![
        (missing, "missing"),
        (folder, "not a file"),
        (fifo, "not a file"),
        (no_session, "no sessionId"),
        (subagent, "subagent"),
    ];
    // A process that may read any file cannot make a file unreadable.
    if std::fs::File::open(&locked).is_err() {
        cases.push((locked, "not readable"));
    }
    for (path, reason) in &cases {
        let out = stapel(dir)
            .args(["tokens", "measure"])
            .arg(path)
            .timeout(std::time::Duration::from_secs(20))
            .assert()
            .code(2)
            .get_output()
            .clone();
        assert_eq!(out.stdout, b"", "{path:?}");
        let err = String::from_utf8(out.stderr).unwrap();
        assert_eq!(err.trim_end().lines().count(), 1, "{path:?}: {err}");
        assert!(err.contains(path.to_str().unwrap()), "{path:?}: {err}");
        assert!(err.contains(reason), "{path:?} {reason}: {err}");
    }

    // No argument: the usage text.
    let out = stapel(dir).args(["tokens", "measure"]).assert().code(2).get_output().clone();
    assert_eq!(out.stdout, b"");
    assert!(String::from_utf8(out.stderr).unwrap().contains("Usage"));
}

#[test]
fn missing_subagent_transcript_writes_nothing() {
    fn subagent_stop(repo: &Path, elsewhere: &Path, path: &Path) {
        let input = json!({
            "hook_event_name": "SubagentStop",
            "session_id": "s-hook",
            "cwd": elsewhere,
            "agent_type": "stapel-builder",
            "agent_transcript_path": path,
        });
        stapel(elsewhere)
            .env("CLAUDE_PROJECT_DIR", repo)
            .args(["hook", "subagent-stop"])
            .write_stdin(input.to_string())
            .assert()
            .success()
            .stdout("");
    }
    let elsewhere = tempfile::tempdir().unwrap();
    let dir = elsewhere.path();
    let repo = repo_with_ticket();

    // A missing path: no record, no file changed.
    let before = std::fs::read(tokens_path(repo.path(), "ABC-1")).ok();
    let status = git_in(repo.path(), &["status", "--porcelain"]);
    subagent_stop(repo.path(), dir, &dir.join("missing.jsonl"));
    assert_eq!(std::fs::read(tokens_path(repo.path(), "ABC-1")).ok(), before);
    assert!(records(repo.path(), "ABC-1").is_empty());
    assert_eq!(git_in(repo.path(), &["status", "--porcelain"]), status);

    // The same input with an existing file writes records.
    let mut lines = vec![assistant(5, "m1", None, true)];
    lines[0]["agentId"] = json!("agent-hook");
    lines[0]["isSidechain"] = json!(true);
    let path = write(dir, "sub.jsonl", &lines);
    subagent_stop(repo.path(), dir, &path);
    assert!(!records(repo.path(), "ABC-1").is_empty());
}

#[test]
fn bad_shared_cursor_file() {
    let elsewhere = tempfile::tempdir().unwrap();
    let dir = elsewhere.path();
    // Events at 11:00:01 (typed), 11:00:20 (dialog), 11:01:25 (2 questions), 11:01:40 (dialog).
    let path = events_transcript(dir, false);
    // 2026-01-01T11:00:00Z in milliseconds.
    let base_ms: u64 = 1_767_265_200_000;

    let shared_file = |repo: &Path| {
        let common = git_in(repo, &["rev-parse", "--git-common-dir"]);
        let common = repo.join(common.trim());
        std::fs::create_dir_all(common.join("stapel")).unwrap();
        common.join("stapel/attention-cursors.json")
    };

    // The file is no JSON, an array, or holds no number for this transcript: no value, every event is recorded.
    for content in ["not json".to_string(), "[]".to_string(), json!({"s-hook": "x"}).to_string()] {
        let repo = repo_with_ticket();
        std::fs::write(shared_file(repo.path()), &content).unwrap();
        hook(repo.path(), dir, "session-end", &path);
        let rs = records(repo.path(), "ABC-1");
        let attention = attention_records(&rs);
        assert_eq!(attention.len(), 1, "{content}: {rs:?}");
        check_numbers(attention[0]);
    }

    // A shared value after the second event and no journal record: only the later events are recorded.
    let repo = repo_with_ticket();
    std::fs::write(shared_file(repo.path()), json!({"s-hook": base_ms + 50_000}).to_string()).unwrap();
    hook(repo.path(), dir, "session-end", &path);
    let rs = records(repo.path(), "ABC-1");
    let attention = attention_records(&rs);
    assert_eq!(attention.len(), 1, "{rs:?}");
    let r = attention[0];
    assert_eq!(r["typed"], 0, "{r}");
    assert_eq!(r["questions"], 2, "{r}");
    assert_eq!(r["dialogs"], 1, "{r}");
    assert_eq!(r["from"], "2026-01-01T11:01:25.000Z", "{r}");
    assert_eq!(r["to"], "2026-01-01T11:01:40.000Z", "{r}");

    // The journal's `to` is later than the shared value: the journal wins.
    let repo = repo_with_ticket();
    std::fs::write(shared_file(repo.path()), json!({"s-hook": base_ms + 20_000}).to_string()).unwrap();
    let file = tokens_path(repo.path(), "ABC-1");
    let text = std::fs::read_to_string(&file).unwrap_or_default() + &planted_attention("s-hook", "2026-01-01T11:00:50.000Z");
    std::fs::write(&file, text).unwrap();
    hook(repo.path(), dir, "session-end", &path);
    let rs = records(repo.path(), "ABC-1");
    let attention = attention_records(&rs);
    assert_eq!(attention.len(), 2, "{rs:?}");
    let r = attention[1];
    assert_eq!(r["typed"], 0, "{r}");
    assert_eq!(r["questions"], 2, "{r}");
    assert_eq!(r["dialogs"], 1, "{r}");
    assert_eq!(r["from"], "2026-01-01T11:01:25.000Z", "{r}");
    assert_eq!(r["to"], "2026-01-01T11:01:40.000Z", "{r}");
}
