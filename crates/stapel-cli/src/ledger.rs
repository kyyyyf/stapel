//! `stapel ledger add --kind KIND --note TEXT [KEY]`: one record appended to the process ledger
//! (STP-18 AC-5, AC-6, AC-11, AC-12).

use crate::fail::{AGENT_SHELL, Fail, MISMATCH};
use crate::repo;
use stapel_core::dialog::ledger_facts;
use stapel_core::grant::consume_with;
use stapel_core::identity::in_agent_shell;
use stapel_core::journal::append_all;
use stapel_core::records::{LEDGER, plan_ledger};

pub fn add(kind: &str, note: &str, key: Option<&str>, grant: Option<&str>) -> Result<(), Fail> {
    if in_agent_shell() && grant.is_none() {
        return Err(Fail::failed(AGENT_SHELL));
    }
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    // Every check runs before the grant is used, so a refusal leaves the grant to expire.
    let plan = plan_ledger(&root, kind, note, key).map_err(Fail::refused)?;
    if let Some(token) = grant {
        consume_with(&root, token, &ledger_facts(&plan), MISMATCH).map_err(Fail::failed)?;
    }
    append_all(&root.join(LEDGER), &[plan.record.line]).map_err(Fail::refused)?;
    say!("recorded: {kind} for {}", plan.key);
    Ok(())
}
