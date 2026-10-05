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
