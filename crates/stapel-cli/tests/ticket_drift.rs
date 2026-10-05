//! Process check (CLAUDE.md, "Before the build" item 9): every ticket's text and the code agree.
//!
//! A ticket names tests as `module::test_name` and quotes output text in backticks. This test fails
//! when a named test does not exist, when a test exists that no ticket names, or when a quoted
//! output line in an acceptance criterion appears nowhere in the code. It runs with every
//! `cargo test`, so drift is found at the commit that causes it, not at review.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Test files by the module name tickets use: `core::config` for the core's config tests, the file
/// stem for the rest.
fn test_files() -> BTreeMap<String, PathBuf> {
    let mut files = BTreeMap::new();
    for (krate, prefix) in [("stapel-core", "core::"), ("stapel-cli", "")] {
        let dir = root().join("crates").join(krate).join("tests");
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "rs") {
                let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
                let name = if stem == "config" && prefix == "core::" {
                    "core::config".to_string()
                } else {
                    stem
                };
                files.insert(name, path);
            }
        }
    }
    files
}

fn tickets() -> Vec<(String, String)> {
    let dir = root().join(".stapel/tickets");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path().join("ticket.md")).ok()?;
            Some((e.file_name().to_string_lossy().into_owned(), text))
        })
        .collect();
    out.sort();
    out
}

/// Backticked spans of a text.
fn spans(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// `module::name` references whose module is a known test file.
/// `module::name` references in table rows only: prose such as "`ok` calls `journal::append`"
/// names code, not tests (STP-3 code review E-1).
fn test_refs(text: &str, files: &BTreeMap<String, PathBuf>) -> BTreeSet<(String, String)> {
    text.lines()
        .filter(|l| l.starts_with('|'))
        .flat_map(spans)
        .filter_map(|span| {
            let (module, name) = span.rsplit_once("::")?;
            let ok = !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            (ok && files.contains_key(module)).then(|| (module.to_string(), name.to_string()))
        })
        .collect()
}

/// Test functions defined in a file: `#[test]` functions and functions inside `proptest!`.
fn defined_tests(path: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(path).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let mut names = BTreeSet::new();
    let mut in_proptest = false;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with("proptest!") {
            in_proptest = true;
        }
        let Some(rest) = t.strip_prefix("fn ") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let attributed = lines[..i]
            .iter()
            .rev()
            .take_while(|l| l.trim().starts_with("#[") || l.trim().starts_with("//"))
            .any(|l| l.trim() == "#[test]");
        let property = in_proptest && rest.contains(" in ");
        if attributed || property {
            names.insert(name);
        }
    }
    names
}

/// All Rust source and golden output under `crates/`, as one text to search.
fn code_text() -> String {
    let mut text = String::new();
    let mut stack = vec![root().join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n == "ticket_drift.rs") {
                // This file quotes wrong lines on purpose.
            } else if path
                .extension()
                .is_some_and(|e| e == "rs" || e == "txt" || e == "toml")
            {
                text.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
                text.push('\n');
            }
        }
    }
    text
}

/// The literal part of a quoted output line before its first placeholder (`<...>`).
fn literal_prefix(span: &str) -> &str {
    span.split('<').next().unwrap_or(span)
}

/// Quoted output lines in acceptance criteria: backticked spans that look like what a command
/// prints, `label: text`, so they can be found in the code.
fn quoted_output(row: &str) -> Vec<&str> {
    spans(row)
        .into_iter()
        .filter(|s| {
            // `{...}` and quotes describe a data structure, not a printed line.
            if s.contains(['{', '"']) {
                return false;
            }
            let Some((label, _)) = s.split_once(": ") else {
                return false;
            };
            !label.is_empty()
                && label
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == ' ' || c == '.')
        })
        .collect()
}

/// Whether a ticket is finished (fully checked) or still being built (STP-3 AC-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Open,
    Closed,
}

fn lifecycle(key: &str) -> Lifecycle {
    let path = root().join(".stapel/tickets").join(key).join("state.json");
    match stapel_core::state::load(&path) {
        stapel_core::state::Loaded::State(s) if s.closed.is_none() => Lifecycle::Open,
        // Closed, legacy (STP-1) and unreadable tickets are held to the full check.
        _ => Lifecycle::Closed,
    }
}

fn repo_paths(text: &str) -> Vec<&str> {
    spans(text)
        .into_iter()
        .filter(|s| {
            (s.starts_with("crates/") || s.starts_with("docs/"))
                && !s.contains(['<', '*', ' ', '{'])
        })
        .collect()
}

fn missing_test(files: &BTreeMap<String, PathBuf>, module: &str, name: &str) -> bool {
    !defined_tests(&files[module]).contains(name)
}

/// The drift of one ticket. A closed ticket is checked fully; an open ticket checks each acceptance
/// criterion once every test it names exists, since until then the criterion is a plan.
fn ticket_problems(
    key: &str,
    text: &str,
    life: Lifecycle,
    files: &BTreeMap<String, PathBuf>,
    code: &str,
) -> Vec<String> {
    let mut problems = Vec::new();
    // An open ticket whose every criterion is built is checked as if closed, so what the close
    // would find shows before it.
    let ac_rows: Vec<&str> = text.lines().filter(|l| l.starts_with("| AC-")).collect();
    let all_built = !ac_rows.is_empty()
        && ac_rows.iter().all(|row| {
            let refs = test_refs(row, files);
            !refs.is_empty() && refs.iter().all(|(m, n)| !missing_test(files, m, n))
        });
    let life = if all_built { Lifecycle::Closed } else { life };
    for row in text.lines().filter(|l| l.starts_with("| AC-")) {
        let id = row.split('|').nth(1).unwrap_or("").trim();
        let refs = test_refs(row, files);
        let built = !refs.is_empty() && refs.iter().all(|(m, n)| !missing_test(files, m, n));
        if life == Lifecycle::Open && !built {
            continue;
        }
        for span in quoted_output(row) {
            let prefix = literal_prefix(span).trim_end();
            if prefix.len() >= 6 && !code.contains(prefix) {
                problems.push(format!(
                    "{key} {id} quotes `{span}`; `{prefix}` is not in the code"
                ));
            }
        }
        if life == Lifecycle::Open {
            for path in repo_paths(row) {
                if !root().join(path.trim_end_matches('/')).exists() {
                    problems.push(format!("{key} {id} names `{path}`, which does not exist"));
                }
            }
        }
    }
    if life == Lifecycle::Closed {
        for (module, name) in test_refs(text, files) {
            if missing_test(files, &module, &name) {
                problems.push(format!(
                    "{key} names {module}::{name}, which does not exist"
                ));
            }
        }
        for path in repo_paths(text) {
            if !root().join(path.trim_end_matches('/')).exists() {
                problems.push(format!("{key} names `{path}`, which does not exist"));
            }
        }
        // In the Proof table every `module::name` is a test.
        if let Some(start) = text.find("## Proof") {
            let rest = &text[start + 1..];
            let proof = &rest[..rest.find("\n## ").unwrap_or(rest.len())];
            for row in proof
                .lines()
                .filter(|l| l.starts_with("| AC-") || l.starts_with("| R-"))
            {
                for span in spans(row) {
                    let Some((module, name)) = span.rsplit_once("::") else {
                        continue;
                    };
                    let looks_like_test = name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                        && (module == "core::config"
                            || module.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
                    if looks_like_test && !files.contains_key(module) {
                        problems.push(format!(
                            "{key} names module {module}, which is not a test file"
                        ));
                    }
                }
            }
        }
    }
    problems
}

#[test]
fn ticket_test_names_exist_and_every_test_is_named() {
    let files = test_files();
    let code = code_text();
    let mut named = BTreeSet::new();
    let mut problems = Vec::new();
    for (key, text) in tickets() {
        problems.extend(ticket_problems(&key, &text, lifecycle(&key), &files, &code));
        named.extend(test_refs(&text, &files));
    }
    for (module, path) in &files {
        if module == "ticket_drift" {
            continue;
        }
        for name in defined_tests(path) {
            if !named.contains(&(module.clone(), name.clone())) {
                problems.push(format!("{module}::{name} is not named by any ticket"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "ticket drift:\n{}",
        problems.join("\n")
    );
}

/// The checker itself: a quoted line that is in the code passes, one that is not is found.
#[test]
fn checker_finds_a_wrong_quote() {
    let row = "| AC-9 | prints `waiting for: <id>` and `stale: x` and `nowhere to be found: <x>` and `closed: {by}` |";
    let found = quoted_output(row);
    assert_eq!(
        found,
        ["waiting for: <id>", "stale: x", "nowhere to be found: <x>"]
    );
    let code = code_text();
    assert!(code.contains(literal_prefix(found[0]).trim_end()));
    assert!(!code.contains(literal_prefix(found[2]).trim_end()));
}

/// STP-3 AC-6: an open ticket's criterion is checked once all tests it names exist; a closed ticket
/// is checked fully.
#[test]
fn open_tickets_are_plans() {
    let files = test_files();
    let code = code_text();
    let ticket = "\
| AC-1 | prints `planned output: <x>` | `ticket_drift::not_written_yet` |
| AC-2 | prints `another missing line: <y>` | `ticket_drift::checker_finds_a_wrong_quote` |
";
    let open = ticket_problems("T-1", ticket, Lifecycle::Open, &files, &code);
    assert_eq!(open.len(), 1, "{open:?}");
    assert!(
        open[0].contains("AC-2") && open[0].contains("another missing line:"),
        "{open:?}"
    );

    let closed = ticket_problems("T-1", ticket, Lifecycle::Closed, &files, &code);
    assert!(
        closed.iter().any(|p| p.contains("not_written_yet")),
        "{closed:?}"
    );
    assert!(
        closed.iter().any(|p| p.contains("planned output:")),
        "{closed:?}"
    );
    assert!(
        closed.iter().any(|p| p.contains("another missing line:")),
        "{closed:?}"
    );

    // In a closed ticket's Proof table every `a::b` is a test; an unknown module is drift.
    let proof = "## Proof\n\n| AC-1 | `nosuchmodule::some_test` |\n\n## Plan\n";
    let closed = ticket_problems("T-1", proof, Lifecycle::Closed, &files, &code);
    assert!(
        closed.iter().any(|p| p.contains("nosuchmodule")),
        "{closed:?}"
    );
}

/// STP-3 code review E-1: `module::name` in prose is not a test name; only table rows name tests.
#[test]
fn prose_mentions_are_not_test_names() {
    let files = test_files();
    let code = code_text();
    let prose = "## Design\n\n- `ok` calls `journal::append` after writing state.\n";
    assert!(ticket_problems("T-1", prose, Lifecycle::Closed, &files, &code).is_empty());
    let table = "| R-1 | risk | `journal::append` |\n";
    let problems = ticket_problems("T-1", table, Lifecycle::Closed, &files, &code);
    assert!(
        problems.iter().any(|p| p.contains("journal::append")),
        "{problems:?}"
    );
}

/// An open ticket whose every criterion is built is checked like a closed one, so a problem that
/// would appear at the close shows before it.
#[test]
fn fully_built_open_ticket_is_checked_fully() {
    let files = test_files();
    let code = code_text();
    let ticket = "| AC-1 | x | `ticket_drift::checker_finds_a_wrong_quote` |\n\n| R-1 | risk | `ticket_drift::not_written` |\n";
    let open = ticket_problems("T-1", ticket, Lifecycle::Open, &files, &code);
    assert!(open.iter().any(|p| p.contains("not_written")), "{open:?}");
}
