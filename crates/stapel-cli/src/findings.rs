//! `stapel findings import FILE [KEY]`: review findings appended to a ticket's `findings.jsonl`
//! (STP-18 AC-1 to AC-4, AC-11, AC-12).

use crate::fail::{AGENT_SHELL, Fail, MISMATCH};
use crate::repo;
use stapel_core::dialog::findings_facts;
use stapel_core::grant::consume_with;
use stapel_core::identity::in_agent_shell;
use stapel_core::journal::append_all;
use stapel_core::records::plan_findings;
use std::path::Path;

pub fn import(file: &str, key: Option<&str>, grant: Option<&str>) -> Result<(), Fail> {
    if in_agent_shell() && grant.is_none() {
        return Err(Fail::failed(AGENT_SHELL));
    }
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    // Every check runs before the grant is used, so a refusal leaves the grant to expire.
    let plan = plan_findings(&root, Path::new(""), file, key).map_err(Fail::refused)?;
    if let Some(token) = grant {
        consume_with(&root, token, &findings_facts(&plan), MISMATCH).map_err(Fail::failed)?;
    }
    append_all(&plan.target, &plan.findings.lines).map_err(Fail::refused)?;
    say!(
        "imported: {} findings into {}",
        plan.findings.lines.len(),
        plan.key
    );
    Ok(())
}
