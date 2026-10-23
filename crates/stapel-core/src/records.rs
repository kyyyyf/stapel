//! Machine records that stapel appends for a person (STP-18): the checks of a findings file, the
//! lines to append and their hash. The CLI and the guard's dialog call the same functions, so the
//! dialog refuses what the command refuses.

use crate::journal::{self, Entry, MAX_LINE};
use crate::tickets::{Status, Ticket, resolve};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

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
/// The fate list of a finding.
pub const FATES: [&str; 6] = ["open", "patch", "bad_plan", "intent_gap", "defer", "no-change"];
/// The verdict list of a triage.
pub const VERDICTS: [&str; 5] = ["high", "medium", "low", "false", "maybe-false"];

/// The records of a findings file that passed every check.
#[derive(Debug)]
pub struct Findings {
    /// The exact lines the command appends, without newlines, in file order.
    pub lines: Vec<String>,
    /// The `id` of each record.
    pub ids: Vec<String>,
    /// SHA-256 of the lines, each followed by a newline, as lowercase hex.
    pub hash: String,
    /// What the dialog shows of each record.
    pub rows: Vec<Row>,
}

/// The fields of a finding that the permission dialog shows.
#[derive(Debug)]
pub struct Row {
    pub id: String,
    pub severity: String,
    pub promise: String,
    pub claim: String,
    pub fate: String,
    pub stage: String,
    pub role: String,
    /// The round as given: a number or a string, shown without quotes.
    pub round: String,
}

/// A ticket that accepts records: open or closed, not legacy or unreadable.
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

/// A findings import that passed every check.
#[derive(Debug)]
pub struct FindingsPlan {
    pub key: String,
    pub closed: bool,
    pub target: PathBuf,
    pub findings: Findings,
}

/// The importer of a record: `{"by":"agent"|"person","user":<name>}` as JSON text.
pub fn importer_json(agent: bool, user: &str) -> String {
    format!(
        "{{\"by\":\"{}\",\"user\":{}}}",
        if agent { "agent" } else { "person" },
        serde_json::to_string(user).expect("a string is JSON")
    )
}

/// `line` with `,"imported_by":<importer>` put before its last `}`, after trailing whitespace is
/// removed; the rest stays byte for byte. `None` when the line does not end in `}`.
pub fn with_importer(line: &str, importer: &str) -> Option<String> {
    let body = line.trim_end().strip_suffix('}')?;
    Some(format!("{body},\"imported_by\":{importer}}}"))
}

/// Every check of `findings import FILE [KEY]`; `base` is the directory that a relative `FILE`
/// is read from.
pub fn plan_findings(
    root: &Path,
    base: &Path,
    file: &str,
    key: Option<&str>,
    importer: &str,
) -> Result<FindingsPlan, String> {
    let ticket = journal_ticket(root, key)?;
    let text = read_input(root, base, file)?;
    let target = ticket.dir.join("findings.jsonl");
    let label = format!(".stapel/tickets/{}/findings.jsonl", ticket.key);
    let findings = check_findings(file, &text, &ticket.key, &target, &label, importer)?;
    Ok(FindingsPlan {
        closed: matches!(ticket.status, Status::Closed(_)),
        key: ticket.key,
        target,
        findings,
    })
}

/// The largest count one token record may carry (the rule of `tokens add`).
pub const MAX_COUNT: u64 = 1_000_000_000_000_000;

const COUNT_FIELDS: [&str; 5] = ["input", "output", "cache_read", "cache_write", "estimate"];
const TOKEN_FIELDS: [&str; 13] = [
    "v",
    "id",
    "ticket",
    "role",
    "model",
    "source",
    "input",
    "output",
    "cache_read",
    "cache_write",
    "estimate",
    "step",
    "note",
];
const SHORT_TEXT: &str = "must be a non-blank string of at most 256 bytes with no control character";

/// One token record of a file that passed the field rules (STP-19 AC-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRecord {
    pub id: String,
    pub role: String,
    pub model: String,
    /// `measured` or `estimate`.
    pub source: String,
    /// `input`, `output`, `cache_read`, `cache_write`; absent counts are `None`.
    pub counts: [Option<u64>; 4],
    pub estimate: Option<u64>,
    pub step: Option<String>,
    pub note: Option<String>,
}

/// The records of a token file that passed every check.
#[derive(Debug)]
pub struct Tokens {
    pub records: Vec<TokenRecord>,
    /// The `id` of each record.
    pub ids: Vec<String>,
    /// SHA-256 of the base name and `\n`, then each record line as read and `\n`, as lowercase hex.
    pub hash: String,
}

/// A token import that passed every check.
#[derive(Debug)]
pub struct TokensPlan {
    pub key: String,
    pub closed: bool,
    pub target: PathBuf,
    /// The last component of `FILE` as typed.
    pub base_name: String,
    pub tokens: Tokens,
}

impl TokensPlan {
    /// The exact lines to append, without newlines, in file order; `at` is the time of the run.
    pub fn lines(&self, at: &str) -> Vec<String> {
        let text = |s: &str| serde_json::to_string(s).expect("a string is JSON");
        self.tokens
            .records
            .iter()
            .map(|r| {
                let mut line = format!(
                    "{{\"v\":1,\"id\":{},\"at\":{},\"ticket\":{},\"role\":{},\"model\":{},\"source\":{}",
                    text(&r.id),
                    text(at),
                    text(&self.key),
                    text(&r.role),
                    text(&r.model),
                    text(&r.source)
                );
                let counts = r.counts.iter().chain([&r.estimate]);
                for (name, count) in COUNT_FIELDS.iter().zip(counts) {
                    if let Some(n) = count {
                        line.push_str(&format!(",\"{name}\":{n}"));
                    }
                }
                if let Some(step) = &r.step {
                    line.push_str(&format!(",\"step\":{}", text(step)));
                }
                if let Some(note) = &r.note {
                    line.push_str(&format!(",\"note\":{}", text(note)));
                }
                line.push_str(&format!(",\"imported_from\":{}}}", text(&self.base_name)));
                line
            })
            .collect()
    }
}

/// Every check of `tokens import-records FILE [KEY]`; `base` is the directory that a relative
/// `FILE` is read from.
pub fn plan_tokens(
    root: &Path,
    base: &Path,
    file: &str,
    key: Option<&str>,
) -> Result<TokensPlan, String> {
    let ticket = journal_ticket(root, key)?;
    let text = read_input(root, base, file)?;
    // `read_input` refused `.`, `..` and `/` already; a regular file has a base name.
    let base_name = Path::new(file)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{file}: not a regular file"))?
        .to_string();
    let target = ticket.dir.join("tokens.jsonl");
    let label = format!(".stapel/tickets/{}/tokens.jsonl", ticket.key);
    let tokens = check_tokens(file, &base_name, &text, &ticket.key, &target, &label)?;
    Ok(TokensPlan {
        closed: matches!(ticket.status, Status::Closed(_)),
        key: ticket.key,
        target,
        base_name,
        tokens,
    })
}

fn short_text_field(m: &Map<String, Value>, name: &str) -> Result<String, String> {
    match m.get(name) {
        None => Err(format!("{name}: missing")),
        Some(Value::String(s))
            if !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control) =>
        {
            Ok(s.clone())
        }
        Some(_) => Err(format!("{name}: {SHORT_TEXT}")),
    }
}

/// The first rule a token record breaks, as `<field>: <reason>`.
fn check_token_record(m: &Map<String, Value>, key: &str) -> Result<TokenRecord, String> {
    text_field(m, "id")?;
    let Some(Value::String(id)) = m.get("id") else {
        unreachable!("text_field needs a text id");
    };
    if id.chars().count() > 64 || id.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(
            "id: must be 1 to 64 characters with no whitespace or control character".into(),
        );
    }
    if let Some(ticket) = m.get("ticket")
        && ticket.as_str() != Some(key)
    {
        return Err(format!("ticket: must be {key}"));
    }
    let role = short_text_field(m, "role")?;
    let model = short_text_field(m, "model")?;
    let source = match m.get("source") {
        None => return Err("source: missing".into()),
        Some(Value::String(s)) if s == "measured" || s == "estimate" => s.clone(),
        Some(_) => return Err("source: must be one of measured, estimate".into()),
    };
    if source == "measured" {
        for name in ["input", "output"] {
            if !m.contains_key(name) {
                return Err(format!("{name}: missing"));
            }
        }
        if m.contains_key("estimate") {
            return Err("estimate: not allowed in a measured record".into());
        }
    } else {
        if !m.contains_key("estimate") {
            return Err("estimate: missing".into());
        }
        for name in ["input", "output", "cache_read", "cache_write"] {
            if m.contains_key(name) {
                return Err(format!("{name}: not allowed in an estimate record"));
            }
        }
    }
    let mut values = [None; 5];
    for (slot, name) in values.iter_mut().zip(COUNT_FIELDS) {
        if let Some(value) = m.get(name) {
            match value.as_u64() {
                Some(n) if n <= MAX_COUNT => *slot = Some(n),
                _ => {
                    return Err(format!(
                        "{name}: must be a whole number from 0 to 10^15"
                    ));
                }
            }
        }
    }
    let step = if m.contains_key("step") {
        Some(short_text_field(m, "step")?)
    } else {
        None
    };
    let note = match m.get("note") {
        None => None,
        Some(Value::String(s))
            if s.len() <= 4096 && !s.chars().any(|c| c.is_control() && c != '\n') =>
        {
            Some(s.clone())
        }
        Some(_) => {
            return Err(
                "note: must be a string of at most 4096 bytes with no control character but newline"
                    .into(),
            );
        }
    };
    if m.get("v").is_some_and(|v| v != 1) {
        return Err("v: must be 1".into());
    }
    // "First" follows line order: serde_json `preserve_order`, enabled by stapel-cli.
    if let Some(other) = m.keys().find(|k| !TOKEN_FIELDS.contains(&k.as_str())) {
        return Err(format!("{other}: not allowed in a token record"));
    }
    Ok(TokenRecord {
        id: id.clone(),
        role,
        model,
        source,
        counts: [values[0], values[1], values[2], values[3]],
        estimate: values[4],
        step,
        note,
    })
}

/// Checks the text of `file` against the token record rules and the target file. `file` is the name
/// as typed, `base_name` its last component, `key` the folder key of the ticket and `target` its
/// `tokens.jsonl`, shown as `target_label`. Reasons start with a place (`file:line` or a path).
pub fn check_tokens(
    file: &str,
    base_name: &str,
    text: &str,
    key: &str,
    target: &Path,
    target_label: &str,
) -> Result<Tokens, String> {
    let mut records: Vec<TokenRecord> = Vec::new();
    let mut numbers: Vec<usize> = Vec::new();
    let mut raws: Vec<&str> = Vec::new();
    let mut known: Vec<String> = Vec::new();
    // The target is read first as a set of ids; its own defects are reported after the records.
    let existing = journal::read(target)?;
    for (_, entry) in &existing {
        let map = match entry {
            Entry::V1(m) | Entry::Legacy(m) => m,
            Entry::Problem(_) => continue,
        };
        if let Some(Value::String(id)) = map.get("id") {
            known.push(id.clone());
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
        if records.len() == MAX_RECORDS {
            return Err(format!(
                "{file}:{n}: more than {MAX_RECORDS} records; split the file"
            ));
        }
        let record =
            check_token_record(&map, key).map_err(|reason| format!("{file}:{n}: {reason}"))?;
        if let Some(at) = records.iter().position(|r| r.id == record.id) {
            return Err(format!(
                "{file}:{n}: id {} repeats line {}",
                record.id, numbers[at]
            ));
        }
        if known.contains(&record.id) {
            return Err(format!(
                "{file}:{n}: id {} is already in {target_label}",
                record.id
            ));
        }
        numbers.push(n);
        raws.push(raw);
        records.push(record);
    }
    if records.is_empty() {
        return Err(format!("{file}: no records"));
    }
    for (n, entry) in &existing {
        if let Entry::Problem(reason) = entry
            && !journal::is_unknown_version(reason)
        {
            return Err(format!("{target_label}:{n}: {reason}"));
        }
    }
    let mut digest = Sha256::new();
    digest.update(base_name.as_bytes());
    digest.update(b"\n");
    for raw in raws {
        digest.update(raw.as_bytes());
        digest.update(b"\n");
    }
    let hash = digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(Tokens {
        ids: records.iter().map(|r| r.id.clone()).collect(),
        records,
        hash,
    })
}

/// A ledger record that passed every check.
#[derive(Debug)]
pub struct LedgerPlan {
    pub key: String,
    pub closed: bool,
    pub kind: String,
    pub note: String,
    pub record: LedgerRecord,
}

/// Every check of `ledger add --kind KIND --note TEXT [KEY]`.
pub fn plan_ledger(
    root: &Path,
    kind: &str,
    note: &str,
    key: Option<&str>,
    importer: Option<&str>,
) -> Result<LedgerPlan, String> {
    let ticket = journal_ticket(root, key)?;
    let record = check_ledger(root, kind, &ticket.key, note, importer)?;
    Ok(LedgerPlan {
        closed: matches!(ticket.status, Status::Closed(_)),
        key: ticket.key,
        kind: kind.to_string(),
        note: note.to_string(),
        record,
    })
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

/// Checks a ledger record and the ledger file of the repository at `root`, and returns the line to
/// append; `at` is today's UTC date. The line, with its newline, is at most `MAX_LINE` bytes.
pub fn check_ledger(
    root: &Path,
    kind: &str,
    key: &str,
    note: &str,
    importer: Option<&str>,
) -> Result<LedgerRecord, String> {
    if !KINDS.contains(&kind) {
        return Err(format!("--kind must be one of {}", KINDS.join(", ")));
    }
    if note.trim().is_empty() {
        return Err("--note must not be empty".into());
    }
    for (n, entry) in &journal::read(&root.join(LEDGER))? {
        if let Entry::Problem(reason) = entry
            && !journal::is_unknown_version(reason)
        {
            return Err(format!("{LEDGER}:{n}: {reason}"));
        }
    }
    let at = &crate::time::now_rfc3339()[..10];
    let text = |s: &str| serde_json::to_string(s).expect("a string is JSON");
    let imported = importer.map_or(String::new(), |i| format!(",\"imported_by\":{i}"));
    let line = format!(
        "{{\"v\":1,\"at\":\"{at}\",\"kind\":{},\"ticket\":{},\"note\":{}{imported}}}",
        text(kind),
        text(key),
        text(note)
    );
    if line.len() + 1 > MAX_LINE {
        return Err(format!(
            "a journal line is at most 64 KiB; this one is {} bytes",
            line.len() + 1
        ));
    }
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
pub fn read_input(root: &Path, base: &Path, file: &str) -> Result<String, String> {
    if file == "-" {
        return Err("-: stdin is not accepted; give a file".into());
    }
    let path = base.join(file);
    let meta = std::fs::metadata(&path).map_err(|e| format!("{file}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{file}: not a regular file"));
    }
    if is_machine_path(root, &path) {
        return Err(format!("{file}: a machine file; it cannot be imported"));
    }
    if meta.len() > MAX_BYTES {
        return Err(format!(
            "{file}: more than {} KiB; split the file",
            MAX_BYTES / 1024
        ));
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("{file}: {e}"))?;
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

/// The optional triage fields of a finding or a triage record: `verdict`, `evidence`, `root`,
/// `note`.
pub fn check_triage_fields(m: &Map<String, Value>) -> Result<(), String> {
    if m.contains_key("verdict") {
        listed(m, "verdict", &VERDICTS)?;
    }
    if let Some(Value::String(v)) = m.get("verdict")
        && (v == "false" || v == "maybe-false")
        && !m.contains_key("evidence")
    {
        return Err(format!("evidence: missing; the verdict {v} needs it"));
    }
    for name in ["evidence", "note"] {
        if m.contains_key(name) {
            text_field(m, name)?;
        }
    }
    if let Some(root) = m.get("root") {
        match root {
            Value::String(s)
                if !s.is_empty()
                    && s.chars().count() <= 64
                    && !s.chars().any(|c| c.is_whitespace() || c.is_control()) => {}
            _ => {
                return Err(
                    "root: must be 1 to 64 characters with no whitespace or control character"
                        .into(),
                );
            }
        }
    }
    Ok(())
}

/// The first rule a record breaks, as `<field>: <reason>`.
fn check_record(m: &Map<String, Value>, key: &str) -> Result<(), String> {
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
    listed(m, "stage", &STAGES)?;
    match m.get("round") {
        None => return Err("round: missing".into()),
        Some(Value::String(s)) if !s.is_empty() => {
            if s.chars().count() > 32 || s.chars().any(|c| c.is_control()) {
                return Err(
                    "round: a string must be 1 to 32 characters with no control character".into(),
                );
            }
        }
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
    listed(m, "fate", &FATES)?;
    if m.contains_key("rule") {
        text_field(m, "rule")?;
    }
    if m.contains_key("kind") {
        return Err(
            "kind: a finding has no kind; triage records go through stapel findings triage"
                .into(),
        );
    }
    check_triage_fields(m)?;
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
    importer: &str,
) -> Result<Findings, String> {
    let mut lines: Vec<String> = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    let mut numbers: Vec<usize> = Vec::new();
    let mut rows: Vec<Row> = Vec::new();
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
            return Err(format!(
                "{file}:{n}: more than {MAX_RECORDS} records; split the file"
            ));
        }
        check_record(&map, key).map_err(|reason| format!("{file}:{n}: {reason}"))?;
        let line = with_importer(raw, importer)
            .ok_or_else(|| format!("{file}:{n}: not a JSON object"))?;
        if line.len() + 1 > MAX_LINE {
            return Err(format!("{file}:{n}: longer than 64 KiB"));
        }
        let Some(Value::String(id)) = map.get("id") else {
            unreachable!("check_record needs a text id");
        };
        if let Some(at) = ids.iter().position(|e| e == id) {
            return Err(format!("{file}:{n}: id {id} repeats line {}", numbers[at]));
        }
        if known.iter().any(|(e, _)| e == id) {
            return Err(format!("{file}:{n}: id {id} is already in {target_label}"));
        }
        let field = |name: &str| {
            map.get(name)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        rows.push(Row {
            id: id.clone(),
            severity: field("severity"),
            promise: field("promise"),
            claim: field("claim"),
            fate: field("fate"),
            stage: field("stage"),
            role: field("role"),
            round: match map.get("round") {
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
                None => String::new(),
            },
        });
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
    Ok(Findings {
        lines,
        ids,
        hash,
        rows,
    })
}
