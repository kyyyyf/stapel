//! Machine records that stapel appends for a person (STP-18): the checks of a findings file, the
//! lines to append and their hash. The CLI and the guard's dialog call the same functions, so the
//! dialog refuses what the command refuses.

use crate::journal::{self, Entry, MAX_LINE};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

/// A findings file has at most this many records.
pub const MAX_RECORDS: usize = 100;
/// A findings file is at most this many bytes.
pub const MAX_BYTES: u64 = 256 * 1024;

const STAGES: [&str; 4] = ["spec-review", "step-drift", "code-review", "after-close"];
const ROLES: [&str; 7] = [
    "external",
    "drift",
    "reviewer",
    "fresh-code",
    "builder",
    "orchestrator",
    "human",
];
const SEVERITIES: [&str; 3] = ["HIGH", "MEDIUM", "LOW"];
const PROMISES: [&str; 2] = ["inside", "outside"];
const CATCHABLE: [&str; 4] = ["spec", "design", "test-plan", "code"];

/// The records of a findings file that passed every check.
#[derive(Debug)]
pub struct Findings {
    /// The exact lines the command appends, without newlines, in file order.
    pub lines: Vec<String>,
    /// The `id` of each record.
    pub ids: Vec<String>,
    /// SHA-256 of the lines, each followed by a newline, as lowercase hex.
    pub hash: String,
}

/// The process ledger, relative to the repository root.
pub const LEDGER: &str = ".stapel/process-ledger.jsonl";

/// The kinds a ledger record may have.
pub const KINDS: [&str; 3] = ["finding", "catches", "decision"];

/// A ledger record that passed every check.
#[derive(Debug)]
pub struct LedgerRecord {
    /// The exact line to append, without its newline.
    pub line: String,
    /// SHA-256 of `kind`, `ticket` and `note`, without `at`, as lowercase hex.
    pub hash: String,
}

/// Checks a ledger record and the ledger file `ledger`, shown as `label`, and returns the line to
/// append; `at` is today's UTC date.
pub fn check_ledger(
    ledger: &Path,
    label: &str,
    kind: &str,
    key: &str,
    note: &str,
) -> Result<LedgerRecord, String> {
    if !KINDS.contains(&kind) {
        return Err(format!("--kind must be one of {}", KINDS.join(", ")));
    }
    if note.trim().is_empty() {
        return Err("--note must not be empty".into());
    }
    for (n, entry) in &journal::read(ledger)? {
        if let Entry::Problem(reason) = entry
            && !reason.starts_with("unknown version")
        {
            return Err(format!("{label}:{n}: {reason}"));
        }
    }
    let at = &crate::time::now_rfc3339()[..10];
    let text = |s: &str| serde_json::to_string(s).expect("a string is JSON");
    let line = format!(
        "{{\"v\":1,\"at\":\"{at}\",\"kind\":{},\"ticket\":{},\"note\":{}}}",
        text(kind),
        text(key),
        text(note)
    );
    let mut digest = Sha256::new();
    for part in [kind, key, note] {
        digest.update(part.as_bytes());
        digest.update([0u8]);
    }
    let hash = digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(LedgerRecord { line, hash })
}

/// The text of `file` when it may be imported: a regular UTF-8 file of at most 256 KiB that is not
/// a machine file. Reasons start with `file` as the person typed it.
pub fn read_input(root: &Path, file: &str) -> Result<String, String> {
    if file == "-" {
        return Err("-: stdin is not accepted; give a file".into());
    }
    let meta = std::fs::metadata(file).map_err(|e| format!("{file}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{file}: not a regular file"));
    }
    if is_machine_path(root, Path::new(file)) {
        return Err(format!("{file}: a machine file; it cannot be imported"));
    }
    if meta.len() > MAX_BYTES {
        return Err(format!("{file}: more than 256 KiB; split the file"));
    }
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    String::from_utf8(bytes).map_err(|_| format!("{file}: not UTF-8"))
}

/// Whether the resolved `path` is a machine file of a ticket folder or the ledger, compared
/// without case.
fn is_machine_path(root: &Path, path: &Path) -> bool {
    let (Ok(path), Ok(root)) = (path.canonicalize(), root.canonicalize()) else {
        return false;
    };
    let Ok(rel) = path.strip_prefix(&root) else {
        return false;
    };
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    match parts.as_slice() {
        [a, b, _, f] if a == ".stapel" && b == "tickets" => crate::worktree::MACHINE_FILES
            .iter()
            .any(|m| m.to_lowercase() == *f),
        [a, f] => a == ".stapel" && f == "process-ledger.jsonl",
        _ => false,
    }
}

fn text_field(m: &Map<String, Value>, name: &str) -> Result<(), String> {
    match m.get(name) {
        None => Err(format!("{name}: missing")),
        Some(Value::String(s)) if !s.is_empty() => Ok(()),
        Some(_) => Err(format!("{name}: must be a non-empty string")),
    }
}

fn listed(m: &Map<String, Value>, name: &str, list: &[&str]) -> Result<(), String> {
    match m.get(name) {
        None => Err(format!("{name}: missing")),
        Some(Value::String(s)) if list.contains(&s.as_str()) => Ok(()),
        Some(_) => Err(format!("{name}: must be one of {}", list.join(", "))),
    }
}

/// The first rule a record breaks, as `<field>: <reason>`.
fn check_record(m: &Map<String, Value>, key: &str) -> Result<(), String> {
    text_field(m, "id")?;
    match m.get("ticket") {
        None => return Err("ticket: missing".into()),
        Some(Value::String(s)) if s == key => {}
        Some(_) => return Err(format!("ticket: must be {key}")),
    }
    listed(m, "stage", &STAGES)?;
    match m.get("round") {
        None => return Err("round: missing".into()),
        Some(Value::String(s)) if !s.is_empty() => {}
        Some(Value::Number(n)) if n.as_u64().is_some() => {}
        Some(_) => {
            return Err("round: must be a non-negative whole number or a non-empty string".into());
        }
    }
    listed(m, "role", &ROLES)?;
    listed(m, "severity", &SEVERITIES)?;
    listed(m, "promise", &PROMISES)?;
    text_field(m, "claim")?;
    listed(m, "catchable_at", &CATCHABLE)?;
    text_field(m, "fate")?;
    if m.contains_key("rule") {
        text_field(m, "rule")?;
    }
    if m.get("v").is_some_and(|v| v != 1) {
        return Err("v: must be 1".into());
    }
    Ok(())
}

/// Checks the text of `file` against the findings rules and the target file, and returns the lines
/// to append. `file` is the name as typed, `key` the folder key of the ticket and `target` its
/// `findings.jsonl`, shown as `target_label`. Reasons start with a place (`file:line` or a path).
pub fn check_findings(
    file: &str,
    text: &str,
    key: &str,
    target: &Path,
    target_label: &str,
) -> Result<Findings, String> {
    let mut lines: Vec<String> = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    let mut numbers: Vec<usize> = Vec::new();
    let mut known: Vec<(String, usize)> = Vec::new();
    // The target is read first as a set of ids; its own defects are reported after the records.
    let existing = journal::read(target)?;
    for (n, entry) in &existing {
        let map = match entry {
            Entry::V1(m) | Entry::Legacy(m) => m,
            Entry::Problem(_) => continue,
        };
        if let Some(Value::String(id)) = map.get("id") {
            known.push((id.clone(), *n));
        }
    }
    for (i, raw) in text.split('\n').enumerate() {
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
            return Err(format!("{file}:{n}: more than 100 records; split the file"));
        }
        check_record(&map, key).map_err(|reason| format!("{file}:{n}: {reason}"))?;
        let Some(Value::String(id)) = map.get("id") else {
            unreachable!("check_record needs a text id");
        };
        if let Some(at) = ids.iter().position(|e| e == id) {
            return Err(format!("{file}:{n}: id {id} repeats line {}", numbers[at]));
        }
        if known.iter().any(|(e, _)| e == id) {
            return Err(format!("{file}:{n}: id {id} is already in {target_label}"));
        }
        ids.push(id.clone());
        numbers.push(n);
        lines.push(raw.to_string());
    }
    if lines.is_empty() {
        return Err(format!("{file}: no records"));
    }
    for (n, entry) in &existing {
        if let Entry::Problem(reason) = entry
            && !reason.starts_with("unknown version")
        {
            return Err(format!("{target_label}:{n}: {reason}"));
        }
    }
    let mut digest = Sha256::new();
    for l in &lines {
        digest.update(l.as_bytes());
        digest.update(b"\n");
    }
    let hash = digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(Findings { lines, ids, hash })
}
