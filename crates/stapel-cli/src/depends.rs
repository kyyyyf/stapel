//! `stapel depends [KEY]` (STP-27 AC-20): the pack of the dependency review of a ticket against
//! each earlier parallel ticket. The agent `stapel-depends` reads only; the pack holds what it needs:
//! the Frame, criteria, Plan and named files of both tickets and the earlier ticket's diff against
//! the base branch. Writes only under `.stapel/scratch/depends/`.

use crate::fail::Fail;
use crate::repo;
use stapel_core::brief::frame_of;
use stapel_core::depends::{Pair, earlier_pairs, named_files};
use stapel_core::review::criteria;
use stapel_core::ticket::{Lookup, Section, find, parse, read};
use stapel_core::tickets::{Status, Ticket, match_branch, resolve};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The most bytes of one named file in a pack.
const FILE_LIMIT: u64 = 64 * 1024;

pub fn run(key: Option<&str>) -> Result<(), Fail> {
    let (root, _config) = repo::open().map_err(Fail::failed)?;
    match_branch(&root, key).map_err(Fail::refused)?;
    let own = resolve(&root, key).map_err(Fail::refused)?;
    match &own.status {
        Status::Open(_) => {}
        Status::Closed(_) => {
            return Err(Fail::refused(format!(
                "ticket {} is closed; it has no dependency review",
                own.key
            )));
        }
        Status::Legacy(_) => {
            return Err(Fail::failed(format!(
                "ticket {} is in the STP-1 format; it has no dependency review",
                own.key
            )));
        }
        Status::Unreadable(reason) => return Err(Fail::failed(reason.clone())),
    }
    let pairs = earlier_pairs(&root, &own);
    if pairs.is_empty() {
        return Err(Fail::refused(format!(
            "{} has no earlier parallel ticket",
            own.key
        )));
    }
    for pair in &pairs {
        let relative = format!(".stapel/scratch/depends/{}-{}", own.key, pair.earlier.key);
        write_pack(&root, &own, pair, &relative)?;
        say!("pack: {relative}/pack.md");
        say!("round: {}", pair.round);
    }
    Ok(())
}

fn io(path: &Path, e: std::io::Error) -> Fail {
    Fail::failed(format!("{}: {e}", path.display()))
}

/// The checkout of a ticket: the folder above its `.stapel/tickets/<KEY>`.
fn root_of(ticket: &Ticket) -> Option<PathBuf> {
    ticket
        .dir
        .parent()?
        .parent()?
        .parent()
        .map(Path::to_path_buf)
}

fn body<'a>(sections: &'a [Section], title: &str) -> &'a str {
    match find(sections, title) {
        Lookup::Found(body) => body,
        _ => "",
    }
}

/// `text` in a fenced block whose fence is longer than any run of backticks in it.
fn fenced(text: &str, lang: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}{lang}\n{}\n{fence}\n", text.trim_end_matches('\n'))
}

/// The text of the named file `rel` under `checkout`, cut at `FILE_LIMIT` bytes; a note when the file
/// is missing, outside the checkout or not text.
fn named_file(checkout: &Path, rel: &str) -> String {
    let path = checkout.join(rel);
    let inside = match (path.canonicalize(), checkout.canonicalize()) {
        (Ok(p), Ok(c)) => p.starts_with(c) && p.is_file(),
        _ => false,
    };
    if !inside {
        return "(no such file in this checkout)\n".to_string();
    }
    let Ok(file) = std::fs::File::open(&path) else {
        return "(cannot be read)\n".to_string();
    };
    let mut bytes = Vec::new();
    use std::io::Read;
    if file.take(FILE_LIMIT + 1).read_to_end(&mut bytes).is_err() {
        return "(cannot be read)\n".to_string();
    }
    let cut = bytes.len() as u64 > FILE_LIMIT;
    bytes.truncate(FILE_LIMIT as usize);
    let Ok(text) = String::from_utf8(bytes) else {
        return "(not UTF-8 text)\n".to_string();
    };
    let mut out = fenced(&text, "");
    if cut {
        out.push_str(&format!("(cut after {FILE_LIMIT} bytes)\n"));
    }
    out
}

/// The part of the pack that describes one ticket.
fn ticket_part(ticket: &Ticket, heading: &str) -> Result<String, Fail> {
    let text = read(&ticket.dir.join("ticket.md")).map_err(Fail::failed)?;
    let sections = parse(&text);
    let checkout = root_of(ticket);
    let plan = body(&sections, "Plan");
    let mut out = format!("## {} ({heading})\n\n", ticket.key);
    if let Some(checkout) = &checkout {
        out.push_str(&format!("- checkout: {}\n\n", checkout.display()));
    }
    out.push_str(&format!(
        "### Frame\n\n{}\n\n",
        frame_of(body(&sections, "Description")).trim()
    ));
    out.push_str("### Criteria\n\n");
    let list = criteria(body(&sections, "Spec"));
    if list.is_empty() {
        out.push_str("- none\n");
    }
    for item in list {
        out.push_str(&format!("- {item}\n"));
    }
    out.push_str(&format!("\n### Plan\n\n{}\n\n### Named files\n\n", plan.trim()));
    let files = named_files(plan);
    if files.is_empty() {
        out.push_str("- none\n\n");
    }
    for rel in files {
        let content = match &checkout {
            Some(checkout) => named_file(checkout, &rel),
            None => "(no checkout)\n".to_string(),
        };
        out.push_str(&format!("#### {rel}\n\n{content}\n"));
    }
    Ok(out)
}

/// `git diff` of the earlier ticket's branch against the base branch, from the merge base.
fn earlier_diff(root: &Path, own: &Ticket, pair: &Pair) -> Result<(String, String), Fail> {
    let base_of = |t: &Ticket| match &t.status {
        Status::Open(s) | Status::Closed(s) => s.base_branch.clone(),
        _ => None,
    };
    let Some(base) = base_of(own).or_else(|| base_of(&pair.earlier)) else {
        return Err(Fail::failed(format!(
            "state.json of {} has no base branch",
            own.key
        )));
    };
    let range = format!("refs/heads/{base}...refs/heads/{}", pair.branch);
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
        .arg(&range)
        .current_dir(root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .map_err(|e| Fail::failed(format!("git diff: {e}")))?;
    if !out.status.success() {
        return Err(Fail::failed(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok((base, String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// Writes `pack.md` of the pair into `relative` under `root`.
fn write_pack(root: &Path, own: &Ticket, pair: &Pair, relative: &str) -> Result<(), Fail> {
    let (base, diff) = earlier_diff(root, own, pair)?;
    let mut pack = format!(
        "# Dependency pack {later} after {earlier}\n\n- later: {later}\n- earlier: {earlier}\n\
         - round: {round}\n- base branch: {base}\n- earlier branch: {branch}\n\n\
         Find what {later} needs from {earlier} or what the two change together: a shared file, \
         a changed signature, a changed rule, an order of merges. Return the records of stage \
         `dependency`, role `depends`; a HIGH or MEDIUM record names `waits` as `spec` (the \
         later spec must wait for the earlier one) or `merge` (the later code needs the earlier \
         code merged). With none, return one LOW record with the claim `no dependency found`.\n\n",
        later = own.key,
        earlier = pair.earlier.key,
        round = pair.round,
        branch = pair.branch,
    );
    pack.push_str(&ticket_part(&pair.earlier, "earlier")?);
    pack.push('\n');
    pack.push_str(&ticket_part(own, "later")?);
    pack.push_str(&format!(
        "\n## Diff of {} against {base}\n\n{}",
        pair.earlier.key,
        if diff.trim().is_empty() {
            "(no change)\n".to_string()
        } else {
            fenced(&diff, "diff")
        }
    ));

    let scratch = root.join(".stapel/scratch/depends");
    let folder = root.join(relative);
    std::fs::create_dir_all(&folder).map_err(|e| io(&folder, e))?;
    // One ignore file for every pack, so a pack never makes the tree dirty; `.ignore` lets search
    // tools read the packs.
    for (name, text) in [(".gitignore", "*\n"), (".ignore", "!*\n")] {
        let path = scratch.join(name);
        std::fs::write(&path, text).map_err(|e| io(&path, e))?;
    }
    let path = folder.join("pack.md");
    std::fs::write(&path, pack).map_err(|e| io(&path, e))
}
