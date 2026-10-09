//! Files that a `.rs` file includes by a literal path (STP-6 AC-4, AC-5): `include!`,
//! `include_str!` and `include_bytes!` with a plain or raw string literal, and `#[path = "<literal>"]`
//! (also inside `#[cfg_attr(<predicate>, path = "<literal>")]`) on a `mod`, found by a text search
//! (comments count, comments inside a call are skipped) and taken relative to the including file's
//! folder. Inside an inline `mod` rustc resolves a `#[path]` under `<file stem>/<mod name>/`; this
//! search does not model that, so such a path is not promised. Every Rust escape of a plain literal
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
    let skip = |i: usize| skip_blank(&c, i);
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '#' {
            if let Some((lit, end)) = path_attribute(&c, i) {
                out.push(lit);
                i = end;
            } else {
                i += 1;
            }
            continue;
        }
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

/// The index after the whitespace and comments (`//`, nested `/* */`) that start at `i`.
fn skip_blank(c: &[char], mut i: usize) -> usize {
    loop {
        while c.get(i).is_some_and(|ch| ch.is_whitespace()) {
            i += 1;
        }
        match (c.get(i), c.get(i + 1)) {
            (Some('/'), Some('/')) => {
                while c.get(i).is_some_and(|ch| *ch != '\n') {
                    i += 1;
                }
            }
            (Some('/'), Some('*')) => {
                let mut depth = 1;
                i += 2;
                while depth > 0 && i < c.len() {
                    match (c[i], c.get(i + 1)) {
                        ('/', Some('*')) => {
                            depth += 1;
                            i += 2;
                        }
                        ('*', Some('/')) => {
                            depth -= 1;
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }
            }
            _ => return i,
        }
    }
}

/// Whether the word `w` starts at `i` and ends there.
fn word_at(c: &[char], i: usize, w: &str) -> bool {
    let n = w.chars().count();
    c.get(i..i + n)
        .is_some_and(|s| s.iter().copied().eq(w.chars()))
        && !c
            .get(i + n)
            .is_some_and(|ch| ch.is_alphanumeric() || *ch == '_')
}

/// The index after a bracketed group that starts at `i` with `open`; `None` when it does not close.
fn group(c: &[char], i: usize, open: char, close: char) -> Option<usize> {
    if c.get(i) != Some(&open) {
        return None;
    }
    let mut depth = 0;
    let mut k = i;
    while k < c.len() {
        if c[k] == open {
            depth += 1;
        } else if c[k] == close {
            depth -= 1;
            if depth == 0 {
                return Some(k + 1);
            }
        }
        k += 1;
    }
    None
}

/// The literal and end of a `#[path = "<literal>"]` or `#[cfg_attr(<anything>, path = "<literal>")]`
/// attribute at `i` that sits on a `mod` item (other attributes and a visibility may stand between).
fn path_attribute(c: &[char], i: usize) -> Option<(Option<String>, usize)> {
    let mut j = skip_blank(c, i + 1);
    if c.get(j) != Some(&'[') {
        return None;
    }
    j = skip_blank(c, j + 1);
    let (lit, end, mut j) = if word_at(c, j, "path") {
        let (lit, end) = path_value(c, j)?;
        (lit, end, skip_blank(c, end))
    } else if word_at(c, j, "cfg_attr") {
        let (lit, end, close) = cfg_attr_path(c, j + 8)?;
        (lit, end, skip_blank(c, close))
    } else {
        return None;
    };
    if c.get(j) != Some(&']') {
        return None;
    }
    j = skip_blank(c, j + 1);
    loop {
        if c.get(j) == Some(&'#') {
            j = skip_blank(c, group(c, skip_blank(c, j + 1), '[', ']')?);
        } else if word_at(c, j, "pub") {
            j = skip_blank(c, j + 3);
            if c.get(j) == Some(&'(') {
                j = skip_blank(c, group(c, j, '(', ')')?);
            }
        } else {
            break;
        }
    }
    word_at(c, j, "mod").then_some((lit, end))
}

/// The literal and end of `path = "<literal>"` that starts at `i` (the word `path`).
fn path_value(c: &[char], i: usize) -> Option<(Option<String>, usize)> {
    let mut j = skip_blank(c, i + 4);
    if c.get(j) != Some(&'=') {
        return None;
    }
    j = skip_blank(c, j + 1);
    literal(c, j)
}

/// The first `path = "<literal>"` item after the predicate in the parentheses of a `cfg_attr` that
/// start at or after `i`: the literal, its end and the index after the closing parenthesis.
fn cfg_attr_path(c: &[char], i: usize) -> Option<(Option<String>, usize, usize)> {
    let mut j = skip_blank(c, i);
    if c.get(j) != Some(&'(') {
        return None;
    }
    j += 1;
    let mut depth = 0usize;
    let mut found: Option<(Option<String>, usize)> = None;
    let mut commas = 0;
    let mut item = true;
    while j < c.len() {
        let ch = c[j];
        if ch == '"' || (ch == 'r' && literal(c, j).is_some()) {
            // A string inside the predicate or another attribute: skipped as one token.
            j = literal(c, j).map_or(j + 1, |(_, end)| end);
            item = false;
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => {
                return found.map(|(lit, end)| (lit, end, j + 1));
            }
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                commas += 1;
                item = true;
                j += 1;
                continue;
            }
            _ => {}
        }
        if item
            && depth == 0
            && commas > 0
            && word_at(c, j, "path")
            && (j == 0 || !is_word(c[j - 1]))
            && found.is_none()
            && let Some((lit, end)) = path_value(c, j)
        {
            found = Some((lit, end));
            j = end;
            item = false;
            continue;
        }
        if !ch.is_whitespace() {
            item = false;
        }
        j += 1;
    }
    None
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
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
