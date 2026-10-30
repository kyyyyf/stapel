//! The page the dialog of `stapel ok KEY` shows (STP-25): the header, one line per waiting
//! section, and the answer line.

use crate::brief::row_number;
use crate::config::Config;
use crate::confirm::Prepared;
use crate::stage::{Reason, freshness, latest};
use crate::ticket::{parse, read, scan};
use similar::{ChangeTag, TextDiff};

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
    for p in waiting
        .iter()
        .filter(|p| p.section.eq_ignore_ascii_case("spec"))
    {
        let part = criteria_part(p);
        if !part.is_empty() {
            lines.push("Criteria:".into());
            lines.extend(part);
        }
    }
    lines.push("Answer Yes or No; do not choose \"don't ask again\".".into());
    lines.join("\n")
}

/// The AC rows and the other logical lines of a Spec, read as `ticket::parse` reads lines: a
/// paragraph or list item is joined by spaces, a blank line, a heading or a row ends it, and a
/// heading is a line of its own. Rows and headings in fences or HTML comments are plain text.
pub fn spec_lines(text: &str) -> (Vec<String>, Vec<String>) {
    let (mut rows, mut out) = (Vec::new(), Vec::new());
    let mut cur: Vec<String> = Vec::new();
    let flush = |cur: &mut Vec<String>, out: &mut Vec<String>| {
        if !cur.is_empty() {
            out.push(cur.join(" "));
            cur.clear();
        }
    };
    for s in scan(text) {
        let line = s.line.trim();
        let item = line.starts_with("- ")
            || line.starts_with("* ")
            || line
                .split_once(". ")
                .is_some_and(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        if line.is_empty() {
            flush(&mut cur, &mut out);
        } else if s.heading.is_some() {
            flush(&mut cur, &mut out);
            out.push(line.to_string());
        } else if !s.in_block && row_number(line).is_some() {
            flush(&mut cur, &mut out);
            rows.push(line.to_string());
        } else {
            if item && !s.in_block {
                flush(&mut cur, &mut out);
            }
            cur.push(line.to_string());
        }
    }
    flush(&mut cur, &mut out);
    (rows, out)
}

/// Inserted (`+ `) and deleted (`- `) lines of a line diff, in diff order, with their counts.
fn diff_lines(old: &[String], new: &[String]) -> (Vec<String>, usize, usize) {
    let (o, n): (Vec<&str>, Vec<&str>) = (
        old.iter().map(String::as_str).collect(),
        new.iter().map(String::as_str).collect(),
    );
    let (mut shown, mut a, mut r) = (Vec::new(), 0, 0);
    for c in TextDiff::from_slices(&o, &n).iter_all_changes() {
        match c.tag() {
            ChangeTag::Insert => {
                a += 1;
                shown.push(format!("+ {}", c.value()));
            }
            ChangeTag::Delete => {
                r += 1;
                shown.push(format!("- {}", c.value()));
            }
            ChangeTag::Equal => {}
        }
    }
    (shown, a, r)
}

/// The lines of the Criteria part for a waiting Spec; empty when nothing is shown.
fn criteria_part(p: &Prepared) -> Vec<String> {
    let old = latest(&p.state, &p.section).and_then(|c| c.text.as_deref());
    let (rows, other) = spec_lines(&p.text);
    if rows.is_empty() {
        return match old {
            None => other.iter().map(|l| format!("  {l}")).collect(),
            Some(old) => diff_lines(&spec_lines(old).1, &other).0,
        };
    }
    let (mut shown, a, r) = match old {
        None => (
            rows.iter().map(|l| format!("  {l}")).collect(),
            other.len(),
            0,
        ),
        Some(old) => {
            let (old_rows, old_other) = spec_lines(old);
            let (shown, ..) = diff_lines(&old_rows, &rows);
            let (_, a, r) = diff_lines(&old_other, &other);
            (shown, a, r)
        }
    };
    if a + r > 0 {
        shown.push(format!("spec, other lines: +{a} -{r}"));
    }
    shown
}
