//! `stapel next [KEY]`: the next action of a ticket, from the facts on disk (STP-20). It changes only the stops of the trial merge (STP-27).

use crate::check::{base_of, history_report, junit_globs_at, mode_of, read_head, shallow_refusal};
use crate::fail::Fail;
use crate::repo;
use stapel_core::brief::assemble;
use stapel_core::build_inputs::Bounds;
use stapel_core::checkstate::{self, CheckLine};
use stapel_core::config::Config;
use stapel_core::journal::{self, Entry};
use stapel_core::next::{Action, Facts, Fault, History, Target, decide};
use stapel_core::outcomes::{open_red, passing};
use stapel_core::review::{finding_round, patch_findings};
use stapel_core::steps::{Marker, git_text, step_commits, steps, walk_range};
use stapel_core::ticket::{parse, read};
use stapel_core::tickets::{Status, Ticket, list, resolve};
use stapel_core::triage::blocking_findings;
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

/// The highest round whose review folder holds `pack.md`; 0 when there is none.
fn pack_round(root: &Path, key: &str) -> u64 {
    let Ok(entries) = std::fs::read_dir(root.join(".stapel/scratch/review")) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let n = name.strip_prefix(key)?.strip_prefix("-r")?;
            let round: u64 = n.parse().ok()?;
            (round.to_string() == n && e.path().join("pack.md").is_file()).then_some(round)
        })
        .max()
        .unwrap_or(0)
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
    let commits = step_commits(root, key, &head, base.as_deref()).map_err(Fail::failed)?;
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
                let globs = junit_globs_at(root);
                let judged = open_red(root, &red, &head, mode_of(&globs)).map_err(Fail::failed)?;
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
                    let changed = s.changed.clone();
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

    let walk = walk_range(base.as_deref(), &head);
    let subjects = git_text(root, &["log", "--first-parent", "--format=%s", &walk])
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
    let own = stapel_core::tickets::branch_ticket(root)
        .is_some_and(|t| t.key.eq_ignore_ascii_case(&ticket.key));
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
        // Both read the branch ticket of the checkout: only its own ticket shows them (W-7).
        merge_main: own.then(|| stapel_core::parallel::merge_main_needed(root)).flatten(),
        merge_review: own
            .then(|| stapel_core::parallel::merge_review_due(root, &ticket.dir))
            .flatten(),
    })
}

pub fn run(key: Option<&str>, json: bool) -> Result<(), Fail> {
    let (root, config) = repo::open().map_err(Fail::failed)?;
    for line in lines(&root, &config, key, json)? {
        say!("{line}");
    }
    Ok(())
}

/// The text of `stapel next` for the repository at `root`, one string per output line.
pub fn text_at(root: &Path) -> Result<Vec<String>, Fail> {
    let config = repo::load(root).map_err(Fail::failed)?;
    lines(root, &config, None, false)
}

fn lines(root: &Path, config: &Config, key: Option<&str>, json: bool) -> Result<Vec<String>, Fail> {
    let (root, config) = (root.to_path_buf(), config.clone());
    let mut out = Vec::new();
    // A checkout without a branch ticket names the tickets that can go on (STP-27 AC-24).
    if key.is_none() && !json {
        out.extend(stapel_core::parallel::go_on_notices(&root));
    }
    // A closed branch ticket still has its close to commit or its branch to merge (AC-23).
    let closing = match (key, stapel_core::tickets::branch_ticket(&root)) {
        (own, Some(t))
            if matches!(t.status, Status::Closed(_))
                && own.is_none_or(|k| k.eq_ignore_ascii_case(&t.key)) =>
        {
            stapel_core::parallel::branch_close(&root, &t)
        }
        _ => None,
    };
    // A branch ticket whose own state cannot be read waits; it is not a checkout without a ticket
    // (STP-27 AC-17).
    if key.is_none()
        && let Some(own) = stapel_core::tickets::branch_ticket(&root)
        && matches!(own.status, Status::Unreadable(_))
    {
        let waits = stapel_core::parallel::waits(&root);
        let facts: Vec<String> = waits.iter().map(|w| format!("blocked: {}", w.line())).collect();
        let action = Action {
            name: "wait",
            subject: None,
            target: Some(Target::Command(
                waits
                    .first()
                    .map_or_else(|| "unknown".to_string(), |w| w.earlier.clone()),
            )),
        };
        if json {
            out.push(json_line(Some(&own.key), &action, &facts));
        } else {
            out.extend(facts);
            out.push(action.line());
        }
        return Ok(out);
    }
    if closing.is_none()
        && key.is_none()
        && !list(&root)
            .iter()
            .any(|t| matches!(t.status, Status::Open(_)))
    {
        let action = Action {
            name: "new-ticket",
            subject: None,
            target: Some(Target::Command("stapel new \"<title>\"".to_string())),
        };
        out.push(if json {
            json_line(None, &action, &[])
        } else {
            action.line()
        });
        return Ok(out);
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
    let entries = journal::read(&ticket.dir.join("findings.jsonl")).map_err(Fail::failed)?;
    let records: Vec<_> = entries
        .into_iter()
        .filter_map(|(_, entry)| match entry {
            Entry::V1(map) | Entry::Legacy(map) => Some(map),
            Entry::Problem(_) => None,
        })
        .collect();
    let finding_stages: Vec<String> = records
        .iter()
        .filter_map(|map| map.get("stage")?.as_str().map(String::from))
        .collect();
    let blocking = blocking_findings(&records);

    let review_round = finding_round(&records).max(pack_round(&root, &ticket.key));
    let patch_finding = !patch_findings(&records).is_empty();

    let mut facts = Facts {
        key: &ticket.key,
        config: &config,
        state,
        sections: &sections,
        finding_stages: &finding_stages,
        blocking: &blocking,
        review_round,
        patch_finding,
        history: None,
    };
    let mut decision = decide(&facts);
    // The history is read only when no rule before the build applies (AC-10).
    let (gathered, with_brief);
    let at_build = decision.needs_history;
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
    match closing {
        Some(stapel_core::parallel::BranchClose::Commit) => {
            decision.action = Action {
                name: "commit-close",
                subject: None,
                target: Some(Target::Command(format!(
                    "git commit .stapel/tickets/{}",
                    ticket.key
                ))),
            };
        }
        Some(stapel_core::parallel::BranchClose::Merge { main, not_ready }) => {
            if let Some(reason) = not_ready {
                decision
                    .facts
                    .insert(0, format!("main checkout not ready: {reason}"));
            }
            decision.action = Action {
                name: "merge-branch",
                subject: None,
                target: Some(Target::Command(format!(
                    "git merge --no-ff {} (in {})",
                    ticket.key,
                    main.display()
                ))),
            };
        }
        None => {}
    }
    // The trial merge with each parallel ticket writes or removes the stops (STP-27 AC-16).
    // A foreign KEY runs none (D11-2).
    let own_key = stapel_core::tickets::branch_ticket(&root).map(|t| t.key);
    let foreign = key.is_some()
        && own_key
            .as_deref()
            .is_none_or(|own| !own.eq_ignore_ascii_case(&ticket.key));
    let trials = if foreign {
        Vec::new()
    } else {
        stapel_core::parallel::trial_merges(&root)
    };
    for trial in trials {
        match trial {
            stapel_core::parallel::Trial::NotRun(reason) => {
                decision.facts.push(format!("trial merge not run: {reason}"));
            }
            stapel_core::parallel::Trial::StopNotWritten(why) => {
                decision.facts.push(format!("stop not written: {why}"));
            }
            _ => {}
        }
    }
    // The dependency review of each earlier parallel ticket comes first (STP-27 AC-20).
    let mut reviewing = false;
    if at_build {
        let pending = stapel_core::depends::pending_for(&root, &ticket);
        if !pending.is_empty() {
            reviewing = true;
            decision
                .facts
                .extend(pending.iter().map(|p| format!("blocked: {}", p.line())));
            decision.action = Action {
                name: "dependency-review",
                subject: None,
                target: Some(Target::Command(format!("stapel depends {}", ticket.key))),
            };
        }
    }
    // A stopped ticket shows the wait in every stage; it takes the place of a build action only
    // (STP-27 AC-17). `next` removes the stops that are obsolete.
    if !foreign {
        stapel_core::parallel::prune_stops(&root);
        let waits = stapel_core::parallel::waits(&root);
        if let Some(first) = waits.first() {
            decision
                .facts
                .extend(waits.iter().map(|w| format!("blocked: {}", w.line())));
            let build = matches!(
                decision.action.name,
                "build-step" | "green-step" | "run-check" | "code-review" | "close"
            );
            if build && !reviewing {
                decision.action = Action {
                    name: "wait",
                    subject: None,
                    target: Some(Target::Command(first.earlier.clone())),
                };
            }
        }
    }
    if json {
        out.push(json_line(
            Some(&ticket.key),
            &decision.action,
            &decision.facts,
        ));
        return Ok(out);
    }
    out.push(format!("ticket: {} — {}", ticket.key, state.title));
    out.extend(decision.facts.iter().cloned());
    out.push(decision.action.line());
    Ok(out)
}
