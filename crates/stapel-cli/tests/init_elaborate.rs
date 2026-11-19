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

fn first_run(dir: &Path) {
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
}

fn rerun(dir: &Path) -> (String, String) {
    let out = stapel(dir).arg("init").assert().success();
    let out = out.get_output();
    (
        String::from_utf8(out.stdout.clone()).unwrap(),
        String::from_utf8(out.stderr.clone()).unwrap(),
    )
}

/// The text of `rel` without its marker, as the hash sees it.
fn unmarked(dir: &Path, rel: &str) -> String {
    let text = read(dir, rel);
    if rel.ends_with("SKILL.md") {
        let line = text.lines().find(|l| l.starts_with("# stapel:")).unwrap();
        text.replacen(&format!("{line}\n"), "", 1)
    } else {
        text.split_once('\n').unwrap().1.to_string()
    }
}

/// `body` marked with `version` and the hash of `body`, in the form of `rel`.
fn marked(rel: &str, version: &str, body: &str) -> String {
    let hash = hex_of(body);
    if rel.ends_with("SKILL.md") {
        let at = body.find("\n---\n").expect("frontmatter end") + 1;
        let mut text = body.to_string();
        text.insert_str(at, &format!("# stapel: {version} sha256:{hash}\n"));
        text
    } else {
        format!("<!-- stapel: {version} sha256:{hash} -->\n{body}")
    }
}

// AC-2: an older unchanged file with other text is updated; an equal file prints nothing.
#[test]
fn updates_an_unchanged_file() {
    let repo = git_repo();
    let dir = repo.path();
    first_run(dir);
    let want: Vec<String> = FILES.iter().map(|r| read(dir, r)).collect();
    for rel in FILES {
        let body = unmarked(dir, rel) + "A hand-made extra line.\n";
        std::fs::write(dir.join(rel), marked(rel, "0.0.1", &body)).unwrap();
    }
    let (stdout, _) = rerun(dir);
    for (rel, text) in FILES.iter().zip(&want) {
        assert!(
            stdout.lines().any(|l| l == format!("updated: {rel}")),
            "{rel}: {stdout}"
        );
        assert_eq!(&read(dir, rel), text, "{rel}");
    }
    let (stdout, stderr) = rerun(dir);
    for rel in FILES {
        assert!(!stdout.contains(rel), "{rel}: {stdout}");
        assert!(!stderr.contains(rel), "{rel}: {stderr}");
    }
}

const KEPT: &str =
    "is not the file stapel wrote; kept. Delete it and run stapel init to take stapel's text";

// AC-3: edited, unmarked, malformed, misplaced, symlinked and non-UTF-8 files are kept with the warning.
#[test]
fn keeps_an_edited_file() {
    let repo = git_repo();
    let dir = repo.path();
    first_run(dir);
    let body = unmarked(dir, FILES[1]);
    let hash = hex_of(&body);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (FILES[0], (read(dir, FILES[0]) + "one more line\n").into_bytes()),
        (FILES[1], format!("{body}one more line\n").into_bytes()),
        (FILES[1], body.clone().into_bytes()),
        (
            FILES[1],
            format!("<!-- stapel: 0.0.1 sha256:{} -->\n{body}", "0".repeat(64)).into_bytes(),
        ),
        (
            FILES[1],
            format!("<!-- stapel: 0.0.1 sha256:{} -->\n{body}", hash.to_uppercase()).into_bytes(),
        ),
        (
            FILES[1],
            format!("<!-- stapel: 0.0.1 sha256:{} -->\n{body}", &hash[..32]).into_bytes(),
        ),
        (
            FILES[1],
            format!("<!-- stapel: 1.2 sha256:{hash} -->\n{body}").into_bytes(),
        ),
        (
            FILES[1],
            format!("<!-- stapel: 0.0.1 sha256:{hash}\n{body}").into_bytes(),
        ),
        (
            FILES[1],
            format!("\n<!-- stapel: 0.0.1 sha256:{hash} -->\n{body}").into_bytes(),
        ),
        (FILES[1], {
            let mut b = marked(FILES[1], "0.0.1", &body).into_bytes();
            b.push(0xff);
            b
        }),
    ];
    let stapels: Vec<Vec<u8>> = FILES
        .iter()
        .map(|r| std::fs::read(dir.join(r)).unwrap())
        .collect();
    for (rel, bytes) in &cases {
        std::fs::write(dir.join(rel), bytes).unwrap();
        let (_, stderr) = rerun(dir);
        assert!(
            stderr
                .lines()
                .any(|l| l == format!("warning: {rel} {KEPT}")),
            "{rel}: {stderr}"
        );
        assert_eq!(&std::fs::read(dir.join(rel)).unwrap(), bytes, "{rel}");
        let i = FILES.iter().position(|f| f == rel).unwrap();
        std::fs::write(dir.join(rel), &stapels[i]).unwrap();
    }
    // The skill file with its marker removed.
    let bare = unmarked(dir, FILES[0]);
    std::fs::write(dir.join(FILES[0]), &bare).unwrap();
    let (_, stderr) = rerun(dir);
    assert!(
        stderr
            .lines()
            .any(|l| l == format!("warning: {} {KEPT}", FILES[0])),
        "{stderr}"
    );
    assert_eq!(read(dir, FILES[0]), bare);
    std::fs::write(dir.join(FILES[0]), &stapels[0]).unwrap();
    // A symlink to a copy of stapel's text.
    let outside = tempfile::tempdir().unwrap();
    let copy = outside.path().join("copy.md");
    std::fs::write(&copy, &stapels[2]).unwrap();
    std::fs::remove_file(dir.join(FILES[2])).unwrap();
    std::os::unix::fs::symlink(&copy, dir.join(FILES[2])).unwrap();
    let (_, stderr) = rerun(dir);
    assert!(
        stderr
            .lines()
            .any(|l| l == format!("warning: {} {KEPT}", FILES[2])),
        "{stderr}"
    );
    assert!(
        std::fs::symlink_metadata(dir.join(FILES[2]))
            .unwrap()
            .is_symlink()
    );
}

// AC-3: an unchanged file of a newer version is kept with the newer-version warning.
#[test]
fn keeps_a_newer_file() {
    let repo = git_repo();
    let dir = repo.path();
    first_run(dir);
    let own = env!("CARGO_PKG_VERSION");
    for rel in FILES {
        let body = unmarked(dir, rel) + "A hand-made extra line.\n";
        let newer = marked(rel, "99.0.0", &body);
        std::fs::write(dir.join(rel), &newer).unwrap();
        let (_, stderr) = rerun(dir);
        let warning = format!(
            "warning: {rel} was written by stapel 99.0.0, newer than this stapel {own}; kept"
        );
        assert!(stderr.lines().any(|l| l == warning), "{rel}: {stderr}");
        assert_eq!(read(dir, rel), newer, "{rel}");
    }
}
