//! STP-18: machine records through stapel. Step 1: `stapel findings import` in a terminal.

mod common;

use common::{repo_with_ticket, stapel};
use serde_json::{Value, json};
use std::path::Path;

const TARGET: &str = ".stapel/tickets/ABC-1/findings.jsonl";

fn golden(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name),
    )
    .unwrap()
}

fn base() -> Value {
    json!({
        "id": "F-1",
        "ticket": "ABC-1",
        "stage": "code-review",
        "round": 1,
        "role": "reviewer",
        "severity": "HIGH",
        "promise": "inside",
        "claim": "a claim",
        "catchable_at": "code",
        "fate": "fixed",
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

/// Runs `findings import` and expects exit 2, an empty stdout and the stderr returned.
fn refused(dir: &Path, args: &[&str]) -> String {
    let out = stapel(dir)
        .arg("findings")
        .arg("import")
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

fn target_text(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(TARGET)).ok()
}

// ---- AC-1 ----

#[test]
fn import_appends_all_in_order() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let old = "{\"id\":\"OLD-1\",\"round\":\"1\"}\n";
    put(dir, TARGET, old);
    // Odd spacing, an unusual key order and extra fields must survive as given.
    let one = r#"{"id": "F-1", "note": "n1", "ticket": "ABC-1", "stage": "spec-review", "round": "2", "role": "external", "severity": "LOW", "promise": "outside", "claim": "first", "catchable_at": "spec", "fate": "kept", "step": 3}"#;
    let two = line(&{
        let mut v = base();
        v["id"] = json!("F-2");
        v["rule"] = json!("item 3");
        v["v"] = json!(1);
        v
    });
    let three = line(&{
        let mut v = base();
        v["id"] = json!("F-3");
        v["note"] = json!("n3");
        v
    });
    put(dir, "in.jsonl", &format!("{one}\n\n{two}   \n{three}\n"));
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 3 findings into ABC-1\n");
    let expect = format!("{old}{one}\n{}\n{}\n", two.trim_end(), three.trim_end());
    assert_eq!(target_text(dir).unwrap(), expect);
}

#[test]
fn import_creates_and_repairs_target() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &line(&base()));
    assert!(target_text(dir).is_none());
    stapel(dir)
        .args(["findings", "import", "in.jsonl", "ABC-1"])
        .assert()
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), line(&base()));

    // A target cut short gets its newline before the append.
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, TARGET, "{\"id\":\"OLD-1\"}");
    put(dir, "in.jsonl", &line(&base()));
    stapel(dir)
        .args(["findings", "import", "in.jsonl", "abc-1"])
        .assert()
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
    assert_eq!(
        target_text(dir).unwrap(),
        format!("{{\"id\":\"OLD-1\"}}\n{}", line(&base()))
    );
}

// ---- AC-2 ----

#[test]
fn import_refuses_bad_fields() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut cases: Vec<(String, Value)> = Vec::new();
    for field in [
        "id",
        "ticket",
        "stage",
        "round",
        "role",
        "severity",
        "promise",
        "claim",
        "catchable_at",
        "fate",
    ] {
        let mut v = base();
        v.as_object_mut().unwrap().remove(field);
        cases.push((format!("missing {field}"), v));
    }
    let set = |field: &str, value: Value| {
        let mut v = base();
        let name = format!("{field} = {value}");
        v[field] = value;
        (name, v)
    };
    for (field, value) in [
        ("id", json!("")),
        ("id", json!(7)),
        ("id", Value::Null),
        ("claim", json!("")),
        ("round", json!(true)),
        ("round", json!("")),
        ("round", json!(-1)),
        ("round", json!(1.5)),
        ("severity", json!("high")),
        ("promise", json!("both")),
        ("catchable_at", json!("later")),
        ("stage", json!("spec-review-2")),
        ("role", json!("nobody")),
        ("ticket", json!("ABC-2")),
        ("rule", json!("")),
        ("fate", json!("")),
        ("v", json!(2)),
        ("v", json!("1")),
    ] {
        cases.push(set(field, value));
    }
    let mut got = String::new();
    for (name, record) in &cases {
        put(dir, "in.jsonl", &line(record));
        let err = refused(dir, &["in.jsonl"]);
        assert!(err.starts_with("stapel: in.jsonl:1: "), "{name}: {err}");
        got.push_str(&err);
        assert!(target_text(dir).is_none(), "{name}: wrote something");
    }
    // The first bad record is named by its line, blank lines counted.
    let mut bad = base();
    bad["id"] = json!("F-2");
    bad["severity"] = json!("high");
    put(
        dir,
        "in.jsonl",
        &format!("{}\n{}", line(&base()), line(&bad)),
    );
    got.push_str(&refused(dir, &["in.jsonl"]));
    assert!(target_text(dir).is_none());
    assert_eq!(got, golden("records_refusals.txt"));
}

// ---- AC-3 ----

#[test]
fn import_refuses_bad_file() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let one = line(&base());
    let many: String = (1..=101)
        .map(|n| {
            let mut v = base();
            v["id"] = json!(format!("F-{n}"));
            line(&v)
        })
        .collect();
    let twice = format!("{one}{one}");
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
            many,
            "stapel: in.jsonl:101: more than 100 records; split the file\n".into(),
        ),
        (
            " ".repeat(256 * 1024 + 1),
            "stapel: in.jsonl: more than 256 KiB; split the file\n".into(),
        ),
        (twice, "stapel: in.jsonl:2: id F-1 repeats line 1\n".into()),
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
    // A legacy line (no `v`, an id) and a record with `v: 1`.
    put(
        dir,
        TARGET,
        "{\"id\":\"F-1\",\"round\":\"1\"}\n{\"v\":1,\"id\":\"F-9\"}\n",
    );
    put(dir, "in.jsonl", &line(&base()));
    let err = refused(dir, &["in.jsonl"]);
    assert_eq!(
        err,
        format!("stapel: in.jsonl:1: id F-1 is already in {TARGET}\n")
    );
    let mut other = base();
    other["id"] = json!("F-9");
    put(dir, "in.jsonl", &line(&other));
    let err = refused(dir, &["in.jsonl"]);
    assert_eq!(
        err,
        format!("stapel: in.jsonl:1: id F-9 is already in {TARGET}\n")
    );
    assert_eq!(
        target_text(dir).unwrap(),
        "{\"id\":\"F-1\",\"round\":\"1\"}\n{\"v\":1,\"id\":\"F-9\"}\n"
    );
}

// ---- AC-4 ----

#[test]
fn import_refuses_bad_input_path() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["new", "Second"]).assert().success();
    let before_state = std::fs::read(dir.join(".stapel/tickets/ABC-2/state.json")).unwrap();
    put(dir, TARGET, &line(&json!({"id": "OLD-1"})));
    let before_target = target_text(dir).unwrap();
    put(dir, ".stapel/process-ledger.jsonl", &line(&base()));
    put(dir, ".STAPEL/tickets/ABC-1/FINDINGS.jsonl", &line(&base()));
    std::fs::create_dir(dir.join("sub")).unwrap();
    std::fs::write(dir.join("latin.jsonl"), b"{\"claim\":\"caf\xe9\"}\n").unwrap();
    #[cfg(unix)]
    {
        let status = std::process::Command::new("mkfifo")
            .arg(dir.join("pipe"))
            .status()
            .unwrap();
        assert!(status.success());
        std::os::unix::fs::symlink(
            dir.join(".stapel/tickets/ABC-2/state.json"),
            dir.join("link-state.jsonl"),
        )
        .unwrap();
    }
    let cases: Vec<(&str, &str)> = vec![
        ("nope.jsonl", "stapel: nope.jsonl: "),
        ("sub", "stapel: sub: not a regular file\n"),
        ("-", "stapel: -: stdin is not accepted; give a file\n"),
        ("latin.jsonl", "stapel: latin.jsonl: not UTF-8\n"),
        (
            TARGET,
            "stapel: .stapel/tickets/ABC-1/findings.jsonl: a machine file; it cannot be imported\n",
        ),
        (
            ".stapel/tickets/ABC-2/state.json",
            "stapel: .stapel/tickets/ABC-2/state.json: a machine file; it cannot be imported\n",
        ),
        (
            ".stapel/process-ledger.jsonl",
            "stapel: .stapel/process-ledger.jsonl: a machine file; it cannot be imported\n",
        ),
        (
            ".STAPEL/tickets/ABC-1/FINDINGS.jsonl",
            "stapel: .STAPEL/tickets/ABC-1/FINDINGS.jsonl: a machine file; it cannot be imported\n",
        ),
        #[cfg(unix)]
        ("pipe", "stapel: pipe: not a regular file\n"),
        #[cfg(unix)]
        (
            "link-state.jsonl",
            "stapel: link-state.jsonl: a machine file; it cannot be imported\n",
        ),
    ];
    for (file, want) in cases {
        let out = stapel(dir)
            .args(["findings", "import", file, "ABC-1"])
            .timeout(std::time::Duration::from_secs(5))
            .assert()
            .code(2)
            .get_output()
            .clone();
        assert!(out.stdout.is_empty(), "{file}");
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(err.starts_with(want), "{file}: want {want:?}, got {err:?}");
        assert_eq!(target_text(dir).unwrap(), before_target, "{file}");
    }
    assert_eq!(
        std::fs::read(dir.join(".stapel/tickets/ABC-2/state.json")).unwrap(),
        before_state
    );

    // A symlink to a regular file is read.
    #[cfg(unix)]
    {
        let mut v = base();
        v["id"] = json!("F-7");
        put(dir, "real.jsonl", &line(&v));
        std::os::unix::fs::symlink(dir.join("real.jsonl"), dir.join("alias.jsonl")).unwrap();
        stapel(dir)
            .args(["findings", "import", "alias.jsonl", "ABC-1"])
            .assert()
            .success()
            .stdout("imported: 1 findings into ABC-1\n");
        assert_eq!(
            target_text(dir).unwrap(),
            format!("{before_target}{}", line(&v))
        );
    }
}

#[test]
fn import_refuses_broken_target() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &line(&base()));
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
}

// ---- AC-3, step 1c: the accepted limits ----

#[test]
fn import_accepts_the_limits() {
    /// A record line of `total` bytes with its newline, padded through `claim`.
    fn padded(id: &str, total: usize) -> String {
        let mut v = base();
        v["id"] = json!(id);
        let short = format!("{v}\n").len();
        v["claim"] = json!("x".repeat(total - short + "a claim".len()));
        let text = format!("{v}\n");
        assert_eq!(text.len(), total);
        text
    }

    // Exactly 100 records.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let hundred: String = (1..=100)
        .map(|n| {
            let mut v = base();
            v["id"] = json!(format!("F-{n}"));
            line(&v)
        })
        .collect();
    put(dir, "in.jsonl", &hundred);
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 100 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), hundred);

    // A line of exactly 64 KiB, newline included.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let big = padded("F-1", 64 * 1024);
    put(dir, "in.jsonl", &big);
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), big);

    // A file of exactly 256 KiB: four lines of 64 KiB.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let file: String = (1..=4)
        .map(|n| padded(&format!("F-{n}"), 64 * 1024))
        .collect();
    assert_eq!(file.len(), 256 * 1024);
    put(dir, "in.jsonl", &file);
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 4 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), file);
}

// ---- Step 2: `ledger add`, the ledger as a machine file ----

const LEDGER: &str = ".stapel/process-ledger.jsonl";

/// Today's date in UTC, `YYYY-MM-DD`.
fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn ledger_text(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join(LEDGER)).ok()
}

/// Runs `ledger add` with the given arguments and expects exit 2, an empty stdout and the stderr
/// returned.
fn ledger_refused(dir: &Path, args: &[&str]) -> String {
    let out = stapel(dir)
        .args(["ledger", "add"])
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

// ---- AC-5 ----

#[test]
fn ledger_add_appends_one_record() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let at = today_utc();
    assert!(ledger_text(dir).is_none());
    stapel(dir)
        .args([
            "ledger",
            "add",
            "--kind",
            "catches",
            "--note",
            "item 3 caught it",
        ])
        .assert()
        .success()
        .stdout("recorded: catches for ABC-1\n");
    let first = format!(
        "{{\"v\":1,\"at\":\"{at}\",\"kind\":\"catches\",\"ticket\":\"ABC-1\",\"note\":\"item 3 caught it\"}}\n"
    );
    assert_eq!(ledger_text(dir).unwrap(), first);

    // A ledger cut short gets its newline first; the key may have any case; the note is kept as
    // given.
    put(dir, LEDGER, first.trim_end());
    stapel(dir)
        .args([
            "ledger",
            "add",
            "--kind",
            "decision",
            "--note",
            " Caf\u{e9} \"q\" \\ ",
            "abc-1",
        ])
        .assert()
        .success()
        .stdout("recorded: decision for ABC-1\n");
    let second = format!(
        "{{\"v\":1,\"at\":\"{at}\",\"kind\":\"decision\",\"ticket\":\"ABC-1\",\"note\":\" Caf\u{e9} \\\"q\\\" \\\\ \"}}\n"
    );
    assert_eq!(ledger_text(dir).unwrap(), format!("{first}{second}"));

    stapel(dir)
        .args(["ledger", "add", "--note", "n", "--kind", "finding", "ABC-1"])
        .assert()
        .success()
        .stdout("recorded: finding for ABC-1\n");
    assert_eq!(ledger_text(dir).unwrap().lines().count(), 3);
}

// ---- AC-6 ----

#[test]
fn ledger_add_refusals() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for (kind, want) in [
        (
            "Finding",
            "stapel: --kind must be one of finding, catches, decision\n",
        ),
        (
            "other",
            "stapel: --kind must be one of finding, catches, decision\n",
        ),
        (
            "",
            "stapel: --kind must be one of finding, catches, decision\n",
        ),
    ] {
        let err = ledger_refused(dir, &["--kind", kind, "--note", "n"]);
        assert_eq!(err, want, "kind {kind:?}");
        assert!(ledger_text(dir).is_none());
    }
    for note in ["", "  ", "\n\t "] {
        let err = ledger_refused(dir, &["--kind", "finding", "--note", note]);
        assert_eq!(err, "stapel: --note must not be empty\n", "note {note:?}");
        assert!(ledger_text(dir).is_none());
    }
    // A missing option is a usage error, also exit 2.
    for args in [&["--kind", "finding"][..], &["--note", "n"][..], &[][..]] {
        let out = stapel(dir)
            .args(["ledger", "add"])
            .args(args)
            .assert()
            .code(2)
            .get_output()
            .clone();
        assert!(out.stdout.is_empty());
        assert!(ledger_text(dir).is_none());
    }
    // A ledger line that is not a JSON object.
    for (text, want) in [
        (
            "{\"v\":1}\nnot json\n",
            format!("stapel: {LEDGER}:2: not JSON"),
        ),
        (
            "{\"v\":1}\n[1]\n",
            format!("stapel: {LEDGER}:2: not a JSON object\n"),
        ),
    ] {
        put(dir, LEDGER, text);
        let err = ledger_refused(dir, &["--kind", "finding", "--note", "n"]);
        assert!(err.starts_with(&want), "want {want:?}, got {err:?}");
        assert_eq!(ledger_text(dir).unwrap(), text);
    }
}

// ---- AC-7, both record commands ----

#[test]
fn closed_ticket_by_key_only() {
    let one_finding = |key: &str| {
        let mut v = base();
        v["ticket"] = json!(key);
        line(&v)
    };
    let ledger_add = |dir: &Path, key: Option<&str>| {
        let mut args = vec!["ledger", "add", "--kind", "finding", "--note", "n"];
        args.extend(key);
        stapel(dir).args(&args).assert()
    };
    let import = |dir: &Path, key: Option<&str>| {
        let mut args = vec!["findings", "import", "in.jsonl"];
        args.extend(key);
        stapel(dir).args(&args).assert()
    };
    let one_line = |err: &str| {
        assert!(
            err.starts_with("stapel: ") && !err.starts_with("stapel: stapel: "),
            "{err}"
        );
        assert_eq!(err.lines().count(), 1, "{err}");
    };

    // A closed ticket by key: both commands write as for an open one, in any case.
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir)
        .args(["close", "--reason", "done"])
        .assert()
        .success();
    put(dir, "in.jsonl", &one_finding("ABC-1"));
    import(dir, Some("abc-1"))
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), one_finding("ABC-1"));
    ledger_add(dir, Some("ABC-1"))
        .success()
        .stdout("recorded: finding for ABC-1\n");
    assert_eq!(ledger_text(dir).unwrap().lines().count(), 1);

    // No KEY and no open ticket: today's refusal, one `stapel: `.
    let out = import(dir, None).code(2).get_output().clone();
    assert!(out.stdout.is_empty());
    let err = String::from_utf8(out.stderr).unwrap();
    assert_eq!(
        err,
        "stapel: no open ticket; name one, e.g. stapel status <KEY>\n"
    );
    let out = ledger_add(dir, None).code(2).get_output().clone();
    assert!(out.stdout.is_empty());
    let err = String::from_utf8(out.stderr).unwrap();
    assert_eq!(
        err,
        "stapel: no open ticket; name one, e.g. stapel status <KEY>\n"
    );
    assert_eq!(ledger_text(dir).unwrap().lines().count(), 1);

    // No KEY with one open ticket: that ticket, not the closed one.
    stapel(dir).args(["new", "Second"]).assert().success();
    put(dir, "in.jsonl", &one_finding("ABC-2"));
    import(dir, None)
        .success()
        .stdout("imported: 1 findings into ABC-2\n");
    assert!(
        std::fs::read_to_string(dir.join(".stapel/tickets/ABC-2/findings.jsonl"))
            .unwrap()
            .contains("ABC-2")
    );
    ledger_add(dir, None)
        .success()
        .stdout("recorded: finding for ABC-2\n");
    assert_eq!(target_text(dir).unwrap(), one_finding("ABC-1"));

    // No KEY with two open tickets.
    stapel(dir).args(["new", "Third"]).assert().success();
    let before = ledger_text(dir).unwrap();
    for out in [
        import(dir, None).code(2).get_output().clone(),
        ledger_add(dir, None).code(2).get_output().clone(),
    ] {
        assert!(out.stdout.is_empty());
        let err = String::from_utf8(out.stderr).unwrap();
        assert_eq!(
            err,
            "stapel: several open tickets, name one: ABC-2, ABC-3\n"
        );
        one_line(&err);
    }
    assert_eq!(ledger_text(dir).unwrap(), before);

    // A legacy ticket by key.
    put(
        dir,
        ".stapel/tickets/ABC-9/state.json",
        r#"{"key":"ABC-9","build":{"allowed":false}}"#,
    );
    put(dir, "in.jsonl", &one_finding("ABC-9"));
    for out in [
        import(dir, Some("ABC-9")).code(2).get_output().clone(),
        ledger_add(dir, Some("abc-9")).code(2).get_output().clone(),
    ] {
        assert!(out.stdout.is_empty());
        let err = String::from_utf8(out.stderr).unwrap();
        assert_eq!(
            err,
            "stapel: ticket ABC-9 has a legacy (STP-1) state.json; its journal is kept by hand\n"
        );
        one_line(&err);
    }
    assert_eq!(ledger_text(dir).unwrap(), before);
    assert!(!dir.join(".stapel/tickets/ABC-9/findings.jsonl").exists());
}

// ---- AC-8 ----

#[test]
fn guard_denies_ledger_writes() {
    let repo = repo_with_ticket();
    let root = repo.path();
    for rel in [
        ".stapel/process-ledger.jsonl",
        ".STAPEL/Process-Ledger.jsonl",
    ] {
        let path = root.join(rel).display().to_string();
        for tool in ["Write", "Edit", "MultiEdit", "NotebookEdit"] {
            let input = match tool {
                "NotebookEdit" => json!({ "notebook_path": path, "new_source": "x" }),
                "MultiEdit" => json!({ "file_path": path, "edits": [] }),
                _ => {
                    json!({ "file_path": path, "content": "x", "old_string": "a", "new_string": "b" })
                }
            };
            let payload = json!({
                "session_id": "test",
                "hook_event_name": "PreToolUse",
                "cwd": root,
                "tool_name": tool,
                "tool_input": input,
            });
            let out = stapel(root)
                .args(["hook", "pre-tool-use"])
                .write_stdin(payload.to_string())
                .assert()
                .code(2)
                .get_output()
                .clone();
            let err = String::from_utf8(out.stderr).unwrap();
            assert!(
                err.contains("only stapel or the human writes"),
                "{tool} {rel}: {err}"
            );
        }
    }
}

// ---- AC-16 ----

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(["-c", "user.email=ci@host.invalid"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// The output of `stapel check` and its exit code.
fn check_text(dir: &Path) -> (i32, String) {
    let out = stapel(dir).args(["check", "ABC-1"]).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn ledger_change_keeps_tree_clean() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let config = std::fs::read_to_string(dir.join(".stapel/stapel.toml")).unwrap();
    put(
        dir,
        ".stapel/stapel.toml",
        &format!("{config}\n[check]\nrunner = \"cargo\"\n"),
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "ABC-1: start"]);
    // Only the ledger changed: the tree is clean, so the check gets as far as the steps.
    put(dir, LEDGER, "{\"v\":1}\n");
    let (code, text) = check_text(dir);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("no step commits"), "{text}");
    assert!(!text.contains("uncommitted"), "{text}");
    // The ledger and a source file changed: the tree is dirty, and the source file is named.
    put(dir, "src/x.rs", "fn main() {}\n");
    let (code, text) = check_text(dir);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("uncommitted changes"), "{text}");
    assert!(text.contains("src/x.rs"), "{text}");
    assert!(!text.contains("process-ledger"), "{text}");
}

#[test]
fn ledger_change_keeps_check_fresh() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let config = std::fs::read_to_string(dir.join(".stapel/stapel.toml")).unwrap();
    put(
        dir,
        ".stapel/stapel.toml",
        &format!("{config}\n[check]\nrunner = \"cargo\"\n"),
    );
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "ABC-1: start"]);
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    let head = String::from_utf8(head.stdout).unwrap().trim().to_string();
    // A passing check recorded at HEAD, as `stapel check` writes it.
    let record = format!(
        "{{\"v\":1,\"kind\":\"check\",\"id\":\"c-000000000000\",\"at\":\"2026-01-01T00:00:00Z\",\"ticket\":\"ABC-1\",\"head\":\"{head}\",\"result\":\"pass\",\"steps\":[]}}\n"
    );
    put(dir, ".stapel/tickets/ABC-1/runs.jsonl", &record);
    let status_line = |dir: &Path| {
        let out = stapel(dir).args(["status", "ABC-1"]).output().unwrap();
        String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .find(|l| l.starts_with("check:"))
            .unwrap_or_default()
            .to_string()
    };
    let pass = format!("check: pass at {}", &head[..7]);
    assert_eq!(status_line(dir), pass);
    // Only the ledger changed: the check stays fresh.
    put(dir, LEDGER, "{\"v\":1}\n");
    assert_eq!(status_line(dir), pass);
    // A source file changed as well: the check is stale.
    put(dir, "src/x.rs", "fn main() {}\n");
    let line = status_line(dir);
    assert!(line.starts_with("check: stale"), "{line}");
    assert!(line.contains("src/x.rs"), "{line}");
}
