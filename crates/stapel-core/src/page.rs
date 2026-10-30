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

/// Lines of the dialog page, and characters of one line.
const MAX_PAGE_LINES: usize = 22;
const MAX_LINE_CHARS: usize = 100;

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
    let parts = parts(waiting);
    let total: usize = parts
        .iter()
        .filter(|(_, l)| !l.is_empty())
        .map(|(_, l)| l.len() + 1)
        .sum();
    // The answer line is always last; the page holds `MAX_PAGE_LINES` lines.
    let room = MAX_PAGE_LINES.saturating_sub(lines.len() + 1);
    if total <= room {
        for (label, part) in &parts {
            if !part.is_empty() {
                lines.push((*label).into());
                lines.extend(part.iter().cloned());
            }
        }
    } else {
        let mut room = room.saturating_sub(1);
        let mut hidden = Vec::new();
        for (label, part) in &parts {
            let take = if room >= 2 {
                part.len().min(room - 1)
            } else {
                0
            };
            if take > 0 {
                lines.push((*label).into());
                lines.extend(part[..take].iter().cloned());
                room -= take + 1;
            }
            hidden.push(part.len() - take);
        }
        lines.push(format!(
            "… {} (frame {}, criteria {}, decisions {}); all of them: stapel status {key}",
            match hidden.iter().sum::<usize>() {
                1 => "1 more line".to_string(),
                n => format!("{n} more lines"),
            },
            hidden[0],
            hidden[1],
            hidden[2]
        ));
    }
    lines.push("Answer Yes or No; do not choose \"don't ask again\".".into());
    lines
        .iter()
        .map(|l| clean_cut(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The Frame, Criteria and Decisions parts of the waiting sections, each with its label.
fn parts(waiting: &[Prepared]) -> [(&'static str, Vec<String>); 3] {
    let description = waiting
        .iter()
        .find(|p| p.section.eq_ignore_ascii_case("description"))
        .map(description_parts);
    let (frame, decisions) = description.unwrap_or_default();
    let criteria: Vec<String> = waiting
        .iter()
        .filter(|p| p.section.eq_ignore_ascii_case("spec"))
        .flat_map(criteria_part)
        .collect();
    [
        ("Frame:", frame),
        ("Criteria:", criteria),
        ("Decisions:", decisions),
    ]
}

/// The terminal page: the label and the clean shown lines of each part, no limit and no cut;
/// every line ends with a newline.
pub fn terminal_page(waiting: &[Prepared]) -> String {
    let clean = |l: &str| -> String { l.chars().map(|c| if c.is_control() { ' ' } else { c }).collect() };
    let mut out = String::new();
    for (label, part) in parts(waiting) {
        if part.is_empty() {
            continue;
        }
        out.push_str(label);
        out.push('\n');
        for line in &part {
            out.push_str(&clean(line));
            out.push('\n');
        }
    }
    out
}

/// A page line with each control character as one space, cut to 99 characters and `…` when
/// longer than 100.
fn clean_cut(line: &str) -> String {
    let flat: String = line
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if flat.chars().count() > MAX_LINE_CHARS {
        format!(
            "{}…",
            flat.chars().take(MAX_LINE_CHARS - 1).collect::<String>()
        )
    } else {
        flat
    }
}

/// The AC rows and the other logical lines of a Spec, read as `ticket::parse` reads lines: a
/// paragraph or list item is joined by spaces, a blank line, a heading or a row ends it, and a
/// heading is a line of its own. Rows and headings in fences or HTML comments are plain text.
pub fn spec_lines(text: &str) -> (Vec<String>, Vec<String>) {
    logical_lines(text, true)
}

/// The logical lines of `spec_lines`; without `with_rows` an AC row is an ordinary line.
fn logical_lines(text: &str, with_rows: bool) -> (Vec<String>, Vec<String>) {
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
        } else if with_rows && !s.in_block && row_number(line).is_some() {
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
    // Rows that went still count: the Spec had rows, so the rows form is used.
    let had_rows = old.is_some_and(|o| !spec_lines(o).0.is_empty());
    if rows.is_empty() && !had_rows {
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

/// A Description split into the lines outside `### Decisions` (the Frame part) and the lines
/// under it, up to the next heading of level 3 or less; the `### Decisions` line is dropped.
fn split_description(text: &str) -> (String, String) {
    let (mut frame, mut decisions) = (Vec::new(), Vec::new());
    let mut in_decisions = false;
    for s in scan(text) {
        match &s.heading {
            Some((3, title)) if title == "Decisions" => {
                in_decisions = true;
                continue;
            }
            Some((level, _)) if *level <= 3 => in_decisions = false,
            _ => {}
        }
        if in_decisions {
            decisions.push(s.line);
        } else {
            frame.push(s.line);
        }
    }
    (frame.join("\n"), decisions.join("\n"))
}

/// The shown lines of the Frame part and of the Decisions part of a waiting Description.
fn description_parts(p: &Prepared) -> (Vec<String>, Vec<String>) {
    let old = latest(&p.state, &p.section)
        .and_then(|c| c.text.as_deref())
        .map(split_description);
    let (frame, decisions) = split_description(&p.text);
    let show = |old: Option<&String>, new: &String| {
        let new = logical_lines(new, false).1;
        match old {
            None => new.iter().map(|l| format!("  {l}")).collect(),
            Some(old) => diff_lines(&logical_lines(old, false).1, &new).0,
        }
    };
    (
        show(old.as_ref().map(|o| &o.0), &frame),
        show(old.as_ref().map(|o| &o.1), &decisions),
    )
}
