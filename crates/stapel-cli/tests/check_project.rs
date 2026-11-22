//! STP-37: the project commands of `stapel check`.

mod common;

use common::{read, stapel, stapel_repo};
use std::path::Path;

/// Adds a `[check]` section with the lines to the repository's `stapel.toml`.
fn add_to_check(dir: &Path, lines: &str) {
    let text = format!(
        "{}\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n{lines}\n",
        read(dir, ".stapel/stapel.toml")
    );
    std::fs::write(dir.join(".stapel/stapel.toml"), text).unwrap();
}

/// Exit code and the text of stderr of `stapel check`.
fn check(dir: &Path) -> (i32, String) {
    let out = stapel(dir).arg("check").output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// AC-1: an empty or whitespace entry of `project` fails the load; a list of two commands loads.
#[test]
fn refuses_an_empty_command() {
    let refusal = "check.project entries must not be empty";
    for bad in [
        "project = [\"\"]",
        "project = [\"  \"]",
        "project = [\"true\", \"\"]",
    ] {
        let repo = stapel_repo();
        add_to_check(repo.path(), bad);
        let (code, err) = check(repo.path());
        assert_eq!(code, 2, "{bad}: {err}");
        assert!(err.contains(refusal), "{bad}: {err}");
    }
    let repo = stapel_repo();
    add_to_check(repo.path(), "project = [\"true\", \"echo two\"]");
    let (_, err) = check(repo.path());
    assert!(!err.contains("stapel.toml"), "{err}");
    assert!(!err.contains(refusal), "{err}");
}
