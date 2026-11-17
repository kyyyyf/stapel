//! `stapel page [KEY]`: writes `.stapel/tickets/<KEY>/page.md`, the page of the waiting sections
//! (STP-32 AC-1).

use crate::repo;
use stapel_core::confirm::prepare_waiting;
use stapel_core::page::{page_hash, page_text};
use stapel_core::tickets::resolve;
use std::path::Path;
use std::process::Command;

/// True when git ignores `rel` in the repository at `root`.
fn ignored(root: &Path, rel: &str) -> Result<bool, String> {
    let status = Command::new("git")
        .args(["check-ignore", "-q", "--no-index", "--", rel])
        .current_dir(root)
        .status()
        .map_err(|e| format!("could not run git: {e}"))?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err("git could not tell whether the page is ignored".into()),
    }
}

pub fn run(key: Option<&str>) -> Result<(), String> {
    let (root, config) = repo::open()?;
    let ticket = resolve(&root, key)?;
    let waiting = prepare_waiting(&root, &config, &ticket.key)?;
    let Some(first) = waiting.first() else {
        say!("nothing to confirm: {}", ticket.key);
        return Ok(());
    };
    let rel = format!(".stapel/tickets/{}/page.md", first.key);
    if !ignored(&root, &rel)? {
        return Err(format!(
            "{rel} is not ignored by git; add /.stapel/tickets/*/page.md to the ignore lines \
             or run stapel init"
        ));
    }
    let text = page_text(&config, &waiting);
    std::fs::write(root.join(&rel), &text).map_err(|e| format!("{rel}: {e}"))?;
    let hash = page_hash(text.as_bytes());
    let short = &hash[..hash.len().min("sha256:".len() + 12)];
    say!("page: {rel} ({short})");
    Ok(())
}
