//! Outcomes of a step that come from git history alone (STP-4 AC-2), in their order:
//! `unpaired`, `duplicate`, `no-tests`, `red-changes-code`, `tests-changed`; and the notes
//! `superseded by <label>` and `retired by <label>`.

use crate::build_inputs::{
    base_merge_parent, dev_redirect, merge_base_had, same_as_parent, target_files,
};
use crate::config::glob_matches;
use crate::includes::Includes;
use crate::rust_tests::{helper_text, test_functions};
use crate::steps::{
    Change, Marker, Step, StepCommit, TestId, changes, git, git_text, is_test_file,
    junit_step_tests, parent, show, step_tests,
};
use crate::ts_tests::{rest_text, test_calls};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

/// The runner whose rules judge a step (STP-28): cargo, or junit with the test globs.
#[derive(Debug, Clone, Copy)]
pub enum Mode<'a> {
    Cargo,
    Junit(&'a [String]),
}

/// Whether a path a junit RED changes is code (STP-28 AC-13): it matches no test glob, is not under
/// `docs/` or `.stapel/` and is not a `.md` file.
fn is_junit_code(path: &str, globs: &[String]) -> bool {
    !(globs.iter().any(|g| glob_matches(g, path))
        || path.starts_with("docs/")
        || path.starts_with(".stapel/")
        || path.ends_with(".md"))
}

/// At most this many paths or names are named in one reason.
pub const MAX_NAMED: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub kind: &'static str,
    pub by: String,
    /// A step test or a protected path.
    pub what: String,
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub label: String,
    pub red: Option<StepCommit>,
    pub green: Option<StepCommit>,
    pub tests: Vec<TestId>,
    /// The outcome from history alone, with its reason; `None` when the tests must run.
    pub fixed: Option<(String, String)>,
    /// The label of another step's RED that lies between this step's RED and GREEN (STP-6 AC-12):
    /// the step reads `unverified: interleaved: <label>` unless a build-input outcome comes first.
    pub interleaved: Option<String>,
    pub notes: Vec<Note>,
    /// Step tests that still run: not retired.
    pub to_run: Vec<TestId>,
    /// The full id of the commit that made the fixed outcome, or of the `interleaved` RED
    /// (STP-14). `None` for a passing outcome and for a RED with no step commit after it.
    pub fault: Option<String>,
    /// STP-37 AC-10: the test files a history commit changed, each with the newest such commit;
    /// the RED run takes the file from it.
    pub red_files: Vec<(String, String)>,
}

/// What a RED commit's own changes say: its step tests, its parent and its changes, and
/// `no-tests` or `red-changes-code` when they fail it.
struct RedVerdict {
    tests: Vec<TestId>,
    base: String,
    changed: Vec<Change>,
    fixed: Option<(String, String)>,
}

/// Judges a RED by its changes (STP-4 AC-2, STP-6 AC-5). `later_greens` are the GREEN commits of
/// the ticket after the RED; a file that a source file includes at the RED, its parent, one of
/// them or `head` counts as code.
fn judge_red(
    files: &mut Files,
    includes: &mut Includes,
    red: &str,
    head: &str,
    later_greens: Vec<String>,
    mode: Mode,
) -> Result<RedVerdict, String> {
    let root = files.root;
    let tests = match mode {
        Mode::Cargo => step_tests(root, red)?,
        Mode::Junit(globs) => junit_step_tests(root, red, globs)?,
    };
    if tests.is_empty() {
        return Ok(RedVerdict {
            tests,
            base: String::new(),
            changed: Vec::new(),
            fixed: Some((
                "no-tests".into(),
                "the RED commit adds or changes no test".into(),
            )),
        });
    }
    let base = parent(root, red);
    let changed = changes(root, red)?;
    if let Mode::Junit(globs) = mode {
        let mut code: Vec<String> = changed
            .iter()
            .flat_map(|c| [c.old.clone(), c.new.clone()])
            .flatten()
            .filter(|p| is_junit_code(p, globs))
            .collect();
        code.sort();
        code.dedup();
        let fixed = (!code.is_empty()).then(|| ("red-changes-code".to_string(), named(&code)));
        return Ok(RedVerdict {
            tests,
            base,
            changed,
            fixed,
        });
    }
    let mut code: Vec<String> = changed
        .iter()
        .filter(|c| is_code(files, red, &base, c))
        .map(|c| c.new.clone().or(c.old.clone()).unwrap_or_default())
        .collect();
    let mut seen: Vec<String> = vec![red.to_string(), base.clone(), head.to_string()];
    seen.extend(later_greens);
    let included = includes.included(&seen, |f| !under_tests(f))?;
    for c in &changed {
        for p in [c.old.as_ref(), c.new.as_ref()].into_iter().flatten() {
            if included.contains(p) && !code.contains(p) {
                code.push(p.clone());
            }
        }
    }
    let fixed = (!code.is_empty()).then(|| ("red-changes-code".to_string(), named(&code)));
    Ok(RedVerdict {
        tests,
        base,
        changed,
        fixed,
    })
}

/// `no-tests` or `red-changes-code` for a RED with no step commit after it (STP-14 AC-18), by the
/// same code as a paired step's RED; `None` when the RED is correct.
pub fn open_red(
    root: &Path,
    red: &str,
    head: &str,
    mode: Mode,
) -> Result<Option<(String, String)>, String> {
    let mut files = Files {
        root,
        cache: HashMap::new(),
        named: HashMap::new(),
    };
    let mut includes = Includes::new(root);
    Ok(judge_red(&mut files, &mut includes, red, head, Vec::new(), mode)?.fixed)
}

/// Contents of files at commits, read once.
struct Files<'a> {
    root: &'a Path,
    cache: HashMap<(String, String), Option<String>>,
    /// The files that the manifests of a commit name, per commit (STP-13 AC-8).
    named: HashMap<String, Option<HashSet<String>>>,
}

impl Files<'_> {
    fn get(&mut self, commit: &str, path: &str) -> Option<String> {
        let key = (commit.to_string(), path.to_string());
        if let Some(v) = self.cache.get(&key) {
            return v.clone();
        }
        let v = show(self.root, commit, path);
        self.cache.insert(key, v.clone());
        v
    }

    /// The files named by `build` and target `path` keys of every manifest at a commit; `None` when
    /// the listing or a manifest cannot be read: the caller counts the commit as code (D4-1).
    fn named_files(&mut self, commit: &str) -> Option<&HashSet<String>> {
        if !self.named.contains_key(commit) {
            let set = self.read_named(commit);
            self.named.insert(commit.to_string(), set);
        }
        self.named[commit].as_ref()
    }

    fn read_named(&mut self, commit: &str) -> Option<HashSet<String>> {
        let mut set = HashSet::new();
        let listed = git(self.root, &["ls-tree", "-r", "-z", "--name-only", commit]).ok()?;
        for path in listed.split(|b| *b == 0) {
            let path = String::from_utf8_lossy(path).into_owned();
            if path.rsplit('/').next() == Some("Cargo.toml") {
                set.extend(target_files(&path, &self.get(commit, &path)?));
            }
        }
        Some(set)
    }
}

pub fn named(items: &[String]) -> String {
    let mut text = items
        .iter()
        .take(MAX_NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > MAX_NAMED {
        text.push_str(&format!(" and {} more", items.len() - MAX_NAMED));
    }
    text
}

/// A manifest without its `dev-dependencies` tables, for comparison.
fn manifest_without_dev(text: &str) -> Option<toml::Table> {
    let mut t: toml::Table = toml::from_str(text).ok()?;
    t.remove("dev-dependencies");
    t.remove("dev_dependencies");
    if let Some(toml::Value::Table(targets)) = t.get_mut("target") {
        for (_, v) in targets.iter_mut() {
            if let toml::Value::Table(target) = v {
                target.remove("dev-dependencies");
                target.remove("dev_dependencies");
            }
        }
        // A target table that held only dev-dependencies is gone with them.
        targets.retain(|_, v| !matches!(v, toml::Value::Table(t) if t.is_empty()));
        if targets.is_empty() {
            t.remove("target");
        }
    }
    Some(t)
}

pub fn under_tests(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    parts.len() >= 4 && parts[0] == "crates" && parts[2] == "tests"
}

/// Whether a path a RED commit changes is code: a `.rs` file outside `crates/*/tests/`, a
/// `build.rs`, a manifest change outside `dev-dependencies`, or any path outside the test, docs,
/// ticket and markdown paths.
fn is_code_stp4(
    files: &mut Files,
    red: &str,
    base: &str,
    old: Option<&str>,
    new: Option<&str>,
) -> bool {
    let path = new.or(old).unwrap_or("");
    let name = path.rsplit('/').next().unwrap_or(path);
    if name == "build.rs" {
        return true;
    }
    if path.ends_with(".rs") {
        return !under_tests(path);
    }
    if under_tests(path) || name == "Cargo.lock" || path.ends_with(".md") {
        return false;
    }
    if path.starts_with(".stapel/") || path.starts_with("docs/") {
        return false;
    }
    if name == "Cargo.toml" {
        let before = old.and_then(|o| files.get(base, o));
        let after = new.and_then(|n| files.get(red, n));
        return match (before, after) {
            (Some(b), Some(a)) => {
                let (b, a) = (manifest_without_dev(&b), manifest_without_dev(&a));
                b.is_none() || a.is_none() || b != a
            }
            _ => true,
        };
    }
    true
}

/// `is_code_stp4` and the STP-13 rules (AC-6, AC-7, AC-8): a dev-dependency redirect in a manifest, a
/// symbolic link, manifest, `.cargo` path or toolchain file under `crates/*/tests/`, and a file that a
/// manifest at the RED or its parent names by `build` or a target `path`.
fn is_code(files: &mut Files, red: &str, base: &str, c: &Change) -> bool {
    let (old, new) = (c.old.as_deref(), c.new.as_deref());
    if is_code_stp4(files, red, base, old, new) {
        return true;
    }
    let paths: Vec<&str> = [old, new].into_iter().flatten().collect();
    let link = c.old_mode == "120000" || c.new_mode == "120000";
    for path in &paths {
        let name = path.rsplit('/').next().unwrap_or(path);
        if under_tests(path)
            && (link
                || name == "Cargo.toml"
                || name == "rust-toolchain"
                || name == "rust-toolchain.toml"
                || path.split('/').any(|p| p == ".cargo"))
        {
            return true;
        }
        // A commit whose manifests cannot be listed counts as code (D4-1).
        let at_red = files.named_files(red).map(|s| s.contains(*path));
        let at_base = files.named_files(base).map(|s| s.contains(*path));
        if at_red != Some(false) || at_base != Some(false) {
            return true;
        }
    }
    let manifest = paths
        .iter()
        .any(|p| p.rsplit('/').next() == Some("Cargo.toml"));
    if manifest && let (Some(o), Some(n)) = (old, new) {
        let before = files.get(base, o);
        let after = files.get(red, n);
        if let (Some(b), Some(a)) = (before, after) {
            let roots = (files.get(base, "Cargo.toml"), files.get(red, "Cargo.toml"));
            return dev_redirect(&b, &a, n, (roots.0.as_deref(), roots.1.as_deref()));
        }
    }
    false
}

/// Whether a RED commit changes code (STP-4 AC-2); such a commit is no RED for the build-input
/// outcome.
pub fn red_changes_code(root: &Path, red: &str) -> Result<bool, String> {
    let mut files = Files {
        root,
        cache: HashMap::new(),
        named: HashMap::new(),
    };
    let base = parent(root, red);
    Ok(changes(root, red)?
        .iter()
        .any(|c| is_code(&mut files, red, &base, c)))
}

/// A part of a step's protected content.
#[derive(Debug, Clone)]
enum Unit {
    Test(TestId),
    /// A `.rs` file's text without its test functions.
    Helpers(String),
    /// A junit test file's text without its test calls.
    Rest(String),
    /// Any other file, byte for byte.
    File(String),
}

impl Unit {
    fn path(&self) -> String {
        match self {
            Unit::Test(t) => t.path(),
            Unit::Helpers(p) | Unit::Rest(p) | Unit::File(p) => p.clone(),
        }
    }
    fn name(&self) -> String {
        match self {
            Unit::Test(t) => t.to_string(),
            Unit::Helpers(p) | Unit::Rest(p) | Unit::File(p) => p.clone(),
        }
    }
    fn value(&self, files: &mut Files, commit: &str, junit: bool) -> Option<String> {
        let src = files.get(commit, &self.path())?;
        match self {
            Unit::Test(t) if junit => test_calls(&src)
                .into_iter()
                .find(|(n, _)| *n == t.name)
                .map(|(_, text)| text),
            Unit::Test(t) => test_functions(&src)
                .into_iter()
                .find(|(n, _)| *n == t.name)
                .map(|(_, text)| text),
            Unit::Rest(_) => Some(rest_text(&src)),
            Unit::Helpers(_) => Some(helper_text(&src)),
            Unit::File(_) => Some(src),
        }
    }
}

/// Commits after `from` up to `head` on the first-parent history that touch `path`, oldest first.
/// Fields are split on NUL, which a commit message cannot hold; the path is given literally.
fn touching(
    root: &Path,
    from: &str,
    head: &str,
    path: &str,
) -> Result<Vec<(String, String)>, String> {
    let raw = git(
        root,
        &[
            "log",
            "-z",
            "--first-parent",
            "--reverse",
            "--format=%H%x00%B",
            &format!("{from}..{head}"),
            "--",
            &format!(":(literal){path}"),
        ],
    )?;
    let fields: Vec<String> = raw
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect();
    Ok(fields
        .chunks(2)
        .filter(|c| c.len() == 2 && !c[0].is_empty())
        .map(|c| (c[0].trim_start_matches('\n').to_string(), c[1].clone()))
        .collect())
}

pub fn analyse(
    root: &Path,
    key: &str,
    head: &str,
    steps: &[Step],
) -> Result<Vec<Analysis>, String> {
    analyse_in(root, key, head, steps, None, None, Mode::Cargo)
}

/// `analyse` under a runner. Under junit the protected content is each step test call, the rest of
/// each test file the RED changed and its other changed files that match a test glob.
pub fn analyse_in(
    root: &Path,
    key: &str,
    head: &str,
    steps: &[Step],
    base: Option<&str>,
    base_branch: Option<&str>,
    mode: Mode,
) -> Result<Vec<Analysis>, String> {
    let mut files = Files {
        root,
        cache: HashMap::new(),
        named: HashMap::new(),
    };
    let mut includes = Includes::new(root);
    // Every step commit of the ticket by sha: its label and marker.
    let mut by_sha: BTreeMap<String, (String, Marker)> = BTreeMap::new();
    let mut position: BTreeMap<String, usize> = BTreeMap::new();
    let mut all: Vec<&StepCommit> = steps.iter().flat_map(|s| s.commits.iter()).collect();
    all.sort_by_key(|c| c.sha.clone());
    for c in &all {
        by_sha.insert(c.sha.clone(), (c.label.clone(), c.marker));
    }
    let order = git_text(
        root,
        &[
            "rev-list",
            "--first-parent",
            "--reverse",
            &crate::steps::walk_range(base, head),
        ],
    )?;
    for (i, sha) in order.lines().enumerate() {
        position.insert(sha.to_string(), i);
    }
    let pos_of = |sha: &str| position.get(sha).copied().unwrap_or(usize::MAX);
    let pos = |c: &StepCommit| pos_of(&c.sha);

    let mut out = Vec::new();
    for step in steps {
        let reds: Vec<&StepCommit> = step.reds().collect();
        let greens: Vec<&StepCommit> = step.greens().collect();
        let mut a = Analysis {
            label: step.label.clone(),
            red: reds.first().map(|c| (*c).clone()),
            green: greens.first().map(|c| (*c).clone()),
            tests: Vec::new(),
            fixed: None,
            interleaved: None,
            notes: Vec::new(),
            to_run: Vec::new(),
            fault: None,
            red_files: Vec::new(),
        };
        let fix = |a: &mut Analysis, o: &str, r: String| a.fixed = Some((o.to_string(), r));
        if reds.is_empty() || greens.is_empty() || pos(greens[0]) < pos(reds[0]) {
            let (reason, fault) = if reds.is_empty() {
                ("no RED commit", Some(greens[0].sha.clone()))
            } else if greens.is_empty() {
                // The first step commit of the ticket after the RED; none for an open RED.
                let after = all
                    .iter()
                    .filter(|c| pos(c) > pos(reds[0]))
                    .min_by_key(|c| pos(c))
                    .map(|c| c.sha.clone());
                ("no GREEN commit", after)
            } else {
                (
                    "the GREEN commit comes before the RED commit",
                    Some(reds[0].sha.clone()),
                )
            };
            fix(&mut a, "unpaired", reason.into());
            a.fault = fault;
            out.push(a);
            continue;
        }
        if reds.len() > 1 || greens.len() > 1 {
            fix(
                &mut a,
                "duplicate",
                format!("{} RED and {} GREEN commits", reds.len(), greens.len()),
            );
            a.fault = [reds.get(1), greens.get(1)]
                .into_iter()
                .flatten()
                .min_by_key(|c| pos(c))
                .map(|c| c.sha.clone());
            out.push(a);
            continue;
        }
        let red = reds[0].sha.clone();
        let later_greens = all
            .iter()
            .filter(|c| c.marker == Marker::Green && pos(c) > pos(reds[0]))
            .map(|c| c.sha.clone())
            .collect();
        let verdict = judge_red(&mut files, &mut includes, &red, head, later_greens, mode)?;
        a.tests = verdict.tests;
        if let Some(fixed) = verdict.fixed {
            a.fixed = Some(fixed);
            a.fault = Some(red);
            out.push(a);
            continue;
        }
        let (base, changed) = (verdict.base, verdict.changed);

        // Protected content: the step tests and the other test files the RED changed.
        let mut units: Vec<Unit> = a.tests.iter().cloned().map(Unit::Test).collect();
        let junit = !matches!(mode, Mode::Cargo);
        for c in &changed {
            let Some(new) = c.new.as_deref() else {
                continue;
            };
            if let Mode::Junit(globs) = mode {
                // STP-28 AC-27: the rest of a test file the RED changed, and any other file of
                // it that matches a test glob.
                if is_test_file(new, globs) {
                    let rest = |s: String| rest_text(&s);
                    let now = files.get(&red, new).map(rest).unwrap_or_default();
                    let before = c
                        .old
                        .as_deref()
                        .and_then(|o| files.get(&base, o))
                        .map(rest)
                        .unwrap_or_default();
                    // Blank space and the `;` after a test call are no helper text.
                    let bare = |s: &str| s.replace(|c: char| c.is_whitespace() || c == ';', "");
                    if bare(&now) != bare(&before) {
                        units.push(Unit::Rest(new.to_string()));
                    }
                } else if globs.iter().any(|g| glob_matches(g, new)) {
                    units.push(Unit::File(new.to_string()));
                }
                continue;
            }
            if !under_tests(new) {
                continue;
            }
            if new.ends_with(".rs") {
                let now = files
                    .get(&red, new)
                    .map(|s| helper_text(&s))
                    .unwrap_or_default();
                let before = c
                    .old
                    .as_deref()
                    .and_then(|o| files.get(&base, o))
                    .map(|s| helper_text(&s))
                    .unwrap_or_default();
                if now != before {
                    units.push(Unit::Helpers(new.to_string()));
                }
            } else {
                units.push(Unit::File(new.to_string()));
            }
        }
        // Every other file under the tests folder of a crate that holds a step test, as at the RED.
        let mut dirs: Vec<&str> = a.tests.iter().map(|t| t.dir.as_str()).collect();
        dirs.sort();
        dirs.dedup();
        for dir in dirs.into_iter().filter(|d| !d.is_empty()) {
            let listed = git(
                root,
                &[
                    "ls-tree",
                    "-r",
                    "-z",
                    "--name-only",
                    &red,
                    "--",
                    &format!(":(literal)crates/{dir}/tests/"),
                ],
            )?;
            for path in listed.split(|b| *b == 0).filter(|p| !p.is_empty()) {
                let path = String::from_utf8_lossy(path).into_owned();
                let known = units
                    .iter()
                    .any(|u| matches!(u, Unit::File(p) if *p == path));
                if !path.ends_with(".rs") && !known {
                    units.push(Unit::File(path));
                }
            }
        }
        // STP-28 AC-42: under junit, each non-test file matching a test glob in the folder of a
        // step test's file, as at the RED.
        if let Mode::Junit(globs) = mode {
            let mut folders: Vec<String> = a
                .tests
                .iter()
                .map(|t| {
                    let p = t.path();
                    p.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default()
                })
                .collect();
            folders.sort();
            folders.dedup();
            for folder in folders {
                let prefix = if folder.is_empty() {
                    String::new()
                } else {
                    format!("{folder}/")
                };
                let mut args = vec!["ls-tree", "-z", "--name-only", red.as_str(), "--"];
                let literal = format!(":(literal){prefix}");
                if !prefix.is_empty() {
                    args.push(&literal);
                }
                let mut listed = git(root, &args)?;
                // A file a later commit adds or deletes in the folder is protected as well.
                let range = format!("{red}..{head}");
                let touched = git(
                    root,
                    &["log", "-z", "--format=", "--name-only", "--no-renames", &range],
                )?;
                for path in touched.split(|b| *b == 0).filter(|p| !p.is_empty()) {
                    let direct = std::str::from_utf8(path)
                        .ok()
                        .and_then(|p| p.strip_prefix(prefix.as_str()))
                        .is_some_and(|rest| !rest.contains('/'));
                    if direct {
                        listed.push(0);
                        listed.extend_from_slice(path);
                    }
                }
                for path in listed.split(|b| *b == 0).filter(|p| !p.is_empty()) {
                    let path = String::from_utf8_lossy(path).into_owned();
                    let known = units
                        .iter()
                        .any(|u| matches!(u, Unit::File(p) | Unit::Rest(p) if *p == path));
                    if !known
                        && !is_test_file(&path, globs)
                        && globs.iter().any(|g| glob_matches(g, &path))
                    {
                        units.push(Unit::File(path));
                    }
                }
            }
        }
        let mut changed_units = Vec::new();
        // The commits where the walk of a changed unit stopped.
        let mut stops: Vec<String> = Vec::new();
        let mut retired = Vec::new();
        for unit in &units {
            let mut current = unit.value(&mut files, &red, junit);
            let at_head = unit.value(&mut files, head, junit);
            for (sha, message) in touching(root, &red, head, &unit.path())? {
                let value = unit.value(&mut files, &sha, junit);
                if value == current {
                    continue;
                }
                // STP-30 AC-1: a Base merge whose value equals its second parent's brings no
                // change; later commits are compared with the value at the merge (AC-16).
                // A unit absent at the merge takes the deletion path below. Under cargo a file
                // unit is Equal by content and mode; under junit, as any other unit, by its value.
                // A file unit absent at the merge and at the second parent is Equal as well, when
                // the merge base of the two parents had it (the base branch deleted it).
                if (value.is_some() || matches!(unit, Unit::File(_)))
                    && let Some(second) = base_merge_parent(root, &sha, base_branch)
                    && match unit {
                        Unit::File(_) if value.is_none() => {
                            unit.value(&mut files, &second, junit).is_none()
                                && merge_base_had(root, &sha, &second, &unit.path())
                        }
                        Unit::File(p) if !junit => same_as_parent(root, &sha, &second, p),
                        _ => value == unit.value(&mut files, &second, junit),
                    }
                {
                    current = value;
                    continue;
                }
                let step_of = by_sha.get(&sha);
                let label = step_of
                    .map(|(l, _)| l.clone())
                    .unwrap_or_else(|| sha[..7.min(sha.len())].to_string());
                if value.is_none() {
                    // A deletion by a commit of the ticket with `spec change:` retires a test that
                    // stays gone; otherwise the walk goes on, so a later re-add is compared with
                    // the protected text.
                    let of_ticket = message.starts_with(&format!("{key} "))
                        || message.starts_with(&format!("{key}:"));
                    if let Unit::Test(t) = unit
                        && at_head.is_none()
                        && of_ticket
                        && message.contains("spec change:")
                    {
                        a.notes.push(Note {
                            kind: "retired by",
                            by: label,
                            what: t.to_string(),
                        });
                        retired.push(t.clone());
                        break;
                    }
                    if at_head.is_none() {
                        // STP-28 AC-42: under junit a protected file that is gone is changed.
                        if junit && matches!(unit, Unit::File(_)) && current.is_some() {
                            changed_units.push(unit.name());
                            stops.push(sha.clone());
                        }
                        break;
                    }
                    continue;
                }
                match step_of {
                    Some((l, Marker::Red)) => {
                        a.notes.push(Note {
                            kind: "superseded by",
                            by: l.clone(),
                            what: unit.name(),
                        });
                        current = value;
                    }
                    // STP-37 AC-9: a history commit may change a protected test; its text is the
                    // protected text from here on.
                    _ if matches!(unit, Unit::Test(_))
                        && crate::steps::history_reason(&message).is_some() =>
                    {
                        let note = Note {
                            kind: "history by",
                            by: label,
                            what: crate::steps::history_reason(&message).unwrap_or_default(),
                        };
                        if !a.notes.contains(&note) {
                            a.notes.push(note);
                        }
                        let path = unit.path();
                        match a.red_files.iter_mut().find(|(p, _)| *p == path) {
                            Some(old) if pos_of(&old.1) > pos_of(&sha) => {}
                            Some(old) => old.1 = sha.clone(),
                            None => a.red_files.push((path, sha.clone())),
                        }
                        current = value;
                    }
                    _ => {
                        if at_head.is_some() {
                            changed_units.push(unit.name());
                            stops.push(sha.clone());
                        }
                        break;
                    }
                }
            }
        }
        if !changed_units.is_empty() {
            changed_units.sort();
            changed_units.dedup();
            fix(&mut a, "tests-changed", named(&changed_units));
            a.fault = stops.into_iter().min_by_key(|s| pos_of(s));
            out.push(a);
            continue;
        }
        let own = pos(greens[0]);
        let inside = all
            .iter()
            .filter(|c| c.marker == Marker::Red && pos(c) > pos(reds[0]) && pos(c) < own)
            .min_by_key(|c| pos(c));
        a.interleaved = inside.map(|c| c.label.clone());
        a.fault = inside.map(|c| c.sha.clone());
        a.to_run = a
            .tests
            .iter()
            .filter(|t| !retired.contains(t))
            .cloned()
            .collect();
        if a.to_run.is_empty() {
            fix(
                &mut a,
                "retired",
                "every step test was retired by a spec change".into(),
            );
            a.fault = None;
        }
        out.push(a);
    }
    Ok(out)
}

/// The outcome of a step that needs no cargo run (STP-14), in the full check's order: the fixed
/// outcome; else `unverified: <blocked>` when a build input blocks the run; else
/// `unverified: interleaved: <label>`. `None` when the step's tests must run.
pub fn history_outcome(a: &Analysis, blocked: Option<&str>) -> Option<(String, String)> {
    if let Some(fixed) = &a.fixed {
        return Some(fixed.clone());
    }
    if let Some(why) = blocked {
        return Some(("unverified".into(), why.to_string()));
    }
    a.interleaved
        .as_ref()
        .map(|label| ("unverified".into(), format!("interleaved: {label}")))
}

/// Whether an outcome counts as passing.
pub fn passing(outcome: &str) -> bool {
    outcome == "pass" || outcome == "retired" || outcome == "cached"
}
