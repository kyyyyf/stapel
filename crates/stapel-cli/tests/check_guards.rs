//! STP-41: guards, machine checks the agent cannot loosen unseen (step 1: the config).

mod common;

use common::{read, stapel, stapel_repo};
use std::path::Path;

/// Appends the text to the repository's `stapel.toml`, with a `[check]` section when asked.
fn add_config(dir: &Path, with_check: bool, tail: &str) {
    let check = if with_check {
        "\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n"
    } else {
        ""
    };
    let text = format!("{}{check}\n{tail}\n", read(dir, ".stapel/stapel.toml"));
    std::fs::write(dir.join(".stapel/stapel.toml"), text).unwrap();
}

/// Exit code and the text of stderr of `stapel check`.
fn check(dir: &Path) -> (i32, String) {
    let out = stapel(dir).arg("check").output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// AC-1: the dir is a relative folder outside .stapel, .git and .claude; guards need [check].
#[test]
fn refuses_a_bad_dir() {
    for dir in [
        "",
        "/abs",
        ".",
        "a/../b",
        "./g",
        "a//b",
        "a/./b",
        "a\\\\b",
        ".stapel/g",
        ".GIT",
        ".Claude/x",
    ] {
        let repo = stapel_repo();
        add_config(repo.path(), true, &format!("[guards]\ndir = \"{dir}\"\n"));
        let (code, err) = check(repo.path());
        let shown = dir.replace("\\\\", "\\");
        let refusal = format!(
            "guards.dir must be a relative folder of the repository outside .stapel, .git and .claude: {shown}"
        );
        assert_eq!(code, 2, "{dir}: {err}");
        assert!(err.contains(&refusal), "{dir}: {err}");
    }

    let repo = stapel_repo();
    add_config(repo.path(), false, "[guards]\ndir = \"guards\"\n");
    let (code, err) = check(repo.path());
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("guards need [check]"), "{err}");

    for dir in ["tests/guards", "guards/"] {
        let repo = stapel_repo();
        add_config(repo.path(), true, &format!("[guards]\ndir = \"{dir}\"\n"));
        let (_, err) = check(repo.path());
        assert!(
            !err.contains("stapel.toml") && !err.contains("guards"),
            "{dir}: {err}"
        );
    }
}

// AC-2: a guard needs a name and a command, a unique name and a relative baseline.
#[test]
fn refuses_bad_entries() {
    let entry = |name: &str, command: &str, baseline: Option<&str>| {
        let base = baseline.map_or(String::new(), |b| format!("baseline = \"{b}\"\n"));
        format!("[[guards.check]]\nname = \"{name}\"\ncommand = \"{command}\"\n{base}")
    };
    let head = "[guards]\ndir = \"guards\"\n";
    let need = "guards.check entries need a name and a command";
    let twice = "guards.check name is used twice: size";
    let cases: Vec<(String, String)> = vec![
        (entry("", "true", None), need.into()),
        (entry("size", "  ", None), need.into()),
        (
            format!(
                "{}{}",
                entry("size", "true", None),
                entry("size", "true", None)
            ),
            twice.into(),
        ),
    ]
    .into_iter()
    .chain(["", "/b", "../b", "a/./b", "a//b", "b/"].map(|b| {
        (
            entry("size", "true", Some(b)),
            format!("guards.check baseline must be a relative path inside guards.dir: {b}"),
        )
    }))
    .collect();
    for (entries, refusal) in cases {
        let repo = stapel_repo();
        add_config(repo.path(), true, &format!("{head}{entries}"));
        let (code, err) = check(repo.path());
        assert_eq!(code, 2, "{entries}: {err}");
        assert!(err.contains(&refusal), "{entries}: {err}");
    }

    let repo = stapel_repo();
    add_config(repo.path(), true, head);
    let (_, err) = check(repo.path());
    assert!(
        !err.contains("stapel.toml") && !err.contains("guards"),
        "{err}"
    );
}
