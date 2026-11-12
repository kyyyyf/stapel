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

/// The cells of a table line.
fn cells(line: &str) -> Vec<&str> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').collect()
}

/// Whether a table line is the separator under a header (`|---|:--:|`).
fn is_separator(line: &str) -> bool {
    let cells = cells(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let c = c.trim();
            c.contains('-') && c.bytes().all(|b| matches!(b, b'-' | b':'))
        })
}

/// Named tests in table rows: backtick text holding `::`, trimmed, in a column headed `Test` or
/// `Tests`. A block of rows without a separator line has no header and names no test.
fn table_names(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut block: Vec<&str> = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with('|') {
            block.push(line);
        }
        if line.starts_with('|') && lines.get(i + 1).is_some_and(|l| l.starts_with('|')) {
            continue;
        }
        if block.is_empty() {
            continue;
        }
        let headed = block.get(1).is_some_and(|l| is_separator(l));
        if !headed {
            block.clear();
            continue;
        }
        let columns: Vec<usize> = cells(block[0])
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c.trim(), "Test" | "Tests"))
            .map(|(i, _)| i)
            .collect();
        for row in &block[2..] {
            for (at, cell) in cells(row).into_iter().enumerate() {
                if !columns.contains(&at) {
                    continue;
                }
                out.extend(
                    spans(cell)
                        .into_iter()
                        .filter(|s| s.contains("::"))
                        .map(|s| s.trim().to_string()),
                );
            }
        }
        block.clear();
    }
    out
}

/// The test lists of a Plan item: the text from `test ` or `tests ` to the next `)`.
fn test_lists(item: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for word in ["tests", "test"] {
        for (i, _) in item.match_indices(word) {
            let before_ok = item[..i]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_');
            let after = &item[i + word.len()..];
            if before_ok && after.starts_with([' ', '\n']) {
                out.push(&after[..after.find(')').unwrap_or(after.len())]);
            }
        }
    }
    out
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
            test_lists(&item)
                .into_iter()
                .flat_map(spans)
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
/// name whose segments are no identifiers. A missing file is a missing test.
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
    // The file part is a known cargo form with `::` (`core::config`), else the first segment.
    let cargo: Vec<(&str, Vec<String>)> = files
        .iter()
        .filter_map(|p| cargo_file(p).map(|(d, s)| (p.as_str(), cargo_forms(d, s))))
        .collect();
    let known = cargo
        .iter()
        .flat_map(|(_, forms)| forms)
        .filter(|f| f.contains("::"))
        .find(|f| {
            name.strip_prefix(f.as_str())
                .is_some_and(|r| r.starts_with("::"))
        });
    let (module, rest) = match known {
        Some(form) => (form.as_str(), &name[form.len() + 2..]),
        None => name.split_once("::")?,
    };
    let segments: Vec<&str> = rest.split("::").collect();
    if segments
        .iter()
        .any(|s| s.is_empty() || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    {
        return None;
    }
    let (test, mods) = segments.split_last()?;
    for (path, forms) in &cargo {
        if !forms.iter().any(|f| f == module) {
            continue;
        }
        let defined = show(root, head, path).is_some_and(|src| {
            if mods.is_empty() {
                test_functions(&src).iter().any(|(n, _)| n == test)
            } else {
                nested_test(&src, mods, test)
            }
        });
        if defined {
            return Some(true);
        }
    }
    // No test file of that name defines it, or there is no such file: the test does not exist.
    Some(false)
}

/// The end of the first whole-word `word` after `from` that `next` accepts.
fn word_after(
    src: &str,
    from: usize,
    word: &str,
    next: impl Fn(&str) -> Option<usize>,
) -> Option<usize> {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    for (i, _) in src[from..].match_indices(word) {
        let at = from + i;
        let end = at + word.len();
        if src[..at].chars().next_back().is_some_and(ident) || src[end..].starts_with(ident) {
            continue;
        }
        if let Some(n) = next(&src[end..]) {
            return Some(end + n);
        }
    }
    None
}

/// The text after whitespace and the identifier `name`, as an offset into `rest`.
fn named_next(rest: &str, name: &str) -> Option<usize> {
    let trimmed = rest.trim_start();
    let after = trimmed.strip_prefix(name)?;
    if after.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some(rest.len() - after.len())
}

/// Whether a `#[test]` function `test` stands in the nested `mod`s `mods` of a source, found by
/// text: each `mod <name>` after the one before, then `fn <test>` under a `#[test]` attribute.
fn nested_test(src: &str, mods: &[&str], test: &str) -> bool {
    let mut from = 0;
    for m in mods {
        let Some(end) = word_after(src, from, "mod", |r| named_next(r, m)) else {
            return false;
        };
        from = end;
    }
    let mut at = from;
    while let Some(end) = word_after(src, at, "fn", |r| named_next(r, test)) {
        let head = src[..end - test.len()].trim_end().len() - "fn".len();
        let item_start = src[..head].rfind(['}', '{', ';']).map_or(0, |i| i + 1);
        if src[item_start..head].contains("#[test]") {
            return true;
        }
        at = end;
    }
    false
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
