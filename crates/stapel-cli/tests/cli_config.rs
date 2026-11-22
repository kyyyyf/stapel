//! STP-2 AC-13: every command refuses an invalid stapel.toml.

mod common;

use common::{repo_with_ticket, stapel};
use predicates::str::contains;

#[test]
fn bad_config_refused_by_every_command() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let path = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("id = \"design\"", "id = \"spec\"");
    std::fs::write(&path, text).unwrap();
    for args in [
        vec!["new", "x"],
        vec!["ok", "spec"],
        vec!["close", "--reason", "x"],
        vec!["status"],
    ] {
        stapel(dir)
            .args(&args)
            .assert()
            .code(1)
            .stderr(contains("duplicate"));
    }
}

// STP-36 AC-7: `[process] dialogs` is `all` (the default) or `close`; another word is a load error.
#[test]
fn process_dialogs() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let path = dir.join(".stapel/stapel.toml");
    let base = std::fs::read_to_string(&path).unwrap();
    for value in [None, Some("all"), Some("close")] {
        let text = match value {
            None => base.clone(),
            Some(v) => format!("{base}\n[process]\ndialogs = \"{v}\"\n"),
        };
        std::fs::write(&path, text).unwrap();
        stapel(dir).arg("status").assert().success();
    }
    for value in ["Close", "none", ""] {
        std::fs::write(&path, format!("{base}\n[process]\ndialogs = \"{value}\"\n")).unwrap();
        stapel(dir)
            .arg("status")
            .assert()
            .code(1)
            .stderr(contains(format!(
                "process.dialogs must be all or close: {value}"
            )));
    }
}
