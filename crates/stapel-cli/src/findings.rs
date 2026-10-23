//! `stapel findings import FILE [KEY]`: review findings appended to a ticket's `findings.jsonl`
//! (STP-18 AC-1 to AC-4, AC-11, AC-12; STP-23 AC-1: no dialog, the importer is recorded).

use crate::fail::Fail;
use crate::repo;
use stapel_core::identity::{in_agent_shell, user_name};
use stapel_core::journal::append_all;
use stapel_core::records::{importer_json, plan_findings};
use stapel_core::triage::plan_triage;
use std::path::Path;

pub fn import(file: &str, key: Option<&str>) -> Result<(), Fail> {
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    let user = user_name(&root).map_err(Fail::failed)?;
    let importer = importer_json(in_agent_shell(), &user);
    let plan = plan_findings(&root, Path::new(""), file, key, &importer).map_err(Fail::refused)?;
    append_all(&plan.target, &plan.findings.lines).map_err(Fail::refused)?;
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
    append_all(&plan.target, &plan.findings.lines).map_err(Fail::refused)?;
    say!(
        "triaged: {} findings of {}",
        plan.findings.lines.len(),
        plan.key
    );
    Ok(())
}
