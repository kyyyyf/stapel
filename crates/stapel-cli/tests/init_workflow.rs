mod common;

fn version_with(value: Option<&str>) -> String {
    let dir = common::bare_dir();
    let mut cmd = common::stapel(dir.path());
    cmd.env_remove("STAPEL_TEST_BUILD_COMMIT");
    if let Some(v) = value {
        cmd.env("STAPEL_TEST_BUILD_COMMIT", v);
    }
    let out = cmd.arg("--version").assert().success().get_output().clone();
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn version_names_the_commit() {
    let version = env!("CARGO_PKG_VERSION");
    let sha40 = "0123456789abcdef0123456789abcdef01234567";
    let sha64 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    // clean
    assert_eq!(
        version_with(Some(sha40)),
        format!("stapel {version} ({sha40})\n")
    );
    assert_eq!(
        version_with(Some(sha64)),
        format!("stapel {version} ({sha64})\n")
    );
    // dirty
    let dirty = format!("{sha40}-dirty");
    assert_eq!(
        version_with(Some(&dirty)),
        format!("stapel {version} ({dirty})\n")
    );
    // missing: unset, empty, uppercase, 39 digits, other text
    let upper = sha40.to_uppercase();
    let short = &sha40[..39];
    for missing in [None, Some(""), Some(upper.as_str()), Some(short), Some("main")] {
        assert_eq!(version_with(missing), format!("stapel {version}\n"));
    }
}
