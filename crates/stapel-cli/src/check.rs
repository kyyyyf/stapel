//! `stapel check [KEY] [--list | --history]`: the RED to GREEN check of a ticket's steps (STP-4);
//! `--history` prints the outcomes that history alone gives, without cargo (STP-14).

use crate::repo;
use stapel_core::build_inputs;
use stapel_core::build_inputs::{Bounds, BuildInputs, Change};
use stapel_core::config::{Check, validate_cargo, validate_path_cargo};
use stapel_core::journal::{append, new_id};
use stapel_core::junit::{
    MAX_REPORT, OVER_SIZE, Outcome as CaseOutcome, read_open_report, read_report,
};
use stapel_core::outcomes::{
    Analysis, MAX_NAMED, Mode, analyse_in, history_outcome, named, open_red, passing,
};
use stapel_core::runner::{
    Parsed, TestState, cargo_version, parse_run, resolve_cargo, run_cargo, run_program,
    summary_counts,
};
use stapel_core::rust_tests::test_functions;
use stapel_core::steps::{
    Marker, Step, TestId, git_text, junit_step_tests, package_name, show, step_commits, step_tests,
    steps,
};
use stapel_core::tickets::{Status, Ticket, resolve};
use stapel_core::time::now_rfc3339;
use stapel_core::ts_tests::{has_only_call, test_calls};
use stapel_core::worktree::{
    Lock, LockError, Worktree, build_input_outside, dirty_paths, resolve_tmpdir, stapel_dir,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

/// The test globs of a junit `[check]`; `None` under cargo.
fn junit_globs_of(check: &Check) -> Option<Vec<String>> {
    (check.runner == "junit").then(|| check.tests.clone().unwrap_or_default())
}

/// The test globs of the repository's `[check]` when its runner is junit; a `stapel.toml` that
/// fails to load is its fault, never a cargo check.
pub(crate) fn junit_globs_at(root: &Path) -> Result<Option<Vec<String>>, String> {
    let config = repo::load(root)?;
    Ok(config.check.and_then(|c| junit_globs_of(&c)))
}

/// The Walk of a junit check read for build inputs (STP-28 AC-24, AC-25): input globs, the
/// configuration, and the first word of `command` when it is a repository path.
fn junit_inputs(
    root: &Path,
    key: &str,
    at: &str,
    bounds: Bounds,
    check: &Check,
) -> Result<BuildInputs, String> {
    let mut fixed = vec![".stapel/stapel.toml".to_string()];
    let first = check
        .command
        .as_deref()
        .and_then(|c| c.split(' ').find(|w| !w.is_empty()));
    if let Some(word) = first {
        if Path::new(word).is_absolute() {
            if let Some(rel) = input_under(root, word) {
                fixed.push(rel);
            }
        } else if word.contains('/') {
            fixed.push(word.trim_start_matches("./").to_string());
        }
    }
    let inputs = check.inputs.clone().unwrap_or_default();
    build_inputs::analyse_junit(root, key, at, bounds, &inputs, &fixed)
}

/// The repository path of the absolute `word` under `root`, with `.` and empty parts dropped and
/// `..` parts resolved: a fixed build input whatever its spelling (STP-28 AC-25, STP-30 CR1-E4).
fn input_under(root: &Path, word: &str) -> Option<String> {
    let rel = Path::new(word).strip_prefix(root).ok()?;
    let mut parts: Vec<&str> = Vec::new();
    for c in rel.components() {
        match c {
            std::path::Component::Normal(p) => parts.push(p.to_str()?),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop()?;
            }
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The part of the absolute `word` below `root`, when it has no `.`, `..` or empty part (STP-30 AC-6).
fn script_under<'a>(root: &Path, word: &'a str) -> Option<&'a str> {
    let rel = word
        .strip_prefix(root.to_str()?.trim_end_matches('/'))?
        .strip_prefix('/')?;
    rel.split('/')
        .all(|p| !p.is_empty() && p != "." && p != "..")
        .then_some(rel)
}

/// The change that blocks a step under junit (AC-24, AC-25, AC-39), the first one in the Walk:
/// an input change at or before the step's GREEN commit; a change of `.stapel/stapel.toml` after
/// the step's RED commit, wherever it falls (a configuration in place at or before the ticket's
/// first RED blocks no step, as that RED is not after it); a change of the command's script after
/// the step's RED up to its GREEN commit.
fn junit_block<'a>(a: &Analysis, inputs: &'a BuildInputs, walk: &[String]) -> Option<&'a Change> {
    let place = |c: Option<&stapel_core::steps::StepCommit>| {
        c.and_then(|c| walk.iter().position(|s| *s == c.sha))
    };
    let (red, green) = (place(a.red.as_ref()), place(a.green.as_ref()));
    inputs.walk_changes.iter().find_map(|(change, fixed)| {
        let at = walk.iter().position(|s| *s == change.sha)?;
        let after_green = green.is_some_and(|g| g < at);
        let after_red = red.is_some_and(|r| r < at);
        let blocks = if !*fixed {
            !after_green
        } else if change.path == ".stapel/stapel.toml" {
            after_red
        } else {
            after_red && !after_green
        };
        blocks.then_some(change)
    })
}

/// The mode that judges steps for the globs of `junit_globs_at`.
pub(crate) fn mode_of(globs: &Option<Vec<String>>) -> Mode<'_> {
    match globs {
        Some(g) => Mode::Junit(g),
        None => Mode::Cargo,
    }
}

/// A refusal: nothing was run, exit 2.
fn refuse(message: impl std::fmt::Display) -> ExitCode {
    warn!("stapel: {message}");
    ExitCode::from(2)
}

/// The command words of a guard's command, each with whether it starts a command (STP-43 AC-3): the
/// command is split at white space and at `; | & ( ) < >` and a backtick, then each `'` and `"` goes.
fn command_words(command: &str) -> Vec<(String, bool)> {
    let mut words: Vec<(String, bool)> = Vec::new();
    let mut piece = String::new();
    let mut starts = true;
    let mut next_starts = true;
    let mut flush = |piece: &mut String, starts: &mut bool, next_starts: &mut bool| {
        if !piece.is_empty() {
            let word: String = piece.chars().filter(|c| !matches!(c, '\'' | '"')).collect();
            if !word.is_empty() {
                words.push((word, *starts));
                *next_starts = false;
            }
            piece.clear();
        }
        *starts = *next_starts;
    };
    let mut previous = ' ';
    for c in command.chars() {
        let after_redirect = matches!(previous, '<' | '>');
        previous = c;
        if c == '&' && after_redirect {
            // The `&` of `2>&1` is part of the redirection and starts no word.
            continue;
        }
        if c.is_whitespace() {
            flush(&mut piece, &mut starts, &mut next_starts);
        } else if matches!(c, ';' | '|' | '&' | '(' | '`') {
            flush(&mut piece, &mut starts, &mut next_starts);
            next_starts = true;
            starts = true;
        } else if matches!(c, ')' | '<' | '>') {
            flush(&mut piece, &mut starts, &mut next_starts);
        } else {
            piece.push(c);
        }
    }
    flush(&mut piece, &mut starts, &mut next_starts);
    words
}

/// The reach warnings of the guards: for each guard one line for each distinct script or config of its
/// command that is an existing regular file of the working tree at `root` outside `dir` (STP-43 AC-3).
pub(crate) fn guard_reach(
    root: &Path,
    dir: &str,
    entries: &[stapel_core::config::GuardEntry],
) -> Vec<String> {
    const INTERPRETERS: [&str; 8] = [
        "sh", "bash", "zsh", "node", "python", "python3", "ruby", "perl",
    ];
    let inside = dir.trim_end_matches('/');
    let outside_file = |word: &str| -> Option<String> {
        let rel = word.strip_prefix("./").unwrap_or(word);
        if rel.starts_with('/')
            || rel.contains(['$', '*', '?', '~', '{'])
            || rel.split('/').any(|p| matches!(p, "" | "." | ".."))
        {
            return None;
        }
        let (rel_lower, inside_lower) = (rel.to_lowercase(), inside.to_lowercase());
        let covered = rel_lower == inside_lower
            || rel_lower
                .strip_prefix(&inside_lower)
                .is_some_and(|rest| rest.starts_with('/'));
        if covered || !std::fs::metadata(root.join(rel)).is_ok_and(|m| m.is_file()) {
            return None;
        }
        Some(rel.to_string())
    };
    let mut lines = Vec::new();
    for entry in entries {
        let mut seen: Vec<String> = Vec::new();
        let mut reach: Vec<&str> = Vec::new();
        let words = command_words(&entry.command);
        let mut after_interpreter = false;
        let mut config_next = false;
        for (word, starts) in &words {
            if *starts {
                after_interpreter = false;
                config_next = false;
                reach.push(word);
            } else if config_next {
                config_next = false;
                reach.push(word);
            } else if after_interpreter && !word.starts_with('-') {
                after_interpreter = false;
                reach.push(word);
            }
            if *starts && INTERPRETERS.contains(&word.as_str()) {
                after_interpreter = true;
            }
            if let Some(option) = word.strip_prefix('-') {
                match option.split_once('=') {
                    Some((name, value)) if name.contains("config") => reach.push(value),
                    None if option.contains("config") => config_next = true,
                    _ => {}
                }
            }
        }
        for word in reach {
            if let Some(file) = outside_file(word) {
                if !seen.contains(&file) {
                    lines.push(format!(
                        "warning: guard {} reads {file} outside {dir}; a guard's scripts and configs belong in {dir}",
                        printable(&entry.name)
                    ));
                    seen.push(file);
                }
            }
        }
    }
    lines
}

// The refusals both modes share (STP-14); each mode calls them in its own order.

/// A legacy or unreadable ticket cannot be checked.
pub(crate) fn status_refusal(ticket: &Ticket) -> Result<(), String> {
    match &ticket.status {
        Status::Open(_) | Status::Closed(_) => Ok(()),
        Status::Legacy(_) => Err(format!(
            "ticket {} has a legacy (STP-1) state.json; it cannot be checked",
            ticket.key
        )),
        Status::Unreadable(r) => Err(r.clone()),
    }
}

/// The full id of `HEAD`.
pub(crate) fn read_head(root: &Path) -> Result<String, String> {
    git_text(root, &["rev-parse", "--verify", "HEAD"]).map(|h| h.trim().to_string())
}

/// A shallow clone hides history the check reads.
pub(crate) fn shallow_refusal(root: &Path) -> Result<(), String> {
    if git_text(root, &["rev-parse", "--is-shallow-repository"])
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
    {
        return Err(
            "this is a shallow clone; the check reads the whole history (git fetch --unshallow)"
                .into(),
        );
    }
    Ok(())
}

/// The `base` of the ticket's `state.json`, which must lie on the first-parent history of `HEAD`.
pub(crate) fn base_of(root: &Path, ticket: &Ticket, head: &str) -> Result<Option<String>, String> {
    let base = match &ticket.status {
        Status::Open(s) | Status::Closed(s) => s.base.clone(),
        _ => None,
    };
    if let Some(base) = &base {
        build_inputs::check_base(root, base, head)?;
    }
    Ok(base)
}

/// The `base_branch` of the ticket's `state.json`.
pub(crate) fn base_branch_of(ticket: &Ticket) -> Option<String> {
    match &ticket.status {
        Status::Open(s) | Status::Closed(s) => s.base_branch.clone(),
        _ => None,
    }
}

/// The steps of the ticket on the first-parent history of `head`.
fn read_steps(root: &Path, key: &str, head: &str, base: Option<&str>) -> Result<Vec<Step>, String> {
    step_commits(root, key, head, base).map(steps)
}

fn no_steps(key: &str) -> String {
    format!(
        "ticket {key} has no step commits (subjects `{key} <label> RED: …` and \
         `{key} <label> GREEN: …` on the first-parent history of HEAD)"
    )
}

/// The end of a closed ticket's range: a closed ticket is checked there, not at HEAD
/// (STP-6 AC-16). `None` for an open ticket.
fn range_end(root: &Path, key: &str, head: &str, bounds: Bounds) -> Result<Option<String>, String> {
    if !bounds.closed {
        return Ok(None);
    }
    match build_inputs::range(root, key, head, bounds)?.last() {
        Some(e) => Ok(Some(e.clone())),
        None => Err(format!(
            "ticket {key} is closed and has no commit whose subject starts with `{key} ` or `{key}:`"
        )),
    }
}

fn sha256_hex(parts: &[&str]) -> String {
    use sha2::Digest;
    let mut digest = sha2::Sha256::new();
    for part in parts {
        digest.update(part.as_bytes());
    }
    digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The test hash of a step: SHA-256 of its sorted test ids, one per line.
fn tests_hash(tests: &[TestId]) -> String {
    let mut ids: Vec<String> = tests.iter().map(ToString::to_string).collect();
    ids.sort();
    let text: String = ids.iter().map(|i| format!("{i}\n")).collect();
    sha256_hex(&[&text])
}

/// The cache key of a step: its test hash; with history commits the hash of the history text of its
/// RED run (path, history commit and text hash) joins it (STP-37 AC-10). A step without history
/// commits keeps the plain test hash.
fn step_key(root: &Path, a: &Analysis) -> String {
    let base = tests_hash(&a.tests);
    if a.red_files.is_empty() {
        return base;
    }
    let mut files: Vec<&(String, String)> = a.red_files.iter().collect();
    files.sort();
    let mut text = format!("{base}\n");
    for (path, commit) in files {
        let body = stapel_core::steps::show(root, commit, path).unwrap_or_default();
        text.push_str(&format!("{path} {commit} {}\n", sha256_hex(&[&body])));
    }
    sha256_hex(&[&text])
}

/// The lock hash at `commit`: SHA-256 of `Cargo.lock` under cargo, of every `package-lock.json`,
/// `yarn.lock` and `pnpm-lock.yaml` under junit; the word `none` when there is none.
fn lock_hash(root: &Path, commit: &str, junit: bool) -> String {
    let mut paths: Vec<String> = Vec::new();
    if junit {
        let listing = git_text(root, &["ls-tree", "-r", "--name-only", commit]).unwrap_or_default();
        for path in listing.lines() {
            let name = path.rsplit('/').next().unwrap_or(path);
            if matches!(name, "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml") {
                paths.push(path.to_string());
            }
        }
        paths.sort();
    } else {
        paths.push("Cargo.lock".to_string());
    }
    let mut parts: Vec<String> = Vec::new();
    for path in &paths {
        if let Ok(text) = git_text(root, &["show", &format!("{commit}:{path}")]) {
            parts.push(format!("{path}\n{text}\n"));
        }
    }
    if parts.is_empty() {
        return "none".to_string();
    }
    let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    sha256_hex(&refs)
}

/// The hash of the Plan section of the ticket, or `none` when it has no single Plan section.
fn plan_hash(dir: &Path) -> String {
    let text = std::fs::read_to_string(dir.join("ticket.md")).unwrap_or_default();
    let sections = stapel_core::ticket::parse(&text);
    match stapel_core::ticket::find(&sections, "Plan") {
        stapel_core::ticket::Lookup::Found(body) => stapel_core::hash::section_hash(body),
        _ => "none".to_string(),
    }
}

pub fn run(key: Option<&str>, list: bool, history: bool, level: Option<&str>) -> ExitCode {
    if let Some(level) = level {
        if list || history {
            return refuse("--level does not go with --list or --history");
        }
        if !matches!(level, "steps" | "head" | "full") {
            return refuse(format!("--level must be steps, head or full: {level}"));
        }
    }
    let (root, config) = match repo::open() {
        Ok(v) => v,
        Err(e) => return refuse(e),
    };
    if let Err(e) = stapel_core::tickets::match_branch(&root, key) {
        return refuse(e);
    }
    let ticket = match resolve(&root, key) {
        Ok(t) => t,
        Err(e) => return refuse(e),
    };
    if let Err(e) = status_refusal(&ticket) {
        return refuse(e);
    }
    let head = match read_head(&root) {
        Ok(h) => h,
        Err(e) => return refuse(e),
    };
    if let Err(e) = shallow_refusal(&root) {
        return refuse(e);
    }
    let base = match base_of(&root, &ticket, &head) {
        Ok(b) => b,
        Err(e) => return refuse(e),
    };
    let steps = match read_steps(&root, &ticket.key, &head, base.as_deref()) {
        Ok(s) => s,
        Err(e) => return refuse(e),
    };
    let globs = config.check.as_ref().and_then(junit_globs_of);
    if list {
        if steps.is_empty() {
            return refuse(no_steps(&ticket.key));
        }
        return match print_list(&root, &ticket.key, &steps, mode_of(&globs)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => refuse(e),
        };
    }
    let base_branch = base_branch_of(&ticket);
    let bounds = Bounds {
        base: base.as_deref(),
        base_branch: base_branch.as_deref(),
        closed: matches!(ticket.status, Status::Closed(_)),
    };
    let end = match range_end(&root, &ticket.key, &head, bounds) {
        Ok(e) => e,
        Err(e) => return refuse(e),
    };
    // The history check needs no `[check]`, no clean tree and no lock (STP-14 AC-5).
    if history {
        if steps.is_empty() {
            return refuse(no_steps(&ticket.key));
        }
        return print_history(&root, &ticket.key, &head, end.as_deref(), &steps, bounds);
    }
    if let Err(f) = crate::findings::guard_block(&ticket.dir) {
        return refuse(f.message);
    }
    let Some(check) = config.check.clone() else {
        return refuse(
            "check is not configured: add `[check]` with `runner = \"cargo\"` to .stapel/stapel.toml",
        );
    };
    match dirty_paths(&root) {
        Ok(d) if !d.is_empty() => {
            let first: Vec<&str> = d.iter().take(5).map(String::as_str).collect();
            return refuse(format!(
                "the working tree has uncommitted changes outside the machine files: {}; commit them first",
                first.join(", ")
            ));
        }
        Ok(_) => {}
        Err(e) => return refuse(e),
    }
    if steps.is_empty() {
        return refuse(no_steps(&ticket.key));
    }
    let dir = match stapel_dir(&root) {
        Ok(d) => d,
        Err(e) => return refuse(e),
    };
    let _lock = match Lock::take(&dir) {
        Ok((lock, previous)) => {
            if let Some(pid) = previous {
                warn!("notice: took over the check lock of process {pid}, which is not running");
            }
            lock
        }
        Err(LockError::Held(pid)) => {
            return refuse(format!(
                "another stapel check runs in this repository (pid {})",
                if pid.is_empty() { "unknown" } else { &pid }
            ));
        }
        Err(LockError::Io(e)) => return refuse(e),
    };
    let at = end.clone().unwrap_or_else(|| head.clone());
    let junit = globs.is_some();
    let analyses = match analyse_in(
        &root,
        &ticket.key,
        &at,
        &steps,
        bounds.base,
        bounds.base_branch,
        mode_of(&globs),
    ) {
        Ok(a) => a,
        Err(e) => return refuse(e),
    };
    let inputs = if junit {
        junit_inputs(&root, &ticket.key, &at, bounds, &check)
    } else {
        build_inputs::analyse(&root, &ticket.key, &at, &steps, bounds)
    };
    let inputs = match inputs {
        Ok(i) => i,
        Err(e) => return refuse(e),
    };
    // Under junit a change blocks the steps whose GREEN is at or after it (AC-24).
    let walk = if junit {
        match build_inputs::range(&root, &ticket.key, &at, bounds) {
            Ok(w) => w,
            Err(e) => return refuse(e),
        }
    } else {
        Vec::new()
    };
    let tmp = match resolve_tmpdir(&root) {
        Ok(t) => t,
        Err(e) => return refuse(e),
    };
    let wt = match Worktree::create(&root, &dir, &at, &tmp) {
        Ok(w) => w,
        Err(e) => return refuse(e),
    };
    // Under junit no cargo is found or checked; the lock is taken as under cargo (AC-40).
    let cargo = if junit {
        PathBuf::new()
    } else {
        match resolve_cargo(check.cargo.as_deref()) {
            Ok(c) => c,
            Err(e) => return refuse(e),
        }
    };
    match check.cargo.as_deref() {
        _ if junit => {}
        Some(key) => {
            if let Err(e) = validate_cargo(key, &root) {
                return refuse(e);
            }
        }
        None => {
            if let Err(e) = validate_path_cargo(&cargo, &root) {
                return refuse(e);
            }
        }
    }
    // One cargo run at a time on the machine; the lock lives until the check ends (STP-27 AC-11).
    let _cargo_lock = match crate::cargo::take("stapel check") {
        Ok(l) => l,
        Err(e) => return refuse(e),
    };
    if _cargo_lock.is_some() {
        // SAFETY: the check is still single-threaded here; no cargo run or reader thread has started.
        unsafe { std::env::set_var(crate::cargo::HELD_MARK, std::process::id().to_string()) };
    }
    let target = dir.join("check-target");
    let timeout = Duration::from_secs(check.timeout_secs);
    // The limit bounds the whole check (STP-35 AC-3), not each program run.
    let deadline = std::time::Instant::now() + timeout;
    // A configuration above the worktree is found before any cargo run (AC-7).
    let outside = if junit {
        None
    } else {
        build_input_outside(&wt.path)
    };
    // A build input changed by a commit of the ticket comes first (STP-6 AC-1, AC-2); no cargo runs.
    let mut blocked = inputs
        .changed
        .as_ref()
        .filter(|_| !junit)
        .map(changed_reason)
        .or(outside.map(|p| format!("build-input-outside: {}", p.display())));
    let cargo_text = if junit {
        String::new()
    } else if blocked.is_some() {
        cargo.display().to_string()
    } else {
        match cargo_version(&cargo, &wt.path, &target, timeout) {
            Ok(v) => format!("{} ({v})", cargo.display()),
            Err(e) => return refuse(format!("{}: {e}", cargo.display())),
        }
    };
    // The search is repeated after every cargo run, `cargo --version` included (AC-7).
    if blocked.is_none() && !junit {
        blocked =
            build_input_outside(&wt.path).map(|p| format!("build-input-outside: {}", p.display()));
    }
    let junit_run = match (junit, &check.command, &check.report) {
        (true, Some(command), Some(report)) => Some(JunitRun { command, report }),
        _ => None,
    };
    let runner = Runner {
        root: &root,
        wt: &wt,
        cargo: &cargo,
        target,
        timeout,
        deadline,
        head: &at,
        key: &ticket.key,
        bounds,
        outside: std::cell::RefCell::new(blocked),
        junit: junit_run,
        prepared: std::cell::Cell::new(false),
        jobs: check.jobs,
        test_threads: check.test_threads,
    };
    if end.is_some() {
        say!("ticket: {} at {} (closed)", ticket.key, short(&at));
    } else {
        say!("ticket: {} at {}", ticket.key, short(&head));
    }
    if !junit {
        say!("cargo: {cargo_text}");
    }
    // The level follows the `cargo:` line (AC-11).
    let level_now = level
        .or(check.level.as_deref())
        .unwrap_or("head")
        .to_string();
    say!("level: {level_now}");
    // Under junit a Node pin of the checked commit is compared with `node --version` (STP-37 AC-6, AC-7).
    let pin = if junit {
        match node_pin(&root, &at) {
            Ok(pin) => pin,
            Err(e) => return refuse(e),
        }
    } else {
        None
    };
    if let Some((file, pin, numbers)) = pin {
        let run = runner.left().and_then(|left| {
            run_program(
                Path::new("node"),
                &wt.path,
                &["--version".to_string()],
                &[],
                left,
            )
        });
        let version = match run {
            Ok(run) if run.timed_out => Err(runner.limit_reason()),
            Ok(run) if run.exit == Some(0) => node_version_of(&run.output)
                .ok_or_else(|| format!("no version in the output: {}", run.output.trim())),
            Ok(run) => Err(format!("exit {:?}: {}", run.exit, run.output.trim())),
            Err(e) => Err(e),
        };
        match version {
            Err(e) => return refuse(format!("node --version failed: {e}")),
            Ok((version, have)) => {
                if have[..numbers.len()] != numbers[..] {
                    return refuse(format!(
                        "node {version} does not match the pin {pin} in {file}"
                    ));
                }
                say!("node: {version} (pin {pin} in {file})");
            }
        }
    }
    if !inputs.unchanged.is_empty() {
        say!("build inputs: {}", named(&inputs.unchanged));
    }
    let lock_now = lock_hash(&root, &at, junit);
    let toolchain_now = if junit {
        check.command.clone().unwrap_or_default()
    } else {
        cargo_text.clone()
    };
    let tool_now = format!("stapel {}", env!("CARGO_PKG_VERSION"));
    let mut ok = true;
    let mut steps_json = Vec::new();
    for a in &analyses {
        // A build input found above the worktree after a cargo run blocks every later step (AC-7).
        let blocked = if junit {
            junit_block(a, &inputs, &walk).map(changed_reason)
        } else {
            runner.outside.borrow().clone()
        };
        let (outcome, reason) = match history_outcome(a, blocked.as_deref()) {
            Some(o) => o,
            None => {
                // A cached step skips RED and GREEN below the level `full` (AC-4); one that
                // does not hold at HEAD runs in full.
                let source = if level_now == "full" {
                    None
                } else {
                    cached_from(
                        &ticket.dir,
                        &ticket.key,
                        a,
                        &step_key(&root, a),
                        &lock_now,
                        &toolchain_now,
                        &tool_now,
                    )
                };
                match source {
                    // At the level `steps` a cached step runs nothing (AC-8).
                    Some(from) if level_now == "steps" => ("cached".to_string(), from),
                    Some(from) => match runner.step(a, true) {
                        (o, _) if o == "cached" => (o, from),
                        _ => runner.step(a, false),
                    },
                    None => runner.step(a, false),
                }
            }
        };
        ok &= passing(&outcome);
        // For a cached step the reason is the id of the record that ran it.
        let from = (outcome == "cached").then(|| reason.clone());
        if steps_json.len() < MAX_STEPS {
            let mut step_json = serde_json::json!({
                "label": cut(&a.label, MAX_LABEL),
                "red": a.red.as_ref().map(|c| c.sha.clone()),
                "green": a.green.as_ref().map(|c| c.sha.clone()),
                "outcome": outcome,
                "reason": cut(&reason, MAX_REASON),
                "tests": a.tests.len(),
                "tests_hash": step_key(&root, a),
                "names": names_of(&outcome, &reason),
            });
            if let Some(from) = &from {
                step_json["from"] = serde_json::json!(from);
                step_json["reason"] = serde_json::json!("");
            }
            steps_json.push(step_json);
        }
        say_step(a, &outcome, &reason);
    }
    // The history commits of the range, before the suite line (STP-37 AC-13).
    let history = match build_inputs::history_commits(&root, &ticket.key, &at, bounds) {
        Ok(h) => h,
        Err(e) => return refuse(e),
    };
    for line in history_lines(&history) {
        say!("{line}");
    }
    // The entries left out of the local suite; at `full` nothing is left out (AC-12).
    let excluded: Vec<String> = if level_now == "full" {
        Vec::new()
    } else {
        check.suite_exclude.clone().unwrap_or_default()
    };
    let suite = if level_now == "steps" {
        Suite {
            outcome: "skipped".into(),
            detail: "level steps".into(),
            counts: (0, 0, 0),
            failing: Vec::new(),
            failing_more: 0,
        }
    } else {
        runner.suite(&excluded)
    };
    ok &= suite.outcome == "pass" || suite.outcome == "skipped";
    say!("suite: {}: {}", suite.outcome, suite.detail);
    if !suite.failing.is_empty() {
        let more = if suite.failing_more > 0 {
            format!(" and {} more", suite.failing_more)
        } else {
            String::new()
        };
        say!("failing: {}{more}", suite.failing.join(", "));
    }
    // The project commands run after the suite, at `head` and `full` (STP-37 AC-2, AC-4, AC-5).
    let project_commands: &[String] = check.project.as_deref().unwrap_or(&[]);
    let mut project_json: Vec<serde_json::Value> = Vec::new();
    if !project_commands.is_empty() {
        if level_now == "steps" {
            say!("project: skipped: level steps");
        } else {
            for (command, outcome) in runner.project(project_commands) {
                ok &= outcome == "pass" || outcome == "skipped";
                project_json.push(serde_json::json!({
                    "command": cut_chars(&command, MAX_COMMAND),
                    "outcome": outcome,
                }));
            }
        }
    }
    // The guards run after the project commands, at `head` and `full` (STP-41 AC-3 to AC-5).
    let guard_entries: &[stapel_core::config::GuardEntry] = config
        .guards
        .as_ref()
        .map(|g| g.check.as_slice())
        .unwrap_or(&[]);
    let mut guards_json: Vec<serde_json::Value> = Vec::new();
    if !guard_entries.is_empty() {
        if level_now == "steps" {
            say!("guards: skipped: level steps");
        } else {
            let guards_dir = config.guards.as_ref().map_or("", |g| g.dir.as_str());
            for line in guard_reach(&root, guards_dir, guard_entries) {
                warn!("{line}");
            }
            for (entry, (outcome, new, removed)) in guard_entries
                .iter()
                .zip(runner.guards(guards_dir, guard_entries))
            {
                ok &= outcome == "pass";
                guards_json.push(serde_json::json!({
                    "name": cut_chars(&entry.name, MAX_COMMAND),
                    "outcome": outcome,
                    "new": new,
                    "removed": removed,
                }));
            }
        }
    }
    say!("result: {}", if ok { "pass" } else { "fail" });
    let mut record = serde_json::json!({
        "v": 1,
        "kind": "check",
        "id": new_id("c"),
        "at": now_rfc3339(),
        "ticket": ticket.key,
        "head": head,
        "tool": tool_now,
    });
    if junit {
        record["runner"] = serde_json::json!("junit");
    } else {
        record["cargo"] = serde_json::json!(cargo_text);
    }
    // The fields that a later check reads (STP-35 AC-6).
    record["level"] = serde_json::json!(level_now);
    record["lock"] = serde_json::json!(lock_now);
    record["toolchain"] = serde_json::json!(toolchain_now);
    record["plan_hash"] = serde_json::json!(plan_hash(&ticket.dir));
    record["suite_exclude"] = serde_json::json!(excluded);
    record["result"] = serde_json::json!(if ok { "pass" } else { "fail" });
    record["steps"] = serde_json::json!(steps_json);
    record["suite"] = serde_json::json!({
        "outcome": suite.outcome,
        "passed": suite.counts.0,
        "failed": suite.counts.1,
        "ignored": suite.counts.2,
    });
    if !suite.failing.is_empty() {
        record["suite"]["failing"] = serde_json::json!(suite.failing);
        record["suite"]["failing_more"] = serde_json::json!(suite.failing_more);
    }
    if !project_json.is_empty() {
        // Past MAX_PROJECT entries the rest are left out; the record says `project_truncated: true`.
        if project_json.len() > MAX_PROJECT {
            project_json.truncate(MAX_PROJECT);
            record["project_truncated"] = serde_json::json!(true);
        }
        record["project"] = serde_json::json!(project_json);
    }
    if !guards_json.is_empty() {
        if guards_json.len() > MAX_PROJECT {
            guards_json.truncate(MAX_PROJECT);
            record["guards_truncated"] = serde_json::json!(true);
        }
        record["guards"] = serde_json::json!(guards_json);
    }
    if !history.is_empty() {
        let (entries, truncated) = history_json(&history);
        record["history"] = serde_json::json!(entries);
        if truncated {
            record["history_truncated"] = serde_json::json!(true);
        }
    }
    if let Some(end) = &end {
        record["end"] = serde_json::json!(end);
    }
    if analyses.len() > MAX_STEPS {
        record["steps_truncated"] = serde_json::json!(true);
    }
    // Steps are dropped from the end until the record fits one journal line.
    while record.to_string().len() > MAX_RECORD {
        let Some(steps) = record["steps"].as_array_mut() else {
            break;
        };
        if steps.pop().is_none() {
            break;
        }
        record["steps_truncated"] = serde_json::json!(true);
    }
    if let Err(e) = append(&ticket.dir.join("runs.jsonl"), &record) {
        warn!("stapel: the check record was not written: {e}");
        return ExitCode::from(1);
    }
    if let Err(e) = stapel_core::review_block::write(&ticket.dir) {
        warn!("stapel: review block not updated: {e}");
    }
    if let Ok(now) = git_text(&root, &["rev-parse", "--verify", "HEAD"])
        && now.trim() != head
    {
        warn!(
            "notice: HEAD moved to {} while the check ran; this check is of {}",
            short(now.trim()),
            short(&at)
        );
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

struct Suite {
    outcome: String,
    detail: String,
    counts: (u64, u64, u64),
    /// The first failing names (at most `MAX_FAILING`) and how many more distinct ones there are.
    failing: Vec<String>,
    failing_more: usize,
}

/// At most this many failing names are printed and recorded.
const MAX_FAILING: usize = 20;

/// The distinct failing names of a cargo run, in output order: `<target> <name>` for each
/// `test <name> ... FAILED` line of `stdout`, `doc <crate> <name>` for a doc test. The k-th block of
/// `stdout` that starts with `running <n> tests` belongs to the k-th `Running` or `Doc-tests` line of
/// `stderr`; a block without such a line gives a bare name.
fn cargo_failing_names(stdout: &str, stderr: &str) -> Vec<String> {
    let targets: Vec<String> = stderr
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("Running ") {
                let path = match rest.find(" (") {
                    Some(at) if rest.ends_with(')') => &rest[at + 2..rest.len() - 1],
                    _ => rest,
                };
                let file = path.trim().rsplit('/').next().unwrap_or("");
                let name = match file.rsplit_once('-') {
                    Some((head, hash))
                        if !head.is_empty()
                            && !hash.is_empty()
                            && hash.bytes().all(|b| b.is_ascii_hexdigit()) =>
                    {
                        head
                    }
                    _ => file,
                };
                Some(name.to_string())
            } else {
                trimmed
                    .strip_prefix("Doc-tests ")
                    .map(|rest| format!("doc {}", rest.trim()))
            }
        })
        .collect();
    let mut names: Vec<String> = Vec::new();
    let mut block: Option<usize> = None;
    for line in stdout.lines() {
        if line.starts_with("running ") && line.ends_with(" tests") || line == "running 1 test" {
            block = Some(block.map_or(0, |b| b + 1));
        } else if let Some(name) = line
            .strip_prefix("test ")
            .and_then(|r| r.strip_suffix(" ... FAILED"))
        {
            let full = match block.and_then(|b| targets.get(b)) {
                Some(target) if !target.is_empty() => format!("{target} {name}"),
                _ => name.to_string(),
            };
            if !names.contains(&full) {
                names.push(full);
            }
        }
    }
    names
}

/// At most this many steps go into a record, and this many bytes of a label.
const MAX_STEPS: usize = 200;
const MAX_LABEL: usize = 200;
const MAX_REASON: usize = 1024;
/// A project command or a history reason is cut to this many characters in a record, and at most
/// `MAX_PROJECT` entries of each are listed.
const MAX_COMMAND: usize = 200;
const MAX_PROJECT: usize = 20;
/// A failing project command prints this many lines of its output.
const PROJECT_TAIL: usize = 20;
/// A record stays below the journal's 64 KiB line limit with room to spare.
const MAX_RECORD: usize = 60 * 1024;

/// The numbers of a version: one to three dot-separated numbers with an optional leading `v`.
fn version_numbers(text: &str) -> Option<Vec<u64>> {
    let text = text.strip_prefix('v').unwrap_or(text);
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() > 3 {
        return None;
    }
    parts
        .iter()
        .map(|p| {
            if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) {
                p.parse().ok()
            } else {
                None
            }
        })
        .collect()
}

/// The Node pin at a commit: the file, the trimmed text and its numbers, for the first of `.nvmrc`
/// and `.node-version` whose text is one line that is a version (STP-37 AC-6).
/// A pin that is not a version, or two pins that disagree, is an error (STP-37 AC-8).
type Pin = (&'static str, String, Vec<u64>);
fn node_pin(root: &Path, at: &str) -> Result<Option<Pin>, String> {
    let mut pins: Vec<Pin> = Vec::new();
    for file in [".nvmrc", ".node-version"] {
        let Some(text) = stapel_core::steps::show(root, at, file) else {
            continue;
        };
        let text = text.trim();
        let numbers = if text.contains('\n') {
            None
        } else {
            version_numbers(text)
        };
        let Some(numbers) = numbers else {
            return Err(format!("{file}: not a version: {text}"));
        };
        pins.push((file, text.to_string(), numbers));
    }
    if pins.len() == 2 && pins[0].2 != pins[1].2 {
        return Err(format!(
            ".nvmrc and .node-version disagree: {}, {}",
            pins[0].1, pins[1].1
        ));
    }
    Ok(pins.into_iter().next())
}

/// The first `v<n>.<n>.<n>` word of the output of `node --version`: the word and its numbers.
fn node_version_of(output: &str) -> Option<(String, Vec<u64>)> {
    output.split_whitespace().find_map(|w| {
        let numbers = version_numbers(w)?;
        (w.starts_with('v') && numbers.len() == 3).then(|| (w.to_string(), numbers))
    })
}

/// One path as one shell word: single-quoted, with `./` before a path that starts with `-`.
/// The text with each control character (a newline included) as a space (STP-37 AC-2).
fn printable(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn shell_word(path: &str) -> String {
    let path = if path.starts_with('-') {
        format!("./{path}")
    } else {
        path.to_string()
    };
    format!("'{}'", path.replace('\'', "'\\''"))
}

/// Whether a whitespace-separated word of the command is exactly `{files}`.
fn has_files_word(command: &str) -> bool {
    command.split_whitespace().any(|w| w == "{files}")
}

/// The command with each whole word `{files}` replaced by `list`, joined by spaces.
fn replace_files_word(command: &str, list: &[String]) -> String {
    let joined = list.join(" ");
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if word == "{files}" {
            out.push_str(&joined);
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in command.chars() {
        if c.is_whitespace() {
            flush(&mut word, &mut out);
            out.push(c);
        } else {
            word.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// The first `max` characters of `text`.
fn cut_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// The history lines of a report (STP-37 AC-13): `history: <short id> <reason>`, control
/// characters of the reason each replaced by a space, the reason cut to `MAX_COMMAND` characters.
fn history_lines(history: &[(String, String)]) -> Vec<String> {
    history
        .iter()
        .map(|(sha, reason)| {
            let shown: String = cut_chars(reason, MAX_COMMAND)
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            format!("history: {} {shown}", short(sha))
        })
        .collect()
}

/// The `history` entries of a check record (STP-37 AC-14): `{commit, reason}`, at most
/// `MAX_PROJECT` of them; the flag says whether the rest were left out.
fn history_json(history: &[(String, String)]) -> (Vec<serde_json::Value>, bool) {
    let entries = history
        .iter()
        .take(MAX_PROJECT)
        .map(|(sha, reason)| {
            serde_json::json!({"commit": sha, "reason": cut_chars(reason, MAX_COMMAND)})
        })
        .collect();
    (entries, history.len() > MAX_PROJECT)
}

fn cut(text: &str, max: usize) -> String {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// The names in a reason that lists tests or paths, at most 20.
fn names_of(outcome: &str, reason: &str) -> Vec<String> {
    if !["no-red", "not-green", "removed", "tests-changed"].contains(&outcome) {
        return Vec::new();
    }
    let list = reason.split(" and ").next().unwrap_or(reason);
    list.split(", ")
        .filter(|n| !n.is_empty())
        .take(MAX_NAMED)
        .map(String::from)
        .collect()
}

struct Runner<'a> {
    root: &'a Path,
    wt: &'a Worktree,
    cargo: &'a Path,
    target: PathBuf,
    timeout: Duration,
    /// The end of the time limit of the whole check.
    deadline: std::time::Instant,
    head: &'a str,
    /// The ticket key and the bounds of its range, which `{files}` reads.
    key: &'a str,
    bounds: Bounds<'a>,
    /// Set when a build input lies above the worktree: no cargo runs, and every step and the suite
    /// read `unverified: build-input-outside: <path>`.
    outside: std::cell::RefCell<Option<String>>,
    /// Set under the junit runner: the command and the report path of `[check]`.
    junit: Option<JunitRun<'a>>,
    /// Set when the check copy was cleaned and set to HEAD for the project commands.
    prepared: std::cell::Cell<bool>,
    /// `[check] jobs` and `test_threads` (STP-45 AC-11).
    jobs: Option<i64>,
    test_threads: Option<i64>,
}

/// The id of the passing record that ran a step with this key, `None` when no record did (STP-35
/// AC-4, AC-5). A record without a key field, a failing record, a malformed line and a record of
/// another ticket are no source; the source step must be `pass` or `cached`.
fn cached_from(
    dir: &Path,
    ticket: &str,
    a: &Analysis,
    want_tests: &str,
    lock: &str,
    toolchain: &str,
    tool: &str,
) -> Option<String> {
    let (red, green) = (a.red.as_ref()?, a.green.as_ref()?);
    let text = std::fs::read_to_string(dir.join("runs.jsonl")).ok()?;
    let mut found = None;
    for line in text.lines() {
        let Ok(r) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let same = |field: &str, value: &str| r[field].as_str() == Some(value);
        if !same("kind", "check")
            || !same("ticket", ticket)
            || !same("result", "pass")
            || !same("lock", lock)
            || !same("toolchain", toolchain)
            || !same("tool", tool)
        {
            continue;
        }
        let Some(id) = r["id"].as_str() else { continue };
        let Some(steps) = r["steps"].as_array() else {
            continue;
        };
        for s in steps {
            let outcome = s["outcome"].as_str().unwrap_or("");
            if matches!(outcome, "pass" | "cached")
                && s["red"].as_str() == Some(red.sha.as_str())
                && s["green"].as_str() == Some(green.sha.as_str())
                && s["tests_hash"].as_str() == Some(want_tests)
            {
                let from = if outcome == "cached" {
                    s["from"].as_str().unwrap_or(id)
                } else {
                    id
                };
                found = Some(from.to_string());
            }
        }
    }
    found
}

/// The `[check]` keys of a junit Per-file run.
struct JunitRun<'a> {
    command: &'a str,
    report: &'a str,
}

/// The state of each test at one commit, or why the run does not count.
type At = Result<BTreeMap<TestId, TestState>, String>;

impl Runner<'_> {
    /// What is left of the time limit of the check, or the reason of `unverified` when it ran out.
    fn left(&self) -> Result<Duration, String> {
        let left = self
            .deadline
            .saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            Err(self.limit_reason())
        } else {
            Ok(left)
        }
    }

    fn limit_reason(&self) -> String {
        format!(
            "the time limit of {} s for the check ran out",
            self.timeout.as_secs()
        )
    }

    /// The search above the worktree, repeated after a cargo run (AC-7): a hit is kept, and every
    /// later run is refused.
    fn found_outside(&self) -> Option<String> {
        let path = build_input_outside(&self.wt.path)?;
        let why = format!("build-input-outside: {}", path.display());
        *self.outside.borrow_mut() = Some(why.clone());
        Some(why)
    }

    /// The program of a junit run: an absolute path under the root is the same path in the check copy
    /// (STP-30 AC-6); any other path or name runs as given.
    fn script_path(&self, program: &str) -> PathBuf {
        if Path::new(program).is_absolute()
            && let Some(rel) = script_under(self.root, program)
        {
            return self.wt.path.join(rel);
        }
        PathBuf::from(program)
    }

    fn package(&self, commit: &str, dir: &str) -> Result<String, String> {
        package_name(self.root, commit, dir)
    }

    /// Runs `tests` at `commit`, one cargo run per test file.
    fn at(&self, commit: &str, tests: &[TestId], at_red: bool, files: &[(String, String)]) -> At {
        // After a build-input hit no further cargo run starts (AC-7).
        if let Some(why) = self.outside.borrow().clone() {
            return Err(why);
        }
        // No checkout runs after the time limit (STP-35 AC-3).
        self.left()?;
        self.wt.checkout(commit)?;
        // STP-37 AC-10: in a test file a history commit changed, the step's own tests come from the
        // newest such commit; the rest of the file stays as at `commit`.
        for (path, from) in files {
            let newer = stapel_core::steps::show(self.root, from, path)
                .ok_or_else(|| format!("{path} is missing at {}", short(from)))?;
            let base = stapel_core::steps::show(self.root, commit, path)
                .ok_or_else(|| format!("{path} is missing at {}", short(commit)))?;
            let names: Vec<String> = tests
                .iter()
                .filter(|t| t.path() == *path)
                .map(|t| t.name.clone())
                .collect();
            let text = if self.junit.is_some() {
                stapel_core::ts_tests::with_calls_from(&base, &newer, &names)
            } else {
                stapel_core::rust_tests::with_tests_from(&base, &newer, &names)
            };
            std::fs::write(self.wt.path.join(path), text).map_err(|e| format!("{path}: {e}"))?;
        }
        if let Some(junit) = &self.junit {
            return self.at_junit(junit, commit, tests);
        }
        let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for t in tests {
            groups
                .entry((t.dir.clone(), t.file.clone()))
                .or_default()
                .push(t.name.clone());
        }
        let mut out = BTreeMap::new();
        for ((dir, file), names) in groups {
            let package = self.package(commit, &dir)?;
            let mut args: Vec<String> = ["test", "--locked"]
                .iter()
                .map(|s| s.to_string())
                .collect();
            if let Some(j) = self.jobs {
                args.push("-j".into());
                args.push(j.to_string());
            }
            args.extend(["-p", &package, "--test", &file, "--"].map(String::from));
            if let Some(t) = self.test_threads {
                args.push(format!("--test-threads={t}"));
            }
            args.push("--exact".into());
            args.extend(names.iter().cloned());
            let left = self.left()?;
            let run = run_cargo(self.cargo, &self.wt.path, &args, &self.target, left)?;
            if let Some(why) = self.found_outside() {
                return Err(why);
            }
            if run.timed_out {
                return Err(self.limit_reason());
            }
            match parse_run(&run.output, &names, run.exit, &file, at_red) {
                Parsed::Unverified(r) => return Err(format!("{r} (at {})", short(commit))),
                Parsed::Counted(states) => {
                    for (name, state) in states {
                        out.insert(
                            TestId {
                                dir: dir.clone(),
                                file: file.clone(),
                                name,
                            },
                            state,
                        );
                    }
                }
            }
        }
        Ok(out)
    }

    /// Links the working tree's `node_modules` into the check copy (STP-28 AC-23).
    fn link_modules(&self) -> Result<(), String> {
        let modules = self.root.join("node_modules");
        if !modules.exists() {
            return Err("node_modules is missing in the working tree".into());
        }
        let modules = std::path::absolute(&modules).map_err(|e| format!("node_modules: {e}"))?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&modules, self.wt.path.join("node_modules"))
            .map_err(|e| format!("node_modules: {e}"))?;
        Ok(())
    }

    /// The whole suite at HEAD under junit (STP-28 AC-26, AC-34): the old report removed, `command`
    /// run once with only the word `{file}` left out, every test suite of the report summed.
    fn suite_junit(&self, junit: &JunitRun) -> Suite {
        let unverified = |reason: String| Suite {
            outcome: "unverified".into(),
            detail: reason,
            counts: (0, 0, 0),
            failing: Vec::new(),
            failing_more: 0,
        };
        if let Err(e) = self.wt.checkout(self.head) {
            return unverified(e);
        }
        if let Err(e) = self.link_modules() {
            return unverified(e);
        }
        let report = self.wt.path.join(junit.report);
        match std::fs::remove_file(&report) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return unverified(format!("{}: {e}", junit.report)),
        }
        let words: Vec<&str> = junit
            .command
            .split(' ')
            .filter(|w| !w.is_empty() && *w != "{file}")
            .collect();
        let Some((program, rest)) = words.split_first() else {
            return unverified("check.command is empty".into());
        };
        let args: Vec<String> = rest.iter().map(|w| w.to_string()).collect();
        let left = match self.left() {
            Ok(l) => l,
            Err(e) => return unverified(e),
        };
        let run = match run_program(
            &self.script_path(program),
            &self.wt.path,
            &args,
            &[("JEST_JUNIT_REPORT_TEST_SUITE_ERRORS", "true")],
            left,
        ) {
            Ok(r) => r,
            Err(e) => return unverified(e),
        };
        if run.timed_out {
            return unverified(self.limit_reason());
        }
        let Some(code) = run.exit else {
            return unverified("the suite run ended by a signal".into());
        };
        let bytes = match read_report_file(&report, junit.report, "the suite") {
            Ok(b) => b,
            Err(e) => return unverified(e),
        };
        let suites = match read_report(&bytes) {
            Ok(s) => s,
            Err(e) => return unverified(format!("report of the suite: {e}")),
        };
        let (mut p, mut f, mut i) = (0u64, 0u64, 0u64);
        for case in suites.iter().flat_map(|s| s.cases.iter()) {
            match case.outcome {
                CaseOutcome::Passed => p += 1,
                CaseOutcome::Failed => f += 1,
                CaseOutcome::Skipped => i += 1,
            }
        }
        let detail = format!("{p} passed, {f} failed, {i} ignored");
        let outcome = if code == 0 && f == 0 && p + i > 0 {
            "pass"
        } else {
            "fail"
        };
        let mut failing: Vec<String> = Vec::new();
        for suite in &suites {
            for case in suite.cases.iter().filter(|c| c.outcome == CaseOutcome::Failed) {
                let name = format!("{} {}", suite.name, case.name.trim());
                if !failing.contains(&name) {
                    failing.push(name);
                }
            }
        }
        let failing_more = failing.len().saturating_sub(MAX_FAILING);
        failing.truncate(MAX_FAILING);
        Suite {
            outcome: outcome.into(),
            detail,
            counts: (p, f, i),
            failing,
            failing_more,
        }
    }

    /// Runs `tests` at `commit` under junit (STP-28 AC-10, AC-11): one Per-file run per test file in
    /// the checked-out copy, with the working tree's `node_modules` linked in and the old report
    /// removed; each test takes the state of its one test case.
    fn at_junit(&self, junit: &JunitRun, commit: &str, tests: &[TestId]) -> At {
        self.link_modules()?;
        let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for t in tests {
            groups
                .entry(t.file.clone())
                .or_default()
                .push(t.name.clone());
        }
        let words: Vec<&str> = junit.command.split(' ').filter(|w| !w.is_empty()).collect();
        let Some((program, rest)) = words.split_first() else {
            return Err("check.command is empty".into());
        };
        let at = short(commit);
        let mut out = BTreeMap::new();
        for (file, names) in groups {
            let report = self.wt.path.join(junit.report);
            match std::fs::remove_file(&report) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("{}: {e}", junit.report)),
            }
            let args: Vec<String> = rest
                .iter()
                .map(|w| {
                    if *w == "{file}" {
                        file.clone()
                    } else {
                        w.to_string()
                    }
                })
                .collect();
            let left = self.left()?;
            let run = run_program(
                &self.script_path(program),
                &self.wt.path,
                &args,
                &[("JEST_JUNIT_REPORT_TEST_SUITE_ERRORS", "true")],
                left,
            )?;
            if run.timed_out {
                return Err(self.limit_reason());
            }
            let Some(code) = run.exit else {
                return Err(format!("the run of {file} ended by a signal (at {at})"));
            };
            let bytes = read_report_file(&report, junit.report, &file)
                .map_err(|e| format!("{e} (at {at})"))?;
            let suites =
                read_report(&bytes).map_err(|e| format!("report of {file}: {e} (at {at})"))?;
            if suites.len() != 1 {
                return Err(if suites.is_empty() {
                    format!("the report of {file} holds no test suite (at {at})")
                } else {
                    format!(
                        "the report of {file} holds {} test suites (at {at})",
                        suites.len()
                    )
                });
            }
            let failed = suites[0]
                .cases
                .iter()
                .filter(|c| c.outcome == CaseOutcome::Failed)
                .count();
            if (code == 0) != (failed == 0) {
                return Err(format!(
                    "exit code {code} with {failed} failed test case{} in the report of {file} (at {at})",
                    if failed == 1 { "" } else { "s" }
                ));
            }
            // A file that does not load: every case has the classname jest gives a failed suite.
            let cases = &suites[0].cases;
            if !cases.is_empty()
                && cases
                    .iter()
                    .all(|c| c.classname == "Test suite failed to run")
            {
                for name in names {
                    out.insert(
                        TestId {
                            dir: String::new(),
                            file: file.clone(),
                            name,
                        },
                        TestState::Compile,
                    );
                }
                continue;
            }
            for name in names {
                let cases: Vec<_> = suites
                    .iter()
                    .flat_map(|s| s.cases.iter())
                    .filter(|c| c.name.trim() == name.trim())
                    .collect();
                let [case] = cases.as_slice() else {
                    return Err(format!(
                        "{} test cases named {name} in the report of {file} (at {at})",
                        cases.len()
                    ));
                };
                let state = match case.outcome {
                    CaseOutcome::Failed => TestState::Failed,
                    CaseOutcome::Skipped => TestState::Ignored,
                    CaseOutcome::Passed => TestState::Passed,
                };
                out.insert(
                    TestId {
                        dir: String::new(),
                        file: file.clone(),
                        name,
                    },
                    state,
                );
            }
        }
        Ok(out)
    }

    /// The outcome of a step without a history outcome (`history_outcome` comes first).
    fn step(&self, a: &Analysis, cached: bool) -> (String, String) {
        let (Some(red), Some(green)) = (&a.red, &a.green) else {
            return ("unpaired".into(), String::new());
        };
        let tests = &a.to_run;
        let names = |v: Vec<&TestId>| {
            v.iter()
                .take(MAX_NAMED)
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        // Under junit a `.only` in a test file at RED or GREEN makes its tests unverified before any run.
        if self.junit.is_some() {
            for (commit, when) in [(&red.sha, "RED"), (&green.sha, "GREEN")] {
                let mut files: Vec<String> = tests.iter().map(|t| t.path()).collect();
                files.dedup();
                for path in files {
                    if show(self.root, commit, &path).is_some_and(|s| has_only_call(&s)) {
                        return (
                            "unverified".into(),
                            format!(
                                "{path} holds a .only call at {when} ({}); no command ran for it",
                                short(commit)
                            ),
                        );
                    }
                }
            }
        }
        // RED: every test fails; one that passes is no RED.
        // A cached step ran at RED and GREEN in an earlier passing check: only HEAD runs (AC-4).
        let (at_red, at_green) = if cached {
            (Ok(BTreeMap::new()), Ok(BTreeMap::new()))
        } else {
            (
                self.at(&red.sha, tests, true, &a.red_files),
                self.at(&green.sha, tests, false, &[]),
            )
        };
        // HEAD: tests missing there are removed; the rest run.
        let present: Vec<TestId> = tests
            .iter()
            .filter(|t| {
                show(self.root, self.head, &t.path())
                    .map(|src| {
                        if self.junit.is_some() {
                            test_calls(&src).iter().any(|(n, _)| *n == t.name)
                        } else {
                            test_functions(&src).iter().any(|(n, _)| *n == t.name)
                        }
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        let missing: Vec<&TestId> = tests.iter().filter(|t| !present.contains(t)).collect();
        let at_head = self.at(self.head, &present, false, &[]);

        for (state, when) in [(&at_red, "RED"), (&at_green, "GREEN"), (&at_head, "HEAD")] {
            match state {
                Err(r) => return ("unverified".into(), r.clone()),
                Ok(states) if when != "HEAD" => {
                    let ignored: Vec<&TestId> = states
                        .iter()
                        .filter(|(_, s)| **s == TestState::Ignored)
                        .map(|(t, _)| t)
                        .collect();
                    if !ignored.is_empty() {
                        return (
                            "unverified".into(),
                            format!("ignored at {when}: {}", names(ignored)),
                        );
                    }
                }
                Ok(_) => {}
            }
        }
        let (red_states, green_states, head_states) = (
            at_red.unwrap_or_default(),
            at_green.unwrap_or_default(),
            at_head.unwrap_or_default(),
        );
        let no_red: Vec<&TestId> = red_states
            .iter()
            .filter(|(_, s)| **s == TestState::Passed)
            .map(|(t, _)| t)
            .collect();
        if !no_red.is_empty() {
            return ("no-red".into(), names(no_red));
        }
        let not_green: Vec<&TestId> = green_states
            .iter()
            .filter(|(_, s)| **s != TestState::Passed)
            .map(|(t, _)| t)
            .collect();
        if !not_green.is_empty() {
            return ("not-green".into(), names(not_green));
        }
        let mut removed = missing;
        removed.extend(
            head_states
                .iter()
                .filter(|(_, s)| **s != TestState::Passed)
                .map(|(t, _)| t),
        );
        if !removed.is_empty() {
            return ("removed".into(), names(removed));
        }
        let assert = red_states
            .values()
            .filter(|s| **s == TestState::Failed)
            .count();
        let compile = red_states
            .values()
            .filter(|s| **s == TestState::Compile)
            .count();
        let n = tests.len();
        if cached {
            return ("cached".into(), String::new());
        }
        (
            "pass".into(),
            format!(
                "{n} test{}, RED {assert} assert, {compile} compile",
                if n == 1 { "" } else { "s" }
            ),
        )
    }

    /// Runs the project commands in order with `sh -c` in the check copy at HEAD, with the
    /// environment of the other runs and under the time limit of the check; prints a line for each
    /// and returns the command with its outcome. After the limit ran out, the running command is
    /// killed with its group and it and the rest are `unverified` (STP-37 AC-2, AC-4).
    fn project(&self, commands: &[String]) -> Vec<(String, &'static str)> {
        let mut done: Vec<(String, &'static str)> = Vec::new();
        let mut stop: Option<String> = None;
        let mut prepared = false;
        let mut files: Option<Result<Vec<String>, String>> = None;
        for command in commands {
            let mut command_run = command.clone();
            let shown = printable(command);
            // The limit comes first: before the checkout and before the `{files}` skip.
            if stop.is_none() && self.left().is_err() {
                stop = Some("time limit".to_string());
            }
            if stop.is_none() && has_files_word(command) {
                let list = files.get_or_insert_with(|| self.changed_files());
                match list {
                    Ok(list) if list.is_empty() => {
                        say!("project: {shown}: skipped: no changed files");
                        done.push((command.clone(), "skipped"));
                        continue;
                    }
                    Ok(list) => command_run = replace_files_word(command, list),
                    Err(e) => stop = Some(e.clone()),
                }
            }
            if stop.is_none() && !prepared {
                let result = self.wt.checkout(self.head).and_then(|()| {
                    if self.junit.is_some() {
                        self.link_modules()
                    } else {
                        Ok(())
                    }
                });
                match result {
                    Ok(()) => {
                        prepared = true;
                        self.prepared.set(true);
                    }
                    Err(e) => stop = Some(e),
                }
            }
            if stop.is_none() && self.left().is_err() {
                stop = Some("time limit".to_string());
            }
            if let Some(why) = &stop {
                say!("project: {shown}: unverified: {why}");
                done.push((command.clone(), "unverified"));
                continue;
            }
            let left = self.left().unwrap_or_default();
            // Under the cargo runner the command gets the environment of the cargo runs.
            let target = self.target.to_string_lossy().into_owned();
            let env: Vec<(&str, &str)> = if self.junit.is_some() {
                Vec::new()
            } else {
                vec![("CARGO_TARGET_DIR", &target), ("CARGO_TERM_COLOR", "never")]
            };
            let run = run_program(
                Path::new("/bin/sh"),
                &self.wt.path,
                &["-c".to_string(), command_run],
                &env,
                left,
            );
            match run {
                Err(e) => {
                    say!("project: {shown}: unverified: {e}");
                    done.push((command.clone(), "unverified"));
                }
                Ok(run) if run.timed_out => {
                    stop = Some("time limit".to_string());
                    say!("project: {shown}: unverified: time limit");
                    done.push((command.clone(), "unverified"));
                }
                Ok(run) => match run.exit {
                    Some(0) => {
                        say!("project: {shown}: pass");
                        done.push((command.clone(), "pass"));
                    }
                    Some(code) => {
                        say!("project: {shown}: fail (exit {code})");
                        let text = run.output.trim_end();
                        let lines: Vec<&str> = text.lines().collect();
                        for l in &lines[lines.len().saturating_sub(PROJECT_TAIL)..] {
                            say!("{}", printable(l));
                        }
                        done.push((command.clone(), "fail"));
                    }
                    None => {
                        say!("project: {shown}: unverified: ended by a signal");
                        done.push((command.clone(), "unverified"));
                    }
                },
            }
        }
        done
    }

    /// Runs the guards in order with `sh -c` in the check copy, each after its tracked files were set
    /// to HEAD (untracked files stay), with the environment of the project commands, under the time
    /// limit; prints a line for each and returns the outcomes (STP-41 AC-3, AC-5).
    fn guards(
        &self,
        dir: &str,
        entries: &[stapel_core::config::GuardEntry],
    ) -> Vec<(&'static str, usize, usize)> {
        let mut done: Vec<(&'static str, usize, usize)> = Vec::new();
        let lines_of = |text: &str| -> std::collections::BTreeSet<String> {
            text.lines()
                .map(|l| l.trim_end().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        };
        let mut stop: Option<String> = None;
        for entry in entries {
            let shown = printable(&entry.name);
            if stop.is_none() && self.left().is_err() {
                stop = Some("time limit".to_string());
            }
            if stop.is_none() {
                let result = if self.prepared.get() {
                    self.wt.reset_tracked(self.head)
                } else {
                    self.wt.checkout(self.head).and_then(|()| {
                        if self.junit.is_some() {
                            self.link_modules()?;
                        }
                        self.prepared.set(true);
                        Ok(())
                    })
                };
                if let Err(e) = result {
                    stop = Some(e);
                }
            }
            if stop.is_none() && self.left().is_err() {
                stop = Some("time limit".to_string());
            }
            if let Some(why) = &stop {
                say!("guard: {shown}: unverified: {why}");
                done.push(("unverified", 0, 0));
                continue;
            }
            // The baseline of a ratchet guard is the file at HEAD, read before the command runs.
            let baseline: Option<Option<String>> = entry.baseline.as_ref().map(|b| {
                let path = format!(
                    "{}:{}/{}",
                    self.head,
                    dir.strip_suffix('/').unwrap_or(dir),
                    b
                );
                // `cat-file blob` refuses a folder, which counts as no baseline.
                git_text(self.root, &["cat-file", "blob", &path]).ok()
            });
            let left = self.left().unwrap_or_default();
            let target = self.target.to_string_lossy().into_owned();
            let env: Vec<(&str, &str)> = if self.junit.is_some() {
                Vec::new()
            } else {
                vec![("CARGO_TARGET_DIR", &target), ("CARGO_TERM_COLOR", "never")]
            };
            let run = run_program(
                Path::new("/bin/sh"),
                &self.wt.path,
                &["-c".to_string(), entry.command.clone()],
                &env,
                left,
            );
            match run {
                Err(e) => {
                    say!("guard: {shown}: unverified: {e}");
                    done.push(("unverified", 0, 0));
                }
                Ok(run) if run.timed_out => {
                    stop = Some("time limit".to_string());
                    say!("guard: {shown}: unverified: time limit");
                    done.push(("unverified", 0, 0));
                }
                Ok(run) if baseline.is_some() && matches!(run.exit, Some(0 | 1)) => {
                    if run.stdout_cut {
                        say!("guard: {shown}: unverified: output cut");
                        done.push(("unverified", 0, 0));
                        continue;
                    }
                    let missing = baseline.as_ref().is_some_and(Option::is_none);
                    let base_text = baseline.flatten().unwrap_or_default();
                    let known = lines_of(&base_text);
                    let mut seen = std::collections::BTreeSet::new();
                    let fresh: Vec<String> = run
                        .stdout
                        .lines()
                        .map(|l| l.trim_end().to_string())
                        .filter(|l| !l.is_empty() && !known.contains(l) && seen.insert(l.clone()))
                        .collect();
                    if fresh.is_empty() && !missing {
                        // Tighten: the baseline's lines that are still violations, in order, once.
                        let live: std::collections::BTreeSet<String> = run
                            .stdout
                            .lines()
                            .map(|l| l.trim_end().to_string())
                            .collect();
                        let mut kept: Vec<String> = Vec::new();
                        for l in base_text.lines() {
                            let l = l.trim_end().to_string();
                            if live.contains(&l) && !l.is_empty() && !kept.contains(&l) {
                                kept.push(l);
                            }
                        }
                        let fixed = known.len() - kept.len();
                        if fixed == 0 {
                            say!("guard: {shown}: pass");
                            done.push(("pass", 0, 0));
                            continue;
                        }
                        if !run.stderr.trim().is_empty() {
                            say!("guard: {shown}: pass; baseline not tightened: standard error");
                            done.push(("pass", 0, 0));
                            continue;
                        }
                        if !run.stdout.lines().any(|l| !l.trim().is_empty()) {
                            say!("guard: {shown}: pass; baseline not tightened: no output");
                            done.push(("pass", 0, 0));
                            continue;
                        }
                        let tree_head = git_text(self.root, &["rev-parse", "HEAD"]).ok();
                        if tree_head.as_deref().map(str::trim) != Some(self.head) {
                            say!(
                                "guard: {shown}: pass; baseline not tightened: checked {} is not HEAD",
                                short(self.head)
                            );
                            done.push(("pass", 0, 0));
                            continue;
                        }
                        let base = entry.baseline.as_deref().unwrap_or_default();
                        let rel = format!("{}/{}", dir.strip_suffix('/').unwrap_or(dir), base);
                        let text: String = kept.iter().map(|l| format!("{l}\n")).collect();
                        let target = self.root.join(&rel);
                        let linked = std::fs::symlink_metadata(&target)
                            .map(|m| m.file_type().is_symlink())
                            .unwrap_or(false);
                        if linked {
                            say!("guard: {shown}: pass; baseline not written: a symlink");
                            done.push(("pass", 0, 0));
                            continue;
                        }
                        match std::fs::write(&target, text) {
                            Ok(()) => {
                                say!(
                                    "guard: {shown}: pass, {fixed} fixed removed from {}; commit it",
                                    printable(&rel)
                                );
                                done.push(("pass", 0, fixed));
                            }
                            Err(e) => {
                                say!("guard: {shown}: pass; baseline not written: {e}");
                                done.push(("pass", 0, 0));
                            }
                        }
                    } else {
                        if missing {
                            let base = entry.baseline.as_deref().unwrap_or_default();
                            let at = format!("{}/{}", dir.strip_suffix('/').unwrap_or(dir), base);
                            say!("guard: {shown}: fail: no baseline {}", printable(&at));
                        } else {
                            say!("guard: {shown}: fail: {} new", fresh.len());
                        }
                        for l in fresh.iter().take(PROJECT_TAIL) {
                            say!("  {}", printable(l));
                        }
                        done.push(("fail", fresh.len(), 0));
                    }
                }
                Ok(run) => match run.exit {
                    Some(0) => {
                        say!("guard: {shown}: pass");
                        done.push(("pass", 0, 0));
                    }
                    Some(code) => {
                        say!("guard: {shown}: fail (exit {code})");
                        let text = run.output.trim_end();
                        let lines: Vec<&str> = text.lines().collect();
                        for l in &lines[lines.len().saturating_sub(PROJECT_TAIL)..] {
                            say!("{}", printable(l));
                        }
                        done.push(("fail", 0, 0));
                    }
                    None => {
                        say!("guard: {shown}: unverified: ended by a signal");
                        done.push(("unverified", 0, 0));
                    }
                },
            }
        }
        done
    }

    /// The changed files of the ticket: the paths of each non-merge commit of the first-parent
    /// range that exist at the checked commit and do not lie under `.stapel/`; sorted, unique, each
    /// one shell-quoted word (STP-37 AC-3).
    fn changed_files(&self) -> Result<Vec<String>, String> {
        let commits = build_inputs::range(self.root, self.key, self.head, self.bounds)?;
        let mut kept = Vec::new();
        for sha in commits {
            let parents = git_text(self.root, &["rev-list", "--parents", "-n", "1", &sha])?;
            if parents.split_whitespace().count() <= 2 {
                kept.push(sha);
            }
        }
        let commits = kept.join("\n");
        let existing = git_text(
            self.root,
            &["ls-tree", "-r", "-z", "--name-only", self.head],
        )?;
        let existing: std::collections::BTreeSet<&str> =
            existing.split('\0').filter(|p| !p.is_empty()).collect();
        let mut files: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for sha in commits.lines() {
            let out = git_text(
                self.root,
                &[
                    "diff-tree",
                    "-z",
                    "--no-renames",
                    "--name-only",
                    "-r",
                    "--root",
                    sha,
                ],
            )?;
            // With a commit argument, the first item is the commit id.
            for path in out.split('\0').filter(|p| !p.is_empty() && *p != sha) {
                if existing.contains(path) && !path.starts_with(".stapel/") {
                    files.insert(path.to_string());
                }
            }
        }
        Ok(files.iter().map(|p| shell_word(p)).collect())
    }

    /// The whole suite at HEAD.
    /// The whole suite at HEAD: outcome, detail and the summed counts.
    fn suite(&self, skip: &[String]) -> Suite {
        if let Some(junit) = &self.junit {
            return self.suite_junit(junit);
        }
        let unverified = |reason: String| Suite {
            outcome: "unverified".into(),
            detail: reason,
            counts: (0, 0, 0),
            failing: Vec::new(),
            failing_more: 0,
        };
        if let Some(why) = self.outside.borrow().clone() {
            return unverified(why);
        }
        if let Err(e) = self.wt.checkout(self.head) {
            return unverified(e);
        }
        let mut args: Vec<String> = ["test", "--workspace", "--locked"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        if let Some(j) = self.jobs {
            args.push("-j".into());
            args.push(j.to_string());
        }
        if self.test_threads.is_some() || !skip.is_empty() {
            args.push("--".into());
            if let Some(t) = self.test_threads {
                args.push(format!("--test-threads={t}"));
            }
            for entry in skip {
                args.push("--skip".into());
                args.push(entry.clone());
            }
        }
        let left = match self.left() {
            Ok(l) => l,
            Err(e) => return unverified(e),
        };
        let run = match run_cargo(self.cargo, &self.wt.path, &args, &self.target, left) {
            Ok(r) => r,
            Err(e) => return unverified(e),
        };
        if let Some(why) = self.found_outside() {
            return unverified(why);
        }
        if run.timed_out {
            return unverified(self.limit_reason());
        }
        if run.output.contains("because --locked was passed") {
            return unverified("Cargo.lock is out of date".into());
        }
        let (mut p, mut f, mut i) = (0u64, 0u64, 0u64);
        for (a, b, c) in run.output.lines().filter_map(summary_counts) {
            p = p.saturating_add(a);
            f = f.saturating_add(b);
            i = i.saturating_add(c);
        }
        let mut detail = format!("{p} passed, {f} failed, {i} ignored");
        let compiles = !run
            .output
            .lines()
            .any(|l| l.starts_with("error: could not compile"));
        if !skip.is_empty() {
            detail = format!("{detail} (left out: {})", skip.join(", "));
        }
        let (outcome, detail) = if !compiles {
            ("fail", format!("{detail}; the workspace does not compile"))
        } else if run.exit == Some(0) && f == 0 {
            ("pass", detail)
        } else {
            ("fail", detail)
        };
        let mut failing = cargo_failing_names(&run.stdout, &run.stderr);
        let failing_more = failing.len().saturating_sub(MAX_FAILING);
        failing.truncate(MAX_FAILING);
        Suite {
            outcome: outcome.into(),
            detail,
            counts: (p, f, i),
            failing,
            failing_more,
        }
    }
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

/// Reads the report of a Per-file run (`what` is the test file) or of the suite (`what` is
/// `the suite`). The kind and the size are checked before the read, so a FIFO does not block it.
fn read_report_file(path: &Path, name: &str, what: &str) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::FileTypeExt;
    let meta =
        std::fs::symlink_metadata(path).map_err(|e| format!("no report {name} for {what}: {e}"))?;
    let kind = meta.file_type();
    let not_regular = if kind.is_symlink() {
        Some("a symbolic link")
    } else if kind.is_dir() {
        Some("a folder")
    } else if kind.is_fifo() {
        Some("a FIFO")
    } else if kind.is_socket() {
        Some("a socket")
    } else if kind.is_block_device() {
        Some("a block device")
    } else if kind.is_char_device() {
        Some("a character device")
    } else if !kind.is_file() {
        Some("of an unknown kind")
    } else {
        None
    };
    if let Some(what_it_is) = not_regular {
        return Err(format!("the report {name} of {what} is {what_it_is}"));
    }
    if meta.len() > MAX_REPORT as u64 {
        return Err(format!(
            "the report of {what} is over {} MiB",
            MAX_REPORT / 1024 / 1024
        ));
    }
    // One open decides: no link is followed and a FIFO swapped in after the check does not block;
    // the open file must be regular and its read is bounded.
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| format!("no report {name} for {what}: {e}"))?;
    let open_meta = file
        .metadata()
        .map_err(|e| format!("no report {name} for {what}: {e}"))?;
    if !open_meta.file_type().is_file() {
        return Err(format!("the report {name} of {what} is not a regular file"));
    }
    read_open_report(&mut file).map_err(|e| {
        if e == OVER_SIZE {
            format!(
                "the report of {what} is over {} MiB",
                MAX_REPORT / 1024 / 1024
            )
        } else {
            format!("no report {name} for {what}: {e}")
        }
    })
}

/// The reason of `unverified` when a commit of the ticket changed a build input.
fn changed_reason(c: &Change) -> String {
    format!("build-input-changed: {} at {}", c.path, short(&c.sha))
}

/// A step's outcome line and its notes, as both modes and the commit hook print them.
fn step_lines(a: &Analysis, outcome: &str, reason: &str) -> Vec<String> {
    let mut lines = vec![if outcome == "cached" {
        format!("{}: cached from {reason}", a.label)
    } else if reason.is_empty() {
        format!("{}: {outcome}", a.label)
    } else {
        format!("{}: {outcome}: {reason}", a.label)
    }];
    for n in &a.notes {
        lines.push(format!("  {} {}: {}", n.kind, n.by, n.what));
    }
    lines
}

fn say_step(a: &Analysis, outcome: &str, reason: &str) {
    for line in step_lines(a, outcome, reason) {
        say!("{line}");
    }
}

/// One step of the history check: its analysis and its history outcome, `None` for `needs-run`.
pub(crate) struct HistoryStep {
    pub analysis: Analysis,
    pub outcome: Option<(String, String)>,
    /// The commit of the build input or configuration change that blocks the step (AC-41): under
    /// junit the step's own blocking change, under cargo the first change of the range.
    pub changed: Option<String>,
}

/// What the history alone says about a ticket at `at` (STP-14): no cargo, no worktree.
pub(crate) struct HistoryReport {
    pub inputs: BuildInputs,
    pub steps: Vec<HistoryStep>,
    /// STP-37 AC-13: the history commits of the range as (full id, reason), oldest first.
    pub history: Vec<(String, String)>,
}

pub(crate) fn history_report(
    root: &Path,
    key: &str,
    at: &str,
    steps: &[Step],
    bounds: Bounds,
) -> Result<HistoryReport, String> {
    let globs = junit_globs_at(root)?;
    let analyses = analyse_in(
        root,
        key,
        at,
        steps,
        bounds.base,
        bounds.base_branch,
        mode_of(&globs),
    )?;
    let junit_check = repo::load(root)?.check.filter(|c| c.runner == "junit");
    let (inputs, walk) = match &junit_check {
        Some(check) => (
            junit_inputs(root, key, at, bounds, check)?,
            build_inputs::range(root, key, at, bounds)?,
        ),
        None => (
            build_inputs::analyse(root, key, at, steps, bounds)?,
            Vec::new(),
        ),
    };
    let steps = analyses
        .into_iter()
        .map(|analysis| {
            let changed = if junit_check.is_some() {
                junit_block(&analysis, &inputs, &walk)
            } else {
                inputs.changed.as_ref()
            };
            let blocked = changed.map(changed_reason);
            HistoryStep {
                outcome: history_outcome(&analysis, blocked.as_deref()),
                changed: changed.map(|c| c.sha.clone()),
                analysis,
            }
        })
        .collect();
    let history = build_inputs::history_commits(root, key, at, bounds)?;
    Ok(HistoryReport {
        inputs,
        steps,
        history,
    })
}

/// `stapel check --history`: the full check's lines for the history outcomes, `needs-run` for the
/// rest; exit 1 when an outcome fails. Nothing is written.
fn print_history(
    root: &Path,
    key: &str,
    head: &str,
    end: Option<&str>,
    steps: &[Step],
    bounds: Bounds,
) -> ExitCode {
    let at = end.unwrap_or(head);
    let report = match history_report(root, key, at, steps, bounds) {
        Ok(r) => r,
        Err(e) => return refuse(e),
    };
    if end.is_some() {
        say!("ticket: {key} at {} (closed)", short(at));
    } else {
        say!("ticket: {key} at {}", short(head));
    }
    if !report.inputs.unchanged.is_empty() {
        say!("build inputs: {}", named(&report.inputs.unchanged));
    }
    let mut ok = true;
    for s in &report.steps {
        match &s.outcome {
            Some((outcome, reason)) => {
                ok &= passing(outcome);
                say_step(&s.analysis, outcome, reason);
            }
            None => say_step(&s.analysis, "needs-run", ""),
        }
    }
    for line in history_lines(&report.history) {
        say!("{line}");
    }
    if ok {
        say!("result: no history faults");
        ExitCode::SUCCESS
    } else {
        say!("result: fail");
        ExitCode::from(1)
    }
}

/// States under the git directory while a rebase, merge, cherry-pick or revert is in progress.
const IN_PROGRESS: [&str; 5] = [
    "rebase-merge",
    "rebase-apply",
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
];

/// The commit hook's lines for the repository at `root` (STP-14 AC-11 to AC-14, AC-18, AC-20):
/// the line of each hook fault, one advice line per fault commit in history order, and the
/// pointer to the history check. `None` when no hook fault has `HEAD` as its fault commit, and
/// for every case where the hook stays silent: an operation in progress, no ticket key in the
/// subject of `HEAD`, a ticket that is not open, no step commits, a refusal of the history check,
/// or any error.
pub(crate) fn hook_faults(root: &Path) -> Option<Vec<String>> {
    let head = read_head(root).ok()?;
    let subject = git_text(root, &["log", "-1", "--format=%s", &head]).ok()?;
    // The key: the subject's prefix up to the first space or colon.
    let word = subject
        .split(|c: char| c.is_whitespace() || c == ':')
        .next()
        .filter(|w| !w.is_empty());
    let ticket = word.and_then(|w| resolve(root, Some(w)).ok());
    // A `stapel.toml` that fails to load: its fault in place of every step line, also during an
    // operation, for a `HEAD` that names an open ticket (STP-30 AC-11).
    if let Err(fault) = repo::load(root) {
        let open = ticket.is_some_and(|t| matches!(t.status, Status::Open(_)));
        return open.then(|| vec![format!("stapel: {fault}")]);
    }
    for name in IN_PROGRESS {
        let path = git_text(root, &["rev-parse", "--git-path", name]).ok()?;
        if root.join(path.trim()).exists() {
            return None;
        }
    }
    let ticket = ticket?;
    if !matches!(ticket.status, Status::Open(_)) {
        return None;
    }
    let key = ticket.key.clone();
    shallow_refusal(root).ok()?;
    let base = base_of(root, &ticket, &head).ok()?;
    let commits = step_commits(root, &key, &head, base.as_deref()).ok()?;
    let last = commits.last().map(|c| c.sha.clone())?;
    let steps = steps(commits);
    let base_branch = base_branch_of(&ticket);
    let bounds = Bounds {
        base: base.as_deref(),
        base_branch: base_branch.as_deref(),
        closed: false,
    };
    let report = history_report(root, &key, &head, &steps, bounds).ok()?;

    let mut lines = Vec::new();
    // Fault commits: id, labels, and whether an amend removes every fault of the commit.
    let mut faults: Vec<(String, Vec<String>, bool)> = Vec::new();
    // Whether `HEAD` is a commit of a step with a hook fault (R2-1).
    let mut seen = false;
    for (step, s) in steps.iter().zip(&report.steps) {
        let a = &s.analysis;
        let open = match step.commits.as_slice() {
            [only] if only.marker == Marker::Red && only.sha == last => Some(only.sha.clone()),
            _ => None,
        };
        let (outcome, reason, fault, amend) = if let Some(red) = open {
            match open_red(root, &red, &head, mode_of(&junit_globs_at(root).ok()?)).ok()? {
                Some((o, r)) => (o, r, Some(red), true),
                None => continue,
            }
        } else {
            match &s.outcome {
                Some((o, r)) if !passing(o) => {
                    let (fault, amend) = match (&a.fixed, &s.changed) {
                        (Some((fixed, _)), _) => (
                            a.fault.clone(),
                            matches!(
                                fixed.as_str(),
                                "no-tests" | "red-changes-code" | "tests-changed"
                            ),
                        ),
                        (None, Some(changed)) => (Some(changed.clone()), true),
                        (None, None) => (a.fault.clone(), false),
                    };
                    (o.clone(), r.clone(), fault, amend)
                }
                _ => continue,
            }
        };
        lines.extend(step_lines(a, &outcome, &reason));
        seen |= step.commits.iter().any(|c| c.sha == head);
        let Some(sha) = fault else { continue };
        match faults.iter_mut().find(|f| f.0 == sha) {
            Some(f) => {
                f.1.push(a.label.clone());
                f.2 &= amend;
            }
            None => faults.push((sha, vec![a.label.clone()], amend)),
        }
    }
    // Faults below `HEAD` of steps without a commit at `HEAD` were reported before (AC-20).
    if lines.is_empty() || !(seen || faults.iter().any(|f| f.0 == head)) {
        return None;
    }
    let walk = stapel_core::steps::walk_range(base.as_deref(), &head);
    let order = git_text(root, &["rev-list", "--first-parent", "--reverse", &walk]).ok()?;
    let position: BTreeMap<&str, usize> = order.lines().enumerate().map(|(i, s)| (s, i)).collect();
    faults.sort_by_key(|f| position.get(f.0.as_str()).copied().unwrap_or(usize::MAX));
    for (sha, labels, amend) in faults {
        let labels = labels.join(", ");
        if sha == head && amend {
            lines.push(format!(
                "stapel: {} is HEAD; fix it with git commit --amend ({labels})",
                short(&sha)
            ));
        } else {
            lines.push(format!(
                "stapel: {} is below HEAD or an amend cannot fix it; stop and ask the human \
                 ({labels})",
                short(&sha)
            ));
        }
    }
    lines.push(format!("details: stapel check {key} --history"));
    Some(lines)
}

fn print_list(root: &std::path::Path, key: &str, steps: &[Step], mode: Mode) -> Result<(), String> {
    say!("ticket: {key}");
    for step in steps {
        say!("{}", step.label);
        let reds: Vec<_> = step.reds().collect();
        let greens: Vec<_> = step.greens().collect();
        for (name, commits) in [("red", &reds), ("green", &greens)] {
            if commits.is_empty() {
                say!("  {name}: —");
            }
            for c in commits {
                say!("  {name}: {} {}", short(&c.sha), c.subject);
            }
        }
        let tests = match reds.first() {
            Some(red) => match mode {
                Mode::Cargo => step_tests(root, &red.sha)?,
                Mode::Junit(globs) => junit_step_tests(root, &red.sha, globs)?,
            },
            None => Vec::new(),
        };
        say!("  tests: {}", tests.len());
        for t in tests {
            say!("    {t}");
        }
    }
    Ok(())
}
