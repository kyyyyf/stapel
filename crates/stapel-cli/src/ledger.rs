//! `stapel ledger add --kind KIND --note TEXT [KEY]`: one record appended to the process ledger
//! (STP-18 AC-5, AC-6, AC-11, AC-12).

use crate::fail::{AGENT_SHELL, Fail, MISMATCH};
use crate::repo;
use stapel_core::dialog::ledger_facts;
use stapel_core::grant::consume_with;
use stapel_core::identity::{in_agent_shell, user_name};
use stapel_core::journal::append_all;
use stapel_core::records::{LEDGER, importer_json, plan_ledger};

pub fn add(kind: &str, note: &str, key: Option<&str>, grant: Option<&str>) -> Result<(), Fail> {
    if kind == "decision" && in_agent_shell() && grant.is_none() {
        return Err(Fail::failed(AGENT_SHELL));
    }
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    // A finding or catches record carries its importer; a decision does not (STP-23).
    let importer = if kind == "finding" || kind == "catches" {
        let user = user_name(&root).map_err(Fail::failed)?;
        Some(importer_json(in_agent_shell(), &user))
    } else {
        None
    };
    // Every check runs before the grant is used, so a refusal leaves the grant to expire.
    let plan = plan_ledger(&root, kind, note, key, importer.as_deref()).map_err(Fail::refused)?;
    if let Some(token) = grant {
        consume_with(&root, token, &ledger_facts(&plan), MISMATCH).map_err(Fail::failed)?;
    }
    append_all(&root.join(LEDGER), &[plan.record.line]).map_err(Fail::refused)?;
    say!("recorded: {kind} for {}", plan.key);
    Ok(())
}

/// The number of a ticket key and its prefix, for ordering: `ABC-12` is `(12, "ABC")`.
fn key_order(key: &str) -> (u64, String) {
    match key.rsplit_once('-') {
        Some((prefix, n)) => (n.parse().unwrap_or(0), prefix.to_string()),
        None => (0, key.to_string()),
    }
}

/// `stapel ledger rules [--last N]` (STP-38 AC-8): the counted findings per rule and ticket.
pub fn rules(last: Option<&str>) -> Result<(), Fail> {
    use stapel_core::triage::state_of;
    use std::collections::BTreeMap;
    let last = match last {
        None => 5,
        Some(text) => match text.parse::<u32>() {
            Ok(n) if n >= 1 => n as usize,
            _ => return Err(Fail::refused("--last: must be a whole number of 1 or more")),
        },
    };
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    // Per ticket with findings: the counts per rule.
    let mut tickets: Vec<(String, BTreeMap<String, u64>)> = Vec::new();
    let mut skipped = 0;
    for (key, dir) in crate::tokens::ticket_folders(&root) {
        let Ok(text) = std::fs::read_to_string(dir.join("findings.jsonl")) else {
            continue;
        };
        let mut records = Vec::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(serde_json::Value::Object(m)) => records.push(m),
                _ => skipped += 1,
            }
        }
        let mut counts: BTreeMap<String, u64> = BTreeMap::new();
        let mut seen: Vec<&str> = Vec::new();
        for m in records.iter().filter(|m| !m.contains_key("kind")) {
            let Some(id) = m.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            let rule = match m.get("rule").and_then(|v| v.as_str()).map(str::trim) {
                Some(r) if !r.is_empty() => r.to_string(),
                _ => "none".to_string(),
            };
            let state = state_of(&records, id);
            let inside = state
                .finding
                .as_ref()
                .is_some_and(|f| f.get("promise").and_then(|v| v.as_str()) == Some("inside"));
            let counted = inside
                && matches!(state.severity.as_deref(), Some("HIGH" | "MEDIUM"))
                && state.verdict.as_deref() != Some("false")
                && state.fate.as_deref() != Some("no-change");
            *counts.entry(rule).or_insert(0) += u64::from(counted);
        }
        if !counts.is_empty() {
            tickets.push((key, counts));
        }
    }
    if skipped > 0 {
        warn!("notice: {skipped} lines of findings.jsonl skipped");
    }
    if tickets.is_empty() {
        say!("rules: no findings");
        return Ok(());
    }
    tickets.sort_by_key(|(k, _)| key_order(k));
    let mut rules: Vec<&String> = tickets.iter().flat_map(|(_, c)| c.keys()).collect();
    rules.sort();
    rules.dedup();
    let shown = &tickets[tickets.len().saturating_sub(last)..];
    let mut header = vec!["rule".to_string()];
    header.extend(shown.iter().map(|(k, _)| k.clone()));
    header.push("total".to_string());
    let mut rows = vec![header];
    for rule in rules {
        let cell = |c: &BTreeMap<String, u64>| c.get(rule).copied().unwrap_or(0);
        let mut row = vec![rule.clone()];
        row.extend(shown.iter().map(|(_, c)| cell(c).to_string()));
        let total: u64 = tickets.iter().map(|(_, c)| cell(c)).sum();
        row.push(total.to_string());
        rows.push(row);
    }
    let widths: Vec<usize> = (0..rows[0].len())
        .map(|i| rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0))
        .collect();
    let lines: Vec<String> = rows
        .iter()
        .map(|r| {
            r.iter()
                .enumerate()
                .map(|(i, c)| {
                    let pad = " ".repeat(widths[i] - c.chars().count());
                    if i == 0 {
                        format!("{c}{pad}")
                    } else {
                        format!("{pad}{c}")
                    }
                })
                .collect::<Vec<_>>()
                .join("  ")
        })
        .collect();
    say!("{}", lines.join("\n"));
    Ok(())
}
