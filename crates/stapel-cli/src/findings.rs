//! `stapel findings import FILE [KEY]`: review findings appended to a ticket's `findings.jsonl`
//! (STP-18 AC-1 to AC-4, AC-11, AC-12; STP-23 AC-1: no dialog, the importer is recorded).

use crate::fail::Fail;
use crate::repo;
use stapel_core::identity::{in_agent_shell, user_name};
use stapel_core::journal::append_all;
use stapel_core::records::{importer_json, plan_findings, with_importer};
use stapel_core::triage::plan_triage;
use std::path::Path;

/// The dependency stops of the ticket after its records changed (STP-27 AC-22). A stop that cannot
/// be written is reported; the records stay.
fn sync_stops(root: &Path, key: &str) {
    let Ok(ticket) = stapel_core::tickets::resolve(root, Some(key)) else {
        return;
    };
    if let Err(e) = stapel_core::depends::sync_stops(root, &ticket) {
        warn!("stapel: dependency stop not updated: {e}");
    }
}

/// The generated block of the Review section after the records changed (STP-38 AC-1). A block
/// that cannot be written is reported; the records stay.
fn write_block(plan_target: &Path) {
    let Some(dir) = plan_target.parent() else {
        return;
    };
    if let Err(e) = stapel_core::review_block::write(dir) {
        warn!("stapel: review block not updated: {e}");
    }
}

/// The refusal of a ticket folder whose `ticket.md` cannot take the generated block, before the
/// command writes anything (STP-38 AC-5); with no Review section it prints the notice and goes on.
pub fn guard_block(dir: &Path) -> Result<(), Fail> {
    match stapel_core::review_block::guard(dir) {
        Ok(true) => Ok(()),
        Ok(false) => {
            warn!(
                "notice: {} has no Review section; no generated block",
                dir.join("ticket.md").display()
            );
            Ok(())
        }
        Err(e) => Err(Fail::refused(e)),
    }
}

pub fn import(file: &str, key: Option<&str>) -> Result<(), Fail> {
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    let user = user_name(&root).map_err(Fail::failed)?;
    let importer = importer_json(in_agent_shell(), &user);
    let plan = plan_findings(&root, Path::new(""), file, key, &importer).map_err(Fail::refused)?;
    guard_block(plan.target.parent().unwrap_or(Path::new("")))?;
    append_all(&plan.target, &plan.findings.lines).map_err(Fail::refused)?;
    write_block(&plan.target);
    sync_stops(&root, &plan.key);
    say!(
        "imported: {} findings into {}",
        plan.findings.lines.len(),
        plan.key
    );
    Ok(())
}

/// `stapel answer KEY ID FATE NOTE` (STP-40 AC-6): the person's triage record of one finding.
pub fn answer(key: &str, id: &str, fate: &str, note: &str) -> Result<(), Fail> {
    if in_agent_shell() {
        return Err(Fail::failed(
            "only a person answers a finding: this shell belongs to an agent (CLAUDECODE or \
             CLAUDE_CODE_ENTRYPOINT is set)",
        ));
    }
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    stapel_core::tickets::match_branch(&root, Some(key)).map_err(Fail::refused)?;
    if stapel_core::tickets::branch_ticket(&root).is_none() {
        return Err(Fail::refused(format!(
            "stapel answer runs in the worktree of {key}; this checkout's branch names no ticket"
        )));
    }
    let ticket = stapel_core::records::journal_ticket(&root, Some(key)).map_err(Fail::refused)?;
    if matches!(ticket.status, stapel_core::tickets::Status::Closed(_)) {
        return Err(Fail::refused(format!("{} is closed", ticket.key)));
    }
    let user = user_name(&root).map_err(Fail::refused)?;
    let importer = importer_json(false, &user);
    if !["patch", "bad_plan", "defer", "no-change"].contains(&fate) {
        return Err(Fail::refused(
            "the fate must be one of patch, bad_plan, defer, no-change",
        ));
    }
    if note.trim().is_empty() {
        return Err(Fail::refused("the note is empty"));
    }
    let target = ticket.dir.join("findings.jsonl");
    let label = format!(".stapel/tickets/{}/findings.jsonl", ticket.key);
    let existing = stapel_core::journal::read(&target).map_err(Fail::refused)?;
    for (n, entry) in &existing {
        if let stapel_core::journal::Entry::Problem(reason) = entry
            && !stapel_core::journal::is_unknown_version(reason)
        {
            return Err(Fail::refused(format!("{label}:{n}: {reason}")));
        }
    }
    let records: Vec<_> = existing
        .iter()
        .filter_map(|(_, e)| match e {
            stapel_core::journal::Entry::V1(m) | stapel_core::journal::Entry::Legacy(m) => {
                Some(m.clone())
            }
            stapel_core::journal::Entry::Problem(_) => None,
        })
        .collect();
    let state = stapel_core::triage::state_of(&records, id);
    if state.finding.is_none() {
        return Err(Fail::refused(format!(
            "{id} is not a finding of {}",
            ticket.key
        )));
    }
    if !state.is_persons() {
        return Err(Fail::refused(format!(
            "{id} of {} does not wait for a person",
            ticket.key
        )));
    }
    let text = |s: &str| serde_json::to_string(s).expect("a string is JSON");
    let body = format!(
        "{{\"kind\":\"triage\",\"ticket\":{},\"id\":{},\"fate\":{},\"note\":{}}}",
        text(&ticket.key),
        text(id),
        text(fate),
        text(note)
    );
    let line = with_importer(&body, &importer).expect("the body ends in a brace");
    if line.len() + 1 > stapel_core::journal::MAX_LINE {
        return Err(Fail::refused(
            "the record is longer than the journal line limit",
        ));
    }
    guard_block(&ticket.dir)?;
    append_all(&target, &[line]).map_err(Fail::refused)?;
    write_block(&target);
    sync_stops(&root, &ticket.key);
    say!("answered: {id} of {}: {fate}", ticket.key);
    Ok(())
}

/// `stapel findings triage FILE [KEY]` (STP-23 AC-5 to AC-7).
pub fn triage(file: &str, key: Option<&str>) -> Result<(), Fail> {
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    let user = user_name(&root).map_err(Fail::failed)?;
    let importer = importer_json(in_agent_shell(), &user);
    let plan = plan_triage(&root, Path::new(""), file, key, &importer).map_err(Fail::refused)?;
    guard_block(plan.target.parent().unwrap_or(Path::new("")))?;
    append_all(&plan.target, &plan.findings.lines).map_err(Fail::refused)?;
    write_block(&plan.target);
    sync_stops(&root, &plan.key);
    say!(
        "triaged: {} findings of {}",
        plan.findings.lines.len(),
        plan.key
    );
    Ok(())
}
