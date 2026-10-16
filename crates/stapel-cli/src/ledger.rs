//! `stapel ledger add --kind KIND --note TEXT [KEY]`: one record appended to the process ledger
//! (STP-18 AC-5, AC-6).

use crate::fail::Fail;
use crate::{repo, tokens};
use stapel_core::journal::append_all;
use stapel_core::records::{LEDGER, check_ledger};

pub fn add(kind: &str, note: &str, key: Option<&str>) -> Result<(), Fail> {
    let (root, _config) = repo::open().map_err(|m| Fail {
        code: 1,
        message: m,
    })?;
    let ticket = tokens::journal_ticket(&root, key).map_err(Fail::refused)?;
    let record =
        check_ledger(&root.join(LEDGER), LEDGER, kind, &ticket.key, note).map_err(Fail::refused)?;
    append_all(&root.join(LEDGER), &[record.line]).map_err(Fail::refused)?;
    say!("recorded: {kind} for {}", ticket.key);
    Ok(())
}
