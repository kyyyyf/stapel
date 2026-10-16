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
