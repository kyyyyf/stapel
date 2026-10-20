//! `stapel brief KEY STEP`: the brief of one Plan step (STP-21). Writes nothing.

use crate::fail::Fail;
use crate::repo;
use stapel_core::brief::assemble;
use stapel_core::ticket::read;
use stapel_core::tickets::{Status, resolve};

/// `STEP` is digits followed by at most one lowercase ASCII letter (`2`, `2c`).
fn step_is_well_formed(step: &str) -> bool {
    let digits = step.trim_end_matches(|c: char| c.is_ascii_lowercase());
    let letters = step.len() - digits.len();
    letters <= 1 && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

pub fn run(key: &str, step: &str) -> Result<(), Fail> {
    if !step_is_well_formed(step) {
        return Err(Fail::refused(format!(
            "STEP must be digits followed by at most one lowercase letter, e.g. 2 or 2c: {step}"
        )));
    }
    let (root, config) = repo::open().map_err(Fail::failed)?;
    let ticket = resolve(&root, Some(key)).map_err(Fail::refused)?;
    let state = match &ticket.status {
        Status::Open(state) | Status::Closed(state) => state,
        Status::Legacy(_) => {
            return Err(Fail::failed(format!(
                "ticket {} is in the STP-1 format; it has no brief",
                ticket.key
            )));
        }
        Status::Unreadable(reason) => return Err(Fail::failed(reason.clone())),
    };
    let text = read(&ticket.dir.join("ticket.md")).map_err(Fail::failed)?;
    let plan_title = match config.section("plan") {
        Some(def) => def.title.clone(),
        None => {
            return Err(Fail::failed(format!(
                "the Plan of {} has no step {step}",
                ticket.key
            )));
        }
    };
    let brief = assemble(&text, &ticket.key, step, state.base.as_deref(), &plan_title)
        .map_err(Fail::failed)?;
    say_raw!("{brief}");
    Ok(())
}
