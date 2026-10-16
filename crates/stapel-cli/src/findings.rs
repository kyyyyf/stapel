//! `stapel findings import FILE [KEY]`: review findings appended to a ticket's `findings.jsonl`
//! (STP-18 AC-1 to AC-4).

use crate::fail::Fail;
use crate::{repo, tokens};
use stapel_core::journal::append_all;
use stapel_core::records::{check_findings, read_input};

pub fn import(file: &str, key: Option<&str>) -> Result<(), Fail> {
    let (root, _config) = repo::open().map_err(|m| Fail {
        code: 1,
        message: m,
    })?;
    let ticket = tokens::journal_ticket(&root, key).map_err(Fail::refused)?;
    let text = read_input(&root, file).map_err(Fail::refused)?;
    let target = ticket.dir.join("findings.jsonl");
    let label = format!(".stapel/tickets/{}/findings.jsonl", ticket.key);
    let findings =
        check_findings(file, &text, &ticket.key, &target, &label).map_err(Fail::refused)?;
    append_all(&target, &findings.lines).map_err(Fail::refused)?;
    say!(
        "imported: {} findings into {}",
        findings.lines.len(),
        ticket.key
    );
    Ok(())
}
