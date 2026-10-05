//! `stapel check [KEY] [--list]`: the RED to GREEN check of a ticket's steps (STP-4).

use crate::repo;
use stapel_core::outcomes::{Analysis, MAX_NAMED, analyse, passing};
use stapel_core::runner::{Parsed, TestState, parse_run, run_cargo, summary_counts};
use stapel_core::rust_tests::test_functions;
use stapel_core::steps::{
    Step, TestId, git_text, package_name, show, step_commits, step_tests, steps,
};
use stapel_core::tickets::{Status, resolve};
use stapel_core::worktree::{Lock, LockError, Worktree, dirty_paths, stapel_dir};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

/// A refusal: nothing was run, exit 2.
fn refuse(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("stapel: {message}");
    ExitCode::from(2)
}

pub fn run(key: Option<&str>, list: bool) -> ExitCode {
    let (root, config) = match repo::open() {
        Ok(v) => v,
        Err(e) => return refuse(e),
    };
    let ticket = match resolve(&root, key) {
        Ok(t) => t,
        Err(e) => return refuse(e),
    };
    match &ticket.status {
        Status::Open(_) | Status::Closed(_) => {}
        Status::Legacy(_) => {
            return refuse(format!(
                "ticket {} has a legacy (STP-1) state.json; it cannot be checked",
                ticket.key
            ));
        }
        Status::Unreadable(r) => return refuse(r),
    }
    let head = match git_text(&root, &["rev-parse", "--verify", "HEAD"]) {
        Ok(h) => h.trim().to_string(),
        Err(e) => return refuse(e),
    };
    let steps = match step_commits(&root, &ticket.key, &head) {
        Ok(c) => steps(c),
        Err(e) => return refuse(e),
    };
    let no_steps = || {
        refuse(format!(
            "ticket {key} has no step commits (subjects `{key} <label> RED: …` and \
             `{key} <label> GREEN: …` on the first-parent history of HEAD)",
            key = ticket.key
        ))
    };
    if list {
        if steps.is_empty() {
            return no_steps();
        }
        return match print_list(&root, &ticket.key, &steps) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => refuse(e),
        };
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
        return no_steps();
    }
    let dir = match stapel_dir(&root) {
        Ok(d) => d,
        Err(e) => return refuse(e),
    };
    let _lock = match Lock::take(&dir) {
        Ok((lock, previous)) => {
            if let Some(pid) = previous {
                eprintln!(
                    "notice: took over the check lock of process {pid}, which is not running"
                );
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
    let analyses = match analyse(&root, &ticket.key, &head, &steps) {
        Ok(a) => a,
        Err(e) => return refuse(e),
    };
    let wt = match Worktree::create(&root, &dir, &head) {
        Ok(w) => w,
        Err(e) => return refuse(e),
    };
    let runner = Runner {
        root: &root,
        wt: &wt,
        target: dir.join("check-target"),
        timeout: Duration::from_secs(check.timeout_secs),
        head: &head,
    };
    println!("ticket: {} at {}", ticket.key, short(&head));
    let mut ok = true;
    for a in &analyses {
        let (outcome, reason) = match &a.fixed {
            Some((o, r)) => (o.clone(), r.clone()),
            None => runner.step(a),
        };
        ok &= passing(&outcome);
        if reason.is_empty() {
            println!("{}: {outcome}", a.label);
        } else {
            println!("{}: {outcome}: {reason}", a.label);
        }
        for n in &a.notes {
            println!("  {} {}: {}", n.kind, n.by, n.test);
        }
    }
    let (suite, detail) = runner.suite();
    ok &= suite == "pass";
    println!("suite: {suite}: {detail}");
    println!("result: {}", if ok { "pass" } else { "fail" });
    if let Ok(now) = git_text(&root, &["rev-parse", "--verify", "HEAD"])
        && now.trim() != head
    {
        eprintln!(
            "notice: HEAD moved to {} while the check ran; this check is of {}",
            short(now.trim()),
            short(&head)
        );
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

struct Runner<'a> {
    root: &'a Path,
    wt: &'a Worktree,
    target: PathBuf,
    timeout: Duration,
    head: &'a str,
}

/// The state of each test at one commit, or why the run does not count.
type At = Result<BTreeMap<TestId, TestState>, String>;

impl Runner<'_> {
    fn package(&self, commit: &str, dir: &str) -> Result<String, String> {
        package_name(self.root, commit, dir)
    }

    /// Runs `tests` at `commit`, one cargo run per test file.
    fn at(&self, commit: &str, tests: &[TestId], at_red: bool) -> At {
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
            let run = run_cargo(&self.wt.path, &args, &self.target, self.timeout)?;
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
        // RED: every test fails; one that passes runs once more.
        let at_red = self.at(&red.sha, tests, true).and_then(|mut states| {
            let passed: Vec<TestId> = states
                .iter()
                .filter(|(_, s)| **s == TestState::Passed)
                .map(|(t, _)| t.clone())
                .collect();
            if !passed.is_empty() {
                for (t, s) in self.at(&red.sha, &passed, true)? {
                    states.insert(t, s);
                }
            }
            Ok(states)
        });
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
    fn suite(&self) -> (String, String) {
        if let Err(e) = self.wt.checkout(self.head) {
            return ("unverified".into(), e);
        }
        let args: Vec<String> = ["test", "--workspace", "--locked"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let run = match run_cargo(&self.wt.path, &args, &self.target, self.timeout) {
            Ok(r) => r,
            Err(e) => return ("unverified".into(), e),
        };
        if run.timed_out {
            return (
                "unverified".into(),
                format!("the time limit of {} s ran out", self.timeout.as_secs()),
            );
        }
        if run.output.contains("because --locked was passed") {
            return ("unverified".into(), "Cargo.lock is out of date".into());
        }
        let (mut p, mut f, mut i) = (0u64, 0u64, 0u64);
        for (a, b, c) in run.output.lines().filter_map(summary_counts) {
            p += a;
            f += b;
            i += c;
        }
        let detail = format!("{p} passed, {f} failed, {i} ignored");
        if run
            .output
            .lines()
            .any(|l| l.starts_with("error: could not compile"))
        {
            return (
                "fail".into(),
                format!("{detail}; the workspace does not compile"),
            );
        }
        if run.exit == Some(0) && f == 0 {
            ("pass".into(), detail)
        } else {
            ("fail".into(), detail)
        }
    }
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

fn print_list(root: &std::path::Path, key: &str, steps: &[Step]) -> Result<(), String> {
    println!("ticket: {key}");
    for step in steps {
        println!("{}", step.label);
        let reds: Vec<_> = step.reds().collect();
        let greens: Vec<_> = step.greens().collect();
        for (name, commits) in [("red", &reds), ("green", &greens)] {
            if commits.is_empty() {
                println!("  {name}: —");
            }
            for c in commits {
                println!("  {name}: {} {}", short(&c.sha), c.subject);
            }
        }
        let tests = match reds.first() {
            Some(red) => step_tests(root, &red.sha)?,
            None => Vec::new(),
        };
        println!("  tests: {}", tests.len());
        for t in tests {
            println!("    {t}");
        }
    }
    Ok(())
}
