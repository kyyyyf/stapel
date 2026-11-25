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

#[test]
fn file_list() {
    use std::os::unix::fs::PermissionsExt;
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let path = fake_cargo_path(&scratch.path().join("bin"));
    let dir = repo.path();
    git_config(dir, "user.email", "test@example.invalid");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(dir.join("gone.rs"), "fn gone() {}\n").unwrap();
    std::fs::write(dir.join("sub/in.rs"), "fn inner() {}\n").unwrap();
    git(dir, &["add", "a.rs", "gone.rs", "sub/in.rs"]);
    git(dir, &["commit", "-q", "-m", "first"]);
    std::fs::remove_file(dir.join("gone.rs")).unwrap();

    std::fs::write(dir.join(".gitignore"), "ignored.rs\n").unwrap();
    std::fs::write(dir.join("ignored.rs"), "fn ignored() {}\n").unwrap();
    std::fs::write(dir.join("untracked.rs"), "fn untracked() {}\n").unwrap();
    std::fs::write(dir.join("sp ace.rs"), "fn spaced() {}\n").unwrap();
    std::fs::write(dir.join("é.rs"), "fn accent() {}\n").unwrap();
    std::fs::write(dir.join("notes.txt"), "text\n").unwrap();
    std::os::unix::fs::symlink("a.rs", dir.join("link.rs")).unwrap();
    let sized = |name: &str, size: usize| {
        let mut text = b"fn sized() {}\n".to_vec();
        text.resize(size, b' ');
        std::fs::write(dir.join(name), text).unwrap();
    };
    sized("edge.rs", 1_048_576);
    sized("big.rs", 1_048_577);
    std::fs::write(dir.join("locked.rs"), "fn locked() {}\n").unwrap();
    std::fs::set_permissions(dir.join("locked.rs"), std::fs::Permissions::from_mode(0)).unwrap();
    build_grammar(dir, &path, &home);

    // From the top: 6 indexed files (a, sub/in, untracked, sp ace, é, edge); skipped: .gitignore,
    // notes.txt, link.rs, big.rs and locked.rs.
    let counts = "rust: 6 files, 6 symbols\nskipped: 5 files\nsyntax errors: 0 files\n";
    let out = stapel(dir)
        .arg("index")
        .env("HOME", &home)
        .assert()
        .success();
    assert_eq!(stdout_of(out), counts);

    let stored = std::fs::read_to_string(dir.join(".stapel/index/index.json")).unwrap();
    let document: serde_json::Value = serde_json::from_str(&stored).unwrap();
    let mut names: Vec<&str> = document["files"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["a.rs", "edge.rs", "sp ace.rs", "sub/in.rs", "untracked.rs", "é.rs"]
    );

    // From a subfolder and a second time: the same answer.
    let out = stapel(&dir.join("sub"))
        .arg("index")
        .env("HOME", &home)
        .assert()
        .success();
    assert_eq!(stdout_of(out), counts);
    let again = std::fs::read_to_string(dir.join(".stapel/index/index.json")).unwrap();
    assert_eq!(again, stored);
    assert!(!dir.join("sub/.stapel").exists());
}

#[test]
fn missing_library() {
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let path = fake_cargo_path(&scratch.path().join("bin"));
    let dir = repo.path();
    std::fs::write(dir.join("a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(dir.join("b.rs"), "fn b() {}\n").unwrap();
    let line = "stapel: no grammar library for rust: run stapel grammar build";
    let expected = "skipped: 2 files\nsyntax errors: 0 files\n";

    // No grammar folder.
    let out = stapel(dir)
        .arg("index")
        .env("HOME", &home)
        .assert()
        .code(2);
    let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(err.contains(line), "stderr: {err}");
    assert_eq!(stdout_of(out), expected);

    // A record whose library file is gone.
    build_grammar(dir, &path, &home);
    let common = dir.join(".git/stapel/grammars");
    let mut removed = 0;
    for folder in std::fs::read_dir(&common).unwrap() {
        for entry in std::fs::read_dir(folder.unwrap().path()).unwrap() {
            let entry = entry.unwrap().path();
            if entry.extension().is_some_and(|e| e == "dylib" || e == "so") {
                std::fs::remove_file(entry).unwrap();
                removed += 1;
            }
        }
    }
    assert_eq!(removed, 1);
    let out = stapel(dir)
        .arg("index")
        .env("HOME", &home)
        .assert()
        .code(2);
    let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(err.contains(line), "stderr: {err}");
    assert_eq!(stdout_of(out), expected);
}

#[test]
fn no_rust_files_load_nothing() {
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let dir = repo.path();
    git_config(dir, "user.email", "test@example.invalid");
    let run = |counts: &str| {
        let out = stapel(dir)
            .arg("index")
            .env("HOME", &home)
            .assert()
            .success();
        let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
        assert!(!err.contains("grammar"), "stderr: {err}");
        assert_eq!(stdout_of(out), counts);
    };

    std::fs::write(dir.join("README.md"), "text\n").unwrap();
    run("skipped: 1 files\nsyntax errors: 0 files\n");

    // A tracked file that is deleted from the tree.
    std::fs::write(dir.join("a.rs"), "fn a() {}\n").unwrap();
    git(dir, &["add", "a.rs"]);
    git(dir, &["commit", "-q", "-m", "first"]);
    std::fs::remove_file(dir.join("a.rs")).unwrap();
    run("skipped: 1 files\nsyntax errors: 0 files\n");

    // Only a symlink with the extension.
    git(dir, &["rm", "-q", "--cached", "a.rs"]);
    std::os::unix::fs::symlink("README.md", dir.join("x.rs")).unwrap();
    run("skipped: 2 files\nsyntax errors: 0 files\n");
}

#[test]
fn refuses_a_changed_library() {
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    // A library of another crate: a proc-macro library that cargo built next to the test executable.
    let other = std::fs::read_dir(&deps)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.extension().is_some_and(|e| e == ext)
                && !p
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains("stapel_grammar_rust")
        })
        .expect("a library of another crate");
    let sha = |path: &Path| -> String {
        let out = std::process::Command::new("shasum")
            .args(["-a", "256"])
            .arg(path)
            .output()
            .unwrap();
        String::from_utf8(out.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_string()
    };

    let cases = ["bytes", "version", "text", "nosymbol"];
    for case in cases {
        let repo = git_repo();
        let scratch = tempfile::tempdir().unwrap();
        let home = scratch.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let path = fake_cargo_path(&scratch.path().join("bin"));
        let dir = repo.path();
        std::fs::write(dir.join("a.rs"), "fn a() {}\n").unwrap();
        build_grammar(dir, &path, &home);
        let folder = std::fs::read_dir(dir.join(".git/stapel/grammars"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .canonicalize()
            .unwrap();
        let record_path = folder.join("rust.json");
        let mut record: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&record_path).unwrap()).unwrap();
        let library = folder.join(record["file"].as_str().unwrap());
        let reason = match case {
            "bytes" => {
                let mut bytes = std::fs::read(&library).unwrap();
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
                std::fs::write(&library, bytes).unwrap();
                "content hash differs from its record".to_string()
            }
            "version" => {
                record["version"] = serde_json::json!("0.0.1-other");
                std::fs::write(&record_path, record.to_string()).unwrap();
                "grammar version 0.0.1-other is not ".to_string()
            }
            "text" => {
                std::fs::write(&library, "not a library\n").unwrap();
                record["sha256"] = serde_json::json!(sha(&library));
                std::fs::write(&record_path, record.to_string()).unwrap();
                "cannot load: ".to_string()
            }
            _ => {
                std::fs::copy(&other, &library).unwrap();
                record["sha256"] = serde_json::json!(sha(&library));
                std::fs::write(&record_path, record.to_string()).unwrap();
                "no symbol stapel_grammar_language".to_string()
            }
        };
        let out = stapel(dir)
            .arg("index")
            .env("HOME", &home)
            .assert()
            .code(2);
        let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
        let head = format!(
            "stapel: grammar library {} refused: {reason}",
            library.display()
        );
        assert!(err.contains(&head), "case {case}, stderr: {err}");
        assert!(
            err.contains("; run stapel grammar build"),
            "case {case}, stderr: {err}"
        );
        assert_eq!(
            stdout_of(out),
            "skipped: 1 files\nsyntax errors: 0 files\n",
            "case {case}"
        );
    }
}

#[test]
fn symbol_text() {
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let path = fake_cargo_path(&scratch.path().join("bin"));
    let dir = repo.path();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("B.rs"), "fn dup() {}\n").unwrap();
    std::fs::write(dir.join("a.rs"), "\nfn dup() {}\nfn dupe() {}\n").unwrap();
    std::fs::write(dir.join("sub/c.rs"), "\n\nstruct dup;\n").unwrap();
    build_grammar(dir, &path, &home);

    let out = stapel(dir)
        .args(["q", "symbol", "dup"])
        .env("HOME", &home)
        .assert()
        .success();
    assert_eq!(
        stdout_of(out),
        "B.rs:1 fn dup\na.rs:2 fn dup\nsub/c.rs:3 struct dup\n"
    );

    // The same from a subfolder: paths stay relative to the top of the tree.
    let out = stapel(&dir.join("sub"))
        .args(["q", "symbol", "dupe"])
        .env("HOME", &home)
        .assert()
        .success();
    assert_eq!(stdout_of(out), "a.rs:3 fn dupe\n");
}

#[test]
fn symbol_json() {
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let path = fake_cargo_path(&scratch.path().join("bin"));
    let dir = repo.path();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("B.rs"), "fn dup() {}\n").unwrap();
    std::fs::write(dir.join("a.rs"), "\nfn dup() {}\nfn dupe() {}\n").unwrap();
    std::fs::write(dir.join("sub/c.rs"), "\n\nstruct dup;\n").unwrap();

    // No library: the array is empty and the exit code is 2.
    let out = stapel(dir)
        .args(["q", "symbol", "dup", "--json"])
        .env("HOME", &home)
        .assert()
        .code(2);
    let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        err.contains("stapel: no grammar library for rust: run stapel grammar build"),
        "stderr: {err}"
    );
    let array: serde_json::Value = serde_json::from_str(&stdout_of(out)).unwrap();
    assert_eq!(array, serde_json::json!([]));

    build_grammar(dir, &path, &home);
    let text = stdout_of(
        stapel(dir)
            .args(["q", "symbol", "dup"])
            .env("HOME", &home)
            .assert()
            .success(),
    );
    let raw = stdout_of(
        stapel(dir)
            .args(["q", "symbol", "dup", "--json"])
            .env("HOME", &home)
            .assert()
            .success(),
    );
    assert_eq!(raw.matches('\n').count(), 1, "one JSON value: {raw}");
    let array: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let items = array.as_array().unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(items.len(), 3);
    assert_eq!(items.len(), lines.len());
    for (item, line) in items.iter().zip(lines) {
        let object = item.as_object().unwrap();
        let keys: Vec<&str> = object.keys().map(String::as_str).collect();
        assert_eq!(keys, ["path", "line", "kind", "name"]);
        let rebuilt = format!(
            "{}:{} {} {}",
            object["path"].as_str().unwrap(),
            object["line"].as_u64().unwrap(),
            object["kind"].as_str().unwrap(),
            object["name"].as_str().unwrap()
        );
        assert_eq!(rebuilt, line);
    }

    // Nothing found: the empty array, exit 1.
    let out = stapel(dir)
        .args(["q", "symbol", "nothing", "--json"])
        .env("HOME", &home)
        .assert()
        .code(1);
    assert_eq!(stdout_of(out).trim(), "[]");
}

#[test]
fn symbol_not_found() {
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let path = fake_cargo_path(&scratch.path().join("bin"));
    let dir = repo.path();
    std::fs::write(dir.join("a.rs"), "fn alpha() {}\n").unwrap();
    build_grammar(dir, &path, &home);

    for name in ["nothing", "Alpha", ""] {
        let out = stapel(dir)
            .args(["q", "symbol", name])
            .env("HOME", &home)
            .assert()
            .code(1);
        let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
        assert!(
            err.contains(&format!("stapel: no definition of {name}\n")),
            "stderr: {err}"
        );
        assert_eq!(stdout_of(out), "", "name {name:?}");
    }
    let out = stapel(dir)
        .args(["q", "symbol", "--", "-x"])
        .env("HOME", &home)
        .assert()
        .code(1);
    assert_eq!(stdout_of(out), "");

    // An index with a Rust name, then the record removed: no output, no message, exit 2.
    stapel(dir)
        .arg("index")
        .env("HOME", &home)
        .assert()
        .success();
    let mut records = 0;
    for folder in std::fs::read_dir(dir.join(".git/stapel/grammars")).unwrap() {
        let record = folder.unwrap().path().join("rust.json");
        std::fs::remove_file(record).unwrap();
        records += 1;
    }
    assert_eq!(records, 1);
    let out = stapel(dir)
        .args(["q", "symbol", "alpha"])
        .env("HOME", &home)
        .assert()
        .code(2);
    let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        err.contains("stapel: no grammar library for rust: run stapel grammar build"),
        "stderr: {err}"
    );
    assert!(!err.contains("no definition of"), "stderr: {err}");
    assert_eq!(stdout_of(out), "");

    // A build, then the name is found.
    build_grammar(dir, &path, &home);
    let out = stapel(dir)
        .args(["q", "symbol", "alpha"])
        .env("HOME", &home)
        .assert()
        .success();
    assert_eq!(stdout_of(out), "a.rs:1 fn alpha\n");
}

#[test]
fn outside_git() {
    let folder = common::bare_dir();
    let home = folder.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    for args in [
        vec!["grammar", "build"],
        vec!["index"],
        vec!["q", "symbol", "x"],
    ] {
        let out = stapel(folder.path())
            .args(&args)
            .env("HOME", &home)
            .assert()
            .code(1);
        let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
        assert!(
            err.contains("stapel: not a git working tree\n"),
            "{args:?}, stderr: {err}"
        );
        assert_eq!(stdout_of(out), "", "{args:?}");
    }
}

#[test]
fn sees_edits_outside_stapel() {
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let path = fake_cargo_path(&scratch.path().join("bin"));
    let dir = repo.path();
    std::fs::write(dir.join("a.rs"), "fn one() {}\n").unwrap();
    std::fs::write(dir.join("b.rs"), "fn two() {}\n").unwrap();
    build_grammar(dir, &path, &home);

    let query = |name: &str| {
        let out = stapel(dir)
            .args(["q", "symbol", name])
            .env("HOME", &home)
            .assert()
            .success();
        let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
        (stdout_of(out), err)
    };
    let (text, _) = query("one");
    assert_eq!(text, "a.rs:1 fn one\n");

    // Unchanged: no index line.
    let (text, err) = query("one");
    assert_eq!(text, "a.rs:1 fn one\n");
    assert!(!err.contains("index:"), "stderr: {err}");

    // A change, an addition and a removal outside stapel.
    std::fs::write(dir.join("a.rs"), "fn uno() {}\n").unwrap();
    std::fs::write(dir.join("c.rs"), "\nfn uno() {}\n").unwrap();
    std::fs::remove_file(dir.join("b.rs")).unwrap();
    let (text, err) = query("uno");
    assert_eq!(text, "a.rs:1 fn uno\nc.rs:2 fn uno\n");
    assert!(err.contains("index: 2 parsed, 1 removed\n"), "stderr: {err}");

    // The old names are gone.
    let out = stapel(dir)
        .args(["q", "symbol", "two"])
        .env("HOME", &home)
        .assert()
        .code(1);
    let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(!err.contains("index:"), "stderr: {err}");

    // Unchanged again: no index line.
    let (_, err) = query("uno");
    assert!(!err.contains("index:"), "stderr: {err}");
}

#[test]
fn rebuilds_on_version_change() {
    for case in ["stapel", "queries"] {
        let repo = git_repo();
        let scratch = tempfile::tempdir().unwrap();
        let home = scratch.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let path = fake_cargo_path(&scratch.path().join("bin"));
        let dir = repo.path();
        std::fs::write(dir.join("a.rs"), "fn one() {}\n").unwrap();
        std::fs::write(dir.join("b.rs"), "fn two() {}\n").unwrap();
        std::fs::write(dir.join("c.txt"), "text\n").unwrap();
        build_grammar(dir, &path, &home);
        let query = || {
            let out = stapel(dir)
                .args(["q", "symbol", "one"])
                .env("HOME", &home)
                .assert()
                .success();
            String::from_utf8(out.get_output().stderr.clone()).unwrap()
        };
        query();
        assert!(!query().contains("index:"), "case {case}");

        let index_path = dir.join(".stapel/index/index.json");
        let mut index: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&index_path).unwrap()).unwrap();
        if case == "stapel" {
            index["stapel"] = serde_json::json!("0.0.0-old");
        } else {
            index["queries"]["rust"] = serde_json::json!("0000");
        }
        std::fs::write(&index_path, index.to_string()).unwrap();

        let err = query();
        assert!(
            err.contains("index: 2 parsed, 0 removed\n"),
            "case {case}, stderr: {err}"
        );
        assert!(!query().contains("index:"), "case {case}");
    }
}

#[test]
fn reparses_on_grammar_change() {
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let path = fake_cargo_path(&scratch.path().join("bin"));
    let dir = repo.path();
    std::fs::write(dir.join("a.rs"), "fn one() {}\n").unwrap();
    std::fs::write(dir.join("b.rs"), "fn two() {}\n").unwrap();
    std::fs::write(dir.join("c.txt"), "text\n").unwrap();
    build_grammar(dir, &path, &home);
    let query = || {
        let out = stapel(dir)
            .args(["q", "symbol", "one"])
            .env("HOME", &home)
            .assert()
            .success();
        String::from_utf8(out.get_output().stderr.clone()).unwrap()
    };
    query();
    assert!(!query().contains("index:"));

    // The index was made with another library.
    let index_path = dir.join(".stapel/index/index.json");
    let mut index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&index_path).unwrap()).unwrap();
    index["libraries"]["rust"] = serde_json::json!("0000");
    std::fs::write(&index_path, index.to_string()).unwrap();

    let err = query();
    assert!(err.contains("index: 2 parsed, 0 removed\n"), "stderr: {err}");
    assert!(!query().contains("index:"));
}
