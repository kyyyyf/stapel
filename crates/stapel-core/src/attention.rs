//! The events reader of a main transcript (STP-29): questions, dialogs and typed messages.
//!
//! A call is a `tool_use` item of an `assistant` line, timed by the first line that holds its id.
//! A result is a `tool_result` item of a `user` line; its source is `permissionDecision.source` of
//! that line. Lines that do not parse, and other shapes, are skipped.

use crate::time::parse_transcript_time;
use crate::tokens::{read_bounded_line, MAX_TRANSCRIPT_LINE};
use serde_json::Value;
use std::collections::HashMap;
use std::io::BufReader;
use std::path::Path;

/// What the person did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// An `AskUserQuestion` call with this many questions.
    Question(u32),
    /// A permission dialog the person answered.
    Dialog,
    /// A message the person typed.
    Typed,
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
    let file = std::fs::File::open(path)?;
    events_of(BufReader::new(file))
}

/// The events of a transcript reader. A line over 16 MiB is skipped.
pub fn events_of(mut reader: impl std::io::BufRead) -> std::io::Result<Vec<Event>> {
    let mut calls: HashMap<String, Call> = HashMap::new();
    let mut events = Vec::new();
    let mut last_assistant: Option<u64> = None;
    let mut buf = Vec::new();
    while read_bounded_line(&mut reader, &mut buf, MAX_TRANSCRIPT_LINE)? {
        if buf.len() > MAX_TRANSCRIPT_LINE {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(&buf) else {
            continue;
        };
        if !v.is_object() {
            continue;
        }
        let Some(time_ms) = v
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_transcript_time)
        else {
            continue;
        };
        let content = v.pointer("/message/content");
        match v.get("type").and_then(Value::as_str) {
            Some("assistant") => {
                last_assistant = Some(time_ms);
                let Some(items) = content.and_then(Value::as_array) else {
                    continue;
                };
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
                let is_result = |item: &Value| {
                    item.get("type").and_then(Value::as_str) == Some("tool_result")
                };
                if let Some(items) = content.and_then(Value::as_array) {
                    if items.iter().any(is_result) {
                        let source = v
                            .pointer("/permissionDecision/source")
                            .and_then(Value::as_str);
                        for item in items.iter().filter(|i| is_result(i)) {
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
                        continue;
                    }
                }
                let flag = |k: &str| v.get(k).and_then(Value::as_bool) == Some(true);
                if flag("isMeta") || flag("isSidechain") || flag("isCompactSummary") {
                    continue;
                }
                let text = match content {
                    Some(Value::String(s)) => Some(s.as_str()),
                    Some(Value::Array(items)) => items
                        .iter()
                        .find(|i| i.get("type").and_then(Value::as_str) == Some("text"))
                        .and_then(|i| i.get("text").and_then(Value::as_str)),
                    _ => None,
                };
                let Some(text) = text else {
                    continue;
                };
                if text.starts_with('<') || text.starts_with("[Request interrupted") {
                    continue;
                }
                events.push(Event {
                    kind: EventKind::Typed,
                    time_ms,
                    wait_ms: last_assistant.map_or(0, |t| time_ms.saturating_sub(t)),
                });
            }
            _ => {}
        }
    }
    Ok(events)
}

/// The cap of one wait in the attention time: 10 minutes.
pub const WAIT_CAP_MS: u64 = 600_000;

/// The attention numbers of a list of events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sums {
    /// Length of the union of the intervals.
    pub waited_ms: u64,
    /// Length of the union with each interval cut to its last `WAIT_CAP_MS`.
    pub attention_ms: u64,
    /// Events whose wait is over the cap.
    pub long_waits: u32,
    /// Questions asked (a call with several questions counts each).
    pub questions: u32,
    /// Dialogs the person answered.
    pub dialogs: u32,
    /// Messages the person typed.
    pub typed: u32,
}

fn union_len(mut spans: Vec<(u64, u64)>) -> u64 {
    spans.sort_unstable();
    let mut total = 0;
    let mut end = 0;
    for (s, e) in spans {
        let s = s.max(end);
        if e > s {
            total += e - s;
            end = e;
        }
    }
    total
}

/// Sum events: union of intervals, cap per interval and counts.
pub fn sum_events(events: &[Event]) -> Sums {
    let mut sums = Sums::default();
    let mut full = Vec::with_capacity(events.len());
    let mut capped = Vec::with_capacity(events.len());
    for e in events {
        let (s, t) = e.interval();
        full.push((s, t));
        capped.push((s.max(t.saturating_sub(WAIT_CAP_MS)), t));
        if e.wait_ms > WAIT_CAP_MS {
            sums.long_waits += 1;
        }
        match e.kind {
            EventKind::Question(n) => sums.questions = sums.questions.saturating_add(n),
            EventKind::Dialog => sums.dialogs += 1,
            EventKind::Typed => sums.typed += 1,
        }
    }
    sums.waited_ms = union_len(full);
    sums.attention_ms = union_len(capped);
    sums
}
