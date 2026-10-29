//! The pure parts of `stapel review start` (STP-24): the criteria of a Spec, the patch findings of
//! round 1 and the text of `pack.md`.

use crate::brief::{cells, row_number};
use crate::triage::{state_of, whole_round};
use serde_json::{Map, Value};

/// The criterion text of every Spec line that starts with `| AC-<n> |`, as `AC-<n>: <text>`.
pub fn criteria(spec: &str) -> Vec<String> {
    spec.lines()
        .filter_map(|line| {
            let n = row_number(line)?;
            let text = cells(line).get(1).copied().unwrap_or("");
            Some(format!("AC-{n}: {text}"))
        })
        .collect()
}

/// A patch finding as the pack lists it: id, current severity and claim.
pub struct PatchFinding {
    pub id: String,
    pub severity: String,
    pub claim: String,
}

fn text<'a>(m: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    m.get(name).and_then(Value::as_str)
}

/// The finding records with stage `code-review` and whole round 1 whose current state has the fate
/// `patch` and is not false, in the file order of their first record.
pub fn patch_findings(records: &[Map<String, Value>]) -> Vec<PatchFinding> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for m in records.iter().filter(|m| !m.contains_key("kind")) {
        let Some(id) = text(m, "id") else { continue };
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let state = state_of(records, id);
        let Some(finding) = &state.finding else {
            continue;
        };
        let is_round_one = text(finding, "stage") == Some("code-review")
            && finding.get("round").and_then(whole_round) == Some(1);
        if !is_round_one
            || state.fate.as_deref() != Some("patch")
            || state.verdict.as_deref() == Some("false")
        {
            continue;
        }
        out.push(PatchFinding {
            id: id.to_string(),
            severity: state.severity.clone().unwrap_or_default(),
            claim: text(finding, "claim").unwrap_or("").to_string(),
        });
    }
    out
}

/// What `pack.md` is made of. `folder` is the review folder, relative to the repository root, with
/// no trailing slash; `findings` is `Some` in round 2.
pub struct Pack<'a> {
    pub key: &'a str,
    pub round: u64,
    pub base: &'a str,
    pub head: &'a str,
    pub folder: &'a str,
    pub stat: &'a str,
    pub criteria: &'a [String],
    pub findings: Option<&'a [PatchFinding]>,
}

pub fn pack_text(p: &Pack) -> String {
    let mut out = format!(
        "# Review pack {key}, round {round}\n\n- ticket: {key}\n- round: {round}\n- base: {base}\n\
         - head: {head}\n\n## Paths\n\n- copy: {folder}/tree\n\
         - ticket: {folder}/tree/.stapel/tickets/{key}/ticket.md\n- diff: {folder}/diff.patch\n\n\
         ## Stat\n\n```\n{stat}\n```\n\n## Criteria\n\n",
        key = p.key,
        round = p.round,
        base = p.base,
        head = p.head,
        folder = p.folder,
        stat = p.stat.trim_end(),
    );
    for c in p.criteria {
        out.push_str(&format!("- {c}\n"));
    }
    if let Some(findings) = p.findings {
        out.push_str("\n## Findings of round 1 with the fate patch\n\n");
        if findings.is_empty() {
            out.push_str("- none\n");
        }
        for f in findings {
            out.push_str(&format!("- {} ({}): {}\n", f.id, f.severity, f.claim));
        }
    }
    out.push_str(
        "\n## Checks\n\n\
- Deletion check: for each removed line or file, say what it guaranteed and what replaces or retires it.\n\
- Claims check: try to falsify each checkable claim of the criteria and of Design \"as built\" against the code.\n\
- Verification gap: for each behaviour change, say whether an existing test would fail if the change were wrong.\n",
    );
    out
}

const KNOWN_TAGS: [&str; 5] = ["security", "guard", "data", "migration", "interface"];

/// The known risk tags in the backtick spans of the paragraph of the Frame that starts with
/// `**Risk tags:**`. The paragraph ends at a blank line, a heading line or a line that starts a
/// new bold label (`**`).
pub fn frame_tags(description: &str) -> Vec<&'static str> {
    let lines = description
        .lines()
        .skip_while(|l| l.trim_end() != "### Frame")
        .skip(1)
        .skip_while(|l| !l.starts_with("**Risk tags:**"));
    let mut tags = Vec::new();
    let mut first = true;
    for line in lines {
        if !first && (line.trim().is_empty() || line.starts_with('#') || line.starts_with("**")) {
            break;
        }
        first = false;
        for (i, span) in line.split('`').enumerate() {
            if i % 2 == 1
                && let Some(tag) = KNOWN_TAGS.iter().find(|t| **t == span)
                && !tags.contains(tag)
            {
                tags.push(*tag);
            }
        }
    }
    tags
}

/// One reviewer of a code review round: the agent, the role of `[models]` and whether it reads
/// the whole ticket.
#[derive(Debug, PartialEq)]
pub struct Reviewer {
    pub agent: &'static str,
    pub role: &'static str,
    pub whole_ticket: bool,
}

/// The reviewers of a round: one for a light or untagged ticket, three for a tagged one.
pub fn reviewers(light: bool, tags: &[&str]) -> Vec<Reviewer> {
    let mut out = vec![Reviewer {
        agent: "stapel-reviewer",
        role: "reviewer",
        whole_ticket: false,
    }];
    if !light && !tags.is_empty() {
        out.push(Reviewer {
            agent: "stapel-reviewer",
            role: "external",
            whole_ticket: false,
        });
        out.push(Reviewer {
            agent: "stapel-drift",
            role: "drift",
            whole_ticket: true,
        });
    }
    out
}
