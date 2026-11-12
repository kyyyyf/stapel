//! `stapel check [KEY] [--list | --history]`: the RED to GREEN check of a ticket's steps (STP-4);
//! `--history` prints the outcomes that history alone gives, without cargo (STP-14).

use crate::repo;
use stapel_core::build_inputs;
use stapel_core::build_inputs::{Bounds, BuildInputs, Change};
use stapel_core::config::{Check, validate_cargo, validate_path_cargo};
use stapel_core::journal::{append, new_id};
use stapel_core::junit::{MAX_REPORT, Outcome as CaseOutcome, read_open_report, read_report};
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
            if let Some(rel) = script_under(root, word) {
                fixed.push(rel.to_string());
            }
        } else if word.contains('/') {
            fixed.push(word.trim_start_matches("./").to_string());
        }
    }
    let inputs = check.inputs.clone().unwrap_or_default();
    build_inputs::analyse_junit(root, key, at, bounds, &inputs, &fixed)
}

/// The part of the absolute `word` below `root`, when it has no `.`, `..` or empty part (STP-30 AC-6).
fn script_under<'a>(root: &Path, word: &'a str) -> Option<&'a str> {
    let rel = word.strip_prefix(root.to_str()?.trim_end_matches('/'))?.strip_prefix('/')?;
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
fn read_steps(
    root: &Path,
    key: &str,
    head: &str,
    base: Option<&str>,
) -> Result<Vec<Step>, String> {
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

pub fn run(key: Option<&str>, list: bool, history: bool) -> ExitCode {
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
    let analyses = match analyse_in(&root, &ticket.key, &at, &steps, bounds.base, bounds.base_branch, mode_of(&globs)) {
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
        head: &at,
        outside: std::cell::RefCell::new(blocked),
        junit: junit_run,
    };
    if end.is_some() {
        say!("ticket: {} at {} (closed)", ticket.key, short(&at));
    } else {
        say!("ticket: {} at {}", ticket.key, short(&head));
    }
    if !junit {
        say!("cargo: {cargo_text}");
    }
    if !inputs.unchanged.is_empty() {
        say!("build inputs: {}", named(&inputs.unchanged));
    }
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
            None => runner.step(a),
        };
        ok &= passing(&outcome);
        if steps_json.len() < MAX_STEPS {
            steps_json.push(serde_json::json!({
                "label": cut(&a.label, MAX_LABEL),
                "red": a.red.as_ref().map(|c| c.sha.clone()),
                "green": a.green.as_ref().map(|c| c.sha.clone()),
                "outcome": outcome,
                "reason": cut(&reason, MAX_REASON),
                "tests": a.tests.len(),
                "names": names_of(&outcome, &reason),
            }));
        }
        say_step(a, &outcome, &reason);
    }
    let suite = runner.suite();
    ok &= suite.outcome == "pass";
    say!("suite: {}: {}", suite.outcome, suite.detail);
    say!("result: {}", if ok { "pass" } else { "fail" });
    let mut record = serde_json::json!({
        "v": 1,
        "kind": "check",
        "id": new_id("c"),
        "at": now_rfc3339(),
        "ticket": ticket.key,
        "head": head,
        "tool": format!("stapel {}", env!("CARGO_PKG_VERSION")),
    });
    if junit {
        record["runner"] = serde_json::json!("junit");
    } else {
        record["cargo"] = serde_json::json!(cargo_text);
    }
    record["result"] = serde_json::json!(if ok { "pass" } else { "fail" });
    record["steps"] = serde_json::json!(steps_json);
    record["suite"] = serde_json::json!({
        "outcome": suite.outcome,
        "passed": suite.counts.0,
        "failed": suite.counts.1,
        "ignored": suite.counts.2,
    });
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
}

/// At most this many steps go into a record, and this many bytes of a label.
const MAX_STEPS: usize = 200;
const MAX_LABEL: usize = 200;
const MAX_REASON: usize = 1024;
/// A record stays below the journal's 64 KiB line limit with room to spare.
const MAX_RECORD: usize = 60 * 1024;

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
    head: &'a str,
    /// Set when a build input lies above the worktree: no cargo runs, and every step and the suite
    /// read `unverified: build-input-outside: <path>`.
    outside: std::cell::RefCell<Option<String>>,
    /// Set under the junit runner: the command and the report path of `[check]`.
    junit: Option<JunitRun<'a>>,
}

/// The `[check]` keys of a junit Per-file run.
struct JunitRun<'a> {
    command: &'a str,
    report: &'a str,
}

/// The state of each test at one commit, or why the run does not count.
type At = Result<BTreeMap<TestId, TestState>, String>;

impl Runner<'_> {
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
    fn at(&self, commit: &str, tests: &[TestId], at_red: bool) -> At {
        // After a build-input hit no further cargo run starts (AC-7).
        if let Some(why) = self.outside.borrow().clone() {
            return Err(why);
        }
        self.wt.checkout(commit)?;
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
            let mut args: Vec<String> = [
                "test", "--locked", "-p", &package, "--test", &file, "--", "--exact",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            args.extend(names.iter().cloned());
            let run = run_cargo(self.cargo, &self.wt.path, &args, &self.target, self.timeout)?;
            if let Some(why) = self.found_outside() {
                return Err(why);
            }
            if run.timed_out {
                return Err(format!(
                    "the time limit of {} s ran out",
                    self.timeout.as_secs()
                ));
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
        let run = match run_program(
            &self.script_path(program),
            &self.wt.path,
            &args,
            &[("JEST_JUNIT_REPORT_TEST_SUITE_ERRORS", "true")],
            self.timeout,
        ) {
            Ok(r) => r,
            Err(e) => return unverified(e),
        };
        if run.timed_out {
            return unverified(format!(
                "the time limit of {} s ran out",
                self.timeout.as_secs()
            ));
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
        Suite {
            outcome: outcome.into(),
            detail,
            counts: (p, f, i),
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
            let run = run_program(
                &self.script_path(program),
                &self.wt.path,
                &args,
                &[("JEST_JUNIT_REPORT_TEST_SUITE_ERRORS", "true")],
                self.timeout,
            )?;
            if run.timed_out {
                return Err(format!(
                    "the time limit of {} s ran out",
                    self.timeout.as_secs()
                ));
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
            if !cases.is_empty() && cases.iter().all(|c| c.classname == "Test suite failed to run")
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
    fn step(&self, a: &Analysis) -> (String, String) {
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
        let at_red = self.at(&red.sha, tests, true);
        let at_green = self.at(&green.sha, tests, false);
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
        let at_head = self.at(self.head, &present, false);

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
        (
            "pass".into(),
            format!(
                "{n} test{}, RED {assert} assert, {compile} compile",
                if n == 1 { "" } else { "s" }
            ),
        )
    }

    /// The whole suite at HEAD.
    /// The whole suite at HEAD: outcome, detail and the summed counts.
    fn suite(&self) -> Suite {
        if let Some(junit) = &self.junit {
            return self.suite_junit(junit);
        }
        let unverified = |reason: String| Suite {
            outcome: "unverified".into(),
            detail: reason,
            counts: (0, 0, 0),
        };
        if let Some(why) = self.outside.borrow().clone() {
            return unverified(why);
        }
        if let Err(e) = self.wt.checkout(self.head) {
            return unverified(e);
        }
        let args: Vec<String> = ["test", "--workspace", "--locked"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let run = match run_cargo(self.cargo, &self.wt.path, &args, &self.target, self.timeout) {
            Ok(r) => r,
            Err(e) => return unverified(e),
        };
        if let Some(why) = self.found_outside() {
            return unverified(why);
        }
        if run.timed_out {
            return unverified(format!(
                "the time limit of {} s ran out",
                self.timeout.as_secs()
            ));
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
        let detail = format!("{p} passed, {f} failed, {i} ignored");
        let compiles = !run
            .output
            .lines()
            .any(|l| l.starts_with("error: could not compile"));
        let (outcome, detail) = if !compiles {
            ("fail", format!("{detail}; the workspace does not compile"))
        } else if run.exit == Some(0) && f == 0 {
            ("pass", detail)
        } else {
            ("fail", detail)
        };
        Suite {
            outcome: outcome.into(),
            detail,
            counts: (p, f, i),
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
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| format!("no report {name} for {what}: {e}"))?;
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
        return Err(format!("the report of {what} is over {} MiB", MAX_REPORT / 1024 / 1024));
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
        if e.contains("MiB") {
            format!("the report of {what} is over {} MiB", MAX_REPORT / 1024 / 1024)
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
    let mut lines = vec![if reason.is_empty() {
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
}

pub(crate) fn history_report(
    root: &Path,
    key: &str,
    at: &str,
    steps: &[Step],
    bounds: Bounds,
) -> Result<HistoryReport, String> {
    let globs = junit_globs_at(root)?;
    let analyses = analyse_in(root, key, at, steps, bounds.base, bounds.base_branch, mode_of(&globs))?;
    let junit_check = repo::load(root)?
        .check
        .filter(|c| c.runner == "junit");
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
    Ok(HistoryReport { inputs, steps })
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
