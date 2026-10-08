//! `stapel tokens`: the token journal of a ticket (STP-3 AC-2..AC-4).

use crate::repo;
use serde_json::{Map, Value, json};
use sha2::Digest;
use stapel_core::journal::{Entry, append, new_id, read};
use stapel_core::tickets::{Status, Ticket, resolve, tickets_dir};
use stapel_core::time::{now_rfc3339, parse_cli_time, parse_transcript_time, rfc3339_millis};
use std::path::Path;

/// The largest count one record may carry; sums of many such records still fit in `u64`.
const MAX_COUNT: u64 = 1_000_000_000_000_000;

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
    match value.parse::<u64>() {
        Ok(n) if n <= MAX_COUNT => Ok(n),
        _ => Err(format!(
            "--{name} must be a whole number of tokens from 0 to 10^15; got \"{value}\""
        )),
    }
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

fn warn_unconfigured(config: &stapel_core::config::Config, role: &str) {
    if !config.models.contains_key(role) {
        eprintln!("warning: role {role} is not in stapel.toml");
    }
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
        return Err("--note must be at most 4 KiB, without control characters but newlines".into());
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
        (Some(_), true) => return Err("give either measured counts or --estimate, not both".into()),
        (None, false) => return Err("give --input and --output, or --estimate".into()),
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
                    record.insert(name.into(), json!(count(&name.replace('_', "-"), v)?));
                }
            }
        }
    }
    let ticket = journal_ticket(&root, a.key.as_deref())?;
    warn_unconfigured(&config, &a.role);
    let id = new_id("t");
    for (k, v) in [
        ("v", json!(1)),
        ("id", json!(id)),
        ("at", json!(now_rfc3339())),
        ("ticket", json!(ticket.key)),
        ("role", json!(a.role)),
        ("model", json!(model)),
    ] {
        record.insert(k.into(), v);
    }
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

/// Seconds a transcript must be quiet, or the window must end before now, for messages to be whole.
const QUIET_SECS: u64 = 5 * 60;

fn window_end(name: &str, value: &Option<String>) -> Result<Option<u64>, String> {
    value
        .as_ref()
        .map(|v| {
            parse_cli_time(v).ok_or_else(|| {
                format!(
                    "--{name} must be a real UTC time like 2026-10-05T07:00:00Z or \
                     2026-10-05T07:00:00.250Z; got \"{v}\""
                )
            })
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
    /// Messages without a final line: their output is not known (STP-5 AC-1).
    partial: u64,
}

pub fn import(a: Import) -> Result<(), String> {
    let (root, config) = repo::open()?;
    short_text("role", &a.role)?;
    let since = window_end("since", &a.since)?.unwrap_or(0);
    let until = window_end("until", &a.until)?;
    if until.is_some_and(|u| u <= since) {
        return Err("--until must be later than --since".into());
    }

    let modified = std::fs::metadata(&a.transcript)
        .and_then(|m| m.modified())
        .map_err(|e| format!("{}: {e}", a.transcript.display()))?;
    let quiet_for = modified.elapsed().map(|d| d.as_secs()).unwrap_or(0);
    if quiet_for < QUIET_SECS {
        match until {
            None => {
                return Err("the transcript is still being written; give --until at least 5 minutes before now".into());
            }
            Some(u) if u / 1000 + QUIET_SECS > now_secs() => {
                return Err("the transcript is still being written; --until must be at least 5 minutes before now".into());
            }
            Some(_) => {}
        }
    }

    let ticket = journal_ticket(&root, a.key.as_deref())?;
    let t = stapel_core::tokens::read_transcript(&a.transcript)?;
    let transcript = t.identity().ok_or(
        "the transcript has no sessionId, so its records could not be told from another file's",
    )?;
    warn_unconfigured(&config, &a.role);

    let mut sums: Vec<Sum> = Vec::new();
    for m in t
        .messages
        .iter()
        .filter(|m| m.time_ms >= since && until.is_none_or(|u| m.time_ms < u))
    {
        let sum = match sums.iter().position(|s| s.model == m.model) {
            Some(i) => &mut sums[i],
            None => {
                sums.push(Sum {
                    model: m.model.clone(),
                    from: m.time.clone(),
                    to: m.time.clone(),
                    from_ms: m.time_ms,
                    to_ms: m.time_ms,
                    messages: 0,
                    partial: 0,
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
        if !m.complete {
            sum.partial += 1;
        }
        let values = [
            m.usage.input,
            m.usage.output,
            m.usage.cache_read,
            m.usage.cache_write,
        ];
        for (i, (field, value)) in sum.fields.iter_mut().zip(values).enumerate() {
            // The output of a message without a final line is a streamed partial: left out.
            if i == 1 && !m.complete {
                continue;
            }
            // A field is present in the sum only when every counted message carried it.
            field.1 = match (field.1, value) {
                (Some(acc), Some(v)) => Some(acc.saturating_add(v)),
                _ => None,
            };
        }
    }
    for s in sums.iter_mut() {
        if s.partial == s.messages {
            s.fields[1].1 = None;
        }
    }
    sums.retain(|s| s.fields.iter().any(|(_, v)| v.is_some_and(|n| n > 0)));
    sums.sort_by(|a, b| a.model.cmp(&b.model));

    println!("skipped: {} error lines", t.error_lines);
    println!("skipped: {} unreadable lines", t.unreadable_lines);
    if sums.is_empty() {
        println!("nothing to import in the window");
        return Ok(());
    }

    // Every record is checked before any is appended.
    let existing: Vec<Map<String, Value>> = read(&ticket.dir.join("tokens.jsonl"))?
        .into_iter()
        .filter_map(|(_, e)| match e {
            Entry::V1(m) => Some(m),
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
            let text = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("");
            let id = e.get("id").and_then(Value::as_str).unwrap_or("?");
            if text("from") == s.from && text("to") == s.to {
                already.push(id.to_string());
                continue;
            }
            let et_ms = parse_transcript_time(text("to")).unwrap_or(u64::MAX);
            if s.from_ms <= et_ms {
                return Err(format!(
                    "the {} range from {} overlaps record {id} (to {}); import with --since {}",
                    s.model,
                    s.from,
                    text("to"),
                    rfc3339_millis(et_ms.saturating_add(1))
                ));
            }
        }
    }
    let next_since = rfc3339_millis(sums.iter().map(|s| s.to_ms).max().unwrap_or(0) + 1);
    if already.len() == sums.len() {
        for id in already {
            println!("already imported: {id}");
        }
        println!("next --since: {next_since}");
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
        for part in [
            ticket.key.as_str(),
            transcript.as_str(),
            &s.model,
            &s.from,
            &s.to,
        ] {
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
        for (k, v) in [
            ("v", json!(1)),
            ("id", json!(id)),
            ("at", json!(now_rfc3339())),
            ("ticket", json!(ticket.key)),
            ("role", json!(a.role)),
            ("model", json!(s.model)),
            ("source", json!("measured")),
        ] {
            record.insert(k.into(), v);
        }
        for (name, value) in s.fields {
            if let Some(v) = value {
                record.insert(name.into(), json!(v));
            }
        }
        record.insert("transcript".into(), json!(transcript));
        record.insert("from".into(), json!(s.from));
        record.insert("to".into(), json!(s.to));
        record.insert("messages".into(), json!(s.messages));
        if s.partial > 0 {
            record.insert("partial".into(), json!(s.partial));
        }
        // Every record of this importer says `importer: 2`, so the report can tell it from the
        // subagent records written before STP-5, whose output may be partial.
        record.insert("importer".into(), json!(2));
        append(&ticket.dir.join("tokens.jsonl"), &Value::Object(record))?;
        if s.partial > 0 {
            println!(
                "recorded: {id} {} ({} messages, partial: {})",
                s.model, s.messages, s.partial
            );
        } else {
            println!("recorded: {id} {} ({} messages)", s.model, s.messages);
        }
    }
    println!("next --since: {next_since}");
    Ok(())
}

/// `stapel tokens session <transcript>`: the last cost-state line into `.stapel/sessions.jsonl`
/// (STP-5 AC-3).
pub fn session(transcript: &Path) -> Result<(), String> {
    let (root, _config) = repo::open()?;
    let c = stapel_core::tokens::last_cost_state(transcript)?;
    let mut digest = sha2::Sha256::new();
    for part in [
        c.session.clone(),
        c.start.to_string(),
        c.duration_ms.to_string(),
    ] {
        digest.update(part.as_bytes());
        digest.update([0u8]);
    }
    let hex: String = digest
        .finalize()
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect();
    let id = format!("s-{hex}");
    let journal = root.join(".stapel/sessions.jsonl");
    let known = read(&journal)?.into_iter().any(|(_, e)| match e {
        Entry::V1(m) => m.get("id").and_then(Value::as_str) == Some(id.as_str()),
        _ => false,
    });
    if known {
        println!("already imported: {id}");
        return Ok(());
    }
    let mut models = Map::new();
    for (model, counts) in &c.models {
        let mut m = Map::new();
        for (name, value) in ["input", "output", "thinking", "cache_read", "cache_write"]
            .iter()
            .zip(counts)
        {
            if let Some(v) = value {
                m.insert((*name).into(), json!(v));
            }
        }
        models.insert(model.clone(), Value::Object(m));
    }
    let record = json!({
        "v": 1,
        "id": id,
        "at": now_rfc3339(),
        "session": c.session,
        "start": c.start,
        "duration_ms": c.duration_ms,
        "models": models,
    });
    append(&journal, &record)?;
    println!(
        "recorded: {id} session {} ({} models)",
        c.session,
        c.models.len()
    );
    Ok(())
}

/// One report row: sums of the records of one role, model and origin.
#[derive(Default)]
struct Row {
    measured: [Option<u64>; 4],
    estimate: Option<u64>,
    /// Some measured record of the row has a partial output (STP-5 AC-1, AC-2).
    partial: bool,
    legacy_total: Option<u64>,
    legacy_kind: Option<String>,
}

/// The sessions table (STP-5 AC-4): per session (its line with the greatest start plus duration)
/// and model, the session's output and thinking, the output of all tickets' measured records of
/// that session (overlapping windows of one transcript and model counted once), and the gap.
fn sessions_table(
    path: &Path,
    records: &[Map<String, Value>],
    problems: &mut Vec<String>,
) -> Result<String, String> {
    let mut latest: std::collections::BTreeMap<String, Map<String, Value>> = Default::default();
    let mut seen = std::collections::BTreeSet::new();
    for (n, entry) in read(path)? {
        let m = match entry {
            Entry::V1(m) => m,
            _ => {
                problems.push(format!("problem: .stapel/sessions.jsonl:{n}"));
                continue;
            }
        };
        let id = m
            .get("id")
            .and_then(Value::as_str)
            .filter(|i| !i.is_empty())
            .map(String::from);
        let session = m.get("session").and_then(Value::as_str).map(String::from);
        let end = m
            .get("start")
            .and_then(Value::as_u64)
            .zip(m.get("duration_ms").and_then(Value::as_u64));
        let (Some(id), Some(session), Some((start, duration)), true) = (
            id,
            session,
            end,
            m.get("models").is_some_and(Value::is_object),
        ) else {
            problems.push(format!("problem: .stapel/sessions.jsonl:{n}"));
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        let later = |o: &Map<String, Value>| {
            let e = |k: &str| o.get(k).and_then(Value::as_u64).unwrap_or(0);
            start.saturating_add(duration) > e("start").saturating_add(e("duration_ms"))
        };
        if latest.get(&session).is_none_or(later) {
            latest.insert(session, m);
        }
    }
    let mut table: Vec<[String; 8]> = vec![
        [
            "session",
            "model",
            "output",
            "thinking",
            "records",
            "partial",
            "duplicates",
            "gap",
        ]
        .map(String::from),
    ];
    for (session, line) in &latest {
        let models = line
            .get("models")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let ours = |r: &&Map<String, Value>| {
            r.get("transcript")
                .and_then(Value::as_str)
                .is_some_and(|t| t == session || t.starts_with(&format!("{session}/")))
        };
        let mut names: std::collections::BTreeSet<String> = models.keys().cloned().collect();
        for r in records.iter().filter(ours) {
            if let Some(m) = r.get("model").and_then(Value::as_str) {
                names.insert(m.to_string());
            }
        }
        for model in names {
            let mut group: Vec<&Map<String, Value>> = records
                .iter()
                .filter(ours)
                .filter(|r| r.get("model").and_then(Value::as_str) == Some(model.as_str()))
                .collect();
            // Preferred first: more messages, then a record of this importer, then the later one.
            group.sort_by(|a, b| {
                let key = |r: &Map<String, Value>| {
                    (
                        r.get("messages").and_then(Value::as_u64).unwrap_or(0),
                        r.get("importer").and_then(Value::as_u64) == Some(2),
                        r.get("at")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    )
                };
                key(b).cmp(&key(a))
            });
            let window = |r: &Map<String, Value>| {
                let t = |k: &str| {
                    r.get(k)
                        .and_then(Value::as_str)
                        .and_then(stapel_core::time::parse_transcript_time)
                };
                t("from").zip(t("to"))
            };
            let mut kept: Vec<&Map<String, Value>> = Vec::new();
            let mut duplicates = 0u64;
            for r in group {
                let overlaps = kept.iter().any(|k| {
                    k.get("transcript") == r.get("transcript")
                        && matches!((window(k), window(r)), (Some((a0, a1)), Some((b0, b1))) if a0 <= b1 && b0 <= a1)
                });
                if overlaps {
                    duplicates += 1;
                } else {
                    kept.push(r);
                }
            }
            let sum = kept
                .iter()
                .filter_map(|r| r.get("output").and_then(Value::as_u64))
                .fold(0u64, u64::saturating_add);
            let partial = kept.iter().filter(|r| partial_output(r)).count();
            let counts = models.get(&model);
            let n = |k: &str| counts.and_then(|c| c.get(k)).and_then(Value::as_u64);
            let gap = match n("output") {
                None => "—".to_string(),
                Some(o) => {
                    let g = i128::from(o) - i128::from(sum);
                    if partial > 0 {
                        format!("≤{g}")
                    } else {
                        g.to_string()
                    }
                }
            };
            table.push([
                session.clone(),
                model.clone(),
                cell(n("output")),
                cell(n("thinking")),
                if partial > 0 {
                    format!("≥{sum}")
                } else {
                    sum.to_string()
                },
                partial.to_string(),
                duplicates.to_string(),
                gap,
            ]);
        }
    }
    let mut block = vec!["sessions".to_string()];
    block.extend(render(&table));
    block.push(
        "note: the gap of a session continued from another file may include that file's totals"
            .to_string(),
    );
    Ok(block.join("\n"))
}

/// A measured record whose output is a lower bound: it says so (`partial`), or it is a subagent
/// record written before STP-5 (`importer` absent, transcript identity `<session>/<agent>`).
fn partial_output(m: &Map<String, Value>) -> bool {
    let subagent = m
        .get("transcript")
        .and_then(Value::as_str)
        .is_some_and(|t| t.contains('/'));
    m.contains_key("partial") || (subagent && !m.contains_key("importer"))
}

/// A measured record's `partial` is an integer of 1 or more and its `importer` the integer 2, when
/// present (STP-5 AC-3); anything else is a problem line.
fn measured_fields_valid(m: &Map<String, Value>) -> bool {
    m.get("partial")
        .is_none_or(|p| p.as_u64().is_some_and(|n| n >= 1))
        && m.get("importer").is_none_or(|i| i.as_u64() == Some(2))
}

fn add_to(slot: &mut Option<u64>, value: Option<u64>) {
    if let Some(v) = value {
        *slot = Some(slot.unwrap_or(0).saturating_add(v));
    }
}

fn cell(v: Option<u64>) -> String {
    v.map_or_else(|| "—".to_string(), |n| n.to_string())
}

/// The smallest width of each column; columns grow with their longest cell, two spaces apart.
const MIN_WIDTHS: [usize; 8] = [14, 28, 9, 9, 12, 13, 10, 17];

fn render(rows: &[[String; 8]]) -> Vec<String> {
    let mut widths = MIN_WIDTHS;
    for row in rows {
        for (i, c) in row.iter().enumerate() {
            let need = c.chars().count() + 2;
            widths[i] = widths[i].max(need);
        }
    }
    rows.iter()
        .map(|r| {
            let mut out = format!("{:<w0$}{:<w1$}", r[0], r[1], w0 = widths[0], w1 = widths[1]);
            for (i, c) in r.iter().enumerate().skip(2) {
                out.push_str(&format!("{c:>w$}", w = widths[i]));
            }
            out
        })
        .collect()
}

/// Ticket folders in key order, read without their `state.json`.
fn ticket_folders(root: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut out: Vec<_> = std::fs::read_dir(tickets_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .collect();
    out.sort();
    out
}

/// `stapel tokens [KEY]` (STP-3 AC-4). Reads only `tokens.jsonl` files.
pub fn report(key: Option<&str>) -> Result<std::process::ExitCode, String> {
    let (root, _config) = repo::open()?;
    let folders: Vec<_> = ticket_folders(&root)
        .into_iter()
        .filter(|(k, _)| key.is_none_or(|want| k.eq_ignore_ascii_case(want)))
        .collect();
    if let (Some(k), true) = (key, folders.is_empty()) {
        return Err(format!("no ticket {k} in .stapel/tickets"));
    }
    let mut blocks = Vec::new();
    let mut problems = Vec::new();
    let mut measured_records: Vec<Map<String, Value>> = Vec::new();
    for (ticket_key, dir) in folders {
        let path = dir.join("tokens.jsonl");
        if !path.exists() {
            continue;
        }
        // Key: role, model, origin (0 for v1 records, 1 for legacy ones), legacy kind.
        let mut rows: std::collections::BTreeMap<(String, String, u8, String), Row> =
            Default::default();
        for (n, entry) in read(&path)? {
            let text = |m: &Map<String, Value>, k: &str| {
                m.get(k).and_then(Value::as_str).unwrap_or("—").to_string()
            };
            let problem = format!("problem: .stapel/tickets/{ticket_key}/tokens.jsonl:{n}");
            match entry {
                Entry::Problem(_) => problems.push(problem),
                Entry::V1(m) => {
                    let num = |k: &str| m.get(k).and_then(Value::as_u64);
                    let key = (text(&m, "role"), text(&m, "model"), 0, String::new());
                    match m.get("source").and_then(Value::as_str) {
                        Some("estimate") => {
                            add_to(&mut rows.entry(key).or_default().estimate, num("estimate"))
                        }
                        Some("measured") if !measured_fields_valid(&m) => problems.push(problem),
                        Some("measured") => {
                            measured_records.push(m.clone());
                            let row = rows.entry(key).or_default();
                            row.partial |= partial_output(&m);
                            for (slot, k) in row.measured.iter_mut().zip([
                                "input",
                                "output",
                                "cache_read",
                                "cache_write",
                            ]) {
                                add_to(slot, num(k));
                            }
                        }
                        // A record that is neither is not counted as either.
                        _ => problems.push(problem),
                    }
                }
                Entry::Legacy(m) => {
                    let kind = text(&m, "kind");
                    let num = |k: &str| m.get(k).and_then(Value::as_u64);
                    let total = num("total_tokens")
                        .or_else(|| Some(num("input_tokens")? + num("output_tokens")?));
                    let row = rows
                        .entry((text(&m, "role"), text(&m, "model"), 1, kind.clone()))
                        .or_default();
                    add_to(&mut row.legacy_total, total);
                    row.legacy_kind = Some(kind);
                }
            }
        }
        let mut table: Vec<[String; 8]> = vec![
            [
                "role",
                "model",
                "input",
                "output",
                "cache read",
                "cache write",
                "estimate",
                "legacy",
            ]
            .map(String::from),
        ];
        for ((role, model, origin, _), row) in &rows {
            let legacy = match (&row.legacy_kind, origin) {
                (Some(kind), 1) => format!("{} {kind}", cell(row.legacy_total)),
                _ => "—".into(),
            };
            let mut m = row.measured.map(cell);
            if row.partial {
                m[1] = format!("≥{}", row.measured[1].unwrap_or(0));
            }
            table.push([
                role.clone(),
                model.clone(),
                m[0].clone(),
                m[1].clone(),
                m[2].clone(),
                m[3].clone(),
                cell(row.estimate),
                legacy,
            ]);
        }
        let mut block = vec![format!("ticket: {ticket_key}")];
        block.extend(render(&table));
        blocks.push(block.join("\n"));
    }
    if key.is_none() {
        let path = root.join(".stapel/sessions.jsonl");
        if path.exists() {
            blocks.push(sessions_table(&path, &measured_records, &mut problems)?);
        }
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
