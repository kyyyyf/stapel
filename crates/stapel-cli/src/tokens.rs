//! `stapel tokens`: the token journal of a ticket (STP-3 AC-2..AC-4).

use crate::repo;
use serde_json::{Map, Value, json};
use sha2::Digest;
use stapel_core::journal::{Entry, append, new_id, read};
use stapel_core::tickets::{Ticket, tickets_dir};
use stapel_core::time::{now_rfc3339, parse_cli_time, parse_transcript_time, rfc3339_millis};
use std::path::Path;

use stapel_core::records::MAX_COUNT;

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
    stapel_core::records::journal_ticket(root, key)
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
        warn!("warning: role {role} is not in stapel.toml");
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
    say!("recorded: {id}");
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

/// The id and the record `tokens import` writes for one model's sum; the hooks add `hook`.
pub fn measured_record(
    ticket: &str,
    transcript: &str,
    role: &str,
    s: &Sum,
) -> (String, Map<String, Value>) {
    let mut digest = sha2::Sha256::new();
    for part in [ticket, transcript, &s.model, &s.from, &s.to] {
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
        ("ticket", json!(ticket)),
        ("role", json!(role)),
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
    (id, record)
}

/// Sums of one model over the counted messages.
pub struct Sum {
    pub model: String,
    pub from: String,
    pub to: String,
    pub from_ms: u64,
    pub to_ms: u64,
    pub messages: u64,
    pub fields: [(&'static str, Option<u64>); 4],
    /// Messages without a final line: their output is not known (STP-5 AC-1).
    pub partial: u64,
}

/// The sums per model of the counted messages, sorted by model; a model with no tokens is left
/// out (STP-5). Shared by `tokens import` and the token hooks (STP-26).
pub fn sum_window<'a>(
    messages: impl Iterator<Item = &'a stapel_core::tokens::Message>,
) -> Vec<Sum> {
    let mut sums: Vec<Sum> = Vec::new();
    for m in messages {
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

    sums
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

    let sums = sum_window(
        t.messages
            .iter()
            .filter(|m| m.time_ms >= since && until.is_none_or(|u| m.time_ms < u)),
    );

    say!("skipped: {} error lines", t.error_lines);
    say!("skipped: {} unreadable lines", t.unreadable_lines);
    if sums.is_empty() {
        say!("nothing to import in the window");
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
            say!("already imported: {id}");
        }
        say!("next --since: {next_since}");
        return Ok(());
    }
    if !already.is_empty() {
        return Err(format!(
            "part of this window is imported already ({}); import a range after it",
            already.join(", ")
        ));
    }
    for s in &sums {
        let (id, record) = measured_record(&ticket.key, &transcript, &a.role, s);
        append(&ticket.dir.join("tokens.jsonl"), &Value::Object(record))?;
        if s.partial > 0 {
            say!(
                "recorded: {id} {} ({} messages, partial: {})",
                s.model,
                s.messages,
                s.partial
            );
        } else {
            say!("recorded: {id} {} ({} messages)", s.model, s.messages);
        }
    }
    say!("next --since: {next_since}");
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
        say!("already imported: {id}");
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
    say!(
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

/// The sums of a ticket's attention records (STP-29 AC-9): dialogs, questions, typed, attention,
/// waited and long waits, and the earliest `from` and latest `to`.
#[derive(Default)]
struct AttentionSum {
    counts: [u64; 6],
    from: Option<u64>,
    to: Option<u64>,
}

/// Milliseconds as whole minutes: `35 min`, `1 h 5 min`, `2 h 0 min`.
fn duration(ms: u64) -> String {
    let minutes = ms / 60_000;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// The count of distinct step labels of the ticket among the subjects of every commit reachable
/// from HEAD, merges followed; `—` when git cannot be read.
fn step_count(root: &Path, key: &str) -> String {
    let out = std::process::Command::new("git")
        .args(["log", "--format=%s", "HEAD"])
        .current_dir(root)
        .output();
    let Ok(out) = out else { return "—".into() };
    if !out.status.success() {
        return "—".into();
    }
    let labels: std::collections::BTreeSet<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|s| stapel_core::steps::parse_subject(key, s))
        .map(|(label, _)| label)
        .collect();
    labels.len().to_string()
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

fn render<const N: usize>(rows: &[[String; N]]) -> Vec<String> {
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

/// The first and last timed line of a transcript file, in milliseconds.
fn timed_span(path: &Path) -> Option<(u64, u64)> {
    use stapel_core::tokens::{MAX_TRANSCRIPT_LINE, read_bounded_line};
    let mut reader = std::io::BufReader::new(std::fs::File::open(path).ok()?);
    let mut buf = Vec::new();
    let mut span: Option<(u64, u64)> = None;
    while read_bounded_line(&mut reader, &mut buf, MAX_TRANSCRIPT_LINE).ok()? {
        if buf.len() > MAX_TRANSCRIPT_LINE {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&buf) else {
            continue;
        };
        let Ok(Value::Object(v)) = serde_json::from_str::<Value>(text.trim_end()) else {
            continue;
        };
        let Some(t) = v
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_transcript_time)
        else {
            continue;
        };
        span = Some(span.map_or((t, t), |(a, b)| (a.min(t), b.max(t))));
    }
    span
}

/// The rows of one source of the measure table: a row per model.
fn measure_rows<'a>(
    source: &str,
    messages: impl Iterator<Item = &'a stapel_core::tokens::Message> + Clone,
) -> Vec<[String; 6]> {
    sum_window(messages.clone())
        .into_iter()
        .map(|s| {
            let of_model = |m: &&stapel_core::tokens::Message| m.model == s.model;
            let partial = messages.clone().filter(of_model).any(|m| !m.complete);
            let mut cells = s.fields.map(|(_, v)| cell(v));
            if partial {
                // The streamed output of a message without a final line is a lower bound.
                let total = messages
                    .clone()
                    .filter(of_model)
                    .filter_map(|m| m.usage.output)
                    .fold(0u64, u64::saturating_add);
                cells[1] = format!("≥{total}");
            }
            [
                source.to_string(),
                s.model.clone(),
                cells[0].clone(),
                cells[1].clone(),
                cells[2].clone(),
                cells[3].clone(),
            ]
        })
        .collect()
}

/// `stapel tokens measure FILE` (STP-29 AC-10, AC-11, AC-14): tokens and attention numbers of a main
/// transcript and its subagent files; no repository is opened and nothing is written.
pub fn measure(file: &Path) -> Result<(), crate::fail::Fail> {
    use crate::fail::Fail;
    let refuse = |reason: &str| Fail::refused(format!("{}: {reason}", file.display()));
    let meta = std::fs::metadata(file).map_err(|e| {
        refuse(if e.kind() == std::io::ErrorKind::NotFound {
            "missing"
        } else {
            "not readable"
        })
    })?;
    // The file type is checked before the file is opened: opening a FIFO would wait.
    if !meta.is_file() {
        return Err(refuse("not a file"));
    }
    if std::fs::File::open(file).is_err() {
        return Err(refuse("not readable"));
    }
    let main = stapel_core::tokens::read_transcript(file).map_err(|_| refuse("not readable"))?;
    let Some(session) = main.session.clone() else {
        return Err(refuse("no sessionId"));
    };
    if main.agent.is_some() {
        return Err(refuse("subagent transcript"));
    }
    let events = stapel_core::attention::read_events(file).map_err(|_| refuse("not readable"))?;
    let sums = stapel_core::attention::sum_events(&events);

    let mut subagent_messages = Vec::new();
    let folder = match file.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let mut names: Vec<_> = std::fs::read_dir(folder.join(&session).join("subagents"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    names.sort();
    for path in names {
        if let Ok(t) = stapel_core::tokens::read_transcript(&path)
            && t.session.as_deref() == Some(session.as_str())
        {
            subagent_messages.extend(t.messages);
        }
    }

    let mut table: Vec<[String; 6]> = vec![
        ["source", "model", "input", "output", "cache read", "cache write"].map(String::from),
    ];
    table.extend(measure_rows("main", main.messages.iter()));
    table.extend(measure_rows("subagents", subagent_messages.iter()));
    let mut out = vec![format!("transcript: {session}")];
    out.extend(render(&table));
    out.push(format!(
        "attention: {} dialogs, {} questions, {} messages; {} attention, {} waited ({} over 10 min)",
        sums.dialogs,
        sums.questions,
        sums.typed,
        duration(sums.attention_ms),
        duration(sums.waited_ms),
        sums.long_waits
    ));
    let wall = timed_span(file).map_or(0, |(a, b)| b.saturating_sub(a));
    out.push(format!("wall time: {}", duration(wall)));
    say!("{}", out.join("\n"));
    Ok(())
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
        // Key: role, model, origin (0 for v1 records, 1 for legacy ones,
        // 2 for records with `imported_from`), legacy kind.
        let mut rows: std::collections::BTreeMap<(String, String, u8, String), Row> =
            Default::default();
        let mut attention: Option<AttentionSum> = None;
        for (n, entry) in read(&path)? {
            let text = |m: &Map<String, Value>, k: &str| {
                m.get(k).and_then(Value::as_str).unwrap_or("—").to_string()
            };
            let problem = format!("problem: .stapel/tickets/{ticket_key}/tokens.jsonl:{n}");
            match entry {
                Entry::Problem(_) => problems.push(problem),
                Entry::V1(m) if m.get("kind").and_then(Value::as_str) == Some("attention") => {
                    let num = |k: &str| m.get(k).and_then(Value::as_u64).unwrap_or(0);
                    let sum = attention.get_or_insert_with(Default::default);
                    for (slot, k) in sum.counts.iter_mut().zip([
                        "dialogs",
                        "questions",
                        "typed",
                        "attention_ms",
                        "waited_ms",
                        "long_waits",
                    ]) {
                        *slot = slot.saturating_add(num(k));
                    }
                    let time = |k: &str| m.get(k).and_then(Value::as_str).and_then(parse_cli_time);
                    if let Some(f) = time("from") {
                        sum.from = Some(sum.from.map_or(f, |c| c.min(f)));
                    }
                    if let Some(t) = time("to") {
                        sum.to = Some(sum.to.map_or(t, |c| c.max(t)));
                    }
                }
                Entry::V1(m) => {
                    let num = |k: &str| m.get(k).and_then(Value::as_u64);
                    let imported = m.contains_key("imported_from");
                    let origin = if imported { 2 } else { 0 };
                    let key = (text(&m, "role"), text(&m, "model"), origin, String::new());
                    match m.get("source").and_then(Value::as_str) {
                        Some("estimate") => {
                            add_to(&mut rows.entry(key).or_default().estimate, num("estimate"))
                        }
                        Some("measured") if !measured_fields_valid(&m) => problems.push(problem),
                        Some("measured") => {
                            // The sessions table compares totals with transcript records only.
                            if !imported {
                                measured_records.push(m.clone());
                            }
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
                if *origin == 2 {
                    format!("{role} (file)")
                } else {
                    role.clone()
                },
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
        if let Some(a) = attention {
            let [dialogs, questions, typed, attention_ms, waited_ms, long] = a.counts;
            block.push(format!(
                "attention: {dialogs} dialogs, {questions} questions, {typed} messages; \
                 {} attention, {} waited ({long} over 10 min)",
                duration(attention_ms),
                duration(waited_ms)
            ));
            let wall = a.from.zip(a.to).map_or(0, |(f, t)| t.saturating_sub(f));
            block.push(format!(
                "steps: {}; wall time: {}",
                step_count(&root, &ticket_key),
                duration(wall)
            ));
        }
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
        say!("{out}");
    }
    Ok(if problems.is_empty() {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    })
}

/// The latest `to` per model among the records of every ticket's `tokens.jsonl` that name this
/// transcript; a line or a file that cannot be read is no cursor (STP-26).
/// The newest `at` among those records is returned with them, in milliseconds.
fn cursors(
    root: &Path,
    identity: &str,
) -> (std::collections::HashMap<String, u64>, Option<u64>) {
    let mut cursors = std::collections::HashMap::new();
    let mut newest: Option<u64> = None;
    for ticket in stapel_core::tickets::list(root) {
        let Ok(entries) = read(&ticket.dir.join("tokens.jsonl")) else {
            continue;
        };
        for (_, entry) in entries {
            let (Entry::V1(map) | Entry::Legacy(map)) = entry else {
                continue;
            };
            let text = |key: &str| map.get(key).and_then(Value::as_str);
            if text("transcript") != Some(identity) {
                continue;
            }
            if let Some(at) = text("at").and_then(parse_cli_time) {
                newest = Some(newest.map_or(at, |n| n.max(at)));
            }
            let (Some(model), Some(to)) = (text("model"), text("to")) else {
                continue;
            };
            if let Some(ms) = parse_cli_time(to) {
                let at = cursors.entry(model.to_string()).or_insert(ms);
                *at = (*at).max(ms);
            }
        }
    }
    (cursors, newest)
}

/// The attention cursor: the latest `to` of the attention records of `identity` in every ticket's
/// `tokens.jsonl`. Other lines are skipped (STP-29).
fn attention_cursor(root: &Path, identity: &str) -> Option<u64> {
    let mut cursor: Option<u64> = None;
    for ticket in stapel_core::tickets::list(root) {
        let Ok(entries) = read(&ticket.dir.join("tokens.jsonl")) else {
            continue;
        };
        for (_, entry) in entries {
            let (Entry::V1(map) | Entry::Legacy(map)) = entry else {
                continue;
            };
            let text = |key: &str| map.get(key).and_then(Value::as_str);
            if text("kind") != Some("attention") || text("transcript") != Some(identity) {
                continue;
            }
            if let Some(ms) = text("to").and_then(parse_cli_time) {
                cursor = Some(cursor.map_or(ms, |c| c.max(ms)));
            }
        }
    }
    cursor
}

/// How far a hook call reads a transcript (STP-26).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Every message; one without a final line is `partial`.
    All,
    /// Up to the last complete message, and only when the transcript's last record is old.
    Stop,
}

/// A `stop` call waits this long after the last record of the transcript.
const STOP_WAIT_MS: u64 = 10 * 60 * 1000;

/// The time up to which `stop` reads: the last message with a final line, moved back while a later
/// message has the same time. `None` when no message is complete; the bool is true when the end is
/// exclusive.
fn stop_end(messages: &[stapel_core::tokens::Message]) -> Option<(u64, bool)> {
    let last = messages.iter().rposition(|m| m.complete)?;
    let at = messages[last].time_ms;
    let lagging = messages[last + 1..].iter().any(|m| m.time_ms == at);
    Some((at, lagging))
}

/// The OS lock on `tokens.lock` in the git common folder; released on drop.
struct TokensLock(std::fs::File);

impl Drop for TokensLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Takes the lock, waiting at most 20 s; `None` when it stays held or cannot be made.
fn take_tokens_lock(root: &Path) -> Option<TokensLock> {
    let dir = stapel_core::worktree::stapel_dir(root).ok()?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("tokens.lock"))
        .ok()?;
    let start = std::time::Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Some(TokensLock(file)),
            Err(std::fs::TryLockError::WouldBlock) => {
                if start.elapsed() >= std::time::Duration::from_secs(20) {
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

/// The transcript and its identity, or the reason it cannot be read (STP-26 AC-9).
fn read_for_hook(path: &Path) -> Result<(stapel_core::tokens::Transcript, String), &'static str> {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => {}
        Ok(_) => return Err("not a file"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err("missing"),
        Err(_) => return Err("not readable"),
    }
    let t = stapel_core::tokens::read_transcript(path).map_err(|_| "not readable")?;
    let identity = t.identity().ok_or("no sessionId")?;
    Ok((t, identity))
}

/// A gap record for an unreadable transcript, into the one open ticket and never into a closed
/// one; once for the same ticket, path and reason (STP-26 AC-9).
fn gap_record(root: &Path, transcript: &Path, role: &str, hook: &str, reason: &str) -> Option<()> {
    let _lock = take_tokens_lock(root)?;
    let tickets = stapel_core::tickets::list(root);
    let by_branch = stapel_core::tickets::branch_ticket(root)
        .filter(|t| matches!(t.status, stapel_core::tickets::Status::Open(_)))
        .and_then(|b| tickets.iter().find(|t| t.key == b.key));
    let mut open = tickets
        .iter()
        .filter(|t| matches!(t.status, stapel_core::tickets::Status::Open(_)));
    let ticket = match by_branch {
        Some(t) => t,
        None => match (open.next(), open.next()) {
            (Some(t), None) => t,
            _ => return None,
        },
    };
    let shown = transcript.to_string_lossy();
    let mut digest = sha2::Sha256::new();
    for part in ["gap", ticket.key.as_str(), shown.as_ref(), reason] {
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
    let path = ticket.dir.join("tokens.jsonl");
    if path.exists() {
        let entries = read(&path).ok()?;
        let seen = entries.iter().any(|(_, entry)| match entry {
            Entry::V1(map) | Entry::Legacy(map) => map.get("id").and_then(Value::as_str) == Some(&id),
            _ => false,
        });
        if seen {
            return Some(());
        }
    }
    let record = json!({
        "v": 1,
        "id": id,
        "at": now_rfc3339(),
        "ticket": ticket.key,
        "role": role,
        "model": "unknown",
        "source": "estimate",
        "estimate": 0,
        "hook": hook,
        "transcript": shown,
        "note": format!("transcript not read: {reason}"),
    });
    stapel_core::journal::append_all(&path, &[serde_json::to_string(&record).ok()?]).ok()?;
    Some(())
}

/// The hook entry of the token journal (STP-26): the records of every model for the part of the
/// transcript after its cursor, into the target ticket. `None` when nothing could be written.
pub fn hook_records(
    root: &Path,
    transcript: &Path,
    role: &str,
    note: Option<&str>,
    hook: &str,
    reach: Reach,
) -> Option<()> {
    let (t, identity) = match read_for_hook(transcript) {
        Ok(read) => read,
        Err(reason) => return gap_record(root, transcript, role, hook, reason),
    };
    // One hook at a time: cursors are read and records appended under `tokens.lock` (AC-15).
    let _lock = take_tokens_lock(root)?;
    let (cursors, newest) = cursors(root, &identity);
    // `Some(None)`: `stop` with no complete message, so no token window.
    let end = match reach {
        Reach::Stop => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_millis() as u64;
            if newest.is_some_and(|at| now.saturating_sub(at) < STOP_WAIT_MS) {
                return Some(());
            }
            Some(stop_end(&t.messages))
        }
        Reach::All => None,
    };
    let window: Vec<_> = t
        .messages
        .iter()
        .filter(|m| cursors.get(&m.model).is_none_or(|&c| m.time_ms > c))
        .filter(|m| match end {
            Some(None) => false,
            Some(Some((at, true))) => m.time_ms < at,
            Some(Some((at, false))) => m.time_ms <= at,
            None => true,
        })
        .collect();
    // Attention events of a main transcript, from `stop` and `session-end` only (STP-29).
    let events = if (hook == "stop" || hook == "session-end") && !identity.contains('/') {
        let after = attention_cursor(root, &identity);
        let mut events =
            stapel_core::attention::read_events_with_cwd(transcript).unwrap_or_default();
        events.retain(|(e, _)| after.is_none_or(|c| e.time_ms > c));
        events
    } else {
        Vec::new()
    };
    let first = window
        .iter()
        .map(|m| m.time_ms)
        .min()
        .or_else(|| events.iter().map(|(e, _)| e.time_ms).min())?;
    let tickets = stapel_core::tickets::list(root);
    let mut open = tickets
        .iter()
        .filter(|t| matches!(t.status, stapel_core::tickets::Status::Open(_)));
    let by_branch = stapel_core::tickets::branch_ticket(root)
        .and_then(|b| tickets.iter().find(|t| t.key == b.key));
    let ticket = match (open.next(), open.next()) {
        _ if by_branch.is_some() => by_branch?,
        (Some(t), None) => t,
        (Some(_), Some(_)) => return None,
        // No open ticket: the one closed latest, not before the window's first message.
        (None, _) => tickets
            .iter()
            .filter_map(|t| match &t.status {
                stapel_core::tickets::Status::Closed(s) => {
                    let at = parse_cli_time(&s.closed.as_ref()?.at)?;
                    (at >= first).then_some((at, t))
                }
                _ => None,
            })
            .max_by_key(|(at, _)| *at)?
            .1,
    };
    let mut lines = Vec::new();
    for s in &sum_window(window.into_iter()) {
        let (_, mut record) = measured_record(&ticket.key, &identity, role, s);
        record.insert("hook".into(), json!(hook));
        if let Some(note) = note {
            record.insert("note".into(), json!(note));
        }
        lines.push(serde_json::to_string(&Value::Object(record)).ok()?);
    }
    // Group the events by worktree ticket, else the target ticket (AC-15).
    let mut groups: Vec<(&stapel_core::tickets::Ticket, Vec<stapel_core::attention::Event>)> =
        Vec::new();
    for (event, cwd) in events {
        let owner = cwd
            .as_deref()
            .and_then(worktree_key)
            .and_then(|key| tickets.iter().find(|t| t.key == key))
            .unwrap_or(ticket);
        match groups.iter_mut().find(|(t, _)| t.key == owner.key) {
            Some((_, list)) => list.push(event),
            None => groups.push((owner, vec![event])),
        }
    }
    let mut extra: Vec<(&Path, String)> = Vec::new();
    for (owner, events) in &groups {
        let sums = stapel_core::attention::sum_events(events);
        let from = events.iter().map(|e| e.time_ms).min()?;
        let to = events.iter().map(|e| e.time_ms).max()?;
        let mut digest = sha2::Sha256::new();
        for part in [owner.key.as_str(), identity.as_str(), &rfc3339_millis(to)] {
            digest.update(part.as_bytes());
            digest.update([0u8]);
        }
        let hex: String = digest
            .finalize()
            .iter()
            .take(6)
            .map(|b| format!("{b:02x}"))
            .collect();
        let record = json!({
            "v": 1,
            "id": format!("a-{hex}"),
            "at": now_rfc3339(),
            "ticket": owner.key,
            "kind": "attention",
            "hook": hook,
            "transcript": identity,
            "from": rfc3339_millis(from),
            "to": rfc3339_millis(to),
            "questions": sums.questions,
            "dialogs": sums.dialogs,
            "typed": sums.typed,
            "waited_ms": sums.waited_ms,
            "attention_ms": sums.attention_ms,
            "long_waits": sums.long_waits,
        });
        let line = serde_json::to_string(&record).ok()?;
        if owner.key == ticket.key {
            lines.push(line);
        } else {
            extra.push((owner.dir.as_path(), line));
        }
    }
    for (dir, line) in extra {
        stapel_core::journal::append_all(&dir.join("tokens.jsonl"), &[line]).ok()?;
    }
    if lines.is_empty() {
        return Some(());
    }
    stapel_core::journal::append_all(&ticket.dir.join("tokens.jsonl"), &lines).ok()?;
    Some(())
}

/// The key after `.claude` and `worktrees` in a path, if the path has them in a row.
fn worktree_key(cwd: &str) -> Option<&str> {
    let parts: Vec<&str> = cwd.split('/').collect();
    parts
        .windows(3)
        .find(|w| w[0] == ".claude" && w[1] == "worktrees" && !w[2].is_empty())
        .map(|w| w[2])
}
