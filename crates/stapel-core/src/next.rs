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

pub fn decide(f: &Facts) -> Decision {
    let path = ticket_path(f.key);
    if f.state.closed.is_some() {
        return Decision {
            facts: Vec::new(),
            action: Action::new("closed", None, None),
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

    let mut unconfirmed = false;
    for def in &confirmed {
        match latest(f.state, &def.id) {
            None => {
                facts.push(format!("{} not confirmed", def.id));
                unconfirmed = true;
            }
            Some(c) => {
                for reason in freshness(f.config, f.sections, c) {
                    facts.push(format!(
                        "{} confirmation is stale: {}",
                        def.id,
                        reason.describe(c)
                    ));
                    unconfirmed = true;
                }
            }
        }
    }
    let facts: Vec<String> = facts.into_iter().map(|s| format!("blocked: {s}")).collect();

    let plan_is_light = f
        .config
        .section("plan")
        .and_then(|def| find(f.sections, &def.title).found())
        .is_some_and(is_light);
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
        Action::new(
            "confirm",
            None,
            Some(Target::Command(format!("stapel ok {}", f.key))),
        )
    } else {
        // Step 1 stops here; the rules that read history come next.
        let first = f
            .config
            .section("plan")
            .and_then(|def| find(f.sections, &def.title).found())
            .and_then(|plan| plan_steps(plan).into_iter().next());
        Action::new("build-step", first, Some(Target::Path(path)))
    };
    Decision { facts, action }
}
