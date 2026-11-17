//! The page the dialog of `stapel ok KEY` shows (STP-25): the header, one line per waiting
//! section (folded past the tenth), the Frame, the criteria and the Decisions with their changed
//! lines, the line that counts lines over the limit, and the answer line.

use crate::brief::row_number;
use crate::config::Config;
use crate::confirm::Prepared;
use crate::stage::{Reason, freshness, latest};
use crate::ticket::{Lookup, find, parse, read, scan};
use similar::{ChangeTag, TextDiff};

/// Section lines shown before the rest fold into one line.
const MAX_SECTION_LINES: usize = 10;

/// Characters of one summary line.
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

/// `sha256:` and the first 12 hex digits of a page hash.
pub fn short_hash(hash: &str) -> &str {
    &hash[..hash.len().min("sha256:".len() + 12)]
}

/// The AC rows of the current Spec of the ticket in `dir`; 0 when there is no such section.
fn criteria_count(config: &Config, dir: &std::path::Path) -> usize {
    let Some(spec) = config
        .sections
        .iter()
        .find(|s| s.id.eq_ignore_ascii_case("spec"))
    else {
        return 0;
    };
    let sections = read(&dir.join("ticket.md"))
        .map(|t| parse(&t))
        .unwrap_or_default();
    match find(&sections, &spec.title) {
        Lookup::Found(body) => spec_lines(body).0.len(),
        _ => 0,
    }
}

/// The summary the dialog of `stapel ok KEY` shows (STP-32): the title, the criteria count, one
/// line per waiting section (folded past the tenth), the page path and its short hash. `program`
/// is the replaced program.
pub fn summary(config: &Config, key: &str, program: &str, waiting: &[Prepared]) -> String {
    let title = waiting.first().map(|p| p.state.title.as_str()).unwrap_or("");
    let criteria = waiting
        .first()
        .map(|p| criteria_count(config, &p.dir))
        .unwrap_or(0);
    let mut lines = vec![
        format!(
            "stapel: confirm {} of {key}; runs: {program}",
            count(waiting.len(), "waiting section")
        ),
        format!("title: {title}"),
        format!("criteria: {criteria}"),
    ];
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
    lines.push(format!(
        "page: .stapel/tickets/{key}/page.md ({})",
        short_hash(&page_hash(page_text(config, waiting).as_bytes()))
    ));
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

/// The page file of the waiting sections (STP-32): the title, the Sections list, then the Frame,
/// Criteria and Decisions parts, each in a `diff` fence with a mark before every line. Ends with
/// one newline.
pub fn page_text(config: &Config, waiting: &[Prepared]) -> String {
    let Some(first) = waiting.first() else {
        return String::new();
    };
    let flat = |s: &str| -> String {
        s.chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect()
    };
    let mut out = format!(
        "# {} — {}\n\n## Sections\n",
        flat(&first.key),
        flat(&first.state.title)
    );
    for p in waiting {
        out.push_str(&format!("- {}\n", flat(&section_line(config, p))));
    }
    for (title, lines, tail) in marked_parts(waiting) {
        if lines.is_empty() {
            continue;
        }
        let lines: Vec<String> = lines.iter().map(|l| flat(l)).collect();
        let longest = lines.iter().map(|l| backtick_run(l)).max().unwrap_or(0);
        let fence = "`".repeat((longest + 1).max(3));
        out.push_str(&format!("\n## {title}\n{fence}diff\n"));
        for line in &lines {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(&fence);
        out.push('\n');
        if let Some(tail) = tail {
            out.push_str(&tail);
            out.push('\n');
        }
    }
    out
}

/// The length of the longest run of backticks in `line`.
fn backtick_run(line: &str) -> usize {
    let (mut best, mut cur) = (0, 0);
    for c in line.chars() {
        cur = if c == '`' { cur + 1 } else { 0 };
        best = best.max(cur);
    }
    best
}

/// The marked lines of a line diff in place (a space, `+` or `-` before each line), with the
/// counts of inserted and deleted lines; without an old text every line has a space.
fn marked_diff(old: Option<&[String]>, new: &[String]) -> (Vec<String>, usize, usize) {
    let Some(old) = old else {
        return (new.iter().map(|l| format!(" {l}")).collect(), 0, 0);
    };
    let (o, n): (Vec<&str>, Vec<&str>) = (
        old.iter().map(String::as_str).collect(),
        new.iter().map(String::as_str).collect(),
    );
    let (mut shown, mut a, mut r) = (Vec::new(), 0, 0);
    for c in TextDiff::from_slices(&o, &n).iter_all_changes() {
        match c.tag() {
            ChangeTag::Insert => {
                a += 1;
                shown.push(format!("+{}", c.value()));
            }
            ChangeTag::Delete => {
                r += 1;
                shown.push(format!("-{}", c.value()));
            }
            ChangeTag::Equal => shown.push(format!(" {}", c.value())),
        }
    }
    (shown, a, r)
}

/// The stored text of the last confirmation of a waiting section.
fn old_text(p: &Prepared) -> Option<&str> {
    latest(&p.state, &p.section).and_then(|c| c.text.as_deref())
}

/// The Frame, Criteria and Decisions parts: heading, marked lines and the line after the fence.
fn marked_parts(waiting: &[Prepared]) -> Vec<(&'static str, Vec<String>, Option<String>)> {
    let (mut frame, mut decisions) = (Vec::new(), Vec::new());
    if let Some(p) = waiting
        .iter()
        .find(|p| p.section.eq_ignore_ascii_case("description"))
    {
        let old = old_text(p).map(split_description);
        let (f, d) = split_description(&p.text);
        let part = |old: Option<&String>, new: &String| {
            let old = old.map(|o| logical_lines(o, false).1);
            marked_diff(old.as_deref(), &logical_lines(new, false).1).0
        };
        frame = part(old.as_ref().map(|o| &o.0), &f);
        decisions = part(old.as_ref().map(|o| &o.1), &d);
    }
    let (mut criteria, mut added, mut removed) = (Vec::new(), 0, 0);
    for p in waiting.iter().filter(|p| p.section.eq_ignore_ascii_case("spec")) {
        let old = old_text(p);
        let (rows, other) = spec_lines(&p.text);
        let old_lines = old.map(spec_lines);
        let had_rows = old_lines.as_ref().is_some_and(|o| !o.0.is_empty());
        if rows.is_empty() && !had_rows {
            let old_other = old_lines.as_ref().map(|o| o.1.as_slice());
            criteria.extend(marked_diff(old_other, &other).0);
            continue;
        }
        let (shown, ..) = marked_diff(old_lines.as_ref().map(|o| o.0.as_slice()), &rows);
        criteria.extend(shown);
        match &old_lines {
            None => added += other.len(),
            Some(o) => {
                let (_, a, r) = marked_diff(Some(&o.1), &other);
                added += a;
                removed += r;
            }
        }
    }
    let count = (added + removed > 0).then(|| format!("spec, other lines: +{added} -{removed}"));
    vec![
        ("Frame", frame, None),
        ("Criteria", criteria, count),
        ("Decisions", decisions, None),
    ]
}

/// `sha256:` and the hex sha256 of the page bytes.
pub fn page_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hex: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("sha256:{hex}")
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
