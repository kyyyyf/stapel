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
    DependencyMissing(String),
    DependencyDuplicated(String),
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
            Reason::DependencyMissing(id) => format!("depends on {id}, which is missing"),
            Reason::DependencyDuplicated(id) => format!("depends on {id}, which is duplicated"),
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
            Err(Reason::Duplicated) => reasons.push(Reason::DependencyDuplicated(dep.clone())),
            Err(_) => reasons.push(Reason::DependencyMissing(dep.clone())),
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

/// The refusal of a close while a section waits for its confirmation (STP-36 AC-5): `None`
/// when every section whose owner is not `generated` is fresh.
pub fn close_refusal(
    config: &Config,
    sections: &[Section],
    state: &State,
    key: &str,
) -> Option<String> {
    let ids: Vec<&str> = config
        .sections
        .iter()
        .filter(|s| s.owner != "generated")
        .filter(|s| !is_fresh(config, sections, state, &s.id))
        .map(|s| s.id.as_str())
        .collect();
    if ids.is_empty() {
        None
    } else {
        Some(format!("{} wait for stapel ok {key}", ids.join(", ")))
    }
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
    /// The Plan waits for its `stapel ok` but only grew by fix steps (STP-36).
    Provisional(String),
    ByHand(String),
}

/// Whether `current` is an appended Plan of the confirmed text `confirmed`: every normal-form line
/// of the confirmed text, each equal and in order, and at least one line more.
pub fn appended_plan(confirmed: &str, current: &str) -> bool {
    let old = normal_form(confirmed);
    let new = normal_form(current);
    let old: Vec<&str> = old.lines().collect();
    let new: Vec<&str> = new.lines().collect();
    new.len() > old.len() && new[..old.len()] == old[..]
}

/// Whether the build goes on without a `stapel ok` of the Plan (STP-36): the newest code review
/// round has a patch finding (`patch_round`), the Plan is the only waiting section and it is an
/// appended Plan.
pub fn provisional(
    config: &Config,
    sections: &[Section],
    state: &State,
    patch_round: bool,
) -> bool {
    level_close(config, sections, state) || fix_steps(config, sections, state, patch_round)
}

/// Whether the level is `close` and the ticket is light (STP-36 AC-6, AC-7): the Plan text of the
/// Plan's last confirmation says `Route: light`, every section was confirmed once and the
/// Description is fresh.
fn level_close(config: &Config, sections: &[Section], state: &State) -> bool {
    if config.process.as_ref().map(|p| p.dialogs.as_str()) != Some("close") {
        return false;
    }
    let Some(plan) = config.section("plan") else {
        return false;
    };
    let Some(confirmed) = latest(state, &plan.id).and_then(|c| c.text.as_deref()) else {
        return false;
    };
    if !crate::next::is_light(confirmed) {
        return false;
    }
    config
        .sections
        .iter()
        .filter(|s| s.owner != "generated")
        .all(|s| {
            latest(state, &s.id).is_some()
                && (s.id != "description" || is_fresh(config, sections, state, &s.id))
        })
}

/// The fix steps of the newest code review round (STP-36 AC-2).
fn fix_steps(config: &Config, sections: &[Section], state: &State, patch_round: bool) -> bool {
    if !patch_round {
        return false;
    }
    let Some(plan) = config.section("plan") else {
        return false;
    };
    let waiting: Vec<&SectionDef> = config
        .sections
        .iter()
        .filter(|s| s.owner != "generated")
        .filter(|s| !is_fresh(config, sections, state, &s.id))
        .collect();
    if waiting.len() != 1 || waiting[0].id != plan.id {
        return false;
    }
    let Some(c) = latest(state, &plan.id) else {
        return false;
    };
    let (Some(confirmed), Lookup::Found(current)) = (c.text.as_deref(), find(sections, &plan.title))
    else {
        return false;
    };
    freshness(config, sections, c) == [Reason::Changed] && appended_plan(confirmed, current)
}

/// Whether the newest code review round of the ticket in `dir` has a patch finding.
fn dir_patch_round(dir: &Path) -> bool {
    use crate::journal::Entry;
    let Ok(entries) = crate::journal::read(&dir.join("findings.jsonl")) else {
        return false;
    };
    let records: Vec<_> = entries
        .into_iter()
        .filter_map(|(_, e)| match e {
            Entry::V1(m) | Entry::Legacy(m) => Some(m),
            Entry::Problem(_) => None,
        })
        .collect();
    crate::review::patch_round(&records)
}

/// Reads a regular file of at most 4 MiB, or nothing.
pub fn read_bounded(path: &Path) -> Option<String> {
    crate::state::read_bounded(path).ok()
}

/// The tickets that can give the permit: the branch ticket alone on a ticket branch, else every
/// ticket (STP-27 AC-9).
fn permit_candidates(root: &Path) -> Vec<crate::tickets::Ticket> {
    match crate::tickets::branch_ticket(root) {
        Some(ticket) => vec![ticket],
        None => list(root),
    }
}

/// Per open ticket, which required sections are not confirmed or are stale, for the guard's
/// denial (STP-2 AC-11).
pub fn permit_report(root: &Path, config: &Config) -> String {
    let mut parts = Vec::new();
    for ticket in permit_candidates(root) {
        let Status::Open(state) = &ticket.status else {
            continue;
        };
        let Some(text) = read_bounded(&ticket.dir.join("ticket.md")) else {
            parts.push(format!("{}: ticket.md cannot be read", ticket.key));
            continue;
        };
        let sections = parse(&text);
        let missing: Vec<String> = config
            .build
            .requires
            .iter()
            .filter_map(|id| match latest(state, id) {
                None => Some(format!("{id} not confirmed")),
                Some(c) if !freshness(config, &sections, c).is_empty() => {
                    Some(format!("{id} stale"))
                }
                Some(_) => None,
            })
            .collect();
        parts.push(format!("{}: {}", ticket.key, missing.join(", ")));
    }
    if parts.is_empty() {
        "there is no open ticket".into()
    } else {
        parts.join("; ")
    }
}

/// The repository-wide permit: the first open ticket whose required sections are fresh, else the
/// first ticket without a `closed` fact that carries the phase-0 hand flag.
pub fn build_permit(root: &Path, config: Option<&Config>) -> Option<Permit> {
    let dir = crate::tickets::tickets_dir(root);
    let mut provisional_key = None;
    if let Some(config) = config {
        for ticket in permit_candidates(root) {
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
            let sections = parse(&text);
            if ticket_permits(config, &sections, state) {
                return Some(Permit::Computed(ticket.key));
            }
            if provisional_key.is_none()
                && provisional(config, &sections, state, dir_patch_round(&ticket.dir))
            {
                provisional_key = Some(ticket.key);
            }
        }
    }
    if let Some(key) = provisional_key {
        return Some(Permit::Provisional(key));
    }
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .collect();
    names.sort();
    let branch = crate::tickets::branch_ticket(root).map(|t| t.key);
    names.into_iter().find_map(|(key, path)| {
        if branch.as_ref().is_some_and(|b| *b != key) {
            return None;
        }
        let text = read_bounded(&path.join("state.json"))?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        (v["build"]["allowed"] == true && v.get("closed").is_none()).then_some(Permit::ByHand(key))
    })
}
