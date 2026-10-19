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
