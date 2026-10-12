//! Build inputs that a commit of a ticket's range changes (STP-6 AC-1, AC-2, AC-6): the target
//! tables and keys of a `Cargo.toml`, cargo configuration, toolchain files, build scripts and
//! symbolic links. Such a change means a test run at HEAD may not run the code under test.

use crate::includes::Includes;
use crate::outcomes::{red_changes_code, under_tests};
use crate::rust_tests::{helper_text, test_functions};
use crate::steps::{Marker, Step, git, parent, show, step_tests};
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

/// What the range says about build inputs.
#[derive(Debug, Clone, Default)]
pub struct BuildInputs {
    /// The first non-RED commit of the range that changes a build input, with the path it names.
    pub changed: Option<Change>,
    /// Build inputs at HEAD that no non-RED commit of the range changes, sorted.
    pub unchanged: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    /// The full id of the commit.
    pub sha: String,
}

/// Where a ticket's range starts and ends (STP-6 AC-13, AC-15).
#[derive(Debug, Clone, Copy, Default)]
pub struct Bounds<'a> {
    /// The `base` of `state.json`: the range starts after it. Without it the range starts at the
    /// oldest commit whose subject carries the key.
    pub base: Option<&'a str>,
    /// A closed ticket: the range ends at the last commit whose subject carries the key.
    pub closed: bool,
}

/// A `base` must be a full hex commit id on the first-parent history of `head`.
pub fn check_base(root: &Path, base: &str, head: &str) -> Result<(), String> {
    let full = matches!(base.len(), 40 | 64)
        && base.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    let on_line = full
        && git(root, &["rev-list", "--first-parent", head])
            .map(|o| o.split(|b| *b == b'\n').any(|l| l == base.as_bytes()))
            .unwrap_or(false);
    if on_line {
        Ok(())
    } else {
        Err(format!(
            "state.json has base {base}, which is not a full commit id on the first-parent history of HEAD"
        ))
    }
}

/// The commits of the ticket's range, oldest first: the first-parent history after `bounds.base`
/// (else from the oldest commit whose subject starts with `<key> ` or `<key>:`) up to `head`, or for
/// a closed ticket up to its last commit whose subject carries the key.
pub fn range(root: &Path, key: &str, head: &str, bounds: Bounds) -> Result<Vec<String>, String> {
    let raw = git(
        root,
        &[
            "log",
            "-z",
            "--first-parent",
            "--reverse",
            "--format=%H%x00%s",
            head,
        ],
    )?;
    let fields: Vec<String> = raw
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect();
    let commits: Vec<(String, &String)> = fields
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].trim_start_matches('\n').to_string(), &c[1]))
        .filter(|(sha, _)| !sha.is_empty())
        .collect();
    let keyed = |s: &str| s.starts_with(&format!("{key} ")) || s.starts_with(&format!("{key}:"));
    let start = match bounds.base {
        Some(base) => commits
            .iter()
            .position(|(sha, _)| sha == base)
            .map(|i| i + 1),
        None => commits.iter().position(|(_, s)| keyed(s)),
    };
    let end = if bounds.closed {
        commits
            .iter()
            .rposition(|(_, s)| keyed(s))
            .map_or(0, |i| i + 1)
    } else {
        commits.len()
    };
    Ok(match start {
        Some(i) if i < end => commits[i..end].iter().map(|(sha, _)| sha.clone()).collect(),
        _ => Vec::new(),
    })
}

/// A path that is a build input by its name: a `.cargo` component, a toolchain file or `build.rs`.
fn named_input(path: &str) -> bool {
    let mut parts = path.split('/');
    let last = parts.next_back().unwrap_or("");
    matches!(last, "rust-toolchain" | "rust-toolchain.toml" | "build.rs")
        || last == ".cargo"
        || parts.any(|p| p == ".cargo")
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

fn is_manifest(path: &str) -> bool {
    path.rsplit('/').next() == Some("Cargo.toml")
}

/// The parts of a manifest that decide which targets exist, for comparison.
fn targets(text: &str) -> Option<toml::Table> {
    let t: toml::Table = toml::from_str(text).ok()?;
    let mut out = toml::Table::new();
    for k in ["lib", "bin", "test", "example", "bench"] {
        if let Some(v) = t.get(k) {
            out.insert(k.into(), v.clone());
        }
    }
    let pick = |section: &str, keys: &[&str]| {
        let mut part = toml::Table::new();
        if let Some(toml::Value::Table(s)) = t.get(section) {
            for k in keys {
                if let Some(v) = s.get(*k) {
                    part.insert((*k).into(), v.clone());
                }
            }
        }
        part
    };
    let ws = pick("workspace", &["members", "exclude", "default-members"]);
    if !ws.is_empty() {
        out.insert("workspace".into(), toml::Value::Table(ws));
    }
    let pkg = pick(
        "package",
        &[
            "build",
            "autolib",
            "autobins",
            "autotests",
            "autoexamples",
            "autobenches",
        ],
    );
    if !pkg.is_empty() {
        out.insert("package".into(), toml::Value::Table(pkg));
    }
    Some(out)
}

/// The repository path that `rel` names from the folder of `manifest_path`; `None` when it leaves
/// the repository.
fn resolve(manifest_path: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = manifest_path.split('/').collect();
    parts.pop();
    for c in rel.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            c => parts.push(c),
        }
    }
    Some(parts.join("/"))
}

/// The file a manifest's `[package] build` key names, relative to the repository.
fn build_file(manifest_path: &str, text: &str) -> Option<String> {
    let t: toml::Table = toml::from_str(text).ok()?;
    let toml::Value::String(name) = t.get("package")?.get("build")? else {
        return None;
    };
    resolve(manifest_path, name)
}

/// The `build` file of a manifest text, or `None` when the text does not parse.
fn parse_build(manifest_path: &str, text: &str) -> Option<Option<String>> {
    toml::from_str::<toml::Table>(text).ok()?;
    Some(build_file(manifest_path, text))
}

/// One entry of `git diff-tree --raw -z`.
struct Entry {
    old_mode: String,
    new_mode: String,
    status: char,
    old: Option<String>,
    new: Option<String>,
}

/// A tree entry of `git ls-tree -r -z`: mode, blob id and path.
struct Leaf {
    mode: String,
    id: String,
    path: String,
}

struct Reader<'a> {
    root: &'a Path,
    /// `[package] build` files of a commit's manifests.
    builds: HashMap<String, BTreeSet<String>>,
    /// The `build` file of a manifest blob; `Err` when the blob cannot be read or parsed.
    blobs: HashMap<(String, String), Result<Option<String>, ()>>,
    /// The directories of a commit's manifests that cannot be read or parsed.
    broken: HashMap<String, Vec<String>>,
}

impl Reader<'_> {
    fn tree(&self, commit: &str) -> Result<Vec<Leaf>, String> {
        let raw = git(self.root, &["ls-tree", "-r", "-z", commit])?;
        let mut out = Vec::new();
        for rec in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let text = String::from_utf8_lossy(rec).into_owned();
            let malformed = || format!("git ls-tree {commit}: malformed record {text:?}");
            let Some((meta, path)) = text.split_once('\t') else {
                return Err(malformed());
            };
            let mut it = meta.split(' ');
            let (Some(mode), Some(_kind), Some(id)) = (it.next(), it.next(), it.next()) else {
                return Err(malformed());
            };
            out.push(Leaf {
                mode: mode.into(),
                id: id.into(),
                path: path.into(),
            });
        }
        Ok(out)
    }

    /// The files that the manifests of a commit name with `build`.
    fn build_files(&mut self, commit: &str) -> Result<BTreeSet<String>, String> {
        if let Some(s) = self.builds.get(commit) {
            return Ok(s.clone());
        }
        let mut set = BTreeSet::new();
        let mut broken: Vec<String> = Vec::new();
        for leaf in self.tree(commit)? {
            if !is_manifest(&leaf.path) || leaf.mode == "120000" {
                continue;
            }
            let key = (leaf.id.clone(), leaf.path.clone());
            let file = match self.blobs.get(&key) {
                Some(f) => f.clone(),
                None => {
                    let f = show(self.root, commit, &leaf.path)
                        .and_then(|text| parse_build(&leaf.path, &text))
                        .ok_or(());
                    self.blobs.insert(key, f.clone());
                    f
                }
            };
            match file {
                Ok(f) => set.extend(f),
                Err(()) => {
                    broken.push(dir_of(&leaf.path).to_string());
                }
            }
        }
        self.broken.insert(commit.into(), broken);
        self.builds.insert(commit.into(), set.clone());
        Ok(set)
    }
}

/// The entries of a commit against its first parent, in `git` order, renames detected. Fields are
/// split on NUL and paths are literal.
///
/// The flag is set when a record is malformed or the output is short: the caller must not skip it.
fn entries(root: &Path, sha: &str) -> Result<(String, Vec<Entry>, bool), String> {
    let base = parent(root, sha);
    let raw = git(
        root,
        &[
            "diff-tree",
            "-r",
            "-z",
            "-M",
            "--raw",
            "--no-commit-id",
            &base,
            sha,
        ],
    )?;
    let fields: Vec<String> = raw
        .split(|b| *b == 0)
        .map(|f| String::from_utf8_lossy(f).into_owned())
        .collect();
    let mut out = Vec::new();
    let mut bad = false;
    let mut i = 0;
    while i < fields.len() {
        if fields[i].is_empty() && i + 1 == fields.len() {
            break;
        }
        let Some(meta) = fields[i].strip_prefix(':') else {
            bad = true;
            i += 1;
            continue;
        };
        let parts: Vec<&str> = meta.split(' ').collect();
        let status = parts.get(4).and_then(|s| s.chars().next()).unwrap_or('M');
        let (old_mode, new_mode) = (
            parts.first().copied().unwrap_or("").to_string(),
            parts.get(1).copied().unwrap_or("").to_string(),
        );
        let two = status == 'R' || status == 'C';
        let n = if two { 2 } else { 1 };
        if i + n >= fields.len() {
            bad = true;
            break;
        }
        let first = fields.get(i + 1).cloned();
        let second = if two {
            fields.get(i + 2).cloned()
        } else {
            None
        };
        let (old, new) = match status {
            'A' => (None, first),
            'D' => (first, None),
            _ if two => (first, second),
            _ => (first.clone(), first),
        };
        out.push(Entry {
            old_mode,
            new_mode,
            status,
            old,
            new,
        });
        i += 1 + n;
    }
    Ok((base, out, bad))
}

/// Reads the range: the first commit that changes a build input, and the build inputs at HEAD that
/// no commit changes. A RED of the ticket that changes no code is skipped; every other commit of the
/// range counts.
pub fn analyse(
    root: &Path,
    key: &str,
    head: &str,
    steps: &[Step],
    bounds: Bounds,
) -> Result<BuildInputs, String> {
    let commits = range(root, key, head, bounds)?;
    let mut reds: HashSet<String> = HashSet::new();
    for c in steps.iter().flat_map(|s| s.commits.iter()) {
        if c.marker == Marker::Red && commits.contains(&c.sha) && !red_changes_code(root, &c.sha)? {
            reds.insert(c.sha.clone());
        }
    }
    let mut reader = Reader {
        root,
        builds: HashMap::new(),
        blobs: HashMap::new(),
        broken: HashMap::new(),
    };
    // Helpers: every `.rs` file under `crates/*/tests/` that holds no step test (AC-3). Included
    // files: what the step test files and the helpers include at the base and at every commit of the
    // range (AC-4).
    // A path is a step file from the first RED of the range, in range order, that added or changed
    // a step test in it (AC-17, AC-18): the index of that RED in the range.
    let mut step_from: HashMap<String, usize> = HashMap::new();
    // The names of the step tests of each path, from every RED of the ticket (AC-17).
    let mut step_names: HashMap<String, BTreeSet<String>> = HashMap::new();
    // The crate folders that hold a step test of the ticket, from every RED of the range: the
    // folders whose tests folder STP-4 protects as a whole (AC-5).
    let mut step_crates: HashSet<String> = HashSet::new();
    for c in steps.iter().flat_map(|s| s.commits.iter()) {
        if c.marker != Marker::Red {
            continue;
        }
        if let Some(at) = commits.iter().position(|s| *s == c.sha) {
            for t in step_tests(root, &c.sha)? {
                step_crates.insert(t.dir.clone());
                step_names
                    .entry(t.path())
                    .or_default()
                    .insert(t.name.clone());
                let first = step_from.entry(t.path()).or_insert(at);
                *first = (*first).min(at);
            }
        }
    }
    let is_step_file = |p: &str, at: usize| step_from.get(p).is_some_and(|from| *from <= at);
    let is_helper =
        |p: &str, at: usize| p.ends_with(".rs") && under_tests(p) && !is_step_file(p, at);
    let mut seen: Vec<String> = vec![head.to_string()];
    if let Some(first) = commits.first() {
        seen.push(parent(root, first));
    }
    seen.extend(commits.iter().cloned());
    let included = Includes::new(root).included(&seen, under_tests)?;
    let mut changed: Option<Change> = None;
    let mut touched: BTreeSet<String> = BTreeSet::new();
    for (at, sha) in commits
        .iter()
        .enumerate()
        .filter(|(_, s)| !reds.contains(*s))
    {
        let (base, list, bad) = entries(root, sha)?;
        if list.is_empty() && !bad {
            continue;
        }
        let mut builds = reader.build_files(&base)?;
        builds.extend(reader.build_files(sha)?);
        // Fail closed: a malformed diff record counts as a change naming the commit; a manifest that
        // cannot be read hides its `build` key, so a file next to it counts (as an unparsable
        // manifest does in `manifest_hit`).
        if bad && changed.is_none() {
            changed = Some(Change {
                path: format!("diff of {sha}"),
                sha: sha.clone(),
            });
        }
        let broken: Vec<String> = [&base, sha]
            .iter()
            .flat_map(|c| reader.broken[*c].iter().cloned())
            .collect();
        for e in &list {
            let paths: Vec<&String> = [e.old.as_ref(), e.new.as_ref()]
                .into_iter()
                .flatten()
                .collect();
            let link = e.old_mode == "120000" || e.new_mode == "120000";
            let mut hit: Option<String> = None;
            for p in &paths {
                if link
                    || named_input(p)
                    || builds.contains(*p)
                    || broken.iter().any(|d| d == dir_of(p))
                {
                    touched.insert((*p).clone());
                    hit.get_or_insert_with(|| (*p).clone());
                }
            }
            if hit.is_none() {
                hit = manifest_hit(root, sha, &base, e);
            }
            if hit.is_none() {
                hit = data_hit(e, &step_crates);
            }
            if hit.is_none() {
                hit = paths
                    .iter()
                    .find(|p| is_helper(p, at) || included.contains(**p))
                    .map(|p| (*p).clone());
            }
            // The text of a step file outside its test functions is a helper's text (AC-17).
            if hit.is_none() {
                hit = paths
                    .iter()
                    .find(|p| {
                        p.ends_with(".rs")
                            && under_tests(p)
                            && is_step_file(p, at)
                            && (helper_text_of(root, &base, e.old.as_deref())
                                != helper_text_of(root, sha, e.new.as_deref())
                                || test_functions_hit(
                                    root,
                                    (&base, e.old.as_deref()),
                                    (sha, e.new.as_deref()),
                                    step_names.get(p.as_str()),
                                ))
                    })
                    .map(|p| (*p).clone());
            }
            if let Some(path) = hit
                && changed.is_none()
            {
                changed = Some(Change {
                    path,
                    sha: sha.clone(),
                });
            }
        }
    }
    let at_head = reader.tree(head)?;
    let builds = reader.build_files(head)?;
    let unchanged = at_head
        .iter()
        .filter(|l| l.mode == "120000" || named_input(&l.path) || builds.contains(&l.path))
        .map(|l| l.path.clone())
        .filter(|p| !touched.contains(p))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(BuildInputs { changed, unchanged })
}

/// The crate folder of a path under `crates/*/tests/`.
fn crate_of(path: &str) -> Option<&str> {
    under_tests(path).then(|| path.split('/').nth(1).unwrap_or(""))
}

/// The path of a data file under `crates/*/tests/` that an entry adds (a submodule entry included),
/// or changes, deletes or renames in a crate that holds no step test of the ticket (STP-13 AC-5).
/// A path is added when the entry has no old path or the old path is in another crate.
fn data_hit(e: &Entry, step_crates: &HashSet<String>) -> Option<String> {
    let data = |p: &&String| under_tests(p) && !p.ends_with(".rs");
    let free = |p: &String| !step_crates.contains(crate_of(p).unwrap_or(""));
    // A rename names the old path first, as the other build-input rules do.
    if let Some(old) = e.old.as_ref().filter(data).filter(|o| free(o)) {
        return Some(old.clone());
    }
    let new = e.new.as_ref().filter(data)?;
    let added = e.old.as_ref().is_none_or(|o| crate_of(o) != crate_of(new));
    (added || free(new)).then(|| new.clone())
}

/// The text of a `.rs` file at a commit without its test functions; a missing file has none.
fn helper_text_of(root: &Path, commit: &str, path: Option<&str>) -> String {
    path.and_then(|p| show(root, commit, p))
        .map(|src| helper_text(&src))
        .unwrap_or_default()
}

/// The test functions of a step file that a non-RED commit may not touch (AC-17): a `#[test]`
/// function that is no step test and is changed or deleted, or is added while a step test names it
/// as a word. A new test function that no step test names stays allowed.
fn test_functions_hit(
    root: &Path,
    before: (&str, Option<&str>),
    after: (&str, Option<&str>),
    steps: Option<&BTreeSet<String>>,
) -> bool {
    let read = |(commit, path): (&str, Option<&str>)| -> Vec<(String, String)> {
        path.and_then(|p| show(root, commit, p))
            .map(|src| test_functions(&src))
            .unwrap_or_default()
    };
    let (old, new) = (read(before), read(after));
    let is_step = |n: &str| steps.is_some_and(|s| s.contains(n));
    let new_of = |n: &str| new.iter().find(|(m, _)| m == n).map(|(_, t)| t);
    if old.iter().any(|(n, t)| !is_step(n) && new_of(n) != Some(t)) {
        return true;
    }
    let step_words: HashSet<&str> = old
        .iter()
        .chain(new.iter())
        .filter(|(n, _)| is_step(n))
        .flat_map(|(_, t)| t.split(|c: char| !(c.is_alphanumeric() || c == '_')))
        .collect();
    new.iter().any(|(n, _)| {
        !is_step(n) && !old.iter().any(|(m, _)| m == n) && step_words.contains(n.as_str())
    })
}

/// The manifest path of an entry whose target tables or keys changed, or that was added or deleted.
fn manifest_hit(root: &Path, sha: &str, base: &str, e: &Entry) -> Option<String> {
    let path = [e.old.as_ref(), e.new.as_ref()]
        .into_iter()
        .flatten()
        .find(|p| is_manifest(p))?;
    if e.status != 'M' && e.status != 'T' {
        return Some(path.clone());
    }
    let (before_text, after_text) = (
        show(root, base, e.old.as_deref()?),
        show(root, sha, e.new.as_deref()?),
    );
    let before = before_text.as_deref().and_then(targets);
    let after = after_text.as_deref().and_then(targets);
    match (before, after) {
        (Some(b), Some(a)) if b == a => {}
        _ => return Some(path.clone()),
    }
    // Both texts parse here: `targets` returned a table for each.
    let parse = |t: &Option<String>| {
        t.as_deref()
            .and_then(|t| toml::from_str::<toml::Table>(t).ok())
    };
    let (Some(b), Some(a)) = (parse(&before_text), parse(&after_text)) else {
        return Some(path.clone());
    };
    let manifest = e.new.as_deref()?;
    let members: OnceCell<Option<BTreeSet<String>>> = OnceCell::new();
    let is_member = |folder: &str| {
        members
            .get_or_init(|| member_folders(root, sha))
            .as_ref()
            .is_some_and(|m| m.contains(folder))
    };
    (build_tables(&b) != build_tables(&a) || dependency_redirect(&b, &a, manifest, &is_member))
        .then(|| path.clone())
}

/// The tables and keys of a manifest that change how a test is built apart from its targets and
/// dependencies (STP-13 AC-3): `[patch]`, `[replace]`, `[profile]`, `[features]`, `[lints]`,
/// `[workspace.lints]` and `edition` of `[package]`, `[project]` and `[workspace.package]`. A key
/// that is absent is left out, so adding and deleting one differ from the other side.
fn build_tables(t: &toml::Table) -> toml::Table {
    let mut out = toml::Table::new();
    for k in ["patch", "replace", "profile", "features", "lints"] {
        if let Some(v) = t.get(k) {
            out.insert(k.into(), v.clone());
        }
    }
    let edition = |t: &toml::Table, section: &str| t.get(section)?.get("edition").cloned();
    for section in ["package", "project"] {
        if let Some(v) = edition(t, section) {
            out.insert(format!("{section}.edition"), v);
        }
    }
    if let Some(toml::Value::Table(ws)) = t.get("workspace") {
        if let Some(v) = ws.get("lints") {
            out.insert("workspace.lints".into(), v.clone());
        }
        if let Some(v) = edition(ws, "package") {
            out.insert("workspace.package.edition".into(), v);
        }
    }
    out
}

/// The kind of a dependency table by its key: `dev-dependencies` and `build-dependencies` have
/// underscore aliases that cargo still reads.
fn dependency_kind(key: &str) -> Option<&'static str> {
    match key {
        "dependencies" => Some("dependencies"),
        "dev-dependencies" | "dev_dependencies" => Some("dev"),
        "build-dependencies" | "build_dependencies" => Some("build"),
        _ => None,
    }
}

/// Every dependency entry of a manifest by scope (top level, `target.<cfg>`, `[workspace]`), kind
/// and name, whatever its form (STP-13 AC-1).
fn dependency_entries(t: &toml::Table) -> BTreeMap<(String, &'static str, String), toml::Value> {
    let mut out = BTreeMap::new();
    let mut add = |scope: &str, tables: &toml::Table, workspace: bool| {
        for (key, value) in tables {
            let kind = if workspace {
                (key == "dependencies").then_some("dependencies")
            } else {
                dependency_kind(key)
            };
            if let (Some(kind), toml::Value::Table(entries)) = (kind, value) {
                for (name, entry) in entries {
                    out.insert((scope.to_string(), kind, name.clone()), entry.clone());
                }
            }
        }
    };
    add("", t, false);
    if let Some(toml::Value::Table(targets)) = t.get("target") {
        for (cfg, tables) in targets {
            if let toml::Value::Table(tables) = tables {
                add(&format!("target {cfg}"), tables, false);
            }
        }
    }
    if let Some(toml::Value::Table(ws)) = t.get("workspace") {
        add("workspace", ws, true);
    }
    out
}

/// The keys of an entry that decide where its code comes from; `None` for an entry that is neither
/// a string nor a table. An empty map is crates.io or `workspace = true`.
fn dependency_source(entry: &toml::Value) -> Option<BTreeMap<&'static str, &toml::Value>> {
    const KEYS: [&str; 8] = [
        "path",
        "git",
        "branch",
        "tag",
        "rev",
        "package",
        "registry",
        "registry-index",
    ];
    match entry {
        toml::Value::String(_) => Some(BTreeMap::new()),
        toml::Value::Table(t) => Some(
            KEYS.iter()
                .filter_map(|k| t.get(*k).map(|v| (*k, v)))
                .collect(),
        ),
        _ => None,
    }
}

/// Whether a manifest change makes a dependency entry point at other code (STP-13 AC-1): an entry
/// that is added or whose source changes, unless the new source is empty or, outside
/// `dev-dependencies`, a `path` (with or without `package`) to a member folder; an entry moved from
/// `path` or `git` to an empty source; an entry that changes and is neither a string nor a table.
/// A deleted entry is no redirect. An entry added to a `dev-dependencies` table is one whatever its
/// source (STP-13 AC-4): a RED that does not compile may be made to pass by it.
fn dependency_redirect(
    before: &toml::Table,
    after: &toml::Table,
    manifest: &str,
    is_member: &dyn Fn(&str) -> bool,
) -> bool {
    let old = dependency_entries(before);
    dependency_entries(after).iter().any(|(id, entry)| {
        let old_entry = old.get(id);
        if old_entry == Some(entry) {
            return false;
        }
        if old_entry.is_none() && id.1 == "dev" {
            return true;
        }
        let Some(new_src) = dependency_source(entry) else {
            return true;
        };
        let old_src = old_entry.and_then(dependency_source).unwrap_or_default();
        if new_src == old_src {
            return false;
        }
        if new_src.is_empty() {
            return old_src.contains_key("path") || old_src.contains_key("git");
        }
        let member_path = id.1 != "dev"
            && new_src.keys().all(|k| matches!(*k, "path" | "package"))
            && matches!(new_src.get("path"), Some(toml::Value::String(rel))
                if resolve(manifest, rel).is_some_and(|f| is_member(&f)));
        !member_path
    })
}

/// The folders of the workspace members of a commit: the root package's folder (the empty string)
/// and the `members` of the root manifest, globs expanded over the commit's manifests, minus
/// `exclude`. `None` when the root manifest cannot be read.
fn member_folders(root: &Path, commit: &str) -> Option<BTreeSet<String>> {
    let text = show(root, commit, "Cargo.toml")?;
    let t: toml::Table = toml::from_str(&text).ok()?;
    let listed = git(root, &["ls-tree", "-r", "-z", commit]).ok()?;
    let folders: Vec<String> = listed
        .split(|b| *b == 0)
        .map(|r| String::from_utf8_lossy(r).into_owned())
        .filter_map(|rec| {
            let (meta, path) = rec.split_once('\t')?;
            (!meta.starts_with("120000") && is_manifest(path)).then(|| dir_of(path).to_string())
        })
        .collect();
    let list = |key: &str| -> Vec<String> {
        match t.get("workspace").and_then(|w| w.get(key)) {
            Some(toml::Value::Array(a)) => a
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| {
                    s.split('/')
                        .filter(|c| !c.is_empty() && *c != ".")
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .collect(),
            _ => Vec::new(),
        }
    };
    let excluded = list("exclude");
    let mut out = BTreeSet::new();
    if t.contains_key("package") || t.contains_key("project") {
        out.insert(String::new());
    }
    for pattern in list("members") {
        for folder in &folders {
            let gone = excluded
                .iter()
                .any(|x| folder == x || folder.starts_with(&format!("{x}/")));
            if !gone && !folder.is_empty() && glob_path(&pattern, folder) {
                out.insert(folder.clone());
            }
        }
    }
    Some(out)
}

/// A path glob as cargo reads `members`: `*`, `?` and `[...]` within one path component.
fn glob_path(pattern: &str, path: &str) -> bool {
    let (p, q): (Vec<&str>, Vec<&str>) = (pattern.split('/').collect(), path.split('/').collect());
    p.len() == q.len()
        && p.iter().zip(&q).all(|(p, q)| {
            let (p, q): (Vec<char>, Vec<char>) = (p.chars().collect(), q.chars().collect());
            glob_component(&p, &q)
        })
}

fn glob_component(p: &[char], q: &[char]) -> bool {
    match p.first() {
        None => q.is_empty(),
        Some('*') => (0..=q.len()).any(|i| glob_component(&p[1..], &q[i..])),
        Some('?') => !q.is_empty() && glob_component(&p[1..], &q[1..]),
        Some('[') => {
            let Some(end) = p.iter().skip(2).position(|c| *c == ']').map(|i| i + 2) else {
                return q.first() == Some(&'[') && glob_component(&p[1..], &q[1..]);
            };
            let (negated, set) = match p[1] {
                '!' | '^' => (true, &p[2..end]),
                _ => (false, &p[1..end]),
            };
            let Some(c) = q.first() else { return false };
            let mut hit = false;
            let mut i = 0;
            while i < set.len() {
                if i + 2 < set.len() && set[i + 1] == '-' {
                    hit |= set[i] <= *c && *c <= set[i + 2];
                    i += 3;
                } else {
                    hit |= set[i] == *c;
                    i += 1;
                }
            }
            hit != negated && glob_component(&p[end + 1..], &q[1..])
        }
        Some(c) => q.first() == Some(c) && glob_component(&p[1..], &q[1..]),
    }
}
