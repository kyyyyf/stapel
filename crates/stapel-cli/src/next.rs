//! `stapel next [KEY]`: the next action of a ticket, from the facts on disk (STP-20). Read-only.

use crate::fail::Fail;
use crate::repo;
use stapel_core::journal::{self, Entry};
use stapel_core::next::{Facts, decide};
use stapel_core::ticket::{parse, read};
use stapel_core::tickets::{Status, list, resolve};

pub fn run(key: Option<&str>) -> Result<(), Fail> {
    let (root, config) = repo::open().map_err(Fail::failed)?;
    if key.is_none()
        && !list(&root)
            .iter()
            .any(|t| matches!(t.status, Status::Open(_)))
    {
        say!("next: new-ticket — stapel new \"<title>\"");
        return Ok(());
    }
    let ticket = resolve(&root, key).map_err(Fail::refused)?;
    let state = match &ticket.status {
        Status::Open(state) | Status::Closed(state) => state,
        Status::Legacy(_) => {
            return Err(Fail::failed(format!(
                "ticket {} is in the STP-1 format; it has no stage to compute",
                ticket.key
            )));
        }
        Status::Unreadable(reason) => return Err(Fail::failed(reason.clone())),
    };
    let text = read(&ticket.dir.join("ticket.md")).map_err(Fail::failed)?;
    let sections = parse(&text);
    let finding_stages: Vec<String> = journal::read(&ticket.dir.join("findings.jsonl"))
        .map_err(Fail::failed)?
        .into_iter()
        .filter_map(|(_, entry)| match entry {
            Entry::V1(map) | Entry::Legacy(map) => map.get("stage")?.as_str().map(String::from),
            Entry::Problem(_) => None,
        })
        .collect();

    let decision = decide(&Facts {
        key: &ticket.key,
        config: &config,
        state,
        sections: &sections,
        finding_stages: &finding_stages,
    });
    say!("ticket: {} — {}", ticket.key, state.title);
    for fact in &decision.facts {
        say!("{fact}");
    }
    say!("{}", decision.action.line());
    Ok(())
}
