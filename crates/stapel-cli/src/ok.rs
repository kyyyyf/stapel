//! `stapel ok [KEY] <section>` and `stapel ok KEY`: a person confirms one section, or every
//! waiting section of a ticket (STP-2 AC-5, AC-16; STP-17 AC-1 to AC-6).

use crate::repo;
use stapel_core::confirm::{Prepared, Target, change_block, prepare, prepare_waiting, target};
use stapel_core::dialog::ok_facts;
use stapel_core::grant::consume;
use stapel_core::identity::{in_agent_shell, user_name};
use stapel_core::state::{Confirmation, State, save};
use stapel_core::time::now_rfc3339;
use std::path::Path;

fn refuse_agent_shell(form: &str) -> String {
    format!(
        "only a person confirms a section: this shell belongs to an agent (CLAUDECODE or \
         CLAUDE_CODE_ENTRYPOINT is set); ask the agent to run `stapel ok` so that Claude Code \
         shows you the permission dialog, or, from a Zed task started from a Claude Code shell, \
         run `env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT {form}`"
    )
}

/// The git identity; a name with spaces is warned about once per command.
fn identity(root: &Path) -> Result<String, String> {
    let by = user_name(root)?;
    if by.contains(char::is_whitespace) {
        warn!(
            "warning: the name \"{by}\" from git config user.name goes into the tracked state.json"
        );
    }
    Ok(by)
}

/// `stapel ok ARG`: a section id, else a ticket key (STP-17 AC-5).
pub fn run_one(arg: &str, grant: Option<&str>) -> Result<(), String> {
    match repo::open().and_then(|(root, config)| target(&root, &config, arg)) {
        Ok(Target::Key(key)) => run_key(&key, grant),
        Ok(Target::Section(_)) => run(None, arg, grant),
        Err(_) if in_agent_shell() && grant.is_none() => {
            Err(refuse_agent_shell(&format!("stapel ok {arg}")))
        }
        Err(reason) => Err(reason),
    }
}

/// The confirmation a person's `ok` adds for one prepared section.
fn confirmation(p: &Prepared, by: &str, at: &str, via: &str) -> Confirmation {
    Confirmation {
        section: p.section.clone(),
        by: by.to_string(),
        at: at.to_string(),
        hash: p.hash.clone(),
        normal_form: stapel_core::hash::NORMAL_FORM,
        depends_on: p.depends_on.clone(),
        text: Some(p.text.clone()),
        via: Some(via.to_string()),
        extra: Default::default(),
    }
}

/// Drops the stored text of older confirmations of `section` and adds the new one.
fn add(state: &mut State, new: Confirmation) {
    for older in state
        .confirmations
        .iter_mut()
        .filter(|c| c.section == new.section)
    {
        older.text = None;
    }
    state.confirmations.push(new);
}

fn confirmed_line(p: &Prepared) {
    let short = &p.hash[..p.hash.len().min("sha256:".len() + 12)];
    say!("confirmed: {} {} ({short})", p.key, p.section);
}

fn record(p: &Prepared, c: &Confirmation) -> Result<(), String> {
    stapel_core::journal::record_decision(
        &p.dir,
        serde_json::json!({
            "by": c.by,
            "action": "ok",
            "section": c.section,
            "hash": c.hash,
            "via": c.via,
        }),
    )
}

pub fn run(key: Option<&str>, section: &str, grant: Option<&str>) -> Result<(), String> {
    if in_agent_shell() && grant.is_none() {
        return Err(refuse_agent_shell(&format!("stapel ok {section}")));
    }
    let (root, config) = repo::open()?;
    let by = identity(&root)?;
    let mut prepared = prepare(&root, &config, key, section)?;
    if let Some(token) = grant {
        consume(&root, token, &ok_facts(&prepared))?;
    }
    let block = change_block(&config, &prepared);
    let new = confirmation(
        &prepared,
        &by,
        &now_rfc3339(),
        if grant.is_some() { "grant" } else { "terminal" },
    );
    add(&mut prepared.state, new.clone());
    save(&prepared.dir.join("state.json"), &prepared.state)?;
    say_raw!("{block}");
    confirmed_line(&prepared);
    record(&prepared, &new)
}

/// `stapel ok KEY`: every waiting section of the ticket, all or nothing (STP-17 AC-1 to AC-4).
/// Prints `to confirm: KEY <ids>` first, or `nothing to confirm: KEY` when no section waits.
fn run_key(key: &str, grant: Option<&str>) -> Result<(), String> {
    if in_agent_shell() && grant.is_none() {
        return Err(refuse_agent_shell("stapel ok KEY"));
    }
    if grant.is_some() {
        return Err("a grant for `stapel ok KEY` is not supported yet".into());
    }
    let (root, config) = repo::open()?;
    let by = identity(&root)?;
    let waiting = prepare_waiting(&root, &config, key)?;
    let Some(first) = waiting.first() else {
        let folder = stapel_core::tickets::resolve(&root, Some(key))?.key;
        say!("nothing to confirm: {folder}");
        return Ok(());
    };
    say!(
        "to confirm: {} {}",
        first.key,
        waiting
            .iter()
            .map(|p| p.section.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    for p in &waiting {
        say_raw!("{}", change_block(&config, p));
    }
    let at = now_rfc3339();
    let mut state = first.state.clone();
    let new: Vec<Confirmation> = waiting
        .iter()
        .map(|p| confirmation(p, &by, &at, "terminal"))
        .collect();
    for c in &new {
        add(&mut state, c.clone());
    }
    save(&first.dir.join("state.json"), &state)?;
    for p in &waiting {
        confirmed_line(p);
    }
    for (p, c) in waiting.iter().zip(&new) {
        record(p, c)?;
    }
    Ok(())
}
