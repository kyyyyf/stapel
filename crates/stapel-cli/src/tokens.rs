//! `stapel tokens`: the token journal of a ticket (STP-3 AC-2..AC-4).

use crate::repo;
use serde_json::{Map, Value, json};
use stapel_core::journal::{append, new_id};
use stapel_core::tickets::{Status, Ticket, resolve};
use stapel_core::time::now_rfc3339;
use std::path::Path;

/// The options of `stapel tokens add`, as typed.
pub struct Add {
    pub key: Option<String>,
    pub role: String,
    pub model: Option<String>,
    pub input: Option<String>,
    pub output: Option<String>,
    pub cache_read: Option<String>,
    pub cache_write: Option<String>,
    pub estimate: Option<String>,
    pub step: Option<String>,
    pub note: Option<String>,
}

/// A ticket that accepts token records: open or closed, not legacy or unreadable.
pub fn journal_ticket(root: &Path, key: Option<&str>) -> Result<Ticket, String> {
    let ticket = resolve(root, key)?;
    match &ticket.status {
        Status::Open(_) | Status::Closed(_) => Ok(ticket),
        Status::Legacy(_) => Err(format!(
            "ticket {} has a legacy (STP-1) state.json; its journal is kept by hand",
            ticket.key
        )),
        Status::Unreadable(reason) => Err(reason.clone()),
    }
}

fn count(name: &str, value: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|_| {
        format!("--{name} must be a whole number of tokens, 0 or more; got \"{value}\"")
    })
}

/// A short text field: non-empty, at most 256 bytes, no control characters.
pub fn short_text(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(format!(
            "--{name} must be non-empty, at most 256 bytes and without control characters"
        ));
    }
    Ok(())
}

pub fn add(a: Add) -> Result<(), String> {
    let (root, config) = repo::open()?;
    short_text("role", &a.role)?;
    if let Some(step) = &a.step {
        short_text("step", step)?;
    }
    if let Some(note) = &a.note
        && (note.len() > 4096 || note.chars().any(|c| c.is_control() && c != '\n'))
    {
        return Err("--note must be at most 4 KiB, without control characters".into());
    }
    let model = match (&a.model, config.models.get(&a.role)) {
        (Some(m), _) => m.clone(),
        (None, Some(binding)) => binding.model.clone(),
        (None, None) => {
            return Err(format!(
                "role {} is not in stapel.toml, so --model is required",
                a.role
            ));
        }
    };
    short_text("model", &model)?;

    let mut record = Map::new();
    let measured = [
        ("input", &a.input),
        ("output", &a.output),
        ("cache_read", &a.cache_read),
        ("cache_write", &a.cache_write),
    ];
    let any_measured = measured.iter().any(|(_, v)| v.is_some());
    match (&a.estimate, any_measured) {
        (Some(_), true) => {
            return Err("give either measured counts or --estimate, not both".into());
        }
        (None, false) => {
            return Err("give --input and --output, or --estimate".into());
        }
        (Some(e), false) => {
            record.insert("source".into(), json!("estimate"));
            record.insert("estimate".into(), json!(count("estimate", e)?));
        }
        (None, true) => {
            if a.input.is_none() || a.output.is_none() {
                return Err("a measured record needs both --input and --output".into());
            }
            record.insert("source".into(), json!("measured"));
            for (name, value) in measured {
                if let Some(v) = value {
                    let flag = name.replace('_', "-");
                    record.insert(name.into(), json!(count(&flag, v)?));
                }
            }
        }
    }
    let ticket = journal_ticket(&root, a.key.as_deref())?;
    if !config.models.contains_key(&a.role) {
        eprintln!("warning: role {} is not in stapel.toml", a.role);
    }
    let id = new_id("t");
    record.insert("v".into(), json!(1));
    record.insert("id".into(), json!(id));
    record.insert("at".into(), json!(now_rfc3339()));
    record.insert("ticket".into(), json!(ticket.key));
    record.insert("role".into(), json!(a.role));
    record.insert("model".into(), json!(model));
    if let Some(step) = a.step {
        record.insert("step".into(), json!(step));
    }
    if let Some(note) = a.note {
        record.insert("note".into(), json!(note));
    }
    append(&ticket.dir.join("tokens.jsonl"), &Value::Object(record))?;
    println!("recorded: {id}");
    Ok(())
}

/// The options of `stapel tokens import`, as typed.
pub struct Import {
    pub transcript: std::path::PathBuf,
    pub key: Option<String>,
    pub role: String,
    pub since: Option<String>,
    pub until: Option<String>,
}

/// Minutes a transcript must be quiet, or the window must end before now, for messages to be whole.
const QUIET_SECS: u64 = 5 * 60;

fn window_end(name: &str, value: &Option<String>) -> Result<Option<u64>, String> {
    value
        .as_ref()
        .map(|v| {
            stapel_core::time::parse_utc(v)
                .map(|s| s * 1000)
                .ok_or_else(|| format!("--{name} must look like 2026-10-05T07:00:00Z; got \"{v}\""))
        })
        .transpose()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Sums of one model over the counted messages.
struct Sum {
    model: String,
    from: String,
    to: String,
    from_ms: u64,
    to_ms: u64,
    messages: u64,
    fields: [(&'static str, Option<u64>); 4],
}

pub fn import(a: Import) -> Result<(), String> {
    let (root, _config) = repo::open()?;
    short_text("role", &a.role)?;
    let since = window_end("since", &a.since)?.unwrap_or(0);
    let until = window_end("until", &a.until)?;

    let modified = std::fs::metadata(&a.transcript)
        .and_then(|m| m.modified())
        .map_err(|e| format!("{}: {e}", a.transcript.display()))?;
    let quiet_for = modified.elapsed().map(|d| d.as_secs()).unwrap_or(0);
    if quiet_for < QUIET_SECS {
        match until {
            None => {
                return Err(
                    "the transcript is still being written; give --until at least 5 minutes before now"
                        .into(),
                );
            }
            Some(u) if u / 1000 + QUIET_SECS > now_secs() => {
                return Err(
                    "the transcript is still being written; --until must be at least 5 minutes before now"
                        .into(),
                );
            }
            Some(_) => {}
        }
    }

    let ticket = journal_ticket(&root, a.key.as_deref())?;
    let t = stapel_core::tokens::read_transcript(&a.transcript)?;
    let transcript = t.identity();

    let mut sums: Vec<Sum> = Vec::new();
    for m in t
        .messages
        .iter()
        .filter(|m| m.time_ms >= since && until.is_none_or(|u| m.time_ms < u))
    {
        let sum = match sums.iter_mut().find(|s| s.model == m.model) {
            Some(s) => s,
            None => {
                sums.push(Sum {
                    model: m.model.clone(),
                    from: m.time.clone(),
                    to: m.time.clone(),
                    from_ms: m.time_ms,
                    to_ms: m.time_ms,
                    messages: 0,
                    fields: [
                        ("input", Some(0)),
                        ("output", Some(0)),
                        ("cache_read", Some(0)),
                        ("cache_write", Some(0)),
                    ],
                });
                sums.last_mut().expect("just pushed")
            }
        };
        if m.time_ms < sum.from_ms {
            (sum.from_ms, sum.from) = (m.time_ms, m.time.clone());
        }
        if m.time_ms > sum.to_ms {
            (sum.to_ms, sum.to) = (m.time_ms, m.time.clone());
        }
        sum.messages += 1;
        let values = [
            m.usage.input,
            m.usage.output,
            m.usage.cache_read,
            m.usage.cache_write,
        ];
        for (field, value) in sum.fields.iter_mut().zip(values) {
            // A field is present in the sum only when every counted message carried it.
            field.1 = match (field.1, value) {
                (Some(acc), Some(v)) => Some(acc + v),
                _ => None,
            };
        }
    }
    sums.retain(|s| s.fields.iter().any(|(_, v)| v.is_some_and(|n| n > 0)));
    sums.sort_by(|a, b| a.model.cmp(&b.model));

    // Every record is checked before any is appended.
    let existing: Vec<serde_json::Map<String, Value>> =
        stapel_core::journal::read(&ticket.dir.join("tokens.jsonl"))?
            .into_iter()
            .filter_map(|(_, e)| match e {
                stapel_core::journal::Entry::V1(m) => Some(m),
                _ => None,
            })
            .filter(|m| m.get("transcript").and_then(Value::as_str) == Some(transcript.as_str()))
            .collect();
    let mut already = Vec::new();
    for s in &sums {
        for e in existing
            .iter()
            .filter(|e| e.get("model").and_then(Value::as_str) == Some(s.model.as_str()))
        {
            let (ef, et) = (
                e.get("from").and_then(Value::as_str).unwrap_or(""),
                e.get("to").and_then(Value::as_str).unwrap_or(""),
            );
            let id = e.get("id").and_then(Value::as_str).unwrap_or("?");
            if ef == s.from && et == s.to {
                already.push(id.to_string());
                continue;
            }
            let et_ms = stapel_core::time::parse_transcript_time(et).unwrap_or(u64::MAX);
            if s.from_ms <= et_ms {
                return Err(format!(
                    "the {} range from {} overlaps record {id} (to {et}); import from after that time",
                    s.model, s.from
                ));
            }
        }
    }
    println!("skipped: {} error lines", t.error_lines);
    println!("skipped: {} unreadable lines", t.unreadable_lines);
    if !sums.is_empty() && already.len() == sums.len() {
        for id in already {
            println!("already imported: {id}");
        }
        return Ok(());
    }
    if !already.is_empty() {
        return Err(format!(
            "part of this window is imported already ({}); import a range after it",
            already.join(", ")
        ));
    }
    for s in &sums {
        let mut digest = sha2::Sha256::new();
        use sha2::Digest;
        for part in [transcript.as_str(), &s.model, &s.from, &s.to] {
            digest.update(part.as_bytes());
            digest.update([0u8]);
        }
        let hex: String = digest
            .finalize()
            .iter()
            .take(6)
            .map(|b| format!("{b:02x}"))
            .collect();
        let id = format!("t-{hex}");
        let mut record = Map::new();
        record.insert("v".into(), json!(1));
        record.insert("id".into(), json!(id));
        record.insert("at".into(), json!(now_rfc3339()));
        record.insert("ticket".into(), json!(ticket.key));
        record.insert("role".into(), json!(a.role));
        record.insert("model".into(), json!(s.model));
        record.insert("source".into(), json!("measured"));
        for (name, value) in s.fields {
            if let Some(v) = value {
                record.insert(name.into(), json!(v));
            }
        }
        record.insert("transcript".into(), json!(transcript));
        record.insert("from".into(), json!(s.from));
        record.insert("to".into(), json!(s.to));
        record.insert("messages".into(), json!(s.messages));
        append(&ticket.dir.join("tokens.jsonl"), &Value::Object(record))?;
        println!("recorded: {id} {} ({} messages)", s.model, s.messages);
    }
    Ok(())
}

/// One report row: sums of the records of one role, model and origin.
#[derive(Default)]
struct Row {
    measured: [Option<u64>; 4],
    estimate: Option<u64>,
    legacy_total: Option<u64>,
    legacy_kind: Option<String>,
}

fn add_to(slot: &mut Option<u64>, value: Option<u64>) {
    if let Some(v) = value {
        *slot = Some(slot.unwrap_or(0) + v);
    }
}

fn cell(v: Option<u64>) -> String {
    v.map_or_else(|| "—".to_string(), |n| n.to_string())
}

fn line(role: &str, model: &str, cells: [&str; 6]) -> String {
    format!(
        "{role:<14}{model:<28}{:>9}{:>9}{:>12}{:>13}{:>10}{:>17}",
        cells[0], cells[1], cells[2], cells[3], cells[4], cells[5]
    )
}

/// `stapel tokens [KEY]` (STP-3 AC-4). Reads only `tokens.jsonl` files.
pub fn report(key: Option<&str>) -> Result<std::process::ExitCode, String> {
    use stapel_core::journal::{Entry, read};
    let (root, _config) = repo::open()?;
    let tickets: Vec<Ticket> = stapel_core::tickets::list(&root)
        .into_iter()
        .filter(|t| key.is_none_or(|k| t.key.eq_ignore_ascii_case(k)))
        .collect();
    if let (Some(k), true) = (key, tickets.is_empty()) {
        return Err(format!("no ticket {k} in .stapel/tickets"));
    }
    let mut blocks = Vec::new();
    let mut problems = Vec::new();
    for ticket in tickets {
        let path = ticket.dir.join("tokens.jsonl");
        if !path.exists() {
            continue;
        }
        // Key: role, model, origin (0 for v1 records, 1 for legacy ones).
        let mut rows: std::collections::BTreeMap<(String, String, u8), Row> = Default::default();
        for (n, entry) in read(&path)? {
            let text = |m: &Map<String, Value>, k: &str| {
                m.get(k).and_then(Value::as_str).unwrap_or("—").to_string()
            };
            match entry {
                Entry::Problem(_) => problems.push(format!(
                    "problem: .stapel/tickets/{}/tokens.jsonl:{n}",
                    ticket.key
                )),
                Entry::V1(m) => {
                    let row = rows
                        .entry((text(&m, "role"), text(&m, "model"), 0))
                        .or_default();
                    let n = |k: &str| m.get(k).and_then(Value::as_u64);
                    if m.get("source").and_then(Value::as_str) == Some("estimate") {
                        add_to(&mut row.estimate, n("estimate"));
                    } else {
                        for (slot, k) in row.measured.iter_mut().zip([
                            "input",
                            "output",
                            "cache_read",
                            "cache_write",
                        ]) {
                            add_to(slot, n(k));
                        }
                    }
                }
                Entry::Legacy(m) => {
                    let kind = text(&m, "kind");
                    let n = |k: &str| m.get(k).and_then(Value::as_u64);
                    let total = n("total_tokens")
                        .or_else(|| Some(n("input_tokens")? + n("output_tokens")?));
                    let row = rows
                        .entry((
                            text(&m, "role"),
                            format!("{}\u{0}{kind}", text(&m, "model")),
                            1,
                        ))
                        .or_default();
                    add_to(&mut row.legacy_total, total);
                    row.legacy_kind = Some(kind);
                }
            }
        }
        let mut block = vec![
            format!("ticket: {}", ticket.key),
            line(
                "role",
                "model",
                [
                    "input",
                    "output",
                    "cache read",
                    "cache write",
                    "estimate",
                    "legacy",
                ],
            ),
        ];
        for ((role, model, origin), row) in &rows {
            let model = model.split('\u{0}').next().unwrap_or(model);
            let legacy = match (&row.legacy_kind, origin) {
                (Some(kind), 1) => format!("{} {kind}", cell(row.legacy_total)),
                _ => "—".into(),
            };
            let m = row.measured.map(cell);
            let estimate = cell(row.estimate);
            block.push(line(
                role,
                model,
                [&m[0], &m[1], &m[2], &m[3], &estimate, &legacy],
            ));
        }
        blocks.push(block.join("\n"));
    }
    let mut out = blocks.join("\n\n");
    if !problems.is_empty() {
        out.push_str("\n\n");
        out.push_str(&problems.join("\n"));
    }
    if !out.is_empty() {
        println!("{out}");
    }
    Ok(if problems.is_empty() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    })
}
