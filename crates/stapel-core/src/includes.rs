//! Files that a `.rs` file includes by a literal path (STP-6 AC-4, AC-5): `include!`,
//! `include_str!` and `include_bytes!` with a plain or raw string literal, found by a text search
//! (comments count) and taken relative to the including file. Every Rust escape of a plain literal
//! is decoded; a literal that cannot be decoded counts as a change of its includer. A non-literal
//! argument or a path outside the repository is not promised here.

use crate::steps::git;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// The string literal paths that `include*!` calls in a text name, in order; `None` for a literal
/// that cannot be decoded.
pub fn literals(text: &str) -> Vec<Option<String>> {
    let c: Vec<char> = text.chars().collect();
    let word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let skip = |mut i: usize| {
        while c.get(i).is_some_and(|ch| ch.is_whitespace()) {
            i += 1;
        }
        i
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] != 'i' || (i > 0 && word(c[i - 1])) {
            i += 1;
            continue;
        }
        let rest: String = c[i..(i + 13).min(c.len())].iter().collect();
        let len = ["include_bytes", "include_str", "include"]
            .iter()
            .find(|n| rest.starts_with(**n))
            .map_or(0, |n| n.len());
        if len == 0 {
            i += 1;
            continue;
        }
        let mut j = skip(i + len);
        if c.get(j) != Some(&'!') {
            i += len;
            continue;
        }
        j = skip(j + 1);
        if !matches!(c.get(j), Some('(' | '[' | '{')) {
            i += len;
            continue;
        }
        j = skip(j + 1);
        if let Some((lit, end)) = literal(&c, j) {
            out.push(lit);
            i = end;
        } else {
            i += len;
        }
    }
    out
}

/// The content (`None` when an escape cannot be decoded) and end of a plain or raw string literal
/// starting at `i`.
fn literal(c: &[char], i: usize) -> Option<(Option<String>, usize)> {
    if c.get(i) == Some(&'r') {
        let mut j = i + 1;
        let mut hashes = 0;
        while c.get(j) == Some(&'#') {
            hashes += 1;
            j += 1;
        }
        if c.get(j) != Some(&'"') {
            return None;
        }
        let start = j + 1;
        let mut k = start;
        while k < c.len() {
            if c[k] == '"' && (1..=hashes).all(|n| c.get(k + n) == Some(&'#')) {
                return Some((Some(c[start..k].iter().collect()), k + 1 + hashes));
            }
            k += 1;
        }
        return None;
    }
    if c.get(i) != Some(&'"') {
        return None;
    }
    let mut out = Some(String::new());
    let mut k = i + 1;
    while k < c.len() {
        match c[k] {
            '"' => return Some((out, k + 1)),
            '\\' => {
                let (ch, next) = escape(c, k + 1);
                match (&mut out, ch) {
                    (Some(o), Some(ch)) => o.push_str(&ch),
                    _ => out = None,
                }
                k = next.max(k + 2);
            }
            ch => {
                if let Some(o) = &mut out {
                    o.push(ch);
                }
                k += 1;
            }
        }
    }
    None
}

/// The text of the escape whose backslash precedes `i`, and the index after it. `None` text for
/// an escape that cannot be decoded.
fn escape(c: &[char], i: usize) -> (Option<String>, usize) {
    let one = |s: &str| (Some(s.to_string()), i + 1);
    match c.get(i) {
        Some('\\') => one("\\"),
        Some('"') => one("\""),
        Some('\'') => one("'"),
        Some('n') => one("\n"),
        Some('r') => one("\r"),
        Some('t') => one("\t"),
        Some('0') => one("\0"),
        Some('\n' | '\r') => {
            let mut j = i;
            while c
                .get(j)
                .is_some_and(|ch| matches!(ch, ' ' | '\t' | '\n' | '\r'))
            {
                j += 1;
            }
            (Some(String::new()), j)
        }
        Some('x') => {
            let digits: String = c.iter().skip(i + 1).take(2).collect();
            let value = (digits.len() == 2 && digits.chars().all(|d| d.is_ascii_hexdigit()))
                .then(|| u32::from_str_radix(&digits, 16).ok())
                .flatten()
                .filter(|v| *v <= 0x7f)
                .and_then(char::from_u32);
            (value.map(String::from), i + 3)
        }
        Some('u') if c.get(i + 1) == Some(&'{') => {
            let mut j = i + 2;
            let mut digits = String::new();
            while let Some(ch) = c.get(j) {
                if *ch == '}' {
                    break;
                }
                digits.push(*ch);
                j += 1;
            }
            let digits = digits.replace('_', "");
            let value = (c.get(j) == Some(&'}')
                && (1..=6).contains(&digits.len())
                && digits.chars().all(|d| d.is_ascii_hexdigit()))
            .then(|| u32::from_str_radix(&digits, 16).ok())
            .flatten()
            .and_then(char::from_u32);
            (value.map(String::from), j + 1)
        }
        _ => (None, i + 1),
    }
}

/// The path that an include of `lit` in the file `from` names, relative to the repository; `None`
/// for an absolute path or one that leaves the repository.
fn resolve(from: &str, lit: &str) -> Option<String> {
    if lit.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in lit.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The include literals of the `.rs` files of commits, read once per blob.
pub struct Includes<'a> {
    root: &'a Path,
    by_blob: HashMap<String, Vec<Option<String>>>,
    by_commit: HashMap<String, Vec<(String, Vec<String>)>>,
}

impl<'a> Includes<'a> {
    pub fn new(root: &'a Path) -> Self {
        Includes {
            root,
            by_blob: HashMap::new(),
            by_commit: HashMap::new(),
        }
    }

    /// Every `.rs` file of a commit with the paths it includes.
    fn at(&mut self, commit: &str) -> Result<&Vec<(String, Vec<String>)>, String> {
        if !self.by_commit.contains_key(commit) {
            let raw = git(self.root, &["ls-tree", "-r", "-z", commit])?;
            let mut files = Vec::new();
            for rec in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
                let text = String::from_utf8_lossy(rec).into_owned();
                let Some((meta, path)) = text.split_once('\t') else {
                    return Err(format!("git ls-tree {commit}: malformed record {text:?}"));
                };
                let mut it = meta.split(' ');
                let (Some(mode), Some(_), Some(id)) = (it.next(), it.next(), it.next()) else {
                    return Err(format!("git ls-tree {commit}: malformed record {text:?}"));
                };
                if !path.ends_with(".rs") || mode == "120000" || mode == "160000" {
                    continue;
                }
                if !self.by_blob.contains_key(id) {
                    let blob = git(self.root, &["cat-file", "blob", id])?;
                    let found = literals(&String::from_utf8_lossy(&blob));
                    self.by_blob.insert(id.to_string(), found);
                }
                let lits = &self.by_blob[id];
                if !lits.is_empty() {
                    // An undecodable literal counts as a change of its includer: the includer's own
                    // path joins the included paths.
                    let resolved = lits
                        .iter()
                        .filter_map(|l| match l {
                            Some(l) => resolve(path, l),
                            None => Some(path.to_string()),
                        })
                        .collect();
                    files.push((path.to_string(), resolved));
                }
            }
            self.by_commit.insert(commit.to_string(), files);
        }
        Ok(&self.by_commit[commit])
    }

    /// The paths that the `.rs` files of the given commits include, for the including files that
    /// `filter` accepts.
    pub fn included(
        &mut self,
        commits: &[String],
        filter: impl Fn(&str) -> bool,
    ) -> Result<BTreeSet<String>, String> {
        let mut out = BTreeSet::new();
        for commit in commits {
            for (file, paths) in self.at(commit)? {
                if filter(file) {
                    out.extend(paths.iter().cloned());
                }
            }
        }
        Ok(out)
    }
}
