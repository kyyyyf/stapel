//! `stapel status [KEY]`: whose decision is needed and what went stale (STP-2 AC-10). Read-only.

use crate::repo;
use stapel_core::stage::{Permit, build_permit, diff, freshness, latest, waiting_for};
use stapel_core::ticket::{parse, read};
use stapel_core::tickets::{Status, resolve};
use std::process::ExitCode;

pub fn run(key: Option<&str>) -> Result<ExitCode, String> {
    let (root, config) = repo::open()?;
    let ticket = resolve(&root, key)?;
    let mut problem = false;

    match &ticket.status {
        Status::Open(state) | Status::Closed(state) => {
            println!("ticket: {} — {}", ticket.key, state.title);
            match &state.closed {
                Some(c) => println!("state: closed by {} at {}: {}", c.by, c.at, c.reason),
                None => println!("state: open"),
            }
            match read(&ticket.dir.join("ticket.md")) {
                Err(reason) => {
                    println!("problem: {reason}");
                    problem = true;
                }
                Ok(text) => {
                    let sections = parse(&text);
                    if state.closed.is_some() {
                        println!("waiting for: none (closed)");
                    } else {
                        match waiting_for(&config, &sections, state) {
                            Some(s) => println!("waiting for: {} (owner: {})", s.id, s.owner),
                            None => println!("waiting for: none"),
                        }
                    }
                    for def in &config.sections {
                        let Some(c) = latest(state, &def.id) else {
                            continue;
                        };
                        for reason in freshness(&config, &sections, c) {
                            println!("stale: {} — {}", def.id, reason.describe(c));
                            if reason == stapel_core::stage::Reason::Changed
                                && let (Some(old), Some(current)) = (
                                    c.text.as_deref(),
                                    stapel_core::ticket::find(&sections, &def.title).found(),
                                )
                            {
                                print!("{}", diff(old, current));
                            }
                        }
                    }
                }
            }
        }
        Status::Legacy(v) => {
            println!(
                "ticket: {} — {}",
                ticket.key,
                v["title"].as_str().unwrap_or("(no title)")
            );
            println!("state: legacy (STP-1 format)");
        }
        Status::Unreadable(reason) => {
            println!("ticket: {}", ticket.key);
            println!("problem: {reason}");
            problem = true;
        }
    }

    for name in ["decisions.jsonl", "tokens.jsonl"] {
        for (line, _) in stapel_core::journal::problems(&ticket.dir.join(name)) {
            println!(
                "warning: journal line .stapel/tickets/{}/{name}:{line} cannot be read",
                ticket.key
            );
        }
    }
    if matches!(ticket.status, Status::Open(_) | Status::Closed(_)) {
        println!(
            "{}",
            stapel_core::checkstate::current(&root, &config, &ticket.dir)
        );
    }
    match build_permit(&root, Some(&config)) {
        Some(Permit::Computed(k)) => println!("build: allowed (by {k})"),
        Some(Permit::ByHand(k)) => println!("build: allowed by hand (phase 0, {k})"),
        None => println!("build: not allowed"),
    }
    Ok(if problem {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
