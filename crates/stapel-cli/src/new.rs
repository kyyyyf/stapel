//! `stapel new`: a ticket folder with `ticket.md` and `state.json` (STP-2 AC-1..AC-3).

use crate::repo;
use stapel_core::state::{State, save};
use stapel_core::tickets::{next_key, tickets_dir};

pub fn run(title: &str, tracker: Option<&str>) -> Result<(), String> {
    let (root, config) = repo::open()?;
    let title = title.trim();
    if title.is_empty() || title.chars().any(char::is_control) {
        return Err("the title must be non-empty and contain no control characters".into());
    }
    if let Some(url) = tracker {
        let scheme_ok = ["http://", "https://"]
            .iter()
            .any(|s| url.len() > s.len() && url.starts_with(s));
        if !scheme_ok || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(format!(
                "the tracker URL \"{}\" must start with http:// or https:// and contain no spaces",
                url.escape_debug()
            ));
        }
    }

    let base = read_base(&root)?;
    let key = next_key(&config, &root);
    let dir = tickets_dir(&root).join(&key);
    if std::fs::symlink_metadata(&dir).is_ok() {
        return Err(format!("ticket {key} already exists at {}", dir.display()));
    }
    std::fs::create_dir_all(dir.parent().expect("tickets dir"))
        .and_then(|()| std::fs::create_dir(&dir))
        .map_err(|e| format!("{}: {e}", dir.display()))?;

    let mut text = format!("# {key} — {title}\n\n## Description\n\n");
    if let Some(url) = tracker {
        text.push_str(&format!("Tracker: {url}\n\n"));
    }
    text.push_str("TODO\n");
    for section in &config.sections {
        text.push_str(&format!("\n## {}\n\nTODO\n", section.title));
    }
    std::fs::write(dir.join("ticket.md"), text).map_err(|e| format!("ticket.md: {e}"))?;

    let mut state = State::new(&key, title);
    state.tracker = tracker.map(String::from);
    state.base = base;
    save(&dir.join("state.json"), &state)?;
    println!("created: {key}");
    Ok(())
}

/// The full id of HEAD. A repository without commits (an unborn HEAD) has no base; any other git
/// error is returned.
fn read_base(root: &std::path::Path) -> Result<Option<String>, String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "-q", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        return Ok(Some(
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        ));
    }
    // With `-q`, git exits 1 without a message when HEAD names no commit yet.
    if out.status.code() == Some(1) && out.stderr.is_empty() {
        return Ok(None);
    }
    Err(format!(
        "git rev-parse --verify HEAD: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    ))
}
