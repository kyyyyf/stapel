mod common;

fn version_with(value: Option<&str>) -> String {
    let dir = common::bare_dir();
    let mut cmd = common::stapel(dir.path());
    cmd.env_remove("STAPEL_TEST_BUILD_COMMIT");
    if let Some(v) = value {
        cmd.env("STAPEL_TEST_BUILD_COMMIT", v);
    }
    let out = cmd.arg("--version").assert().success().get_output().clone();
    String::from_utf8(out.stdout).unwrap()
}

const WORKFLOW: &str = ".github/workflows/stapel-check.yml";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn init_with(dir: &std::path::Path, commit: Option<&str>, args: &[&str]) -> String {
    let mut cmd = common::stapel(dir);
    cmd.env_remove("STAPEL_TEST_BUILD_COMMIT");
    if let Some(v) = commit {
        cmd.env("STAPEL_TEST_BUILD_COMMIT", v);
    }
    let out = cmd.arg("init").args(args).assert().success().get_output().clone();
    String::from_utf8(out.stdout).unwrap() + &String::from_utf8(out.stderr).unwrap()
}

// STP-33 AC-2: the lines of the written workflow.
#[test]
fn workflow_lines() {
    let repo = common::git_repo();
    let dir = repo.path();
    init_with(dir, Some(SHA), &["--prefix", "ABC"]);
    let text = common::read(dir, WORKFLOW);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let install = format!(
        "cargo install --git https://github.com/kyyyyf/stapel --rev {SHA} --locked stapel-cli"
    );
    for want in [
        "push:",
        "pull_request:",
        "branches: [\"ABC-[0-9]+\"]",
        "if: github.event_name == 'push' || startsWith(github.head_ref, 'ABC-')",
        "ref: ${{ github.event.pull_request.head.sha || github.sha }}",
        "fetch-depth: 0",
        "persist-credentials: false",
        "git fetch --no-tags --update-head-ok origin '+refs/heads/*:refs/heads/*'",
        "contents: read",
        "timeout-minutes: 60",
        "BRANCH: ${{ github.head_ref || github.ref_name }}",
        install.as_str(),
        "stapel check \"$BRANCH\"",
    ] {
        assert!(
            lines.iter().any(|l| l.contains(want)),
            "missing {want:?} in {text}"
        );
    }
    let at = lines.iter().position(|l| *l == "permissions:");
    assert!(
        at.is_some_and(|i| lines.get(i + 1) == Some(&"contents: read")),
        "missing permissions: contents: read in {text}"
    );
    assert!(!text.contains("secrets."), "{text}");
}

// STP-33 AC-1: the written file is the marker line plus the golden cargo workflow, and `init` says
// `created:`.
#[test]
fn writes_the_workflow() {
    use sha2::{Digest, Sha256};
    let repo = common::git_repo();
    let dir = repo.path();
    let output = init_with(dir, Some(SHA), &["--prefix", "ABC"]);
    assert!(
        output
            .lines()
            .any(|l| l == format!("created: {WORKFLOW}")),
        "{output}"
    );
    let text = common::read(dir, WORKFLOW);
    let (marker, body) = text.split_once('\n').expect("marker line");
    let rest = marker.strip_prefix("# stapel: ").expect("marker form");
    let (version, hash) = rest.split_once(" sha256:").expect("marker form");
    assert_eq!(version, env!("CARGO_PKG_VERSION"));
    let digest: String = Sha256::digest(body.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(digest, hash, "the hash does not match");
    let golden = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/stapel_check_cargo.yml"),
    )
    .unwrap()
    .replace("<prefix>", "ABC")
    .replace("<commit>", SHA);
    assert_eq!(body, golden);
}

// STP-33 AC-6: the local mode writes no workflow and prints nothing about it.
#[test]
fn local_mode_writes_no_workflow() {
    let sha_dirty = format!("{SHA}-dirty");
    // (commit, extra toml appended before a second run, folder at the workflow path)
    let cases: [(Option<&str>, &str, bool); 5] = [
        (Some(SHA), "", false),
        (None, "", false),
        (Some(SHA), "\n[tickets]\nkey = \"{n}\"\n", false),
        (Some(SHA), "\n[check]\nrunner = \"junit\"\ncommand = \"true {file}\"\nreport = \"junit.xml\"\n", false),
        (Some(SHA), "", true),
    ];
    for (commit, extra, folder) in cases {
        let repo = common::git_repo();
        let dir = repo.path();
        if folder {
            std::fs::create_dir_all(dir.join(WORKFLOW)).unwrap();
        }
        let mut outputs = vec![init_with(dir, commit, &["--local", "--prefix", "ABC"])];
        if !extra.is_empty() {
            let path = dir.join(".stapel/stapel.toml");
            let mut text = std::fs::read_to_string(&path).unwrap();
            if extra.contains("[tickets]") {
                text = text.replace("ABC-{n}", "{n}");
            } else {
                text.push_str(extra);
            }
            std::fs::write(&path, text).unwrap();
            outputs.push(init_with(dir, commit, &["--local"]));
        }
        outputs.push(init_with(dir, Some(&sha_dirty), &["--local"]));
        for out in outputs {
            for line in out.lines() {
                assert!(
                    !line.contains("stapel-check") && !line.contains("workflow"),
                    "line about the workflow: {line}\n{out}"
                );
            }
        }
        if folder {
            assert!(dir.join(WORKFLOW).is_dir());
        } else {
            assert!(!dir.join(".github").exists());
        }
    }
}

// STP-33 AC-7: a missing build commit writes no workflow and prints the warning; the skill is written.
#[test]
fn no_build_commit() {
    let warning = "warning: this stapel has no build commit, so .github/workflows/stapel-check.yml \
                   was not written; reinstall stapel from a clean checkout as docs/install.md says";
    let upper = SHA.to_uppercase();
    let short = &SHA[..39];
    for commit in [None, Some(""), Some("abc"), Some(short), Some(upper.as_str())] {
        let repo = common::git_repo();
        let dir = repo.path();
        let output = init_with(dir, commit, &["--prefix", "ABC"]);
        assert!(output.lines().any(|l| l == warning), "{commit:?}: {output}");
        assert!(!dir.join(".github").exists(), "{commit:?}");
        assert!(dir.join(".claude/skills/stapel/SKILL.md").is_file(), "{commit:?}");
    }
}

// STP-33 AC-7: a dirty build commit writes no workflow and the warning names the value.
#[test]
fn dirty_build_commit() {
    let repo = common::git_repo();
    let dir = repo.path();
    let value = format!("{SHA}-dirty");
    let output = init_with(dir, Some(&value), &["--prefix", "ABC"]);
    let warning = format!(
        "warning: this stapel was built from a dirty tree ({value}), so \
         .github/workflows/stapel-check.yml was not written; commit and push the change, then \
         reinstall stapel"
    );
    assert!(output.lines().any(|l| l == warning), "{output}");
    assert!(!dir.join(".github").exists());
    assert!(dir.join(".claude/skills/stapel/SKILL.md").is_file());
}

// STP-33 AC-8: a key without a `<PREFIX>-{n}` form writes no workflow and prints the warning, also
// when the build commit is missing.
#[test]
fn key_without_prefix() {
    let bad = ["{n}", "abc-{n}", "ABC{n}", "A-{n}", "ABCDEFGHI-{n}"];
    for commit in [Some(SHA), None] {
        for key in bad {
            let repo = common::git_repo();
            let dir = repo.path();
            init_with(dir, None, &["--prefix", "ABC"]);
            let path = dir.join(".stapel/stapel.toml");
            let text = std::fs::read_to_string(&path).unwrap();
            std::fs::write(&path, text.replace("ABC-{n}", key)).unwrap();
            let output = init_with(dir, commit, &[]);
            let warning = format!(
                "warning: the ticket key {key} is not <PREFIX>-{{n}}, so \
                 .github/workflows/stapel-check.yml was not written"
            );
            assert!(output.lines().any(|l| l == warning), "{key} {commit:?}: {output}");
            assert!(!output.contains("no build commit"), "{key} {commit:?}: {output}");
            assert!(!dir.join(".github").exists(), "{key} {commit:?}");
        }
    }
    for key in ["AB-{n}", "ABCDEFGH-{n}"] {
        let repo = common::git_repo();
        let dir = repo.path();
        init_with(dir, None, &["--prefix", "ABC"]);
        let path = dir.join(".stapel/stapel.toml");
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replace("ABC-{n}", key)).unwrap();
        let output = init_with(dir, Some(SHA), &[]);
        assert!(!output.contains("is not <PREFIX>-{n}"), "{key}: {output}");
        assert!(dir.join(WORKFLOW).is_file(), "{key}");
    }
}

fn has_line(output: &str, line: &str) -> bool {
    output.lines().any(|l| l == line)
}

fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

const NOT_STAPELS: &str = "warning: .github/workflows/stapel-check.yml is not the file stapel \
                           wrote; kept. Delete it and run stapel init to take stapel's text";

// STP-33 AC-3: an unchanged workflow of stapel is rewritten when its text differs, silently skipped
// when equal, and left alone under a dirty commit or a key without prefix.
#[test]
fn rewrites_an_unchanged_workflow() {
    let other = "fedcba9876543210fedcba9876543210fedcba98";
    let sha64 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let repo = common::git_repo();
    let dir = repo.path();
    init_with(dir, Some(SHA), &["--prefix", "ABC"]);
    let first = common::read(dir, WORKFLOW);

    let again = init_with(dir, Some(SHA), &[]);
    assert!(!again.contains("stapel-check"), "{again}");
    assert_eq!(common::read(dir, WORKFLOW), first);

    let updated = init_with(dir, Some(other), &[]);
    assert!(has_line(&updated, &format!("updated: {WORKFLOW}")), "{updated}");
    let text = common::read(dir, WORKFLOW);
    assert!(text.contains(other) && !text.contains(SHA), "{text}");

    let updated = init_with(dir, Some(sha64), &[]);
    assert!(has_line(&updated, &format!("updated: {WORKFLOW}")), "{updated}");
    assert!(common::read(dir, WORKFLOW).contains(sha64));
    let before = common::read(dir, WORKFLOW);

    let dirty = format!("{SHA}-dirty");
    let out = init_with(dir, Some(&dirty), &[]);
    assert!(out.contains("built from a dirty tree"), "{out}");
    assert!(!out.contains("updated:") && !out.contains(NOT_STAPELS), "{out}");
    assert_eq!(common::read(dir, WORKFLOW), before);

    let path = dir.join(".stapel/stapel.toml");
    let toml = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, toml.replace("ABC-{n}", "{n}")).unwrap();
    let out = init_with(dir, Some(SHA), &[]);
    assert!(out.contains("is not <PREFIX>-{n}"), "{out}");
    assert!(!out.contains("updated:") && !out.contains(NOT_STAPELS), "{out}");
    assert_eq!(common::read(dir, WORKFLOW), before);
}

// STP-33 AC-4: a workflow that is not stapel's is kept with a warning, byte for byte.
#[test]
fn keeps_a_hand_edited_workflow() {
    let other = "fedcba9876543210fedcba9876543210fedcba98";
    let repo = common::git_repo();
    let dir = repo.path();
    init_with(dir, Some(SHA), &["--prefix", "ABC"]);
    let path = dir.join(WORKFLOW);
    let original = std::fs::read_to_string(&path).unwrap();

    let marker_removed: String = original.split_inclusive('\n').skip(1).collect();
    let variants = [
        format!("{original}# my line\n"),
        marker_removed,
        String::new(),
    ];
    for edited in variants {
        std::fs::write(&path, &edited).unwrap();
        let out = init_with(dir, Some(other), &[]);
        assert!(has_line(&out, NOT_STAPELS), "{out}");
        assert!(!out.contains("updated:"), "{out}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), edited);
    }

    // A hand-edited file with a missing build commit: only the missing-commit warning.
    std::fs::write(&path, format!("{original}# my line\n")).unwrap();
    let out = init_with(dir, None, &[]);
    assert!(out.contains("has no build commit"), "{out}");
    assert!(!out.contains(NOT_STAPELS), "{out}");

    // A symlink to a stapel workflow is not stapel's file.
    #[cfg(unix)]
    {
        let target = dir.join("elsewhere.yml");
        std::fs::write(&target, &original).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        let out = init_with(dir, Some(other), &[]);
        assert!(has_line(&out, NOT_STAPELS), "{out}");
        assert!(std::fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), original);
    }
}

// STP-33 AC-4: a workflow of a newer stapel is kept with the newer-version warning.
#[test]
fn keeps_a_newer_workflow() {
    let repo = common::git_repo();
    let dir = repo.path();
    init_with(dir, Some(SHA), &["--prefix", "ABC"]);
    let path = dir.join(WORKFLOW);
    let original = std::fs::read_to_string(&path).unwrap();
    let (_, body) = original.split_once('\n').unwrap();
    let newer = format!("# stapel: 99.0.0 sha256:{}\n{body}", sha256_hex(body));
    std::fs::write(&path, &newer).unwrap();
    let out = init_with(dir, Some("fedcba9876543210fedcba9876543210fedcba98"), &[]);
    let own = env!("CARGO_PKG_VERSION");
    let warning = format!(
        "warning: {WORKFLOW} was written by stapel 99.0.0, newer than this stapel {own}; kept"
    );
    assert!(has_line(&out, &warning), "{out}");
    assert!(!out.contains("updated:"), "{out}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
}

#[test]
fn version_names_the_commit() {
    let version = env!("CARGO_PKG_VERSION");
    let sha40 = "0123456789abcdef0123456789abcdef01234567";
    let sha64 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    // clean
    assert_eq!(
        version_with(Some(sha40)),
        format!("stapel {version} ({sha40})\n")
    );
    assert_eq!(
        version_with(Some(sha64)),
        format!("stapel {version} ({sha64})\n")
    );
    // dirty
    let dirty = format!("{sha40}-dirty");
    assert_eq!(
        version_with(Some(&dirty)),
        format!("stapel {version} ({dirty})\n")
    );
    // missing: unset, empty, uppercase, 39 digits, other text
    let upper = sha40.to_uppercase();
    let short = &sha40[..39];
    for missing in [None, Some(""), Some(upper.as_str()), Some(short), Some("main")] {
        assert_eq!(version_with(missing), format!("stapel {version}\n"));
    }
}

fn refused(dir: &std::path::Path, commit: Option<&str>, message: &str) {
    let mut cmd = common::stapel(dir);
    cmd.env_remove("STAPEL_TEST_BUILD_COMMIT");
    if let Some(v) = commit {
        cmd.env("STAPEL_TEST_BUILD_COMMIT", v);
    }
    let out = cmd
        .args(["init", "--prefix", "ABC"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.lines().any(|l| l == format!("stapel: {message}")),
        "{stderr}"
    );
    assert!(!dir.join(".stapel").exists());
    assert!(!dir.join(".claude").exists());
}

// STP-33 AC-5: a directory at the workflow's path refuses before any write.
#[test]
fn workflow_path_is_a_directory() {
    for commit in [Some(SHA), None] {
        let repo = common::git_repo();
        let dir = repo.path();
        std::fs::create_dir_all(dir.join(WORKFLOW)).unwrap();
        refused(
            dir,
            commit,
            &format!("{WORKFLOW} is a directory; nothing was written"),
        );
    }
}

// STP-33 AC-5: `.github` or `.github/workflows` that is a file or a symlink refuses before any
// write, and nothing is written through the symlink.
#[test]
fn github_folder_is_not_a_directory() {
    for commit in [Some(SHA), None] {
        // `.github` a file.
        let repo = common::git_repo();
        let dir = repo.path();
        std::fs::write(dir.join(".github"), "x").unwrap();
        refused(
            dir,
            commit,
            ".github is not a directory; nothing was written",
        );
        assert_eq!(common::read(dir, ".github"), "x");

        // `.github/workflows` a file.
        let repo = common::git_repo();
        let dir = repo.path();
        std::fs::create_dir_all(dir.join(".github")).unwrap();
        std::fs::write(dir.join(".github/workflows"), "x").unwrap();
        refused(
            dir,
            commit,
            ".github/workflows is not a directory; nothing was written",
        );
        assert_eq!(common::read(dir, ".github/workflows"), "x");

        // `.github` a symlink to a folder outside the repository.
        #[cfg(unix)]
        {
            let repo = common::git_repo();
            let dir = repo.path();
            let outside = common::bare_dir();
            std::os::unix::fs::symlink(outside.path(), dir.join(".github")).unwrap();
            refused(
                dir,
                commit,
                ".github is not a directory; nothing was written",
            );
            assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
        }
    }
}
