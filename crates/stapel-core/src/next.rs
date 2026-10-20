//! The next action of a ticket, computed from facts (STP-20 AC-5 to AC-9).
//!
//! `decide` is a pure function over facts the command gathered: it reads no file and runs no git.

use crate::config::Config;
use crate::stage::{freshness, latest};
use crate::state::State;
use crate::ticket::{Lookup, Section, find};

/// What an action points at: a command to run or a file to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Command(String),
    Path(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub name: &'static str,
    pub subject: Option<String>,
    pub target: Option<Target>,
}

impl Action {
    fn new(name: &'static str, subject: Option<String>, target: Option<Target>) -> Action {
        Action {
            name,
            subject,
            target,
        }
    }

    /// `next: <action>`, then ` <subject>` and ` — <target>` when there are any.
    pub fn line(&self) -> String {
        let mut line = format!("next: {}", self.name);
        if let Some(subject) = &self.subject {
            line.push(' ');
            line.push_str(subject);
        }
        match &self.target {
            Some(Target::Command(t) | Target::Path(t)) => line.push_str(&format!(" — {t}")),
            None => {}
        }
        line
    }
}

/// The blocking facts, in print order, and the one action.
#[derive(Debug, PartialEq, Eq)]
pub struct Decision {
    pub facts: Vec<String>,
    pub action: Action,
    /// Set when no rule before the build applied and `Facts::history` was `None`: the action is a
    /// placeholder, and the caller reads the history and decides again.
    pub needs_history: bool,
}

/// A step with a history outcome that blocks the build (STP-20 AC-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub label: String,
    pub outcome: String,
    /// The short id of the fault commit.
    pub sha: String,
    pub reason: String,
}

/// What the rules that read history need (STP-20 AC-11 to AC-15).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    /// The labels of the step commits of the ticket, in history order, each once.
    pub labels: Vec<String>,
    /// The labels that have a GREEN commit.
    pub greens: Vec<String>,
    /// The faults, in history order.
    pub faults: Vec<Fault>,
    /// The subjects of the first-parent history of `HEAD`.
    pub subjects: Vec<String>,
    /// The short id of a current failing check.
    pub check_fail: Option<String>,
    /// Whether `[check]` is set in `stapel.toml`.
    pub check_configured: bool,
    /// Whether the newest check record is a pass that is still current: its commit is an ancestor
    /// of `HEAD` and no file outside the machine files has changed since (`CheckLine::Pass`).
    pub check_pass: bool,
    /// The label of an open RED without a fault: the only step commit of its step, the newest
    /// step commit of the ticket.
    pub open_red: Option<String>,
    /// The paths with uncommitted changes outside the machine files (`worktree::dirty_paths`).
    pub dirty: Vec<String>,
}

/// Whether `subject` is `<KEY> <label> <word>: …` with a word other than `RED:` and `GREEN:`
/// (a coverage or cleanup commit of the step).
pub fn closes_step(key: &str, label: &str, subject: &str) -> bool {
    let rest = subject
        .strip_prefix(key)
        .and_then(|r| r.strip_prefix(' '))
        .and_then(|r| r.strip_prefix(label))
        .and_then(|r| r.strip_prefix(' '));
    rest.and_then(|r| r.split_whitespace().next())
        .is_some_and(|w| w.len() > 1 && w.ends_with(':') && w != "RED:" && w != "GREEN:")
}

/// What the rules before the build read.
pub struct Facts<'a> {
    /// The folder key of the ticket.
    pub key: &'a str,
    pub config: &'a Config,
    pub state: &'a State,
    pub sections: &'a [Section],
    /// The `stage` of each record of `findings.jsonl`.
    pub finding_stages: &'a [String],
    /// The history facts; `None` until the caller has read the history.
    pub history: Option<&'a History>,
}

/// The ticket file, relative to the repository root.
pub fn ticket_path(key: &str) -> String {
    format!(".stapel/tickets/{key}/ticket.md")
}

/// A section found once whose body is neither empty nor `TODO`.
fn written(sections: &[Section], title: &str) -> bool {
    find(sections, title)
        .found()
        .is_some_and(|body| !matches!(body.trim(), "" | "TODO"))
}

/// The label of a Plan step line: digits, at most one lowercase letter, `.` and a space, at the
/// first column. `1c.` gives `step 1c`.
fn step_label(line: &str) -> Option<String> {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = &line[digits..];
    let letter = if rest.starts_with(|c: char| c.is_ascii_lowercase()) {
        1
    } else {
        0
    };
    rest[letter..].strip_prefix(". ")?;
    Some(format!("step {}{}", &line[..digits], &rest[..letter]))
}

/// The labels of the Plan steps, in Plan order, repeats included.
pub fn plan_steps(plan: &str) -> Vec<String> {
    plan.lines().filter_map(step_label).collect()
}

/// Whether the first line of the Plan that starts with `Route:` has `light` as its first word.
pub fn is_light(plan: &str) -> bool {
    plan.lines()
        .find_map(|line| line.strip_prefix("Route:"))
        .and_then(|rest| rest.split_whitespace().next())
        .is_some_and(|word| {
            word.trim_end_matches([',', '.', ';', ':'])
                .eq_ignore_ascii_case("light")
        })
}

/// The facts about a Plan with no step or with a label twice.
fn plan_faults(plan: &str) -> Vec<String> {
    let labels = plan_steps(plan);
    if labels.is_empty() {
        return vec!["the Plan has no steps".into()];
    }
    let mut seen: Vec<&String> = Vec::new();
    let mut twice: Vec<&String> = Vec::new();
    for label in &labels {
        if seen.contains(&label) && !twice.contains(&label) {
            twice.push(label);
        }
        seen.push(label);
    }
    twice
        .into_iter()
        .map(|label| format!("the Plan names {label} twice"))
        .collect()
}

/// The history facts of AC-11 to AC-13 and the action of AC-11 to AC-20.
///
/// `plan` is `None` when no section of id `plan` is configured: no Plan step is read (AC-21).
fn history_action(f: &Facts, h: &History, plan: Option<&str>, facts: &mut Vec<String>) -> Action {
    let path = ticket_path(f.key);
    let labels = plan.map(plan_steps).unwrap_or_default();
    let outside: Vec<&String> = if plan.is_some() {
        h.labels.iter().filter(|l| !labels.contains(l)).collect()
    } else {
        Vec::new()
    };
    for label in &outside {
        facts.push(format!("history has {label}, the Plan does not"));
    }
    for fault in &h.faults {
        facts.push(format!(
            "{}: {} at {} — {}",
            fault.label, fault.outcome, fault.sha, fault.reason
        ));
    }
    if let Some(sha) = &h.check_fail {
        facts.push(format!("check fails at {sha}"));
    }
    if !outside.is_empty() {
        let title = f
            .config
            .section("plan")
            .map_or("Plan", |d| d.title.as_str());
        return Action::new(
            "write-section",
            Some(title.to_string()),
            Some(Target::Path(path)),
        );
    }
    if let Some(fault) = h.faults.first() {
        return Action::new(
            "fix-history",
            Some(fault.label.clone()),
            Some(Target::Command(format!("stapel check --history {}", f.key))),
        );
    }
    if h.check_fail.is_some() {
        return Action::new(
            "fix-check",
            None,
            Some(Target::Path(format!(
                ".stapel/tickets/{}/runs.jsonl",
                f.key
            ))),
        );
    }
    if let Some(label) = &h.open_red {
        return Action::new("green-step", Some(label.clone()), Some(Target::Path(path)));
    }
    // A word commit closes a step that has no RED and no GREEN (AC-15).
    let open = labels.iter().find(|l| {
        !h.greens.contains(l)
            && (h.labels.contains(l) || !h.subjects.iter().any(|s| closes_step(f.key, l, s)))
    });
    match open {
        Some(label) if h.labels.contains(label) => {
            Action::new("green-step", Some(label.clone()), Some(Target::Path(path)))
        }
        Some(label) => Action::new("build-step", Some(label.clone()), Some(Target::Path(path))),
        None => after_the_steps(f, h, facts),
    }
}

/// The rules once every Plan step is done (STP-20 AC-16 to AC-20).
fn after_the_steps(f: &Facts, h: &History, facts: &mut Vec<String>) -> Action {
    if !h.dirty.is_empty() {
        let first: Vec<&str> = h.dirty.iter().take(5).map(String::as_str).collect();
        facts.push(format!("uncommitted changes: {}", first.join(", ")));
        return Action::new(
            "commit",
            None,
            Some(Target::Command("git status".to_string())),
        );
    }
    if h.check_configured && !h.check_pass {
        return Action::new(
            "run-check",
            None,
            Some(Target::Command(format!("stapel check {}", f.key))),
        );
    }
    for (id, name) in [("review", "code-review"), ("summary", "write-summary")] {
        if let Some(def) = f.config.section(id)
            && !written(f.sections, &def.title)
        {
            return Action::new(name, None, Some(Target::Path(ticket_path(f.key))));
        }
    }
    Action::new(
        "close",
        None,
        Some(Target::Command(format!("stapel close {}", f.key))),
    )
}

pub fn decide(f: &Facts) -> Decision {
    let path = ticket_path(f.key);
    if f.state.closed.is_some() {
        return Decision {
            facts: Vec::new(),
            action: Action::new("closed", None, None),
            needs_history: false,
        };
    }

    let confirmed: Vec<_> = f
        .config
        .sections
        .iter()
        .filter(|s| s.owner != "generated")
        .collect();
    let mut facts = Vec::new();
    // The first confirmed section that is not written; the Plan also needs steps, each label once.
    let mut unwritten = None;
    for def in &confirmed {
        let mut ok = written(f.sections, &def.title);
        if find(f.sections, &def.title) == Lookup::Duplicated {
            facts.push(format!("{} is duplicated", def.title));
        }
        if ok
            && def.id == "plan"
            && let Some(plan) = find(f.sections, &def.title).found()
        {
            let faults = plan_faults(plan);
            ok = faults.is_empty();
            facts.extend(faults);
        }
        if !ok && unwritten.is_none() {
            unwritten = Some(def.title.clone());
        }
    }

    // The confirmation facts (AC-9); they are printed only with the action `confirm`.
    let mut confirm_facts = Vec::new();
    for def in &confirmed {
        match latest(f.state, &def.id) {
            None => confirm_facts.push(format!("{} not confirmed", def.id)),
            Some(c) => {
                for reason in freshness(f.config, f.sections, c) {
                    confirm_facts.push(format!(
                        "{} confirmation is stale: {}",
                        def.id,
                        reason.describe(c)
                    ));
                }
            }
        }
    }
    let unconfirmed = !confirm_facts.is_empty();

    let plan = f
        .config
        .section("plan")
        .and_then(|def| find(f.sections, &def.title).found());
    let plan_is_light = plan.is_some_and(is_light);
    let mut needs_history = false;
    let action = if !written(f.sections, "Description")
        || !find(f.sections, "Description")
            .found()
            .is_some_and(|body| body.lines().any(|l| l.trim_end() == "### Frame"))
    {
        Action::new("write-frame", None, Some(Target::Path(path)))
    } else if let Some(title) = unwritten {
        Action::new("write-section", Some(title), Some(Target::Path(path)))
    } else if !plan_is_light
        && f.state.confirmations.is_empty()
        && !f.finding_stages.iter().any(|s| s == "spec-review")
    {
        Action::new("spec-review", None, Some(Target::Path(path)))
    } else if unconfirmed {
        facts.extend(confirm_facts);
        Action::new(
            "confirm",
            None,
            Some(Target::Command(format!("stapel ok {}", f.key))),
        )
    } else if let Some(h) = f.history {
        history_action(f, h, plan, &mut facts)
    } else {
        // The rules that read history come next; the caller reads it and decides again.
        needs_history = true;
        let first = plan.and_then(|plan| plan_steps(plan).into_iter().next());
        Action::new("build-step", first, Some(Target::Path(path)))
    };
    let facts = facts.into_iter().map(|s| format!("blocked: {s}")).collect();
    Decision {
        facts,
        action,
        needs_history,
    }
}
