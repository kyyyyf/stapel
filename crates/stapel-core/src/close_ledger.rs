//! STP-37 AC-16, AC-17: the ledger records that `stapel close` writes.

use crate::journal::{self, Entry};
use crate::records::check_ledger;
use crate::triage::state_of;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// The longest claim a `finding` record keeps, in characters.
const CLAIM_MAX: usize = 500;

fn text<'a>(m: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    m.get(name).and_then(Value::as_str)
}

/// The lines (without newlines) to append to the ledger when `key` closes: one `catches` record and
/// one `finding` record per deferred finding. A malformed line of `findings.jsonl` or of the ledger
/// is an error that names its place.
pub fn close_records(root: &Path, ticket_dir: &Path, key: &str) -> Result<Vec<String>, String> {
    let label = format!(".stapel/tickets/{key}/findings.jsonl");
    let mut records: Vec<Map<String, Value>> = Vec::new();
    for (n, entry) in journal::read(&ticket_dir.join("findings.jsonl"))? {
        match entry {
            Entry::V1(m) | Entry::Legacy(m) => records.push(m),
            Entry::Problem(reason) if journal::is_unknown_version(&reason) => {}
            Entry::Problem(reason) => return Err(format!("{label}:{n}: {reason}")),
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    let mut counts: BTreeMap<(String, String), u64> = BTreeMap::new();
    let mut deferred: Vec<String> = Vec::new();
    for m in records.iter().filter(|m| !m.contains_key("kind")) {
        let Some(id) = text(m, "id") else { continue };
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let state = state_of(&records, id);
        let Some(finding) = state.finding.as_ref() else {
            continue;
        };
        if state.verdict.as_deref() == Some("false") {
            continue;
        }
        let severity = state.severity.as_deref().unwrap_or("");
        let stage = text(finding, "stage").unwrap_or("");
        if matches!(severity, "HIGH" | "MEDIUM")
            && text(finding, "promise") == Some("inside")
            && state.fate.as_deref() != Some("no-change")
        {
            let rule = text(finding, "rule")
                .unwrap_or("none")
                .replace([';', '='], "_");
            *counts.entry((stage.to_string(), rule)).or_insert(0) += 1;
        }
        if state.fate.as_deref() == Some("defer") {
            let round = match finding.get("round") {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            };
            let claim: String = text(finding, "claim")
                .unwrap_or("")
                .chars()
                .take(CLAIM_MAX)
                .collect();
            deferred.push(format!(
                "At the close, deferred: {id} {severity} {stage} round {round}: {claim}"
            ));
        }
    }
    let counted = if counts.is_empty() {
        "none".to_string()
    } else {
        counts
            .iter()
            .map(|((stage, rule), n)| format!("{stage}/{rule}={n}"))
            .collect::<Vec<_>>()
            .join("; ")
    };
    let mut notes = vec![format!("At the close: counted findings: {counted}")];
    notes.extend(deferred);
    let mut lines = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        let kind = if i == 0 { "catches" } else { "finding" };
        lines.push(check_ledger(root, kind, key, note, None)?.line);
    }
    Ok(lines)
}
