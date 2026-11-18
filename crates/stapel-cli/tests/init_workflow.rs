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
