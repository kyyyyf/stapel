//! STP-44: `stapel index` and the queries. The library comes from the dev-dependency
//! `stapel-grammar-rust`, which cargo builds next to the test binaries; a fake `cargo` on `PATH` copies
//! it where `stapel grammar build` looks, so no test runs a nested cargo build (STP-4).

mod common;

use common::{git_config, git_repo, stapel};
use std::path::{Path, PathBuf};

/// A fake cargo in `bin` that copies the library cargo built next to the test executable into the
/// target folder of the build; returns the `PATH` that has it first.
fn fake_cargo_path(bin: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let deps: PathBuf = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let library = deps.join(format!("libstapel_grammar_rust.{ext}"));
    assert!(library.is_file(), "no library at {}", library.display());
    std::fs::create_dir_all(bin).unwrap();
    let script = format!(
        "#!/bin/sh\n\
         T=target\n\
         while [ $# -gt 0 ]; do\n\
         \x20 if [ \"$1\" = --target-dir ]; then T=\"$2\"; fi\n\
         \x20 shift\n\
         done\n\
         mkdir -p \"$T/release\"\n\
         cp '{lib}' \"$T/release/libstapel_grammar_rust.{ext}\"\n\
         exit 0\n",
        lib = library.display()
    );
    let cargo = bin.join("cargo");
    std::fs::write(&cargo, script).unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    format!("{}:{}", bin.display(), std::env::var("PATH").unwrap())
}

fn build_grammar(dir: &Path, path: &str, home: &Path) {
    stapel(dir)
        .args(["grammar", "build"])
        .env("PATH", path)
        .env("HOME", home)
        .env_remove("STAPEL_CARGO_LOCK_HELD")
        .assert()
        .success();
}

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn stdout_of(out: assert_cmd::assert::Assert) -> String {
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

#[test]
fn index_prints_counts() {
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let path = fake_cargo_path(&scratch.path().join("bin"));
    let dir = repo.path();
    std::fs::write(dir.join("two.rs"), "fn a() {}\nfn b() {}\n").unwrap();
    std::fs::write(dir.join("one.rs"), "struct S;\n").unwrap();
    std::fs::write(dir.join("none.rs"), "").unwrap();
    std::fs::write(dir.join("notes.txt"), "text\n").unwrap();
    build_grammar(dir, &path, &home);

    let out = stapel(dir)
        .arg("index")
        .env("HOME", &home)
        .assert()
        .success();
    assert_eq!(
        stdout_of(out),
        "rust: 3 files, 3 symbols\nskipped: 1 files\nsyntax errors: 0 files\n"
    );
}

#[test]
fn worktrees_share_the_grammar() {
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let path = fake_cargo_path(&scratch.path().join("bin"));
    let dir = repo.path();
    git_config(dir, "user.email", "test@example.invalid");
    std::fs::write(dir.join("a.rs"), "fn a() {}\n").unwrap();
    git(dir, &["add", "a.rs"]);
    git(dir, &["commit", "-q", "-m", "first"]);
    build_grammar(dir, &path, &home);

    let tree = scratch.path().join("tree");
    git(dir, &["worktree", "add", "-q", tree.to_str().unwrap()]);
    std::fs::write(tree.join("b.rs"), "fn b() {}\nfn c() {}\n").unwrap();

    // No cargo of any kind is reachable: the worktree must not build.
    let out = stapel(&tree)
        .arg("index")
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", &home)
        .assert()
        .success();
    assert_eq!(
        stdout_of(out),
        "rust: 2 files, 3 symbols\nskipped: 0 files\nsyntax errors: 0 files\n"
    );
}
