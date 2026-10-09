//! Build inputs that a commit of a ticket's range changes (STP-6 AC-1, AC-2, AC-6): the target
//! tables and keys of a `Cargo.toml`, cargo configuration, toolchain files, build scripts and
//! symbolic links. Such a change means a test run at HEAD may not run the code under test.

use crate::includes::Includes;
use crate::outcomes::{red_changes_code, under_tests};
use crate::rust_tests::helper_text;
use crate::steps::{Marker, Step, git, parent, show, step_tests};
use std::collections::{BTreeSet, HashMap, HashSet};
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

/// The file a manifest's `[package] build` key names, relative to the repository.
fn build_file(manifest_path: &str, text: &str) -> Option<String> {
    let t: toml::Table = toml::from_str(text).ok()?;
    let toml::Value::String(name) = t.get("package")?.get("build")? else {
        return None;
    };
    let mut parts: Vec<&str> = manifest_path.split('/').collect();
    parts.pop();
    for c in name.split('/') {
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
    for c in steps.iter().flat_map(|s| s.commits.iter()) {
        if c.marker != Marker::Red {
            continue;
        }
        if let Some(at) = commits.iter().position(|s| *s == c.sha) {
            for t in step_tests(root, &c.sha)? {
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
                            && helper_text_of(root, &base, e.old.as_deref())
                                != helper_text_of(root, sha, e.new.as_deref())
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

/// The text of a `.rs` file at a commit without its test functions; a missing file has none.
fn helper_text_of(root: &Path, commit: &str, path: Option<&str>) -> String {
    path.and_then(|p| show(root, commit, p))
        .map(|src| helper_text(&src))
        .unwrap_or_default()
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
    let before = show(root, base, e.old.as_deref()?).and_then(|t| targets(&t));
    let after = show(root, sha, e.new.as_deref()?).and_then(|t| targets(&t));
    match (before, after) {
        (Some(b), Some(a)) if b == a => None,
        _ => Some(path.clone()),
    }
}
