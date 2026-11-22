//! STP-37: the Node pin of `stapel check` under the junit runner.

mod common;

use common::{JunitRepo, git_in, junit_repo, read, stapel};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn commit(dir: &Path, message: &str) {
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "--allow-empty", "-m", message]);
}

/// One full `stapel check` at a time on the machine.
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A folder holding an executable `node` that prints `v20.11.1`.
fn fake_node() -> tempfile::TempDir {
    let bin = tempfile::tempdir().unwrap();
    let path = bin.path().join("node");
    std::fs::write(&path, "#!/bin/sh\necho v20.11.1\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

/// Exit code and the text of stdout and stderr of `stapel check ABC-1` with the args, with the fake
/// `node` first on an absolute `PATH`.
fn run(dir: &Path, bin: &Path, args: &[&str]) -> (i32, String) {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let old = std::env::var("PATH").unwrap_or_default();
    let out = stapel(dir)
        .env("PATH", format!("{}:{old}", bin.display()))
        .args(["check", "ABC-1"])
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// A junit ticket with one passing step; `files` are committed before the step.
fn pinned(files: &[(&str, &str)]) -> JunitRepo {
    let repo = junit_repo();
    let dir = repo.dir();
    for (name, text) in files {
        write(dir, name, text);
    }
    commit(dir, "setup: the pins");
    write(
        dir,
        "__tests__/x.test.ts",
        "it(\"one\", () => { /* needs src/ok.ts */ });\n",
    );
    commit(dir, "ABC-1 step 1 RED: one");
    write(dir, "src/ok.ts", "export const ok = 1;\n");
    commit(dir, "ABC-1 step 1 GREEN: one");
    repo
}

// AC-6: a pin that matches `node --version` is printed after the `level:` line and the check goes on;
// the cargo runner and `--history` do not read the pins.
#[test]
fn a_matching_pin_runs() {
    let bin = fake_node();
    for pin in ["20", "v20.11", "20.11.1", "  v20 \n"] {
        for level in ["steps", "head", "full"] {
            let repo = pinned(&[(".nvmrc", &format!("{pin}\n"))]);
            let (code, text) = run(repo.dir(), bin.path(), &["--level", level]);
            assert_eq!(code, 0, "{pin:?} {level}: {text}");
            let lines: Vec<&str> = text.lines().collect();
            let at = lines
                .iter()
                .position(|l| *l == format!("level: {level}"))
                .unwrap_or_else(|| panic!("no level line:\n{text}"));
            assert_eq!(
                lines[at + 1],
                format!("node: v20.11.1 (pin {} in .nvmrc)", pin.trim()),
                "{pin:?} {level}: {text}"
            );
            assert!(
                lines.iter().any(|l| l.starts_with("step 1: pass")),
                "{text}"
            );
        }
    }
    // `.node-version` is read too.
    let repo = pinned(&[(".node-version", "20.11\n")]);
    let (code, text) = run(repo.dir(), bin.path(), &[]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.lines()
            .any(|l| l == "node: v20.11.1 (pin 20.11 in .node-version)"),
        "{text}"
    );
    // Two files that agree: `20` and `v20`.
    let repo = pinned(&[(".nvmrc", "20\n"), (".node-version", "v20\n")]);
    let (code, text) = run(repo.dir(), bin.path(), &[]);
    assert_eq!(code, 0, "{text}");
    assert!(text.lines().any(|l| l.starts_with("node: v20.11.1")), "{text}");
    // No pin: no `node:` line.
    let repo = pinned(&[]);
    let (code, text) = run(repo.dir(), bin.path(), &[]);
    assert_eq!(code, 0, "{text}");
    assert!(!text.contains("node:"), "{text}");

    // `--history` does not read a mismatching pin.
    let repo = pinned(&[(".nvmrc", "18\n")]);
    let (_, text) = run(repo.dir(), bin.path(), &["--history"]);
    assert!(!text.contains("node") && !text.contains("pin"), "{text}");

    // Under the cargo runner a `.nvmrc` of `lts/*` is ignored.
    let repo = common::git_repo();
    let dir = repo.path();
    write(
        dir,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/tiny\"]\nresolver = \"2\"\n",
    );
    write(
        dir,
        "crates/tiny/Cargo.toml",
        "[package]\nname = \"tiny\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(dir, "crates/tiny/src/lib.rs", "pub fn one() {}\n");
    write(
        dir,
        "Cargo.lock",
        "version = 3\n\n[[package]]\nname = \"tiny\"\nversion = \"0.1.0\"\n",
    );
    write(dir, ".gitignore", "/target\n");
    write(dir, ".nvmrc", "lts/*\n");
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    stapel(dir).args(["new", "First"]).assert().success();
    commit(dir, "initial");
    write(
        dir,
        "crates/tiny/tests/basic.rs",
        "#[test]\nfn needs_two() {\n    // fake: needs pub fn two\n}\n",
    );
    commit(dir, "ABC-1 step 1 RED: two is missing");
    write(
        dir,
        "crates/tiny/src/lib.rs",
        "pub fn one() {}\npub fn two() {}\n",
    );
    commit(dir, "ABC-1 step 1 GREEN: two");
    let scripts = tempfile::tempdir().unwrap();
    let fake = common::fake_cargo(scripts.path());
    let text: String = read(dir, ".stapel/stapel.toml")
        .lines()
        .map(|l| {
            if l.starts_with("timeout_secs = ") {
                format!("{l}\ncargo = \"{fake}\"\n")
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    write(dir, ".stapel/stapel.toml", &text);
    commit(dir, "ABC-1: the check keys");
    let (code, text) = run(dir, bin.path(), &[]);
    assert_eq!(code, 0, "{text}");
    assert!(!text.contains("node:") && !text.contains("not a version"), "{text}");
}

// AC-8: a pin that is not a version, two pins that disagree, and a `node` that fails refuse the check
// with exit 2 and no test runs.
#[test]
fn refuses_a_bad_pin() {
    let bin = fake_node();
    let refused = |files: &[(&str, &str)], bin: &Path, want: &str| {
        let repo = pinned(files);
        let (code, text) = run(repo.dir(), bin, &[]);
        assert_eq!(code, 2, "{files:?}: {text}");
        assert!(
            text.lines().any(|l| l.starts_with(want)),
            "{files:?} wants {want:?}: {text}"
        );
        assert!(!text.contains("step 1:"), "no test ran: {text}");
        assert!(repo.runs().is_empty(), "{:?}", repo.runs());
    };
    for (pin, shown) in [
        ("lts/*\n", "lts/*"),
        ("\n", ""),
        ("20.x\n", "20.x"),
        ("20.1.2.3\n", "20.1.2.3"),
    ] {
        refused(
            &[(".nvmrc", pin)],
            bin.path(),
            &format!("stapel: .nvmrc: not a version: {shown}"),
        );
    }
    refused(
        &[(".node-version", "lts/*\n")],
        bin.path(),
        "stapel: .node-version: not a version: lts/*",
    );
    refused(
        &[(".nvmrc", "20\n18\n")],
        bin.path(),
        "stapel: .nvmrc: not a version: ",
    );
    refused(
        &[(".nvmrc", "20\n"), (".node-version", "18\n")],
        bin.path(),
        "stapel: .nvmrc and .node-version disagree: 20, 18",
    );
    // The alias is named, not the disagreement.
    refused(
        &[(".nvmrc", "lts/*\n"), (".node-version", "18\n")],
        bin.path(),
        "stapel: .nvmrc: not a version: lts/*",
    );

    // A `node` that exits 1 and a `node` that prints no version.
    for body in ["#!/bin/sh\nexit 1\n", "#!/bin/sh\necho hello\n"] {
        let bad = tempfile::tempdir().unwrap();
        let path = bad.path().join("node");
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        refused(
            &[(".nvmrc", "20\n")],
            bad.path(),
            "stapel: node --version failed: ",
        );
    }

    // No `node` on `PATH`: a folder with `git` only, then the system folders.
    let git = String::from_utf8(
        std::process::Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let only = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(git.trim(), only.path().join("git")).unwrap();
    let repo = pinned(&[(".nvmrc", "20\n")]);
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let out = stapel(repo.dir())
        .env("PATH", format!("{}:/usr/bin:/bin", only.path().display()))
        .args(["check", "ABC-1"])
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(
        text.lines()
            .any(|l| l.starts_with("stapel: node --version failed: ")),
        "{text}"
    );
    assert!(!text.contains("step 1:"), "no test ran: {text}");
}

// AC-7: a pin that does not match refuses the check with exit 2 and no test runs.
#[test]
fn a_mismatch_refuses() {
    let bin = fake_node();
    for (pin, file) in [
        ("18", ".nvmrc"),
        ("20.1", ".nvmrc"),
        ("v20.12", ".node-version"),
        ("20.11.2", ".nvmrc"),
    ] {
        let repo = pinned(&[(file, &format!("{pin}\n"))]);
        let (code, text) = run(repo.dir(), bin.path(), &[]);
        assert_eq!(code, 2, "{pin}: {text}");
        assert!(
            text.lines().any(|l| l
                == format!("stapel: node v20.11.1 does not match the pin {pin} in {file}")),
            "{pin}: {text}"
        );
        assert!(!text.contains("step 1:"), "no test ran: {text}");
        assert!(repo.runs().is_empty(), "{:?}", repo.runs());
    }
    // The refusal holds at the level `steps` too.
    let repo = pinned(&[(".nvmrc", "18\n")]);
    let (code, text) = run(repo.dir(), bin.path(), &["--level", "steps"]);
    assert_eq!(code, 2, "{text}");
    assert!(repo.runs().is_empty(), "{:?}", repo.runs());
}

// AC-6 (CR1-4): a `node` that hangs ends at the time limit of the check; the check is refused with
// exit 2 and no test runs.
#[test]
fn a_hanging_node_ends_at_the_limit() {
    let bad = tempfile::tempdir().unwrap();
    let path = bad.path().join("node");
    std::fs::write(&path, "#!/bin/sh\nexec sleep 60\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let repo = pinned(&[(".nvmrc", "20\n")]);
    let dir = repo.dir();
    let toml = read(dir, ".stapel/stapel.toml");
    assert!(toml.contains("timeout_secs = 60"), "{toml}");
    write(
        dir,
        ".stapel/stapel.toml",
        &toml.replace("timeout_secs = 60", "timeout_secs = 10"),
    );
    commit(dir, "ABC-1: a short limit");
    let started = std::time::Instant::now();
    let (code, text) = run(dir, bad.path(), &[]);
    assert!(started.elapsed().as_secs() < 50, "{:?}", started.elapsed());
    assert_eq!(code, 2, "{text}");
    assert!(
        text.lines().any(|l| l
            == "stapel: node --version failed: the time limit of 10 s for the check ran out"),
        "{text}"
    );
    assert!(!text.contains("step 1:"), "no test ran: {text}");
    assert!(repo.runs().is_empty(), "{:?}", repo.runs());
}
