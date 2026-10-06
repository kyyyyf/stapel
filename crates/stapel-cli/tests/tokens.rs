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

// ---- AC-3: import from Claude Code transcripts ----

fn fixture(dir: &Path, name: &str) -> std::path::PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let dst = dir.join(name);
    std::fs::copy(src, &dst).unwrap();
    dst
}

/// Imports with a window that ends long before now, so the fresh copy counts as finished.
fn import(dir: &Path, file: &Path, since: Option<&str>, until: &str) -> assert_cmd::assert::Assert {
    let mut args = vec![
        "tokens".to_string(),
        "import".into(),
        file.display().to_string(),
        "--role".into(),
        "orchestrator".into(),
        "--until".into(),
        until.into(),
    ];
    if let Some(s) = since {
        args.push("--since".into());
        args.push(s.into());
    }
    stapel(dir).args(&args).assert()
}

fn by_model<'a>(rs: &'a [serde_json::Value], model: &str) -> Vec<&'a serde_json::Value> {
    rs.iter().filter(|r| r["model"] == model).collect()
}

#[test]
fn import_sums_usage_per_message() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    import(dir, &main, None, "2026-01-02T00:00:00Z").success();
    let rs = records(dir, "ABC-1");
    assert_eq!(rs.len(), 2, "{rs:?}");
    let opus = by_model(&rs, "claude-opus-5-5")[0];
    // msg_B counts with its last line: output 4618, not 2.
    assert_eq!(
        (opus["input"].clone(), opus["output"].clone()),
        (7.into(), 4631.into())
    );
    assert_eq!(
        (opus["cache_read"].clone(), opus["cache_write"].clone()),
        (600.into(), 30.into())
    );
    assert_eq!(opus["messages"], 3);
    assert_eq!(opus["source"], "measured");
    assert_eq!(opus["transcript"], "s-test");
    assert_eq!(opus["from"], "2026-01-01T10:00:00.100Z");
    assert_eq!(opus["to"], "2026-01-01T10:20:00.000Z");
    let haiku = by_model(&rs, "claude-haiku-4-5-20251001")[0];
    assert_eq!(
        (haiku["cache_read"].clone(), haiku["cache_write"].clone()),
        (0.into(), 0.into())
    );

    let sub = fixture(dir, "subagent.jsonl");
    import(dir, &sub, None, "2026-01-02T00:00:00Z").success();
    let rs = records(dir, "ABC-1");
    let sonnet = by_model(&rs, "claude-sonnet-5-5")[0];
    assert_eq!(sonnet["output"], 129);
    assert_eq!(sonnet["transcript"], "s-test/agent-test");
}

#[test]
fn import_skips_error_and_unreadable_lines() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    import(dir, &main, None, "2026-01-02T00:00:00Z")
        .success()
        .stdout(contains("skipped: 1 error lines"))
        .stdout(contains("skipped: 1 unreadable lines"));
    assert!(by_model(&records(dir, "ABC-1"), "<synthetic>").is_empty());
}

#[test]
fn import_is_idempotent() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    import(dir, &main, None, "2026-01-02T00:00:00Z").success();
    import(dir, &main, None, "2026-01-02T00:00:00Z")
        .success()
        .stdout(contains("already imported: t-"));
    assert_eq!(records(dir, "ABC-1").len(), 2);
}

#[test]
fn import_refuses_overlap() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    import(
        dir,
        &main,
        Some("2026-01-01T10:00:00Z"),
        "2026-01-01T10:10:00Z",
    )
    .success();
    assert_eq!(records(dir, "ABC-1").len(), 1);
    // opus overlaps its record (msg_B at 10:05 again); haiku would be new, but nothing is appended.
    import(
        dir,
        &main,
        Some("2026-01-01T10:05:00Z"),
        "2026-01-01T11:00:00Z",
    )
    .code(1)
    .stderr(contains("overlaps"))
    .stderr(contains("2026-01-01T10:05:00.000Z"));
    assert_eq!(records(dir, "ABC-1").len(), 1);
}

#[test]
fn import_appends_a_later_range() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    import(
        dir,
        &main,
        Some("2026-01-01T10:00:00Z"),
        "2026-01-01T10:10:00Z",
    )
    .success();
    import(
        dir,
        &main,
        Some("2026-01-01T10:10:00Z"),
        "2026-01-01T11:00:00Z",
    )
    .success();
    let rs = records(dir, "ABC-1");
    assert_eq!(rs.len(), 3, "{rs:?}");
    let opus = by_model(&rs, "claude-opus-5-5");
    assert_eq!(opus.len(), 2);
    assert_eq!(opus[0]["messages"], 2);
    assert_eq!(opus[1]["messages"], 1);
}

#[test]
fn import_refuses_live_transcript_without_until() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    let path = main.display().to_string();
    stapel(dir)
        .args(["tokens", "import", &path, "--role", "orchestrator"])
        .assert()
        .code(1)
        .stderr(contains("--until"));
    let recent = stapel_core::time::rfc3339(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 60,
    );
    stapel(dir)
        .args([
            "tokens",
            "import",
            &path,
            "--role",
            "orchestrator",
            "--until",
            &recent,
        ])
        .assert()
        .code(1)
        .stderr(contains("5 minutes"));
    assert!(records(dir, "ABC-1").is_empty());
}

#[test]
fn import_respects_window() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    // [10:05, 10:10): msg_B (first line 10:05:00) is in, msg_C at exactly 10:10:00 is out.
    import(
        dir,
        &main,
        Some("2026-01-01T10:05:00Z"),
        "2026-01-01T10:10:00Z",
    )
    .success();
    let rs = records(dir, "ABC-1");
    assert_eq!(rs.len(), 1, "{rs:?}");
    assert_eq!(rs[0]["output"], 4618);
    for bad in [
        "2026-01-01 10:00:00",
        "2026-01-01T10:00:00+00:00",
        "yesterday",
    ] {
        stapel(dir)
            .args([
                "tokens",
                "import",
                &main.display().to_string(),
                "--role",
                "orchestrator",
                "--until",
                bad,
            ])
            .assert()
            .code(1);
    }
}

proptest::proptest! {
    #[test]
    fn parsers_never_panic(line in "\\PC{0,300}", bytes in proptest::collection::vec(proptest::num::u8::ANY, 0..300)) {
        let _ = stapel_core::tokens::parse_transcript_line(&line);
        let _ = stapel_core::tokens::parse_transcript_line(&String::from_utf8_lossy(&bytes));
        let _ = stapel_core::time::parse_utc(&line);
        let _ = stapel_core::time::parse_transcript_time(&line);
    }
}

// ---- AC-4: the report ----

fn golden(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name),
    )
    .unwrap()
}

/// ABC-1 with measured, estimated and imported records.
fn ticket_with_records() -> tempfile::TempDir {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for args in [
        vec![
            "--role",
            "builder",
            "--input",
            "10",
            "--output",
            "20",
            "--cache-read",
            "0",
        ],
        vec!["--role", "author", "--estimate", "5000"],
        vec!["--role", "builder", "--estimate", "70"],
    ] {
        let mut a = vec!["tokens", "add", "ABC-1"];
        a.extend(args);
        stapel(dir).args(&a).assert().success();
    }
    let main = fixture(dir, "main.jsonl");
    import(dir, &main, None, "2026-01-02T00:00:00Z").success();
    repo
}

#[test]
fn report_matches_golden() {
    let repo = ticket_with_records();
    let out = stapel(repo.path())
        .args(["tokens", "ABC-1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(out).unwrap(), golden("tokens_report.txt"));
}

#[test]
fn report_all_matches_golden() {
    let repo = ticket_with_records();
    let dir = repo.path();
    for title in ["Second", "Third", "Fourth"] {
        stapel(dir).args(["new", title]).assert().success();
    }
    // ABC-2: the three shapes of hand-written lines found in STP-1..STP-3.
    std::fs::write(
        dir.join(".stapel/tickets/ABC-2/tokens.jsonl"),
        concat!(
            r#"{"ts":"2026-10-02T18:00:00Z","ticket":"ABC-2","role":"reviewer","model":"claude-sonnet-5-5","total_tokens":50786,"kind":"measured"}"#, "\n",
            r#"{"ts":"2026-10-02T19:00:00Z","ticket":"ABC-2","role":"builder","model":"claude-opus-5-5","input_tokens":900000,"output_tokens":90000,"kind":"estimate"}"#, "\n",
            r#"{"ts":"2026-10-05T07:00:00Z","ticket":"ABC-2","role":"author","model":"claude-opus-5-5","input_tokens":null,"output_tokens":null,"kind":"estimate"}"#, "\n",
        ),
    )
    .unwrap();
    stapel(dir)
        .args([
            "tokens",
            "add",
            "ABC-3",
            "--role",
            "builder",
            "--estimate",
            "1",
        ])
        .assert()
        .success();
    let mut journal = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join(".stapel/tickets/ABC-3/tokens.jsonl"))
        .unwrap();
    std::io::Write::write_all(&mut journal, b"{broken\n").unwrap();
    // ABC-4 has no journal and is left out.
    let out = stapel(dir)
        .arg("tokens")
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        golden("tokens_report_all.txt")
    );
}

#[test]
fn report_never_mixes_sources() {
    let repo = ticket_with_records();
    let out = stapel(repo.path())
        .args(["tokens", "ABC-1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let builder: Vec<&str> = text
        .lines()
        .find(|l| l.starts_with("builder"))
        .unwrap()
        .split_whitespace()
        .collect();
    // input, output, cache read, cache write, estimate: the estimate of 70 stays out of the measured counts.
    assert_eq!(builder[2..7], ["10", "20", "0", "—", "70"]);
}

#[test]
fn report_keeps_columns_apart() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args([
            "tokens",
            "add",
            "--role",
            "a-very-long-role-name",
            "--model",
            "a-very-long-model-name-for-the-column",
            "--estimate",
            "1",
        ])
        .assert()
        .success();
    let out = stapel(dir)
        .args(["tokens", "ABC-1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let row = text
        .lines()
        .find(|l| l.starts_with("a-very-long-role-name"))
        .unwrap();
    let words: Vec<&str> = row.split_whitespace().collect();
    assert_eq!(
        words[..2],
        [
            "a-very-long-role-name",
            "a-very-long-model-name-for-the-column"
        ]
    );
}

// ---- STP-3 code review round 1 ----

// F-1, F-2: large counts are bounded, sums never overflow, and number columns stay apart.
#[test]
fn report_survives_large_numbers() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args([
            "tokens",
            "add",
            "--role",
            "builder",
            "--input",
            "18446744073709551615",
            "--output",
            "1",
        ])
        .assert()
        .code(1);
    for _ in 0..2 {
        stapel(dir)
            .args([
                "tokens",
                "add",
                "--role",
                "builder",
                "--input",
                "999999999999999",
                "--output",
                "1234567890",
            ])
            .assert()
            .success();
    }
    let out = stapel(dir)
        .args(["tokens", "ABC-1"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let row: Vec<&str> = text
        .lines()
        .find(|l| l.starts_with("builder"))
        .unwrap()
        .split_whitespace()
        .collect();
    assert_eq!(row[2..4], ["1999999999999998", "2469135780"], "{text}");
}

// F-3: windows take milliseconds, and an import names the next --since.
#[test]
fn import_window_takes_milliseconds_and_names_the_next_since() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    import(dir, &main, None, "2026-01-01T10:00:00.200Z")
        .success()
        .stdout(contains("next --since: 2026-01-01T10:00:00.101Z"));
    import(
        dir,
        &main,
        Some("2026-01-01T10:00:00.101Z"),
        "2026-01-02T00:00:00Z",
    )
    .success();
    let opus = records(dir, "ABC-1")
        .into_iter()
        .filter(|r| r["model"] == "claude-opus-5-5")
        .count();
    assert_eq!(opus, 2);
}

// F-4: impossible dates are refused on the command line and unreadable in a transcript.
#[test]
fn impossible_dates_are_refused() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    for bad in [
        "2023-02-29T00:00:00Z",
        "2026-04-31T00:00:00Z",
        "2026-01-01T10:00:60Z",
    ] {
        import(dir, &main, None, bad).code(1);
    }
    import(dir, &main, None, "2024-02-29T00:00:00Z").success();
    let bad = fixture(dir, "bad_date.jsonl");
    import(dir, &bad, None, "2026-06-01T00:00:00Z")
        .success()
        .stdout(contains("skipped: 1 unreadable lines"));
}

// F-5: a v1 line with an unknown source is a problem, not a measured row.
#[test]
fn report_flags_unknown_source() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/tokens.jsonl"),
        "{\"v\":1,\"id\":\"t-x\",\"role\":\"r\",\"model\":\"m\",\"source\":\"bogus\",\"estimate\":5}\n",
    )
    .unwrap();
    stapel(dir)
        .args(["tokens", "ABC-1"])
        .assert()
        .code(1)
        .stdout(contains("problem: .stapel/tickets/ABC-1/tokens.jsonl:1"));
}

// F-7: a transcript without a session id is refused; the ticket is part of the record id.
#[test]
fn import_refuses_transcript_without_session() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let no = fixture(dir, "no_session.jsonl");
    import(dir, &no, None, "2026-06-01T00:00:00Z")
        .code(1)
        .stderr(contains("session"));
    let main = fixture(dir, "main.jsonl");
    import(dir, &main, None, "2026-01-02T00:00:00Z").success();
    stapel(dir).args(["new", "Second"]).assert().success();
    let path = main.display().to_string();
    stapel(dir)
        .args([
            "tokens",
            "import",
            &path,
            "ABC-2",
            "--role",
            "orchestrator",
            "--until",
            "2026-01-02T00:00:00Z",
        ])
        .assert()
        .success();
    let ids1: Vec<String> = records(dir, "ABC-1")
        .iter()
        .map(|r| r["id"].to_string())
        .collect();
    let ids2: Vec<String> = records(dir, "ABC-2")
        .iter()
        .map(|r| r["id"].to_string())
        .collect();
    assert!(ids1.iter().all(|i| !ids2.contains(i)), "{ids1:?} {ids2:?}");
}

// F-8, E-3: an empty window says so; a reversed window is refused.
#[test]
fn import_reports_an_empty_window() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    import(
        dir,
        &main,
        Some("2025-01-01T00:00:00Z"),
        "2025-01-02T00:00:00Z",
    )
    .success()
    .stdout(contains("nothing to import in the window"));
    import(
        dir,
        &main,
        Some("2026-01-02T00:00:00Z"),
        "2026-01-01T00:00:00Z",
    )
    .code(1);
}

// E-4, D-3: a long last line still counts (16 MiB bound), and reading stays bounded.
#[test]
fn import_counts_a_long_last_line() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let long = fixture(dir, "long_line.jsonl");
    import(dir, &long, None, "2026-06-01T00:00:00Z").success();
    assert_eq!(records(dir, "ABC-1")[0]["output"], 5000);
}

// E-6: import warns about a role that is not in stapel.toml, like add.
#[test]
fn import_warns_on_unconfigured_role() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let main = fixture(dir, "main.jsonl");
    stapel(dir)
        .args([
            "tokens",
            "import",
            &main.display().to_string(),
            "--role",
            "orchestratr",
            "--until",
            "2026-01-02T00:00:00Z",
        ])
        .assert()
        .success()
        .stderr(contains("warning: role orchestratr is not in stapel.toml"));
}

// D-8: the report reads tokens.jsonl only, so a broken state.json does not hide a journal.
#[test]
fn report_ignores_state_files() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args(["tokens", "add", "--role", "builder", "--estimate", "3"])
        .assert()
        .success();
    std::fs::write(dir.join(".stapel/tickets/ABC-1/state.json"), "{ broken").unwrap();
    stapel(dir)
        .arg("tokens")
        .assert()
        .success()
        .stdout(contains("ticket: ABC-1"));
}

// ---- STP-5: measured usage of subagents ----

// STP-5 AC-1: messages without a final line keep their input and cache and lose only their output.
#[test]
fn partial_messages_lose_only_their_output() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file = fixture(dir, "partial.jsonl");
    import(dir, &file, None, "2026-01-02T00:00:00Z").success();
    let rs = records(dir, "ABC-1");
    assert_eq!(rs.len(), 1, "{rs:?}");
    let r = &rs[0];
    assert_eq!(r["output"], 1500, "{r}");
    assert_eq!(r["partial"], 2, "{r}");
    assert_eq!(r["input"], 15, "{r}");
    assert_eq!(r["cache_read"], 600, "{r}");
    assert_eq!(r["cache_write"], 27, "{r}");
    assert_eq!(r["importer"], 2, "{r}");
    let out = stapel(dir).args(["tokens", "ABC-1"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.lines()
            .any(|l| l.starts_with("orchestrator") && l.contains("≥1500")),
        "{text}"
    );

    // A complete transcript gets no `partial` field.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file = fixture(dir, "subagent.jsonl");
    import(dir, &file, None, "2026-01-02T00:00:00Z").success();
    let rs = records(dir, "ABC-1");
    assert!(
        rs.iter()
            .all(|r| r.get("partial").is_none() && r["importer"] == 2),
        "{rs:?}"
    );
}

// STP-5 AC-2: a subagent record written before STP-5 shows its output as partial.
#[test]
fn older_subagent_records_show_partial_output() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    std::fs::write(
        dir.join(".stapel/tickets/ABC-1/tokens.jsonl"),
        "{\"v\":1,\"id\":\"t-a\",\"role\":\"reviewer\",\"model\":\"m\",\"source\":\"measured\",\"input\":1,\"output\":16,\"transcript\":\"s-1/agent-1\"}\n\
         {\"v\":1,\"id\":\"t-b\",\"role\":\"orchestrator\",\"model\":\"m\",\"source\":\"measured\",\"input\":1,\"output\":40,\"transcript\":\"s-1\"}\n\
         {\"v\":1,\"id\":\"t-c\",\"role\":\"drift\",\"model\":\"m\",\"source\":\"measured\",\"input\":1,\"output\":9,\"transcript\":\"s-1/agent-2\",\"importer\":2}\n",
    )
    .unwrap();
    let out = stapel(dir).args(["tokens", "ABC-1"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let line = |role: &str| {
        text.lines()
            .find(|l| l.starts_with(role))
            .unwrap_or("")
            .to_string()
    };
    assert!(line("reviewer").contains("≥16"), "{text}");
    assert!(!line("orchestrator").contains('≥'), "{text}");
    assert!(!line("drift").contains('≥'), "{text}");
}

fn sessions(dir: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join(".stapel/sessions.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

// STP-5 AC-3: the last cost-state line of a transcript is recorded once.
#[test]
fn session_totals_are_imported_once() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file = fixture(dir, "session.jsonl");
    stapel(dir)
        .args(["tokens", "session", &file.display().to_string()])
        .assert()
        .success();
    let s = sessions(dir);
    assert_eq!(s.len(), 1, "{s:?}");
    let r = &s[0];
    assert_eq!(r["v"], 1);
    assert!(r["id"].as_str().unwrap().starts_with("s-"), "{r}");
    assert!(r["at"].as_str().unwrap().ends_with('Z'), "{r}");
    assert_eq!(r["session"], "s-sess");
    assert_eq!(r["start"], 1767268800000u64);
    assert_eq!(r["duration_ms"], 120000);
    let opus = &r["models"]["claude-opus-5-5"];
    assert_eq!(opus["output"], 800, "{r}");
    assert_eq!(opus["thinking"], 150);
    assert_eq!(opus["input"], 5);
    assert_eq!(opus["cache_read"], 100);
    assert_eq!(opus["cache_write"], 20);
    assert_eq!(r["models"]["claude-sonnet-5-5"]["output"], 1200);

    let id = r["id"].as_str().unwrap().to_string();
    stapel(dir)
        .args(["tokens", "session", &file.display().to_string()])
        .assert()
        .success()
        .stdout(contains(format!("already imported: {id}")));
    assert_eq!(sessions(dir).len(), 1);
}

// STP-5 AC-3: no cost-state line, nothing recorded.
#[test]
fn session_without_cost_state_is_refused() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file = fixture(dir, "main.jsonl");
    stapel(dir)
        .args(["tokens", "session", &file.display().to_string()])
        .assert()
        .code(1)
        .stderr(contains("cost-state"));
    assert!(!dir.join(".stapel/sessions.jsonl").exists());
}

// STP-5 AC-3: a half-written last cost-state line is refused, not replaced by an earlier one.
#[test]
fn truncated_cost_state_is_refused() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file = fixture(dir, "session_truncated.jsonl");
    stapel(dir)
        .args(["tokens", "session", &file.display().to_string()])
        .assert()
        .code(1)
        .stderr(contains("line 4"));
    assert!(!dir.join(".stapel/sessions.jsonl").exists());
}
