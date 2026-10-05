//! STP-3 AC-2..AC-4: `stapel tokens add`, `import` and the report.

mod common;

use common::{repo_with_ticket, stapel};
use predicates::str::contains;
use std::path::Path;

fn records(dir: &Path, key: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join(format!(".stapel/tickets/{key}/tokens.jsonl")))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn add_measured() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args([
            "tokens",
            "add",
            "--role",
            "builder",
            "--input",
            "10",
            "--output",
            "20",
            "--cache-read",
            "0",
            "--step",
            "s1",
        ])
        .assert()
        .success()
        .stdout(contains("recorded: t-"));
    let r = &records(dir, "ABC-1")[0];
    assert_eq!(r["v"], 1);
    assert_eq!(r["ticket"], "ABC-1");
    assert_eq!(r["role"], "builder");
    assert_eq!(r["model"], "claude-sonnet-5-5");
    assert_eq!(r["source"], "measured");
    assert_eq!(
        (
            r["input"].clone(),
            r["output"].clone(),
            r["cache_read"].clone()
        ),
        (10.into(), 20.into(), 0.into())
    );
    assert!(r.get("cache_write").is_none(), "{r}");
    assert!(r.get("estimate").is_none(), "{r}");
    assert_eq!(r["step"], "s1");
    assert!(r["id"].as_str().unwrap().starts_with("t-"));
}

#[test]
fn add_estimate() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args([
            "tokens",
            "add",
            "ABC-1",
            "--role",
            "author",
            "--estimate",
            "5000",
        ])
        .assert()
        .success();
    let r = &records(dir, "ABC-1")[0];
    assert_eq!(r["source"], "estimate");
    assert_eq!(r["estimate"], 5000);
    assert_eq!(r["model"], "claude-opus-5-5");
    assert!(r.get("input").is_none() && r.get("output").is_none(), "{r}");
}

#[test]
fn add_refuses_bad_input() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::create_dir_all(dir.join(".stapel/tickets/ABC-9")).unwrap();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-9/state.json"),
        r#"{"key":"ABC-9"}"#,
    )
    .unwrap();
    let long = "x".repeat(300);
    let note = "n".repeat(5000);
    let cases: Vec<Vec<&str>> = vec![
        vec!["--role", "builder", "--input", "-1", "--output", "2"],
        vec!["--role", "builder", "--input", "x", "--output", "2"],
        vec!["--role", "builder", "--input", "1.5", "--output", "2"],
        vec!["--role", "builder", "--input", "5"],
        vec!["--role", "builder", "--output", "5"],
        vec![
            "--role",
            "builder",
            "--input",
            "1",
            "--output",
            "1",
            "--estimate",
            "3",
        ],
        vec!["--role", "builder"],
        vec!["--role", "", "--estimate", "1"],
        vec!["--role", "a\nb", "--estimate", "1"],
        vec!["--role", "builder", "--model", &long, "--estimate", "1"],
        vec!["--role", "builder", "--step", &long, "--estimate", "1"],
        vec!["--role", "builder", "--note", &note, "--estimate", "1"],
        vec!["ABC-9", "--role", "builder", "--estimate", "1"],
        vec!["ABC-7", "--role", "builder", "--estimate", "1"],
    ];
    for case in cases {
        let mut args = vec!["tokens", "add"];
        args.extend(&case);
        stapel(dir).args(&args).assert().code(1);
    }
    assert!(records(dir, "ABC-1").is_empty());
}

#[test]
fn add_warns_on_unconfigured_role() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args([
            "tokens", "add", "--role", "research", "--model", "m-1", "--input", "1", "--output",
            "1",
        ])
        .assert()
        .success()
        .stderr(contains("warning: role research is not in stapel.toml"));
    stapel(dir)
        .args([
            "tokens", "add", "--role", "research", "--input", "1", "--output", "1",
        ])
        .assert()
        .code(1)
        .stderr(contains("--model"));
    assert_eq!(records(dir, "ABC-1").len(), 1);
}

#[test]
fn add_accepts_closed_ticket() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args(["close", "--reason", "done"])
        .assert()
        .success();
    stapel(dir)
        .args([
            "tokens", "add", "ABC-1", "--role", "reviewer", "--input", "3", "--output", "4",
        ])
        .assert()
        .success();
    assert_eq!(records(dir, "ABC-1").len(), 1);
}
