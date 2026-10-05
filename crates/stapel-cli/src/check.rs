//! `stapel check [KEY] [--list]`: the RED to GREEN check of a ticket's steps (STP-4).

use crate::repo;
use stapel_core::outcomes::{analyse, passing};
use stapel_core::steps::{Step, git_text, step_commits, step_tests, steps};
use stapel_core::tickets::{Status, resolve};
use std::process::ExitCode;

/// A refusal: nothing was run, exit 2.
fn refuse(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("stapel: {message}");
    ExitCode::from(2)
}

pub fn run(key: Option<&str>, list: bool) -> ExitCode {
    let (root, _config) = match repo::open() {
        Ok(v) => v,
        Err(e) => return refuse(e),
    };
    let ticket = match resolve(&root, key) {
        Ok(t) => t,
        Err(e) => return refuse(e),
    };
    match &ticket.status {
        Status::Open(_) | Status::Closed(_) => {}
        Status::Legacy(_) => {
            return refuse(format!(
                "ticket {} has a legacy (STP-1) state.json; it cannot be checked",
                ticket.key
            ));
        }
        Status::Unreadable(r) => return refuse(r),
    }
    let head = match git_text(&root, &["rev-parse", "--verify", "HEAD"]) {
        Ok(h) => h.trim().to_string(),
        Err(e) => return refuse(e),
    };
    let steps = match step_commits(&root, &ticket.key, &head) {
        Ok(c) => steps(c),
        Err(e) => return refuse(e),
    };
    if steps.is_empty() {
        return refuse(format!(
            "ticket {key} has no step commits (subjects `{key} <label> RED: …` and \
             `{key} <label> GREEN: …` on the first-parent history of HEAD)",
            key = ticket.key
        ));
    }
    if list {
        return match print_list(&root, &ticket.key, &steps) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => refuse(e),
        };
    }
    let analyses = match analyse(&root, &head, &steps) {
        Ok(a) => a,
        Err(e) => return refuse(e),
    };
    println!("ticket: {} at {}", ticket.key, short(&head));
    let mut ok = true;
    for a in &analyses {
        match &a.fixed {
            Some((outcome, reason)) => {
                ok &= passing(outcome);
                if outcome == "retired" {
                    println!("{}: {outcome}", a.label);
                } else {
                    println!("{}: {outcome}: {reason}", a.label);
                }
            }
            None => {
                ok = false;
                println!("{}: not run (the runner comes with STP-4 step 5)", a.label);
            }
        }
        for n in &a.notes {
            println!("  {} {}: {}", n.kind, n.by, n.test);
        }
    }
    println!("result: {}", if ok { "pass" } else { "fail" });
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

fn print_list(root: &std::path::Path, key: &str, steps: &[Step]) -> Result<(), String> {
    println!("ticket: {key}");
    for step in steps {
        println!("{}", step.label);
        let reds: Vec<_> = step.reds().collect();
        let greens: Vec<_> = step.greens().collect();
        for (name, commits) in [("red", &reds), ("green", &greens)] {
            if commits.is_empty() {
                println!("  {name}: —");
            }
            for c in commits {
                println!("  {name}: {} {}", short(&c.sha), c.subject);
            }
        }
        let tests = match reds.first() {
            Some(red) => step_tests(root, &red.sha)?,
            None => Vec::new(),
        };
        println!("  tests: {}", tests.len());
        for t in tests {
            println!("    {t}");
        }
    }
    Ok(())
}
