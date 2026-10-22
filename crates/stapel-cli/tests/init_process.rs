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

/// Rewrites `rel` as an unchanged file of version 0.0.1 whose body has an extra line.
fn make_old(dir: &Path, rel: &str) {
    let text = read(dir, rel);
    let marker = frontmatter(&text)
        .into_iter()
        .find(|l| l.starts_with("# stapel:"))
        .expect("marker");
    let mut without = text.replacen(&format!("{marker}\n"), "", 1);
    without.push_str("A hand-made extra line.\n");
    let digest: String = Sha256::digest(without.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let at = without.find("\n---\n").expect("frontmatter end") + 1;
    without.insert_str(at, &format!("# stapel: 0.0.1 sha256:{digest}\n"));
    std::fs::write(dir.join(rel), without).unwrap();
}

// AC-7: an unchanged file of an older version with other text is updated; the others stay.
#[test]
fn updates_an_unchanged_file() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let want = read(dir, AGENTS[0]);
    make_old(dir, AGENTS[0]);
    assert_ne!(read(dir, AGENTS[0]), want);
    let others = read(dir, AGENTS[1]);

    let out = stapel(dir).arg("init").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout
            .lines()
            .any(|l| l == format!("updated: {}", AGENTS[0])),
        "{stdout}"
    );
    assert_eq!(stdout.matches("updated:").count(), 1, "{stdout}");
    assert_eq!(read(dir, AGENTS[0]), want);
    assert_eq!(read(dir, AGENTS[1]), others);
}

// AC-11: a first run ends with the restart note; a run that creates no folder prints none.
#[test]
fn restart_note() {
    let note = "note: restart Claude Code so that it loads the new agents and skill";
    let repo = git_repo();
    let dir = repo.path();
    let out = stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.lines().last(), Some(note), "{stdout}");

    std::fs::remove_file(dir.join(AGENTS[2])).unwrap();
    let out = stapel(dir).arg("init").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains(&format!("created: {}", AGENTS[2])),
        "{stdout}"
    );
    assert!(!stdout.contains("restart Claude Code"), "{stdout}");
}

const KEPT: &str =
    "is not the file stapel wrote; kept. Delete it and run stapel init to take stapel's text";

fn kept_warning(rel: &str) -> String {
    format!("warning: {rel} {KEPT}")
}

fn run_init(dir: &Path) -> (String, String) {
    let out = stapel(dir).arg("init").assert().success();
    let out = out.get_output();
    (
        String::from_utf8(out.stdout.clone()).unwrap(),
        String::from_utf8(out.stderr.clone()).unwrap(),
    )
}

/// `rel` with its marker line replaced by `marker` (removed when `None`).
fn with_marker(dir: &Path, rel: &str, marker: Option<&str>) -> String {
    let text = read(dir, rel);
    let line = frontmatter(&text)
        .into_iter()
        .find(|l| l.starts_with("# stapel:"))
        .expect("marker");
    let new = marker.map(|m| format!("{m}\n")).unwrap_or_default();
    text.replacen(&format!("{line}\n"), &new, 1)
}

/// Rewrites `rel` as an unchanged file of `version` whose body has an extra line.
fn make_version(dir: &Path, rel: &str, version: &str) {
    let without = with_marker(dir, rel, None) + "A hand-made extra line.\n";
    let digest: String = Sha256::digest(without.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let at = without.find("\n---\n").expect("frontmatter end") + 1;
    let mut text = without;
    text.insert_str(at, &format!("# stapel: {version} sha256:{digest}\n"));
    std::fs::write(dir.join(rel), text).unwrap();
}

// AC-8: an edited file is kept with the warning; the other files are written; a run that has only
// the warning does not say that nothing changed.
#[test]
fn keeps_an_edited_file() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let mut text = read(dir, AGENTS[0]);
    text.push_str("one more line\n");
    std::fs::write(dir.join(AGENTS[0]), &text).unwrap();
    std::fs::remove_file(dir.join(AGENTS[1])).unwrap();

    let (stdout, stderr) = run_init(dir);
    assert!(
        stderr.lines().any(|l| l == kept_warning(AGENTS[0])),
        "{stderr}"
    );
    assert_eq!(read(dir, AGENTS[0]), text);
    assert!(
        stdout.contains(&format!("created: {}", AGENTS[1])),
        "{stdout}"
    );
    assert!(dir.join(AGENTS[1]).is_file());

    let (stdout, stderr) = run_init(dir);
    assert!(
        stderr.lines().any(|l| l == kept_warning(AGENTS[0])),
        "{stderr}"
    );
    assert!(!stdout.contains("already set up"), "{stdout}");
    assert_eq!(read(dir, AGENTS[0]), text);
}

// AC-8: no marker line, a marker that is not valid, two markers, a marker in the body only.
#[test]
fn keeps_a_file_without_a_valid_marker() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let line = frontmatter(&read(dir, AGENTS[0]))
        .into_iter()
        .find(|l| l.starts_with("# stapel:"))
        .unwrap();
    let hash = line.split_once("sha256:").unwrap().1.to_string();
    let valid = format!("# stapel: 0.0.1 sha256:{hash}");
    let cases: Vec<(&str, String)> = vec![
        (SKILL, with_marker(dir, SKILL, None)),
        (
            AGENTS[0],
            with_marker(
                dir,
                AGENTS[0],
                Some(&format!("# stapel: 0.1 sha256:{hash}")),
            ),
        ),
        (
            AGENTS[1],
            with_marker(
                dir,
                AGENTS[1],
                Some(&format!("# stapel: 0.0.1 sha256:{}", hash.to_uppercase())),
            ),
        ),
        (
            AGENTS[2],
            with_marker(dir, AGENTS[2], Some(&format!("{line}\n{valid}"))),
        ),
        (
            AGENTS[3],
            with_marker(dir, AGENTS[3], None) + &format!("{valid}\n"),
        ),
    ];
    for (rel, text) in &cases {
        std::fs::write(dir.join(rel), text).unwrap();
    }

    let (stdout, stderr) = run_init(dir);
    for (rel, text) in &cases {
        assert!(
            stderr.lines().any(|l| l == kept_warning(rel)),
            "{rel}: {stderr}"
        );
        assert_eq!(&read(dir, rel), text, "{rel}");
    }
    assert!(!stdout.contains("already set up"), "{stdout}");
}

// AC-8: a symlink (to a copy of stapel's text, or dangling) and a file that is not UTF-8.
#[test]
fn keeps_a_symlink_and_non_utf8() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let outside = tempfile::tempdir().unwrap();
    let copy = outside.path().join("copy.md");
    std::fs::write(&copy, read(dir, AGENTS[0])).unwrap();
    let dangling = outside.path().join("missing.md");
    let mut bytes = read(dir, AGENTS[2]).into_bytes();
    bytes.push(0xff);

    std::fs::remove_file(dir.join(AGENTS[0])).unwrap();
    std::os::unix::fs::symlink(&copy, dir.join(AGENTS[0])).unwrap();
    std::fs::remove_file(dir.join(AGENTS[1])).unwrap();
    std::os::unix::fs::symlink(&dangling, dir.join(AGENTS[1])).unwrap();
    std::fs::write(dir.join(AGENTS[2]), &bytes).unwrap();

    let (stdout, stderr) = run_init(dir);
    for rel in [AGENTS[0], AGENTS[1], AGENTS[2]] {
        assert!(
            stderr.lines().any(|l| l == kept_warning(rel)),
            "{rel}: {stderr}"
        );
    }
    assert!(!stdout.contains("already set up"), "{stdout}");
    assert!(std::fs::symlink_metadata(dir.join(AGENTS[0]))
        .unwrap()
        .is_symlink());
    assert!(std::fs::symlink_metadata(dir.join(AGENTS[1]))
        .unwrap()
        .is_symlink());
    assert!(!dangling.exists(), "written through a dangling symlink");
    assert_eq!(std::fs::read(dir.join(AGENTS[2])).unwrap(), bytes);
}

// AC-9: an unchanged file of a greater version is kept with a warning; a version that is smaller
// by its numbers (0.0.10 against 0.1.x) is updated.
#[test]
fn keeps_a_newer_file() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    make_version(dir, AGENTS[0], "999.0.0");
    make_version(dir, AGENTS[1], "0.0.10");
    let newer = read(dir, AGENTS[0]);

    let (stdout, stderr) = run_init(dir);
    let own = env!("CARGO_PKG_VERSION");
    let warning = format!(
        "warning: {} was written by stapel 999.0.0, newer than this stapel {own}; kept",
        AGENTS[0]
    );
    assert!(stderr.lines().any(|l| l == warning), "{stderr}");
    assert_eq!(read(dir, AGENTS[0]), newer);
    assert!(
        stdout
            .lines()
            .any(|l| l == format!("updated: {}", AGENTS[1])),
        "{stdout}"
    );
    assert!(!stdout.contains("already set up"), "{stdout}");
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

const SETTINGS: &str = ".claude/settings.json";
const SESSION_LINE: &str =
    "command -v stapel >/dev/null 2>&1 || exit 0; stapel hook session-start 2>/dev/null || exit 0";

fn settings_json(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&read(dir, SETTINGS)).unwrap()
}

fn init_in(dir: &Path) -> String {
    let out = stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

// AC-12: a fresh install adds the SessionStart entry; a second run adds none.
#[test]
fn installs_the_session_hook() {
    let repo = git_repo();
    let dir = repo.path();
    init_in(dir);

    let s = settings_json(dir);
    let entries = s["hooks"]["SessionStart"].as_array().expect("SessionStart");
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0]["matcher"], "startup|clear|compact");
    let hooks = entries[0]["hooks"].as_array().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0]["type"], "command");
    assert_eq!(hooks[0]["command"], SESSION_LINE);
    assert_eq!(hooks[0]["timeout"], 10);

    let before = read(dir, SETTINGS);
    init_in(dir);
    assert_eq!(read(dir, SETTINGS), before);
    assert_eq!(
        settings_json(dir)["hooks"]["SessionStart"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

// AC-12: a foreign SessionStart entry stays; the session hook alone gets its own label; a
// SessionStart that is not an array refuses and leaves the file as it was.
#[test]
fn session_hook_next_to_others() {
    let repo = git_repo();
    let dir = repo.path();
    init_in(dir);

    let mut s = settings_json(dir);
    let foreign = serde_json::json!({
        "matcher": "startup",
        "hooks": [{ "type": "command", "command": "echo foreign" }]
    });
    s["hooks"]["SessionStart"] = serde_json::json!([foreign]);
    s["theme"] = serde_json::json!("dark");
    std::fs::write(
        dir.join(SETTINGS),
        serde_json::to_string_pretty(&s).unwrap(),
    )
    .unwrap();

    let stdout = init_in(dir);
    assert!(
        stdout
            .lines()
            .any(|l| l == "appended: .claude/settings.json (stapel session hook)"),
        "{stdout}"
    );
    let after = settings_json(dir);
    let entries = after["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "{entries:?}");
    assert_eq!(entries[0], foreign);
    assert_eq!(entries[1]["hooks"][0]["command"], SESSION_LINE);
    assert_eq!(after["theme"], "dark");
    assert_eq!(after["hooks"]["PreToolUse"], s["hooks"]["PreToolUse"]);
    assert_eq!(after["hooks"]["PostToolUse"], s["hooks"]["PostToolUse"]);

    let mut bad = after.clone();
    bad["hooks"]["SessionStart"] = serde_json::json!({ "matcher": "x", "hooks": [] });
    let start = serde_json::to_string_pretty(&bad).unwrap();
    std::fs::write(dir.join(SETTINGS), &start).unwrap();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .code(1)
        .stderr(contains(
            "stapel: .claude/settings.json: the hooks.SessionStart field must be an array",
        ));
    assert_eq!(read(dir, SETTINGS), start);
}

// AC-14: the installed session line prints nothing and exits 0 when `stapel` is not on PATH.
#[test]
fn session_hook_line_without_stapel() {
    let repo = git_repo();
    let dir = repo.path();
    init_in(dir);

    let s = settings_json(dir);
    let line = s["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .expect("the installed session line")
        .to_string();
    let empty = tempfile::tempdir().unwrap();
    let out = std::process::Command::new("/bin/sh")
        .args(["-c", &line])
        .env("PATH", empty.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out.status);
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
}

/// The body of a process file: the text after its frontmatter, whitespace collapsed to single spaces.
fn body_of(dir: &Path, rel: &str) -> String {
    let text = read(dir, rel);
    let fm = frontmatter(&text);
    let rest: Vec<&str> = text.lines().skip(fm.len()).collect();
    rest.join("\n")
}

fn collapsed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// AC-15, AC-16: the skill body has the four headings, nine numbered rules under the spec process and
// the two exact sentences.
#[test]
fn skill_text_holds_the_process() {
    let repo = git_repo();
    let dir = repo.path();
    init_in(dir);
    let body = body_of(dir, SKILL);

    for heading in [
        "## The loop",
        "## How work is done",
        "## The spec process",
        "## Build and review",
    ] {
        assert!(body.lines().any(|l| l == heading), "no heading {heading:?}");
    }

    let mut numbers = Vec::new();
    let mut inside = false;
    for line in body.lines() {
        if line.starts_with("## ") {
            inside = line == "## The spec process";
        } else if inside {
            if let Some((n, _)) = line.split_once(". ") {
                if let Ok(n) = n.parse::<u32>() {
                    numbers.push(n);
                }
            }
        }
    }
    assert_eq!(numbers, (1..=9).collect::<Vec<u32>>());

    let flat = collapsed(&body);
    for sentence in [
        "Before the spec review, run stapel brief on every Plan step and split the steps it refuses.",
        "Spec changes inside the promise during the build are collected and confirmed by one stapel ok before code review; a change that widens the promise or changes a criterion's meaning stops the build until it is confirmed.",
    ] {
        assert!(flat.contains(sentence), "missing {sentence:?}");
    }
}

// AC-17: each agent body holds the role lines.
#[test]
fn agent_texts_hold_their_rules() {
    let repo = git_repo();
    let dir = repo.path();
    init_in(dir);
    let roles: [(&str, &[&str]); 4] = [
        (
            AGENTS[0],
            &[
                "stapel brief",
                "GREEN never changes files under tests/",
                "Write files with Edit or Write, never with sed, cat or a redirection",
                "Never git push",
                "never switch branches",
                "never run stapel ok or stapel close",
            ],
        ),
        (AGENTS[1], &["only the step's diff and its criteria"]),
        (
            AGENTS[2],
            &[
                "You work on a copy",
                "run no builds",
                "catchable_at",
                "deletion check",
                "claims check",
                "verification gap",
            ],
        ),
        (
            AGENTS[3],
            &[
                "first read only the Frame and the criteria",
                "Clear, Partial or Missing",
                "inside",
                "outside",
            ],
        ),
    ];
    for (rel, phrases) in roles {
        let flat = collapsed(&body_of(dir, rel));
        for phrase in phrases {
            assert!(flat.contains(phrase), "{rel}: missing {phrase:?}");
        }
    }
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

// AC-18: a new repository with this repository's config gets process files equal to this repository's.
#[test]
fn this_repository_has_the_current_files() {
    let root = repo_root();
    let repo = git_repo();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".stapel")).unwrap();
    std::fs::copy(
        root.join(".stapel/stapel.toml"),
        dir.join(".stapel/stapel.toml"),
    )
    .unwrap();
    stapel(dir).arg("init").assert().success();
    for rel in std::iter::once(SKILL).chain(AGENTS) {
        let ours = std::fs::read(root.join(rel))
            .unwrap_or_else(|e| panic!("this repository has no {rel}: {e}"));
        let fresh = std::fs::read(dir.join(rel)).unwrap();
        assert!(ours == fresh, "{rel} differs from what init writes");
    }
}

// AC-19: CLAUDE.md keeps only its own rules and names the skill.
#[test]
fn claude_md_names_the_skill() {
    let text = std::fs::read_to_string(repo_root().join("CLAUDE.md")).unwrap();
    assert!(!text.lines().any(|l| l == "## The spec process"));
    assert!(!text.lines().any(|l| l == "## Build and review"));
    assert!(text.contains(".claude/skills/stapel/SKILL.md"));
}

// AC-2, D1-1: the marker of every written file holds the version of the binary.
#[test]
fn marker_holds_the_binary_version() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    for rel in std::iter::once(SKILL).chain(AGENTS) {
        assert_eq!(check_marker(dir, rel), env!("CARGO_PKG_VERSION"), "{rel}");
    }
}

// AC-3, D1-2: `plan` on the reviewer binding reaches both reviewer files.
#[test]
fn reviewer_mode_reaches_both_reviewers() {
    let repo = repo_with_config(|t| {
        replace_line(
            t,
            "reviewer",
            "reviewer = { provider = \"claude-code\", model = \"claude-sonnet-5-5\", permission_mode = \"plan\" }",
        )
    });
    let dir = repo.path();
    stapel(dir).arg("init").assert().success();
    for rel in [AGENTS[2], AGENTS[3]] {
        assert_eq!(
            line_of(dir, rel, "permissionMode:").as_deref(),
            Some("permissionMode: plan"),
            "{rel}"
        );
    }
}

// AC-4, AC-5, D2-1: the refusals hold on `drift` and on `reviewer`, not only on `builder`.
#[test]
fn refusals_on_every_checked_role() {
    for role in ["drift", "reviewer"] {
        refused_binding(
            role,
            "{ provider = \"claude-code\", model = \"claude-sonnet-5-5\", permission_mode = \"bypassPermissions\" }",
            &format!(
                "stapel: [models] {role}: permission_mode bypassPermissions is not allowed: it turns off the permission dialog for every call the guard lets through"
            ),
        );
        refused_binding(
            role,
            "{ provider = \"claude-code\", model = \"claude-sonnet-5-5\", permission_mode = \"yolo\" }",
            &format!(
                "stapel: [models] {role}: permission_mode \"yolo\" is not one of default, acceptEdits, auto, dontAsk, plan, manual"
            ),
        );
        refused_binding(
            role,
            "{ provider = \"claude-code\", model = \"x#y\" }",
            &format!("stapel: [models] {role}: model \"x#y\" cannot be written to an agent file"),
        );
    }
}

// AC-10, D2-2, D2-3: a directory at the first or the last process path refuses and writes nothing.
#[test]
fn refuses_a_directory_at_any_process_path() {
    for rel in [SKILL, AGENTS[3]] {
        let repo = git_repo();
        let dir = repo.path();
        std::fs::create_dir_all(dir.join(rel)).unwrap();
        let before = snapshot(dir);
        stapel(dir)
            .args(["init", "--prefix", "ABC"])
            .assert()
            .code(1)
            .stderr(contains(format!(
                "stapel: {rel} is a directory; nothing was written"
            )));
        assert_eq!(snapshot(dir), before, "{rel}");
    }
}

// AC-11 (D3-4): with one of the two folders missing, the note is the last line, after `created:`.
#[test]
fn restart_note_for_one_missing_folder() {
    let note = "note: restart Claude Code so that it loads the new agents and skill";
    for (missing, created) in [(".claude/agents", AGENTS[0]), (".claude/skills", SKILL)] {
        let repo = git_repo();
        let dir = repo.path();
        stapel(dir)
            .args(["init", "--prefix", "ABC"])
            .assert()
            .success();
        std::fs::remove_dir_all(dir.join(missing)).unwrap();

        let out = stapel(dir).arg("init").assert().success();
        let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
        assert_eq!(stdout.lines().last(), Some(note), "{missing}: {stdout}");
        let at = stdout
            .lines()
            .position(|l| l == format!("created: {created}"))
            .unwrap_or_else(|| panic!("{missing}: {stdout}"));
        assert!(at + 1 < stdout.lines().count(), "{missing}: {stdout}");
    }
}

// AC-7 (D3-5): the hash in the marker is the SHA-256 the test computes by hand.
#[test]
fn hash_computed_by_hand() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let want = read(dir, AGENTS[0]);
    let text = read(dir, AGENTS[0]);
    let marker = text
        .lines()
        .find(|l| l.starts_with("# stapel:"))
        .expect("marker")
        .to_string();
    let mut without = text.replacen(&format!("{marker}\n"), "", 1);
    without.push_str("Another hand-made line.\n");
    let mut hex = String::new();
    for b in Sha256::digest(without.as_bytes()) {
        hex.push_str(&format!("{b:02x}"));
    }
    let at = without.find("\n---\n").expect("frontmatter end") + 1;
    without.insert_str(at, &format!("# stapel: 0.0.1 sha256:{hex}\n"));
    std::fs::write(dir.join(AGENTS[0]), without).unwrap();

    let out = stapel(dir).arg("init").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        stdout
            .lines()
            .any(|l| l == format!("updated: {}", AGENTS[0])),
        "{stdout}"
    );
    assert!(!stderr.contains("warning"), "{stderr}");
    assert_eq!(read(dir, AGENTS[0]), want);
}

// AC-9 (D3B-2): one unchanged newer file alone gives the newer warning and no `already set up`.
#[test]
fn newer_file_alone() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let text = read(dir, AGENTS[2]);
    let marker = text
        .lines()
        .find(|l| l.starts_with("# stapel:"))
        .expect("marker")
        .to_string();
    let mut without = text.replacen(&format!("{marker}\n"), "", 1);
    without.push_str("A hand-made extra line.\n");
    let mut hex = String::new();
    for b in Sha256::digest(without.as_bytes()) {
        hex.push_str(&format!("{b:02x}"));
    }
    let at = without.find("\n---\n").expect("frontmatter end") + 1;
    without.insert_str(at, &format!("# stapel: 999.0.0 sha256:{hex}\n"));
    std::fs::write(dir.join(AGENTS[2]), &without).unwrap();

    let out = stapel(dir).arg("init").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    let own = env!("CARGO_PKG_VERSION");
    let warning = format!(
        "warning: {} was written by stapel 999.0.0, newer than this stapel {own}; kept",
        AGENTS[2]
    );
    assert!(stderr.lines().any(|l| l == warning), "{stderr}");
    assert!(!stdout.contains("already set up"), "{stdout}");
    assert_eq!(read(dir, AGENTS[2]), without);
}

// AC-9 (D3B-3): the version compares by numbers: 0.09.0 is newer than 0.1.x, though older as text.
#[test]
fn versions_compare_by_numbers() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let text = read(dir, AGENTS[1]);
    let marker = text
        .lines()
        .find(|l| l.starts_with("# stapel:"))
        .expect("marker")
        .to_string();
    let mut without = text.replacen(&format!("{marker}\n"), "", 1);
    without.push_str("A hand-made extra line.\n");
    let mut hex = String::new();
    for b in Sha256::digest(without.as_bytes()) {
        hex.push_str(&format!("{b:02x}"));
    }
    let at = without.find("\n---\n").expect("frontmatter end") + 1;
    without.insert_str(at, &format!("# stapel: 0.09.0 sha256:{hex}\n"));
    std::fs::write(dir.join(AGENTS[1]), &without).unwrap();

    let out = stapel(dir).arg("init").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    let own = env!("CARGO_PKG_VERSION");
    let head = format!("warning: {} was written by stapel ", AGENTS[1]);
    let tail = format!(", newer than this stapel {own}; kept");
    assert!(
        stderr
            .lines()
            .any(|l| l.starts_with(&head) && l.ends_with(&tail)),
        "{stderr}"
    );
    assert!(!stdout.contains("updated:"), "{stdout}");
    assert_eq!(read(dir, AGENTS[1]), without);
}

// AC-8 (D3B-4): a version greater than the binary's with a hash that does not match gets the AC-8
// warning, not the newer one.
#[test]
fn newer_file_with_a_wrong_hash() {
    let repo = git_repo();
    let dir = repo.path();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    let text = read(dir, AGENTS[3]);
    let marker = text
        .lines()
        .find(|l| l.starts_with("# stapel:"))
        .expect("marker")
        .to_string();
    let wrong = format!("# stapel: 999.0.0 sha256:{}", "0".repeat(64));
    let edited = text.replacen(&marker, &wrong, 1);
    std::fs::write(dir.join(AGENTS[3]), &edited).unwrap();

    let out = stapel(dir).arg("init").assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    let kept = format!(
        "warning: {} is not the file stapel wrote; kept. Delete it and run stapel init to take stapel's text",
        AGENTS[3]
    );
    assert!(stderr.lines().any(|l| l == kept), "{stderr}");
    assert!(!stderr.contains("newer than"), "{stderr}");
    assert!(!stdout.contains("already set up"), "{stdout}");
    assert_eq!(read(dir, AGENTS[3]), edited);
}

// AC-12 (D4-2): a foreign SessionStart command that contains the session command counts as installed.
#[test]
fn session_hook_inside_a_wrapper() {
    let repo = git_repo();
    let dir = repo.path();
    init_in(dir);

    let mut s = settings_json(dir);
    s["hooks"]["SessionStart"] = serde_json::json!([{
        "matcher": "startup",
        "hooks": [{ "type": "command", "command": "my-wrapper; stapel hook session-start" }]
    }]);
    let text = serde_json::to_string_pretty(&s).unwrap() + "\n";
    std::fs::write(dir.join(SETTINGS), &text).unwrap();

    let stdout = init_in(dir);
    assert!(!stdout.contains("session hook"), "{stdout}");
    assert!(!stdout.contains(".claude/settings.json"), "{stdout}");
    assert_eq!(read(dir, SETTINGS), text);
}

// AC-12 (D4-3): the session hook label is only for a run that changes nothing else in the file.
#[test]
fn session_hook_labels_with_other_changes() {
    let repo = git_repo();
    let dir = repo.path();
    let stdout = init_in(dir);
    assert!(
        stdout
            .lines()
            .any(|l| l == "created: .claude/settings.json"),
        "{stdout}"
    );

    // The bare PreToolUse line of an earlier version, and no SessionStart at all.
    let mut s = settings_json(dir);
    s["hooks"]["PreToolUse"][0]["hooks"][0]["command"] =
        serde_json::json!("stapel hook pre-tool-use");
    s["hooks"].as_object_mut().unwrap().remove("SessionStart");
    std::fs::write(
        dir.join(SETTINGS),
        serde_json::to_string_pretty(&s).unwrap(),
    )
    .unwrap();

    let stdout = init_in(dir);
    assert!(
        stdout
            .lines()
            .any(|l| l == "updated: .claude/settings.json (stapel hook line)"),
        "{stdout}"
    );
    assert!(!stdout.contains("session hook"), "{stdout}");
    let after = settings_json(dir);
    assert_eq!(
        after["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        SESSION_LINE
    );
    assert_ne!(
        after["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        "stapel hook pre-tool-use"
    );
}

// AC-12 (D4-4, D4-6): a foreign entry in each of the four events stays in place; the local settings
// file is not touched.
#[test]
fn session_hook_keeps_every_event() {
    let repo = git_repo();
    let dir = repo.path();
    init_in(dir);

    let events = [
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "SessionStart",
    ];
    let mut s = settings_json(dir);
    let mut foreign = Vec::new();
    for event in events {
        let entry = serde_json::json!({
            "matcher": "Bash",
            "hooks": [{ "type": "command", "command": format!("echo foreign {event}") }]
        });
        s["hooks"][event]
            .as_array_mut()
            .unwrap()
            .insert(0, entry.clone());
        foreign.push(entry);
    }
    // The stapel session entry is missing, so the run has one thing to add.
    s["hooks"]["SessionStart"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    std::fs::write(
        dir.join(SETTINGS),
        serde_json::to_string_pretty(&s).unwrap(),
    )
    .unwrap();
    let local = b"{ \"permissions\": {\"allow\": [ \"Bash(ls)\" ]},\n  \"x\":1 }";
    std::fs::write(dir.join(".claude/settings.local.json"), local).unwrap();

    let stdout = init_in(dir);
    assert!(
        stdout
            .lines()
            .any(|l| l == "appended: .claude/settings.json (stapel session hook)"),
        "{stdout}"
    );
    let after = settings_json(dir);
    for (event, entry) in events.iter().zip(&foreign) {
        assert_eq!(&after["hooks"][event][0], entry, "{event}");
    }
    assert_eq!(after["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    assert_eq!(after["hooks"]["PostToolUse"].as_array().unwrap().len(), 2);
    assert_eq!(
        after["hooks"]["PostToolUseFailure"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let session = after["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(session.len(), 2, "{session:?}");
    assert_eq!(session[1]["hooks"][0]["command"], SESSION_LINE);
    assert_eq!(
        std::fs::read(dir.join(".claude/settings.local.json")).unwrap(),
        local
    );
}

// AC-14 (D4-7): the installed line hides a failing stapel: empty stdout and stderr, exit 0.
#[test]
fn session_hook_line_with_a_failing_stapel() {
    use std::os::unix::fs::PermissionsExt;

    let repo = git_repo();
    let dir = repo.path();
    init_in(dir);
    let line = settings_json(dir)["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .expect("the installed session line")
        .to_string();

    let bin = tempfile::tempdir().unwrap();
    let fake = bin.path().join("stapel");
    std::fs::write(&fake, "#!/bin/sh\necho boom >&2\nexit 3\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

    let out = std::process::Command::new("/bin/sh")
        .args(["-c", &line])
        .env("PATH", bin.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{:?}", out.status);
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
}

// AC-17 (D5-5): the three read-only roles say they have no write access and run no builds.
#[test]
fn read_only_agents_say_so() {
    let repo = git_repo();
    let dir = repo.path();
    init_in(dir);
    for rel in [AGENTS[1], AGENTS[2], AGENTS[3]] {
        let flat = collapsed(&body_of(dir, rel));
        for phrase in ["no write access", "run no builds"] {
            assert!(flat.contains(phrase), "{rel}: missing {phrase:?}");
        }
    }
}
