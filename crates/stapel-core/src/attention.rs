//! The events reader of a main transcript (STP-29): questions and dialogs the person answered.
//!
//! A call is a `tool_use` item of an `assistant` line, timed by the first line that holds its id.
//! A result is a `tool_result` item of a `user` line; its source is `permissionDecision.source` of
//! that line. Lines that do not parse, and other shapes, are skipped.

use crate::time::parse_transcript_time;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// What the person did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// An `AskUserQuestion` call with this many questions.
    Question(u32),
    /// A permission dialog the person answered.
    Dialog,
}

/// One attention event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    /// Time of the result, milliseconds since 1970.
    pub time_ms: u64,
    /// Wait from the call to the result; 0 when negative.
    pub wait_ms: u64,
}

impl Event {
    /// The interval of the wait: from the time minus the wait to the time.
    pub fn interval(&self) -> (u64, u64) {
        (self.time_ms.saturating_sub(self.wait_ms), self.time_ms)
    }
}

struct Call {
    time_ms: u64,
    name: String,
    questions: u32,
}

/// Read the events of a main transcript file.
pub fn read_events(path: &Path) -> std::io::Result<Vec<Event>> {
    let text = std::fs::read_to_string(path)?;
    Ok(events_of(&text))
}

/// The events of transcript text.
pub fn events_of(text: &str) -> Vec<Event> {
    let mut calls: HashMap<String, Call> = HashMap::new();
    let mut events = Vec::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(time_ms) = v
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_transcript_time)
        else {
            continue;
        };
        let Some(items) = v.pointer("/message/content").and_then(Value::as_array) else {
            continue;
        };
        match v.get("type").and_then(Value::as_str) {
            Some("assistant") => {
                for item in items {
                    if item.get("type").and_then(Value::as_str) != Some("tool_use") {
                        continue;
                    }
                    let Some(id) = item.get("id").and_then(Value::as_str) else {
                        continue;
                    };
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("");
                    let questions = match item.pointer("/input/questions").and_then(Value::as_array)
                    {
                        Some(a) if !a.is_empty() => u32::try_from(a.len()).unwrap_or(u32::MAX),
                        _ => 1,
                    };
                    calls.entry(id.to_string()).or_insert(Call {
                        time_ms,
                        name: name.to_string(),
                        questions,
                    });
                }
            }
            Some("user") => {
                let source = v
                    .pointer("/permissionDecision/source")
                    .and_then(Value::as_str);
                for item in items {
                    if item.get("type").and_then(Value::as_str) != Some("tool_result") {
                        continue;
                    }
                    let Some(call) = item
                        .get("tool_use_id")
                        .and_then(Value::as_str)
                        .and_then(|id| calls.get(id))
                    else {
                        continue;
                    };
                    let kind = if call.name == "AskUserQuestion" {
                        EventKind::Question(call.questions)
                    } else if source.is_some_and(|s| s.starts_with("user")) {
                        EventKind::Dialog
                    } else {
                        continue;
                    };
                    events.push(Event {
                        kind,
                        time_ms,
                        wait_ms: time_ms.saturating_sub(call.time_ms),
                    });
                }
            }
            _ => {}
        }
    }
    events
}
