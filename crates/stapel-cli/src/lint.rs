//! `stapel lint [KEY]`: the two test-name checks of one ticket, for the cargo and the junit runner
//! (STP-28): a test the ticket names does not exist, and a step test no ticket names.

use crate::check::{base_of, junit_globs_at, read_head, shallow_refusal, status_refusal};
use crate::repo;
use stapel_core::rust_tests::test_functions;
use stapel_core::steps::{TestId, git, junit_step_tests, show, step_commits, step_tests, steps};
use stapel_core::tickets::{Status, list, match_branch, resolve};
use stapel_core::ts_tests::test_calls;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

/// A refusal: nothing was judged, exit 2.
fn refuse(message: impl std::fmt::Display) -> ExitCode {
    warn!("stapel: {message}");
    ExitCode::from(2)
}

/// Backticked spans of a text.
fn spans(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// Named tests in table rows: backtick text holding `::`, trimmed.
fn table_names(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|l| l.starts_with('|'))
        .flat_map(spans)
        .filter(|s| s.contains("::"))
        .map(|s| s.trim().to_string())
        .collect()
}

/// The items of the Plan section as (step label, text): an item starts with `<n>. ` at the start of
/// a line and runs to the next one; its label is `step <n>`.
fn plan_items(text: &str) -> Vec<(String, String)> {
    let Some(at) = text
        .match_indices("## Plan\n")
        .find(|(i, _)| *i == 0 || text.as_bytes()[i - 1] == b'\n')
        .map(|(i, m)| i + m.len())
    else {
        return Vec::new();
    };
    let rest = &text[at..];
    let body = &rest[..rest.find("\n## ").map(|i| i + 1).unwrap_or(rest.len())];
    let mut items: Vec<(String, String)> = Vec::new();
    for line in body.lines() {
        let number = line.split_once(". ").map(|(n, _)| n).filter(|n| {
            let digits = n.trim_end_matches(|c: char| c.is_ascii_lowercase());
            !digits.is_empty()
                && n.len() - digits.len() <= 1
                && digits.bytes().all(|b| b.is_ascii_digit())
        });
        match (number, items.last_mut()) {
            (Some(n), _) => items.push((format!("step {n}"), line.to_string())),
            (None, Some(item)) => {
                item.1.push('\n');
                item.1.push_str(line);
            }
            (None, None) => {}
        }
    }
    items
}

/// Named tests in the items of the Plan section that `keep` accepts.
fn plan_names(text: &str, keep: impl Fn(&str) -> bool) -> BTreeSet<String> {
    plan_items(text)
        .into_iter()
        .filter(|(label, _)| keep(label))
        .flat_map(|(_, item)| {
            spans(&item)
                .into_iter()
                .filter(|s| s.contains("::"))
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn is_script(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .and_then(|n| n.rsplit_once('.'))
        .is_some_and(|(_, ext)| {
            ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"].contains(&ext)
        })
}

/// The names a cargo test file `crates/<dir>/tests/<stem>.rs` goes by in a ticket: `<stem>`,
/// `<dir>/<stem>`, and `<suffix>/<stem>` with the crate suffix after any `-` of the folder name.
fn cargo_forms(dir: &str, stem: &str) -> Vec<String> {
    let mut forms = vec![stem.to_string(), format!("{dir}/{stem}")];
    for (i, _) in dir.match_indices('-') {
        forms.push(format!("{}::{stem}", &dir[i + 1..]));
    }
    forms
}

/// `(dir, stem)` of `crates/<dir>/tests/<stem>.rs`.
fn cargo_file(path: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["crates", dir, "tests", file] => Some((dir, file.strip_suffix(".rs")?)),
        _ => None,
    }
}

/// Whether the test a ticket names exists at `head`; `None` when the text names no test, as a
/// `module::name` whose module is no test file.
fn exists(root: &Path, head: &str, files: &BTreeSet<String>, name: &str) -> Option<bool> {
    if let Some((path, test)) = name.split_once("::")
        && is_script(path)
        && !path.contains(char::is_whitespace)
    {
        let test = test.trim();
        return Some(
            files.contains(path)
                && show(root, head, path)
                    .is_some_and(|src| test_calls(&src).iter().any(|(n, _)| n.trim() == test)),
        );
    }
    let (module, test) = name.rsplit_once("::")?;
    if test.is_empty() || !test.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return None;
    }
    let mut found = None;
    for path in files {
        let Some((dir, stem)) = cargo_file(path) else {
            continue;
        };
        if !cargo_forms(dir, stem).iter().any(|f| f == module) {
            continue;
        }
        let defined = show(root, head, path)
            .is_some_and(|src| test_functions(&src).iter().any(|(n, _)| n == test));
        if defined {
            return Some(true);
        }
        found = Some(false);
    }
    found
}

/// Every path of the tree at `head`.
fn tree_files(root: &Path, head: &str) -> Result<BTreeSet<String>, String> {
    let raw = git(root, &["ls-tree", "-r", "-z", "--name-only", head])?;
    Ok(raw
        .split(|b| *b == 0)
        .filter(|f| !f.is_empty())
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect())
}

/// The names under which a step test may stand in a table row of a ticket.
fn forms_of(test: &TestId) -> Vec<String> {
    if test.dir.is_empty() {
        return vec![test.to_string()];
    }
    cargo_forms(&test.dir, &test.file)
        .into_iter()
        .map(|f| format!("{f}::{}", test.name))
        .collect()
}

pub fn run(key: Option<&str>) -> ExitCode {
    let (root, _) = match repo::open() {
        Ok(v) => v,
        Err(e) => return refuse(e),
    };
    if let Err(e) = match_branch(&root, key) {
        return refuse(e);
    }
    let ticket = match resolve(&root, key) {
        Ok(t) => t,
        Err(e) => return refuse(e),
    };
    if let Err(e) = status_refusal(&ticket) {
        return refuse(e);
    }
    let closed = matches!(ticket.status, Status::Closed(_));
    let head = match read_head(&root) {
        Ok(h) => h,
        Err(e) => return refuse(e),
    };
    if let Err(e) = shallow_refusal(&root) {
        return refuse(e);
    }
    let base = match base_of(&root, &ticket, &head) {
        Ok(b) => b,
        Err(e) => return refuse(e),
    };
    let text = match std::fs::read_to_string(ticket.dir.join("ticket.md")) {
        Ok(t) => t,
        Err(e) => return refuse(format!("{}/ticket.md: {e}", ticket.dir.display())),
    };
    let commits = match step_commits(&root, &ticket.key, &head, base.as_deref()) {
        Ok(c) => steps(c),
        Err(e) => return refuse(e),
    };
    let files = match tree_files(&root, &head) {
        Ok(f) => f,
        Err(e) => return refuse(e),
    };

    // The tests the ticket names that must exist: all of a closed ticket, the Plan steps with a
    // GREEN of an open one.
    let checked = if closed {
        let mut all = table_names(&text);
        all.extend(plan_names(&text, |_| true));
        all
    } else {
        plan_names(&text, |label| {
            commits
                .iter()
                .any(|s| s.label == label && s.greens().next().is_some())
        })
    };
    let missing: BTreeSet<String> = checked
        .into_iter()
        .filter(|n| exists(&root, &head, &files, n) == Some(false))
        .collect();

    // The step tests of the RED commits that exist at HEAD and that no ticket names in a table row.
    let globs = junit_globs_at(&root);
    let mut step_tests_of = BTreeSet::new();
    for red in commits.iter().flat_map(|s| s.reds()) {
        let found = match &globs {
            Some(g) => junit_step_tests(&root, &red.sha, g),
            None => step_tests(&root, &red.sha),
        };
        match found {
            Ok(t) => step_tests_of.extend(t),
            Err(e) => return refuse(e),
        }
    }
    let mut named = BTreeSet::new();
    for t in list(&root) {
        if let Ok(t) = std::fs::read_to_string(t.dir.join("ticket.md")) {
            named.extend(table_names(&t));
        }
    }
    let unnamed: BTreeSet<String> = step_tests_of
        .iter()
        .filter(|t| exists(&root, &head, &files, &t.to_string()) == Some(true))
        .filter(|t| !forms_of(t).iter().any(|f| named.contains(f)))
        .map(|t| t.to_string())
        .collect();

    for name in &missing {
        say!("{} names {name}, which does not exist", ticket.key);
    }
    for name in &unnamed {
        say!(
            "{name} is a step test of {} and no ticket names it",
            ticket.key
        );
    }
    if missing.is_empty() && unnamed.is_empty() {
        say!("lint: {} clean", ticket.key);
        return ExitCode::SUCCESS;
    }
    ExitCode::from(1)
}
