//! `stapel grammar build` (STP-44): builds the Rust grammar library of the pinned version with cargo
//! under the machine-wide lock, into the grammar folder of the git common folder.

use crate::fail::Fail;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The lock file of the wrapper crate, built in; it is the `Cargo.lock` of a build.
const GRAMMAR_LOCK: &str = include_str!("../../stapel-grammar-rust/grammar.lock");
/// The standalone manifest of the wrapper crate, built in; it is the `Cargo.toml` of a build.
const STANDALONE_MANIFEST: &str = include_str!("../../stapel-grammar-rust/standalone.toml");
/// The source of the wrapper crate, built in.
const WRAPPER_SOURCE: &str = include_str!("../../stapel-grammar-rust/src/lib.rs");

/// The library extension of this machine.
const LIBRARY_EXTENSION: &str = if cfg!(target_os = "macos") {
    "dylib"
} else {
    "so"
};

/// The version of the crate `tree-sitter-rust` in the built-in `grammar.lock`.
pub fn pinned_version() -> String {
    let mut lines = GRAMMAR_LOCK.lines();
    while let Some(line) = lines.next() {
        if line == "name = \"tree-sitter-rust\"" {
            if let Some(version) = lines.next().and_then(|l| l.strip_prefix("version = \"")) {
                return version.trim_end_matches('"').to_string();
            }
        }
    }
    unreachable!("grammar.lock names tree-sitter-rust")
}

/// The git common folder of the current folder, made absolute.
pub fn common_dir() -> Result<PathBuf, Fail> {
    let cwd = std::env::current_dir().map_err(|e| Fail::failed(format!("no current folder: {e}")))?;
    let out = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(&cwd)
        .output()
        .map_err(|e| Fail::failed(format!("git did not start: {e}")))?;
    if !out.status.success() {
        return Err(Fail::failed("not in a git working tree"));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let path = PathBuf::from(text.trim_end_matches('\n'));
    Ok(if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    })
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A random part for a folder or a temporary file name.
pub fn random_part() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut buf = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        use std::io::Read;
        let _ = f.read_exact(&mut buf);
    }
    let digest = Sha256::digest([&nanos.to_le_bytes()[..], &buf[..]].concat());
    hex(&digest[..6])
}

/// The build folder, removed when the value drops: on every exit path.
struct BuildFolder(PathBuf);

impl Drop for BuildFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn io_fail(what: &str, path: &Path, e: std::io::Error) -> Fail {
    Fail::failed(format!("{what} {}: {e}", path.display()))
}

/// Writes `bytes` to `path` through a temporary file next to it and a rename.
pub fn write_replacing(path: &Path, bytes: &[u8]) -> Result<(), Fail> {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let temporary = path.with_file_name(format!(".{name}.tmp-{}-{}", std::process::id(), random_part()));
    let result = std::fs::write(&temporary, bytes)
        .map_err(|e| io_fail("cannot write", &temporary, e))
        .and_then(|()| {
            std::fs::rename(&temporary, path).map_err(|e| io_fail("cannot replace", path, e))
        });
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// `stapel grammar build`.
pub fn build() -> Result<(), Fail> {
    let common = common_dir()?;
    let grammars = common.join("stapel/grammars");
    let folder = grammars.join(env!("CARGO_PKG_VERSION"));
    std::fs::create_dir_all(&grammars).map_err(|e| io_fail("cannot make", &grammars, e))?;
    let pinned = pinned_version();

    let lock = crate::cargo::take("stapel grammar build").map_err(Fail::failed)?;
    let work = loop {
        let path = grammars.join(format!("build-{}-{}", std::process::id(), random_part()));
        match std::fs::create_dir(&path) {
            Ok(()) => break BuildFolder(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_fail("cannot make", &path, e)),
        }
    };
    let dir = &work.0;
    std::fs::create_dir(dir.join("src")).map_err(|e| io_fail("cannot make", dir, e))?;
    for (rel, text) in [
        ("Cargo.toml", STANDALONE_MANIFEST),
        ("Cargo.lock", GRAMMAR_LOCK),
        ("src/lib.rs", WRAPPER_SOURCE),
    ] {
        let path = dir.join(rel);
        std::fs::write(&path, text).map_err(|e| io_fail("cannot write", &path, e))?;
    }

    let target = dir.join("target");
    let mut cargo = Command::new("cargo");
    cargo
        .args(["build", "--release", "--locked", "--target-dir"])
        .arg(&target)
        .current_dir(dir);
    if lock.is_some() {
        cargo.env(crate::cargo::HELD_MARK, std::process::id().to_string());
    }
    let output = cargo.output();
    drop(lock);
    let failed = |output: Option<&std::process::Output>| {
        if let Some(o) = output {
            warn!("{}", String::from_utf8_lossy(&o.stdout).trim_end());
            warn!("{}", String::from_utf8_lossy(&o.stderr).trim_end());
        }
        Fail::failed("grammar build failed for rust")
    };
    let output = match output {
        Ok(o) => o,
        Err(e) => {
            warn!("cargo did not start: {e}");
            return Err(failed(None));
        }
    };
    let built = target
        .join("release")
        .join(format!("libstapel_grammar_rust.{LIBRARY_EXTENSION}"));
    let bytes = match std::fs::read(&built) {
        Ok(b) if output.status.success() => b,
        _ => return Err(failed(Some(&output))),
    };

    let digest = hex(&Sha256::digest(&bytes));
    let file_name = format!("rust-{}.{LIBRARY_EXTENSION}", &digest[..16]);
    std::fs::create_dir_all(&folder).map_err(|e| io_fail("cannot make", &folder, e))?;
    let library = folder.join(&file_name);
    let kept = std::fs::read(&library)
        .map(|old| hex(&Sha256::digest(&old)) == digest)
        .unwrap_or(false);
    if !kept {
        write_replacing(&library, &bytes)?;
    }
    let record = serde_json::json!({
        "version": pinned,
        "file": file_name,
        "sha256": digest,
    });
    let mut text = serde_json::to_string_pretty(&record).unwrap();
    text.push('\n');
    write_replacing(&folder.join("rust.json"), text.as_bytes())?;
    drop(work);
    say!("built: rust {pinned} {}", library.display());
    Ok(())
}
