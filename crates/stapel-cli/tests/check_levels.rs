//! STP-35: the levels of `stapel check` and the points where `stapel next` demands one.

mod common;

use common::{read, stapel, stapel_repo};
use std::path::Path;

/// Adds a `[check]` section with the lines to the repository's `stapel.toml`.
fn add_to_check(dir: &Path, lines: &str) {
    let text = format!(
        "{}\n[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n{lines}\n",
        read(dir, ".stapel/stapel.toml")
    );
    std::fs::write(dir.join(".stapel/stapel.toml"), text).unwrap();
}

/// Exit code and the text of stderr of `stapel check` with the args.
fn check(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = stapel(dir).arg("check").args(args).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// AC-13: `points` holds `close` and only `step`, `review`, `close`; absent means all three.
#[test]
fn points_need_close() {
    let refusal = "check.points must hold close and only step, review, close";
    for bad in [
        "points = []",
        "points = [\"review\"]",
        "points = [\"step\", \"review\"]",
        "points = [\"close\", \"deploy\"]",
        "points = [\"Close\"]",
    ] {
        let repo = stapel_repo();
        add_to_check(repo.path(), bad);
        let (code, err) = check(repo.path(), &[]);
        assert_eq!(code, 2, "{bad}: {err}");
        assert!(err.contains(refusal), "{bad}: {err}");
    }
    for good in [
        "",
        "points = [\"close\"]",
        "points = [\"step\", \"close\"]",
        "points = [\"close\", \"close\", \"review\"]",
        "points = [\"step\", \"review\", \"close\"]",
    ] {
        let repo = stapel_repo();
        if !good.is_empty() {
            add_to_check(repo.path(), good);
        }
        let (_, err) = check(repo.path(), &[]);
        assert!(!err.contains("stapel.toml"), "{good}: {err}");
    }
}

// AC-7: the key `level` and the flag `--level` take `steps`, `head` or `full`, nothing else.
#[test]
fn refuses_an_unknown_level() {
    for bad in ["fast", "", "HEAD", "Full"] {
        let repo = stapel_repo();
        add_to_check(repo.path(), &format!("level = \"{bad}\""));
        let (code, err) = check(repo.path(), &[]);
        assert_eq!(code, 2, "key {bad:?}: {err}");
        assert!(
            err.contains(&format!(
                "check.level must be steps, head or full: {bad}"
            )),
            "key {bad:?}: {err}"
        );
    }
    let repo = stapel_repo();
    for good in ["steps", "head", "full"] {
        let repo = stapel_repo();
        add_to_check(repo.path(), &format!("level = \"{good}\""));
        let (_, err) = check(repo.path(), &[]);
        assert!(!err.contains("stapel.toml"), "key {good}: {err}");
    }
    for bad in ["fast", "", "HEAD"] {
        let (code, err) = check(repo.path(), &["--level", bad]);
        assert_eq!(code, 2, "flag {bad:?}: {err}");
        assert!(
            err.contains(&format!("stapel: --level must be steps, head or full: {bad}")),
            "flag {bad:?}: {err}"
        );
    }
    for other in ["--list", "--history"] {
        let (code, err) = check(repo.path(), &["--level", "head", other]);
        assert_eq!(code, 2, "{other}: {err}");
        assert!(
            err.contains("stapel: --level does not go with --list or --history"),
            "{other}: {err}"
        );
    }
}
