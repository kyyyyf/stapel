//! STP-1 AC-1: the workspace layout and the `stapel` binary.

use assert_cmd::Command;
use std::path::Path;

// STP-44 adds the grammar wrapper crate.
const MEMBERS: [&str; 6] = [
    "stapel-core",
    "stapel-index",
    "stapel-mcp",
    "stapel-lsp",
    "stapel-cli",
    "stapel-grammar-rust",
];

// STP-39 AC-15: the two empty crates are removed.
#[test]
fn no_empty_crates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["agent", "forge"] {
        assert!(
            !root.join(format!("crates/stapel-{name}")).exists(),
            "crates/stapel-{name} still exists"
        );
    }
}

#[test]
fn workspace_has_seven_members() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&root)
        .output()
        .expect("cargo metadata runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut names: Vec<&str> = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    names.sort();
    let mut expected = MEMBERS.to_vec();
    expected.sort();
    assert_eq!(names, expected);
}

#[test]
fn version_prints_package_version() {
    Command::cargo_bin("stapel")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("stapel {}\n", env!("CARGO_PKG_VERSION")));
}

// AC-1: the five packages beyond core and cli are stubs with a doc comment only.
#[test]
fn stub_packages_are_empty() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // STP-44 fills stapel-index.
    for name in ["mcp"] {
        let lib = root.join(format!("crates/stapel-{name}/src/lib.rs"));
        let text = std::fs::read_to_string(&lib).unwrap();
        assert!(
            text.lines()
                .all(|l| l.trim().is_empty() || l.starts_with("//")),
            "{} has code",
            lib.display()
        );
    }
}
