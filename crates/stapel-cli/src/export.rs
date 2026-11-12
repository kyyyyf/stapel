//! `stapel export [KEY] [--onto <ref>]` (STP-31 AC-15, AC-16): writes the branch `export/<KEY>`, the
//! commits of `<onto>..<KEY>` without the process paths, built on git plumbing. No working tree,
//! index or `HEAD` of the checkout is touched.

use crate::fail::Fail;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The paths that `git rm` removes from each tree; the agents are a glob.
const REMOVE: [&str; 4] = [
    ".stapel",
    ".claude/skills/stapel",
    ".claude/settings.json",
    ":(glob).claude/agents/stapel-*.md",
];

pub fn run(key: Option<&str>, onto: Option<&str>) -> Result<(), Fail> {
    let cwd = std::env::current_dir().map_err(|e| Fail::failed(e.to_string()))?;
    let key = match key {
        Some(key) => key.to_string(),
        None => stapel_core::parallel::current_branch(&cwd)
            .ok_or_else(|| Fail::failed("no ticket key and no current branch; name the ticket: stapel export KEY"))?,
    };
    if let Some(root) = stapel_core::guard::stapel_root(&cwd) {
        let config = crate::repo::load(&root).map_err(Fail::failed)?;
        if stapel_core::tickets::key_number(&config, &key).is_none() {
            return Err(Fail::failed(format!(
                "{key} does not match the ticket key pattern {} of stapel.toml",
                config.tickets.key
            )));
        }
    }
    let base_branch = branch_state(&cwd, &key).map_err(Fail::failed)?;
    let onto = match onto {
        Some(onto) => onto.to_string(),
        None => upstream_of(&cwd, base_branch.as_deref()).map_err(Fail::failed)?,
    };
    export(&cwd, &key, &onto).map_err(Fail::failed)
}

/// The `base_branch` of the Branch state of `key`: `.stapel/tickets/<key>/state.json` as it is in
/// the commit `refs/heads/<key>`.
fn branch_state(dir: &Path, key: &str) -> Result<Option<String>, String> {
    let source = format!("refs/heads/{key}");
    git(dir, &["rev-parse", "--verify", "--end-of-options", &source], &[], b"")
        .map_err(|_| format!("no branch {key}"))?;
    let path = format!(".stapel/tickets/{key}/state.json");
    let raw = git(dir, &["show", &format!("{source}:{path}")], &[], b"")
        .map_err(|_| format!("branch {key} has no {path}"))?;
    let value: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| format!("{path} in branch {key} is not valid JSON: {e}"))?;
    if !value.is_object() {
        return Err(format!("{path} in branch {key} is not a JSON object"));
    }
    Ok(value
        .get("base_branch")
        .and_then(|b| b.as_str())
        .map(String::from))
}

/// The full name of the upstream of `base_branch`, the Onto ref when `--onto` is absent.
fn upstream_of(dir: &Path, base_branch: Option<&str>) -> Result<String, String> {
    let base = base_branch.ok_or("the Branch state has no base_branch; name the Onto ref with --onto <ref>")?;
    let name = git(
        dir,
        &["for-each-ref", "--format=%(upstream)", &format!("refs/heads/{base}")],
        &[],
        b"",
    )
    .map(text)
    .unwrap_or_default()
    .lines()
    .next()
    .unwrap_or("")
    .to_string();
    if name.is_empty() {
        return Err(format!(
            "branch {base} has no upstream; name the Onto ref with --onto <ref>"
        ));
    }
    Ok(name)
}

/// A folder of the system's temporary folder that goes away with the value.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "stapel-export-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Scratch(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs git in `dir`; returns stdout.
fn git(
    dir: &Path,
    args: &[&str],
    env: &[(&str, OsString)],
    input: &[u8],
) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().map_err(|e| format!("git: {e}"))?;
    let mut stdin = child.stdin.take().expect("piped");
    let bytes = input.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&bytes);
    });
    let out = child.wait_with_output().map_err(|e| format!("git: {e}"))?;
    let _ = writer.join();
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim().to_string()
}

/// Whether a tree path is a process path.
fn is_process_path(path: &[u8]) -> bool {
    let path = String::from_utf8_lossy(path);
    path.starts_with(".stapel/")
        || path.starts_with(".claude/skills/stapel/")
        || path == ".claude/settings.json"
        || path.strip_prefix(".claude/agents/").is_some_and(|name| {
            name.starts_with("stapel-") && name.ends_with(".md") && !name.contains('/')
        })
}

/// The `ls-tree -r -z` entries of the Onto commit that lie on a process path.
fn onto_entries(dir: &Path, onto: &str) -> Result<Vec<u8>, String> {
    let listing = git(
        dir,
        &[
            "ls-tree",
            "-r",
            "-z",
            onto,
            "--",
            ".stapel",
            ".claude/skills/stapel",
            ".claude/settings.json",
            ".claude/agents",
        ],
        &[],
        b"",
    )?;
    let mut kept = Vec::new();
    for entry in listing.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let Some(tab) = entry.iter().position(|b| *b == b'\t') else {
            continue;
        };
        if is_process_path(&entry[tab + 1..]) {
            kept.extend_from_slice(entry);
            kept.push(0);
        }
    }
    Ok(kept)
}

/// The value of a header line (`author`, `committer`) split into name, email and date.
fn ident(header: &[u8]) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let open = header.iter().position(|b| *b == b'<')?;
    let close = open + header[open..].iter().position(|b| *b == b'>')?;
    let name = header[..open].strip_suffix(b" ").unwrap_or(&header[..open]);
    let date = header[close + 1..]
        .strip_prefix(b" ")
        .unwrap_or(&header[close + 1..]);
    Some((name.to_vec(), header[open + 1..close].to_vec(), date.to_vec()))
}

fn export(dir: &Path, key: &str, onto: &str) -> Result<(), String> {
    let onto_commit = text(git(
        dir,
        &["rev-parse", "--verify", "--end-of-options", &format!("{onto}^{{commit}}")],
        &[],
        b"",
    )?);
    let source = format!("refs/heads/{key}");
    git(
        dir,
        &["rev-parse", "--verify", "--end-of-options", &source],
        &[],
        b"",
    )?;
    let range = format!("{onto_commit}..{source}");
    let listing = text(git(
        dir,
        &["rev-list", "--topo-order", "--reverse", "--parents", &range],
        &[],
        b"",
    )?);
    let process_entries = onto_entries(dir, &onto_commit)?;

    let scratch = Scratch::new()?;
    let index = scratch.0.join("index");
    let index_env = [("GIT_INDEX_FILE", index.clone().into_os_string())];

    // Each commit of the range to the commit that stands in its place.
    let mut replaced: HashMap<String, String> = HashMap::new();
    let mut made: Vec<String> = Vec::new();
    let mut last: Option<String> = None;
    for line in listing.lines() {
        let mut ids = line.split_whitespace();
        let Some(commit) = ids.next() else { continue };
        let mut parents: Vec<String> = Vec::new();
        for parent in ids {
            let mapped = replaced.get(parent).map_or(parent, String::as_str);
            if !parents.iter().any(|p| p == mapped) {
                parents.push(mapped.to_string());
            }
        }
        let had_parents = line.split_whitespace().count() - 1;

        git(dir, &["read-tree", commit], &index_env, b"")?;
        git(
            dir,
            &["rm", "--cached", "-r", "-f", "-q", "--ignore-unmatch", "--"]
                .into_iter()
                .chain(REMOVE)
                .collect::<Vec<_>>(),
            &index_env,
            b"",
        )?;
        if !process_entries.is_empty() {
            git(
                dir,
                &["update-index", "-z", "--index-info"],
                &index_env,
                &process_entries,
            )?;
        }
        let tree = text(git(dir, &["write-tree"], &index_env, b"")?);

        let drop_for = if had_parents >= 2 && parents.len() == 1 {
            Some(parents[0].clone())
        } else if had_parents == 1 {
            let parent_tree = text(git(
                dir,
                &["rev-parse", "--verify", "--end-of-options", &format!("{}^{{tree}}", parents[0])],
                &[],
                b"",
            )?);
            (parent_tree == tree).then(|| parents[0].clone())
        } else {
            None
        };
        if let Some(parent) = drop_for {
            replaced.insert(commit.to_string(), parent);
            last = Some(commit.to_string());
            continue;
        }

        let raw = git(dir, &["cat-file", "commit", commit], &[], b"")?;
        let split = raw
            .windows(2)
            .position(|w| w == b"\n\n")
            .map(|i| (&raw[..i], &raw[i + 2..]))
            .unwrap_or((&raw[..], &raw[raw.len()..]));
        let (header, message) = split;
        let mut env: Vec<(&str, OsString)> = Vec::new();
        for line in header.split(|b| *b == b'\n') {
            for (prefix, vars) in [
                (&b"author "[..], ["GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_AUTHOR_DATE"]),
                (&b"committer "[..], [
                    "GIT_COMMITTER_NAME",
                    "GIT_COMMITTER_EMAIL",
                    "GIT_COMMITTER_DATE",
                ]),
            ] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    let (name, email, date) =
                        ident(rest).ok_or_else(|| format!("commit {commit}: bad {} line", String::from_utf8_lossy(prefix).trim()))?;
                    env.push((vars[0], OsString::from_vec(name)));
                    env.push((vars[1], OsString::from_vec(email)));
                    env.push((vars[2], OsString::from_vec(date)));
                }
            }
        }
        let mut args: Vec<&str> = vec!["commit-tree", &tree];
        for parent in &parents {
            args.push("-p");
            args.push(parent);
        }
        let new = text(git(dir, &args, &env, message)?);
        made.push(new.clone());
        replaced.insert(commit.to_string(), new);
        last = Some(commit.to_string());
    }

    let tip = last
        .and_then(|c| replaced.get(&c).cloned())
        .filter(|t| made.contains(t))
        .ok_or("the Export range holds no commit to write")?;
    git(
        dir,
        &[
            "update-ref",
            "-m",
            "stapel export",
            &format!("refs/heads/export/{key}"),
            &tip,
        ],
        &[],
        b"",
    )?;
    Ok(())
}
