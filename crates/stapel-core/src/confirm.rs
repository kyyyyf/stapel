//! What a confirmation of one section would record, checked the same way for `ok` and for the
//! guard that asks the person about it.

use crate::config::Config;
use crate::hash::{normal_form, section_hash};
use crate::state::State;
use crate::ticket::{Lookup, find, parse, read};
use crate::tickets::{Status, resolve};
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
    let state = match ticket.status {
        Status::Open(state) => state,
        Status::Closed(state) => {
            let c = state.closed.expect("closed");
            return Err(format!(
                "ticket {} was closed by {} at {}: {}; reopening comes in phase 1",
                ticket.key, c.by, c.at, c.reason
            ));
        }
        Status::Legacy(_) => {
            return Err(format!(
                "ticket {} has a legacy (STP-1) state.json without schema_version",
                ticket.key
            ));
        }
        Status::Unreadable(reason) => return Err(reason),
    };
    let confirmable: Vec<&str> = config
        .sections
        .iter()
        .filter(|s| s.owner != "generated")
        .map(|s| s.id.as_str())
        .collect();
    let Some(def) = config.section(section).filter(|s| s.owner != "generated") else {
        return Err(format!(
            "\"{section}\" cannot be confirmed; sections that can: {}",
            confirmable.join(", ")
        ));
    };

    let text = read(&ticket.dir.join("ticket.md"))?;
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
        key: ticket.key,
        dir: ticket.dir,
        state,
        section: def.id.clone(),
        hash: section_hash(&body),
        text: normal_form(&body),
        depends_on,
    })
}
