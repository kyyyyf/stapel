//! `stapel review start KEY --round N` (STP-24): refuses an unknown, closed or legacy ticket, a bad
//! round, a dirty tree, a bad base commit and a round 2 without round 1 findings or with an
//! unreadable `state.json` (AC-1, AC-2). A success removes the review folders of other rounds and of
//! closed tickets, writes the copy of HEAD under `tree/` with the `.gitignore` and `.ignore` of the
//! review folder (AC-3), writes `diff.patch` and `pack.md` (AC-4, AC-5), and prints one line per
//! reviewer (AC-6).

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
    let (root, config) = repo::open().map_err(Fail::failed)?;
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
    let ticket_text =
        stapel_core::ticket::read(&ticket.dir.join("ticket.md")).map_err(Fail::failed)?;
    let ticket_sections = stapel_core::ticket::parse(&ticket_text);
    let has_criteria = match stapel_core::ticket::find(&ticket_sections, "Spec") {
        stapel_core::ticket::Lookup::Found(spec) => !stapel_core::review::criteria(spec).is_empty(),
        _ => false,
    };
    // AC-10 prints `stapel: the Spec of <KEY> has no criteria; a review pack needs them`; `Fail` adds
    // the `stapel: ` prefix.
    if !has_criteria {
        return Err(Fail::refused(format!(
            "the Spec of {} has no criteria; a review pack needs them",
            ticket.key
        )));
    }
    remove_old_copies(&root, &ticket.key, round)?;
    copy_head(&root, &ticket.key, round)?;
    write_pack(&root, &ticket.key, &ticket.dir, round, base, &head)?;
    let folder = format!(".stapel/scratch/review/{}-r{round}/", ticket.key);
    say!("copy: {folder}tree (HEAD {})", &head[..head.len().min(12)]);
    say!("pack: {folder}pack.md");
    let text = stapel_core::ticket::read(&ticket.dir.join("ticket.md")).map_err(Fail::failed)?;
    let sections = stapel_core::ticket::parse(&text);
    let body_of = |title: &str| match stapel_core::ticket::find(&sections, title) {
        stapel_core::ticket::Lookup::Found(body) => body,
        _ => "",
    };
    let light = stapel_core::next::is_light(body_of("Plan"));
    let tags = stapel_core::review::frame_tags(body_of("Description"));
    for r in stapel_core::review::reviewers(light, &tags) {
        let model = match config.models.get(r.role) {
            Some(binding) => format!("model {}", binding.model),
            None => "no model".to_string(),
        };
        let tail = if r.whole_ticket {
            ", on the whole ticket"
        } else {
            ""
        };
        say!("reviewer: {}, {model} (role {}){tail}", r.agent, r.role);
    }
    Ok(())
}

/// The output of `git diff` of `base..head` in the form AC-4 pins, with `extra` flags.
fn diff(root: &Path, extra: &[&str], base: &str, head: &str) -> Result<Vec<u8>, Fail> {
    let out = Command::new("git")
        .args(["-c", "core.quotepath=false", "diff"])
        .args([
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--find-renames",
            "--src-prefix=a/",
            "--dst-prefix=b/",
        ])
        .args(extra)
        .arg(format!("{base}..{head}"))
        .current_dir(root)
        .output()
        .map_err(|e| Fail::failed(format!("git diff: {e}")))?;
    if !out.status.success() {
        return Err(Fail::failed(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(out.stdout)
}

/// Writes `diff.patch` and then `pack.md` into the review folder of the round.
fn write_pack(
    root: &Path,
    key: &str,
    ticket_dir: &Path,
    round: u64,
    base: &str,
    head: &str,
) -> Result<(), Fail> {
    let relative = format!(".stapel/scratch/review/{key}-r{round}");
    let folder = root.join(&relative);
    let write = |name: &str, bytes: &[u8]| {
        let path = folder.join(name);
        std::fs::write(&path, bytes).map_err(|e| Fail::failed(format!("{}: {e}", path.display())))
    };
    write("diff.patch", &diff(root, &[], base, head)?)?;
    let stat = diff(root, &["--stat=120", "--stat-graph-width=20"], base, head)?;
    let ticket_md = ticket_dir.join("ticket.md");
    let text = stapel_core::ticket::read(&ticket_md).map_err(Fail::failed)?;
    let sections = stapel_core::ticket::parse(&text);
    let spec = match stapel_core::ticket::find(&sections, "Spec") {
        stapel_core::ticket::Lookup::Found(body) => body,
        _ => "",
    };
    let findings = if round == 2 {
        let entries = journal::read(&ticket_dir.join("findings.jsonl")).map_err(Fail::failed)?;
        let records: Vec<_> = entries
            .into_iter()
            .filter_map(|(_, entry)| match entry {
                Entry::V1(map) | Entry::Legacy(map) => Some(map),
                Entry::Problem(_) => None,
            })
            .collect();
        Some(stapel_core::review::patch_findings(&records))
    } else {
        None
    };
    let pack = stapel_core::review::pack_text(&stapel_core::review::Pack {
        key,
        round,
        base,
        head,
        folder: &relative,
        stat: &String::from_utf8_lossy(&stat),
        spec,
        findings: findings.as_deref(),
    });
    write("pack.md", pack.as_bytes())
}

/// Removes the folders `<KEY>-r<n>` directly under `.stapel/scratch/review/` of this ticket with
/// another round and of closed tickets. A symlink is removed itself, never followed.
fn remove_old_copies(root: &Path, key: &str, round: u64) -> Result<(), Fail> {
    let review = root.join(".stapel/scratch/review");
    let Ok(entries) = std::fs::read_dir(&review) else {
        return Ok(());
    };
    let tickets = stapel_core::tickets::list(root);
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((owner, n)) = name.rsplit_once("-r") else {
            continue;
        };
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let closed = tickets
            .iter()
            .any(|t| t.key == owner && matches!(t.status, Status::Closed(_)));
        if !(closed || (owner == key && n.parse::<u64>().ok() != Some(round))) {
            continue;
        }
        let path = entry.path();
        let is_dir = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_dir());
        let removed = if is_dir {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.map_err(|e| Fail::failed(format!("{}: {e}", path.display())))?;
    }
    Ok(())
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
    // Search tools that read `.ignore` (ripgrep) let `.ignore` override the `.gitignore` above.
    let search = review.join(".ignore");
    std::fs::write(&search, "!*\n").map_err(|e| io(&search, e))?;
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
