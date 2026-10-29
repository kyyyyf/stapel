//! The page the dialog of `stapel ok KEY` shows (STP-25): the header, one line per waiting
//! section, and the answer line.

use crate::config::Config;
use crate::confirm::Prepared;
use crate::stage::{Reason, freshness, latest};
use crate::ticket::{parse, read};

/// Section lines shown before the rest fold into one line.
const MAX_SECTION_LINES: usize = 10;

/// `1 line` for one, `<N> lines` otherwise.
pub fn count(n: usize, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}

/// `<id>: new, <count>`, `<id>: <count> changed (+a -b)` or `<id>: unchanged`, then
/// `; <reason>` for each stale reason other than the text change.
pub fn section_line(config: &Config, p: &Prepared) -> String {
    let new = || format!("new, {}", count(p.text.lines().count(), "line"));
    let Some(old) = latest(&p.state, &p.section) else {
        return format!("{}: {}", p.section, new());
    };
    let mut change = match old.text.as_deref() {
        None => new(),
        Some(old_text) => {
            let (mut added, mut removed) = (0, 0);
            for change in similar::TextDiff::from_lines(old_text, &p.text).iter_all_changes() {
                match change.tag() {
                    similar::ChangeTag::Insert => added += 1,
                    similar::ChangeTag::Delete => removed += 1,
                    similar::ChangeTag::Equal => {}
                }
            }
            if added + removed == 0 {
                "unchanged".to_string()
            } else {
                format!(
                    "{} changed (+{added} -{removed})",
                    count(added + removed, "line")
                )
            }
        }
    };
    let sections = read(&p.dir.join("ticket.md"))
        .map(|t| parse(&t))
        .unwrap_or_default();
    for reason in freshness(config, &sections, old)
        .iter()
        .filter(|r| **r != Reason::Changed)
    {
        change.push_str(&format!("; {}", reason.describe(old)));
    }
    format!("{}: {change}", p.section)
}

/// The dialog page for the waiting sections of `key`; `program` is the replaced program.
pub fn dialog_page(config: &Config, key: &str, program: &str, waiting: &[Prepared]) -> String {
    let mut lines = vec![format!(
        "stapel: confirm {} of {key}; runs: {program}",
        count(waiting.len(), "waiting section")
    )];
    lines.extend(
        waiting
            .iter()
            .take(MAX_SECTION_LINES)
            .map(|p| section_line(config, p)),
    );
    if waiting.len() > MAX_SECTION_LINES {
        let rest = &waiting[MAX_SECTION_LINES..];
        lines.push(format!(
            "… {} more waiting section{}: {}",
            rest.len(),
            if rest.len() == 1 { "" } else { "s" },
            rest.iter()
                .map(|p| p.section.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines.push("Answer Yes or No; do not choose \"don't ask again\".".into());
    lines.join("\n")
}
