//! STP-3 AC-7: fixtures, golden files and test sources carry no private data.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in [
        "crates/stapel-cli/tests/fixtures",
        "crates/stapel-cli/tests/golden",
    ] {
        if let Ok(entries) = std::fs::read_dir(root().join(dir)) {
            out.extend(entries.flatten().map(|e| e.path()));
        }
    }
    for krate in ["stapel-core", "stapel-cli"] {
        for e in std::fs::read_dir(root().join("crates").join(krate).join("tests"))
            .unwrap()
            .flatten()
        {
            let p = e.path();
            // This file spells the patterns it looks for.
            if p.extension().is_some_and(|x| x == "rs")
                && p.file_name().is_some_and(|n| n != "fixtures.rs")
            {
                out.push(p);
            }
        }
    }
    out
}

/// An e-mail address: `@`, a name, a dot and letters.
fn has_email(text: &str) -> bool {
    text.match_indices('@').any(|(i, _)| {
        let after = &text[i + 1..];
        let host: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '.')
            .collect();
        let parts: Vec<&str> = host.split('.').collect();
        parts.len() >= 2
            && !parts[0].is_empty()
            && parts
                .last()
                .is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
    })
}

/// Terms from the untracked `.stapel/config/private.toml`: `terms = ["...", ...]`.
fn private_terms() -> Option<Vec<String>> {
    let text = std::fs::read_to_string(root().join(".stapel/config/private.toml")).ok()?;
    let v: toml::Value = toml::from_str(&text).ok()?;
    Some(
        v.get("terms")?
            .as_array()?
            .iter()
            .filter_map(|t| t.as_str().map(str::to_lowercase))
            .collect(),
    )
}

fn has_word(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + term.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric()) && !after.is_some_and(|c| c.is_alphanumeric())
    })
}

#[test]
fn contain_no_private_data() {
    let terms = private_terms();
    if terms.is_none() {
        eprintln!(
            "notice: .stapel/config/private.toml not found; checking paths and e-mail addresses only"
        );
    }
    let mut problems = Vec::new();
    for path in files() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let lower = text.to_lowercase();
        let rel = path
            .strip_prefix(root())
            .unwrap_or(&path)
            .display()
            .to_string();
        for pattern in ["/home/", "/users/", "c:\\users\\"] {
            if lower.contains(pattern) {
                problems.push(format!("{rel} contains {pattern}"));
            }
        }
        if has_email(&text) {
            problems.push(format!("{rel} contains an e-mail address"));
        }
        for term in terms.iter().flatten() {
            if has_word(&lower, term) {
                problems.push(format!("{rel} contains a private term"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "private data:\n{}",
        problems.join("\n")
    );
}

#[test]
fn checker_finds_private_data() {
    assert!(has_email("write to someone@example.org today"));
    assert!(!has_email("version@2 and @sign"));
    assert!(has_word("the user ek ran it", "ek"));
    assert!(!has_word("check the code", "ek"));
}
