//! STP-44: `stapel grammar build`. A fake `cargo` first on `PATH` writes a library and copies what it
//! finds; no test runs a real nested cargo build (STP-4).

mod common;

use common::{git_repo, stapel};
use sha2::{Digest, Sha256};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// The version of the crate `name` in the lock file `text`.
fn locked_version(text: &str, name: &str) -> String {
    let lock: toml::Table = text.parse().unwrap();
    let packages = lock["package"].as_array().unwrap();
    packages
        .iter()
        .find(|p| p["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("no {name} in the lock"))["version"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn builds_into_the_common_folder() {
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let bin = scratch.path().join("bin");
    let ctl = scratch.path().join("ctl");
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&ctl).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    // The fake cargo: logs its arguments and folder, copies the manifest and the lock file it finds,
    // copies the holder lines of the cargo lock, then writes a library where cargo would.
    let script = format!(
        "#!/bin/sh\n\
         D='{ctl}'\n\
         echo \"$*\" >> \"$D/args\"\n\
         pwd >> \"$D/pwd\"\n\
         cp Cargo.toml \"$D/manifest\"\n\
         cp Cargo.lock \"$D/lock\"\n\
         if [ -f src/lib.rs ]; then echo yes > \"$D/has_lib\"; fi\n\
         cat \"$HOME\"/.cache/stapel/cargo*.lock > \"$D/holders\" 2>/dev/null\n\
         T=target\n\
         while [ $# -gt 0 ]; do\n\
         \x20 if [ \"$1\" = --target-dir ]; then T=\"$2\"; fi\n\
         \x20 shift\n\
         done\n\
         mkdir -p \"$T/release\"\n\
         printf 'fake library bytes' > \"$T/release/libstapel_grammar_rust.{ext}\"\n\
         exit 0\n",
        ctl = ctl.display()
    );
    let cargo = bin.join("cargo");
    std::fs::write(&cargo, script).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());

    let run = || {
        stapel(repo.path())
            .args(["grammar", "build"])
            .env("PATH", &path)
            .env("HOME", &home)
            // The tests run under `stapel cargo`, which sets the mark; the build must take a slot.
            .env_remove("STAPEL_CARGO_LOCK_HELD")
            .assert()
    };
    let out = run().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();

    let pinned = locked_version(
        &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"))
            .unwrap(),
        "tree-sitter-rust",
    );
    let digest = Sha256::digest(b"fake library bytes");
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let name = format!("rust-{}.{ext}", &hex[..16]);
    let grammars = repo.path().join(".git/stapel/grammars");
    let folder = grammars.join(env!("CARGO_PKG_VERSION"));
    let library = folder.join(&name);

    // The printed line names the pinned version and the library path.
    let line = stdout.trim_end();
    let prefix = format!("built: rust {pinned} ");
    let printed = line
        .strip_prefix(&prefix)
        .unwrap_or_else(|| panic!("line {line:?}"));
    assert_eq!(
        PathBuf::from(printed).canonicalize().unwrap(),
        library.canonicalize().unwrap()
    );
    assert_eq!(std::fs::read(&library).unwrap(), b"fake library bytes");

    // The record names the version, the file and the hash.
    let record = std::fs::read_to_string(folder.join("rust.json")).unwrap();
    assert!(record.contains(&pinned), "record {record}");
    assert!(record.contains(&name), "record {record}");
    assert!(record.contains(&hex), "record {record}");

    // The grammar folder holds the library and the record, nothing else; no build folder is left.
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut want = vec![name.clone(), "rust.json".to_string()];
    want.sort();
    assert_eq!(names, want);
    let others: Vec<String> = std::fs::read_dir(&grammars)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(others, [env!("CARGO_PKG_VERSION")], "no build folder is left");

    // What cargo saw.
    let args = std::fs::read_to_string(ctl.join("args")).unwrap();
    assert!(
        args.starts_with("build --release --locked"),
        "cargo arguments {args:?}"
    );
    let built_in = std::fs::read_to_string(ctl.join("pwd")).unwrap();
    assert!(
        built_in.contains("/stapel/grammars/build-"),
        "build folder {built_in:?}"
    );
    assert!(ctl.join("has_lib").exists(), "src/lib.rs is written");
    let manifest = std::fs::read_to_string(ctl.join("manifest")).unwrap();
    assert!(
        manifest.lines().any(|l| l.trim() == "[workspace]"),
        "manifest {manifest}"
    );
    assert!(!manifest.contains(".workspace = true"), "manifest {manifest}");
    let lock = std::fs::read(ctl.join("lock")).unwrap();
    let wrapper_lock = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../stapel-grammar-rust/grammar.lock"),
    )
    .unwrap();
    assert_eq!(lock, wrapper_lock, "grammar.lock is the Cargo.lock");
    let holders = std::fs::read_to_string(ctl.join("holders")).unwrap();
    assert!(!holders.trim().is_empty(), "a slot of the cargo lock is held");

    // A second build of the same bytes keeps the library file.
    let inode = std::fs::metadata(&library).unwrap().ino();
    run().success();
    assert_eq!(std::fs::metadata(&library).unwrap().ino(), inode);
    assert_eq!(std::fs::read(&library).unwrap(), b"fake library bytes");
}

#[test]
fn a_failed_build_leaves_the_folder() {
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let repo = git_repo();
    let scratch = tempfile::tempdir().unwrap();
    let bin = scratch.path().join("bin");
    let ctl = scratch.path().join("ctl");
    let home = scratch.path().join("home");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&ctl).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    // The fake cargo reads its mode from a file: `ok` writes a library whose bytes are the mode
    // file's second line, `fail` prints and exits 1, `empty` exits 0 and writes no library.
    let script = format!(
        "#!/bin/sh\n\
         D='{ctl}'\n\
         MODE=$(head -n 1 \"$D/mode\")\n\
         BYTES=$(tail -n 1 \"$D/mode\")\n\
         T=target\n\
         while [ $# -gt 0 ]; do\n\
         \x20 if [ \"$1\" = --target-dir ]; then T=\"$2\"; fi\n\
         \x20 shift\n\
         done\n\
         if [ \"$MODE\" = fail ]; then echo 'fake cargo exploded'; echo 'fake cargo stderr' >&2; exit 1; fi\n\
         if [ \"$MODE\" = empty ]; then exit 0; fi\n\
         mkdir -p \"$T/release\"\n\
         printf '%s' \"$BYTES\" > \"$T/release/libstapel_grammar_rust.{ext}\"\n\
         exit 0\n",
        ctl = ctl.display()
    );
    let cargo = bin.join("cargo");
    std::fs::write(&cargo, script).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let run = || {
        stapel(repo.path())
            .args(["grammar", "build"])
            .env("PATH", &path)
            .env("HOME", &home)
            .env_remove("STAPEL_CARGO_LOCK_HELD")
            .assert()
    };
    let grammars = repo.path().join(".git/stapel/grammars");
    let folder = grammars.join(env!("CARGO_PKG_VERSION"));
    let snapshot = |dir: &Path| -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    };

    // An earlier build gives a library and a record.
    std::fs::write(ctl.join("mode"), "ok\nearlier bytes").unwrap();
    run().success();
    let before = snapshot(&folder);
    assert_eq!(before.len(), 2, "the library and the record");

    for mode in ["fail", "empty"] {
        std::fs::write(ctl.join("mode"), format!("{mode}\nlater bytes")).unwrap();
        let out = run().code(1);
        let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
        let all = format!("{stdout}{stderr}");
        assert!(
            all.contains("stapel: grammar build failed for rust"),
            "{mode}: {all}"
        );
        if mode == "fail" {
            assert!(all.contains("fake cargo exploded"), "{mode}: {all}");
            assert!(all.contains("fake cargo stderr"), "{mode}: {all}");
        }
        assert_eq!(snapshot(&folder), before, "{mode}: the folder is unchanged");
        let others: Vec<String> = std::fs::read_dir(&grammars)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            others,
            [env!("CARGO_PKG_VERSION")],
            "{mode}: no build folder is left"
        );
    }
}

#[test]
fn wrapper_lock_matches_the_workspace() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workspace = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();
    let wrapper =
        std::fs::read_to_string(root.join("crates/stapel-grammar-rust/grammar.lock")).unwrap();
    for name in ["tree-sitter-rust", "tree-sitter"] {
        assert_eq!(
            locked_version(&wrapper, name),
            locked_version(&workspace, name),
            "{name}"
        );
    }
}

#[test]
fn standalone_manifest_matches_the_member() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stapel-grammar-rust");
    let read = |name: &str| -> toml::Table {
        std::fs::read_to_string(dir.join(name))
            .unwrap()
            .parse()
            .unwrap()
    };
    let member = read("Cargo.toml");
    let standalone = read("standalone.toml");

    let mut expected = member["dependencies"].as_table().unwrap().clone();
    let mut got = standalone["dependencies"].as_table().unwrap().clone();
    assert!(
        got.remove("tree-sitter").is_some(),
        "the standalone manifest names tree-sitter"
    );
    // The member may name tree-sitter too; the standalone one always does.
    expected.remove("tree-sitter");
    assert_eq!(got, expected, "the same dependencies besides tree-sitter");

    let types: Vec<&str> = standalone["lib"]["crate-type"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    assert_eq!(types, ["cdylib"]);

    let workspace = standalone["workspace"].as_table().expect("a [workspace]");
    assert!(workspace.is_empty(), "the [workspace] table is empty");
}
