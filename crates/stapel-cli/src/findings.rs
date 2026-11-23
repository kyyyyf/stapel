//! `stapel findings import FILE [KEY]`: review findings appended to a ticket's `findings.jsonl`
//! (STP-18 AC-1 to AC-4, AC-11, AC-12; STP-23 AC-1: no dialog, the importer is recorded).

use crate::fail::Fail;
use crate::repo;
use stapel_core::identity::{in_agent_shell, user_name};
use stapel_core::journal::append_all;
use stapel_core::records::{importer_json, plan_findings};
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
