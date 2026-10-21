//! `stapel next [KEY]`: the next action of a ticket, from the facts on disk (STP-20). Read-only.

use crate::check::{base_of, history_report, read_head, shallow_refusal};
use crate::fail::Fail;
use crate::repo;
use stapel_core::brief::assemble;
use stapel_core::build_inputs::Bounds;
use stapel_core::checkstate::{self, CheckLine};
use stapel_core::config::Config;
use stapel_core::journal::{self, Entry};
use stapel_core::next::{Action, Facts, Fault, History, Target, decide};
use stapel_core::outcomes::{open_red, passing};
use stapel_core::steps::{Marker, git_text, step_commits, steps};
use stapel_core::ticket::{parse, read};
use stapel_core::tickets::{Status, Ticket, list, resolve};
use stapel_core::worktree::dirty_paths;
use std::path::Path;

/// The `--json` line: the keys of AC-2 in order, `facts` without the `blocked: ` prefix.
fn json_line(ticket: Option<&str>, action: &Action, facts: &[String]) -> String {
    let (mut command, mut path) = (None, None);
    match &action.target {
        Some(Target::Command(c)) => command = Some(c.clone()),
        Some(Target::Path(p)) => path = Some(p.clone()),
        None => {}
    }
    let facts: Vec<&str> = facts
        .iter()
        .map(|f| f.strip_prefix("blocked: ").unwrap_or(f))
        .collect();
    serde_json::json!({
        "v": 1,
        "ticket": ticket,
        "action": action.name,
        "subject": action.subject,
        "command": command,
        "path": path,
        "facts": facts,
    })
    .to_string()
}

fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// The history facts of an open ticket: its steps, their faults, the subjects of the first-parent
/// history and a failing current check. The refusals are those of `stapel check`, with exit 1.
fn gather(root: &Path, config: &Config, ticket: &Ticket) -> Result<History, Fail> {
    let key = &ticket.key;
    let head = match read_head(root) {
        Ok(head) => head,
        // A repository with no commit has no history.
        Err(_) if git_text(root, &["rev-parse", "--git-dir"]).is_ok() => {
            return Ok(History::default());
        }
        Err(e) => return Err(Fail::failed(e)),
    };
    shallow_refusal(root).map_err(Fail::failed)?;
    let base = base_of(root, ticket, &head).map_err(Fail::failed)?;
    let commits = step_commits(root, key, &head).map_err(Fail::failed)?;
    let last = commits.last().map(|c| c.sha.clone());
    let steps = steps(commits);
    let bounds = Bounds {
        base: base.as_deref(),
        closed: false,
    };
    let report = history_report(root, key, &head, &steps, bounds).map_err(Fail::failed)?;

    let mut faults = Vec::new();
    let mut open_red_label = None;
    for (step, s) in steps.iter().zip(&report.steps) {
        let a = &s.analysis;
        // An open RED is the only commit of its step and the newest step commit: it is judged by
        // its own changes, and its `unpaired` is no fault.
        let open = match step.commits.as_slice() {
            [only] if only.marker == Marker::Red && Some(&only.sha) == last.as_ref() => {
                Some(only.sha.clone())
            }
            _ => None,
        };
        let found = match open {
            Some(red) => {
                let judged = open_red(root, &red, &head).map_err(Fail::failed)?;
                if judged.is_none() {
                    open_red_label = Some(a.label.clone());
                }
                judged.map(|(outcome, reason)| (outcome, reason, red))
            }
            None => match &s.outcome {
                // `unverified: interleaved: …` is no fault; a changed build input is (AC-12).
                Some((outcome, reason))
                    if !passing(outcome)
                        && !(outcome == "unverified" && reason.starts_with("interleaved: ")) =>
                {
                    let changed = report.inputs.changed.as_ref().map(|c| c.sha.clone());
                    let sha = a
                        .fault
                        .clone()
                        .or_else(|| if a.fixed.is_none() { changed } else { None })
                        .or_else(|| a.red.as_ref().map(|c| c.sha.clone()))
                        .or_else(|| a.green.as_ref().map(|c| c.sha.clone()))
                        .unwrap_or_default();
                    Some((outcome.clone(), reason.clone(), sha))
                }
                _ => None,
            },
        };
        if let Some((outcome, reason, sha)) = found {
            faults.push(Fault {
                label: a.label.clone(),
                outcome,
                sha: short(&sha),
                reason,
            });
        }
    }

    let subjects = git_text(root, &["log", "--first-parent", "--format=%s", &head])
        .map_err(Fail::failed)?
        .lines()
        .map(String::from)
        .collect();
    let line = checkstate::current(root, config, &ticket.dir);
    let check_configured = line != CheckLine::NotConfigured;
    let check_pass = matches!(line, CheckLine::Pass(_));
    let check_fail = match line {
        CheckLine::Fail(sha) => Some(sha),
        _ => None,
    };
    let dirty = dirty_paths(root).map_err(Fail::failed)?;
    Ok(History {
        labels: steps.iter().map(|s| s.label.clone()).collect(),
        greens: steps
            .iter()
            .filter(|s| s.greens().next().is_some())
            .map(|s| s.label.clone())
            .collect(),
        faults,
        subjects,
        check_fail,
        check_configured,
        check_pass,
        open_red: open_red_label,
        dirty,
        brief: None,
    })
}

pub fn run(key: Option<&str>, json: bool) -> Result<(), Fail> {
    let (root, config) = repo::open().map_err(Fail::failed)?;
    if key.is_none()
        && !list(&root)
            .iter()
            .any(|t| matches!(t.status, Status::Open(_)))
    {
        let action = Action {
            name: "new-ticket",
            subject: None,
            target: Some(Target::Command("stapel new \"<title>\"".to_string())),
        };
        if json {
            say!("{}", json_line(None, &action, &[]));
        } else {
            say!("{}", action.line());
        }
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

    let mut facts = Facts {
        key: &ticket.key,
        config: &config,
        state,
        sections: &sections,
        finding_stages: &finding_stages,
        history: None,
    };
    let mut decision = decide(&facts);
    // The history is read only when no rule before the build applies (AC-10).
    let (gathered, with_brief);
    if decision.needs_history {
        gathered = gather(&root, &config, &ticket)?;
        facts.history = Some(&gathered);
        decision = decide(&facts);
        // The brief of the step the action names, and only of that step (STP-21 AC-16, AC-17).
        let label = match (decision.action.name, &decision.action.subject) {
            ("build-step" | "green-step", Some(label)) => label.strip_prefix("step "),
            _ => None,
        };
        if let (Some(step), Some(def)) = (label, config.section("plan")) {
            let path = ticket.dir.join("findings.jsonl");
            let findings = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => {
                    return Err(Fail::failed(format!("cannot read {}: {e}", path.display())));
                }
            };
            let refusal = assemble(
                &text,
                &ticket.key,
                step,
                state.base.as_deref(),
                &def.title,
                &findings,
            )
            .err();
            with_brief = History {
                brief: Some((format!("step {step}"), refusal)),
                ..gathered.clone()
            };
            facts.history = Some(&with_brief);
            decision = decide(&facts);
        }
    }
    if json {
        say!(
            "{}",
            json_line(Some(&ticket.key), &decision.action, &decision.facts)
        );
        return Ok(());
    }
    say!("ticket: {} — {}", ticket.key, state.title);
    for fact in &decision.facts {
        say!("{fact}");
    }
    say!("{}", decision.action.line());
    Ok(())
}
