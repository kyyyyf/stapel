//! The `cargo` runner of `stapel check` (STP-4 AC-3): one run with a time limit, and a strict
//! reading of libtest's output.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

/// Output kept per stream; the rest is read and discarded.
pub const MAX_OUTPUT: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestState {
    Passed,
    /// A result line `FAILED`.
    Failed,
    /// The test's target did not compile.
    Compile,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    Counted(BTreeMap<String, TestState>),
    /// The run does not count; the reason.
    Unverified(String),
}

/// `running 1 test` or `running <n> tests`.
fn running_count(line: &str) -> Option<usize> {
    let rest = line.strip_prefix("running ")?;
    let (n, word) = rest.split_once(' ')?;
    let n: usize = n.parse().ok()?;
    let ok = (n == 1 && word == "test") || (n != 1 && word == "tests");
    ok.then_some(n)
}

/// A result line: the test name and its state.
fn result_line(line: &str) -> Option<(&str, TestState)> {
    let rest = line.strip_prefix("test ")?;
    let (name, state) = rest.split_once(" ... ")?;
    let name = name.strip_suffix(" - should panic").unwrap_or(name);
    let state = match state {
        "ok" => TestState::Passed,
        "FAILED" => TestState::Failed,
        s if s == "ignored" || s.starts_with("ignored, ") => TestState::Ignored,
        _ => return None,
    };
    Some((name, state))
}

/// `test result: ok. 2 passed; 0 failed; 1 ignored; …` as (passed, failed, ignored).
pub fn summary_counts(line: &str) -> Option<(u64, u64, u64)> {
    let rest = line.strip_prefix("test result: ")?;
    let (_, counts) = rest.split_once(". ")?;
    let get = |word: &str| -> Option<u64> {
        counts
            .split("; ")
            .find_map(|part| part.strip_suffix(&format!(" {word}")))?
            .trim()
            .parse()
            .ok()
    };
    Some((get("passed")?, get("failed")?, get("ignored")?))
}

fn first_error(output: &str) -> String {
    output
        .lines()
        .find(|l| l.starts_with("error"))
        .unwrap_or("no error line")
        .chars()
        .take(200)
        .collect()
}

/// Reads one `cargo test -p <package> --test <target> -- --exact <names>` run. At RED, a compile
/// error of the step's test target fails every name; any other build error does not count. At
/// GREEN and HEAD any compile error fails every name.
pub fn parse_run(
    output: &str,
    names: &[String],
    exit: Option<i32>,
    target: &str,
    at_red: bool,
) -> Parsed {
    if output.contains("because --locked was passed") {
        return Parsed::Unverified("Cargo.lock is out of date".into());
    }
    let running: Vec<usize> = output.lines().filter_map(running_count).collect();
    if running.is_empty() {
        let compile: Vec<&str> = output
            .lines()
            .filter(|l| l.starts_with("error: could not compile"))
            .collect();
        if compile.is_empty() {
            return Parsed::Unverified(format!("cargo ran no test: {}", first_error(output)));
        }
        let ours = format!("(test \"{target}\")");
        if at_red && !compile.iter().all(|l| l.contains(&ours)) {
            return Parsed::Unverified(format!(
                "another target does not compile: {}",
                compile
                    .iter()
                    .find(|l| !l.contains(&ours))
                    .unwrap_or(&compile[0])
            ));
        }
        return Parsed::Counted(
            names
                .iter()
                .map(|n| (n.clone(), TestState::Compile))
                .collect(),
        );
    }
    let summaries: Vec<(u64, u64, u64)> = output.lines().filter_map(summary_counts).collect();
    if running.len() != 1 || summaries.len() != 1 {
        return Parsed::Unverified(format!(
            "the output has {} running lines and {} result summaries",
            running.len(),
            summaries.len()
        ));
    }
    if running[0] != names.len() {
        return Parsed::Unverified(format!(
            "cargo ran {} tests for {} names",
            running[0],
            names.len()
        ));
    }
    let mut states: BTreeMap<String, Vec<TestState>> = BTreeMap::new();
    for (name, state) in output.lines().filter_map(result_line) {
        if names.iter().any(|n| n == name) {
            states.entry(name.to_string()).or_default().push(state);
        }
    }
    let mut out = BTreeMap::new();
    for name in names {
        match states.get(name).map(Vec::as_slice) {
            Some([one]) => {
                out.insert(name.clone(), *one);
            }
            other => {
                return Parsed::Unverified(format!(
                    "{name} has {} result lines",
                    other.map_or(0, <[TestState]>::len)
                ));
            }
        }
    }
    let count = |s: TestState| out.values().filter(|v| **v == s).count() as u64;
    let (p, f, i) = summaries[0];
    if (p, f, i)
        != (
            count(TestState::Passed),
            count(TestState::Failed),
            count(TestState::Ignored),
        )
    {
        return Parsed::Unverified("the result summary does not match the result lines".into());
    }
    let agrees = match exit {
        Some(0) => f == 0,
        Some(101) => f > 0,
        _ => false,
    };
    if !agrees {
        return Parsed::Unverified(format!(
            "the exit code {} does not match {f} failed tests",
            exit.map_or("none".to_string(), |c| c.to_string())
        ));
    }
    Parsed::Counted(out)
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub exit: Option<i32>,
    pub timed_out: bool,
    /// stdout, then stderr.
    pub output: String,
}

/// Reads a stream into a shared buffer, keeping at most `MAX_OUTPUT` bytes; says when done.
fn read_capped(mut r: impl Read + Send + 'static) -> (Arc<Mutex<Vec<u8>>>, mpsc::Receiver<()>) {
    let kept = Arc::new(Mutex::new(Vec::new()));
    let (done, finished) = mpsc::channel();
    let buf_kept = Arc::clone(&kept);
    std::thread::spawn(move || {
        let mut buf = [0u8; 64 * 1024];
        loop {
            match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut k = buf_kept.lock().unwrap_or_else(|e| e.into_inner());
                    let room = MAX_OUTPUT.saturating_sub(k.len());
                    k.extend_from_slice(&buf[..n.min(room)]);
                }
            }
        }
        let _ = done.send(());
    });
    (kept, finished)
}

/// What a reader has read: it gets until `deadline` to reach the end of its stream, since a
/// process outside the group may hold the stream open for ever.
fn collect(reader: (Arc<Mutex<Vec<u8>>>, mpsc::Receiver<()>), deadline: Instant) -> Vec<u8> {
    let (kept, finished) = reader;
    let _ = finished.recv_timeout(deadline.saturating_duration_since(Instant::now()));
    kept.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[cfg(unix)]
fn own_group(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

#[cfg(not(unix))]
fn own_group(_cmd: &mut Command) {}

#[cfg(unix)]
fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

/// Variables of the caller that reach a cargo run (STP-6 AC-8); the check adds its own.
pub const ENV_ALLOW: [&str; 9] = [
    "HOME",
    "USER",
    "PATH",
    "LANG",
    "TMPDIR",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "CARGO_BUILD_JOBS",
];

/// The cargo program (STP-6 AC-9): `configured` if set, else the first `cargo` in an absolute
/// `PATH` entry; relative entries are skipped.
pub fn resolve_cargo(configured: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = configured {
        return Ok(PathBuf::from(path));
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .filter(|p| p.is_absolute())
        .map(|p| p.join("cargo"))
        .find(|p| p.is_file())
        .ok_or_else(|| "no cargo found in an absolute PATH entry".to_string())
}

/// The first line of `cargo --version` run in `dir`, cut to 200 bytes; `?` when empty. A failing
/// or slow run is an error (STP-6 AC-9, AC-11).
pub fn cargo_version(
    cargo: &Path,
    dir: &Path,
    target_dir: &Path,
    timeout: Duration,
) -> Result<String, String> {
    let run = run_cargo(cargo, dir, &["--version".to_string()], target_dir, timeout)
        .map_err(|e| format!("cargo --version: {e}"))?;
    if run.timed_out {
        return Err(format!(
            "cargo --version: the time limit of {} s ran out",
            timeout.as_secs()
        ));
    }
    if run.exit != Some(0) {
        return Err(format!("cargo --version failed (exit {:?})", run.exit));
    }
    let line = run.output.lines().next().unwrap_or("");
    let mut end = line.len().min(200);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    let line = line[..end].trim_end();
    Ok(if line.is_empty() {
        "?".into()
    } else {
        line.into()
    })
}

/// Runs `cargo <args>` in `dir` in its own process group, killed as a group after `timeout`.
/// The run gets an empty environment plus the allow-listed variables of the caller.
pub fn run_cargo(
    cargo: &Path,
    dir: &Path,
    args: &[String],
    target_dir: &Path,
    timeout: Duration,
) -> Result<RunResult, String> {
    let mut cmd = Command::new(cargo);
    cmd.env_clear();
    for name in ENV_ALLOW {
        if let Some(value) = std::env::var_os(name) {
            cmd.env(name, value);
        }
    }
    cmd.args(args)
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_TERM_COLOR", "never")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    own_group(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cargo did not start: {e}"))?;
    let out = child.stdout.take().expect("piped");
    let err = child.stderr.take().expect("piped");
    let out = read_capped(out);
    let err = read_capped(err);
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                timed_out = true;
                kill_group(child.id());
                let _ = child.kill();
                break child.wait().ok();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("waiting for cargo: {e}")),
        }
    };
    // Children of the test that outlive it would keep the pipes open; the group goes too.
    kill_group(child.id());
    let drain = Instant::now() + Duration::from_secs(2);
    let mut output = String::from_utf8_lossy(&collect(out, drain)).into_owned();
    output.push('\n');
    output.push_str(&String::from_utf8_lossy(&collect(err, drain)));
    Ok(RunResult {
        exit: status.and_then(|s| s.code()),
        timed_out,
        output,
    })
}
