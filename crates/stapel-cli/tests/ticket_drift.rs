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
fn test_refs(text: &str, files: &BTreeMap<String, PathBuf>) -> BTreeSet<(String, String)> {
    spans(text)
        .into_iter()
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

#[test]
fn ticket_test_names_exist_and_every_test_is_named() {
    let files = test_files();
    let mut named = BTreeSet::new();
    let mut problems = Vec::new();
    for (key, text) in tickets() {
        for (module, name) in test_refs(&text, &files) {
            if !defined_tests(&files[&module]).contains(&name) {
                problems.push(format!(
                    "{key} names {module}::{name}, which does not exist"
                ));
            }
            named.insert((module, name));
        }
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

#[test]
fn quoted_output_in_criteria_appears_in_the_code() {
    let code = code_text();
    let mut problems = Vec::new();
    for (key, text) in tickets() {
        for row in text.lines().filter(|l| l.starts_with("| AC-")) {
            for span in quoted_output(row) {
                let prefix = literal_prefix(span).trim_end();
                if prefix.len() >= 6 && !code.contains(prefix) {
                    let id = row.split('|').nth(1).unwrap_or("").trim();
                    problems.push(format!(
                        "{key} {id} quotes `{span}`; `{prefix}` is not in the code"
                    ));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "ticket drift:\n{}",
        problems.join("\n")
    );
}

/// Paths a ticket names inside the repository exist (`crates/...`, `docs/...`).
#[test]
fn named_repository_paths_exist() {
    let mut problems = Vec::new();
    for (key, text) in tickets() {
        for span in spans(&text) {
            let is_repo_path = (span.starts_with("crates/") || span.starts_with("docs/"))
                && !span.contains(['<', '*', ' ', '{']);
            if is_repo_path && !root().join(span.trim_end_matches('/')).exists() {
                problems.push(format!("{key} names `{span}`, which does not exist"));
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
