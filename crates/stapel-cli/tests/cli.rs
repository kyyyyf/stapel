//! STP-1 AC-1: the workspace layout and the `stapel` binary.

use assert_cmd::Command;
use std::path::Path;

const MEMBERS: [&str; 7] = [
    "stapel-core",
    "stapel-agent",
    "stapel-index",
    "stapel-forge",
    "stapel-mcp",
    "stapel-lsp",
    "stapel-cli",
];

#[test]
fn workspace_has_seven_members() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&root)
        .output()
        .expect("cargo metadata runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
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
