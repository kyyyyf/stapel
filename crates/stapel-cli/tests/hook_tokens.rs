//! STP-26: the token hooks of Claude Code write `tokens.jsonl` records.

mod common;

use common::{repo_with_ticket, stapel};
use std::path::{Path, PathBuf};

fn records(dir: &Path, key: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join(format!(".stapel/tickets/{key}/tokens.jsonl")))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// One streamed line of a subagent transcript: (message id, model, time, [in, out, read, write], final).
type Msg<'a> = (&'a str, &'a str, &'a str, [u64; 4], bool);

fn transcript(dir: &Path, name: &str, messages: &[Msg]) -> PathBuf {
    let mut text = String::from(
        "{\"type\":\"user\",\"timestamp\":\"2026-01-01T11:00:00.000Z\",\"sessionId\":\"s-hook\",\"agentId\":\"agent-hook\",\"isSidechain\":true}\n",
    );
    for (id, model, time, u, last) in messages {
        let stop = if *last { ",\"stop_reason\":\"end_turn\"" } else { "" };
        text.push_str(&format!(
            "{{\"type\":\"assistant\",\"timestamp\":\"{time}\",\"sessionId\":\"s-hook\",\"agentId\":\"agent-hook\",\"isSidechain\":true,\"message\":{{\"id\":\"{id}\",\"model\":\"{model}\",\"usage\":{{\"input_tokens\":{},\"output_tokens\":{},\"cache_read_input_tokens\":{},\"cache_creation_input_tokens\":{}}}{stop}}}}}\n",
            u[0], u[1], u[2], u[3]
        ));
    }
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

/// Runs `stapel hook subagent-stop` with the root named by `CLAUDE_PROJECT_DIR` and `cwd` at `elsewhere`.
fn subagent_stop(repo: &Path, elsewhere: &Path, agent_type: &str, path: &Path) -> assert_cmd::assert::Assert {
    let input = serde_json::json!({
        "hook_event_name": "SubagentStop",
        "session_id": "s-hook",
        "cwd": elsewhere,
        "agent_type": agent_type,
        "agent_transcript_path": path,
        "other": "ignored",
    });
    stapel(elsewhere)
        .env("CLAUDE_PROJECT_DIR", repo)
        .args(["hook", "subagent-stop"])
        .write_stdin(input.to_string())
        .assert()
}

fn by_model<'a>(rs: &'a [serde_json::Value], model: &str) -> &'a serde_json::Value {
    rs.iter().find(|r| r["model"] == model).unwrap()
}

#[test]
fn subagent_stop_records_each_model() {
    let repo = repo_with_ticket();
    let elsewhere = tempfile::tempdir().unwrap();
    let path = transcript(
        elsewhere.path(),
        "sub.jsonl",
        &[
            ("m1", "claude-sonnet-5-5", "2026-01-01T11:00:05.000Z", [10, 20, 100, 5], true),
            ("m2", "claude-sonnet-5-5", "2026-01-01T11:00:10.000Z", [1, 2, 3, 4], true),
            ("m3", "claude-sonnet-5-5", "2026-01-01T11:00:15.000Z", [5, 6, 7, 8], true),
            ("m4", "claude-opus-5-5", "2026-01-01T11:00:20.000Z", [2, 9, 50, 0], true),
        ],
    );
    subagent_stop(repo.path(), elsewhere.path(), "stapel-builder", &path)
        .success()
        .stdout("");
    let rs = records(repo.path(), "ABC-1");
    assert_eq!(rs.len(), 2, "{rs:?}");
    let s = by_model(&rs, "claude-sonnet-5-5");
    assert_eq!(s["role"], "builder", "{s}");
    assert_eq!(s["hook"], "subagent-stop", "{s}");
    assert_eq!(s["source"], "measured", "{s}");
    assert_eq!(s["ticket"], "ABC-1", "{s}");
    assert_eq!(s["messages"], 3, "{s}");
    assert_eq!(s["input"], 16, "{s}");
    assert_eq!(s["output"], 28, "{s}");
    assert_eq!(s["cache_read"], 110, "{s}");
    assert_eq!(s["cache_write"], 17, "{s}");
    assert_eq!(s["from"], "2026-01-01T11:00:05.000Z", "{s}");
    assert_eq!(s["to"], "2026-01-01T11:00:15.000Z", "{s}");
    assert_eq!(s["transcript"], "s-hook/agent-hook", "{s}");
    assert_eq!(s["importer"], 2, "{s}");
    assert!(s.get("partial").is_none(), "{s}");
    let o = by_model(&rs, "claude-opus-5-5");
    assert_eq!(o["role"], "builder", "{o}");
    assert_eq!(o["messages"], 1, "{o}");
    assert_eq!(o["output"], 9, "{o}");
    assert_eq!(o["hook"], "subagent-stop", "{o}");

    // D2-1: no CLAUDE_PROJECT_DIR, the input's cwd at the repository, the process elsewhere.
    let repo2 = repo_with_ticket();
    let path2 = transcript(
        elsewhere.path(),
        "sub2.jsonl",
        &[("n1", "claude-sonnet-5-5", "2026-01-01T11:00:05.000Z", [1, 2, 3, 4], true)],
    );
    let input = serde_json::json!({
        "hook_event_name": "SubagentStop",
        "session_id": "s-hook",
        "cwd": repo2.path(),
        "agent_type": "stapel-builder",
        "agent_transcript_path": path2,
    });
    stapel(elsewhere.path())
        .env_remove("CLAUDE_PROJECT_DIR")
        .args(["hook", "subagent-stop"])
        .write_stdin(input.to_string())
        .assert()
        .success()
        .stdout("");
    let rs2 = records(repo2.path(), "ABC-1");
    assert_eq!(rs2.len(), 1, "{rs2:?}");
    assert_eq!(rs2[0]["role"], "builder", "{}", rs2[0]);
}

#[test]
fn partial_messages_as_import() {
    let repo = repo_with_ticket();
    let elsewhere = tempfile::tempdir().unwrap();
    let path = transcript(
        elsewhere.path(),
        "sub.jsonl",
        &[
            ("m1", "claude-sonnet-5-5", "2026-01-01T11:00:05.000Z", [10, 1500, 100, 20], true),
            ("m2", "claude-sonnet-5-5", "2026-01-01T11:01:00.000Z", [2, 4, 200, 0], false),
        ],
    );
    subagent_stop(repo.path(), elsewhere.path(), "stapel-reviewer", &path)
        .success()
        .stdout("");
    let rs = records(repo.path(), "ABC-1");
    assert_eq!(rs.len(), 1, "{rs:?}");
    let r = &rs[0];
    assert_eq!(r["role"], "reviewer", "{r}");
    assert_eq!(r["partial"], 1, "{r}");
    assert_eq!(r["messages"], 2, "{r}");
    assert_eq!(r["output"], 1500, "{r}");
    assert_eq!(r["input"], 12, "{r}");
    assert_eq!(r["cache_read"], 300, "{r}");
}

const DEFAULT_ROLES: &str = "[roles]\nstapel-builder = \"builder\"\nstapel-drift = \"drift\"\nstapel-reviewer = \"reviewer\"\nstapel-spec-reviewer = \"reviewer\"\n";

/// The one record that `subagent-stop` writes for `agent_type` in a repository whose `[roles]`
/// table is replaced by `roles` (an empty text removes the table).
fn one_record(roles: &str, agent_type: &str) -> serde_json::Value {
    let repo = repo_with_ticket();
    let config = repo.path().join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    assert!(text.contains(DEFAULT_ROLES), "{text}");
    std::fs::write(&config, text.replacen(DEFAULT_ROLES, roles, 1)).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let path = transcript(
        elsewhere.path(),
        "sub.jsonl",
        &[("m1", "claude-sonnet-5-5", "2026-01-01T11:00:05.000Z", [1, 2, 3, 4], true)],
    );
    subagent_stop(repo.path(), elsewhere.path(), agent_type, &path)
        .success()
        .stdout("");
    let mut rs = records(repo.path(), "ABC-1");
    assert_eq!(rs.len(), 1, "{rs:?}");
    rs.remove(0)
}

#[test]
fn roles_from_the_table() {
    let table = "[roles]\nExplore = \"research2\"\n";
    let r = one_record(table, "Explore");
    assert_eq!(r["role"], "research2", "{r}");
    assert!(r.get("note").is_none(), "{r}");
    // The default rows do not apply when the table exists.
    let r = one_record(table, "stapel-builder");
    assert_eq!(r["role"], "research", "{r}");
    assert_eq!(r["note"], "agent type stapel-builder", "{r}");
    // An empty type has no row.
    let r = one_record(table, "");
    assert_eq!(r["role"], "research", "{r}");
    // A control character shows as a space, and the note shows 64 characters.
    let long = format!("a\tb{}", "c".repeat(77));
    assert_eq!(long.chars().count(), 80);
    let r = one_record(table, &long);
    assert_eq!(r["role"], "research", "{r}");
    let shown = format!("a b{}", "c".repeat(61));
    assert_eq!(r["note"], format!("agent type {shown}"), "{r}");
}

#[test]
fn roles_without_a_table() {
    let r = one_record("", "stapel-spec-reviewer");
    assert_eq!(r["role"], "reviewer", "{r}");
    assert!(r.get("note").is_none(), "{r}");
    let r = one_record("", "general-purpose");
    assert_eq!(r["role"], "research", "{r}");
    assert_eq!(r["note"], "agent type general-purpose", "{r}");
}
