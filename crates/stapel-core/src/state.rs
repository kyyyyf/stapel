//! `state.json`: the facts of one ticket (STP-2 AC-12).
//!
//! Only facts are stored — confirmations and the closed record; the stage is computed elsewhere.
//! Writes are atomic, unknown fields survive a rewrite, and a file that cannot be read is never
//! overwritten.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub schema_version: u32,
    pub key: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracker: Option<String>,
    #[serde(default)]
    pub confirmations: Vec<Confirmation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed: Option<Closed>,
    /// Fields this version does not know, such as the phase-0 `build` hand permit.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Confirmation {
    pub section: String,
    pub by: String,
    pub at: String,
    pub hash: String,
    pub normal_form: u32,
    #[serde(default)]
    pub depends_on: BTreeMap<String, String>,
    /// The normalized text; kept on the latest confirmation of a section only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// `grant` when confirmed through the permission dialog, `terminal` otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Closed {
    pub by: String,
    pub at: String,
    pub reason: String,
}

#[derive(Debug)]
pub enum Loaded {
    State(State),
    /// A file without `schema_version`, written by hand in STP-1.
    Legacy(Value),
    /// Missing, not JSON, a wrong shape or another schema version; the reason says which.
    Unreadable(String),
}

impl State {
    pub fn new(key: &str, title: &str) -> State {
        State {
            schema_version: SCHEMA_VERSION,
            key: key.to_string(),
            title: title.to_string(),
            tracker: None,
            confirmations: Vec::new(),
            closed: None,
            extra: Map::new(),
        }
    }
}

pub fn load(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => return Loaded::Unreadable(format!("{}: {e}", path.display())),
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return Loaded::Unreadable(format!("{} is not valid JSON: {e}", path.display())),
    };
    match value.get("schema_version") {
        None if value.is_object() => return Loaded::Legacy(value),
        Some(v) if v != SCHEMA_VERSION => {
            return Loaded::Unreadable(format!(
                "{} has schema_version {v}; this stapel reads {SCHEMA_VERSION}",
                path.display()
            ));
        }
        _ => {}
    }
    match serde_json::from_value(value) {
        Ok(state) => Loaded::State(state),
        Err(e) => Loaded::Unreadable(format!(
            "{} does not match schema_version {SCHEMA_VERSION}: {e}",
            path.display()
        )),
    }
}

pub fn save(path: &Path, state: &State) -> Result<(), String> {
    save_with(path, state, || Ok(()))
}

/// Writes atomically; `before_rename` runs after the temporary file is complete and lets tests
/// inject a failure at the one point where a crash matters.
pub fn save_with(
    path: &Path,
    state: &State,
    before_rename: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("{} has no folder", path.display()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let mut text = serde_json::to_string_pretty(state).expect("state serializes");
    text.push('\n');

    let result = (|| {
        let _ = std::fs::remove_file(&temp);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| format!("{}: {e}", temp.display()))?;
        file.write_all(text.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| format!("{}: {e}", temp.display()))?;
        before_rename()?;
        std::fs::rename(&temp, path).map_err(|e| format!("{}: {e}", path.display()))?;
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
        return result;
    }
    remove_leftover_temps(dir, &name);
    Ok(())
}

/// Temporary files of earlier writes that were interrupted.
fn remove_leftover_temps(dir: &Path, name: &str) {
    let prefix = format!(".{name}.");
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let n = entry.file_name().to_string_lossy().into_owned();
            if n.starts_with(&prefix) && n.ends_with(".tmp") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}
