//! Outcomes of a step that come from git history alone (STP-4 AC-2), in their order:
//! `unpaired`, `duplicate`, `no-tests`, `red-changes-code`, `tests-changed`; and the notes
//! `superseded by <label>` and `retired by <label>`.

use crate::build_inputs::{dev_redirect, target_files};
use crate::includes::Includes;
use crate::rust_tests::{helper_text, test_functions};
use crate::steps::{
    Change, Marker, Step, StepCommit, TestId, changes, git, git_text, parent, show, step_tests,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

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
) -> Result<RedVerdict, String> {
    let root = files.root;
    let tests = step_tests(root, red)?;
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
pub fn open_red(root: &Path, red: &str, head: &str) -> Result<Option<(String, String)>, String> {
    let mut files = Files {
        root,
        cache: HashMap::new(),
        named: HashMap::new(),
    };
    let mut includes = Includes::new(root);
    Ok(judge_red(&mut files, &mut includes, red, head, Vec::new())?.fixed)
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
    /// Any other file, byte for byte.
    File(String),
}

impl Unit {
    fn path(&self) -> String {
        match self {
            Unit::Test(t) => t.path(),
            Unit::Helpers(p) | Unit::File(p) => p.clone(),
        }
    }
    fn name(&self) -> String {
        match self {
            Unit::Test(t) => t.to_string(),
            Unit::Helpers(p) | Unit::File(p) => p.clone(),
        }
    }
    fn value(&self, files: &mut Files, commit: &str) -> Option<String> {
        let src = files.get(commit, &self.path())?;
        match self {
            Unit::Test(t) => test_functions(&src)
                .into_iter()
                .find(|(n, _)| *n == t.name)
                .map(|(_, text)| text),
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
    let order = git_text(root, &["rev-list", "--first-parent", "--reverse", head])?;
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
        let verdict = judge_red(&mut files, &mut includes, &red, head, later_greens)?;
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
        for c in &changed {
            let Some(new) = c.new.as_deref() else {
                continue;
            };
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
        for dir in dirs {
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
        let mut changed_units = Vec::new();
        // The commits where the walk of a changed unit stopped.
        let mut stops: Vec<String> = Vec::new();
        let mut retired = Vec::new();
        for unit in &units {
            let mut current = unit.value(&mut files, &red);
            let at_head = unit.value(&mut files, head);
            for (sha, message) in touching(root, &red, head, &unit.path())? {
                let value = unit.value(&mut files, &sha);
                if value == current {
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
    outcome == "pass" || outcome == "retired"
}
