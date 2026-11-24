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
    // One slot keeps the STP-27 tests to one run at a time.
    let config = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[cargo]\nslots = 1\n")).unwrap();
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

// STP-28 AC-40: a junit `stapel check` takes the machine-wide lock as a cargo check does: it waits
// with the wait line while the lock is held and runs after the release.
#[test]
fn junit_check_takes_the_lock() {
    let root = tempfile::tempdir().unwrap();
    let standin = Standin::new(root.path());
    let home = root.path().join("home");
    let lock_dir = home.join(".cache/stapel");
    std::fs::create_dir_all(&lock_dir).unwrap();
    let repo = common::junit_repo();
    let dir = repo.dir();
    // STP-42: two slots; the check waits only while both are held and names both holders.
    let config = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[cargo]\nslots = 2\n")).unwrap();
    std::fs::write(
        dir.join("__tests__/x.test.ts"),
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    )
    .unwrap();
    commit_all(dir, "ABC-1 step 1 RED: one");
    std::fs::write(dir.join("src/ok.ts"), "export const ok = 1;\n").unwrap();
    commit_all(dir, "ABC-1 step 1 GREEN: one");

    let hold = |name: &str, holder: &str| {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(lock_dir.join(name))
            .unwrap();
        file.lock().unwrap();
        std::fs::write(lock_dir.join(name), holder).unwrap();
        file
    };
    let file = hold("cargo.lock", "first junit holder");
    let second = hold("cargo-2.lock", "second junit holder");
    let err = root.path().join("junit.err");
    let out = root.path().join("junit.out");
    let mut cmd = standin.command(dir, &home, &["check", "ABC-1"]);
    cmd.stdout(std::fs::File::create(&out).unwrap())
        .stderr(std::fs::File::create(&err).unwrap());
    let mut check = cmd.spawn().unwrap();
    wait_until("the wait line", || {
        std::fs::read_to_string(&err)
            .unwrap_or_default()
            .contains(WAIT_LINE)
    });
    std::thread::sleep(Duration::from_millis(600));
    assert!(check.try_wait().unwrap().is_none(), "the check did not wait");
    assert!(
        !std::fs::read_to_string(&out)
            .unwrap_or_default()
            .contains("step 1:"),
        "the check ran while the lock was held"
    );
    let waited = std::fs::read_to_string(&err).unwrap_or_default();
    drop(file);
    drop(second);
    wait_exit(&mut check, "the check");
    assert!(
        waited.contains("first junit holder; second junit holder"),
        "the wait line names both slot holders: {waited}"
    );
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("step 1: pass"), "{text}");
}

// STP-28 AC-40: the holder line names the junit check; the held mark reaches the Per-file and the
// suite commands; with the mark set the check takes no lock; a refused check leaves the lock free.
#[test]
fn junit_lock_edges() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let lock_dir = home.join(".cache/stapel");
    std::fs::create_dir_all(&lock_dir).unwrap();
    let standin = Standin::new(root.path());
    let repo = common::junit_repo();
    let dir = repo.dir();

    // `command` goes through a wrapper that records the mark and the holder line, then runs the
    // junit stand-in.
    let toml = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&toml).unwrap();
    let line = text.lines().find(|l| l.starts_with("command = ")).unwrap();
    let inner = line
        .trim_start_matches("command = \"sh ")
        .trim_end_matches(" {file}\"")
        .to_string();
    let wrapper = root.path().join("wrap.sh");
    std::fs::write(
        &wrapper,
        format!(
            "echo \"${{STAPEL_CARGO_LOCK_HELD-unset}}\" >> '{ctl}/junit-marks'\n\
             cat \"$HOME/.cache/stapel/cargo.lock\" > '{ctl}/junit-holder'\n\
             exec sh '{inner}' \"$@\"\n",
            ctl = standin.ctl.display()
        ),
    )
    .unwrap();
    std::fs::write(
        &toml,
        text.replace(
            line,
            &format!("command = \"sh {} {{file}}\"", wrapper.display()),
        ),
    )
    .unwrap();
    commit_all(dir, "wrapper");
    std::fs::write(
        dir.join("__tests__/x.test.ts"),
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    )
    .unwrap();
    commit_all(dir, "ABC-1 step 1 RED: one");
    std::fs::write(dir.join("src/ok.ts"), "export const ok = 1;\n").unwrap();
    commit_all(dir, "ABC-1 step 1 GREEN: one");
    git(dir, &["switch", "-q", "-c", "ABC-1"]);

    // The lock is free: the holder line names the check, the mark is the check's pid.
    let mut check = standin
        .command(dir, &home, &["check", "ABC-1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = check.id();
    wait_exit(&mut check, "the check");
    let holder = std::fs::read_to_string(standin.ctl.join("junit-holder")).unwrap();
    assert!(
        holder.starts_with(&format!("ABC-1, pid {pid}, since 20")),
        "{holder}"
    );
    assert!(holder.trim_end().ends_with(", stapel check"), "{holder}");
    let marks = std::fs::read_to_string(standin.ctl.join("junit-marks")).unwrap();
    assert!(marks.lines().count() >= 2, "Per-file and suite runs: {marks}");
    for mark in marks.lines() {
        assert_eq!(mark, pid.to_string(), "marks: {marks}");
    }

    // The mark is set: the lock stays held elsewhere, the check does not wait and writes no line.
    let _ = std::fs::remove_file(standin.ctl.join("junit-marks"));
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(lock_dir.join("cargo.lock"))
        .unwrap();
    file.lock().unwrap();
    let err = root.path().join("marked.err");
    let mut cmd = standin.command(dir, &home, &["check", "ABC-1"]);
    cmd.env("STAPEL_CARGO_LOCK_HELD", "4242");
    let mut child = spawn_to(cmd, &err);
    wait_exit(&mut child, "the marked check");
    let text = std::fs::read_to_string(&err).unwrap();
    assert!(!text.contains(WAIT_LINE), "{text}");
    let marks = std::fs::read_to_string(standin.ctl.join("junit-marks")).unwrap();
    assert!(!marks.is_empty(), "no junit run");
    for mark in marks.lines() {
        assert_eq!(mark, "4242", "marks: {marks}");
    }
    let line = std::fs::read_to_string(lock_dir.join("cargo.lock")).unwrap();
    assert!(line.is_empty(), "a holder line was written: {line}");
    drop(file);

    // A refused check (an unknown ticket) leaves the lock free.
    let out = standin
        .command(dir, &home, &["check", "ABC-77"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(lock_dir.join("cargo.lock"))
        .unwrap();
    assert!(file.try_lock().is_ok(), "the lock is still held");
}

// STP-42 AC-1: `[cargo] slots` is a whole number from 1 to 16; absent is 3; an unknown key fails.
#[test]
fn slots_setting() {
    let repo = common::repo_with_ticket();
    let dir = repo.path();
    let path = dir.join(".stapel/stapel.toml");
    let base = std::fs::read_to_string(&path).unwrap();

    for table in [
        String::new(),
        "\n[cargo]\n".to_string(),
        "\n[cargo]\nslots = 1\n".to_string(),
        "\n[cargo]\nslots = 16\n".to_string(),
    ] {
        std::fs::write(&path, format!("{base}{table}")).unwrap();
        let out = stapel(dir).arg("status").output().unwrap();
        assert!(out.status.success(), "{table:?}: {out:?}");
    }

    for value in ["0", "17", "-1", "2.5", "\"3\""] {
        std::fs::write(&path, format!("{base}\n[cargo]\nslots = {value}\n")).unwrap();
        let out = stapel(dir).arg("status").output().unwrap();
        assert_eq!(out.status.code(), Some(1), "slots = {value}: {out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("cargo.slots must be a whole number from 1 to 16"),
            "slots = {value}: {stderr}"
        );
    }

    std::fs::write(&path, format!("{base}\n[cargo]\nx = 1\n")).unwrap();
    let out = stapel(dir).arg("status").output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("unknown field"),
        "{out:?}"
    );
}

// STP-42 AC-11: a `stapel.toml` that fails loading stops `stapel cargo` before cargo runs; outside any
// stapel repository the run goes on with N = 3.
#[test]
fn a_bad_config_refuses() {
    let root = tempfile::tempdir().unwrap();
    let standin = Standin::new(root.path());
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let repo = stapel_repo();
    let dir = repo.path();
    let path = dir.join(".stapel/stapel.toml");
    let base = std::fs::read_to_string(&path).unwrap();

    for (text, expect) in [
        (
            format!("{base}\n[cargo]\nslots = 0\n"),
            "cargo.slots must be a whole number from 1 to 16",
        ),
        ("this is = [not toml\n".to_string(), ""),
    ] {
        std::fs::write(&path, text).unwrap();
        let out = standin
            .command(dir, &home, &["cargo", "print"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.starts_with("stapel: "), "{stderr}");
        assert!(stderr.contains(expect), "{stderr}");
        assert_eq!(standin.log(), "", "cargo ran with a bad config");
    }

    let outside = root.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let out = standin
        .command(&outside, &home, &["cargo", "print"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    assert_eq!(standin.runs("print"), 1);
}

/// A stapel repository of its own (so its own worktree) with `[cargo] slots = n`.
fn slot_repo(n: u32) -> tempfile::TempDir {
    let repo = stapel_repo();
    let config = repo.path().join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[cargo]\nslots = {n}\n")).unwrap();
    repo
}

/// A stand-in `cargo` of its own under `<root>/<name>`, set to hold its next run.
fn holding_standin(root: &Path, name: &str) -> Standin {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let standin = Standin::new(&dir);
    standin.touch("hold");
    standin
}

fn slot_file(home: &Path, slot: u32) -> PathBuf {
    let name = if slot == 1 {
        "cargo.lock".to_string()
    } else {
        format!("cargo-{slot}.lock")
    };
    home.join(".cache/stapel").join(name)
}

fn slot_text(home: &Path, slot: u32) -> String {
    std::fs::read_to_string(slot_file(home, slot)).unwrap_or_default()
}

// STP-42 AC-2: with N = 2, a second run from another worktree takes slot 2 and runs at once while the
// first holds slot 1; each slot holds its holder line; `stapel check` takes the free slot too.
#[test]
fn runs_in_parallel_up_to_slots() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let repo_a = slot_repo(2);
    let repo_b = slot_repo(2);
    let standin_a = holding_standin(root.path(), "a");
    let standin_b = holding_standin(root.path(), "b");

    let mut first = standin_a
        .command(repo_a.path(), &home, &["cargo", "hold-a"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("the first cargo", || standin_a.ctl.join("started").exists());
    let err_b = root.path().join("b.err");
    let mut second = spawn_to(
        standin_b.command(repo_b.path(), &home, &["cargo", "hold-b"]),
        &err_b,
    );
    wait_until("the second cargo, in parallel", || {
        standin_b.ctl.join("started").exists()
    });
    assert!(
        slot_text(&home, 1).contains(&format!("pid {}", first.id())),
        "slot 1: {:?}",
        slot_text(&home, 1)
    );
    assert!(
        slot_text(&home, 1).contains("stapel cargo hold-a"),
        "slot 1: {:?}",
        slot_text(&home, 1)
    );
    assert!(
        slot_text(&home, 2).contains(&format!("pid {}", second.id())),
        "slot 2: {:?}",
        slot_text(&home, 2)
    );
    assert!(
        slot_text(&home, 2).contains("stapel cargo hold-b"),
        "slot 2: {:?}",
        slot_text(&home, 2)
    );
    let text = std::fs::read_to_string(&err_b).unwrap_or_default();
    assert!(!text.contains(WAIT_LINE), "{text}");

    // The second run ends; `stapel check` runs on slot 2 while the first still holds slot 1.
    standin_b.touch("release");
    let status = wait_exit(&mut second, "the second run");
    assert!(status.success(), "{status:?}");
    let check = check_repo();
    git(check.path(), &["switch", "-q", "-c", "ABC-1"]);
    let config = check.path().join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, text.replace("slots = 1", "slots = 2")).unwrap();
    commit_all(check.path(), "two slots");
    let standin_c = holding_standin(root.path(), "c");
    let err_c = root.path().join("c.err");
    let mut checker = spawn_to(
        standin_c.command(check.path(), &home, &["check", "ABC-1"]),
        &err_c,
    );
    wait_until("the check's cargo, in parallel", || {
        standin_c.ctl.join("started").exists()
    });
    assert!(
        slot_text(&home, 2).contains("stapel check"),
        "slot 2: {:?}",
        slot_text(&home, 2)
    );
    assert!(
        slot_text(&home, 1).contains(&format!("pid {}", first.id())),
        "slot 1: {:?}",
        slot_text(&home, 1)
    );
    let text = std::fs::read_to_string(&err_c).unwrap_or_default();
    assert!(!text.contains(WAIT_LINE), "{text}");
    standin_c.touch("release");
    standin_a.touch("release");
    wait_exit(&mut checker, "the check");
    wait_exit(&mut first, "the first run");
}

// STP-42 AC-3: with N = 2 and both slots held by foreign processes, a run prints one waiting line with
// both holders, waits, and starts on the slot that is freed; a third run waits while two runs hold.
#[test]
fn all_slots_held_waits() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let lock_dir = home.join(".cache/stapel");
    std::fs::create_dir_all(&lock_dir).unwrap();
    let repo_a = slot_repo(2);
    let repo_c = slot_repo(2);
    let standin_a = holding_standin(root.path(), "a");
    let standin_c = Standin::new(&root.path().join("c"));

    // Slot 1 held with a holder line, slot 2 held with an empty file.
    let open = |slot: u32| {
        std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(slot_file(&home, slot))
            .unwrap()
    };
    let foreign_one = open(1);
    foreign_one.lock().unwrap();
    std::fs::write(
        slot_file(&home, 1),
        "XYZ-9, pid 4242, since 2026-01-01T00:00:00Z, cargo test",
    )
    .unwrap();
    let foreign_two = open(2);
    foreign_two.lock().unwrap();

    let err = root.path().join("a.err");
    let mut waiter = spawn_to(
        standin_a.command(repo_a.path(), &home, &["cargo", "second"]),
        &err,
    );
    wait_until("the wait line", || {
        std::fs::read_to_string(&err)
            .unwrap_or_default()
            .contains(WAIT_LINE)
    });
    std::thread::sleep(Duration::from_millis(600));
    assert!(waiter.try_wait().unwrap().is_none(), "the run did not wait");
    assert!(
        !standin_a.ctl.join("started").exists(),
        "cargo ran while both slots were held"
    );
    let text = std::fs::read_to_string(&err).unwrap();
    assert_eq!(text.matches(WAIT_LINE).count(), 1, "{text}");
    let line = text.lines().find(|l| l.contains(WAIT_LINE)).unwrap();
    assert!(
        line.contains("XYZ-9, pid 4242, since 2026-01-01T00:00:00Z, cargo test; an unknown holder"),
        "{line}"
    );

    // Slot 2 freed: the run starts on slot 2.
    drop(foreign_two);
    wait_until("the run on slot 2", || {
        standin_a.ctl.join("started").exists()
    });
    assert!(
        slot_text(&home, 2).contains(&format!("pid {}", waiter.id())),
        "slot 2: {:?}",
        slot_text(&home, 2)
    );
    let text = std::fs::read_to_string(&err).unwrap();
    assert_eq!(text.matches(WAIT_LINE).count(), 1, "{text}");

    // Two runs hold slots 1 and 2: a third run, from a third worktree, waits.
    let err_c = root.path().join("c.err");
    let mut third = spawn_to(
        standin_c.command(repo_c.path(), &home, &["cargo", "third"]),
        &err_c,
    );
    wait_until("the third run's wait line", || {
        std::fs::read_to_string(&err_c)
            .unwrap_or_default()
            .contains(WAIT_LINE)
    });
    std::thread::sleep(Duration::from_millis(600));
    assert!(
        third.try_wait().unwrap().is_none(),
        "the third run did not wait"
    );
    assert_eq!(standin_c.runs("third"), 0, "{}", standin_c.log());
    let text = std::fs::read_to_string(&err_c).unwrap();
    assert_eq!(text.matches(WAIT_LINE).count(), 1, "{text}");
    let line = text.lines().find(|l| l.contains(WAIT_LINE)).unwrap();
    assert!(
        line.contains("XYZ-9, pid 4242") && line.contains(&format!("pid {}", waiter.id())),
        "{line}"
    );

    // The foreign holder of slot 1 ends: the third run goes on.
    drop(foreign_one);
    wait_until("the third run", || standin_c.runs("third") == 1);
    standin_a.touch("release");
    wait_exit(&mut waiter, "the second run");
    wait_exit(&mut third, "the third run");
}

// STP-42 AC-4: a run takes the worktree lock before a slot and writes its holder line into it; a second
// run from a subfolder of that worktree waits with the holder's line and holds no slot while free slots
// exist; a run from another worktree of the same repository runs at once.
#[test]
fn one_run_per_worktree() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let lock_dir = home.join(".cache/stapel");
    std::fs::create_dir_all(&home).unwrap();
    let repo = slot_repo(3);
    let dir = repo.path();
    commit_all(dir, "base");
    stapel(dir).args(["new", "First"]).assert().success();
    let wt = dir.join(".claude/worktrees/ABC-1");
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let standin_a = holding_standin(root.path(), "a");
    let standin_b = Standin::new(&root.path().join("b"));
    let standin_c = holding_standin(root.path(), "c");

    let mut first = standin_a
        .command(dir, &home, &["cargo", "hold-a"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("the first cargo", || standin_a.ctl.join("started").exists());
    let worktree_locks = || -> Vec<PathBuf> {
        std::fs::read_dir(&lock_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("worktree-")
            })
            .collect()
    };
    let locks = worktree_locks();
    assert_eq!(locks.len(), 1, "{locks:?}");
    let line = std::fs::read_to_string(&locks[0]).unwrap();
    assert!(
        line.contains(&format!("pid {}", first.id())) && line.contains("stapel cargo hold-a"),
        "worktree lock: {line:?}"
    );

    // A second run from a subfolder of the same worktree waits, holding no slot.
    let err = root.path().join("b.err");
    let mut second = spawn_to(standin_b.command(&sub, &home, &["cargo", "second"]), &err);
    wait_until("the wait line", || {
        std::fs::read_to_string(&err)
            .unwrap_or_default()
            .contains(WAIT_LINE)
    });
    std::thread::sleep(Duration::from_millis(600));
    assert!(second.try_wait().unwrap().is_none(), "the run did not wait");
    assert_eq!(standin_b.runs("second"), 0, "{}", standin_b.log());
    let text = std::fs::read_to_string(&err).unwrap();
    assert_eq!(text.matches(WAIT_LINE).count(), 1, "{text}");
    let wait = text.lines().find(|l| l.contains(WAIT_LINE)).unwrap();
    assert!(
        wait.contains(&format!("pid {}", first.id())) && wait.contains("stapel cargo hold-a"),
        "{wait}"
    );
    assert!(
        slot_text(&home, 1).contains(&format!("pid {}", first.id())),
        "slot 1: {:?}",
        slot_text(&home, 1)
    );
    for slot in [2, 3] {
        assert_eq!(slot_text(&home, slot), "", "slot {slot} has a holder line");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(slot_file(&home, slot))
            .unwrap();
        assert!(file.try_lock().is_ok(), "the waiter holds slot {slot}");
    }

    // Another worktree of the same repository runs at once.
    let mut other = standin_c
        .command(&wt, &home, &["cargo", "hold-c"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("the run in the other worktree", || {
        standin_c.ctl.join("started").exists()
    });
    assert_eq!(worktree_locks().len(), 2, "{:?}", worktree_locks());
    standin_c.touch("release");
    wait_exit(&mut other, "the other worktree's run");

    // The first run ends: the second goes on.
    standin_a.touch("release");
    wait_exit(&mut first, "the first run");
    wait_exit(&mut second, "the second run");
    assert_eq!(standin_b.runs("second"), 1, "{}", standin_b.log());
}

// STP-42 AC-1 (DR1-1): with no `[cargo]` table, and outside any stapel repository, N is 3: three runs
// from three folders hold at once and a fourth waits until one ends.
#[test]
fn default_slots_are_three() {
    /// Ends every run it holds on drop, also when an assertion failed: it releases every stand-in
    /// `cargo` and waits for each `stapel cargo` to end, so no stand-in is left holding a slot.
    struct Reaper {
        releases: Vec<PathBuf>,
        children: Vec<Child>,
    }
    impl Drop for Reaper {
        fn drop(&mut self) {
            for path in &self.releases {
                let _ = std::fs::write(path, "");
            }
            let end = Instant::now() + Duration::from_secs(20);
            for child in &mut self.children {
                while child.try_wait().ok().flatten().is_none() && Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(20));
                }
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    let root = tempfile::tempdir().unwrap();
    let outside: Vec<PathBuf> = (0..4)
        .map(|i| {
            let dir = root.path().join(format!("outside-{i}"));
            std::fs::create_dir_all(&dir).unwrap();
            dir
        })
        .collect();
    let repos: Vec<tempfile::TempDir> = (0..4).map(|_| stapel_repo()).collect();
    let inside: Vec<PathBuf> = repos.iter().map(|r| r.path().to_path_buf()).collect();
    let text = std::fs::read_to_string(inside[0].join(".stapel/stapel.toml")).unwrap();
    assert!(!text.contains("[cargo]"), "{text}");

    for (setting, dirs) in [("repository", inside), ("no repository", outside)] {
        let home = root.path().join(format!("home-{setting}"));
        std::fs::create_dir_all(&home).unwrap();
        let mut reaper = Reaper {
            releases: Vec::new(),
            children: Vec::new(),
        };
        let mut standins = Vec::new();
        for (i, dir) in dirs.iter().take(3).enumerate() {
            let standin = holding_standin(root.path(), &format!("{}-{i}", setting.replace(' ', "-")));
            reaper.releases.push(standin.ctl.join("release"));
            let err = root.path().join(format!("{setting}-{i}.err"));
            let child = spawn_to(standin.command(dir, &home, &["cargo", "hold"]), &err);
            reaper.children.push(child);
            wait_until(&format!("run {i} to hold ({setting})"), || {
                standin.ctl.join("started").exists()
            });
            let text = std::fs::read_to_string(&err).unwrap_or_default();
            assert!(!text.contains(WAIT_LINE), "{setting} run {i}: {text}");
            standins.push(standin);
        }
        for slot in 1..=3 {
            assert!(
                slot_text(&home, slot).contains("stapel cargo hold"),
                "{setting}: slot {slot}: {:?}",
                slot_text(&home, slot)
            );
        }

        // A fourth run waits.
        let fourth = holding_standin(root.path(), &format!("{}-4", setting.replace(' ', "-")));
        reaper.releases.push(fourth.ctl.join("release"));
        let err = root.path().join(format!("{setting}-4.err"));
        let child = spawn_to(fourth.command(&dirs[3], &home, &["cargo", "hold"]), &err);
        reaper.children.push(child);
        wait_until(&format!("the fourth wait line ({setting})"), || {
            std::fs::read_to_string(&err)
                .unwrap_or_default()
                .contains(WAIT_LINE)
        });
        std::thread::sleep(Duration::from_millis(600));
        assert!(
            !fourth.ctl.join("started").exists(),
            "{setting}: the fourth run started with three holders"
        );
        assert!(
            reaper.children[3].try_wait().unwrap().is_none(),
            "{setting}: the fourth run ended"
        );

        // One run ends: the fourth goes on.
        standins[0].touch("release");
        wait_until(&format!("the fourth run ({setting})"), || {
            fourth.ctl.join("started").exists()
        });
        drop(reaper);
    }
}

// STP-42 AC-3 (DR1-1): a run that waits for its worktree lock and then for the slots prints two waiting
// lines, the first with the worktree holder and the second with the slot holder.
#[test]
fn two_waiting_lines() {
    /// Kills the waiting run on drop, also when an assertion failed.
    struct Reaper(Child);
    impl Drop for Reaper {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let lock_dir = home.join(".cache/stapel");
    std::fs::create_dir_all(&home).unwrap();
    let repo = slot_repo(1);
    let dir = repo.path();
    let standin = Standin::new(root.path());

    // A first run makes the worktree lock file; then both locks are held by foreign processes.
    let out = standin
        .command(dir, &home, &["cargo", "discover"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let worktree_lock: Vec<PathBuf> = std::fs::read_dir(&lock_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("worktree-")
        })
        .collect();
    assert_eq!(worktree_lock.len(), 1, "{worktree_lock:?}");
    let open = |path: &Path| {
        std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(true)
            .open(path)
            .unwrap()
    };
    let foreign_worktree = open(&worktree_lock[0]);
    foreign_worktree.lock().unwrap();
    std::fs::write(
        &worktree_lock[0],
        "XYZ-1, pid 1111, since 2026-01-01T00:00:00Z, cargo build",
    )
    .unwrap();
    let foreign_slot = open(&slot_file(&home, 1));
    foreign_slot.lock().unwrap();
    std::fs::write(
        slot_file(&home, 1),
        "XYZ-2, pid 2222, since 2026-01-01T00:00:00Z, cargo test",
    )
    .unwrap();

    let err = root.path().join("run.err");
    let child = spawn_to(standin.command(dir, &home, &["cargo", "waits"]), &err);
    let mut run = Reaper(child);
    let lines = |n: usize| {
        let text = std::fs::read_to_string(&err).unwrap_or_default();
        text.matches(WAIT_LINE).count() >= n
    };
    wait_until("the first wait line", || lines(1));
    let text = std::fs::read_to_string(&err).unwrap();
    let first = text.lines().find(|l| l.contains(WAIT_LINE)).unwrap();
    assert!(first.contains("XYZ-1, pid 1111"), "{first}");
    assert!(!first.contains("XYZ-2"), "{first}");

    // The worktree lock is freed: the run waits for the slot, with a second line.
    drop(foreign_worktree);
    wait_until("the second wait line", || lines(2));
    let text = std::fs::read_to_string(&err).unwrap();
    let waits: Vec<&str> = text.lines().filter(|l| l.contains(WAIT_LINE)).collect();
    assert_eq!(waits.len(), 2, "{text}");
    assert!(waits[1].contains("XYZ-2, pid 2222"), "{}", waits[1]);
    assert!(!waits[1].contains("XYZ-1"), "{}", waits[1]);
    std::thread::sleep(Duration::from_millis(600));
    assert!(run.0.try_wait().unwrap().is_none(), "the run did not wait");
    assert_eq!(standin.runs("waits"), 0, "{}", standin.log());

    // The slot is freed: the run goes on, with no third line.
    drop(foreign_slot);
    let status = wait_exit(&mut run.0, "the run");
    assert!(status.success(), "{status:?}");
    assert_eq!(standin.runs("waits"), 1, "{}", standin.log());
    let text = std::fs::read_to_string(&err).unwrap();
    assert_eq!(text.matches(WAIT_LINE).count(), 2, "{text}");
}

// STP-42 AC-4 (DR1-2): the holder `stapel cargo` killed while its cargo child runs: the worktree lock
// stays held until the child ends, and a second run from that worktree waits for it; a run with the
// held mark takes no worktree lock.
#[test]
fn worktree_lock_outlives_its_holder() {
    /// Ends the stand-in `cargo` and the waiting run on drop, also when an assertion failed, and
    /// waits until the stand-in has let go of the worktree lock.
    struct Reaper {
        release: PathBuf,
        lock: Option<PathBuf>,
        children: Vec<Child>,
    }
    impl Drop for Reaper {
        fn drop(&mut self) {
            let _ = std::fs::write(&self.release, "");
            let end = Instant::now() + Duration::from_secs(20);
            for child in &mut self.children {
                while child.try_wait().ok().flatten().is_none() && Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(20));
                }
                let _ = child.kill();
                let _ = child.wait();
            }
            if let Some(path) = &self.lock {
                if let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(path) {
                    while file.try_lock().is_err() && Instant::now() < end {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
            }
        }
    }

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let lock_dir = home.join(".cache/stapel");
    std::fs::create_dir_all(&home).unwrap();
    let repo = slot_repo(3);
    let dir = repo.path();
    let standin_a = holding_standin(root.path(), "a");
    let standin_b = Standin::new(&root.path().join("b"));
    let standin_c = Standin::new(&root.path().join("c"));
    let mut reaper = Reaper {
        release: standin_a.ctl.join("release"),
        lock: None,
        children: Vec::new(),
    };

    let mut holder = standin_a
        .command(dir, &home, &["cargo", "hold-a"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("the holder's cargo", || standin_a.ctl.join("started").exists());
    let worktree_locks = || -> Vec<PathBuf> {
        std::fs::read_dir(&lock_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("worktree-")
            })
            .collect()
    };
    let locks = worktree_locks();
    assert_eq!(locks.len(), 1, "{locks:?}");
    let lock = locks[0].clone();
    reaper.lock = Some(lock.clone());
    let holder_pid = holder.id();

    // The holder dies; its cargo child keeps both the worktree lock and the slot.
    kill9(&mut holder);
    let is_held = |path: &Path| {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        file.try_lock().is_err()
    };
    std::thread::sleep(Duration::from_millis(300));
    assert!(is_held(&lock), "the worktree lock was freed with its holder");
    assert!(
        is_held(&slot_file(&home, 1)),
        "the slot was freed with its holder"
    );

    // A second run from the worktree waits and names the dead holder.
    let err = root.path().join("b.err");
    reaper.children.push(spawn_to(
        standin_b.command(dir, &home, &["cargo", "second"]),
        &err,
    ));
    wait_until("the wait line", || {
        std::fs::read_to_string(&err)
            .unwrap_or_default()
            .contains(WAIT_LINE)
    });
    let text = std::fs::read_to_string(&err).unwrap();
    let wait = text.lines().find(|l| l.contains(WAIT_LINE)).unwrap();
    assert!(wait.contains(&format!("pid {holder_pid}")), "{wait}");
    assert_eq!(standin_b.runs("second"), 0, "{}", standin_b.log());

    // A run with the held mark takes no worktree lock and does not wait.
    let err_c = root.path().join("c.err");
    let mut marked = standin_c.command(dir, &home, &["cargo", "marked"]);
    marked.env("STAPEL_CARGO_LOCK_HELD", "4242");
    let mut marked = spawn_to(marked, &err_c);
    let status = wait_exit(&mut marked, "the marked run");
    assert!(status.success(), "{status:?}");
    let text = std::fs::read_to_string(&err_c).unwrap();
    assert!(!text.contains(WAIT_LINE), "{text}");
    assert_eq!(standin_c.runs("marked"), 1, "{}", standin_c.log());
    assert_eq!(worktree_locks(), vec![lock.clone()]);
    assert_eq!(standin_b.runs("second"), 0, "{}", standin_b.log());

    // The child ends: the worktree lock is freed and the second run goes on.
    standin_a.touch("release");
    let mut second = reaper.children.pop().unwrap();
    let status = wait_exit(&mut second, "the second run");
    assert!(status.success(), "{status:?}");
    assert_eq!(standin_b.runs("second"), 1, "{}", standin_b.log());
    wait_until("the worktree lock to be free", || !is_held(&lock));
}

// STP-42 AC-4 (XR1-2): with N = 1 and slot 1 held by a foreign holder, run A of a worktree waits for the
// slot; run B of that worktree waits for the worktree lock and names the holder line of run A.
#[test]
fn worktree_holder_named_during_slot_wait() {
    /// Kills every waiting run on drop, also when an assertion failed.
    struct Reaper(Vec<Child>);
    impl Drop for Reaper {
        fn drop(&mut self) {
            for child in &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir_all(home.join(".cache/stapel")).unwrap();
    let repo = slot_repo(1);
    let dir = repo.path();
    let standin_a = Standin::new(&root.path().join("a"));
    let standin_b = Standin::new(&root.path().join("b"));

    let foreign = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(true)
        .open(slot_file(&home, 1))
        .unwrap();
    foreign.lock().unwrap();
    std::fs::write(
        slot_file(&home, 1),
        "XYZ-9, pid 4242, since 2026-01-01T00:00:00Z, cargo test",
    )
    .unwrap();

    let mut reaper = Reaper(Vec::new());
    let err_a = root.path().join("a.err");
    reaper.0.push(spawn_to(
        standin_a.command(dir, &home, &["cargo", "first"]),
        &err_a,
    ));
    let pid_a = reaper.0[0].id();
    wait_until("the wait line of run A", || {
        std::fs::read_to_string(&err_a)
            .unwrap_or_default()
            .contains(WAIT_LINE)
    });

    let err_b = root.path().join("b.err");
    reaper.0.push(spawn_to(
        standin_b.command(dir, &home, &["cargo", "second"]),
        &err_b,
    ));
    wait_until("the wait line of run B", || {
        std::fs::read_to_string(&err_b)
            .unwrap_or_default()
            .contains(WAIT_LINE)
    });
    let text = std::fs::read_to_string(&err_b).unwrap();
    let wait = text.lines().find(|l| l.contains(WAIT_LINE)).unwrap();
    assert!(
        wait.contains(&format!("pid {pid_a}")) && wait.contains("stapel cargo first"),
        "{wait}"
    );
    assert!(!wait.contains("an unknown holder"), "{wait}");
    assert_eq!(standin_a.runs("first"), 0, "{}", standin_a.log());
    assert_eq!(standin_b.runs("second"), 0, "{}", standin_b.log());
}
