//! STP-27 AC-20 and AC-22: the dependency review of parallel tickets and the stops it writes.

mod common;

use common::{Pair, git_in, set_section, stapel, two_ticket_worktrees};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Writes `text` to `rel` in `dir` and commits it with git, outside any hook.
fn write_commit(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "-m", "wip"]);
}

/// `stapel next` in `dir`: its stdout.
fn next(dir: &Path) -> String {
    let out = stapel(dir).arg("next").assert().success().get_output().stdout.clone();
    String::from_utf8_lossy(&out).into_owned()
}

/// The pre-tool-use hook for a Write of `rel` in `dir`: the assertion on its answer.
fn write_hook(dir: &Path, rel: &str) -> assert_cmd::assert::Assert {
    let payload = json!({
        "tool_name": "Write",
        "cwd": dir,
        "permission_mode": "auto",
        "tool_input": { "file_path": dir.join(rel), "content": "x" },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .assert()
}

/// Writes the five sections of `key` in `dir`, confirms them, sets `[check]` and commits a Plan step
/// closed by a word commit: the next action is then `run-check`.
fn build_ready(dir: &Path, key: &str) {
    set_section(dir, key, "Description", "### Frame\n\nThe problem.");
    set_section(dir, key, "Spec", "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | The spec. | t |");
    set_section(dir, key, "Design", "The design.");
    set_section(dir, key, "Proof", "The proof.");
    set_section(dir, key, "Plan", "Route: light, auto.\n\n1. One.");
    let toml = dir.join(".stapel/stapel.toml");
    let mut text = std::fs::read_to_string(&toml).unwrap();
    text.push_str("\n[check]\nrunner = \"cargo\"\ntimeout_secs = 900\n");
    std::fs::write(&toml, text).unwrap();
    for section in ["spec", "design", "proof", "plan"] {
        stapel(dir).args(["ok", section]).assert().success();
    }
    write_commit(dir, "src/note.txt", "note\n");
    git_in(
        dir,
        &["commit", "-q", "--allow-empty", "-m", &format!("{key} step 1 skip: nothing")],
    );
}

const DEPEND_B: &str = "next: dependency-review — stapel depends ABC-2";
const RUN_CHECK_B: &str = "next: run-check — stapel check ABC-2";

/// The stop file of the pair: ABC-2 waits for ABC-1.
fn stop_file(pair: &Pair) -> PathBuf {
    pair.repo
        .path()
        .join(".git/stapel/stops/ABC-2/ABC-1-dependency.json")
}

/// The `round:` line of `stapel depends` in `dir`, without its label.
fn round_of(dir: &Path) -> String {
    let out = stapel(dir).arg("depends").assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).unwrap();
    text.lines()
        .find_map(|l| l.strip_prefix("round: "))
        .unwrap_or_else(|| panic!("no round line: {text}"))
        .to_string()
}

/// A dependency record of ABC-2.
fn record(id: &str, round: &str, severity: &str, waits: Option<&str>, claim: &str) -> Value {
    let mut v = json!({
        "id": id,
        "ticket": "ABC-2",
        "stage": "dependency",
        "round": round,
        "role": "depends",
        "severity": severity,
        "promise": "inside",
        "claim": claim,
        "catchable_at": "spec",
        "fate": "defer",
    });
    if let Some(waits) = waits {
        v["waits"] = json!(waits);
    }
    v
}

/// `stapel findings <verb> FILE` in `dir` with the lines of `records`, the file outside the tree.
fn put_records(dir: &Path, verb: &str, records: &[Value]) {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("in.jsonl");
    let text: String = records.iter().map(|r| format!("{r}\n")).collect();
    std::fs::write(&file, text).unwrap();
    stapel(dir)
        .args(["findings", verb])
        .arg(&file)
        .assert()
        .success();
}

/// The review of the pair's current round returned `records`.
fn review_returns(pair: &Pair, records: &[Value]) {
    put_records(&pair.b, "import", records);
}

/// A LOW record that finds no dependency.
fn nothing_found(id: &str, round: &str) -> Value {
    record(id, round, "LOW", None, "no dependency found")
}

fn stop_kind(pair: &Pair) -> String {
    let text = std::fs::read_to_string(stop_file(pair)).expect("the stop file");
    let v: Value = serde_json::from_str(&text).unwrap();
    assert!(v["files"].is_array(), "{text}");
    assert!(v["at"].is_string(), "{text}");
    v["kind"].as_str().unwrap().to_string()
}

/// Replaces the hash of the `spec` confirmation of `key` in `dir`.
fn set_hash(dir: &Path, key: &str, hash: &str) {
    let path = dir.join(format!(".stapel/tickets/{key}/state.json"));
    let mut state: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    state["confirmations"][0]["hash"] = json!(hash);
    std::fs::write(&path, state.to_string()).unwrap();
}

#[test]
fn dependency_review_named() {
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let b = pair.b.as_path();

    // No record of the round: the action, and a denied code write; `.stapel/` stays open.
    let said = next(b);
    assert!(said.lines().any(|l| l == DEPEND_B), "{said}");
    assert!(!said.contains("run-check"), "{said}");
    write_hook(b, "crates/x.rs")
        .code(2)
        .stderr(predicates::str::contains("dependency review"));
    write_hook(b, ".stapel/note.md").success();

    // `stapel depends` without KEY takes the branch ticket and prints the pack and the round.
    let out = stapel(b).arg("depends").assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.lines()
            .any(|l| l == "pack: .stapel/scratch/depends/ABC-2-ABC-1/pack.md"),
        "{text}"
    );
    let state = common::state_json(b, "ABC-2");
    let hash = state["confirmations"]
        .as_array()
        .unwrap()
        .iter()
        .rfind(|c| c["section"] == "spec")
        .unwrap()["hash"]
        .as_str()
        .unwrap()
        .to_string();
    let hex: String = hash
        .trim_start_matches("sha256:")
        .chars()
        .take(8)
        .collect();
    assert!(
        text.lines().any(|l| l == format!("round: ABC-1:h:{hex}")),
        "{text}"
    );
    assert!(b.join(".stapel/scratch/depends/ABC-2-ABC-1/pack.md").is_file());

    // A record of the round: the action goes away and the write passes.
    let round = round_of(b);
    review_returns(&pair, &[nothing_found("D-1", &round)]);
    let said = next(b);
    assert!(said.lines().any(|l| l == RUN_CHECK_B), "{said}");
    assert!(!said.contains("dependency-review"), "{said}");
    write_hook(b, "crates/x.rs").success();

    // The earlier ticket has no earlier parallel ticket: the refusal, exit 2.
    stapel(&pair.a)
        .arg("depends")
        .assert()
        .code(2)
        .stderr(predicates::str::contains("ABC-1 has no earlier parallel ticket"));
    stapel(&pair.a)
        .args(["depends", "ABC-1"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("ABC-1 has no earlier parallel ticket"));

    // A KEY of another ticket than the branch ticket is refused.
    stapel(b).args(["depends", "ABC-1"]).assert().code(2);
}

#[test]
fn pack_holds_both_tickets() {
    let pair = two_ticket_worktrees();
    let (a, b) = (pair.a.as_path(), pair.b.as_path());
    set_section(a, "ABC-1", "Description", "### Frame\n\nProblem of the first.");
    set_section(
        a,
        "ABC-1",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | WHEN a THE first criterion holds. | `t::a` |",
    );
    set_section(
        a,
        "ABC-1",
        "Plan",
        "Route: light, auto.\n\n1. Change `src/shared.txt` and `crates/zzz/lib.rs` (AC-1; test `t::a`).",
    );
    set_section(b, "ABC-2", "Description", "### Frame\n\nProblem of the second.");
    set_section(
        b,
        "ABC-2",
        "Spec",
        "| № | Criterion | Test |\n|---|---|---|\n| AC-1 | WHEN b THE second criterion holds. | `t::b` |",
    );
    set_section(
        b,
        "ABC-2",
        "Plan",
        "Route: light, auto.\n\n1. Change `src/main.rs` (AC-1).",
    );
    write_commit(
        a,
        "src/shared.txt",
        &(1..=10)
            .map(|i| if i == 2 { "from a\n".to_string() } else { format!("line {i}\n") })
            .collect::<String>(),
    );
    write_commit(
        b,
        "src/main.rs",
        &(1..=20)
            .map(|i| if i == 1 { "from b\n".to_string() } else { format!("line {i}\n") })
            .collect::<String>(),
    );

    let out = stapel(b).arg("depends").assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).unwrap();
    let path = text
        .lines()
        .find_map(|l| l.strip_prefix("pack: "))
        .unwrap_or_else(|| panic!("no pack line: {text}"));
    assert!(text.contains("round: ABC-1:h:h"), "{text}");
    let pack = std::fs::read_to_string(b.join(path)).unwrap();

    // Both Frames, criteria and Plans.
    for needle in [
        "Problem of the first.",
        "Problem of the second.",
        "WHEN a THE first criterion holds.",
        "WHEN b THE second criterion holds.",
        "Change `src/shared.txt` and `crates/zzz/lib.rs`",
        "Change `src/main.rs`",
    ] {
        assert!(pack.contains(needle), "{needle}: {pack}");
    }
    // The named files of both Plans, each from its own checkout; a test name is no file.
    assert!(pack.contains("from a"), "{pack}");
    assert!(pack.contains("from b"), "{pack}");
    assert!(pack.contains("src/shared.txt"), "{pack}");
    assert!(pack.contains("src/main.rs"), "{pack}");
    assert!(pack.contains("crates/zzz/lib.rs"), "{pack}");
    // The earlier ticket's diff against the base branch.
    assert!(pack.contains("diff --git a/src/shared.txt b/src/shared.txt"), "{pack}");
    assert!(pack.contains("+from a"), "{pack}");
    assert!(!pack.contains("+from b"), "{pack}");

    // The pack folder leaves no change in the tree.
    let status = git_in(b, &["status", "--porcelain", "--untracked-files=all"]);
    assert!(!status.contains("scratch"), "{status}");
}

#[test]
fn confirmed_dependency_stops() {
    // A MEDIUM record with `waits`: the kind of the stop is that of its waits; both give `merge`.
    for (waits, kind) in [
        (vec!["spec"], "spec"),
        (vec!["merge"], "merge"),
        (vec!["spec", "merge"], "merge"),
    ] {
        let pair = two_ticket_worktrees();
        build_ready(&pair.b, "ABC-2");
        let round = round_of(&pair.b);
        assert!(!stop_file(&pair).exists());
        let records: Vec<Value> = waits
            .iter()
            .enumerate()
            .map(|(i, w)| record(&format!("D-{i}"), &round, "MEDIUM", Some(w), "a dependency"))
            .collect();
        review_returns(&pair, &records);
        assert_eq!(stop_kind(&pair), kind, "{waits:?}");
    }

    // A verdict of the triage grades the record: `medium` keeps the stop.
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let round = round_of(&pair.b);
    review_returns(&pair, &[record("D-1", &round, "HIGH", Some("spec"), "a dependency")]);
    put_records(
        &pair.b,
        "triage",
        &[json!({"id": "D-1", "ticket": "ABC-2", "fate": "defer", "verdict": "medium"})],
    );
    assert_eq!(stop_kind(&pair), "spec");

    // A HIGH record whose `waits` a triage edit lost: the stop has the kind `merge` (D14-4).
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let round = round_of(&pair.b);
    review_returns(&pair, &[record("D-1", &round, "HIGH", Some("spec"), "a dependency")]);
    assert_eq!(stop_kind(&pair), "spec");
    let findings = pair.b.join(".stapel/tickets/ABC-2/findings.jsonl");
    let lost: String = std::fs::read_to_string(&findings)
        .unwrap()
        .lines()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            v.as_object_mut().unwrap().remove("waits");
            format!("{v}\n")
        })
        .collect();
    std::fs::write(&findings, lost).unwrap();
    put_records(
        &pair.b,
        "triage",
        &[json!({"id": "D-1", "ticket": "ABC-2", "fate": "defer", "verdict": "high"})],
    );
    assert_eq!(stop_kind(&pair), "merge");

    // Only LOW records: no stop.
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let round = round_of(&pair.b);
    review_returns(&pair, &[nothing_found("D-1", &round)]);
    assert!(!stop_file(&pair).exists());
    let said = next(&pair.b);
    assert!(said.lines().any(|l| l == RUN_CHECK_B), "{said}");

    // The earlier spec confirmed again: a new round, and the review is asked again.
    set_hash(&pair.a, "ABC-1", "feedfacefeedface");
    let said = next(&pair.b);
    assert!(said.lines().any(|l| l == DEPEND_B), "{said}");
    assert_ne!(round_of(&pair.b), round);
    write_hook(&pair.b, "crates/x.rs").code(2);
}

#[test]
fn false_verdict_lifts_the_stop() {
    // A triage with the verdict `false` and a refutation removes the stop.
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let round = round_of(&pair.b);
    review_returns(&pair, &[record("D-1", &round, "MEDIUM", Some("merge"), "a dependency")]);
    assert_eq!(stop_kind(&pair), "merge");
    put_records(
        &pair.b,
        "triage",
        &[json!({
            "id": "D-1",
            "ticket": "ABC-2",
            "fate": "no-change",
            "verdict": "false",
            "evidence": "the two tickets touch no common file",
        })],
    );
    assert!(!stop_file(&pair).exists());
    let said = next(&pair.b);
    assert!(said.lines().any(|l| l == RUN_CHECK_B), "{said}");

    // One false and one kept record: the stop stays, with the kind of the kept record.
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let round = round_of(&pair.b);
    review_returns(
        &pair,
        &[
            record("D-1", &round, "MEDIUM", Some("merge"), "a dependency"),
            record("D-2", &round, "MEDIUM", Some("spec"), "another dependency"),
        ],
    );
    assert_eq!(stop_kind(&pair), "merge");
    put_records(
        &pair.b,
        "triage",
        &[json!({
            "id": "D-1",
            "ticket": "ABC-2",
            "fate": "no-change",
            "verdict": "false",
            "evidence": "the two tickets touch no common file",
        })],
    );
    assert_eq!(stop_kind(&pair), "spec");

    // A new round with only `no dependency found`: the stop is removed.
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let round = round_of(&pair.b);
    review_returns(&pair, &[record("D-1", &round, "HIGH", Some("merge"), "a dependency")]);
    assert_eq!(stop_kind(&pair), "merge");
    set_hash(&pair.a, "ABC-1", "feedfacefeedface");
    let new_round = round_of(&pair.b);
    assert_ne!(new_round, round);
    review_returns(&pair, &[nothing_found("D-2", &new_round)]);
    assert!(!stop_file(&pair).exists());
}

#[test]
fn merge_review_after_merge_main() {
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let b = pair.b.as_path();
    let main = pair.repo.path();
    let round = round_of(b);
    review_returns(&pair, &[nothing_found("D-1", &round)]);

    // The base branch gains a close of ABC-1; ABC-2 merges it.
    let base = git_in(main, &["symbolic-ref", "--short", "HEAD"]).trim().to_string();
    let close_on_base = |key: &str| {
        let rel = format!(".stapel/tickets/{key}/state.json");
        let path = main.join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let closed = json!({
            "key": key,
            "closed": {"by": "test-user", "at": "2026-01-02T10:00:00Z", "reason": "done"},
        });
        std::fs::write(&path, closed.to_string()).unwrap();
        git_in(main, &["add", &rel]);
        git_in(main, &["commit", "-q", "-m", "close"]);
    };
    let pass_at_head = || {
        let head = git_in(b, &["rev-parse", "HEAD"]).trim().to_string();
        let rec = json!({
            "v": 1, "kind": "check", "id": "c1", "at": "2026-10-19T10:00:00Z",
            "ticket": "ABC-2", "head": head, "result": "pass", "steps": [],
        });
        std::fs::write(b.join(".stapel/tickets/ABC-2/runs.jsonl"), format!("{rec}\n")).unwrap();
    };
    let merge_round = || {
        let merge = git_in(b, &["rev-list", "--merges", "-n", "1", "HEAD"]);
        format!("merge:{}", &merge.trim()[..8])
    };
    close_on_base("ABC-1");
    git_in(b, &["merge", "-q", "--no-edit", &base]);
    let first = merge_round();

    // The code review was done before the merge and stays valid.
    set_section(b, "ABC-2", "Review", "Reviewed: no findings.");
    set_section(b, "ABC-2", "Summary", "Done.");
    git_in(b, &["add", "-A"]);
    git_in(b, &["commit", "-q", "-m", "ABC-2: review and summary"]);
    pass_at_head();
    let ask = |round: &str| format!("next: code-review — stapel review start ABC-2 --round {round}");
    let said = next(b);
    assert!(said.lines().any(|l| l == ask("merge")), "{said}");

    // `close` refuses until the merge round has an import.
    stapel(b)
        .args(["close", "ABC-2", "--reason", "done"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("merge review"));

    // The pack holds the ticket diff against the base branch and the merge diff.
    stapel(b)
        .args(["review", "start", "ABC-2", "--round", "merge"])
        .assert()
        .success();
    let folder = b.join(".stapel/scratch/review/ABC-2-rmerge");
    let pack = std::fs::read_to_string(folder.join("pack.md")).unwrap();
    assert!(pack.contains(&first), "{pack}");
    let ticket_diff = std::fs::read_to_string(folder.join("diff.patch")).unwrap();
    assert!(ticket_diff.contains("src/note.txt"), "{ticket_diff}");
    assert!(!ticket_diff.contains("tickets/ABC-1/state.json"), "{ticket_diff}");
    let merge_diff = std::fs::read_to_string(folder.join("merge.patch")).unwrap();
    assert!(merge_diff.contains("tickets/ABC-1/state.json"), "{merge_diff}");
    assert!(!merge_diff.contains("src/note.txt"), "{merge_diff}");
    assert!(std::fs::read_to_string(b.join(".stapel/scratch/review/ABC-2-rmerge/pack.md")).is_ok());
    let said = next(b);
    assert!(said.lines().any(|l| l == ask("merge")), "{said}");

    // An import of that merge round: the close is allowed.
    let mut found = nothing_found("M-1", &first);
    found["stage"] = json!("code-review");
    found["role"] = json!("reviewer");
    put_records(b, "import", &[found]);
    let said = next(b);
    assert!(
        said.lines().any(|l| l.starts_with("next: close — ")),
        "{said}"
    );
    assert!(!said.contains("--round merge"), "{said}");

    // A second merge of main with a third ticket's close asks again.
    close_on_base("ABC-3");
    git_in(b, &["merge", "-q", "--no-edit", &base]);
    let second = merge_round();
    assert_ne!(first, second);
    pass_at_head();
    let said = next(b);
    assert!(said.lines().any(|l| l == ask("merge")), "{said}");
    stapel(b)
        .args(["close", "ABC-2", "--reason", "done"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("merge review"));
    stapel(b)
        .args(["review", "start", "ABC-2", "--round", "merge"])
        .assert()
        .success();
    let pack = std::fs::read_to_string(folder.join("pack.md")).unwrap();
    assert!(pack.contains(&second), "{pack}");
    let mut found = nothing_found("M-2", &second);
    found["stage"] = json!("code-review");
    found["role"] = json!("reviewer");
    put_records(b, "import", &[found]);
    stapel(b)
        .args(["close", "ABC-2", "--reason", "done"])
        .assert()
        .success();
}

#[test]
fn stop_edges() {
    // A pending round and a stop at once: `dependency-review` comes before `wait`, in next and in
    // the guard; the older round's stop stays until the new round has a record (D14-1, D14-2).
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let round = round_of(&pair.b);
    review_returns(&pair, &[record("D-1", &round, "MEDIUM", Some("spec"), "a dependency")]);
    assert_eq!(stop_kind(&pair), "spec");
    set_hash(&pair.a, "ABC-1", "feedfacefeedface");
    let new_round = round_of(&pair.b);
    assert_ne!(new_round, round);
    assert!(stop_file(&pair).exists());
    let said = next(&pair.b);
    assert!(said.lines().any(|l| l == DEPEND_B), "{said}");
    assert!(!said.lines().any(|l| l.starts_with("next: wait")), "{said}");
    write_hook(&pair.b, "crates/x.rs")
        .code(2)
        .stderr(predicates::str::contains("stapel depends"));
    // The stop of the older round stays while the new round has no record.
    assert_eq!(stop_kind(&pair), "spec");

    // Records in the old and in the new round: only the new round counts.
    review_returns(&pair, &[record("D-2", &new_round, "MEDIUM", Some("merge"), "again")]);
    assert_eq!(stop_kind(&pair), "merge");
    let said = next(&pair.b);
    assert!(!said.lines().any(|l| l == DEPEND_B), "{said}");

    // A triage that grades the only record down to LOW lifts the stop (D14-6).
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let round = round_of(&pair.b);
    review_returns(&pair, &[record("D-1", &round, "HIGH", Some("merge"), "a dependency")]);
    assert_eq!(stop_kind(&pair), "merge");
    put_records(
        &pair.b,
        "triage",
        &[json!({"id": "D-1", "ticket": "ABC-2", "fate": "defer", "verdict": "low"})],
    );
    assert!(!stop_file(&pair).exists());
    let said = next(&pair.b);
    assert!(said.lines().any(|l| l == RUN_CHECK_B), "{said}");
}

#[test]
fn pack_cut() {
    let pair = two_ticket_worktrees();
    let (a, b) = (pair.a.as_path(), pair.b.as_path());
    set_section(
        a,
        "ABC-1",
        "Plan",
        "Route: light, auto.\n\n1. Change `src/big.txt`, `src/gone.txt` and `src/blob.bin` (AC-1).",
    );
    set_section(b, "ABC-2", "Description", "### Frame\n\nProblem of the second.");
    set_section(b, "ABC-2", "Plan", "Route: light, auto.\n\n1. Change `src/main.rs` (AC-1).");
    // 70 KiB: the head is kept, the tail after 64 KiB is not.
    let mut big = String::from("HEADMARK\n");
    while big.len() < 70 * 1024 - 9 {
        big.push_str("0123456789012345678901234567890123456789012345678901234567890123\n");
    }
    big.push_str("TAILMARK\n");
    assert!(big.len() > 70 * 1024 - 100);
    write_commit(a, "src/big.txt", &big);
    // Not UTF-8.
    /// A small file that is not UTF-8.
    fn dir_blob() -> Vec<u8> {
        vec![0xff, 0xfe, 0x00, 0xc3, 0x28, 0x41]
    }
    let blob = dir_blob();
    std::fs::write(a.join("src/blob.bin"), &blob).unwrap();
    git_in(a, &["add", "-A"]);
    git_in(a, &["commit", "-q", "-m", "blob"]);
    write_commit(b, "src/main.rs", "fn main() {}\n");

    let out = stapel(b).arg("depends").assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).unwrap();
    let path = text
        .lines()
        .find_map(|l| l.strip_prefix("pack: "))
        .unwrap_or_else(|| panic!("no pack line: {text}"));
    let pack = std::fs::read_to_string(b.join(path)).unwrap();

    let big_part = pack
        .split("#### src/big.txt")
        .nth(1)
        .unwrap_or_else(|| panic!("no big.txt part: {pack}"));
    let big_part = big_part.split("\n#### ").next().unwrap();
    assert!(big_part.contains("HEADMARK"), "{big_part}");
    assert!(!big_part.contains("TAILMARK"), "{big_part}");
    assert!(big_part.contains("(cut after 65536 bytes)"), "{big_part}");
    assert!(big_part.len() < 66 * 1024 + 200, "{}", big_part.len());

    let gone = pack.split("#### src/gone.txt").nth(1).unwrap_or_else(|| panic!("{pack}"));
    let gone = gone.split("\n#### ").next().unwrap();
    assert!(gone.contains("(no such file in this checkout)"), "{gone}");

    let blob_part = pack.split("#### src/blob.bin").nth(1).unwrap_or_else(|| panic!("{pack}"));
    let blob_part = blob_part.split("\n#### ").next().unwrap();
    assert!(blob_part.contains("(not UTF-8 text)"), "{blob_part}");
}

#[test]
fn merge_review_edges() {
    // The merge ask comes before the summary; round-1 findings imported before the merge stay
    // valid; `close` with a grant still refuses; a stale first-round import does not satisfy a
    // later merge round (D15-1, D15-3).
    let pair = two_ticket_worktrees();
    build_ready(&pair.b, "ABC-2");
    let b = pair.b.as_path();
    let main = pair.repo.path();
    let round = round_of(b);
    review_returns(&pair, &[nothing_found("D-1", &round)]);

    // A numeric code review round is imported before the merge.
    let mut r1 = nothing_found("R-1", "1");
    r1["stage"] = json!("code-review");
    r1["role"] = json!("reviewer");
    put_records(b, "import", &[r1]);

    let base = git_in(main, &["symbolic-ref", "--short", "HEAD"]).trim().to_string();
    let close_on_base = |key: &str| {
        let rel = format!(".stapel/tickets/{key}/state.json");
        let path = main.join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let closed = json!({
            "key": key,
            "closed": {"by": "test-user", "at": "2026-01-02T10:00:00Z", "reason": "done"},
        });
        std::fs::write(&path, closed.to_string()).unwrap();
        git_in(main, &["add", &rel]);
        git_in(main, &["commit", "-q", "-m", "close"]);
    };
    let pass_at_head = || {
        let head = git_in(b, &["rev-parse", "HEAD"]).trim().to_string();
        let rec = json!({
            "v": 1, "kind": "check", "id": "c1", "at": "2026-10-19T10:00:00Z",
            "ticket": "ABC-2", "head": head, "result": "pass", "steps": [],
        });
        std::fs::write(b.join(".stapel/tickets/ABC-2/runs.jsonl"), format!("{rec}\n")).unwrap();
    };
    let merge_round = || {
        let merge = git_in(b, &["rev-list", "--merges", "-n", "1", "HEAD"]);
        format!("merge:{}", &merge.trim()[..8])
    };
    let ask = "next: code-review — stapel review start ABC-2 --round merge";
    close_on_base("ABC-1");
    git_in(b, &["merge", "-q", "--no-edit", &base]);
    let first = merge_round();
    set_section(b, "ABC-2", "Review", "Reviewed: no findings.");
    git_in(b, &["add", "-A"]);
    git_in(b, &["commit", "-q", "-m", "ABC-2: review"]);
    pass_at_head();

    // No summary yet: the merge ask comes first, and the round-1 import is not asked again.
    let said = next(b);
    assert!(said.lines().any(|l| l == ask), "{said}");
    assert!(!said.contains("write-summary"), "{said}");
    assert!(!said.contains("--round 1"), "{said}");

    // `close` with a grant refuses on the merge review before it looks at the grant.
    stapel(b)
        .args(["close", "ABC-2", "--reason", "done", "--grant", "bogus"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("merge review"));

    // The import of the first merge round satisfies it; the summary is asked next.
    let mut m1 = nothing_found("M-1", &first);
    m1["stage"] = json!("code-review");
    m1["role"] = json!("reviewer");
    put_records(b, "import", &[m1]);
    let said = next(b);
    assert!(!said.contains("--round merge"), "{said}");
    assert!(said.contains("write-summary"), "{said}");

    // A second merge round: the import of the first one does not satisfy it.
    close_on_base("ABC-3");
    git_in(b, &["merge", "-q", "--no-edit", &base]);
    let second = merge_round();
    assert_ne!(first, second);
    pass_at_head();
    let said = next(b);
    assert!(said.lines().any(|l| l == ask), "{said}");
    stapel(b)
        .args(["close", "ABC-2", "--reason", "done", "--grant", "bogus"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("merge review"));
}
