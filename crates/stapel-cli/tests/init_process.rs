//! STP-22 step 1: `stapel init` writes the process files (the skill and four agents).

mod common;

use common::{git_repo, read, snapshot, stapel};
use predicates::str::contains;
use sha2::{Digest, Sha256};
use std::path::Path;

const SKILL: &str = ".claude/skills/stapel/SKILL.md";
const AGENTS: [&str; 4] = [
    ".claude/agents/stapel-builder.md",
    ".claude/agents/stapel-drift.md",
    ".claude/agents/stapel-reviewer.md",
    ".claude/agents/stapel-spec-reviewer.md",
];
const GOLDEN: &str = "tests/golden/init_agent_frontmatter.txt";

/// The frontmatter lines of `text`, from the first `---` to the next one, both included.
fn frontmatter(text: &str) -> Vec<String> {
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("---"), "no frontmatter in {text:?}");
    let mut out = vec!["---".to_string()];
    for line in lines {
        out.push(line.to_string());
        if line == "---" {
            return out;
        }
    }
    panic!("frontmatter is not closed in {text:?}");
}

/// Asserts that the file has exactly one valid marker line in its frontmatter and that the hash is
/// the SHA-256 of the file's bytes without that line and its newline; returns the version.
fn check_marker(dir: &Path, rel: &str) -> String {
    let text = read(dir, rel);
    let fm = frontmatter(&text);
    let markers: Vec<&String> = fm.iter().filter(|l| l.starts_with("# stapel:")).collect();
    assert_eq!(markers.len(), 1, "{rel}: {fm:?}");
    let line = markers[0];
    let rest = line.strip_prefix("# stapel: ").expect("marker form");
    let (version, hash) = rest.split_once(" sha256:").expect("marker form");
    let parts: Vec<&str> = version.split('.').collect();
    assert_eq!(parts.len(), 3, "{rel}: version {version}");
    assert!(parts.iter().all(|p| p.parse::<u64>().is_ok()), "{rel}");
    assert_eq!(hash.len(), 64, "{rel}");
    assert!(
        hash.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "{rel}"
    );
    let without = text.replacen(&format!("{line}\n"), "", 1);
    let digest: String = Sha256::digest(without.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(digest, hash, "{rel}: the hash does not match");
    version.to_string()
}

/// A repository whose `.stapel/stapel.toml` is the starter text with `edit` applied, `init` not yet
/// run on it.
fn repo_with_config(edit: impl Fn(String) -> String) -> tempfile::TempDir {
    let scratch = git_repo();
    stapel(scratch.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let text = edit(read(scratch.path(), ".stapel/stapel.toml"));
    let repo = git_repo();
    std::fs::create_dir_all(repo.path().join(".stapel")).unwrap();
    std::fs::write(repo.path().join(".stapel/stapel.toml"), text).unwrap();
    repo
}

fn replace_line(text: String, start: &str, new: &str) -> String {
    assert!(text.lines().any(|l| l.starts_with(start)), "no {start}");
    text.lines()
        .map(|l| if l.starts_with(start) { new } else { l })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn line_of(dir: &Path, rel: &str, key: &str) -> Option<String> {
    frontmatter(&read(dir, rel))
        .into_iter()
        .find(|l| l.starts_with(key))
}

// AC-1, git dirty tree: five created lines, each file with a valid marker.
#[test]
fn creates_the_process_files() {
    let repo = git_repo();
    let dir = repo.path();
    // A dirty tree changes nothing: an untracked file is already there.
    std::fs::write(dir.join("untracked.txt"), "x").unwrap();

    let mut cmd = stapel(dir);
    let assert = cmd.args(["init", "--prefix", "ABC"]).assert().success();
    let assert = assert.stdout(contains(format!("created: {SKILL}")));
    let mut assert = assert;
    for agent in AGENTS {
        assert = assert.stdout(contains(format!("created: {agent}")));
    }
    for rel in std::iter::once(SKILL).chain(AGENTS) {
        check_marker(dir, rel);
    }
}

// AC-2: the golden file holds the four frontmatters, in process-file order, each followed by a line
// `===` except the last; the marker version is written `<version>` and the hash `<hash>`.
#[test]
fn agent_frontmatter() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();

    let mut blocks = Vec::new();
    for rel in AGENTS {
        let version = check_marker(dir, rel);
        let fm = frontmatter(&read(dir, rel)).join("\n");
        let hash_at = fm.find("sha256:").expect("marker") + "sha256:".len();
        let hash = fm[hash_at..].lines().next().unwrap().to_string();
        blocks.push(
            fm.replace(
                &format!("{version} sha256:{hash}"),
                "<version> sha256:<hash>",
            ) + "\n",
        );
    }
    let got = blocks.join("===\n");
    let want = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN)).unwrap();
    assert_eq!(got, want);
}

// AC-2: the model comes from the role binding; reviewers share `reviewer`; no binding, no line.
#[test]
fn model_from_the_role() {
    let repo = repo_with_config(|t| {
        let t = replace_line(
            t,
            "reviewer",
            "reviewer = { provider = \"claude-code\", model = \"claude-opus-5-5[1m]\" }",
        );
        t.lines()
            .filter(|l| !l.starts_with("drift"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    });
    let dir = repo.path();
    stapel(dir).arg("init").assert().success();

    for rel in [AGENTS[2], AGENTS[3]] {
        assert_eq!(
            line_of(dir, rel, "model:").as_deref(),
            Some("model: claude-opus-5-5[1m]"),
            "{rel}"
        );
    }
    assert_eq!(
        line_of(dir, AGENTS[0], "model:").as_deref(),
        Some("model: claude-sonnet-5-5")
    );
    assert_eq!(line_of(dir, AGENTS[1], "model:"), None);
    check_marker(dir, AGENTS[1]);
}

// AC-3: the defaults, and the value of an allowed mode from the role binding.
#[test]
fn permission_mode_of_the_role() {
    let plain = git_repo();
    stapel(plain.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    assert_eq!(
        line_of(plain.path(), AGENTS[0], "permissionMode:").as_deref(),
        Some("permissionMode: acceptEdits")
    );
    for rel in &AGENTS[1..] {
        assert_eq!(
            line_of(plain.path(), rel, "permissionMode:").as_deref(),
            Some("permissionMode: default"),
            "{rel}"
        );
    }

    let set = repo_with_config(|t| {
        let t = replace_line(
            t,
            "drift",
            "drift = { provider = \"claude-code\", model = \"claude-sonnet-5-5\", permission_mode = \"plan\" }",
        );
        replace_line(
            t,
            "builder",
            "builder = { provider = \"claude-code\", model = \"claude-sonnet-5-5\", permission_mode = \"manual\" }",
        )
    });
    let dir = set.path();
    stapel(dir).arg("init").assert().success();
    assert_eq!(
        line_of(dir, AGENTS[1], "permissionMode:").as_deref(),
        Some("permissionMode: plan")
    );
    assert_eq!(
        line_of(dir, AGENTS[0], "permissionMode:").as_deref(),
        Some("permissionMode: manual")
    );
    assert_eq!(
        line_of(dir, AGENTS[2], "permissionMode:").as_deref(),
        Some("permissionMode: default")
    );
}

// AC-6: name, description, marker, in this order.
#[test]
fn skill_frontmatter() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();

    check_marker(dir, SKILL);
    let fm = frontmatter(&read(dir, SKILL));
    assert_eq!(fm.len(), 5, "{fm:?}");
    assert_eq!(fm[0], "---");
    assert_eq!(fm[1], "name: stapel");
    assert!(fm[2].starts_with("description: ") && fm[2].len() > "description: ".len());
    assert!(fm[3].starts_with("# stapel: "));
    assert_eq!(fm[4], "---");
}

// A second run changes nothing and says so.
#[test]
fn second_run_changes_nothing() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    for rel in std::iter::once(SKILL).chain(AGENTS) {
        assert!(dir.join(rel).is_file(), "{rel}");
    }
    let before = snapshot(dir);

    stapel(dir)
        .arg("init")
        .assert()
        .success()
        .stdout(contains("already set up: nothing changed"));

    assert_eq!(snapshot(dir), before);
}

/// Runs `init` in a repository whose `role` binding is `binding`; asserts exit 1, the stderr text
/// and that nothing was written.
fn refused_binding(role: &str, binding: &str, text: &str) {
    let repo = repo_with_config(|t| replace_line(t, role, &format!("{role} = {binding}")));
    let dir = repo.path();
    let before = snapshot(dir);
    stapel(dir)
        .arg("init")
        .assert()
        .code(1)
        .stderr(contains(text));
    assert_eq!(snapshot(dir), before, "{role} {binding}");
    for rel in std::iter::once(SKILL).chain(AGENTS) {
        assert!(!dir.join(rel).exists(), "{rel}");
    }
    assert!(!dir.join(".claude/settings.json").exists());
}

// AC-4: a bypass or unknown mode refuses; `author` is not an agent role and is accepted.
#[test]
fn refuses_a_bad_permission_mode() {
    refused_binding(
        "builder",
        "{ provider = \"claude-code\", model = \"claude-sonnet-5-5\", permission_mode = \"bypassPermissions\" }",
        "stapel: [models] builder: permission_mode bypassPermissions is not allowed: it turns off the permission dialog for every call the guard lets through",
    );
    refused_binding(
        "reviewer",
        "{ provider = \"claude-code\", model = \"claude-sonnet-5-5\", permission_mode = \"yolo\" }",
        "stapel: [models] reviewer: permission_mode \"yolo\" is not one of default, acceptEdits, auto, dontAsk, plan, manual",
    );
    refused_binding(
        "drift",
        "{ provider = \"claude-code\", model = \"claude-sonnet-5-5\", permission_mode = \"Default\" }",
        "stapel: [models] drift: permission_mode \"Default\" is not one of default, acceptEdits, auto, dontAsk, plan, manual",
    );

    let repo = repo_with_config(|t| {
        replace_line(
            t,
            "author",
            "author = { provider = \"claude-code\", model = \"claude-opus-5-5\", permission_mode = \"bypassPermissions\" }",
        )
    });
    stapel(repo.path()).arg("init").assert().success();
}

// AC-5: a model that cannot be written to an agent file refuses.
#[test]
fn refuses_a_bad_model() {
    for model in ["", "[1m]", "a b", "x\\npermissionMode: plan", "x#y"] {
        let shown = model.replace("\\n", "\n");
        refused_binding(
            "builder",
            &format!("{{ provider = \"claude-code\", model = \"{model}\" }}"),
            &format!(
                "stapel: [models] builder: model \"{shown}\" cannot be written to an agent file"
            ),
        );
    }
}

// AC-10: a directory at a process file's path refuses before any write.
#[test]
fn refuses_a_directory() {
    let repo = git_repo();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".claude/agents/stapel-builder.md")).unwrap();

    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .code(1)
        .stderr(contains(
            "stapel: .claude/agents/stapel-builder.md is a directory; nothing was written",
        ));

    assert!(!dir.join(".stapel").exists());
    assert!(!dir.join(".claude/settings.json").exists());
    for rel in std::iter::once(SKILL).chain(AGENTS) {
        assert!(!dir.join(rel).is_file(), "{rel}");
    }
}
