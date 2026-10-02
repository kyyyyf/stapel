//! Ticket folders under `.stapel/tickets/` and their keys (STP-2 AC-1, AC-4).

use crate::config::Config;
use std::path::{Path, PathBuf};

pub fn tickets_dir(root: &Path) -> PathBuf {
    root.join(".stapel/tickets")
}

/// The number in a folder name that matches `tickets.key`, compared without case; leading zeros
/// are ignored.
pub fn key_number(config: &Config, name: &str) -> Option<u64> {
    let (prefix, suffix) = config.tickets.key.split_once("{n}")?;
    let lower = name.to_lowercase();
    let rest = lower.strip_prefix(&prefix.to_lowercase())?;
    let digits = rest.strip_suffix(&suffix.to_lowercase())?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The key of the next ticket: one more than the largest number among existing ticket folders.
pub fn next_key(config: &Config, root: &Path) -> String {
    let largest = std::fs::read_dir(tickets_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| key_number(config, &e.file_name().to_string_lossy()))
        .max()
        .unwrap_or(0);
    config
        .tickets
        .key
        .replace("{n}", &(largest + 1).to_string())
}
