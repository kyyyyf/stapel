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
        "fate": "patch",
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

/// The lines of `text` as `findings import` appends them for a person (STP-23): the importer
/// insert before the last `}`, trailing whitespace of the line removed.
fn inserted(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|l| match l.trim_end().strip_suffix('}') {
            Some(body) => format!(
                "{body},\"imported_by\":{{\"by\":\"person\",\"user\":\"test-user\"}}}}\n"
            ),
            None => l.to_string(),
        })
        .collect()
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
    let mut free = base();
    free["fate"] = json!("kept");
    put(dir, "free.jsonl", &line(&free));
    assert_eq!(
        refused(dir, &["free.jsonl"]),
        "stapel: free.jsonl:1: fate: must be one of open, patch, bad_plan, intent_gap, defer, no-change\n"
    );
    assert!(target_text(dir).is_none());
    let old = "{\"id\":\"OLD-1\",\"round\":\"1\"}\n";
    put(dir, TARGET, old);
    // Odd spacing, an unusual key order and extra fields must survive as given.
    let one = r#"{"id": "F-1", "note": "n1", "ticket": "ABC-1", "stage": "spec-review", "round": "2", "role": "external", "severity": "LOW", "promise": "outside", "claim": "first", "catchable_at": "spec", "fate": "defer", "step": 3}"#;
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
    let expect = format!(
        "{old}{}",
        inserted(&format!("{one}\n{}\n{}\n", two.trim_end(), three.trim_end()))
    );
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
    assert_eq!(target_text(dir).unwrap(), inserted(&line(&base())));

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
        format!("{{\"id\":\"OLD-1\"}}\n{}", inserted(&line(&base())))
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

#[test]
fn import_refuses_unsafe_id() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for (name, id) in [
        ("65 characters", "x".repeat(65)),
        ("space", "F 1".to_string()),
        ("tab", "F\t1".to_string()),
        ("newline", "F\n1".to_string()),
    ] {
        let mut v = base();
        v["id"] = json!(id);
        // `line` writes the control characters as JSON escapes, so the record stays one line.
        put(dir, "in.jsonl", &line(&v));
        let err = refused(dir, &["in.jsonl"]);
        assert!(err.starts_with("stapel: in.jsonl:1: id: "), "{name}: {err}");
        assert!(target_text(dir).is_none(), "{name}: wrote something");
    }
    let mut v = base();
    v["id"] = json!("x".repeat(64));
    put(dir, "in.jsonl", &line(&v));
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), inserted(&line(&v)));
}

#[test]
fn import_refuses_unsafe_round() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for (name, round) in [
        ("33 characters", "я".repeat(33)),
        ("newline", "r\n1".to_string()),
    ] {
        let mut v = base();
        v["round"] = json!(round);
        // `line` writes the control characters as JSON escapes, so the record stays one line.
        put(dir, "in.jsonl", &line(&v));
        let err = refused(dir, &["in.jsonl"]);
        assert!(
            err.starts_with("stapel: in.jsonl:1: round: "),
            "{name}: {err}"
        );
        assert_eq!(err.lines().count(), 1, "{name}: {err}");
        assert!(target_text(dir).is_none(), "{name}: wrote something");
    }
    // 32 characters, 64 bytes: characters are counted, not bytes.
    let mut v = base();
    v["round"] = json!("я".repeat(32));
    put(dir, "in.jsonl", &line(&v));
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), inserted(&line(&v)));
}

#[test]
fn id_rule_counts_characters_and_controls() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let reason = "stapel: in.jsonl:1: id: must be 1 to 64 characters with no whitespace or control character\n";
    for (name, id) in [
        ("control character", "F\u{1}1".to_string()),
        ("65 Cyrillic characters", "ж".repeat(65)),
    ] {
        let mut v = base();
        v["id"] = json!(id);
        put(dir, "in.jsonl", &line(&v));
        assert_eq!(refused(dir, &["in.jsonl"]), reason, "{name}");
        assert!(target_text(dir).is_none(), "{name}: wrote something");
    }
    let mut v = base();
    v["id"] = json!("ж".repeat(64));
    put(dir, "in.jsonl", &line(&v));
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), inserted(&line(&v)));
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
            format!("{before_target}{}", inserted(&line(&v)))
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
    assert_eq!(target_text(dir).unwrap(), inserted(&hundred));

    // A line that is exactly 64 KiB, newline included, with the importer insert.
    let extra = inserted("{}\n").len() - "{}\n".len();
    let repo = repo_with_ticket();
    let dir = repo.path();
    let big = padded("F-1", 64 * 1024 - extra);
    put(dir, "in.jsonl", &big);
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), inserted(&big));
    assert_eq!(target_text(dir).unwrap().len(), 64 * 1024);

    // A file of exactly 256 KiB: four such lines and blank lines for the rest.
    let repo = repo_with_ticket();
    let dir = repo.path();
    let lines: String = (1..=4)
        .map(|n| padded(&format!("F-{n}"), 64 * 1024 - extra))
        .collect();
    let file = format!("{lines}{}", "\n".repeat(4 * extra));
    assert_eq!(file.len(), 256 * 1024);
    put(dir, "in.jsonl", &file);
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 4 findings into ABC-1\n");
    assert_eq!(target_text(dir).unwrap(), inserted(&lines));
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
        "{{\"v\":1,\"at\":\"{at}\",\"kind\":\"catches\",\"ticket\":\"ABC-1\",\"note\":\"item 3 caught it\",\"imported_by\":{{\"by\":\"person\",\"user\":\"test-user\"}}}}\n"
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
    assert_eq!(target_text(dir).unwrap(), inserted(&one_finding("ABC-1")));
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
    assert_eq!(target_text(dir).unwrap(), inserted(&one_finding("ABC-1")));

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

// ---- Step 3: the dialog, the grant and the agent-shell refusal (AC-9 to AC-15) ----

const DENY: i32 = 2;
const TAIL: &str = "Answer Yes or No; do not choose \"don't ask again\".";
const MISMATCH: &str = "stapel: the grant does not match: the records, the ticket or the stapel build changed since the dialog, or the grant was used already; ask the agent to run the command again\n";
const GRANT_DENIED: &str = "a --grant comes only from the permission dialog; stapel ok, stapel close, stapel findings import, stapel ledger add and stapel tokens import-records may not carry one of their own or words filled in at run time";

/// The block `## name` of `golden/records_dialog.txt`: the reason, then the replaced command with
/// its token shown as `<token>`.
fn dialog_golden(name: &str) -> String {
    let text = golden("records_dialog.txt");
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

fn close_ticket(dir: &Path, key: &str) {
    stapel(dir)
        .args(["close", key, "--reason", "done"])
        .assert()
        .success();
}

// ---- AC-13 ----

#[test]
fn dialog_for_ledger_add() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let note = "n".repeat(201);
    let command = format!("stapel ledger add --kind decision --note {note}");
    let (reason, replaced) = common::ask(dir, &command);
    assert_eq!(
        dialog_text(&reason, &replaced),
        dialog_golden("ledger"),
        "{reason}"
    );
    assert_eq!(grant_files(dir), 1);
    assert!(ledger_text(dir).is_none());

    // The replaced command keeps the whole note, quoted; the reason shows control characters as
    // one space each; the key is added in its folder case.
    let (reason, replaced) = common::ask(
        dir,
        "stapel ledger add --kind=decision --note='two words\nnext' abc-1",
    );
    assert_eq!(
        reason,
        format!(
            "stapel: add a ledger record for ABC-1: decision: two words next; runs: stapel. {TAIL}"
        )
    );
    assert!(
        replaced.starts_with(
            "stapel ledger add --kind decision --note 'two words\nnext' ABC-1 --grant "
        ),
        "{replaced}"
    );

    // The `cargo run` form as written.
    let (reason, replaced) = common::ask(
        dir,
        "cargo run -q -p stapel-cli -- ledger add --kind decision --note x",
    );
    assert!(
        reason.ends_with(&format!("; runs: cargo run -q -p stapel-cli --. {TAIL}")),
        "{reason}"
    );
    assert!(
        replaced.starts_with(
            "cargo run -q -p stapel-cli -- ledger add --kind decision --note x ABC-1 --grant "
        ),
        "{replaced}"
    );
    assert_eq!(grant_files(dir), 3);

    // A closed ticket by its key.
    close_ticket(dir, "ABC-1");
    let (reason, _) = common::ask(dir, "stapel ledger add --kind decision --note x ABC-1");
    assert_eq!(
        reason,
        format!(
            "stapel: add a ledger record for ABC-1 (closed): decision: x; runs: stapel. {TAIL}"
        )
    );
    // A finding record on a closed ticket has no dialog.
    common::hook_bash(dir, "stapel ledger add --kind finding --note x ABC-1", "auto")
        .success()
        .stdout("");
}

// ---- AC-10 ----

#[test]
fn guard_denies_what_the_command_refuses() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut bad = base();
    bad["stage"] = json!("nobody");
    put(dir, "bad.jsonl", &line(&bad));
    put(dir, "ok.jsonl", &line(&base()));
    put(dir, TARGET, "not json\n");
    put(dir, "dir/x", "");

    // The guard gives the reason of the command, and writes no grant.
    // `findings import` has no dialog: the guard allows it and the command refuses by itself.
    for file in ["bad.jsonl", "missing.jsonl", "ok.jsonl", "dir"] {
        common::hook_bash(dir, &format!("stapel findings import {file}"), "auto")
            .success()
            .stdout("");
    }
    let cases: [(&str, Vec<&str>); 1] = [(
        "stapel ledger add --kind other --note x",
        vec!["ledger", "add", "--kind", "other", "--note", "x"],
    )];
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
    let blank = ledger_refused(dir, &["--kind", "decision", "--note", " "]);
    common::hook_bash(dir, "stapel ledger add --kind decision --note ' '", "auto")
        .code(DENY)
        .stderr(blank);
    // A finding record needs no dialog: the guard allows it and the command refuses by itself.
    common::hook_bash(dir, "stapel ledger add --kind finding --note ' '", "auto")
        .success()
        .stdout("");
    assert_eq!(grant_files(dir), 0);

    // Modes without a dialog deny.
    std::fs::remove_file(dir.join(TARGET)).unwrap();
    for command in ["stapel ledger add --kind decision --note x"] {
        for mode in ["bypassPermissions", "dontAsk", ""] {
            common::hook_bash(dir, command, mode)
                .code(DENY)
                .stderr(predicates::str::contains("dialog"));
        }
    }
    assert_eq!(grant_files(dir), 0);

    // An allow rule that would skip the dialog denies.
    put(
        dir,
        ".claude/settings.local.json",
        r#"{"permissions":{"allow":["Bash(stapel findings:*)","Bash(stapel ledger:*)"]}}"#,
    );
    for command in ["stapel ledger add --kind decision --note x"] {
        common::hook_bash(dir, command, "auto")
            .code(DENY)
            .stderr(predicates::str::contains("allow rule"));
    }
    assert_eq!(grant_files(dir), 0);
}

// ---- AC-11 ----

#[test]
fn grant_writes_once() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &line(&base()));

    // `findings import` has no grant: the guard allows it, writes none, and the command writes
    // once without one.
    common::hook_bash(dir, "stapel findings import in.jsonl", "auto")
        .success()
        .stdout("");
    assert_eq!(grant_files(dir), 0);
    agent(dir, &["findings", "import", "in.jsonl", "ABC-1"])
        .success()
        .stdout("imported: 1 findings into ABC-1\n");
    assert_eq!(
        target_text(dir).unwrap(),
        inserted(&line(&base())).replace("\"person\"", "\"agent\"")
    );

    // A `catches` record has no grant either.
    common::hook_bash(dir, "stapel ledger add --kind catches --note 'a note'", "auto")
        .success()
        .stdout("");
    assert_eq!(grant_files(dir), 0);

    // `ledger add --kind decision`: written once; the same token then does not match.
    let at = today_utc();
    let (_, replaced) = common::ask(dir, "stapel ledger add --kind decision --note 'a note'");
    let token = common::token_of(&replaced);
    let args = [
        "ledger", "add", "--kind", "decision", "--note", "a note", "ABC-1", "--grant", &token,
    ];
    agent(dir, &args)
        .success()
        .stdout("recorded: decision for ABC-1\n");
    let record = format!(
        "{{\"v\":1,\"at\":\"{at}\",\"kind\":\"decision\",\"ticket\":\"ABC-1\",\"note\":\"a note\"}}\n"
    );
    assert_eq!(ledger_text(dir).unwrap(), record);
    agent(dir, &args).code(1).stderr(MISMATCH).stdout("");
    assert_eq!(ledger_text(dir).unwrap(), record);
}

// ---- AC-14 ----

#[test]
fn grant_names_the_records() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    stapel(dir).args(["new", "Second"]).assert().success();
    put(dir, "in.jsonl", &line(&base()));
    // `findings import` takes no grant: an option `--grant` is a usage error.
    agent(
        dir,
        &["findings", "import", "in.jsonl", "ABC-1", "--grant", "abc"],
    )
    .code(2);
    let add = |kind: &str, note: &str, key: &str, token: &str| {
        agent(
            dir,
            &[
                "ledger", "add", "--kind", kind, "--note", note, key, "--grant", token,
            ],
        )
    };
    let token = |command: &str| common::token_of(&common::ask(dir, command).1);

    // A finding record has no dialog and so no grant.
    common::hook_bash(dir, "stapel ledger add --kind finding --note x ABC-1", "auto")
        .success()
        .stdout("");
    assert_eq!(grant_files(dir), 0);

    // Another note or key after the dialog.
    let t = token("stapel ledger add --kind decision --note x ABC-1");
    add("decision", "y", "ABC-1", &t).code(1).stderr(MISMATCH);
    let t = token("stapel ledger add --kind decision --note x ABC-1");
    add("decision", "x", "ABC-2", &t).code(1).stderr(MISMATCH);
    // A grant of `ok` on `ledger add`, and a grant of `ledger add` on `ok`.
    let t = token("stapel ok ABC-1 spec");
    add("decision", "x", "ABC-1", &t).code(1).stderr(MISMATCH);
    let t = token("stapel ledger add --kind decision --note x ABC-1");
    agent(dir, &["ok", "ABC-1", "spec", "--grant", &t])
        .code(1)
        .stderr(predicates::str::contains("the grant does not match"));
    assert!(ledger_text(dir).is_none());

    // An expired grant.
    let t = token("stapel ledger add --kind decision --note x ABC-1");
    for entry in std::fs::read_dir(common::grants_dir(dir)).unwrap() {
        std::fs::write(entry.unwrap().path(), r#"{"expires_at":1}"#).unwrap();
    }
    add("decision", "x", "ABC-1", &t)
        .code(1)
        .stderr("stapel: the grant expired; ask the agent to run the command again\n");
    assert!(ledger_text(dir).is_none());
    assert_eq!(grant_files(dir), 0);
}

// ---- AC-12 ----

#[test]
fn agent_shell_without_grant_refused() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &line(&base()));
    let before = common::snapshot(dir);
    let refusal = "stapel: only a person writes ledger decisions: this shell belongs to an agent (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); run the command as one plain command so that Claude Code shows the permission dialog, or, from a Zed task started from a Claude Code shell, run it with env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT\n";
    for var in ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"] {
        stapel(dir)
            .args(["ledger", "add", "--kind", "decision", "--note", "x"])
            .env(var, "1")
            .assert()
            .code(1)
            .stdout("")
            .stderr(refusal);
    }
    assert_eq!(common::snapshot(dir), before);
    // `findings import` needs no grant in an agent's shell.
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .env("CLAUDECODE", "1")
        .assert()
        .success();
}

// ---- AC-15 ----

#[test]
fn guard_denies_own_grant() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "f", "x");
    for command in [
        "stapel findings import in.jsonl --grant abc",
        "stapel findings import in.jsonl ABC-1 --grant=abc",
        "stapel ledger add --kind finding --note x --grant abc",
        "cargo run -q -p stapel-cli -- findings import in.jsonl --grant abc",
        "cargo run -q -p stapel-cli -- ledger add --kind finding --note x --grant abc",
        "stapel findings import $(cat f)",
        "stapel ledger add --kind finding --note $NOTE",
        "stapel ok spec --grant abc",
        "stapel close --reason done --grant abc",
    ] {
        common::hook_bash(dir, command, "auto")
            .code(DENY)
            .stdout("")
            .stderr(format!("stapel: {GRANT_DENIED}\n"));
    }
    assert_eq!(grant_files(dir), 0);
}

// ---- Step 3c: coverage ----

#[test]
fn ledger_grant_survives_a_failed_check() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let at = today_utc();
    common::hook_bash(dir, "stapel ledger add --kind catches --note 'a note'", "auto")
        .success()
        .stdout("");
    let (_, replaced) = common::ask(dir, "stapel ledger add --kind decision --note 'a note'");
    let token = common::token_of(&replaced);
    let args = [
        "ledger", "add", "--kind", "decision", "--note", "a note", "ABC-1", "--grant", &token,
    ];
    assert_eq!(grant_files(dir), 1);

    // A broken line added after the dialog: the run refuses, writes nothing, keeps the grant.
    put(dir, LEDGER, "not json\n");
    let out = agent(dir, &args).code(2).get_output().clone();
    assert!(out.stdout.is_empty());
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(
        err.starts_with(&format!("stapel: {LEDGER}:1: not JSON")),
        "{err}"
    );
    assert_eq!(ledger_text(dir).unwrap(), "not json\n");
    assert_eq!(grant_files(dir), 1);

    // With the line removed, the same grant writes once.
    put(dir, LEDGER, "");
    agent(dir, &args)
        .success()
        .stdout("recorded: decision for ABC-1\n");
    let record = format!(
        "{{\"v\":1,\"at\":\"{at}\",\"kind\":\"decision\",\"ticket\":\"ABC-1\",\"note\":\"a note\"}}\n"
    );
    assert_eq!(ledger_text(dir).unwrap(), record);
    assert_eq!(grant_files(dir), 0);
}

#[test]
fn dialog_cuts_at_the_limits() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    // `findings import` has no dialog to cut.
    put(dir, "in.jsonl", &line(&base()));
    common::hook_bash(dir, "stapel findings import in.jsonl", "auto")
        .success()
        .stdout("");

    let note = "n".repeat(200);
    let command = format!("stapel ledger add --kind decision --note {note}");
    common::hook_bash(dir, &command.replace("decision", "catches"), "auto")
        .success()
        .stdout("");
    let (reason, _) = common::ask(dir, &command);
    assert!(
        reason.contains(&format!("decision: {note}; runs:")),
        "{reason}"
    );
    assert!(!reason.contains('…'), "{reason}");
}

// ---- Step 4d: coverage after code review round 1 ----

#[test]
fn as_built_line_rules() {
    let repo = repo_with_ticket();
    let dir = repo.path();

    // CRLF input is stored without the `\r`.
    put(dir, "in.jsonl", &line(&base()).replace('\n', "\r\n"));
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success();
    assert_eq!(target_text(dir).unwrap(), inserted(&line(&base())));

    // A target line with an unknown version is skipped: its id does not count.
    let old = "{\"v\":2,\"id\":\"F-1\"}\n";
    put(dir, TARGET, old);
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success();
    assert_eq!(
        target_text(dir).unwrap(),
        format!("{old}{}", inserted(&line(&base())))
    );
    put(dir, TARGET, "");

    // A duplicate key is read by its last value, and the line is kept as given.
    let text = line(&base());
    let body = text.trim_end().strip_prefix('{').unwrap().to_string();
    let accepted = format!("{{\"severity\":\"nobody\",{body}\n");
    put(dir, "in.jsonl", &accepted);
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success();
    assert_eq!(target_text(dir).unwrap(), inserted(&accepted));
    put(dir, TARGET, "");
    let rejected = format!(
        "{},\"severity\":\"nobody\"}}\n",
        text.trim_end().strip_suffix('}').unwrap()
    );
    put(dir, "in.jsonl", &rejected);
    assert_eq!(
        refused(dir, &["in.jsonl"]),
        "stapel: in.jsonl:1: severity: must be one of HIGH, MEDIUM, LOW\n"
    );
    assert!(target_text(dir).is_none_or(|t| t.is_empty()));

    // The 101st line is refused by its own reason: a bad line by its defect, a good one by the limit.
    let hundred: String = (0..100)
        .map(|i| {
            let mut v = base();
            v["id"] = json!(format!("F-{i}"));
            line(&v)
        })
        .collect();
    put(dir, "in.jsonl", &format!("{hundred}not json\n"));
    let err = refused(dir, &["in.jsonl"]);
    assert!(err.starts_with("stapel: in.jsonl:101: not JSON"), "{err}");
    let mut extra = base();
    extra["id"] = json!("F-100");
    put(dir, "in.jsonl", &format!("{hundred}{}", line(&extra)));
    assert_eq!(
        refused(dir, &["in.jsonl"]),
        "stapel: in.jsonl:101: more than 100 records; split the file\n"
    );
    put(dir, "in.jsonl", &format!("{hundred}[1]\n"));
    assert_eq!(
        refused(dir, &["in.jsonl"]),
        "stapel: in.jsonl:101: not a JSON object\n"
    );
}

// ---- STP-23: `findings import` has no dialog; these tests of its former dialog now assert that ----

/// The guard allows the command with no output, and writes no grant.
fn allowed_without_dialog(dir: &Path, command: &str) {
    common::hook_bash(dir, command, "auto").success().stdout("");
    assert_eq!(grant_files(dir), 0, "{command}");
}

#[test]
fn dialog_for_findings_import() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &line(&base()));
    for command in [
        "stapel findings import in.jsonl",
        "stapel findings import in.jsonl abc-1",
        "cargo run -q -p stapel-cli -- findings import in.jsonl",
    ] {
        allowed_without_dialog(dir, command);
    }
    // A closed ticket by its key.
    close_ticket(dir, "ABC-1");
    allowed_without_dialog(dir, "stapel findings import in.jsonl ABC-1");
    assert!(target_text(dir).is_none());
}

#[test]
fn dialog_limits_to_30_records() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let records: String = (0..31)
        .map(|i| {
            let mut v = base();
            v["id"] = json!(format!("F-{i}"));
            line(&v)
        })
        .collect();
    put(dir, "in.jsonl", &records);
    // No limit of 30 records in the guard.
    allowed_without_dialog(dir, "stapel findings import in.jsonl");
    stapel(dir)
        .args(["findings", "import", "in.jsonl"])
        .assert()
        .success()
        .stdout("imported: 31 findings into ABC-1\n");
}

#[test]
fn dialog_reads_file_from_the_hook_cwd() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "sub/in.jsonl", &line(&base()));
    // The guard reads no file: the same relative name is allowed from the subdirectory and from
    // the root.
    for cwd in [dir.join("sub"), dir.to_path_buf()] {
        let payload = json!({
            "tool_name": "Bash",
            "cwd": cwd,
            "permission_mode": "auto",
            "tool_input": { "command": "stapel findings import in.jsonl", "description": "test" },
        });
        stapel(dir)
            .args(["hook", "pre-tool-use"])
            .write_stdin(payload.to_string())
            .assert()
            .success()
            .stdout("");
    }
    assert_eq!(grant_files(dir), 0);
}

#[test]
fn dialog_shows_fate_and_summary() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut free = base();
    free["fate"] = json!("f".repeat(41));
    put(dir, "free.jsonl", &line(&free));
    assert_eq!(
        refused(dir, &["free.jsonl"]),
        "stapel: free.jsonl:1: fate: must be one of open, patch, bad_plan, intent_gap, defer, no-change\n"
    );
    assert!(target_text(dir).is_none());
    let mut v = base();
    v["note"] = json!("f".repeat(41));
    put(dir, "in.jsonl", &line(&v));
    // There is no summary: the guard answers with nothing.
    allowed_without_dialog(dir, "stapel findings import in.jsonl");
}

#[test]
fn dialog_summary_edges() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut other = base();
    other["id"] = json!("F-2");
    other["round"] = json!("1");
    other["role"] = json!("external");
    put(
        dir,
        "in.jsonl",
        &format!("{}{}", line(&base()), line(&other)),
    );
    allowed_without_dialog(dir, "stapel findings import in.jsonl");
}

#[test]
fn hidden_field_edit_voids_the_grant() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let mut free = base();
    free["fate"] = json!("rejected");
    put(dir, "free.jsonl", &line(&free));
    assert_eq!(
        refused(dir, &["free.jsonl"]),
        "stapel: free.jsonl:1: fate: must be one of open, patch, bad_plan, intent_gap, defer, no-change\n"
    );
    assert!(target_text(dir).is_none());
    put(dir, "in.jsonl", &line(&base()));
    allowed_without_dialog(dir, "stapel findings import in.jsonl");
    // A file edited after the guard answered is imported as it is: there is no grant to void.
    let mut edited = base();
    edited["fate"] = json!("no-change");
    put(dir, "in.jsonl", &line(&edited));
    stapel(dir)
        .args(["findings", "import", "in.jsonl", "ABC-1"])
        .assert()
        .success();
    assert_eq!(target_text(dir).unwrap(), inserted(&line(&edited)));
}

#[test]
fn dialog_usage_denials() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    let ledger_usage = "usage: stapel ledger add --kind KIND --note TEXT [KEY]";
    // `findings import` has no dialog: the guard leaves its usage to the command.
    for command in [
        "stapel findings import --x in.jsonl",
        "stapel findings import",
        "stapel findings import a b c",
    ] {
        common::hook_bash(dir, command, "auto").success().stdout("");
    }
    let cases = [
        (
            "stapel ledger add --kind decision --note x --bogus",
            "unexpected option --bogus for stapel ledger add",
        ),
        (
            "stapel ledger add --kind decision --note x ABC-1 extra",
            ledger_usage,
        ),
        ("stapel ledger add --kind decision", ledger_usage),
        ("stapel ledger add --note x", ledger_usage),
    ];
    for (command, reason) in cases {
        common::hook_bash(dir, command, "auto")
            .code(DENY)
            .stdout("")
            .stderr(format!("stapel: {reason}\n"));
    }
    assert_eq!(grant_files(dir), 0);
    // A finding or catches record has no dialog: the guard leaves its usage to the command.
    for command in [
        "stapel ledger add --kind finding --note x --bogus",
        "stapel ledger add --kind catches",
    ] {
        common::hook_bash(dir, command, "auto").success().stdout("");
    }

    // The `--kind=` and `--note=` forms are read as the separate-word forms.
    let (reason, replaced) = common::ask(dir, "stapel ledger add --kind=decision --note=hello");
    assert_eq!(
        reason,
        format!("stapel: add a ledger record for ABC-1: decision: hello; runs: stapel. {TAIL}")
    );
    assert!(
        replaced.starts_with("stapel ledger add --kind decision --note hello ABC-1 --grant "),
        "{replaced}"
    );
    assert_eq!(grant_files(dir), 1);
}

// ---- Step 5: code review round 1 fixes ----

#[test]
fn dialog_refuses_a_broken_config() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    put(dir, "in.jsonl", &line(&base()));
    put(dir, ".stapel/stapel.toml", "this is = [not toml\n");

    // `findings import` has no dialog: the guard allows it and the command gives the reason.
    common::hook_bash(dir, "stapel findings import in.jsonl", "auto")
        .success()
        .stdout("");

    // The guard gives the reason that the command gives, and writes no grant.
    let out = stapel(dir)
        .args(["ledger", "add", "--kind", "decision", "--note", "x"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let expected = String::from_utf8(out.stderr).unwrap();
    assert!(
        expected.starts_with("stapel: .stapel/stapel.toml: "),
        "{expected}"
    );
    // A finding record has no dialog: the guard allows it.
    common::hook_bash(dir, "stapel ledger add --kind finding --note x", "auto")
        .success()
        .stdout("");
    common::hook_bash(dir, "stapel ledger add --kind decision --note x", "auto")
        .code(DENY)
        .stdout("")
        .stderr(expected);
    assert_eq!(grant_files(dir), 0);
}

#[test]
fn ledger_line_limit_before_the_grant() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    // Quotes take two bytes in the record and one in the command, so the record can pass 64 KiB
    // while the command stays under the guard's own limit.
    let overhead = format!(
        "{{\"v\":1,\"at\":\"{}\",\"kind\":\"decision\",\"ticket\":\"ABC-1\",\"note\":\"\"}}\n",
        today_utc()
    )
    .len();
    let note = |line_len: usize| {
        let extra = line_len - overhead;
        format!("{}{}", "\"".repeat(extra / 2), "a".repeat(extra % 2))
    };
    let command = |note: &str| format!("stapel ledger add --kind decision --note '{note}'");

    // A finding or catches record has no dialog, so the guard leaves its line limit to the command.
    common::hook_bash(
        dir,
        &command(&note(65_537)).replace("decision", "catches"),
        "auto",
    )
    .success()
    .stdout("");

    // A line of 64 KiB + 1 byte: the command refuses, and the guard denies with the same reason
    // before any grant.
    let over = note(65_537);
    let expected = ledger_refused(dir, &["--kind", "decision", "--note", &over]);
    assert!(expected.contains("64 KiB"), "{expected}");
    common::hook_bash(dir, &command(&over), "auto")
        .code(DENY)
        .stdout("")
        .stderr(expected);
    assert_eq!(grant_files(dir), 0);
    assert!(ledger_text(dir).is_none());

    // A line of exactly 64 KiB asks.
    let exact = note(65_536);
    let (_, replaced) = common::ask(dir, &command(&exact));
    assert!(replaced.contains("--grant "), "{}", replaced.len());
    assert_eq!(grant_files(dir), 1);
}

#[test]
fn own_grant_in_any_case() {
    let repo = repo_with_ticket();
    let dir = repo.path();
    for command in [
        "stapel ledger ADD --kind finding --note x --grant abc",
        "stapel Ledger Add --kind finding --note x --grant=abc",
        "stapel findings IMPORT in.jsonl --grant abc",
        "stapel FINDINGS import in.jsonl $x",
        "cargo run -q -p stapel-cli -- findings IMPORT in.jsonl $x",
    ] {
        common::hook_bash(dir, command, "auto")
            .code(DENY)
            .stdout("")
            .stderr(format!("stapel: {GRANT_DENIED}\n"));
    }
    assert_eq!(grant_files(dir), 0);
}

#[test]
fn dialog_refuses_an_unreadable_config() {
    use std::os::unix::fs::PermissionsExt;
    let repo = repo_with_ticket();
    let dir = repo.path();
    let config = dir.join(".stapel/stapel.toml");
    put(dir, ".stapel/stapel.toml", "[x]\n");
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o000)).unwrap();
    // A process that can still read the file (root) cannot show the case.
    if std::fs::read_to_string(&config).is_ok() {
        return;
    }

    // `findings import` has no dialog: the guard allows it and the command gives the reason.
    common::hook_bash(dir, "stapel findings import in.jsonl", "auto")
        .success()
        .stdout("");
    let out = stapel(dir)
        .args(["ledger", "add", "--kind", "decision", "--note", "x"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let expected = String::from_utf8(out.stderr).unwrap();
    assert!(
        expected.starts_with("stapel: ") && expected.contains("stapel.toml: Permission denied"),
        "{expected}"
    );
    // A finding record has no dialog: the guard allows it.
    common::hook_bash(dir, "stapel ledger add --kind finding --note x", "auto")
        .success()
        .stdout("");
    common::hook_bash(dir, "stapel ledger add --kind decision --note x", "auto")
        .code(DENY)
        .stdout("")
        .stderr(expected);
    assert_eq!(grant_files(dir), 0);
}
