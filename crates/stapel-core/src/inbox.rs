//! STP-40: the inbox of the decisions that wait for the person: its entries and the text of
//! `.stapel/inbox.md`.

use crate::config::Config;
use crate::next::{is_light, plan_steps};
use crate::stage::{freshness, latest};
use crate::state::State;
use crate::ticket::{Section, find};
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
    /// Sections that wait for `stapel ok`, as ids in config order.
    Confirmation { waiting: Vec<String> },
}

/// The ids of the sections that wait for `stapel ok`, in config order, for a ticket past its spec
/// review; empty before it. Past the review: the Description has its Frame, every section is
/// written and the Plan has steps, and the Plan is light, a section was confirmed once or a
/// spec-review finding exists.
pub fn waiting_sections(
    config: &Config,
    sections: &[Section],
    state: &State,
    finding_stages: &[String],
) -> Vec<String> {
    let written = |title: &str| {
        find(sections, title)
            .found()
            .is_some_and(|body| !matches!(body.trim(), "" | "TODO"))
    };
    let framed = written("Description")
        && find(sections, "Description")
            .found()
            .is_some_and(|body| body.lines().any(|l| l.trim_end() == "### Frame"));
    let confirmable: Vec<_> = config
        .sections
        .iter()
        .filter(|s| s.owner != "generated")
        .collect();
    let all_written = confirmable.iter().all(|def| {
        written(&def.title)
            && (def.id != "plan"
                || find(sections, &def.title)
                    .found()
                    .is_some_and(|plan| !plan_steps(plan).is_empty()))
    });
    let light = config
        .section("plan")
        .and_then(|def| find(sections, &def.title).found())
        .is_some_and(is_light);
    let reviewed = light
        || !state.confirmations.is_empty()
        || finding_stages.iter().any(|s| s == "spec-review");
    if !(framed && all_written && reviewed) {
        return Vec::new();
    }
    confirmable
        .iter()
        .filter(|def| match latest(state, &def.id) {
            None => true,
            Some(c) => !freshness(config, sections, c).is_empty(),
        })
        .map(|def| def.id.clone())
        .collect()
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
            Kind::Confirmation { waiting } => {
                text.push_str(&format!(
                    "\n### {} confirmation\nwaiting: {}\npage: {}/.stapel/tickets/{}/page.md\n",
                    e.key,
                    waiting.join(", "),
                    e.worktree,
                    e.key
                ));
            }
        }
    }
    text
}
