//! `stapel review start KEY --round N`: the refusals before a code review round (STP-24 AC-1, AC-2).
//! A success writes the copy of HEAD (AC-3) and prints nothing yet.

use crate::check::read_head;
use crate::fail::Fail;
use crate::repo;
use stapel_core::build_inputs::check_base;
use stapel_core::journal::{self, Entry};
use stapel_core::tickets::{Status, resolve};
use stapel_core::triage::whole_round;
use stapel_core::worktree::dirty_paths;
use std::path::Path;
use std::process::Command;

pub fn start(key: &str, round: u64) -> Result<(), Fail> {
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    let ticket = resolve(&root, Some(key)).map_err(Fail::refused)?;
    let state = match &ticket.status {
        Status::Open(state) => state,
        Status::Closed(_) => {
            return Err(Fail::refused(format!(
                "ticket {} is closed; it has no review round",
                ticket.key
            )));
        }
        Status::Legacy(_) => {
            return Err(Fail::failed(format!(
                "ticket {} is in the STP-1 format; it has no review pack",
                ticket.key
            )));
        }
        Status::Unreadable(reason) => return Err(Fail::failed(reason.clone())),
    };
    if !(1..=2).contains(&round) {
        return Err(Fail::refused(
            "--round must be 1 or 2; after round 2 the person decides",
        ));
    }
    let dirty = dirty_paths(&root).map_err(Fail::failed)?;
    if !dirty.is_empty() {
        let first: Vec<&str> = dirty.iter().take(5).map(String::as_str).collect();
        return Err(Fail::refused(format!(
            "uncommitted changes: {}; commit them before a review round",
            first.join(", ")
        )));
    }
    let head = read_head(&root).map_err(Fail::failed)?;
    let Some(base) = &state.base else {
        return Err(Fail::failed(format!(
            "state.json of {} has no base commit",
            ticket.key
        )));
    };
    check_base(&root, base, &head).map_err(Fail::failed)?;
    if round == 2 {
        let path = ticket.dir.join("findings.jsonl");
        let entries = journal::read(&path).map_err(Fail::failed)?;
        let has_round_one = entries.into_iter().any(|(_, entry)| match entry {
            Entry::V1(map) | Entry::Legacy(map) => {
                map.get("stage").and_then(|s| s.as_str()) == Some("code-review")
                    && map.get("round").and_then(whole_round) == Some(1)
            }
            Entry::Problem(_) => false,
        });
        if !has_round_one {
            return Err(Fail::refused(format!(
                "round 2 needs the code review findings of round 1 in \
                 .stapel/tickets/{}/findings.jsonl",
                ticket.key
            )));
        }
    }
    copy_head(&root, &ticket.key, round)
}

/// Replaces `.stapel/scratch/review/<KEY>-r<N>/tree/` with the files of HEAD as a checkout with
/// `core.autocrlf=false` writes them: a temporary index of the folder, `read-tree`, `checkout-index`.
/// The index of the repository is not touched.
fn copy_head(root: &Path, key: &str, round: u64) -> Result<(), Fail> {
    let review = root.join(".stapel/scratch/review");
    let folder = review.join(format!("{key}-r{round}"));
    let tree = folder.join("tree");
    let io = |path: &Path, e: std::io::Error| Fail::failed(format!("{}: {e}", path.display()));
    if folder.exists() {
        std::fs::remove_dir_all(&folder).map_err(|e| io(&folder, e))?;
    }
    std::fs::create_dir_all(&tree).map_err(|e| io(&tree, e))?;
    // One ignore file for every copy, so a review copy never makes the tree dirty.
    let ignore = review.join(".gitignore");
    std::fs::write(&ignore, "*\n").map_err(|e| io(&ignore, e))?;
    let index = folder.join("index");
    let mut prefix = tree.clone().into_os_string();
    prefix.push("/");
    let steps: [(&str, Vec<&std::ffi::OsStr>); 2] = [
        ("read-tree", vec!["read-tree".as_ref(), "HEAD".as_ref()]),
        (
            "checkout-index",
            vec![
                "checkout-index".as_ref(),
                "-a".as_ref(),
                "--prefix".as_ref(),
                prefix.as_os_str(),
            ],
        ),
    ];
    // `--prefix <value>` as two arguments keeps a path with spaces whole.
    for (name, args) in steps {
        let out = Command::new("git")
            .args(["-c", "core.autocrlf=false"])
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .current_dir(root)
            .output()
            .map_err(|e| Fail::failed(format!("git {name}: {e}")))?;
        if !out.status.success() {
            return Err(Fail::failed(format!(
                "git {name} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
    }
    std::fs::remove_file(&index).map_err(|e| io(&index, e))?;
    Ok(())
}
