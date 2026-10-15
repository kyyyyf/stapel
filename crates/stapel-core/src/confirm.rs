//! What a confirmation of one section, or of every waiting section of a ticket, would record,
//! checked the same way for `ok` and for the guard that asks the person about it.

use crate::config::Config;
use crate::hash::{normal_form, section_hash};
use crate::stage::{Reason, diff, freshness, latest};
use crate::state::State;
use crate::ticket::{Lookup, find, parse, read};
use crate::tickets::{Status, Ticket, list, resolve};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Prepared {
    pub key: String,
    pub dir: PathBuf,
    pub state: State,
    pub section: String,
    pub hash: String,
    pub text: String,
    pub depends_on: BTreeMap<String, String>,
}

/// Resolves the ticket and section and computes the hashes; every refusal names its reason.
pub fn prepare(
    root: &Path,
    config: &Config,
    key: Option<&str>,
    section: &str,
) -> Result<Prepared, String> {
    let ticket = resolve(root, key)?;
    let (ticket_key, dir, state) = open_ticket(ticket)?;
    let confirmable = confirmable_ids(config);
    let Some(def) = config.section(section).filter(|s| s.owner != "generated") else {
        return Err(format!(
            "\"{section}\" cannot be confirmed; sections that can: {}",
            confirmable.join(", ")
        ));
    };

    let text = read(&dir.join("ticket.md"))?;
    let sections = parse(&text);
    let body_of = |id: &str| -> Result<String, String> {
        let title = &config.section(id).expect("validated config").title;
        match find(&sections, title) {
            Lookup::Found(body) => Ok(body.to_string()),
            Lookup::Missing => Err(format!("section {id} (## {title}) is missing in ticket.md")),
            Lookup::Duplicated => Err(format!("section {id} (## {title}) appears more than once")),
        }
    };
    let body = body_of(&def.id)?;
    let mut depends_on = BTreeMap::new();
    for dep in &def.depends_on {
        let dep_body = body_of(dep).map_err(|e| format!("{} depends on {dep}: {e}", def.id))?;
        depends_on.insert(dep.clone(), section_hash(&dep_body));
    }
    Ok(Prepared {
        key: ticket_key,
        dir,
        state,
        section: def.id.clone(),
        hash: section_hash(&body),
        text: normal_form(&body),
        depends_on,
    })
}

/// The key, folder and state of an open ticket; every other status is a refusal.
fn open_ticket(ticket: Ticket) -> Result<(String, PathBuf, State), String> {
    match ticket.status {
        Status::Open(state) => Ok((ticket.key, ticket.dir, state)),
        Status::Closed(state) => {
            let c = state.closed.expect("closed");
            Err(format!(
                "ticket {} was closed by {} at {}: {}; reopening comes in phase 1",
                ticket.key, c.by, c.at, c.reason
            ))
        }
        Status::Legacy(_) => Err(format!(
            "ticket {} has a legacy (STP-1) state.json without schema_version",
            ticket.key
        )),
        Status::Unreadable(reason) => Err(reason),
    }
}

fn confirmable_ids(config: &Config) -> Vec<&str> {
    config
        .sections
        .iter()
        .filter(|s| s.owner != "generated")
        .map(|s| s.id.as_str())
        .collect()
}

/// What the one argument of `stapel ok` names (STP-17 AC-5).
#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    /// A section id of `stapel.toml`, as typed; `prepare` refuses a generated one.
    Section(String),
    /// A ticket folder, as typed; `resolve` refuses several folders that differ only in case.
    Key(String),
}

/// A section id wins over a ticket folder; neither is a refusal that lists both.
pub fn target(root: &Path, config: &Config, arg: &str) -> Result<Target, String> {
    if config.section(arg).is_some() {
        return Ok(Target::Section(arg.to_string()));
    }
    let tickets = list(root);
    if tickets.iter().any(|t| t.key.eq_ignore_ascii_case(arg)) {
        return Ok(Target::Key(arg.to_string()));
    }
    let open: Vec<&str> = tickets
        .iter()
        .filter(|t| matches!(t.status, Status::Open(_)))
        .map(|t| t.key.as_str())
        .collect();
    Err(format!(
        "\"{arg}\" is neither a section nor a ticket; sections: {}; open tickets: {}",
        confirmable_ids(config).join(", "),
        if open.is_empty() {
            "none".to_string()
        } else {
            open.join(", ")
        }
    ))
}

/// Every confirmable section of the ticket without a fresh confirmation, in `stapel.toml`
/// order, each prepared as `prepare` does; the first refusal wins (STP-17 AC-1, AC-3).
pub fn prepare_waiting(root: &Path, config: &Config, key: &str) -> Result<Vec<Prepared>, String> {
    let (key, dir, state) = open_ticket(resolve(root, Some(key))?)?;
    let sections = parse(&read(&dir.join("ticket.md"))?);
    let mut waiting = Vec::new();
    for id in confirmable_ids(config) {
        let fresh = latest(&state, id).is_some_and(|c| freshness(config, &sections, c).is_empty());
        if !fresh {
            waiting.push(prepare(root, config, Some(&key), id)?);
        }
    }
    Ok(waiting)
}

fn new_line(p: &Prepared) -> String {
    let n = p.text.lines().count();
    format!("new, {n} line{}", if n == 1 { "" } else { "s" })
}

/// What the person sees about a section before its `confirmed:` line (STP-17 AC-2); call it
/// before the confirmation is added to `p.state`.
pub fn change_block(config: &Config, p: &Prepared) -> String {
    let heading = |rest: String| format!("== {}: {rest}\n", p.section);
    let Some(old) = latest(&p.state, &p.section) else {
        return heading(new_line(p));
    };
    let sections = read(&p.dir.join("ticket.md"))
        .map(|t| parse(&t))
        .unwrap_or_default();
    let others: Vec<String> = freshness(config, &sections, old)
        .iter()
        .filter(|r| **r != Reason::Changed)
        .map(|r| r.describe(old))
        .collect();
    let others = others.join(", ");
    let Some(old_text) = old.text.as_deref() else {
        return heading(if others.is_empty() {
            new_line(p)
        } else {
            format!("{}; {others}", new_line(p))
        });
    };
    if old.hash == p.hash {
        return if others.is_empty() {
            heading("unchanged".into())
        } else {
            heading(format!("unchanged; {others}"))
        };
    }
    let mut block = if others.is_empty() {
        heading("changed since the last confirmation".into())
    } else {
        heading(format!("changed since the last confirmation; {others}"))
    };
    block.push_str(&diff(old_text, &p.text));
    block
}
