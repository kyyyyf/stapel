//! Append-only journals: one JSON object per line (STP-3 AC-5).
//!
//! A line is written with one `write` call in append mode, so writers never lose or merge each
//! other's lines on a local file system. Nothing ever rewrites a journal: a line that cannot be
//! read is reported where it is and kept.

use serde_json::{Map, Value};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// A journal line, newline included, is at most this long.
pub const MAX_LINE: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// A line with `v: 1`.
    V1(Map<String, Value>),
    /// A line written before the journals had a version (STP-1, STP-2).
    Legacy(Map<String, Value>),
    /// A line that cannot be read, with the reason.
    Problem(String),
}

/// Appends `record` as one line.
pub fn append(path: &Path, record: &Value) -> Result<(), String> {
    let mut line = serde_json::to_string(record).map_err(|e| e.to_string())?;
    line.push('\n');
    if line.len() > MAX_LINE {
        return Err(format!(
            "a journal line is at most 64 KiB; this one is {} bytes",
            line.len()
        ));
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    // A line cut short by a crash must not swallow the next record.
    if ends_without_newline(&mut file).map_err(|e| format!("{}: {e}", path.display()))? {
        line.insert(0, '\n');
    }
    let written = file
        .write(line.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if written != line.len() {
        return Err(format!(
            "{}: short write ({written} of {} bytes); the journal may need a look",
            path.display(),
            line.len()
        ));
    }
    Ok(())
}

fn ends_without_newline(file: &mut std::fs::File) -> std::io::Result<bool> {
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(len - 1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

/// Every line with its number (1-based); empty lines are skipped; a missing file has no lines.
pub fn read(path: &Path) -> Result<Vec<(usize, Entry)>, String> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut entries = Vec::new();
    for (i, raw) in bytes.split(|b| *b == b'\n').enumerate() {
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        if raw.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        entries.push((i + 1, parse_line(raw)));
    }
    Ok(entries)
}

fn parse_line(raw: &[u8]) -> Entry {
    if raw.len() >= MAX_LINE {
        return Entry::Problem("longer than 64 KiB".into());
    }
    let Ok(text) = std::str::from_utf8(raw) else {
        return Entry::Problem("not UTF-8".into());
    };
    let map = match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => map,
        Ok(_) => return Entry::Problem("not a JSON object".into()),
        Err(e) => return Entry::Problem(format!("not JSON: {e}")),
    };
    match map.get("v") {
        None => Entry::Legacy(map),
        Some(v) if v == 1 => Entry::V1(map),
        Some(v) => Entry::Problem(format!("unknown version {v}")),
    }
}

/// `(line, reason)` of each problem line in a journal.
pub fn problems(path: &Path) -> Vec<(usize, String)> {
    match read(path) {
        Ok(entries) => entries
            .into_iter()
            .filter_map(|(n, e)| match e {
                Entry::Problem(reason) => Some((n, reason)),
                _ => None,
            })
            .collect(),
        Err(reason) => vec![(0, reason)],
    }
}

/// A record id: `prefix` and 12 hex digits from 48 random bits.
pub fn new_id(prefix: &str) -> String {
    let mut bytes = [0u8; 6];
    getrandom::getrandom(&mut bytes).expect("the OS random generator works");
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{prefix}-{hex}")
}

/// Appends a decision to a ticket's `decisions.jsonl` (STP-3 AC-1).
pub fn record_decision(ticket_dir: &Path, fields: Value) -> Result<(), String> {
    let mut record = serde_json::json!({
        "v": 1,
        "id": new_id("d"),
        "at": crate::time::now_rfc3339(),
    });
    if let (Value::Object(r), Value::Object(f)) = (&mut record, fields) {
        r.extend(f);
    }
    append(&ticket_dir.join("decisions.jsonl"), &record)
        .map_err(|e| format!("warning: decision not recorded: {e}"))
}
