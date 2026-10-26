//! STP-23: triage records of findings (`stapel findings triage FILE [KEY]`) and the current state
//! of a finding that they define.

use crate::journal::{self, Entry, MAX_LINE};
use crate::records::{
    FATES, Findings, FindingsPlan, MAX_RECORDS, check_triage_fields, journal_ticket, listed,
    read_input, text_field, with_importer,
};
use crate::tickets::Status;
use serde_json::{Map, Value};
use std::path::Path;

/// The fields of a triage line.
const FIELDS: [&str; 7] = [
    "id", "ticket", "fate", "verdict", "evidence", "root", "note",
];

/// The current state of one finding: the records of its id, in file order.
#[derive(Debug, Default)]
pub struct State {
    /// The last finding record (no `kind`) of the id.
    pub finding: Option<Map<String, Value>>,
    /// The `fate` of the last record of the id, when it is a string.
    pub fate: Option<String>,
    /// The uppercase form of the last verdict among `high`, `medium`, `low`, else the finding's own
    /// severity.
    pub severity: Option<String>,
    /// The last verdict that is a string.
    pub verdict: Option<String>,
}

fn text<'a>(m: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    m.get(name).and_then(Value::as_str)
}

/// The state of `id` among the readable records of a findings file.
pub fn state_of(records: &[Map<String, Value>], id: &str) -> State {
    let mut state = State::default();
    let mut own = None;
    let mut graded = None;
    for m in records.iter().filter(|m| text(m, "id") == Some(id)) {
        let kind = text(m, "kind");
        if !m.contains_key("kind") {
            state.finding = Some(m.clone());
            own = text(m, "severity").map(str::to_string);
        } else if kind != Some("triage") {
            continue;
        }
        state.fate = text(m, "fate").map(str::to_string);
        if let Some(v) = text(m, "verdict") {
            state.verdict = Some(v.to_string());
            if ["high", "medium", "low"].contains(&v) {
                graded = Some(v.to_ascii_uppercase());
            }
        }
    }
    state.severity = graded.or(own);
    state
}

/// The whole number a `round` stands for.
pub fn whole_round(round: &Value) -> Option<u64> {
    match round {
        Value::Number(n) => n.as_u64().or_else(|| {
            n.as_f64()
                .filter(|f| *f >= 0.0 && f.fract() == 0.0)
                .map(|f| f as u64)
        }),
        Value::String(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
            Some(s.parse().unwrap_or(u64::MAX))
        }
        _ => None,
    }
}

impl State {
    /// A finding record, not legacy, not false, whose fate is `intent_gap`, or `open` with a
    /// promise `outside` or a severity `HIGH` or `MEDIUM`.
    pub fn is_blocking(&self) -> bool {
        let Some(finding) = &self.finding else {
            return false;
        };
        let Some(fate) = self.fate.as_deref().filter(|f| FATES.contains(f)) else {
            return false;
        };
        if self.verdict.as_deref() == Some("false") {
            return false;
        }
        fate == "intent_gap"
            || (fate == "open"
                && (text(finding, "promise") == Some("outside")
                    || matches!(self.severity.as_deref(), Some("HIGH" | "MEDIUM"))))
    }

    /// A blocking finding that waits for the person.
    pub fn is_persons(&self) -> bool {
        let (true, Some(finding)) = (self.is_blocking(), &self.finding) else {
            return false;
        };
        self.fate.as_deref() == Some("intent_gap")
            || text(finding, "promise") == Some("outside")
            || (finding
                .get("round")
                .and_then(whole_round)
                .is_some_and(|r| r >= 2)
                && matches!(text(finding, "stage"), Some("spec-review" | "code-review")))
    }
}

/// A blocking finding as `next` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocking {
    /// `finding <id> (<severity>, <promise>, <stage> round <round>): <fate>`.
    pub line: String,
    /// Whether it waits for the person.
    pub persons: bool,
}

/// The blocking findings of a findings file, in file order of their ids.
pub fn blocking_findings(records: &[Map<String, Value>]) -> Vec<Blocking> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for m in records.iter().filter(|m| !m.contains_key("kind")) {
        let Some(id) = text(m, "id") else { continue };
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let state = state_of(records, id);
        if !state.is_blocking() {
            continue;
        }
        let finding = state.finding.as_ref().expect("a blocking finding exists");
        let round = match finding.get("round") {
            Some(Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => String::new(),
        };
        out.push(Blocking {
            line: format!(
                "finding {id} ({}, {}, {} round {round}): {}",
                state.severity.as_deref().unwrap_or(""),
                text(finding, "promise").unwrap_or(""),
                text(finding, "stage").unwrap_or(""),
                state.fate.as_deref().unwrap_or(""),
            ),
            persons: state.is_persons(),
        });
    }
    out
}

/// The first rule a triage line breaks before the ticket's file is looked at.
fn check_line(m: &Map<String, Value>, key: &str) -> Result<(), String> {
    if m.contains_key("imported_by") {
        return Err("imported_by: written by stapel, not by the file".into());
    }
    text_field(m, "id")?;
    if let Some(Value::String(id)) = m.get("id")
        && (id.chars().count() > 64 || id.chars().any(|c| c.is_whitespace() || c.is_control()))
    {
        return Err(
            "id: must be 1 to 64 characters with no whitespace or control character".into(),
        );
    }
    match m.get("ticket") {
        None => return Err("ticket: missing".into()),
        Some(Value::String(s)) if s == key => {}
        Some(_) => return Err(format!("ticket: must be {key}")),
    }
    listed(m, "fate", &FATES)?;
    if let Some(name) = m.keys().find(|k| !FIELDS.contains(&k.as_str())) {
        return Err(format!("{name}: not a triage field"));
    }
    check_triage_fields(m)
}

/// Every check of `findings triage FILE [KEY]`; `base` is the directory that a relative `FILE` is
/// read from. The lines are the triage records to append.
pub fn plan_triage(
    root: &Path,
    base: &Path,
    file: &str,
    key: Option<&str>,
    importer: &str,
) -> Result<FindingsPlan, String> {
    let ticket = journal_ticket(root, key)?;
    let input = read_input(root, base, file)?;
    let target = ticket.dir.join("findings.jsonl");
    let label = format!(".stapel/tickets/{}/findings.jsonl", ticket.key);
    let existing = journal::read(&target)?;
    let records: Vec<Map<String, Value>> = existing
        .iter()
        .filter_map(|(_, e)| match e {
            Entry::V1(m) | Entry::Legacy(m) => Some(m.clone()),
            Entry::Problem(_) => None,
        })
        .collect();
    let mut lines: Vec<String> = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    let mut numbers: Vec<usize> = Vec::new();
    for (i, raw) in input.split('\n').enumerate() {
        let n = i + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        if raw.trim().is_empty() {
            continue;
        }
        if raw.len() + 1 > MAX_LINE {
            return Err(format!("{file}:{n}: longer than 64 KiB"));
        }
        let map = match serde_json::from_str::<Value>(raw) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err(format!("{file}:{n}: not a JSON object")),
            Err(e) => return Err(format!("{file}:{n}: not JSON: {e}")),
        };
        if lines.len() == MAX_RECORDS {
            return Err(format!(
                "{file}:{n}: more than {MAX_RECORDS} records; split the file"
            ));
        }
        check_line(&map, &ticket.key).map_err(|reason| format!("{file}:{n}: {reason}"))?;
        let Some(Value::String(id)) = map.get("id") else {
            unreachable!("check_line needs a text id");
        };
        if let Some(at) = ids.iter().position(|e| e == id) {
            return Err(format!("{file}:{n}: id {id} repeats line {}", numbers[at]));
        }
        let state = state_of(&records, id);
        if state.finding.is_none() {
            return Err(format!("{file}:{n}: id {id} is not a finding in {label}"));
        }
        if state.is_persons() && !map.contains_key("note") {
            return Err(format!(
                "{file}:{n}: note: missing; a finding for the person needs the person's answer"
            ));
        }
        let after = &raw[raw.find('{').expect("a JSON object has a brace") + 1..];
        let line = with_importer(&format!("{{\"kind\":\"triage\",{after}"), importer)
            .ok_or_else(|| format!("{file}:{n}: not a JSON object"))?;
        if line.len() + 1 > MAX_LINE {
            return Err(format!("{file}:{n}: longer than 64 KiB"));
        }
        ids.push(id.clone());
        numbers.push(n);
        lines.push(line);
    }
    if lines.is_empty() {
        return Err(format!("{file}: no records"));
    }
    for (n, entry) in &existing {
        if let Entry::Problem(reason) = entry
            && !journal::is_unknown_version(reason)
        {
            return Err(format!("{label}:{n}: {reason}"));
        }
    }
    Ok(FindingsPlan {
        closed: matches!(ticket.status, Status::Closed(_)),
        key: ticket.key,
        target,
        findings: Findings { lines, ids },
    })
}
