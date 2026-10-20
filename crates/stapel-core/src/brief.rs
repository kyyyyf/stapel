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

    let mut out = format!("# Brief: {key} step {step}\n");
    out.push_str(&format!("ticket: .stapel/tickets/{key}/ticket.md\n"));
    out.push_str(&format!("base: {}\n", base.unwrap_or("none")));
    if let Lookup::Found(description) = find(&sections, "Description") {
        let frame = frame_of(description);
        if !frame.is_empty() {
            out.push_str(&format!("\n## Frame\n{frame}\n"));
        }
    }
    out.push_str(&format!("\n## Step\n{item}\n"));
    Ok(out)
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
