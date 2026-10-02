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
