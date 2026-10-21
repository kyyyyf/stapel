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
    }
    Ok(out)
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
