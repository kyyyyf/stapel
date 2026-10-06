//! Reading Claude Code transcripts for the token journal (STP-3 AC-3).
//!
//! A message is identified by `message.id` within one file; its time is the timestamp of its
//! first line and its usage that of its last line, because output tokens grow across the lines
//! of one message in subagent transcripts.

use crate::time::parse_transcript_time;
use serde_json::Value;
use std::io::{BufRead, Read};
use std::path::Path;

/// A transcript line longer than this is unreadable; real lines stay far below it.
pub const MAX_TRANSCRIPT_LINE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Message {
        id: String,
        model: String,
        time_ms: u64,
        time: String,
        usage: Usage,
        /// The line has a `stop_reason`: it carries the message's final usage (STP-5).
        complete: bool,
    },
    Error,
    Unreadable,
    Other,
}

/// What one transcript line is, and the session and agent ids it names.
pub fn parse_transcript_line(line: &str) -> (Line, Option<String>, Option<String>) {
    let Ok(Value::Object(v)) = serde_json::from_str::<Value>(line) else {
        return (Line::Unreadable, None, None);
    };
    let text = |key: &str| v.get(key).and_then(Value::as_str).map(String::from);
    let (session, agent) = (text("sessionId"), text("agentId"));
    let message = v.get("message").and_then(Value::as_object);
    let model = message.and_then(|m| m.get("model")).and_then(Value::as_str);
    if v.get("isApiErrorMessage") == Some(&Value::Bool(true)) || model == Some("<synthetic>") {
        return (Line::Error, session, agent);
    }
    let usage = message
        .and_then(|m| m.get("usage"))
        .and_then(Value::as_object);
    let id = message.and_then(|m| m.get("id")).and_then(Value::as_str);
    let time = v.get("timestamp").and_then(Value::as_str);
    let (Some(usage), Some(id), Some(model), Some(time)) = (usage, id, model, time) else {
        return (Line::Other, session, agent);
    };
    let Some(time_ms) = parse_transcript_time(time) else {
        return (Line::Unreadable, session, agent);
    };
    let n = |key: &str| usage.get(key).and_then(Value::as_u64);
    let complete = message
        .and_then(|m| m.get("stop_reason"))
        .is_some_and(|r| r.is_string());
    let line = Line::Message {
        id: id.into(),
        model: model.into(),
        time_ms,
        time: time.into(),
        usage: Usage {
            input: n("input_tokens"),
            output: n("output_tokens"),
            cache_read: n("cache_read_input_tokens"),
            cache_write: n("cache_creation_input_tokens"),
        },
        complete,
    };
    (line, session, agent)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub model: String,
    pub time_ms: u64,
    pub time: String,
    pub usage: Usage,
    /// Its last line has a `stop_reason`; otherwise its output count is a streamed partial.
    pub complete: bool,
}

#[derive(Debug, Default)]
pub struct Transcript {
    pub session: Option<String>,
    pub agent: Option<String>,
    /// In the order of their first line.
    pub messages: Vec<Message>,
    pub error_lines: usize,
    pub unreadable_lines: usize,
}

impl Transcript {
    /// `session` or `session/agent`; `None` without a session id, since two such files could not
    /// be told apart.
    pub fn identity(&self) -> Option<String> {
        let session = self.session.clone()?;
        Some(match &self.agent {
            Some(agent) => format!("{session}/{agent}"),
            None => session,
        })
    }
}

/// Reads one line into `buf`, keeping at most `max + 1` bytes and discarding the rest of a longer
/// line, so memory stays bounded. Returns false at the end of the input.
pub fn read_bounded_line(
    reader: &mut impl BufRead,
    buf: &mut Vec<u8>,
    max: usize,
) -> std::io::Result<bool> {
    buf.clear();
    let n = reader
        .by_ref()
        .take(max as u64 + 1)
        .read_until(b'\n', buf)?;
    if n == 0 {
        return Ok(false);
    }
    if buf.len() > max && buf.last() != Some(&b'\n') {
        let mut sink = Vec::new();
        loop {
            sink.clear();
            let m = reader
                .by_ref()
                .take(64 * 1024)
                .read_until(b'\n', &mut sink)?;
            if m == 0 || sink.last() == Some(&b'\n') {
                break;
            }
        }
    }
    Ok(true)
}

/// Streams a transcript line by line; the file may be any size.
pub fn read_transcript(path: &Path) -> Result<Transcript, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let mut t = Transcript::default();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut buf = Vec::new();
    while read_bounded_line(&mut reader, &mut buf, MAX_TRANSCRIPT_LINE)
        .map_err(|e| format!("{}: {e}", path.display()))?
    {
        if buf.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        if buf.len() > MAX_TRANSCRIPT_LINE {
            t.unreadable_lines += 1;
            continue;
        }
        let Ok(text) = std::str::from_utf8(&buf) else {
            t.unreadable_lines += 1;
            continue;
        };
        let (line, session, agent) = parse_transcript_line(text.trim_end());
        if t.session.is_none() {
            t.session = session;
        }
        // A subagent file names its agent on its message lines; a main transcript does not.
        if t.agent.is_none() && matches!(line, Line::Message { .. }) {
            t.agent = agent;
        }
        match line {
            Line::Unreadable => t.unreadable_lines += 1,
            Line::Error => t.error_lines += 1,
            Line::Other => {}
            Line::Message {
                id,
                model,
                time_ms,
                time,
                usage,
                complete,
            } => match index.get(&id) {
                // A later line of the same message carries its full usage.
                Some(&i) => {
                    t.messages[i].usage = usage;
                    t.messages[i].complete = complete;
                }
                None => {
                    index.insert(id.clone(), t.messages.len());
                    t.messages.push(Message {
                        id,
                        model,
                        time_ms,
                        time,
                        usage,
                        complete,
                    });
                }
            },
        }
    }
    Ok(t)
}
