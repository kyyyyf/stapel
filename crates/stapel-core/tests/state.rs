//! STP-2 AC-12: state.json model, atomic write, legacy and unreadable files.

use stapel_core::state::{Loaded, State, load, save, save_with};
use std::path::Path;

fn sample(key: &str) -> State {
    State::new(key, "A title")
}

fn temps(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect()
}

fn loaded_state(path: &Path) -> State {
    match load(path) {
        Loaded::State(s) => s,
        other => panic!("{other:?}"),
    }
}

#[test]
fn failed_write_keeps_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    save(&path, &sample("ABC-1")).unwrap();
    let before = std::fs::read(&path).unwrap();

    let err = save_with(&path, &sample("ABC-2"), || Err("injected".to_string())).unwrap_err();
    assert!(err.contains("injected"), "{err}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(temps(dir.path()).is_empty(), "{:?}", temps(dir.path()));
}

#[test]
fn leftover_temp_is_ignored_and_removed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    save(&path, &sample("ABC-1")).unwrap();
    std::fs::write(dir.path().join(".state.json.99999.tmp"), "{garbage").unwrap();

    assert_eq!(loaded_state(&path).key, "ABC-1");
    save(&path, &sample("ABC-1")).unwrap();
    assert!(temps(dir.path()).is_empty(), "{:?}", temps(dir.path()));
}

#[test]
fn write_preserves_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    std::fs::write(
        &path,
        r#"{"schema_version":1,"key":"ABC-1","title":"t","confirmations":[
            {"section":"spec","by":"me","at":"2026-10-02T00:00:00Z","hash":"sha256:00","normal_form":1,"depends_on":{},"text":"x","future":7}
        ],"build":{"allowed":true,"by":"human"},"later":[1,2]}"#,
    )
    .unwrap();
    let state = loaded_state(&path);
    save(&path, &state).unwrap();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["build"]["allowed"], true);
    assert_eq!(v["later"], serde_json::json!([1, 2]));
    assert_eq!(v["confirmations"][0]["future"], 7);
}

#[test]
fn refuses_corrupt_or_unknown_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    std::fs::write(&path, "{ not json").unwrap();
    assert!(matches!(load(&path), Loaded::Unreadable(_)));
    std::fs::write(&path, r#"{"schema_version":2,"key":"ABC-1","title":"t"}"#).unwrap();
    match load(&path) {
        Loaded::Unreadable(reason) => assert!(reason.contains("schema_version"), "{reason}"),
        other => panic!("{other:?}"),
    }
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(load(&path), Loaded::Unreadable(_)));
}

#[test]
fn refuses_wrong_shape() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    std::fs::write(&path, r#"{"schema_version":1,"key":"ABC-1","title":"t","confirmations":"x"}"#).unwrap();
    assert!(matches!(load(&path), Loaded::Unreadable(_)));
}

#[test]
fn legacy_file_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    std::fs::write(&path, r#"{"key":"STP-1","build":{"allowed":false}}"#).unwrap();
    assert!(matches!(load(&path), Loaded::Legacy(_)));
}

#[cfg(unix)]
#[test]
fn write_fails_cleanly_on_readonly_dir() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    save(&path, &sample("ABC-1")).unwrap();
    let before = std::fs::read(&path).unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(dir.path().join("probe"), "x").is_ok() {
        eprintln!("skipped: running with permission to write into a read-only folder (root)");
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let result = save(&path, &sample("ABC-2"));
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
