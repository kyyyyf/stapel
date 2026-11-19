//! STP-34 step 1: `stapel init` writes the four files of the skill `stapel-elaborate`.

mod common;

use common::{git_repo, read, stapel};
use sha2::{Digest, Sha256};
use std::path::Path;

const DIR: &str = ".claude/skills/stapel-elaborate";
const FILES: [&str; 4] = [
    ".claude/skills/stapel-elaborate/SKILL.md",
    ".claude/skills/stapel-elaborate/template.md",
    ".claude/skills/stapel-elaborate/checklist.md",
    ".claude/skills/stapel-elaborate/NOTICE.md",
];

fn hex_of(bytes: &str) -> String {
    Sha256::digest(bytes.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn valid_version_and_hash(rel: &str, version: &str, hash: &str) {
    let parts: Vec<&str> = version.split('.').collect();
    assert_eq!(parts.len(), 3, "{rel}: version {version}");
    assert!(parts.iter().all(|p| p.parse::<u64>().is_ok()), "{rel}");
    assert_eq!(hash.len(), 64, "{rel}");
    assert!(
        hash.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        "{rel}"
    );
}

/// The first line must be `<!-- stapel: <version> sha256:<hex> -->` and the hash must be that of the
/// bytes after it.
fn check_line_marker(dir: &Path, rel: &str) {
    let text = read(dir, rel);
    let (first, rest) = text.split_once('\n').expect("a marker line");
    let inner = first
        .strip_prefix("<!-- stapel: ")
        .and_then(|s| s.strip_suffix(" -->"))
        .unwrap_or_else(|| panic!("{rel}: no line marker in {first:?}"));
    let (version, hash) = inner.split_once(" sha256:").expect("marker form");
    valid_version_and_hash(rel, version, hash);
    assert_eq!(hex_of(rest), hash, "{rel}: the hash does not match");
}

/// The frontmatter marker of `SKILL.md`, the form of the skill `stapel`.
fn check_frontmatter(dir: &Path, rel: &str) {
    let text = read(dir, rel);
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("---"), "{rel}");
    let mut fm = Vec::new();
    let mut closed = false;
    for line in lines {
        if line == "---" {
            closed = true;
            break;
        }
        fm.push(line);
    }
    assert!(closed, "{rel}: frontmatter is not closed");
    assert!(fm.contains(&"name: stapel-elaborate"), "{rel}: {fm:?}");
    let description = fm
        .iter()
        .find_map(|l| l.strip_prefix("description: "))
        .unwrap_or_else(|| panic!("{rel}: no description in {fm:?}"));
    assert!(
        description.len() > 2 && description.starts_with('"') && description.ends_with('"'),
        "{rel}: description is not quoted: {description:?}"
    );
    let markers: Vec<&&str> = fm.iter().filter(|l| l.starts_with("# stapel:")).collect();
    assert_eq!(markers.len(), 1, "{rel}: {fm:?}");
    let line = *markers[0];
    let rest = line.strip_prefix("# stapel: ").unwrap();
    let (version, hash) = rest.split_once(" sha256:").expect("marker form");
    valid_version_and_hash(rel, version, hash);
    let without = text.replacen(&format!("{line}\n"), "", 1);
    assert_eq!(hex_of(&without), hash, "{rel}: the hash does not match");
}

// AC-1: a fresh repository gets the four files, each with `created:`, a valid marker, and the
// frontmatter of `SKILL.md`.
#[test]
fn creates_the_files() {
    let repo = git_repo();
    let dir = repo.path();
    let out = stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success()
        .get_output()
        .clone();
    let output = String::from_utf8(out.stdout).unwrap() + &String::from_utf8(out.stderr).unwrap();
    for rel in FILES {
        assert!(
            output.lines().any(|l| l == format!("created: {rel}")),
            "no created line for {rel} in {output}"
        );
        assert!(dir.join(rel).is_file(), "{rel} is missing in {DIR}");
    }
    check_frontmatter(dir, FILES[0]);
    for rel in &FILES[1..] {
        check_line_marker(dir, rel);
    }
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

// AC-12: the files of this repository equal what `init` writes.
#[test]
fn this_repository_has_the_files() {
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
    for rel in FILES {
        let ours = std::fs::read(root.join(rel))
            .unwrap_or_else(|e| panic!("this repository has no {rel}: {e}"));
        let fresh = std::fs::read(dir.join(rel))
            .unwrap_or_else(|e| panic!("init wrote no {rel}: {e}"));
        assert!(ours == fresh, "{rel} differs from what init writes");
    }
}
