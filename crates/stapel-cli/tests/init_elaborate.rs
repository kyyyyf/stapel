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

// AC-4: a directory at a file path, or a file or a symlink at the folder path, stops init with
// exit 1 and writes nothing.
#[test]
fn refuses_a_directory() {
    for rel in FILES {
        let repo = git_repo();
        let dir = repo.path();
        std::fs::create_dir_all(dir.join(rel)).unwrap();
        let out = stapel(dir).args(["init", "--prefix", "ABC"]).assert().code(1);
        let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
        assert!(
            stderr.contains(&format!("{rel} is a directory; nothing was written")),
            "{rel}: {stderr}"
        );
        for other in FILES.iter().filter(|f| **f != rel) {
            assert!(!dir.join(other).exists(), "{other} was written");
        }
        assert!(!dir.join(".stapel/stapel.toml").exists());
    }
    for kind in ["file", "symlink"] {
        let repo = git_repo();
        let dir = repo.path();
        std::fs::create_dir_all(dir.join(".claude/skills")).unwrap();
        let outside = tempfile::tempdir().unwrap();
        if kind == "file" {
            std::fs::write(dir.join(DIR), "x").unwrap();
        } else {
            std::os::unix::fs::symlink(outside.path(), dir.join(DIR)).unwrap();
        }
        let out = stapel(dir).args(["init", "--prefix", "ABC"]).assert().code(1);
        let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
        assert!(
            stderr.contains(&format!("{DIR} is not a directory; nothing was written")),
            "{kind}: {stderr}"
        );
        assert!(!dir.join(".stapel/stapel.toml").exists(), "{kind}");
        assert!(!dir.join(".claude/skills/stapel/SKILL.md").exists(), "{kind}");
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0, "{kind}");
    }
}

// AC-5: the local checklist adds the folder of the skill with `git add -f`.
#[test]
fn local_checklist_names_the_folder() {
    let repo = git_repo();
    let dir = repo.path();
    let out = stapel(dir)
        .args(["init", "--local", "--prefix", "ABC"])
        .assert()
        .success()
        .get_output()
        .clone();
    let output = String::from_utf8(out.stdout).unwrap() + &String::from_utf8(out.stderr).unwrap();
    let want = "   git add -f .claude/skills/stapel .claude/skills/stapel-elaborate .claude/agents/stapel-*.md .claude/settings.json";
    assert!(output.lines().any(|l| l == want), "no checklist line in {output}");
}

/// The text after the frontmatter of an installed skill file.
fn skill_body(dir: &Path, rel: &str) -> String {
    let text = read(dir, rel);
    let mut parts = text.splitn(3, "---\n");
    parts.next();
    parts.next();
    parts.next().expect("a body after the frontmatter").to_string()
}

// AC-6: the collapsed body of `SKILL.md` holds the eight phrases.
#[test]
fn skill_text_holds_the_steps() {
    let repo = git_repo();
    let dir = repo.path();
    first_run(dir);
    let flat = skill_body(dir, FILES[0])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for phrase in [
        "larger than one ticket",
        "elicit the person's intent, never author it",
        "at most four questions in one dialog, each with options and a recommendation",
        ".stapel/features/<slug>.md",
        "without the first line of template.md",
        "run checklist.md over the draft",
        "name the path of the draft in the chat",
        ".claude/skills/stapel-elaborate/",
    ] {
        assert!(flat.contains(phrase), "missing {phrase:?}");
    }
}

// AC-11: one line of the skill `stapel` names `stapel-elaborate` for work larger than one ticket.
#[test]
fn stapel_skill_names_it() {
    let repo = git_repo();
    let dir = repo.path();
    first_run(dir);
    let body = skill_body(dir, ".claude/skills/stapel/SKILL.md");
    let lines = body
        .lines()
        .filter(|l| l.contains("stapel-elaborate") && l.contains("larger than one ticket"))
        .count();
    assert_eq!(lines, 1, "{body}");
}

// AC-7: the headings of `template.md` in order, the example ids, the Breakdown words and the three
// guidance lines.
#[test]
fn template_sections() {
    let repo = git_repo();
    let dir = repo.path();
    first_run(dir);
    let text = read(dir, FILES[1]);
    let body = text.split_once("\n\n").expect("a blank line after the marker").1;
    assert_eq!(body.lines().next(), Some("# [Title of the feature]"), "{text}");
    let headings: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix("## ")).collect();
    assert_eq!(
        headings,
        [
            "Description",
            "Outcome",
            "Done when",
            "Boundaries",
            "Requirements",
            "Constraints",
            "Decisions",
            "Breakdown"
        ]
    );
    let section = |name: &str| -> String {
        let mut out = String::new();
        let mut inside = false;
        for l in text.lines() {
            if let Some(h) = l.strip_prefix("## ") {
                inside = h == name;
            } else if inside {
                out.push_str(l);
                out.push('\n');
            }
        }
        out
    };
    let reqs = section("Requirements");
    for id in ["R1", "R2"] {
        assert!(
            reqs.lines().any(|l| l.trim_start().starts_with(id)),
            "no line starting with {id} in {reqs}"
        );
    }
    let breakdown = section("Breakdown");
    assert!(breakdown.contains("covers") && breakdown.contains("after"), "{breakdown}");
    for phrase in [
        "three to six checks",
        "at least one thing out",
        "never reused or renumbered",
    ] {
        assert!(text.contains(phrase), "missing {phrase:?}");
    }
}

// AC-8: check lines `- EL-<n>:` with increasing ids, and a check in each of the four groups.
#[test]
fn checklist_ids() {
    let repo = git_repo();
    let dir = repo.path();
    first_run(dir);
    let text = read(dir, FILES[2]);
    let mut last = 0u64;
    let mut counts: Vec<(String, usize)> = Vec::new();
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            counts.push((h.to_string(), 0));
        } else if let Some(rest) = line.strip_prefix("- EL-") {
            let (n, _) = rest
                .split_once(':')
                .unwrap_or_else(|| panic!("no colon in {line:?}"));
            let n: u64 = n.parse().unwrap_or_else(|_| panic!("bad id in {line:?}"));
            assert!(n > last, "id {n} does not increase after {last}");
            last = n;
            counts
                .last_mut()
                .unwrap_or_else(|| panic!("check outside a group: {line:?}"))
                .1 += 1;
        }
    }
    for g in ["Draft", "Requirements", "Breakdown", "Dependencies"] {
        let found = counts.iter().find(|(h, _)| h == g);
        assert!(matches!(found, Some((_, c)) if *c >= 1), "group {g}: {counts:?}");
    }
}

// AC-9: the body of `NOTICE.md` equals the golden file (the asset keeps one blank line after the marker).
#[test]
fn notice_text() {
    let repo = git_repo();
    let dir = repo.path();
    first_run(dir);
    let body = unmarked(dir, FILES[3]);
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/elaborate_notice.md"),
    )
    .unwrap();
    // The whole LICENSE of the source, its trademark notice included (X1-2).
    assert!(golden.contains("\nTRADEMARK NOTICE:\n"), "{golden}");
    assert_eq!(body.strip_prefix('\n').unwrap_or(&body), golden);
}

// AC-10: the method's name is in `NOTICE.md` and in no other file `init` writes, in both modes.
#[test]
fn method_name_only_in_notice() {
    for local in [false, true] {
        let repo = git_repo();
        let dir = repo.path();
        let mut args = vec!["init", "--prefix", "ABC"];
        if local {
            args.push("--local");
        }
        stapel(dir).args(args).assert().success();
        let mut found: Vec<String> = Vec::new();
        for (rel, bytes, _) in common::snapshot(dir) {
            if String::from_utf8_lossy(&bytes).to_lowercase().contains("bmad") {
                found.push(rel);
            }
        }
        assert_eq!(found, [FILES[3]], "local={local}");
    }
}

// AC-13: the report has the three headings, and each item under Taken and Not taken has a `Why:` part.
#[test]
fn method_report() {
    let path = repo_root().join("docs/method-reports/e435a22.md");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut section = String::new();
    let mut items: Vec<(String, String)> = Vec::new();
    let mut headings: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            section = h.trim().to_string();
            headings.push(section.clone());
        } else if line.starts_with("- ") {
            items.push((section.clone(), line.to_string()));
        } else if line.starts_with("  ") && !line.trim().is_empty() {
            if let Some(last) = items.last_mut() {
                last.1.push(' ');
                last.1.push_str(line.trim());
            }
        }
    }
    for h in ["Taken", "Not taken", "Ideas for later"] {
        assert!(headings.iter().any(|x| x == h), "heading {h}: {headings:?}");
    }
    for s in ["Taken", "Not taken"] {
        let list: Vec<_> = items.iter().filter(|(sec, _)| sec == s).collect();
        assert!(!list.is_empty(), "no items under {s}");
        for (_, item) in list {
            assert!(item.contains("Why:"), "no `Why:` in {item}");
        }
    }
}
