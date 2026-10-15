//! `stapel check [KEY] [--list | --history]`: the RED to GREEN check of a ticket's steps (STP-4);
//! `--history` prints the outcomes that history alone gives, without cargo (STP-14).

use crate::repo;
use stapel_core::build_inputs;
use stapel_core::build_inputs::{Bounds, BuildInputs, Change};
use stapel_core::config::{validate_cargo, validate_path_cargo};
use stapel_core::journal::{append, new_id};
use stapel_core::outcomes::{
    Analysis, MAX_NAMED, analyse, history_outcome, named, open_red, passing,
};
use stapel_core::runner::{
    Parsed, TestState, cargo_version, parse_run, resolve_cargo, run_cargo, summary_counts,
};
use stapel_core::rust_tests::test_functions;
use stapel_core::steps::{
    Marker, Step, TestId, git_text, package_name, show, step_commits, step_tests, steps,
};
use stapel_core::tickets::{Status, Ticket, resolve};
use stapel_core::time::now_rfc3339;
use stapel_core::worktree::{
    Lock, LockError, Worktree, build_input_outside, dirty_paths, resolve_tmpdir, stapel_dir,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

/// A refusal: nothing was run, exit 2.
fn refuse(message: impl std::fmt::Display) -> ExitCode {
    warn!("stapel: {message}");
    ExitCode::from(2)
}

// The refusals both modes share (STP-14); each mode calls them in its own order.

/// A legacy or unreadable ticket cannot be checked.
fn status_refusal(ticket: &Ticket) -> Result<(), String> {
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
fn read_head(root: &Path) -> Result<String, String> {
    git_text(root, &["rev-parse", "--verify", "HEAD"]).map(|h| h.trim().to_string())
}

/// A shallow clone hides history the check reads.
fn shallow_refusal(root: &Path) -> Result<(), String> {
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
fn base_of(root: &Path, ticket: &Ticket, head: &str) -> Result<Option<String>, String> {
    let base = match &ticket.status {
        Status::Open(s) | Status::Closed(s) => s.base.clone(),
        _ => None,
    };
    if let Some(base) = &base {
        build_inputs::check_base(root, base, head)?;
    }
    Ok(base)
}

/// The steps of the ticket on the first-parent history of `head`.
fn read_steps(root: &Path, key: &str, head: &str) -> Result<Vec<Step>, String> {
    step_commits(root, key, head).map(steps)
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
    let steps = match read_steps(&root, &ticket.key, &head) {
        Ok(s) => s,
        Err(e) => return refuse(e),
    };
    if list {
        if steps.is_empty() {
            return refuse(no_steps(&ticket.key));
        }
        return match print_list(&root, &ticket.key, &steps) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => refuse(e),
        };
    }
    let bounds = Bounds {
        base: base.as_deref(),
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
    let analyses = match analyse(&root, &ticket.key, &at, &steps) {
        Ok(a) => a,
        Err(e) => return refuse(e),
    };
    let inputs = match build_inputs::analyse(&root, &ticket.key, &at, &steps, bounds) {
        Ok(i) => i,
        Err(e) => return refuse(e),
    };
    let tmp = match resolve_tmpdir(&root) {
        Ok(t) => t,
        Err(e) => return refuse(e),
    };
    let wt = match Worktree::create(&root, &dir, &at, &tmp) {
        Ok(w) => w,
        Err(e) => return refuse(e),
    };
    let cargo = match resolve_cargo(check.cargo.as_deref()) {
        Ok(c) => c,
        Err(e) => return refuse(e),
    };
    match check.cargo.as_deref() {
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
    let target = dir.join("check-target");
    let timeout = Duration::from_secs(check.timeout_secs);
    // A configuration above the worktree is found before any cargo run (AC-7).
    let outside = build_input_outside(&wt.path);
    // A build input changed by a commit of the ticket comes first (STP-6 AC-1, AC-2); no cargo runs.
    let mut blocked = inputs
        .changed
        .as_ref()
        .map(changed_reason)
        .or(outside.map(|p| format!("build-input-outside: {}", p.display())));
    let cargo_text = if blocked.is_some() {
        cargo.display().to_string()
    } else {
        match cargo_version(&cargo, &wt.path, &target, timeout) {
            Ok(v) => format!("{} ({v})", cargo.display()),
            Err(e) => return refuse(format!("{}: {e}", cargo.display())),
        }
    };
    // The search is repeated after every cargo run, `cargo --version` included (AC-7).
    if blocked.is_none() {
        blocked =
            build_input_outside(&wt.path).map(|p| format!("build-input-outside: {}", p.display()));
    }
    let runner = Runner {
        root: &root,
        wt: &wt,
        cargo: &cargo,
        target,
        timeout,
        head: &at,
        outside: std::cell::RefCell::new(blocked),
    };
    if end.is_some() {
        say!("ticket: {} at {} (closed)", ticket.key, short(&at));
    } else {
        say!("ticket: {} at {}", ticket.key, short(&head));
    }
    say!("cargo: {cargo_text}");
    if !inputs.unchanged.is_empty() {
        say!("build inputs: {}", named(&inputs.unchanged));
    }
    let mut ok = true;
    let mut steps_json = Vec::new();
    for a in &analyses {
        // A build input found above the worktree after a cargo run blocks every later step (AC-7).
        let blocked = runner.outside.borrow().clone();
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
        "cargo": cargo_text,
        "result": if ok { "pass" } else { "fail" },
        "steps": steps_json,
        "suite": {
            "outcome": suite.outcome,
            "passed": suite.counts.0,
            "failed": suite.counts.1,
            "ignored": suite.counts.2,
        },
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
        // RED: every test fails; one that passes is no RED.
        let at_red = self.at(&red.sha, tests, true);
        let at_green = self.at(&green.sha, tests, false);
        // HEAD: tests missing there are removed; the rest run.
        let present: Vec<TestId> = tests
            .iter()
            .filter(|t| {
                show(self.root, self.head, &t.path())
                    .map(|src| test_functions(&src).iter().any(|(n, _)| *n == t.name))
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
    let analyses = analyse(root, key, at, steps)?;
    let inputs = build_inputs::analyse(root, key, at, steps, bounds)?;
    let blocked = inputs.changed.as_ref().map(changed_reason);
    let steps = analyses
        .into_iter()
        .map(|analysis| HistoryStep {
            outcome: history_outcome(&analysis, blocked.as_deref()),
            analysis,
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

/// The commit hook's lines for the repository at `root` (STP-14 AC-11 to AC-14, AC-18): the line
/// of each hook fault, one advice line per fault commit in history order, and the pointer to the
/// history check. `None` when there is no hook fault, and for every case where the hook stays
/// silent: an operation in progress, no ticket key in the subject of `HEAD`, a ticket that is not
/// open, no step commits, a refusal of the history check, or any error.
pub(crate) fn hook_faults(root: &Path) -> Option<Vec<String>> {
    for name in IN_PROGRESS {
        let path = git_text(root, &["rev-parse", "--git-path", name]).ok()?;
        if root.join(path.trim()).exists() {
            return None;
        }
    }
    let head = read_head(root).ok()?;
    let subject = git_text(root, &["log", "-1", "--format=%s", &head]).ok()?;
    // The key: the subject's prefix up to the first space or colon.
    let word = subject
        .split(|c: char| c.is_whitespace() || c == ':')
        .next()
        .filter(|w| !w.is_empty())?;
    let ticket = resolve(root, Some(word)).ok()?;
    if !matches!(ticket.status, Status::Open(_)) {
        return None;
    }
    let key = ticket.key.clone();
    shallow_refusal(root).ok()?;
    let base = base_of(root, &ticket, &head).ok()?;
    let commits = step_commits(root, &key, &head).ok()?;
    let last = commits.last().map(|c| c.sha.clone())?;
    let steps = steps(commits);
    let bounds = Bounds {
        base: base.as_deref(),
        closed: false,
    };
    let report = history_report(root, &key, &head, &steps, bounds).ok()?;

    let mut lines = Vec::new();
    // Fault commits: id, labels, and whether an amend removes every fault of the commit.
    let mut faults: Vec<(String, Vec<String>, bool)> = Vec::new();
    for (step, s) in steps.iter().zip(&report.steps) {
        let a = &s.analysis;
        let open = match step.commits.as_slice() {
            [only] if only.marker == Marker::Red && only.sha == last => Some(only.sha.clone()),
            _ => None,
        };
        let (outcome, reason, fault, amend) = if let Some(red) = open {
            match open_red(root, &red, &head).ok()? {
                Some((o, r)) => (o, r, Some(red), true),
                None => continue,
            }
        } else {
            match &s.outcome {
                Some((o, r)) if !passing(o) => {
                    let (fault, amend) = match (&a.fixed, &report.inputs.changed) {
                        (Some((fixed, _)), _) => (
                            a.fault.clone(),
                            matches!(
                                fixed.as_str(),
                                "no-tests" | "red-changes-code" | "tests-changed"
                            ),
                        ),
                        (None, Some(changed)) => (Some(changed.sha.clone()), true),
                        (None, None) => (a.fault.clone(), false),
                    };
                    (o.clone(), r.clone(), fault, amend)
                }
                _ => continue,
            }
        };
        lines.extend(step_lines(a, &outcome, &reason));
        let Some(sha) = fault else { continue };
        match faults.iter_mut().find(|f| f.0 == sha) {
            Some(f) => {
                f.1.push(a.label.clone());
                f.2 &= amend;
            }
            None => faults.push((sha, vec![a.label.clone()], amend)),
        }
    }
    if lines.is_empty() {
        return None;
    }
    let order = git_text(root, &["rev-list", "--first-parent", "--reverse", &head]).ok()?;
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

fn print_list(root: &std::path::Path, key: &str, steps: &[Step]) -> Result<(), String> {
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
            Some(red) => step_tests(root, &red.sha)?,
            None => Vec::new(),
        };
        say!("  tests: {}", tests.len());
        for t in tests {
            say!("    {t}");
        }
    }
    Ok(())
}
