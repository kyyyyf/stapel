//! Process check (CLAUDE.md, "Before the build" item 9): every ticket's text and the code agree.
//!
//! A ticket names tests as `module::test_name` and quotes output text in backticks. This test fails
//! when a named test does not exist, when a test exists that no ticket names, or when a quoted
//! output line in an acceptance criterion appears nowhere in the code. It runs with every
//! `cargo test`, so drift is found at the commit that causes it, not at review.

// The STP-4 step 1 RED test passes a cloned String slice; a GREEN commit may not change a RED test.
#![allow(clippy::cloned_ref_to_slice_refs)]

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

/// Test functions defined in a file, by the same lexer as `stapel check` (STP-4 "Step test"), so a
/// test inside a string literal is not one.
fn defined_tests(path: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(path).unwrap();
    stapel_core::rust_tests::test_functions(&text)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
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

/// An open ticket whose every criterion is built is checked as if closed, so what the close would
/// find shows before it.
fn effective_lifecycle(
    text: &str,
    life: Lifecycle,
    files: &BTreeMap<String, PathBuf>,
) -> Lifecycle {
    let ac_rows: Vec<&str> = text.lines().filter(|l| l.starts_with("| AC-")).collect();
    let all_built = !ac_rows.is_empty()
        && ac_rows.iter().all(|row| {
            let refs = test_refs(row, files);
            !refs.is_empty() && refs.iter().all(|(m, n)| !missing_test(files, m, n))
        });
    if all_built { Lifecycle::Closed } else { life }
}

/// Tickets written before the rules of STP-4 AC-8; they keep their shape.
const EARLIER_TICKETS: [&str; 3] = ["STP-1", "STP-2", "STP-3"];

/// Options that existed before STP-4 AC-8 required an Inputs table row for each.
const OLDER_OPTIONS: [&str; 14] = [
    "--cache-read",
    "--cache-write",
    "--estimate",
    "--input",
    "--model",
    "--note",
    "--output",
    "--prefix",
    "--reason",
    "--role",
    "--since",
    "--step",
    "--tracker",
    "--until",
];

/// The body of a level-2 section, up to the next level-2 heading.
fn section<'a>(text: &'a str, title: &str) -> &'a str {
    let head = format!("## {title}\n");
    let Some(start) = text
        .match_indices(&head)
        .find(|(i, _)| *i == 0 || text.as_bytes()[i - 1] == b'\n')
        .map(|(i, _)| i + head.len())
    else {
        return "";
    };
    let rest = &text[start..];
    &rest[..rest.find("\n## ").map(|i| i + 1).unwrap_or(rest.len())]
}

/// STP-4 AC-8: risk tags, and for `guard` or `security` an abuse table and a dated self-check.
fn process_problems(key: &str, text: &str) -> Vec<String> {
    if EARLIER_TICKETS.contains(&key) {
        return Vec::new();
    }
    let Some(tags_line) = text.lines().find(|l| l.starts_with("**Risk tags:**")) else {
        return vec![format!("{key} has no risk tags line (`**Risk tags:**`)")];
    };
    let tags = spans(tags_line);
    if !tags.iter().any(|t| *t == "guard" || *t == "security") {
        return Vec::new();
    }
    let mut problems = Vec::new();
    if !text.lines().any(|l| l.starts_with("### Abuse")) {
        problems.push(format!(
            "{key} is tagged guard or security but has no abuse table"
        ));
    }
    let dated = |l: &str| {
        l.as_bytes().windows(10).any(|w| {
            w.iter().enumerate().all(|(i, c)| {
                if i == 4 || i == 7 {
                    *c == b'-'
                } else {
                    c.is_ascii_digit()
                }
            })
        })
    };
    let done = text
        .lines()
        .any(|l| l.starts_with("**Author self-check") && dated(l) && !l.contains("To be done"));
    if !done {
        problems.push(format!(
            "{key} is tagged guard or security but has no dated author self-check"
        ));
    }
    problems
}

/// STP-4 AC-8: the tests of criteria and risks equal those of the Proof and are all in the Test plan.
fn list_problems(key: &str, text: &str, files: &BTreeMap<String, PathBuf>) -> Vec<String> {
    if EARLIER_TICKETS.contains(&key) {
        return Vec::new();
    }
    let rows = |body: &str, prefix: &str| -> String {
        body.lines()
            .filter(|l| l.starts_with(prefix))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut named = test_refs(&rows(section(text, "Spec"), "| AC-"), files);
    named.extend(test_refs(&rows(section(text, "Design"), "| R-"), files));
    let proof = test_refs(section(text, "Proof"), files);
    let plan = test_refs(section(text, "Test plan"), files);
    let mut problems = Vec::new();
    for (m, n) in named.difference(&proof) {
        problems.push(format!(
            "{key} names {m}::{n} in its criteria or risks but not in its Proof"
        ));
    }
    for (m, n) in proof.difference(&named) {
        problems.push(format!(
            "{key} names {m}::{n} in its Proof but not in its criteria or risks"
        ));
    }
    for (m, n) in named.difference(&plan) {
        problems.push(format!(
            "{key} names {m}::{n} in its criteria or risks but not in its Test plan"
        ));
    }
    problems
}

/// Every long option of every `stapel` subcommand, from `--help`.
fn stapel_options() -> BTreeSet<String> {
    fn help(path: &[String]) -> String {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_stapel"))
            .args(path)
            .arg("--help")
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
    let mut options = BTreeSet::new();
    let mut queue: Vec<Vec<String>> = vec![Vec::new()];
    while let Some(path) = queue.pop() {
        let text = help(&path);
        let mut in_commands = false;
        for line in text.lines() {
            if line.starts_with("Commands:") {
                in_commands = true;
                continue;
            }
            if in_commands {
                let Some(name) = line
                    .strip_prefix("  ")
                    .and_then(|l| l.split_whitespace().next())
                else {
                    in_commands = false;
                    continue;
                };
                if name != "help" {
                    let mut sub = path.clone();
                    sub.push(name.to_string());
                    queue.push(sub);
                }
            }
        }
        for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
            let ok = word.len() > 2
                && word.starts_with("--")
                && word[2..].starts_with(|c: char| c.is_ascii_lowercase());
            if ok && word != "--help" && word != "--version" {
                options.insert(word.to_string());
            }
        }
    }
    options
}

/// Options named in no ticket's Inputs table and not older than the rule.
fn options_missing(options: &BTreeSet<String>, tickets: &[String]) -> Vec<String> {
    let mut named = BTreeSet::new();
    for text in tickets {
        // Every `### Inputs…` section, up to the next heading of level 2 or 3; table rows only.
        let mut in_inputs = false;
        for line in text.lines() {
            if line.starts_with("## ") || line.starts_with("### ") {
                in_inputs = line.starts_with("### Inputs");
                continue;
            }
            if in_inputs && line.starts_with('|') {
                for span in spans(line) {
                    let word = span.split([' ', '=']).next().unwrap_or("");
                    named.insert(word.to_string());
                }
            }
        }
    }
    options
        .iter()
        .filter(|o| !OLDER_OPTIONS.contains(&o.as_str()) && !named.contains(*o))
        .cloned()
        .collect()
}

/// The drift of one ticket with the STP-4 AC-8 rules, which apply once it is built or closed.
fn full_problems(
    key: &str,
    text: &str,
    life: Lifecycle,
    files: &BTreeMap<String, PathBuf>,
    code: &str,
) -> Vec<String> {
    let mut problems = ticket_problems(key, text, life, files, code);
    if effective_lifecycle(text, life, files) == Lifecycle::Closed {
        problems.extend(process_problems(key, text));
        problems.extend(list_problems(key, text, files));
    }
    problems
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
    let life = effective_lifecycle(text, life, files);
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
        problems.extend(full_problems(&key, &text, lifecycle(&key), &files, &code));
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

/// STP-4 AC-8: a guard or security ticket needs an abuse table and a dated self-check; every ticket
/// needs a risk tags line. Earlier tickets are exempt.
#[test]
fn abuse_table_and_self_check_are_required() {
    let tagged = "## Description\n\n**Risk tags:** `guard` (x), `data`.\n\n## Test plan\n";
    let p = process_problems("T-9", tagged);
    assert!(p.iter().any(|x| x.contains("abuse table")), "{p:?}");
    assert!(p.iter().any(|x| x.contains("self-check")), "{p:?}");

    let placeholder = format!(
        "{tagged}\n### Abuse table\n\n**Author self-check (item 7).** To be done 2026-10-05.\n"
    );
    let p = process_problems("T-9", &placeholder);
    assert!(p.iter().any(|x| x.contains("self-check")), "{p:?}");
    assert!(!p.iter().any(|x| x.contains("abuse table")), "{p:?}");

    let done = format!(
        "{tagged}\n### Abuse table\n\n**Author self-check (item 7).** Done on 2026-10-05: nothing new.\n"
    );
    assert!(process_problems("T-9", &done).is_empty());

    let untagged = "## Description\n\n**Risk tags:** `data`.\n";
    assert!(process_problems("T-9", untagged).is_empty());

    let no_tags = "## Description\n\nNo tags here.\n";
    let p = process_problems("T-9", no_tags);
    assert!(p.iter().any(|x| x.contains("risk tags")), "{p:?}");

    assert!(process_problems("STP-2", no_tags).is_empty());
}

/// STP-4 AC-8: the tests of criteria and risks equal those of the Proof, and all are in the Test plan.
#[test]
fn test_lists_must_agree() {
    let files = test_files();
    let a = "`ticket_drift::checker_finds_a_wrong_quote`";
    let b = "`ticket_drift::open_tickets_are_plans`";
    let ticket = |proof: &str, plan: &str| {
        format!(
            "## Description\n\n**Risk tags:** `data`.\n\n## Spec\n\n| AC-1 | x | {a} |\n\n## Design\n\n| R-1 | y | {b} |\n\n## Test plan\n\n| Main | z | {plan} |\n\n## Proof\n\n| AC-1, R-1 | {proof} |\n\n## Plan\n"
        )
    };
    assert!(
        list_problems(
            "T-9",
            &ticket(&format!("{a}, {b}"), &format!("{a}, {b}")),
            &files
        )
        .is_empty()
    );
    let p = list_problems("T-9", &ticket(a, &format!("{a}, {b}")), &files);
    assert!(
        p.iter()
            .any(|x| x.contains("open_tickets_are_plans") && x.contains("Proof")),
        "{p:?}"
    );
    let p = list_problems("T-9", &ticket(&format!("{a}, {b}"), a), &files);
    assert!(
        p.iter()
            .any(|x| x.contains("open_tickets_are_plans") && x.contains("Test plan")),
        "{p:?}"
    );
    assert!(list_problems("STP-3", &ticket(a, a), &files).is_empty());
}

/// STP-4 AC-8: every long option of every `stapel` subcommand is in some ticket's Inputs table,
/// apart from those older than the rule.
#[test]
fn every_option_is_in_an_inputs_table() {
    let options = stapel_options();
    assert!(
        options.contains("--reason") && options.contains("--until"),
        "{options:?}"
    );
    let tickets: Vec<String> = tickets().into_iter().map(|(_, t)| t).collect();
    let missing = options_missing(&options, &tickets);
    assert!(
        missing.is_empty(),
        "options in no Inputs table: {missing:?}"
    );

    let fake: BTreeSet<String> = ["--reason", "--brand-new"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let inputs = "### Inputs\n\n| `--other` | flag |\n".to_string();
    assert_eq!(options_missing(&fake, &[inputs.clone()]), ["--brand-new"]);
    let named = format!("{inputs}| `--brand-new` | flag |\n");
    assert!(options_missing(&fake, &[named]).is_empty());
}

/// STP-4 step 1 drift review D1-1: the AC-8 rules wait until every criterion is built or the ticket
/// is closed.
#[test]
fn process_rules_wait_until_built() {
    let files = test_files();
    let code = code_text();
    let unbuilt = "## Spec\n\n| AC-1 | x | `ticket_drift::not_written_yet` |\n";
    let open = full_problems("T-9", unbuilt, Lifecycle::Open, &files, &code);
    assert!(!open.iter().any(|p| p.contains("risk tags")), "{open:?}");
    let built = "## Spec\n\n| AC-1 | x | `ticket_drift::checker_finds_a_wrong_quote` |\n";
    let open = full_problems("T-9", built, Lifecycle::Open, &files, &code);
    assert!(open.iter().any(|p| p.contains("risk tags")), "{open:?}");
    let closed = full_problems("T-9", unbuilt, Lifecycle::Closed, &files, &code);
    assert!(closed.iter().any(|p| p.contains("risk tags")), "{closed:?}");
}

/// STP-4 step 1 drift review D1-2, D1-3: every Inputs table counts, and only its rows do.
#[test]
fn inputs_tables_are_read_in_full() {
    let options: BTreeSet<String> = ["--alpha", "--beta", "--gamma"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let ticket = "### Inputs\n\n| `--alpha` | flag |\n\n### Other\n\n| `--gamma` | flag |\n\n\
                  ### Inputs of the second command\n\n| `--beta <n>` | number |\n\nProse names `--gamma`.\n";
    assert_eq!(
        options_missing(&options, &[ticket.to_string()]),
        ["--gamma"]
    );
}
/// STP-4 step 1 drift review D1-5: `security` alone needs the same, and an undated self-check fails.
#[test]
fn security_tag_and_undated_self_check() {
    let tagged = "**Risk tags:** `security`.\n\n### Abuse table\n\n";
    let undated = format!("{tagged}**Author self-check.** Done: nothing new.\n");
    let p = process_problems("T-9", &undated);
    assert!(p.iter().any(|x| x.contains("self-check")), "{p:?}");
    let not_heading = "**Risk tags:** `security`.\n\n#### Abuse table\n\n**Author self-check.** Done 2026-10-05.\n";
    let p = process_problems("T-9", not_heading);
    assert!(p.iter().any(|x| x.contains("abuse table")), "{p:?}");
}
