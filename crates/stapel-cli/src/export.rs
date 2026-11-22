//! `stapel export [KEY] [--onto <ref>]` (STP-31 AC-15, AC-16): writes the branch `export/<KEY>`, the
//! commits of `<onto>..<KEY>` without the process paths, built on git plumbing. No working tree,
//! index or `HEAD` of the checkout is touched.

use crate::fail::Fail;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The paths that `git rm` removes from each tree; the agents are a glob.
const REMOVE: [&str; 5] = [
    ".stapel",
    ".claude/skills/stapel",
    ".claude/skills/stapel-elaborate",
    ".claude/settings.json",
    ":(glob).claude/agents/stapel-*.md",
];

pub fn run(key: Option<&str>, onto: Option<&str>) -> Result<(), Fail> {
    let here = std::env::current_dir().map_err(|e| Fail::failed(e.to_string()))?;
    // Every git call runs at the top of the work tree, so the pathspecs are the same from a subfolder.
    let cwd = git(&here, &["rev-parse", "--show-toplevel"], &[], b"")
        .map(|top| PathBuf::from(text(top)))
        .map_err(Fail::failed)?;
    let key = match key {
        Some(key) => key.to_string(),
        None => stapel_core::parallel::current_branch(&cwd)
            .ok_or_else(|| Fail::failed("no ticket key and no current branch; name the ticket: stapel export KEY"))?,
    };
    // `--branch` expands `@{-1}`; the name must come back as it went in.
    let valid = git(&cwd, &["check-ref-format", "--branch", &key], &[], b"")
        .map(|out| text(out) == key)
        .unwrap_or(false);
    if !valid {
        return Err(Fail::failed(format!("{key} is not a valid branch name")));
    }
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
    let done = export(&cwd, &key, &onto).map_err(Fail::failed)?;
    say!(
        "export/{key} {}: commits {}, dropped {}, onto {}",
        done.tip,
        done.commits,
        done.dropped,
        done.onto
    );
    if let Some(remote) = &done.push_remote {
        say!("push: git push {remote} export/{key}:{key}");
    }
    if let Some(path) = write_merge_request(&cwd, &key).map_err(Fail::failed)? {
        say!("merge request: {path}");
    }
    Ok(())
}

/// STP-38 AC-7: writes `.stapel/tickets/<key>/merge-request.md` in the work tree at `top`; `None`
/// when the work tree holds no folder of the ticket (an export from the base checkout).
fn write_merge_request(top: &Path, key: &str) -> Result<Option<String>, String> {
    use stapel_core::journal::{self, Entry};
    let rel = format!(".stapel/tickets/{key}");
    let folder = top.join(&rel);
    let state_path = folder.join("state.json");
    if !state_path.is_file() {
        return Ok(None);
    }
    let title = match stapel_core::state::load(&state_path) {
        stapel_core::state::Loaded::State(state) => state.title,
        stapel_core::state::Loaded::Legacy(value) => value
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string(),
        stapel_core::state::Loaded::Unreadable(reason) => return Err(reason),
    };
    let summary = match std::fs::symlink_metadata(folder.join("ticket.md")) {
        Ok(_) => {
            let text = stapel_core::ticket::read(&folder.join("ticket.md"))?;
            let sections = stapel_core::ticket::parse(&text);
            stapel_core::ticket::find(&sections, "Summary")
                .found()
                .unwrap_or("")
                .to_string()
        }
        Err(_) => String::new(),
    };
    let records: Vec<_> = journal::read(&folder.join("findings.jsonl"))?
        .into_iter()
        .filter_map(|(_, e)| match e {
            Entry::V1(m) | Entry::Legacy(m) => Some(m),
            Entry::Problem(_) => None,
        })
        .collect();
    let field = |m: &serde_json::Map<String, serde_json::Value>, name: &str| {
        m.get(name)
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default()
    };
    let mut seen: Vec<String> = Vec::new();
    let mut items = String::new();
    for m in records.iter().filter(|m| !m.contains_key("kind")) {
        let Some(id) = m.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        if seen.iter().any(|s| s == id) {
            continue;
        }
        seen.push(id.to_string());
        let state = stapel_core::triage::state_of(&records, id);
        let Some(finding) = state.finding.as_ref() else {
            continue;
        };
        if state.fate.as_deref() != Some("defer") || state.verdict.as_deref() == Some("false") {
            continue;
        }
        items.push_str(&format!(
            "- {id} {} {} round {}: {}\n",
            state.severity.as_deref().unwrap_or(""),
            field(finding, "stage"),
            field(finding, "round"),
            field(finding, "claim").replace('\n', " "),
        ));
    }
    if items.is_empty() {
        items.push_str("none\n");
    }
    let body = summary.trim_matches('\n');
    let text = format!(
        "# {key} \u{2014} {title}\n\n{body}\n\n## Deferred findings\n\n{items}",
        body = body.trim_end()
    );
    let path = folder.join("merge-request.md");
    let temp = folder.join(format!(".merge-request.md.{}.tmp", std::process::id()));
    std::fs::write(&temp, text)
        .and_then(|()| std::fs::rename(&temp, &path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            format!("{}: {e}", path.display())
        })?;
    Ok(Some(format!("{rel}/merge-request.md")))
}

/// What an export wrote.
struct Done {
    tip: String,
    commits: usize,
    dropped: usize,
    /// The full name of the Onto ref when it has one, else the value given.
    onto: String,
    /// The configured remote that holds a remote Onto ref.
    push_remote: Option<String>,
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
    // An empty name would read as the current branch.
    let name = if base.is_empty() {
        String::new()
    } else {
        git(
            dir,
            &[
                "rev-parse",
                "--symbolic-full-name",
                "--end-of-options",
                &format!("{base}@{{upstream}}"),
            ],
            &[],
            b"",
        )
        .map(text)
        .unwrap_or_default()
        .lines()
        // rev-parse echoes `--end-of-options`; the name is the last line.
        .filter(|l| *l != "--end-of-options")
        .next_back()
        .unwrap_or("")
        .to_string()
    };
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

/// Runs git in `dir`; returns the captured output.
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
        || path.starts_with(".claude/skills/stapel-elaborate/")
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
            ".claude/skills/stapel-elaborate",
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

/// The raw commit object with its `tree` and `parent` lines replaced and the `gpgsig` and `mergetag`
/// headers (with their continuation lines) removed; every other byte stays.
fn rewrite_raw(raw: &[u8], tree: &str, parents: &[String]) -> Vec<u8> {
    let split = raw.windows(2).position(|w| w == b"\n\n").unwrap_or(raw.len());
    let (header, rest) = raw.split_at(split);
    let mut lines: Vec<Vec<u8>> = vec![format!("tree {tree}").into_bytes()];
    for parent in parents {
        lines.push(format!("parent {parent}").into_bytes());
    }
    let mut skipping = false;
    for line in header.split(|b| *b == b'\n') {
        if skipping && line.starts_with(b" ") {
            continue;
        }
        skipping = line.starts_with(b"gpgsig ")
            || line.starts_with(b"gpgsig-sha256 ")
            || line.starts_with(b"mergetag ");
        if skipping || line.starts_with(b"tree ") || line.starts_with(b"parent ") {
            continue;
        }
        lines.push(line.to_vec());
    }
    let mut out = lines.join(&b'\n');
    out.extend_from_slice(rest);
    out
}

/// The Clean tree of a commit: its tree without the process paths, those paths as they are in the
/// Onto commit. The Clean tree of a commit that is already clean is its own tree.
fn clean_tree(
    dir: &Path,
    commit: &str,
    process_entries: &[u8],
    index_env: &[(&str, OsString)],
) -> Result<String, String> {
    git(dir, &["read-tree", commit], index_env, b"")?;
    git(
        dir,
        &["rm", "--cached", "-r", "-f", "-q", "--ignore-unmatch", "--"]
            .into_iter()
            .chain(REMOVE)
            .collect::<Vec<_>>(),
        index_env,
        b"",
    )?;
    if !process_entries.is_empty() {
        git(
            dir,
            &["update-index", "-z", "--index-info"],
            index_env,
            process_entries,
        )?;
    }
    Ok(text(git(dir, &["write-tree"], index_env, b"")?))
}

fn export(dir: &Path, key: &str, onto: &str) -> Result<Done, String> {
    let onto_commit = git(
        dir,
        &["rev-parse", "--verify", "--end-of-options", &format!("{onto}^{{commit}}")],
        &[],
        b"",
    )
    .map(text)
    .map_err(|_| format!("the Onto ref {onto} names no commit"))?;
    let onto_name = git(
        dir,
        &["rev-parse", "--symbolic-full-name", "--end-of-options", onto],
        &[],
        b"",
    )
    .map(text)
    .ok()
    .and_then(|t| {
        t.lines()
            .find(|l| *l != "--end-of-options")
            .map(String::from)
    })
    .filter(|n| !n.is_empty())
    .unwrap_or_else(|| onto.to_string());
    let source = format!("refs/heads/{key}");
    git(
        dir,
        &["rev-parse", "--verify", "--end-of-options", &source],
        &[],
        b"",
    )?;
    let in_onto_tree = git(dir, &["ls-tree", &onto_commit, "--", ".stapel"], &[], b"")?;
    if !in_onto_tree.is_empty() {
        return Err(format!("the tree of the Onto ref {onto} has .stapel"));
    }
    git(dir, &["merge-base", &onto_commit, &source], &[], b"")
        .map_err(|_| format!("the Onto ref {onto} shares no common commit with {key}"))?;
    let target = format!("refs/heads/export/{key}");
    let worktrees = text(git(dir, &["worktree", "list", "--porcelain"], &[], b"")?);
    if worktrees
        .lines()
        .any(|l| l.strip_prefix("branch ").is_some_and(|b| b == target))
    {
        return Err(format!("export/{key} is checked out in a worktree"));
    }
    let range = format!("{onto_commit}..{source}");
    let listing = text(git(
        dir,
        &["rev-list", "--topo-order", "--reverse", "--parents", &range],
        &[],
        b"",
    )?);
    if listing.is_empty() {
        return Err(format!("the Export range {onto}..{key} is empty"));
    }
    if let Some(root) = listing.lines().find(|l| l.split_whitespace().count() == 1) {
        return Err(format!(
            "commit {} of the Export range has no parents",
            root.trim()
        ));
    }
    let total = listing.lines().count();
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

        let tree = clean_tree(dir, commit, &process_entries, &index_env)?;

        let drop_for = if had_parents >= 1 && parents.len() == 1 {
            let parent_tree = clean_tree(dir, &parents[0], &process_entries, &index_env)?;
            (parent_tree == tree).then(|| parents[0].clone())
        } else if parents.len() == 2 {
            // A merge made by `git pull`: one parent is an ancestor of the other, and the merge adds
            // nothing to the later one.
            let is_ancestor = |old: &str, new: &str| {
                git(dir, &["merge-base", "--is-ancestor", old, new], &[], b"").is_ok()
            };
            let later = if is_ancestor(&parents[0], &parents[1]) {
                Some(&parents[1])
            } else if is_ancestor(&parents[1], &parents[0]) {
                Some(&parents[0])
            } else {
                None
            };
            match later {
                Some(later) => {
                    let later_tree = clean_tree(dir, later, &process_entries, &index_env)?;
                    (later_tree == tree).then(|| later.clone())
                }
                None => None,
            }
        } else {
            None
        };
        if let Some(parent) = drop_for {
            replaced.insert(commit.to_string(), parent);
            last = Some(commit.to_string());
            continue;
        }

        let raw = git(dir, &["cat-file", "commit", commit], &[], b"")?;
        let rewritten = rewrite_raw(&raw, &tree, &parents);
        let new = text(git(
            dir,
            &["hash-object", "-t", "commit", "-w", "--literally", "--stdin"],
            &[],
            &rewritten,
        )?);
        made.push(new.clone());
        replaced.insert(commit.to_string(), new);
        last = Some(commit.to_string());
    }

    let tip = last
        .and_then(|c| replaced.get(&c).cloned())
        .filter(|t| made.contains(t))
        .ok_or("every commit of the Export range is dropped: nothing is left to write")?;
    // The remote list is read before the ref is written: `update-ref` is the last git call.
    let push_remote = onto_name.strip_prefix("refs/remotes/").and_then(|rest| {
        let remotes = git(dir, &["remote"], &[], b"").map(text).unwrap_or_default();
        remotes
            .lines()
            .filter(|r| rest.strip_prefix(*r).is_some_and(|tail| tail.starts_with('/')))
            .max_by_key(|r| r.len())
            .map(str::to_string)
    });
    git(
        dir,
        &["update-ref", "-m", "stapel export", &target, &tip],
        &[],
        b"",
    )?;
    Ok(Done {
        tip,
        commits: made.len(),
        dropped: total - made.len(),
        onto: onto_name,
        push_remote,
    })
}
