//! `stapel tokens import-records FILE [KEY]`: token records appended to a ticket's `tokens.jsonl`
//! (STP-19 AC-1 to AC-5, AC-10).

use crate::fail::{AGENT_SHELL_TOKENS, Fail, MISMATCH};
use crate::repo;
use stapel_core::identity::in_agent_shell;
use stapel_core::journal::append_all;
use stapel_core::records::plan_tokens;
use stapel_core::time::now_rfc3339;
use std::path::Path;

pub fn import(file: &str, key: Option<&str>, grant: Option<&str>) -> Result<(), Fail> {
    if in_agent_shell() && grant.is_none() {
        return Err(Fail::failed(AGENT_SHELL_TOKENS));
    }
    // The dialog and its grant come with STP-19 step 3; until then no grant matches.
    if grant.is_some() {
        return Err(Fail::failed(MISMATCH));
    }
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    let plan = plan_tokens(&root, Path::new(""), file, key).map_err(Fail::refused)?;
    // One `at` for every record of the run.
    let lines = plan.lines(&now_rfc3339());
    append_all(&plan.target, &lines).map_err(Fail::refused)?;
    say!("imported: {} token records into {}", lines.len(), plan.key);
    Ok(())
}
