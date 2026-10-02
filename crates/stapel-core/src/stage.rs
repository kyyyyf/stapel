//! The stage of a ticket, computed from facts (STP-2 AC-7, AC-10, AC-11).

use crate::config::{Config, Section as SectionDef};
use crate::hash::{NORMAL_FORM, normal_form, section_hash};
use crate::state::{Confirmation, State};
use crate::ticket::{Lookup, Section, find, parse};
use crate::tickets::{Status, list};
use std::path::Path;

/// Why a latest confirmation is stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    Changed,
    DependencyChanged(String),
    DependencyRemoved(String),
    Missing,
    Duplicated,
    NormalForm(u32),
}

impl Reason {
    pub fn describe(&self, c: &Confirmation) -> String {
        match self {
            Reason::Changed => format!("changed since confirmed by {} at {}", c.by, c.at),
            Reason::DependencyChanged(id) => format!("depends on {id}, which changed"),
            Reason::DependencyRemoved(id) => {
                format!("depends on {id}, which is no longer configured")
            }
            Reason::Missing => "section missing".into(),
            Reason::Duplicated => "section duplicated".into(),
            Reason::NormalForm(n) => format!("confirmed under normal form {n}"),
        }
    }
}

/// The current hash of section `id`, or why there is none.
fn current_hash(config: &Config, sections: &[Section], id: &str) -> Result<String, Reason> {
    let def = config
        .section(id)
        .ok_or_else(|| Reason::DependencyRemoved(id.to_string()))?;
    match find(sections, &def.title) {
        Lookup::Found(body) => Ok(section_hash(body)),
        Lookup::Missing => Err(Reason::Missing),
        Lookup::Duplicated => Err(Reason::Duplicated),
    }
}

/// The last confirmation of each section, in array order.
pub fn latest<'a>(state: &'a State, id: &str) -> Option<&'a Confirmation> {
    state
        .confirmations
        .iter()
        .rev()
        .find(|c| c.section.eq_ignore_ascii_case(id))
}

/// Empty when fresh; one reason per cause otherwise.
pub fn freshness(config: &Config, sections: &[Section], c: &Confirmation) -> Vec<Reason> {
    let mut reasons = Vec::new();
    if c.normal_form != NORMAL_FORM {
        reasons.push(Reason::NormalForm(c.normal_form));
    }
    match current_hash(config, sections, &c.section) {
        Ok(h) if h != c.hash => reasons.push(Reason::Changed),
        Ok(_) => {}
        Err(Reason::DependencyRemoved(_)) => reasons.push(Reason::Missing),
        Err(r) => reasons.push(r),
    }
    for (dep, recorded) in &c.depends_on {
        match current_hash(config, sections, dep) {
            Ok(h) if &h != recorded => reasons.push(Reason::DependencyChanged(dep.clone())),
            Ok(_) => {}
            Err(Reason::DependencyRemoved(id)) => reasons.push(Reason::DependencyRemoved(id)),
            Err(_) => reasons.push(Reason::DependencyChanged(dep.clone())),
        }
    }
    reasons
}

pub fn is_fresh(config: &Config, sections: &[Section], state: &State, id: &str) -> bool {
    latest(state, id).is_some_and(|c| freshness(config, sections, c).is_empty())
}

/// The first confirmable section, in config order, without a fresh confirmation.
pub fn waiting_for<'a>(
    config: &'a Config,
    sections: &[Section],
    state: &State,
) -> Option<&'a SectionDef> {
    config
        .sections
        .iter()
        .filter(|s| s.owner != "generated")
        .find(|s| !is_fresh(config, sections, state, &s.id))
}

/// Whether this open ticket's required sections are all freshly confirmed.
pub fn ticket_permits(config: &Config, sections: &[Section], state: &State) -> bool {
    config
        .build
        .requires
        .iter()
        .all(|id| is_fresh(config, sections, state, id))
}

/// A unified diff from the confirmed text to the current one.
pub fn diff(confirmed: &str, current: &str) -> String {
    similar::TextDiff::from_lines(
        &format!("{confirmed}\n"),
        &format!("{}\n", normal_form(current)),
    )
    .unified_diff()
    .context_radius(3)
    .header("confirmed", "current")
    .to_string()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Permit {
    Computed(String),
    ByHand(String),
}

/// Files above this size give no permit (STP-2 AC-11).
pub const MAX_FILE: u64 = 4 * 1024 * 1024;

/// Reads a regular file of at most `MAX_FILE` bytes.
pub fn read_bounded(path: &Path) -> Option<String> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() || meta.len() > MAX_FILE {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// The repository-wide permit: the first open ticket whose required sections are fresh, else the
/// first ticket without a `closed` fact that carries the phase-0 hand flag.
pub fn build_permit(root: &Path, config: Option<&Config>) -> Option<Permit> {
    let dir = crate::tickets::tickets_dir(root);
    if let Some(config) = config {
        for ticket in list(root) {
            let Status::Open(state) = &ticket.status else {
                continue;
            };
            if read_bounded(&ticket.dir.join("state.json")).is_none() {
                continue;
            }
            let all_confirmed = config
                .build
                .requires
                .iter()
                .all(|id| latest(state, id).is_some());
            if !all_confirmed {
                continue;
            }
            let Some(text) = read_bounded(&ticket.dir.join("ticket.md")) else {
                continue;
            };
            if ticket_permits(config, &parse(&text), state) {
                return Some(Permit::Computed(ticket.key));
            }
        }
    }
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .collect();
    names.sort();
    names.into_iter().find_map(|(key, path)| {
        let text = read_bounded(&path.join("state.json"))?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        (v["build"]["allowed"] == true && v.get("closed").is_none()).then_some(Permit::ByHand(key))
    })
}
