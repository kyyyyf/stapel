//! The brief of one Plan step (STP-21): the parts a builder reads, assembled from `ticket.md`.

use crate::next::step_label;
use crate::ticket::{Lookup, find, parse};

/// The brief of step `step` (`2`, `2c`) of ticket `key`, from the text of its `ticket.md`.
/// `plan_title` is the title of the configured `plan` section; `base` is the base of `state.json`.
pub fn assemble(
    text: &str,
    key: &str,
    step: &str,
    base: Option<&str>,
    plan_title: &str,
    findings: &str,
) -> Result<String, String> {
    let sections = parse(text);
    let plan = match find(&sections, plan_title) {
        Lookup::Found(body) => body,
        _ => return Err(format!("the Plan of {key} has no step {step}")),
    };
    let item = step_item(plan, key, step)?;

    let spec = match find(&sections, "Spec") {
        Lookup::Found(body) => body,
        _ => "",
    };
    let criteria = criteria_part(&item, spec, step)?;
    if named_criteria(&item).is_empty() && !names_a_finding(&item, findings) {
        return Err(format!("step {step} names no criterion and no finding"));
    }

    let mut out = format!("# Brief: {key} step {step}\n");
    out.push_str(&format!("ticket: .stapel/tickets/{key}/ticket.md\n"));
    out.push_str(&format!("base: {}\n", base.unwrap_or("none")));
    if !criteria.tests.is_empty() {
        out.push_str(&format!("tests: {}\n", criteria.tests.join(", ")));
    }
    if let Lookup::Found(description) = find(&sections, "Description") {
        let frame = frame_of(description);
        if !frame.is_empty() {
            out.push_str(&format!("\n## Frame\n{frame}\n"));
        }
    }
    out.push_str(&format!("\n## Step\n{item}\n"));
    if !criteria.text.is_empty() {
        out.push_str(&format!("\n## Criteria\n{}\n", criteria.text));
        let terms = terms_part(spec, &criteria.text);
        if !terms.is_empty() {
            out.push_str(&format!("\n## Terms\n{terms}\n"));
        }
    }
    Ok(out)
}

/// The finding prefix of an id: the part before its last `-<digits>`.
fn finding_split(word: &str) -> Option<(&str, &str)> {
    let (prefix, digits) = word.rsplit_once('-')?;
    (!prefix.is_empty() && !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then_some((prefix, digits))
}

/// Whether the step item has a word `<prefix>-<digits>` (not `AC`) whose prefix is the finding
/// prefix of a well-formed record (a JSON line with a string `id`) of `findings`.
fn names_a_finding(item: &str, findings: &str) -> bool {
    let prefixes: Vec<String> = findings
        .lines()
        .filter_map(|l| {
            let value: serde_json::Value = serde_json::from_str(l).ok()?;
            let id = value.get("id")?.as_str()?;
            finding_split(id).map(|(p, _)| p.to_string())
        })
        .collect();
    item.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .filter_map(finding_split)
        .any(|(p, _)| p != "AC" && prefixes.iter().any(|q| q == p))
}

/// One term of the Spec: its lines, and its name and text in the normal form of a match.
struct Term {
    first_line: usize,
    last_line: usize,
    name: String,
    text: String,
}

/// The text with every run of whitespace as one space and ASCII letters in lower case.
fn normal(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// Whether `name` occurs in `text` as whole words, the last word also with `s` or `es` added.
fn uses(text: &str, name: &str) -> bool {
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(i) = text[from..].find(name) {
        let at = from + i;
        let end = at + name.len();
        if (at == 0 || !word(bytes[at - 1])) && !name.is_empty() {
            let boundary = |e: usize| bytes.get(e).is_none_or(|b| !word(*b));
            if boundary(end)
                || (text[end..].starts_with('s') && boundary(end + 1))
                || (text[end..].starts_with("es") && boundary(end + 2))
            {
                return true;
            }
        }
        from = at + 1;
        while !text.is_char_boundary(from) {
            from += 1;
        }
    }
    false
}

/// The terms of the Spec: each `**<name>**:` of the list between the line `Terms:` and the first
/// table line, with its text up to the next such name.
fn spec_terms(lines: &[&str]) -> Vec<Term> {
    let Some(start) = lines.iter().position(|l| l.trim() == "Terms:") else {
        return Vec::new();
    };
    let end = (start + 1..lines.len())
        .find(|&i| lines[i].starts_with('|'))
        .unwrap_or(lines.len());
    // Each name with its line, the start of its text and the start of its mark.
    let mut marks: Vec<(usize, String, usize, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate().take(end).skip(start + 1) {
        let mut at = 0;
        while let Some(open) = line[at..].find("**").map(|p| at + p) {
            let Some(close) = line[open + 2..].find("**").map(|p| open + 2 + p) else {
                break;
            };
            let name = &line[open + 2..close];
            if line[close + 2..].starts_with(':') && !name.trim().is_empty() {
                marks.push((i, normal(name), close + 3, open));
                at = close + 3;
            } else {
                at = close + 2;
            }
        }
    }
    let mut terms = Vec::new();
    for (k, (line, name, text_from, _)) in marks.iter().enumerate() {
        let mut text = String::new();
        let (last_line, text_to_line, text_to) = match marks.get(k + 1) {
            Some((l, _, _, open)) => (if l == line { *line } else { l - 1 }, *l, *open),
            None => (end - 1, end - 1, lines[end - 1].len()),
        };
        for (i, l) in lines.iter().enumerate().take(text_to_line + 1).skip(*line) {
            let from = if i == *line { *text_from } else { 0 };
            let to = if i == text_to_line { text_to } else { l.len() };
            text.push_str(&l[from.min(to)..to]);
            text.push(' ');
        }
        let mut last = last_line;
        while last > *line && lines[last].trim().is_empty() {
            last -= 1;
        }
        terms.push(Term {
            first_line: *line,
            last_line: last,
            name: name.clone(),
            text: normal(&text),
        });
    }
    terms
}

/// The Terms part: the terms the printed criterion rows use, and the terms those terms use,
/// printed as written in the Spec, in Spec order, each line once; empty when no term is used.
fn terms_part(spec: &str, rows: &str) -> String {
    let lines: Vec<&str> = spec.lines().collect();
    let terms = spec_terms(&lines);
    let mut used = vec![false; terms.len()];
    let mut texts = vec![normal(rows)];
    while let Some(text) = texts.pop() {
        for (i, term) in terms.iter().enumerate() {
            if !used[i] && uses(&text, &term.name) {
                used[i] = true;
                texts.push(term.text.clone());
            }
        }
    }
    let mut printed = vec![false; lines.len()];
    for (term, _) in terms.iter().zip(&used).filter(|(_, u)| **u) {
        for flag in &mut printed[term.first_line..=term.last_line] {
            *flag = true;
        }
    }
    let kept: Vec<&str> = lines
        .iter()
        .zip(&printed)
        .filter(|(_, p)| **p)
        .map(|(l, _)| *l)
        .collect();
    kept.join("\n")
}

/// The `## Criteria` text of a step and its step tests.
struct Criteria {
    text: String,
    tests: Vec<String>,
}

/// The `n` of a Spec line that starts with `| AC-<n> |`.
fn row_number(line: &str) -> Option<u64> {
    let rest = line.strip_prefix("| AC-")?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || !rest[digits..].starts_with(" |") {
        return None;
    }
    rest[..digits].parse().ok()
}

/// The named criteria of a step item as inclusive ranges in the order of the item. Every run of
/// whitespace counts as one space. A range with n > m is kept as it is.
fn named_criteria(item: &str) -> Vec<(u64, u64)> {
    let text = item.split_whitespace().collect::<Vec<_>>().join(" ");
    let bytes = text.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    // `AC-<digits>` as a whole word at `at`: the number and the end of the id.
    let id_at = |at: usize| -> Option<(u64, usize)> {
        if at > 0 && word(bytes[at - 1]) {
            return None;
        }
        let rest = text.get(at..)?.strip_prefix("AC-")?;
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let end = at + 3 + digits;
        if digits == 0 || bytes.get(end).is_some_and(|b| word(*b)) {
            return None;
        }
        Some((rest[..digits].parse().ok()?, end))
    };
    let mut named = Vec::new();
    let mut at = 0;
    while at < text.len() {
        if let Some((n, end)) = id_at(at) {
            let mut range = (n, n);
            let mut next = end;
            let upper = if text[end..].starts_with(" to ") {
                id_at(end + 4)
            } else {
                None
            };
            if let Some((m, end2)) = upper {
                range = (n, m);
                next = end2;
            }
            named.push(range);
            at = next;
        } else {
            at += 1;
        }
    }
    named
}

/// The Criteria part and the step tests for the criteria the step item names; empty when none.
fn criteria_part(item: &str, spec: &str, step: &str) -> Result<Criteria, String> {
    let named = named_criteria(item);
    if named.is_empty() {
        return Ok(Criteria {
            text: String::new(),
            tests: Vec::new(),
        });
    }
    let lines: Vec<&str> = spec.lines().collect();
    let rows: Vec<(u64, &str)> = lines
        .iter()
        .filter_map(|l| row_number(l).map(|n| (n, *l)))
        .collect();
    for &(n, m) in &named {
        if n > m {
            return Err(format!(
                "step {step} names AC-{n} to AC-{m}, which is not a range"
            ));
        }
        let mut i = n;
        while i <= m {
            if !rows.iter().any(|(r, _)| *r == i) {
                return Err(format!(
                    "step {step} names AC-{i}, which the Spec does not have"
                ));
            }
            i += 1;
        }
    }
    let is_separator = |l: &str| {
        l.starts_with('|')
            && l.contains('-')
            && l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
    };
    let mut out: Vec<&str> = Vec::new();
    // The table that holds the criterion rows: the block of `|` lines around the first row.
    if let Some(first) = lines.iter().position(|l| row_number(l).is_some()) {
        let mut top = first;
        while top > 0 && lines[top - 1].starts_with('|') {
            top -= 1;
        }
        if let Some(sep) = (top..first).find(|&i| is_separator(lines[i])) {
            if sep > top {
                out.push(lines[sep - 1]);
            }
            out.push(lines[sep]);
        }
    }
    let mut seen: Vec<u64> = Vec::new();
    let mut tests: Vec<String> = Vec::new();
    for (n, line) in &rows {
        if seen.contains(n) || !named.iter().any(|&(a, b)| a <= *n && *n <= b) {
            continue;
        }
        seen.push(*n);
        out.push(line);
        let cell = line.trim_end().trim_end_matches('|').rsplit('|').next();
        for (k, part) in cell.unwrap_or("").split('`').enumerate() {
            let name = part.trim();
            if k % 2 == 1 && name.contains("::") && !tests.iter().any(|t| t == name) {
                tests.push(name.to_string());
            }
        }
    }
    Ok(Criteria {
        text: out.join("\n"),
        tests,
    })
}

/// The step line of the label `step <step>` and the lines after it up to the next step line, a
/// blank line or the end of the Plan.
fn step_item(plan: &str, key: &str, step: &str) -> Result<String, String> {
    let label = format!("step {step}");
    let lines: Vec<&str> = plan.lines().collect();
    let starts: Vec<usize> = (0..lines.len())
        .filter(|&i| step_label(lines[i]).as_deref() == Some(label.as_str()))
        .collect();
    let start = match starts.as_slice() {
        [one] => *one,
        [] => return Err(format!("the Plan of {key} has no step {step}")),
        _ => return Err(format!("the Plan of {key} names step {step} twice")),
    };
    let item: Vec<&str> = std::iter::once(lines[start])
        .chain(
            lines[start + 1..]
                .iter()
                .copied()
                .take_while(|l| !l.trim().is_empty() && step_label(l).is_none()),
        )
        .collect();
    Ok(item.join("\n"))
}

/// Whether the line is an ATX heading.
fn is_heading(line: &str) -> bool {
    let rest = line.trim_start_matches(' ');
    let hashes = rest.bytes().take_while(|b| *b == b'#').count();
    line.len() - rest.len() <= 3
        && (1..=6).contains(&hashes)
        && rest[hashes..].starts_with([' ', '\t'])
}

/// The lines of the `### Frame` subsection of a Description up to the next heading, without each
/// paragraph that starts with `**Prior art.**` or `**Size.**`, trimmed; empty when there is none.
fn frame_of(description: &str) -> String {
    let mut lines = description.lines().skip_while(|l| {
        !(is_heading(l)
            && l.trim()
                .trim_start_matches('#')
                .trim()
                .eq_ignore_ascii_case("frame"))
    });
    lines.next();
    let mut kept: Vec<&str> = Vec::new();
    let mut skipping = false;
    let mut paragraph_start = true;
    for line in lines {
        if is_heading(line) {
            break;
        }
        if line.trim().is_empty() {
            paragraph_start = true;
            if skipping {
                skipping = false;
                continue;
            }
        } else {
            if paragraph_start
                && (line.starts_with("**Prior art.**") || line.starts_with("**Size.**"))
            {
                skipping = true;
            }
            paragraph_start = false;
        }
        if !skipping {
            kept.push(line);
        }
    }
    kept.join("\n").trim().to_string()
}
