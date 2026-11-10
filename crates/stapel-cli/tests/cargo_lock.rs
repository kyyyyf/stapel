//! STP-27: the machine-wide cargo lock and `stapel cargo`. A stand-in `cargo` script runs first on
//! `PATH` and `HOME` is a temporary folder, so no real build runs.

mod common;

use common::{stapel, stapel_repo};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const WAIT_LINE: &str = "waiting for the cargo lock: held by ";

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["config", "user.email", "test-user"]);
    git(dir, &["config", "user.name", "test-user"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", message]);
}

/// A stand-in `cargo` in `<root>/bin`, with its control files in `<root>/ctl`:
/// every run appends `run <args>` to `log`; while `hold` exists it writes `started` and waits for
/// `release`; every run appends the held mark (or `unset`) to `marks`; `--version` prints a version line; `print` prints its arguments and the held mark,
/// copies the lock file to `holder` and exits 3; `term` kills itself with SIGTERM.
struct Standin {
    bin: PathBuf,
    ctl: PathBuf,
}

impl Standin {
    fn new(root: &Path) -> Standin {
        let bin = root.join("bin");
        let ctl = root.join("ctl");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&ctl).unwrap();
        let script = format!(
            "#!/bin/sh\n\
             D='{ctl}'\n\
             echo \"run $*\" >> \"$D/log\"\n\
             echo \"${{STAPEL_CARGO_LOCK_HELD-unset}}\" >> \"$D/marks\"\n\
             if [ -e \"$D/hold\" ]; then\n\
             \x20 echo \"$*\" > \"$D/started\"\n\
             \x20 while [ ! -e \"$D/release\" ]; do sleep 0.05; done\n\
             fi\n\
             case \"$1\" in\n\
             --version) echo 'cargo 0.0.0 (stand-in)' ;;\n\
             print)\n\
             \x20 echo \"args: $*\"\n\
             \x20 echo \"mark=$STAPEL_CARGO_LOCK_HELD\"\n\
             \x20 cat \"$HOME/.cache/stapel/cargo.lock\" > \"$D/holder\"\n\
             \x20 exit 3 ;;\n\
             term) kill -TERM $$ ;;\n\
             esac\n\
             exit 0\n",
            ctl = ctl.display()
        );
        let path = bin.join("cargo");
        std::fs::write(&path, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        Standin { bin, ctl }
    }

    fn path_var(&self) -> String {
        format!("{}:{}", self.bin.display(), std::env::var("PATH").unwrap())
    }

    fn touch(&self, name: &str) {
        std::fs::write(self.ctl.join(name), "").unwrap();
    }

    fn remove(&self, name: &str) {
        let _ = std::fs::remove_file(self.ctl.join(name));
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.ctl.join("log")).unwrap_or_default()
    }

    fn runs(&self, args: &str) -> usize {
        let line = format!("run {args}");
        self.log().lines().filter(|l| *l == line).count()
    }

    fn command(&self, dir: &Path, home: &Path, args: &[&str]) -> Command {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("stapel"));
        cmd.args(args)
            .current_dir(dir)
            .env("HOME", home)
            .env("PATH", self.path_var())
            .env_remove("STAPEL_CARGO_LOCK_HELD")
            .env_remove("CLAUDE_PROJECT_DIR")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env(
                "GIT_CEILING_DIRECTORIES",
                std::env::temp_dir().canonicalize().unwrap(),
            )
            .stdin(Stdio::null());
        cmd
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(30);
    while !done() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_exit(child: &mut Child, what: &str) -> std::process::ExitStatus {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < end, "timed out waiting for {what} to end");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Starts `cmd` with stderr in `err`.
fn spawn_to(mut cmd: Command, err: &Path) -> Child {
    cmd.stdout(Stdio::null())
        .stderr(std::fs::File::create(err).unwrap())
        .spawn()
        .unwrap()
}

fn kill9(child: &mut Child) {
    let status = Command::new("kill")
        .args(["-KILL", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let _ = child.wait();
}

/// A stapel repository with a one-crate workspace and ticket ABC-1 with one committed step, so
/// `stapel check ABC-1` reaches its first cargo run.
fn check_repo() -> tempfile::TempDir {
    let repo = common::git_repo();
    let dir = repo.path();
    let files = [
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/tiny\"]\nresolver = \"2\"\n",
        ),
        (
            "crates/tiny/Cargo.toml",
            "[package]\nname = \"tiny\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "crates/tiny/src/lib.rs",
            "pub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n",
        ),
        (".gitignore", "/target\n"),
    ];
    for (rel, text) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    // `init` writes `[check]` in a cargo repository.
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    stapel(dir).args(["new", "First"]).assert().success();
    commit_all(dir, "initial");
    std::fs::create_dir_all(dir.join("crates/tiny/tests")).unwrap();
    std::fs::write(
        dir.join("crates/tiny/tests/basic.rs"),
        "#[test]\nfn starts() {\n    assert_eq!(tiny::add(1, 1), 2);\n}\n",
    )
    .unwrap();
    commit_all(dir, "ABC-1 step 1 RED: first test");
    commit_all(dir, "ABC-1 step 1 GREEN: first code");
    repo
}

// STP-27 AC-10: the wrapper takes the lock, writes the holder line, runs cargo with the held mark and
// returns its exit code; a signal gives 128 plus its number; a missing or unwritable HOME refuses.
#[test]
fn wrapper_runs_cargo() {
    let root = tempfile::tempdir().unwrap();
    let standin = Standin::new(root.path());
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    // On a ticket branch: the holder line names the ticket.
    let repo = stapel_repo();
    let dir = repo.path();
    commit_all(dir, "base");
    stapel(dir).args(["new", "First"]).assert().success();
    let wt = dir.join(".claude/worktrees/ABC-1");
    let out = standin
        .command(&wt, &home, &["cargo", "print", "a", "b"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("args: print a b"), "{stdout}");
    let holder = std::fs::read_to_string(standin.ctl.join("holder")).unwrap();
    let holder = holder.trim();
    assert!(holder.starts_with("ABC-1, pid "), "{holder}");
    assert!(holder.contains(", since 20"), "{holder}");
    assert!(holder.contains("stapel cargo print a b"), "{holder}");
    let pid = holder["ABC-1, pid ".len()..].split(',').next().unwrap();
    assert!(
        stdout.contains(&format!("mark={pid}")),
        "mark is the holder pid {pid}: {stdout}"
    );

    // On a base branch: no branch ticket, the holder line starts with `-`.
    let out = standin
        .command(dir, &home, &["cargo", "print"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let holder = std::fs::read_to_string(standin.ctl.join("holder")).unwrap();
    assert!(holder.starts_with("-, pid "), "{holder}");

    // A cargo killed by SIGTERM: 128 plus 15.
    let out = standin
        .command(dir, &home, &["cargo", "term"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(143), "{out:?}");

    // HOME empty or unset: the refusal, cargo not run.
    let before = standin.log();
    for home_value in [Some(""), None] {
        let mut cmd = standin.command(dir, &home, &["cargo", "print"]);
        match home_value {
            Some(v) => cmd.env("HOME", v),
            None => cmd.env_remove("HOME"),
        };
        let out = cmd.output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("no folder for the cargo lock: HOME is not set"),
            "{stderr}"
        );
    }
    assert_eq!(standin.log(), before, "cargo ran without a lock");

    // HOME a read-only folder: refused (skipped when the account can write there anyway).
    use std::os::unix::fs::PermissionsExt;
    let ro = root.path().join("readonly");
    std::fs::create_dir_all(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(ro.join("probe"), "").is_err() {
        let out = standin
            .command(dir, &ro, &["cargo", "print"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        assert!(!out.stderr.is_empty(), "{out:?}");
        assert_eq!(standin.log(), before, "cargo ran without a lock");
    }
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
}

// STP-27 AC-11: a waiter names the holder once and goes on after the release, for `stapel cargo` and
// `stapel check`; a lock held with an empty file has an unknown holder.
#[test]
fn second_waits_and_names_the_holder() {
    let root = tempfile::tempdir().unwrap();
    let standin = Standin::new(root.path());
    let home = root.path().join("home");
    let lock_dir = home.join(".cache/stapel");
    std::fs::create_dir_all(&lock_dir).unwrap();
    let repo = check_repo();
    let dir = repo.path();

    // Without a holder, `stapel check` reaches its stand-in cargo (the fixture works).
    let out = standin
        .command(dir, &home, &["check", "ABC-1"])
        .output()
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains(WAIT_LINE),
        "{out:?}"
    );
    assert!(
        standin.log().contains("run --version"),
        "log {:?}, {out:?}",
        standin.log()
    );
    std::fs::remove_file(standin.ctl.join("log")).unwrap();

    // A real holder: `stapel cargo` with a cargo that waits.
    standin.touch("hold");
    let mut holder = standin
        .command(dir, &home, &["cargo", "hold"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("the holder's cargo", || standin.ctl.join("started").exists());
    standin.remove("hold");

    for (name, args, ran) in [
        ("cargo", vec!["cargo", "second"], "second"),
        ("check", vec!["check", "ABC-1"], "--version"),
    ] {
        let before = standin.runs(ran);
        let err = root.path().join(format!("{name}.err"));
        let mut waiter = spawn_to(standin.command(dir, &home, &args), &err);
        wait_until("the wait line", || {
            std::fs::read_to_string(&err)
                .unwrap_or_default()
                .contains(WAIT_LINE)
        });
        std::thread::sleep(Duration::from_millis(600));
        assert!(waiter.try_wait().unwrap().is_none(), "{name} did not wait");
        assert_eq!(standin.runs(ran), before, "{name} ran cargo while waiting");
        let text = std::fs::read_to_string(&err).unwrap();
        assert_eq!(text.matches(WAIT_LINE).count(), 1, "{text}");
        let line = text.lines().find(|l| l.contains(WAIT_LINE)).unwrap();
        assert!(
            line.contains(&format!("pid {}", holder.id())) && line.contains("stapel cargo hold"),
            "{line}"
        );
        if name == "cargo" {
            // Release the holder; the waiter goes on and runs cargo once.
            standin.touch("release");
            let status = wait_exit(&mut holder, "the holder");
            assert!(status.success(), "{status:?}");
            wait_exit(&mut waiter, "the waiter");
            assert_eq!(standin.runs("second"), 1, "{}", standin.log());
            let text = std::fs::read_to_string(&err).unwrap();
            assert_eq!(text.matches(WAIT_LINE).count(), 1, "{text}");
            // The next holder, for the check waiter.
            standin.remove("release");
            standin.remove("started");
            standin.touch("hold");
            holder = standin
                .command(dir, &home, &["cargo", "hold"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            wait_until("the second holder's cargo", || {
                standin.ctl.join("started").exists()
            });
            standin.remove("hold");
            continue;
        }
        standin.touch("release");
        let status = wait_exit(&mut holder, "the holder");
        assert!(status.success(), "{status:?}");
        wait_exit(&mut waiter, "the check waiter");
        assert!(standin.runs("--version") > before, "{}", standin.log());
    }

    // A lock held with an empty lock file: an unknown holder.
    std::fs::remove_file(standin.ctl.join("log")).unwrap();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(lock_dir.join("cargo.lock"))
        .unwrap();
    file.lock().unwrap();
    let err = root.path().join("unknown.err");
    let mut waiter = spawn_to(standin.command(dir, &home, &["cargo", "third"]), &err);
    wait_until("the wait line", || {
        std::fs::read_to_string(&err)
            .unwrap_or_default()
            .contains(WAIT_LINE)
    });
    let text = std::fs::read_to_string(&err).unwrap();
    assert!(
        text.contains("waiting for the cargo lock: held by an unknown holder"),
        "{text}"
    );
    assert!(
        text.contains(
            "held by an unknown holder (a process started by an earlier cargo run may hold it)"
        ),
        "{text}"
    );
    assert_eq!(standin.runs("third"), 0);
    drop(file);
    wait_exit(&mut waiter, "the waiter");
    assert_eq!(standin.runs("third"), 1, "{}", standin.log());
}

// STP-27 AC-11: a killed `stapel cargo` or `stapel check` holder keeps the lock until its cargo child ends.
#[test]
fn killed_holder_frees_the_lock() {
    let root = tempfile::tempdir().unwrap();
    let standin = Standin::new(root.path());
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let repo = check_repo();
    let dir = repo.path();

    for (name, args) in [
        ("cargo", vec!["cargo", "hold"]),
        ("check", vec!["check", "ABC-1"]),
    ] {
        standin.remove("release");
        standin.remove("started");
        standin.touch("hold");
        let mut holder = standin
            .command(dir, &home, &args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        wait_until("the holder's cargo", || standin.ctl.join("started").exists());
        standin.remove("hold");

        let tag = format!("after-{name}");
        let err = root.path().join(format!("{name}.err"));
        let mut waiter = spawn_to(standin.command(dir, &home, &["cargo", &tag]), &err);
        wait_until("the wait line", || {
            std::fs::read_to_string(&err)
                .unwrap_or_default()
                .contains(WAIT_LINE)
        });

        kill9(&mut holder);
        std::thread::sleep(Duration::from_millis(700));
        assert!(
            waiter.try_wait().unwrap().is_none(),
            "{name}: the waiter went on while the holder's cargo child still ran"
        );
        assert_eq!(standin.runs(&tag), 0, "{name}: {}", standin.log());

        standin.touch("release");
        let status = wait_exit(&mut waiter, "the waiter");
        assert!(status.success(), "{name}: {status:?}");
        assert_eq!(standin.runs(&tag), 1, "{name}: {}", standin.log());
    }
}

// STP-27 AC-12: `stapel check` takes the lock before its first cargo run, the holder line names the
// ticket and `stapel check`, and every cargo run sees the held mark with the check's pid.
#[test]
fn check_takes_the_lock() {
    let root = tempfile::tempdir().unwrap();
    let standin = Standin::new(root.path());
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let repo = check_repo();
    let dir = repo.path();
    git(dir, &["switch", "-q", "-c", "ABC-1"]);

    standin.touch("hold");
    let mut check = standin
        .command(dir, &home, &["check", "ABC-1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("the check's cargo", || standin.ctl.join("started").exists());
    let holder = std::fs::read_to_string(home.join(".cache/stapel/cargo.lock")).unwrap();
    assert!(
        holder.starts_with(&format!("ABC-1, pid {}, since 20", check.id())),
        "{holder}"
    );
    assert!(holder.trim_end().ends_with(", stapel check"), "{holder}");
    standin.remove("hold");
    standin.touch("release");
    wait_exit(&mut check, "the check");

    let marks = std::fs::read_to_string(standin.ctl.join("marks")).unwrap();
    assert!(!marks.is_empty(), "no cargo run");
    for mark in marks.lines() {
        assert_eq!(mark, check.id().to_string(), "marks: {marks}");
    }
}

// STP-27 AC-12: with the held mark set, `stapel cargo` and `stapel check` take no lock, and the mark
// goes on to cargo.
#[test]
fn held_mark_takes_no_lock() {
    let root = tempfile::tempdir().unwrap();
    let standin = Standin::new(root.path());
    let home = root.path().join("home");
    let lock_dir = home.join(".cache/stapel");
    std::fs::create_dir_all(&lock_dir).unwrap();
    let repo = check_repo();
    let dir = repo.path();

    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(lock_dir.join("cargo.lock"))
        .unwrap();
    file.lock().unwrap();

    for (name, args) in [
        ("cargo", vec!["cargo", "marked"]),
        ("check", vec!["check", "ABC-1"]),
    ] {
        let _ = std::fs::remove_file(standin.ctl.join("marks"));
        let err = root.path().join(format!("{name}.err"));
        let mut cmd = standin.command(dir, &home, &args);
        cmd.env("STAPEL_CARGO_LOCK_HELD", "4242");
        let mut child = spawn_to(cmd, &err);
        wait_exit(&mut child, name);
        let text = std::fs::read_to_string(&err).unwrap();
        assert!(!text.contains(WAIT_LINE), "{name}: {text}");
        let marks = std::fs::read_to_string(standin.ctl.join("marks")).unwrap();
        assert!(!marks.is_empty(), "{name}: no cargo run");
        for mark in marks.lines() {
            assert_eq!(mark, "4242", "{name}: marks {marks}");
        }
    }
    let line = std::fs::read_to_string(lock_dir.join("cargo.lock")).unwrap();
    assert!(line.is_empty(), "a holder line was written: {line}");
}

// STP-27 AC-11 (D7-4): an empty held mark takes the lock; a killed `stapel cargo` holder with a
// `stapel check` waiter: the waiter names the holder and goes on after the cargo child ends.
#[test]
fn lock_edges() {
    let root = tempfile::tempdir().unwrap();
    let standin = Standin::new(root.path());
    let home = root.path().join("home");
    let lock_dir = home.join(".cache/stapel");
    std::fs::create_dir_all(&lock_dir).unwrap();
    let repo = check_repo();
    let dir = repo.path();

    // An empty held mark is no mark: the lock is taken and the holder line written.
    let mut cmd = standin.command(dir, &home, &["cargo", "print"]);
    cmd.env("STAPEL_CARGO_LOCK_HELD", "");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let holder = std::fs::read_to_string(standin.ctl.join("holder")).unwrap();
    assert!(holder.contains(", pid "), "no holder line: {holder:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("mark=\n"), "empty mark passed on: {stdout}");
    std::fs::remove_file(standin.ctl.join("log")).unwrap();

    // A killed `stapel cargo` holder, a `stapel check` waiter.
    standin.touch("hold");
    let mut holder = standin
        .command(dir, &home, &["cargo", "hold"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("the holder's cargo", || standin.ctl.join("started").exists());
    standin.remove("hold");
    let err = root.path().join("check.err");
    let mut waiter = spawn_to(standin.command(dir, &home, &["check", "ABC-1"]), &err);
    wait_until("the wait line", || {
        std::fs::read_to_string(&err)
            .unwrap_or_default()
            .contains(WAIT_LINE)
    });
    let holder_pid = holder.id();
    kill9(&mut holder);
    std::thread::sleep(Duration::from_millis(700));
    assert!(
        waiter.try_wait().unwrap().is_none(),
        "the check went on while the holder's cargo child still ran"
    );
    assert_eq!(standin.runs("--version"), 0, "{}", standin.log());
    standin.touch("release");
    wait_exit(&mut waiter, "the check waiter");
    assert!(standin.runs("--version") > 0, "{}", standin.log());
    let text = std::fs::read_to_string(&err).unwrap();
    assert_eq!(text.matches(WAIT_LINE).count(), 1, "{text}");
    let line = text.lines().find(|l| l.contains(WAIT_LINE)).unwrap();
    assert!(
        line.contains(&format!("pid {holder_pid}")) && line.contains("stapel cargo hold"),
        "{line}"
    );
}
