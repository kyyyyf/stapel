//! STP-40: the inbox of the decisions that wait for the person: its entries and the text of
//! `.stapel/inbox.md`.

use crate::triage::state_of;
use serde_json::{Map, Value};

/// A text with each run of whitespace, line breaks included, replaced by one space, and trimmed.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A decision that waits for the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// A finding: its id and the line `<stage> round <round>, <severity>, <promise>: <claim>`.
    Finding { id: String, line: String },
}

/// One owed decision of an open ticket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub title: String,
    pub worktree: String,
    pub kind: Kind,
}

fn field(record: &Map<String, Value>, name: &str) -> String {
    match record.get(name) {
        Some(Value::String(s)) if !one_line(s).is_empty() => one_line(s),
        Some(Value::Number(n)) => n.to_string(),
        _ => "?".to_string(),
    }
}

/// The findings of a findings file that wait for the person (`triage::State::is_persons`), in file
/// order of their ids, as `(id, line)`.
pub fn owed_findings(records: &[Map<String, Value>]) -> Vec<(String, String)> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for m in records.iter().filter(|m| !m.contains_key("kind")) {
        let Some(id) = m.get("id").and_then(Value::as_str) else {
            continue;
        };
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let state = state_of(records, id);
        if !state.is_persons() {
            continue;
        }
        let finding = state.finding.as_ref().expect("an owed finding exists");
        let severity = state
            .severity
            .as_deref()
            .map(one_line)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "?".to_string());
        let line = format!(
            "{} round {}, {}, {}: {}",
            field(finding, "stage"),
            field(finding, "round"),
            severity,
            field(finding, "promise"),
            field(finding, "claim"),
        );
        out.push((one_line(id), line));
    }
    out
}

/// The text of `inbox.md` for `entries`, which come grouped by ticket in the order to write.
pub fn render(entries: &[Entry]) -> String {
    let mut text = String::from("# Inbox\n");
    if entries.is_empty() {
        text.push_str("\nNothing waits.\n");
        return text;
    }
    let mut last: Option<&str> = None;
    for e in entries {
        if last != Some(e.key.as_str()) {
            text.push_str(&format!(
                "\n## {} — {}\nworktree: {}\n",
                e.key,
                one_line(&e.title),
                e.worktree
            ));
            last = Some(&e.key);
        }
        match &e.kind {
            Kind::Finding { id, line } => {
                text.push_str(&format!("\n### {} finding {id}\n{line}\nnote:\n", e.key));
            }
        }
    }
    text
}
