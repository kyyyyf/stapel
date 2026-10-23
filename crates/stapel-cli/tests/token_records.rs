//! STP-19: token records imported from a file. Step 1: `stapel tokens import-records` in a
//! terminal (AC-1 to AC-5).

mod common;

use common::{repo_with_ticket, stapel};
use serde_json::{Value, json};
use std::path::Path;

const TARGET: &str = ".stapel/tickets/ABC-1/tokens.jsonl";
const COUNT_REASON: &str = "must be a whole number from 0 to 10^15";
const SHORT_REASON: &str =
    "must be a non-blank string of at most 256 bytes with no control character";

fn golden(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name),
    )
    .unwrap()
}

/// A measured record of a file.
fn measured(id: &str) -> Value {
    json!({
        "id": id,
        "role": "builder",
        "model": "m-1",
        "source": "measured",
        "input": 10,
        "output": 5,
    })
}

/// An estimate record of a file.
fn estimate(id: &str) -> Value {
    json!({
        "id": id,
        "role": "builder",
        "model": "m-1",
        "source": "estimate",
        "estimate": 100,
    })
}

fn put(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn line(v: &Value) -> String {
    format!("{v}\n")
}

fn target_text(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(TARGET)).ok()
}

/// Runs `tokens import-records` and expects exit 2, an empty stdout and the stderr returned.
fn refused(dir: &Path, args: &[&str]) -> String {
    let out = stapel(dir)
        .args(["tokens", "import-records"])
        .args(args)
        .assert()
        .code(2)
        .get_output()
        .clone();
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    String::from_utf8(out.stderr).unwrap()
}

fn run(dir: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    stapel(dir)
        .args(["tokens", "import-records"])
        .args(args)
        .assert()
}

/// Writes `text` as `in.jsonl` and runs the import against an empty target: the written text
/// when it is accepted, the stderr when it is refused (exit 2).
fn try_import(dir: &Path, text: &str) -> Result<String, String> {
    let _ = std::fs::remove_file(dir.join(TARGET));
    put(dir, "in.jsonl", text);
    let out = stapel(dir)
        .args(["tokens", "import-records", "in.jsonl"])
        .output()
        .unwrap();
    match out.status.code() {
        Some(0) => Ok(target_text(dir).unwrap()),
        Some(2) => {
            assert!(out.stdout.is_empty());
            assert!(target_text(dir).is_none(), "wrote something");
            Err(String::from_utf8(out.stderr).unwrap())
        }
        other => panic!("exit {other:?}: {}", String::from_utf8_lossy(&out.stderr)),
    }
}

/// `at` of a written line: the time of the run in UTC.
fn at_of(written_line: &str) -> String {
    let v: Value = serde_json::from_str(written_line).unwrap();
    let at = v["at"].as_str().unwrap().to_string();
    assert_eq!(at.len(), 20, "{at}");
    assert!(at.starts_with("20") && at.ends_with('Z'), "{at}");
    at
}

// ---- AC-1 ----

#[test]
fn import_appends_all_in_order() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let old = "{\"id\":\"OLD-1\",\"role\":\"x\"}\n";
    put(dir, TARGET, old);
    // Odd spacing, an unusual key order, escapes and a blank line.
    let one = r#"{"note": "a \"q\" я\nb", "id": "T-1", "step": "3", "output": 20, "cache_write": 4, "input": 10, "cache_read": 3, "model": "m-1", "role": "builder", "source": "measured"}"#;
    let two = r#"{"source":"measured","output":7,"input":9,"model":"m-2","role":"reviewer","id":"T-2"}"#;
    let three = r#"{"v":1,"ticket":"ABC-1","id":"T-3","role":"drift","model":"m-3","source":"estimate","estimate":500,"step":"s"}"#;
    put(
        dir,
        "sub/in.jsonl",
        &format!("{one}\n\n{two}   \n{three}\n"),
    );
    run(dir, &["sub/in.jsonl"])
        .success()
        .stdout("imported: 3 token records into ABC-1\n");
    let text = target_text(dir).unwrap();
    let written: Vec<&str> = text.strip_prefix(old).unwrap().lines().collect();
    assert_eq!(written.len(), 3, "{text}");
    let at = at_of(written[0]);
    for l in &written {
        assert_eq!(at_of(l), at);
    }
    let expect = [
        format!(
            "{{\"v\":1,\"id\":\"T-1\",\"at\":\"{at}\",\"ticket\":\"ABC-1\",\"role\":\"builder\",\"model\":\"m-1\",\"source\":\"measured\",\"input\":10,\"output\":20,\"cache_read\":3,\"cache_write\":4,\"step\":\"3\",\"note\":\"a \\\"q\\\" я\\nb\",\"imported_from\":\"in.jsonl\"}}"
        ),
        format!(
            "{{\"v\":1,\"id\":\"T-2\",\"at\":\"{at}\",\"ticket\":\"ABC-1\",\"role\":\"reviewer\",\"model\":\"m-2\",\"source\":\"measured\",\"input\":9,\"output\":7,\"imported_from\":\"in.jsonl\"}}"
        ),
        format!(
            "{{\"v\":1,\"id\":\"T-3\",\"at\":\"{at}\",\"ticket\":\"ABC-1\",\"role\":\"drift\",\"model\":\"m-3\",\"source\":\"estimate\",\"estimate\":500,\"step\":\"s\",\"imported_from\":\"in.jsonl\"}}"
        ),
    ];
    assert_eq!(written, expect);
}

#[test]
fn import_creates_and_repairs_target() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &line(&measured("T-1")));
    assert!(target_text(dir).is_none());
    run(dir, &["in.jsonl", "ABC-1"])
        .success()
        .stdout("imported: 1 token records into ABC-1\n");
    let text = target_text(dir).unwrap();
    let at = at_of(text.trim_end());
    assert_eq!(
        text,
        format!(
            "{{\"v\":1,\"id\":\"T-1\",\"at\":\"{at}\",\"ticket\":\"ABC-1\",\"role\":\"builder\",\"model\":\"m-1\",\"source\":\"measured\",\"input\":10,\"output\":5,\"imported_from\":\"in.jsonl\"}}\n"
        )
    );

    // A target cut short gets its newline before the append.
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, TARGET, "{\"id\":\"OLD-1\"}");
    put(dir, "in.jsonl", &line(&estimate("T-1")));
    run(dir, &["in.jsonl", "abc-1"])
        .success()
        .stdout("imported: 1 token records into ABC-1\n");
    let text = target_text(dir).unwrap();
    let rest = text.strip_prefix("{\"id\":\"OLD-1\"}\n").unwrap();
    let at = at_of(rest.trim_end());
    assert_eq!(
        rest,
        format!(
            "{{\"v\":1,\"id\":\"T-1\",\"at\":\"{at}\",\"ticket\":\"ABC-1\",\"role\":\"builder\",\"model\":\"m-1\",\"source\":\"estimate\",\"estimate\":100,\"imported_from\":\"in.jsonl\"}}\n"
        )
    );
}

// ---- AC-2 ----

#[test]
fn import_refuses_bad_fields() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut cases: Vec<(String, String)> = Vec::new();
    let mut add = |name: &str, text: String| cases.push((name.to_string(), text));
    // A record with one field removed or replaced.
    let edit = |mut v: Value, field: &str, value: Option<Value>| {
        match value {
            Some(x) => v[field] = x,
            None => {
                v.as_object_mut().unwrap().remove(field);
            }
        }
        line(&v)
    };
    let raw_input = |input: &str| {
        format!(
            "{{\"id\":\"T-1\",\"role\":\"builder\",\"model\":\"m-1\",\"source\":\"measured\",\"input\":{input},\"output\":5}}\n"
        )
    };
    add("id missing", edit(measured("T-1"), "id", None));
    add("id empty", edit(measured("T-1"), "id", Some(json!(""))));
    add("id number", edit(measured("T-1"), "id", Some(json!(7))));
    add(
        "ticket other",
        edit(measured("T-1"), "ticket", Some(json!("ABC-2"))),
    );
    add("role missing", edit(measured("T-1"), "role", None));
    add("role empty", edit(measured("T-1"), "role", Some(json!(""))));
    add(
        "role blank",
        edit(measured("T-1"), "role", Some(json!("  "))),
    );
    add("role number", edit(measured("T-1"), "role", Some(json!(7))));
    add(
        "role tab",
        edit(measured("T-1"), "role", Some(json!("a\tb"))),
    );
    add("model missing", edit(measured("T-1"), "model", None));
    add("source missing", edit(measured("T-1"), "source", None));
    add(
        "source case",
        edit(measured("T-1"), "source", Some(json!("Measured"))),
    );
    add("measured no input", edit(measured("T-1"), "input", None));
    add("measured no output", edit(measured("T-1"), "output", None));
    add(
        "measured with estimate",
        edit(measured("T-1"), "estimate", Some(json!(1))),
    );
    add("estimate no estimate", edit(estimate("T-1"), "estimate", None));
    add(
        "estimate with input",
        edit(estimate("T-1"), "input", Some(json!(1))),
    );
    add(
        "estimate with cache_write",
        edit(estimate("T-1"), "cache_write", Some(json!(1))),
    );
    add(
        "estimate with cache_read",
        edit(estimate("T-1"), "cache_read", Some(json!(1))),
    );
    for (name, input) in [
        ("input -1", "-1"),
        ("input 1.0", "1.0"),
        ("input 1e3", "1e3"),
        ("input string", "\"5\""),
        ("input null", "null"),
    ] {
        add(name, raw_input(input));
    }
    add(
        "output -1",
        edit(measured("T-1"), "output", Some(json!(-1))),
    );
    add(
        "cache_read -1",
        edit(measured("T-1"), "cache_read", Some(json!(-1))),
    );
    add(
        "cache_write 1.5",
        edit(measured("T-1"), "cache_write", Some(json!(1.5))),
    );
    add(
        "estimate -1",
        edit(estimate("T-1"), "estimate", Some(json!(-1))),
    );
    add("step empty", edit(measured("T-1"), "step", Some(json!(""))));
    add("note number", edit(measured("T-1"), "note", Some(json!(7))));
    add(
        "note tab",
        edit(measured("T-1"), "note", Some(json!("a\tb"))),
    );
    add("v 2", edit(measured("T-1"), "v", Some(json!(2))));
    add("v string", edit(measured("T-1"), "v", Some(json!("1"))));
    for field in [
        "at",
        "transcript",
        "importer",
        "partial",
        "from",
        "to",
        "messages",
        "imported_from",
        "kind",
    ] {
        add(
            &format!("field {field}"),
            edit(measured("T-1"), field, Some(json!("x"))),
        );
    }
    // The order of AC-2 by records that break two rules each (SR1-4).
    let mut a = measured("T-1");
    a.as_object_mut().unwrap().remove("input");
    a["estimate"] = json!(3);
    add("two rules: no input and estimate", line(&a));
    let mut b = estimate("T-1");
    b["input"] = json!(1);
    b["note"] = json!(7);
    add("two rules: input and note", line(&b));
    let mut c = measured("T-1");
    c["at"] = json!("2026-01-01T00:00:00Z");
    c["cache_read"] = json!(-1);
    add("two rules: at and cache_read", line(&c));
    // The first bad record is named by its line, a blank line counted.
    let mut bad = measured("T-2");
    bad.as_object_mut().unwrap().remove("role");
    add(
        "bad third line",
        format!("{}\n{}", line(&measured("T-1")), line(&bad)),
    );

    let mut got = String::new();
    for (name, text) in &cases {
        put(dir, "in.jsonl", text);
        let err = refused(dir, &["in.jsonl"]);
        assert!(err.starts_with("stapel: in.jsonl:"), "{name}: {err}");
        assert_eq!(err.lines().count(), 1, "{name}: {err}");
        got.push_str(&err);
        assert!(target_text(dir).is_none(), "{name}: wrote something");
    }
    assert_eq!(got, golden("token_records_refusals.txt"));
}

#[test]
fn field_limits_count_bytes_and_characters() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let with = |field: &str, value: Value| {
        let mut v = measured("T-1");
        v[field] = value;
        line(&v)
    };
    let parsed = |text: &str| -> Value { serde_json::from_str(text.lines().next().unwrap()).unwrap() };

    // `id`: 64 characters, not bytes.
    for id in ["x".repeat(64), "ж".repeat(64)] {
        let text = try_import(dir, &line(&measured(&id))).unwrap();
        assert_eq!(parsed(&text)["id"], json!(id));
    }
    for id in ["x".repeat(65), "ж".repeat(65)] {
        let err = try_import(dir, &line(&measured(&id))).unwrap_err();
        assert_eq!(
            err,
            "stapel: in.jsonl:1: id: must be 1 to 64 characters with no whitespace or control character\n"
        );
    }
    for id in ["T 1", "T\t1", "T\u{1}1"] {
        let err = try_import(dir, &line(&measured(id))).unwrap_err();
        assert!(err.starts_with("stapel: in.jsonl:1: id: must be 1 to 64"), "{err}");
    }

    // Counts: 0 and 10^15 accepted, 10^15 + 1 refused.
    for field in ["input", "output", "cache_read", "cache_write"] {
        for n in [0u64, 1_000_000_000_000_000] {
            let text = try_import(dir, &with(field, json!(n))).unwrap();
            assert_eq!(parsed(&text)[field], json!(n), "{field}");
        }
        let err = try_import(dir, &with(field, json!(1_000_000_000_000_001u64))).unwrap_err();
        assert_eq!(
            err,
            format!("stapel: in.jsonl:1: {field}: {COUNT_REASON}\n")
        );
    }
    for n in [0u64, 1_000_000_000_000_000] {
        let mut v = estimate("T-1");
        v["estimate"] = json!(n);
        let text = try_import(dir, &line(&v)).unwrap();
        assert_eq!(parsed(&text)["estimate"], json!(n));
    }
    let mut v = estimate("T-1");
    v["estimate"] = json!(1_000_000_000_000_001u64);
    let err = try_import(dir, &line(&v)).unwrap_err();
    assert_eq!(err, format!("stapel: in.jsonl:1: estimate: {COUNT_REASON}\n"));

    // `role`, `model`, `step`: 256 bytes, not characters.
    for field in ["role", "model", "step"] {
        for ok in ["x".repeat(256), "ж".repeat(128)] {
            let text = try_import(dir, &with(field, json!(ok))).unwrap();
            assert_eq!(parsed(&text)[field], json!(ok), "{field}");
        }
        for bad in ["x".repeat(257), "ж".repeat(129)] {
            let err = try_import(dir, &with(field, json!(bad))).unwrap_err();
            assert_eq!(
                err,
                format!("stapel: in.jsonl:1: {field}: {SHORT_REASON}\n"),
                "{field}"
            );
        }
    }

    // `note`: 4096 bytes, a newline allowed, the empty note allowed.
    for ok in [
        "x".repeat(4096),
        "ж".repeat(2048),
        "a\nb".to_string(),
        String::new(),
    ] {
        let text = try_import(dir, &with("note", json!(ok))).unwrap();
        assert_eq!(parsed(&text)["note"], json!(ok));
    }
    for bad in ["x".repeat(4097), "ж".repeat(2049)] {
        let err = try_import(dir, &with("note", json!(bad))).unwrap_err();
        assert_eq!(
            err,
            "stapel: in.jsonl:1: note: must be a string of at most 4096 bytes with no control character but newline\n"
        );
    }
}

// ---- AC-3 ----

#[test]
fn import_refuses_bad_file() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let one = line(&estimate("T-1"));
    let many = |n: usize| -> String {
        (1..=n).map(|i| line(&estimate(&format!("T-{i}")))).collect()
    };
    let cases: Vec<(String, String)> = vec![
        (
            "[1]\n".into(),
            "stapel: in.jsonl:1: not a JSON object\n".into(),
        ),
        (
            format!("{one}[1]\n"),
            "stapel: in.jsonl:2: not a JSON object\n".into(),
        ),
        ("{\"id\": \n".into(), "stapel: in.jsonl:1: not JSON".into()),
        (
            format!("{}\n", "x".repeat(64 * 1024 + 1)),
            "stapel: in.jsonl:1: longer than 64 KiB\n".into(),
        ),
        ("\n  \n\n".into(), "stapel: in.jsonl: no records\n".into()),
        (String::new(), "stapel: in.jsonl: no records\n".into()),
        (
            many(101),
            "stapel: in.jsonl:101: more than 100 records; split the file\n".into(),
        ),
        (
            format!("{}[1]\n", many(100)),
            "stapel: in.jsonl:101: not a JSON object\n".into(),
        ),
        (
            format!("{}not json\n", many(100)),
            "stapel: in.jsonl:101: not JSON".into(),
        ),
        (
            " ".repeat(256 * 1024 + 1),
            "stapel: in.jsonl: more than 256 KiB; split the file\n".into(),
        ),
        (
            format!("{one}{one}"),
            "stapel: in.jsonl:2: id T-1 repeats line 1\n".into(),
        ),
        (
            format!("{one}\n{one}"),
            "stapel: in.jsonl:3: id T-1 repeats line 1\n".into(),
        ),
    ];
    for (text, want) in cases {
        put(dir, "in.jsonl", &text);
        let err = refused(dir, &["in.jsonl"]);
        assert!(err.starts_with(&want), "want {want:?}, got {err:?}");
        assert!(target_text(dir).is_none(), "{want}: wrote something");
    }
}

#[test]
fn import_refuses_repeated_id() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let out = stapel(dir)
        .args([
            "tokens", "add", "--role", "builder", "--model", "m-1", "--estimate", "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    let added = stdout.trim().strip_prefix("recorded: ").unwrap().to_string();
    // A legacy line with an id, and a line of an unknown version, which is skipped.
    let before = target_text(dir).unwrap();
    put(
        dir,
        TARGET,
        &format!(
            "{before}{{\"ts\":\"2026-01-01T00:00:00Z\",\"kind\":\"x\",\"role\":\"r\",\"id\":\"L-1\",\"total_tokens\":5}}\n{{\"v\":2,\"id\":\"U-1\"}}\n"
        ),
    );
    let before = target_text(dir).unwrap();
    for id in [added.as_str(), "L-1"] {
        put(dir, "in.jsonl", &line(&estimate(id)));
        let err = refused(dir, &["in.jsonl"]);
        assert_eq!(
            err,
            format!("stapel: in.jsonl:1: id {id} is already in {TARGET}\n")
        );
        assert_eq!(target_text(dir).unwrap(), before);
    }
    // The id of a line with `v: 2` is not known: it may be imported.
    put(dir, "in.jsonl", &line(&estimate("U-1")));
    run(dir, &["in.jsonl"])
        .success()
        .stdout("imported: 1 token records into ABC-1\n");
    assert!(target_text(dir).unwrap().starts_with(&before));
    assert_eq!(target_text(dir).unwrap().lines().count(), 4);
}

#[test]
fn import_accepts_the_limits() {
    /// A record line of `total` bytes with its newline, padded with spaces after the record.
    fn padded(id: &str, total: usize) -> String {
        let record = format!("{}", estimate(id));
        let text = format!("{record}{}\n", " ".repeat(total - record.len() - 1));
        assert_eq!(text.len(), total);
        text
    }
    let many = |n: usize| -> String {
        (1..=n).map(|i| line(&estimate(&format!("T-{i}")))).collect()
    };

    // Exactly 100 records.
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &many(100));
    run(dir, &["in.jsonl"])
        .success()
        .stdout("imported: 100 token records into ABC-1\n");
    assert_eq!(target_text(dir).unwrap().lines().count(), 100);

    // A line of exactly 64 KiB, newline included.
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &padded("T-1", 64 * 1024));
    run(dir, &["in.jsonl"])
        .success()
        .stdout("imported: 1 token records into ABC-1\n");
    assert_eq!(target_text(dir).unwrap().lines().count(), 1);

    // A file of exactly 256 KiB: four lines of 64 KiB.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file: String = (1..=4)
        .map(|n| padded(&format!("T-{n}"), 64 * 1024))
        .collect();
    assert_eq!(file.len(), 256 * 1024);
    put(dir, "in.jsonl", &file);
    run(dir, &["in.jsonl"])
        .success()
        .stdout("imported: 4 token records into ABC-1\n");
    assert_eq!(target_text(dir).unwrap().lines().count(), 4);

    // CRLF line ends.
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(
        dir,
        "in.jsonl",
        &format!(
            "{}\r\n{}\r\n",
            estimate("T-1"),
            measured("T-2")
        ),
    );
    run(dir, &["in.jsonl"])
        .success()
        .stdout("imported: 2 token records into ABC-1\n");
    let text = target_text(dir).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(!text.contains('\r'), "{text:?}");
}

// ---- AC-4 ----

#[test]
fn import_refuses_bad_input_path() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["new", "Second"]).assert().success();
    put(dir, TARGET, "{\"id\":\"OLD-1\"}\n");
    let before_target = target_text(dir).unwrap();
    put(
        dir,
        ".stapel/tickets/ABC-2/findings.jsonl",
        &line(&estimate("T-1")),
    );
    put(dir, ".stapel/process-ledger.jsonl", &line(&estimate("T-1")));
    put(
        dir,
        ".STAPEL/tickets/ABC-1/TOKENS.jsonl",
        &line(&estimate("T-1")),
    );
    std::fs::create_dir(dir.join("sub")).unwrap();
    std::fs::write(dir.join("latin.jsonl"), b"{\"note\":\"caf\xe9\"}\n").unwrap();
    #[cfg(unix)]
    {
        let status = std::process::Command::new("mkfifo")
            .arg(dir.join("pipe"))
            .status()
            .unwrap();
        assert!(status.success());
        std::os::unix::fs::symlink(dir.join(TARGET), dir.join("link-tokens.jsonl")).unwrap();
    }
    let machine = "a machine file; it cannot be imported\n";
    let cases: Vec<(&str, String)> = vec![
        ("nope.jsonl", "stapel: nope.jsonl: ".into()),
        ("sub", "stapel: sub: not a regular file\n".into()),
        ("sub/..", "stapel: sub/..: not a regular file\n".into()),
        ("-", "stapel: -: stdin is not accepted; give a file\n".into()),
        ("latin.jsonl", "stapel: latin.jsonl: not UTF-8\n".into()),
        (TARGET, format!("stapel: {TARGET}: {machine}")),
        (
            ".stapel/tickets/ABC-2/findings.jsonl",
            format!("stapel: .stapel/tickets/ABC-2/findings.jsonl: {machine}"),
        ),
        (
            ".stapel/process-ledger.jsonl",
            format!("stapel: .stapel/process-ledger.jsonl: {machine}"),
        ),
        (
            ".STAPEL/tickets/ABC-1/TOKENS.jsonl",
            format!("stapel: .STAPEL/tickets/ABC-1/TOKENS.jsonl: {machine}"),
        ),
        #[cfg(unix)]
        ("pipe", "stapel: pipe: not a regular file\n".into()),
        #[cfg(unix)]
        (
            "link-tokens.jsonl",
            format!("stapel: link-tokens.jsonl: {machine}"),
        ),
    ];
    for (file, want) in cases {
        let out = stapel(dir)
            .args(["tokens", "import-records", file, "ABC-1"])
            .timeout(std::time::Duration::from_secs(5))
            .assert()
            .code(2)
            .get_output()
            .clone();
        assert!(out.stdout.is_empty(), "{file}");
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(err.starts_with(&want), "{file}: want {want:?}, got {err:?}");
        assert_eq!(target_text(dir).unwrap(), before_target, "{file}");
    }

    // A symlink to a regular file is read; the base name is the link's own.
    #[cfg(unix)]
    {
        put(dir, "real.jsonl", &line(&estimate("T-7")));
        std::os::unix::fs::symlink(dir.join("real.jsonl"), dir.join("alias.jsonl")).unwrap();
        run(dir, &["alias.jsonl", "ABC-1"])
            .success()
            .stdout("imported: 1 token records into ABC-1\n");
        let text = target_text(dir).unwrap();
        let rest = text.strip_prefix(&before_target).unwrap();
        assert!(rest.contains("\"id\":\"T-7\""), "{rest}");
        assert!(rest.ends_with(",\"imported_from\":\"alias.jsonl\"}\n"), "{rest}");
    }
}

#[test]
fn import_refuses_broken_target() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &line(&estimate("T-1")));
    for (text, want) in [
        (
            "{\"id\":\"OLD-1\"}\nnot json\n",
            format!("stapel: {TARGET}:2: not JSON"),
        ),
        (
            "{\"id\":\"OLD-1\"}\n[1]\n",
            format!("stapel: {TARGET}:2: not a JSON object\n"),
        ),
    ] {
        put(dir, TARGET, text);
        let err = refused(dir, &["in.jsonl"]);
        assert!(err.starts_with(&want), "want {want:?}, got {err:?}");
        assert_eq!(target_text(dir).unwrap(), text);
    }
    // A line of an unknown version is skipped, not a defect.
    put(dir, TARGET, "{\"v\":2,\"id\":\"U-1\"}\n");
    run(dir, &["in.jsonl"])
        .success()
        .stdout("imported: 1 token records into ABC-1\n");
    // A bad record in the file: its reason comes before the defect of the target.
    let mut bad = estimate("T-2");
    bad.as_object_mut().unwrap().remove("role");
    put(dir, "in.jsonl", &line(&bad));
    put(dir, TARGET, "not json\n");
    let err = refused(dir, &["in.jsonl"]);
    assert_eq!(err, "stapel: in.jsonl:1: role: missing\n");
    assert_eq!(target_text(dir).unwrap(), "not json\n");
}

// ---- AC-5 ----

#[test]
fn closed_ticket_by_key_only() {
    let one_record = |id: &str| line(&estimate(id));
    let import = |dir: &Path, key: Option<&str>| {
        let mut args = vec!["in.jsonl"];
        args.extend(key);
        run(dir, &args)
    };
    let refusal = |dir: &Path, key: Option<&str>, want: &str| {
        let out = import(dir, key).code(2).get_output().clone();
        assert!(out.stdout.is_empty());
        let err = String::from_utf8(out.stderr).unwrap();
        assert_eq!(err, want);
        assert_eq!(err.lines().count(), 1, "{err}");
    };

    // A closed ticket by key: written as for an open one, in any case.
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args(["close", "--reason", "done"])
        .assert()
        .success();
    put(dir, "in.jsonl", &one_record("T-1"));
    import(dir, Some("abc-1"))
        .success()
        .stdout("imported: 1 token records into ABC-1\n");
    assert_eq!(target_text(dir).unwrap().lines().count(), 1);
    put(dir, "in.jsonl", &one_record("T-2"));
    import(dir, Some("ABC-1"))
        .success()
        .stdout("imported: 1 token records into ABC-1\n");
    assert_eq!(target_text(dir).unwrap().lines().count(), 2);

    // No KEY and no open ticket.
    refusal(
        dir,
        None,
        "stapel: no open ticket; name one, e.g. stapel status <KEY>\n",
    );
    assert_eq!(target_text(dir).unwrap().lines().count(), 2);

    // No KEY with one open ticket: that ticket, not the closed one.
    stapel(dir).args(["new", "Second"]).assert().success();
    put(dir, "in.jsonl", &one_record("T-3"));
    import(dir, None)
        .success()
        .stdout("imported: 1 token records into ABC-2\n");
    let second =
        std::fs::read_to_string(dir.join(".stapel/tickets/ABC-2/tokens.jsonl")).unwrap();
    assert!(second.contains("\"ticket\":\"ABC-2\""), "{second}");
    assert_eq!(target_text(dir).unwrap().lines().count(), 2);

    // No KEY with two open tickets.
    stapel(dir).args(["new", "Third"]).assert().success();
    refusal(
        dir,
        None,
        "stapel: several open tickets, name one: ABC-2, ABC-3\n",
    );
    assert!(!dir.join(".stapel/tickets/ABC-3/tokens.jsonl").exists());

    // A key with no folder.
    refusal(
        dir,
        Some("ABC-7"),
        "stapel: no ticket ABC-7 in .stapel/tickets\n",
    );

    // A legacy ticket by key.
    put(
        dir,
        ".stapel/tickets/ABC-9/state.json",
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    );
    refusal(
        dir,
        Some("abc-9"),
        "stapel: ticket ABC-9 has a legacy (STP-1) state.json; its journal is kept by hand\n",
    );
    assert!(!dir.join(".stapel/tickets/ABC-9/tokens.jsonl").exists());
}

// ---- Coverage (step 1c) ----

#[test]
fn field_check_order_edges() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let first_error = |v: Value| try_import(dir, &line(&v)).unwrap_err();

    // Two other fields: the first in the line, not in a sorted order.
    let mut v = measured("T-1");
    v["zzz"] = json!(1);
    v["at"] = json!("x");
    assert_eq!(
        first_error(v),
        "stapel: in.jsonl:1: zzz: not allowed in a token record\n"
    );

    // Two bad counts: `output` before `cache_read`.
    let mut v = measured("T-1");
    v["output"] = json!(-1);
    v["cache_read"] = json!(-1);
    assert_eq!(
        first_error(v),
        format!("stapel: in.jsonl:1: output: {COUNT_REASON}\n")
    );

    // `role` and `model` both missing: `role` first.
    let mut v = measured("T-1");
    v.as_object_mut().unwrap().remove("role");
    v.as_object_mut().unwrap().remove("model");
    assert_eq!(first_error(v), "stapel: in.jsonl:1: role: missing\n");

    // A blank id.
    let mut v = measured("T-1");
    v["id"] = json!("  ");
    assert_eq!(
        first_error(v),
        "stapel: in.jsonl:1: id: must be 1 to 64 characters with no whitespace or control character\n"
    );

    // `model` with DEL.
    let mut v = measured("T-1");
    v["model"] = json!("m\u{7f}1");
    assert_eq!(
        first_error(v),
        format!("stapel: in.jsonl:1: model: {SHORT_REASON}\n")
    );

    // `note` with a carriage return.
    let mut v = measured("T-1");
    v["note"] = json!("a\rb");
    assert_eq!(
        first_error(v),
        "stapel: in.jsonl:1: note: must be a string of at most 4096 bytes with no control character but newline\n"
    );

    // `step` null is present and not a short text.
    let mut v = measured("T-1");
    v["step"] = Value::Null;
    assert_eq!(
        first_error(v),
        format!("stapel: in.jsonl:1: step: {SHORT_REASON}\n")
    );
}

#[test]
fn file_limit_edges() {
    let repo = repo_with_ticket();
    let dir = repo.path();

    // A 101st line with a bad field gets the limit reason.
    let mut text: String = (1..=100)
        .map(|i| line(&estimate(&format!("T-{i}"))))
        .collect();
    let mut bad = estimate("T-101");
    bad.as_object_mut().unwrap().remove("role");
    text.push_str(&line(&bad));
    assert_eq!(
        try_import(dir, &text).unwrap_err(),
        "stapel: in.jsonl:101: more than 100 records; split the file\n"
    );

    // A CRLF line is measured without its CR: 64 KiB with LF plus a CR is accepted.
    let crlf = |total_without_cr: usize| -> String {
        let record = format!("{}", estimate("T-1"));
        let pad = " ".repeat(total_without_cr - record.len() - 1);
        let text = format!("{record}{pad}\r\n");
        assert_eq!(text.len(), total_without_cr + 1);
        text
    };
    let text = try_import(dir, &crlf(64 * 1024)).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(!text.contains('\r'), "{text:?}");
    // One byte more is refused.
    assert_eq!(
        try_import(dir, &crlf(64 * 1024 + 1)).unwrap_err(),
        "stapel: in.jsonl:1: longer than 64 KiB\n"
    );
}

#[test]
fn missing_file_reason() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    assert_eq!(
        refused(dir, &["nope.jsonl"]),
        "stapel: nope.jsonl: No such file or directory (os error 2)\n"
    );
    assert!(target_text(dir).is_none());
}

#[test]
fn ticket_chosen_before_file() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    assert_eq!(
        refused(dir, &["nope.jsonl", "NOPE-1"]),
        "stapel: no ticket NOPE-1 in .stapel/tickets\n"
    );
}

#[test]
fn report_shows_imported_rows() {
    // Rows of a ticket with every origin, in the order of the golden file.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let lines = [
        r#"{"v":1,"id":"t-1","role":"orchestrator","model":"claude-opus-5-5","source":"measured","output":300,"transcript":"s-sess","from":"2026-01-01T12:00:00.000Z","to":"2026-01-01T12:00:00.000Z","importer":2}"#,
        r#"{"v":1,"id":"t-2","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"builder","model":"claude-opus-5-5","source":"measured","input":10,"output":20,"cache_read":0}"#,
        r#"{"ts":"2026-10-02T18:00:00Z","ticket":"ABC-1","role":"builder","model":"claude-opus-5-5","total_tokens":500,"kind":"measured"}"#,
        r#"{"v":1,"id":"t-4","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"builder","model":"claude-opus-5-5","source":"measured","input":4,"output":90,"transcript":"s-sess","from":"2026-01-01T12:01:00.000Z","to":"2026-01-01T12:01:00.000Z","importer":2,"imported_from":"in.jsonl"}"#,
        r#"{"v":1,"id":"t-5","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"builder","model":"claude-opus-5-5","source":"estimate","estimate":80,"imported_from":"in.jsonl"}"#,
        r#"{"v":1,"id":"t-6","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"reviewer","model":"claude-opus-5-5","source":"measured","input":1,"output":2,"imported_from":"in.jsonl"}"#,
    ];
    put(dir, TARGET, &(lines.join("\n") + "\n"));
    let report = |dir: &Path, args: &[&str]| {
        let out = stapel(dir)
            .args(["tokens"])
            .args(args)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        String::from_utf8(out).unwrap()
    };
    assert_eq!(report(dir, &["ABC-1"]), golden("tokens_report_file.txt"));

    // The sessions table counts the transcript record only, not the imported one of the same
    // session and model.
    put(
        dir,
        ".stapel/sessions.jsonl",
        "{\"v\":1,\"id\":\"s-1\",\"at\":\"2026-01-02T00:00:00Z\",\"session\":\"s-sess\",\"start\":1767268800000,\"duration_ms\":120000,\"models\":{\"claude-opus-5-5\":{\"output\":800,\"thinking\":150}}}\n",
    );
    let text = report(dir, &[]);
    let row: Vec<&str> = text
        .lines()
        .find(|l| l.starts_with("s-sess"))
        .unwrap_or_else(|| panic!("no session row:\n{text}"))
        .split_whitespace()
        .collect();
    assert_eq!(
        row,
        ["s-sess", "claude-opus-5-5", "800", "150", "300", "0", "0", "500"]
    );

    // A ticket of the history shape, with no `imported_from`, keeps its report.
    let history = repo_with_ticket();
    let hdir = history.path();
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
        stapel(hdir).args(&a).assert().success();
    }
    let main = hdir.join("main.jsonl");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/main.jsonl"),
        &main,
    )
    .unwrap();
    stapel(hdir)
        .args(["tokens", "import"])
        .arg(&main)
        .args(["--role", "orchestrator", "--until", "2026-01-02T00:00:00Z"])
        .assert()
        .success();
    assert_eq!(report(hdir, &["ABC-1"]), golden("tokens_report.txt"));
}

#[test]
fn report_imported_row_edges() {
    // Hand-written imported records go straight into the target: the import command refuses
    // the two invalid ones.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let lines = [
        r#"{"v":1,"id":"e-1","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"builder","model":"claude-opus-5-5","source":"measured","input":4,"output":90,"imported_from":"in.jsonl"}"#,
        r#"{"v":1,"id":"e-2","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"builder","model":"claude-sonnet-5-5","source":"measured","input":7,"output":8}"#,
        r#"{"v":1,"id":"e-3","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"reviewer","model":"claude-opus-5-5","source":"measured","input":1,"output":2,"imported_from":"in.jsonl"}"#,
        r#"{"v":1,"id":"e-4","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"reviewer","model":"claude-opus-5-5","source":"measured","input":3,"output":4,"imported_from":"in.jsonl"}"#,
        r#"{"v":1,"id":"e-5","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"reviewer","model":"claude-opus-5-5","source":"estimate","estimate":10,"imported_from":"in.jsonl"}"#,
        r#"{"v":1,"id":"e-6","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"reviewer","model":"claude-opus-5-5","source":"estimate","estimate":20,"imported_from":"in.jsonl"}"#,
        r#"{"v":1,"id":"e-7","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"builder","model":"claude-opus-5-5","source":"measured","input":1000,"output":1000,"partial":0,"imported_from":"in.jsonl"}"#,
        r#"{"v":1,"id":"e-8","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"builder","model":"claude-opus-5-5","source":"other","input":2000,"output":2000,"imported_from":"in.jsonl"}"#,
    ];
    put(dir, TARGET, &(lines.join("\n") + "\n"));
    let out = stapel(dir)
        .args(["tokens", "ABC-1"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let (table, problems) = text.split_once("\n\n").unwrap();
    // The two invalid records give a problem line each and are left out of the sums.
    assert_eq!(
        problems,
        "problem: .stapel/tickets/ABC-1/tokens.jsonl:7\nproblem: .stapel/tickets/ABC-1/tokens.jsonl:8\n"
    );
    let rows: Vec<Vec<&str>> = table
        .lines()
        .map(|l| l.split_whitespace().collect())
        .collect();
    assert_eq!(
        rows,
        [
            vec!["ticket:", "ABC-1"],
            vec![
                "role", "model", "input", "output", "cache", "read", "cache", "write",
                "estimate", "legacy"
            ],
            // The imported row of opus sorts before the plain row of sonnet; the model is
            // the key before the origin.
            vec!["builder", "(file)", "claude-opus-5-5", "4", "90", "—", "—", "—", "—"],
            vec!["builder", "claude-sonnet-5-5", "7", "8", "—", "—", "—", "—"],
            // Two measured and two estimate records are summed in one row.
            vec!["reviewer", "(file)", "claude-opus-5-5", "4", "6", "—", "—", "30", "—"],
        ]
    );
}

#[test]
fn sessions_ignore_imported_without_transcript() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let lines = [
        r#"{"v":1,"id":"n-1","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"orchestrator","model":"claude-opus-5-5","source":"measured","output":300,"transcript":"s-sess","from":"2026-01-01T12:00:00.000Z","to":"2026-01-01T12:00:00.000Z","importer":2}"#,
        r#"{"v":1,"id":"n-2","at":"2026-01-02T00:00:00Z","ticket":"ABC-1","role":"builder","model":"claude-opus-5-5","source":"measured","input":4,"output":90,"imported_from":"in.jsonl"}"#,
    ];
    put(dir, TARGET, &(lines.join("\n") + "\n"));
    put(
        dir,
        ".stapel/sessions.jsonl",
        "{\"v\":1,\"id\":\"s-1\",\"at\":\"2026-01-02T00:00:00Z\",\"session\":\"s-sess\",\"start\":1767268800000,\"duration_ms\":120000,\"models\":{\"claude-opus-5-5\":{\"output\":800,\"thinking\":150}}}\n",
    );
    let out = stapel(dir)
        .args(["tokens"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    let rows: Vec<Vec<&str>> = text
        .lines()
        .filter(|l| l.starts_with("s-sess"))
        .map(|l| l.split_whitespace().collect())
        .collect();
    // Only the record with a transcript counts: the imported one adds nothing to the session.
    assert_eq!(
        rows,
        [["s-sess", "claude-opus-5-5", "800", "150", "300", "0", "0", "500"]]
    );
}

// ---- Step 3: the dialog, the grant, the own-grant denial (AC-6 to AC-9, AC-11) ----

const DENY: i32 = 2;
const TAIL: &str = "Answer Yes or No; do not choose \"don't ask again\".";
const MISMATCH: &str = "stapel: the grant does not match: the records, the ticket or the stapel build changed since the dialog, or the grant was used already; ask the agent to run the command again\n";
const GRANT_DENIED: &str = "a --grant comes only from the permission dialog; stapel ok, stapel close, stapel findings import, stapel ledger add and stapel tokens import-records may not carry one of their own or words filled in at run time";

/// The block `## name` of `golden/token_records_dialog.txt`: the reason, then the replaced
/// command with its token shown as `<token>`.
fn dialog_golden(name: &str) -> String {
    let text = golden("token_records_dialog.txt");
    let start = text.find(&format!("## {name}\n")).unwrap() + name.len() + 4;
    let end = text[start..].find("## ").map_or(text.len(), |i| start + i);
    text[start..end].to_string()
}

/// The dialog's answer as `reason\nreplaced command\n`, the token shown as `<token>`.
fn dialog_text(reason: &str, replaced: &str) -> String {
    let token = common::token_of(replaced);
    format!("{reason}\n{}\n", replaced.replace(&token, "<token>"))
}

fn grant_files(dir: &Path) -> usize {
    std::fs::read_dir(common::grants_dir(dir)).map_or(0, |d| d.count())
}

/// A command run in an agent's shell: `CLAUDECODE` is set.
fn agent(dir: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    stapel(dir).args(args).env("CLAUDECODE", "1").assert()
}

/// `n` measured records `r-0`, `r-1`, … as the lines of a file.
fn many(n: usize) -> String {
    (0..n).map(|i| line(&measured(&format!("r-{i}")))).collect()
}

#[test]
fn dialog_for_import() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut first = measured("m-1");
    first["cache_read"] = json!(3);
    first["cache_write"] = json!(2);
    first["step"] = json!("step 1");
    first["note"] = json!("first");
    let mut second = measured("m-2");
    second["input"] = json!(20);
    second["output"] = json!(7);
    let mut third = estimate("e-1");
    third["role"] = json!("reviewer");
    third["model"] = json!("m-2");
    put(
        dir,
        "in.jsonl",
        &format!("{}{}{}", line(&first), line(&second), line(&third)),
    );
    let before = std::fs::read_to_string(dir.join(".stapel/tickets/ABC-1/state.json")).unwrap();

    // The golden: reason and replaced command; the key is added; one grant file per dialog.
    let (reason, replaced) = common::ask(dir, "stapel tokens import-records in.jsonl");
    assert_eq!(
        dialog_text(&reason, &replaced),
        dialog_golden("import"),
        "{reason}"
    );
    assert_eq!(common::token_of(&replaced).len(), 32);
    assert_eq!(grant_files(dir), 1);
    // Nothing is written by the dialog.
    assert!(target_text(dir).is_none());
    assert_eq!(
        std::fs::read_to_string(dir.join(".stapel/tickets/ABC-1/state.json")).unwrap(),
        before
    );

    // A key in any case, the sub-action in any case, and the `cargo run` form as written.
    let (reason, replaced) = common::ask(dir, "stapel TOKENS Import-Records in.jsonl abc-1");
    assert!(reason.starts_with("stapel: import 3 token records from in.jsonl into ABC-1: "));
    assert!(replaced.starts_with("stapel tokens import-records in.jsonl ABC-1 --grant "));
    let (reason, replaced) = common::ask(
        dir,
        "cargo run -q -p stapel-cli -- tokens import-records in.jsonl",
    );
    assert!(
        reason.ends_with(&format!("; runs: cargo run -q -p stapel-cli --. {TAIL}")),
        "{reason}"
    );
    assert!(
        replaced.starts_with(
            "cargo run -q -p stapel-cli -- tokens import-records in.jsonl ABC-1 --grant "
        ),
        "{replaced}"
    );
    assert_eq!(grant_files(dir), 3);

    // No estimate record: the estimate sum is 0.
    put(dir, "m.jsonl", &line(&measured("x-1")));
    let (reason, _) = common::ask(dir, "stapel tokens import-records m.jsonl");
    assert_eq!(
        reason,
        format!(
            "stapel: import 1 token records from m.jsonl into ABC-1: measured input 10, output 5, cache read 0, cache write 0; estimate 0 | x-1 builder m-1 measured input 10 output 5 cache read — cache write — step — note —; runs: stapel. {TAIL}"
        )
    );
    // Only estimate records: the measured sums are 0.
    put(
        dir,
        "e.jsonl",
        &format!("{}{}", line(&estimate("y-1")), line(&estimate("y-2"))),
    );
    let (reason, _) = common::ask(dir, "stapel tokens import-records e.jsonl");
    assert!(
        reason.starts_with(
            "stapel: import 2 token records from e.jsonl into ABC-1: measured input 0, output 0, cache read 0, cache write 0; estimate 200 | y-1 builder m-1 estimate estimate 100 step — note — | y-2 "
        ),
        "{reason}"
    );

    // A closed ticket by its key: the reason says so.
    stapel(dir)
        .args(["close", "ABC-1", "--reason", "done"])
        .assert()
        .success();
    let (reason, _) = common::ask(dir, "stapel tokens import-records m.jsonl ABC-1");
    assert!(
        reason.starts_with("stapel: import 1 token records from m.jsonl into ABC-1 (closed): "),
        "{reason}"
    );
}

#[test]
fn dialog_cuts_the_note() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut records = Vec::new();
    let mut with_note = |id: &str, note: String| {
        let mut v = measured(id);
        v["note"] = json!(note);
        records.push(v);
    };
    with_note("n40", "n".repeat(40));
    with_note("n41", "n".repeat(41));
    with_note("c41", "я".repeat(41));
    with_note("nl", "a\nb".to_string());
    let mut whole = measured("whole");
    whole["role"] = json!("r".repeat(40));
    whole["model"] = json!("m".repeat(40));
    whole["step"] = json!("s".repeat(40));
    records.push(whole);
    let mut cut = measured("cut");
    cut["role"] = json!("r".repeat(41));
    cut["model"] = json!("m".repeat(41));
    cut["step"] = json!("s".repeat(41));
    records.push(cut);
    let text: String = records.iter().map(line).collect();
    // A tab in the base name is shown as a space (the word is quoted, so the tab stays).
    put(dir, "t\tab.jsonl", &text);
    let (reason, _) = common::ask(dir, "stapel tokens import-records 't\tab.jsonl'");
    assert!(
        reason.starts_with("stapel: import 6 token records from t ab.jsonl into ABC-1: "),
        "{reason}"
    );
    let counts = "measured input 10 output 5 cache read — cache write —";
    let entry = |id: &str, role: &str, model: &str, step: &str, note: &str| {
        format!("{id} {role} {model} {counts} step {step} note {note}")
    };
    let expected = [
        entry("n40", "builder", "m-1", "—", &"n".repeat(40)),
        entry("n41", "builder", "m-1", "—", &format!("{}…", "n".repeat(40))),
        entry("c41", "builder", "m-1", "—", &format!("{}…", "я".repeat(40))),
        entry("nl", "builder", "m-1", "—", "a b"),
        entry(
            "whole",
            &"r".repeat(40),
            &"m".repeat(40),
            &"s".repeat(40),
            "—",
        ),
        entry(
            "cut",
            &format!("{}…", "r".repeat(40)),
            &format!("{}…", "m".repeat(40)),
            &format!("{}…", "s".repeat(40)),
            "—",
        ),
    ]
    .join(" | ");
    assert!(
        reason.contains(&format!("| {expected}; runs: stapel.")),
        "{reason}"
    );
}

#[test]
fn guard_denies_what_the_command_refuses() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut bad = measured("b-1");
    bad.as_object_mut().unwrap().remove("role");
    put(dir, "bad.jsonl", &line(&bad));
    put(dir, "ok.jsonl", &line(&measured("o-1")));
    put(dir, "dup.jsonl", &line(&measured("o-1")));
    put(dir, "dir/x", "");

    // The guard gives the reason of the command, and writes no grant.
    let cases: [(&str, Vec<&str>); 4] = [
        (
            "stapel tokens import-records bad.jsonl",
            vec!["tokens", "import-records", "bad.jsonl"],
        ),
        (
            "stapel tokens import-records missing.jsonl",
            vec!["tokens", "import-records", "missing.jsonl"],
        ),
        (
            "stapel tokens import-records dir",
            vec!["tokens", "import-records", "dir"],
        ),
        (
            "stapel tokens import-records ok.jsonl NOPE-1",
            vec!["tokens", "import-records", "ok.jsonl", "NOPE-1"],
        ),
    ];
    for (command, args) in cases {
        let out = stapel(dir)
            .args(&args)
            .assert()
            .code(2)
            .get_output()
            .clone();
        let expected = String::from_utf8(out.stderr).unwrap();
        assert!(expected.starts_with("stapel: "), "{expected}");
        let got = common::hook_bash(dir, command, "auto")
            .code(DENY)
            .get_output()
            .clone();
        assert_eq!(
            String::from_utf8(got.stderr).unwrap(),
            expected,
            "{command}"
        );
        assert!(got.stdout.is_empty());
    }
    // An id that is already in the journal.
    put(dir, TARGET, &line(&measured("o-1")));
    let expected = refused(dir, &["ok.jsonl"]);
    common::hook_bash(dir, "stapel tokens import-records ok.jsonl", "auto")
        .code(DENY)
        .stdout("")
        .stderr(expected);
    std::fs::remove_file(dir.join(TARGET)).unwrap();
    assert_eq!(grant_files(dir), 0);

    // Modes without a dialog deny.
    for mode in ["bypassPermissions", "dontAsk", ""] {
        common::hook_bash(dir, "stapel tokens import-records ok.jsonl", mode)
            .code(DENY)
            .stderr(predicates::str::contains("dialog"));
    }
    assert_eq!(grant_files(dir), 0);

    // A relative `FILE` is read from the hook's `cwd`; from the root it is a missing file.
    put(dir, "sub/in.jsonl", &line(&measured("s-1")));
    let hook = |cwd: &Path| {
        let payload = json!({
            "tool_name": "Bash",
            "cwd": cwd,
            "permission_mode": "auto",
            "tool_input": { "command": "stapel tokens import-records in.jsonl", "description": "test" },
        });
        stapel(dir)
            .args(["hook", "pre-tool-use"])
            .write_stdin(payload.to_string())
            .assert()
    };
    let out = hook(&dir.join("sub")).success().get_output().stdout.clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    let h = &v["hookSpecificOutput"];
    assert_eq!(h["permissionDecision"], "ask", "{v}");
    assert!(
        h["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .starts_with("stapel: import 1 token records from in.jsonl into ABC-1:"),
        "{v}"
    );
    assert!(
        h["updatedInput"]["command"]
            .as_str()
            .unwrap()
            .starts_with("stapel tokens import-records in.jsonl ABC-1 --grant "),
        "{v}"
    );
    assert_eq!(grant_files(dir), 1);
    let err = String::from_utf8(hook(dir).code(DENY).get_output().stderr.clone()).unwrap();
    assert!(err.starts_with("stapel: in.jsonl: "), "{err}");
    assert_eq!(grant_files(dir), 1);

    // An allow rule that would skip the dialog denies, and writes no new grant.
    put(
        dir,
        ".claude/settings.local.json",
        r#"{"permissions":{"allow":["Bash(stapel tokens:*)"]}}"#,
    );
    common::hook_bash(dir, "stapel tokens import-records ok.jsonl", "auto")
        .code(DENY)
        .stderr(predicates::str::contains("allow rule"));
    assert_eq!(grant_files(dir), 1);
    std::fs::remove_file(dir.join(".claude/settings.local.json")).unwrap();

    // A broken configuration: the reason of the command.
    put(dir, ".stapel/stapel.toml", "this is = [not toml\n");
    let out = stapel(dir)
        .args(["tokens", "import-records", "ok.jsonl"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let expected = String::from_utf8(out.stderr).unwrap();
    assert!(
        expected.starts_with("stapel: .stapel/stapel.toml: "),
        "{expected}"
    );
    common::hook_bash(dir, "stapel tokens import-records ok.jsonl", "auto")
        .code(DENY)
        .stdout("")
        .stderr(expected);
    assert_eq!(grant_files(dir), 1);
}

#[test]
fn dialog_limits_to_30_records() {
    let repo = repo_with_ticket();
    let dir = repo.path();

    // 31 records: a denial with the exact text, no grant.
    put(dir, "in.jsonl", &many(31));
    common::hook_bash(dir, "stapel tokens import-records in.jsonl", "auto")
        .code(DENY)
        .stdout("")
        .stderr(
            "stapel: the permission dialog shows at most 30 token records; split the file into parts of at most 30 records\n",
        );
    assert_eq!(grant_files(dir), 0);

    // 30 records ask.
    put(dir, "in.jsonl", &many(30));
    let (reason, _) = common::ask(dir, "stapel tokens import-records in.jsonl");
    assert!(
        reason.starts_with("stapel: import 30 token records from in.jsonl into ABC-1:"),
        "{reason}"
    );
    assert_eq!(grant_files(dir), 1);

    // A person in a terminal imports 31.
    put(dir, "in.jsonl", &many(31));
    run(dir, &["in.jsonl"])
        .success()
        .stdout("imported: 31 token records into ABC-1\n");
}

#[test]
fn dialog_usage_denials() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let usage = "usage: stapel tokens import-records FILE [KEY]";
    let cases = [
        (
            "stapel tokens import-records --role x",
            "unexpected option --role for stapel tokens import-records",
        ),
        (
            "stapel tokens import-records in.jsonl --note=y",
            "unexpected option --note=y for stapel tokens import-records",
        ),
        ("stapel tokens import-records a b c", usage),
        ("stapel tokens import-records", usage),
    ];
    put(dir, "in.jsonl", &many(1));
    for (command, reason) in cases {
        common::hook_bash(dir, command, "auto")
            .code(DENY)
            .stdout("")
            .stderr(format!("stapel: {reason}\n"));
    }
    assert_eq!(grant_files(dir), 0);
}

#[test]
fn grant_writes_once() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &many(2));

    // Written once; the grant is gone; the same token then meets the check of the repeated id
    // first (the checks run before the grant is used).
    let (_, replaced) = common::ask(dir, "stapel tokens import-records in.jsonl");
    let token = common::token_of(&replaced);
    let args = [
        "tokens",
        "import-records",
        "in.jsonl",
        "ABC-1",
        "--grant",
        &token,
    ];
    agent(dir, &args)
        .success()
        .stdout("imported: 2 token records into ABC-1\n");
    let written = target_text(dir).unwrap();
    assert_eq!(written.lines().count(), 2, "{written}");
    assert!(written.contains("\"id\":\"r-0\"") && written.contains("\"id\":\"r-1\""));
    assert!(written.contains("\"imported_from\":\"in.jsonl\""));
    assert_eq!(grant_files(dir), 0);
    agent(dir, &args)
        .code(2)
        .stderr(predicates::str::contains("id r-0 is already in"))
        .stdout("");
    assert_eq!(target_text(dir).unwrap(), written);
    // With the records gone from the target, the checks pass and the used grant does not match.
    put(dir, TARGET, "");
    agent(dir, &args).code(1).stderr(MISMATCH).stdout("");
    assert_eq!(target_text(dir).unwrap(), "");
}

#[test]
fn grant_survives_a_failed_check() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &many(2));
    let (_, replaced) = common::ask(dir, "stapel tokens import-records in.jsonl");
    let token = common::token_of(&replaced);
    let args = [
        "tokens",
        "import-records",
        "in.jsonl",
        "ABC-1",
        "--grant",
        &token,
    ];

    // An id of `FILE` added to the journal after the dialog: a refusal of the checks, the grant
    // stays.
    let other = "{\"v\":1,\"id\":\"r-1\",\"at\":\"2026-01-01T00:00:00Z\",\"ticket\":\"ABC-1\",\"role\":\"x\",\"model\":\"m\",\"source\":\"estimate\",\"estimate\":1}\n";
    put(dir, TARGET, other);
    agent(dir, &args)
        .code(2)
        .stderr(predicates::str::contains("id r-1 is already in"))
        .stdout("");
    assert_eq!(target_text(dir).unwrap(), other);
    assert_eq!(grant_files(dir), 1);

    // With that line gone the same grant writes once.
    std::fs::remove_file(dir.join(TARGET)).unwrap();
    agent(dir, &args)
        .success()
        .stdout("imported: 2 token records into ABC-1\n");
    assert_eq!(target_text(dir).unwrap().lines().count(), 2);
    assert_eq!(grant_files(dir), 0);
}

#[test]
fn grant_names_the_records() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["new", "Second"]).assert().success();
    put(
        dir,
        "free.jsonl",
        &line(&json!({
            "id": "F-1", "ticket": "ABC-1", "stage": "code-review", "round": 1,
            "role": "reviewer", "severity": "HIGH", "promise": "inside", "claim": "a claim",
            "catchable_at": "code", "fate": "kept",
        })),
    );
    stapel(dir)
        .args(["findings", "import", "free.jsonl", "ABC-1"])
        .assert()
        .code(2)
        .stdout("")
        .stderr("stapel: free.jsonl:1: fate: must be one of open, patch, bad_plan, intent_gap, defer, no-change\n");
    assert!(!dir.join(".stapel/tickets/ABC-1/findings.jsonl").exists());
    put(dir, "in.jsonl", &many(1));
    let import = |file: &str, key: &str, token: &str| {
        agent(
            dir,
            &["tokens", "import-records", file, key, "--grant", token],
        )
    };
    let token = |command: &str| common::token_of(&common::ask(dir, command).1);
    let nothing_written = || {
        assert!(target_text(dir).is_none());
        assert!(!dir.join(".stapel/tickets/ABC-2/tokens.jsonl").exists());
    };

    // `FILE` edited in a count after the dialog.
    let t = token("stapel tokens import-records in.jsonl ABC-1");
    let mut edited = measured("r-0");
    edited["output"] = json!(6);
    put(dir, "in.jsonl", &line(&edited));
    import("in.jsonl", "ABC-1", &t).code(1).stderr(MISMATCH);
    nothing_written();

    // `FILE` copied to another base name.
    put(dir, "in.jsonl", &many(1));
    put(dir, "copy.jsonl", &many(1));
    let t = token("stapel tokens import-records in.jsonl ABC-1");
    import("copy.jsonl", "ABC-1", &t).code(1).stderr(MISMATCH);
    nothing_written();

    // Another key, with records that carry no `ticket`.
    let t = token("stapel tokens import-records in.jsonl ABC-1");
    import("in.jsonl", "ABC-2", &t).code(1).stderr(MISMATCH);
    nothing_written();

    // A grant of `ledger add` and of `ok` on the import command, and a grant of the import command
    // on `findings import`, which has no `--grant` option.
    put(
        dir,
        "f.jsonl",
        &line(&json!({
            "id": "F-1", "ticket": "ABC-1", "stage": "code-review", "round": 1,
            "role": "reviewer", "severity": "HIGH", "promise": "inside", "claim": "a claim",
            "catchable_at": "code", "fate": "patch",
        })),
    );
    common::hook_bash(dir, "stapel ledger add --kind finding --note x ABC-1", "auto")
        .success()
        .stdout("");
    let t = token("stapel ledger add --kind decision --note x ABC-1");
    import("in.jsonl", "ABC-1", &t).code(1).stderr(MISMATCH);
    let t = token("stapel ok ABC-1 spec");
    import("in.jsonl", "ABC-1", &t)
        .code(1)
        .stderr(predicates::str::contains("the grant does not match"));
    nothing_written();
    let t = token("stapel tokens import-records in.jsonl ABC-1");
    agent(dir, &["findings", "import", "f.jsonl", "ABC-1", "--grant", &t]).code(2);
    assert!(!dir.join(".stapel/tickets/ABC-1/findings.jsonl").exists());

    // Used twice: the first run writes, the second (with the records gone) does not match.
    let t = token("stapel tokens import-records in.jsonl ABC-1");
    import("in.jsonl", "ABC-1", &t).success();
    assert_eq!(target_text(dir).unwrap().lines().count(), 1);
    put(dir, TARGET, "");
    import("in.jsonl", "ABC-1", &t).code(1).stderr(MISMATCH);
    assert_eq!(target_text(dir).unwrap(), "");
    std::fs::remove_file(dir.join(TARGET)).unwrap();

    // An expired grant.
    let t = token("stapel tokens import-records in.jsonl ABC-1");
    for entry in std::fs::read_dir(common::grants_dir(dir)).unwrap() {
        std::fs::write(entry.unwrap().path(), r#"{"expires_at":1}"#).unwrap();
    }
    import("in.jsonl", "ABC-1", &t)
        .code(1)
        .stderr("stapel: the grant expired; ask the agent to run the command again\n");
    nothing_written();
    assert_eq!(grant_files(dir), 0);
}

#[test]
fn guard_denies_own_grant() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "f", "x");
    for command in [
        "stapel tokens import-records in.jsonl --grant abc",
        "stapel tokens import-records in.jsonl ABC-1 --grant=abc",
        "stapel TOKENS IMPORT-RECORDS in.jsonl --grant x",
        "cargo run -q -p stapel-cli -- tokens import-records in.jsonl --grant abc",
        "stapel tokens import-records $(cat f)",
        "stapel tokens import-records in.jsonl $KEY",
        "stapel findings import in.jsonl --grant abc",
        "stapel ledger add --kind finding --note x --grant abc",
        "stapel ok spec --grant abc",
        "stapel close --reason done --grant abc",
    ] {
        common::hook_bash(dir, command, "auto")
            .code(DENY)
            .stdout("")
            .stderr(format!("stapel: {GRANT_DENIED}\n"));
    }
    // The other tokens commands are not read by this rule.
    for command in [
        "stapel tokens import t.jsonl --role r $x",
        "stapel tokens add --role r --estimate 1 --note $x",
    ] {
        common::hook_bash(dir, command, "auto")
            .success()
            .stdout("")
            .stderr("");
    }
    assert_eq!(grant_files(dir), 0);
}

// ---- Step 3c: coverage ----

#[test]
fn agent_shell_without_grant_refused() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &line(&measured("a-1")));
    let before = common::snapshot(dir);
    let refusal = "stapel: only a person imports token records: this shell belongs to an agent (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); run the command as one plain command so that Claude Code shows the permission dialog, or, from a Zed task started from a Claude Code shell, run it with env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT\n";
    for var in ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"] {
        for args in [vec!["in.jsonl"], vec!["in.jsonl", "ABC-1"]] {
            stapel(dir)
                .args(["tokens", "import-records"])
                .args(&args)
                .env(var, "1")
                .assert()
                .code(1)
                .stdout("")
                .stderr(refusal);
        }
    }
    assert_eq!(common::snapshot(dir), before);
}

#[test]
fn other_tokens_commands_stay_allowed() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for command in [
        "stapel tokens add --role r --estimate 1",
        "stapel tokens import t.jsonl --role r",
        "stapel tokens",
        "stapel tokens ABC-1",
    ] {
        common::hook_bash(dir, command, "auto")
            .success()
            .stdout("")
            .stderr("");
    }
    assert!(!common::grants_dir(dir).exists());
}

// ---- Step 3d: coverage ----

#[test]
fn dialog_text_edges() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    // A measured record with `cache_read` only.
    let mut cache = measured("c-1");
    cache["cache_read"] = json!(3);
    // A note with a newline (the only control character the records check lets through).
    let mut control = measured("k-1");
    control["note"] = json!("a\nb");
    // A Cyrillic note of exactly 40 characters (80 bytes) stays whole.
    let mut cyr_note = measured("n-1");
    cyr_note["note"] = json!("я".repeat(40));
    // A Cyrillic role of 41 characters is cut at 40.
    let mut cyr_role = measured("r-1");
    cyr_role["role"] = json!("ы".repeat(41));
    let text: String = [cache, control, cyr_note, cyr_role].iter().map(line).collect();
    // A tab, a carriage return and an escape character in the base name: one space each.
    put(dir, "a\tb\rc\u{1b}d.jsonl", &text);
    let (reason, _) = common::ask(
        dir,
        "stapel tokens import-records 'a\tb\rc\u{1b}d.jsonl'",
    );
    let expected = format!(
        "stapel: import 4 token records from a b c d.jsonl into ABC-1: measured input 40, output 20, cache read 3, cache write 0; estimate 0 | c-1 builder m-1 measured input 10 output 5 cache read 3 cache write — step — note — | k-1 builder m-1 measured input 10 output 5 cache read — cache write — step — note a b | n-1 builder m-1 measured input 10 output 5 cache read — cache write — step — note {} | r-1 {}… m-1 measured input 10 output 5 cache read — cache write — step — note —; runs: stapel. {TAIL}",
        "я".repeat(40),
        "ы".repeat(40),
    );
    assert_eq!(reason, expected);
}

#[test]
fn grants_of_other_commands_do_not_cross() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &many(1));
    let token = |command: &str| common::token_of(&common::ask(dir, command).1);
    let state = || std::fs::read_to_string(dir.join(".stapel/tickets/ABC-1/state.json")).unwrap();
    let before = state();
    let nothing_written = || {
        assert!(target_text(dir).is_none());
        assert!(!dir.join(".stapel/tickets/ABC-1/ledger.jsonl").exists());
        assert_eq!(state(), before);
    };

    // A grant of the import command on `ledger add`, `ok` and `close`.
    let t = token("stapel tokens import-records in.jsonl ABC-1");
    agent(
        dir,
        &["ledger", "add", "--kind", "finding", "--note", "x", "ABC-1", "--grant", &t],
    )
    .code(1)
    .stderr(MISMATCH);
    nothing_written();
    let t = token("stapel tokens import-records in.jsonl ABC-1");
    agent(dir, &["ok", "ABC-1", "spec", "--grant", &t])
        .code(1)
        .stderr("stapel: the grant does not match: the section, a dependency or the stapel build changed since the dialog, or the grant was used already; ask the agent to run the command again\n");
    nothing_written();
    let t = token("stapel tokens import-records in.jsonl ABC-1");
    agent(
        dir,
        &["close", "ABC-1", "--reason", "done", "--grant", &t],
    )
    .code(1)
    .stderr("stapel: the grant does not match: the section, a dependency or the stapel build changed since the dialog, or the grant was used already; ask the agent to run the command again\n");
    nothing_written();

    // A grant of `close` on the import command.
    let t = token("stapel close ABC-1 --reason done");
    agent(
        dir,
        &["tokens", "import-records", "in.jsonl", "ABC-1", "--grant", &t],
    )
    .code(1)
    .stderr(MISMATCH);
    nothing_written();
}

#[test]
fn own_grant_edges() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for command in [
        "stapel tokens import-records in.jsonl --grant x; true",
        "stapel tokens import-records a$b",
    ] {
        common::hook_bash(dir, command, "auto")
            .code(DENY)
            .stdout("")
            .stderr(format!("stapel: {GRANT_DENIED}\n"));
    }
    // Removing CLAUDECODE is denied by the rule of the pre-push layer.
    common::hook_bash(
        dir,
        "env -u CLAUDECODE stapel tokens import-records in.jsonl",
        "auto",
    )
    .code(DENY)
    .stdout("")
    .stderr("stapel: the command may switch off a second layer of protection (the git pre-push hook, or the Claude Code variables the hooks rely on); agents may not do that. To read a hook, use the Read tool\n");
    // `tokens add` is not read by the own-grant rule.
    common::hook_bash(
        dir,
        "stapel tokens add --role r --estimate 1 --grant x",
        "auto",
    )
    .success()
    .stdout("")
    .stderr("");
    assert_eq!(grant_files(dir), 0);
}

// ---- Step 4c: coverage after code review round 1 (DR-1, R-4, E-7, E-1, F-8) ----

#[test]
fn ticket_and_v_types() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    // `ticket` of the wrong type, and `v` that is a number but not the integer 1.
    let cases: [(&str, Value, &str); 3] = [
        ("ticket", json!(7), "stapel: in.jsonl:1: ticket: must be ABC-1\n"),
        ("ticket", Value::Null, "stapel: in.jsonl:1: ticket: must be ABC-1\n"),
        ("v", json!(1.0), "stapel: in.jsonl:1: v: must be 1\n"),
    ];
    for (field, value, expected) in cases {
        let mut record = measured("T-1");
        record[field] = value.clone();
        put(dir, "in.jsonl", &line(&record));
        assert_eq!(refused(dir, &["in.jsonl"]), expected, "{field} {value}");
        assert!(target_text(dir).is_none(), "{field} {value}: wrote something");
    }
}

#[test]
fn grant_binds_text_not_line_endings() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let run_with = |token: &str| {
        agent(
            dir,
            &["tokens", "import-records", "in.jsonl", "ABC-1", "--grant", token],
        )
    };
    let text = many(2);
    put(dir, "in.jsonl", &text);
    let (_, replaced) = common::ask(dir, "stapel tokens import-records in.jsonl");
    let token = common::token_of(&replaced);

    // The file turned to CRLF with a blank line added: the same records, so the grant holds.
    let crlf = format!("{}\r\n\r\n", text.trim_end().replace('\n', "\r\n"));
    put(dir, "in.jsonl", &crlf);
    run_with(&token)
        .success()
        .stdout("imported: 2 token records into ABC-1\n");
    let written = target_text(dir).unwrap();
    assert_eq!(written.lines().count(), 2, "{written}");
    assert!(written.contains("\"id\":\"r-0\"") && written.contains("\"id\":\"r-1\""));
    assert_eq!(grant_files(dir), 0);

    // After another dialog, a changed `step` or `note` is another text: the grant does not match.
    for (field, value) in [("step", "changed"), ("note", "changed")] {
        put(dir, TARGET, "");
        put(dir, "in.jsonl", &text);
        let (_, replaced) = common::ask(dir, "stapel tokens import-records in.jsonl");
        let token = common::token_of(&replaced);
        let mut first = measured("r-0");
        first[field] = json!(value);
        put(dir, "in.jsonl", &format!("{}{}", line(&first), line(&measured("r-1"))));
        run_with(&token).code(1).stderr(MISMATCH).stdout("");
        assert_eq!(target_text(dir).unwrap(), "", "{field}");
    }
}

#[test]
fn foreign_grant_leaves_the_ledger() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &many(1));
    let ledger = ".stapel/process-ledger.jsonl";
    let first = "{\"v\":1,\"at\":\"2026-01-01T00:00:00Z\",\"kind\":\"catches\",\"ticket\":\"ABC-1\",\"note\":\"item 3 caught it\"}\n";
    put(dir, ledger, first);
    let (_, replaced) = common::ask(dir, "stapel tokens import-records in.jsonl ABC-1");
    let token = common::token_of(&replaced);
    agent(
        dir,
        &["ledger", "add", "--kind", "finding", "--note", "x", "ABC-1", "--grant", &token],
    )
    .code(1)
    .stderr(MISMATCH)
    .stdout("");
    assert_eq!(std::fs::read(dir.join(ledger)).unwrap(), first.as_bytes());
    assert!(target_text(dir).is_none());
}

#[test]
fn allow_rule_forms() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &many(1));
    let settings = ".claude/settings.local.json";
    let allow = |rule: &str| {
        put(
            dir,
            settings,
            &json!({"permissions": {"allow": [rule]}}).to_string(),
        );
    };
    // These rules do not skip the dialog of `import-records`: it asks.
    for rule in ["Bash(stapel tokens import:*)", "Bash(stapel tokens import *)"] {
        allow(rule);
        let before = grant_files(dir);
        let (reason, replaced) = common::ask(dir, "stapel tokens import-records in.jsonl");
        assert!(reason.starts_with("stapel: import 1 token records from in.jsonl"), "{rule}");
        assert!(replaced.contains(" --grant "), "{rule}");
        assert_eq!(grant_files(dir), before + 1, "{rule}");
    }
    // These rules would skip the dialog: denied, and no grant file is written.
    let before = grant_files(dir);
    for rule in ["Bash(stapel tokens import*)", "Bash(stapel tokens import-records:*)"] {
        allow(rule);
        common::hook_bash(dir, "stapel tokens import-records in.jsonl", "auto")
            .code(DENY)
            .stdout("")
            .stderr(predicates::str::contains("allow rule"));
        assert_eq!(grant_files(dir), before, "{rule}");
    }
}
