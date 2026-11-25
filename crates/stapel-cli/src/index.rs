//! `stapel index` (STP-44): lists the files of the working tree, parses the ones an adapter owns with
//! the grammar library of the common folder and stores the definitions in `.stapel/index/`.

use crate::fail::Fail;
use crate::grammar::{common_dir, hex, pinned_version, write_replacing};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The first library line that a query prints: the reason the Rust files are skipped.
enum Library {
    Loaded(stapel_index::Library),
    Missing,
    Refused(PathBuf, String),
}

/// Loads the library the record names, with the checks in their order: record, library file, version,
/// hash, load, symbol, ABI.
fn load(adapter: &stapel_index::Adapter) -> Result<Library, Fail> {
    let folder = common_dir()?
        .join("stapel/grammars")
        .join(env!("CARGO_PKG_VERSION"));
    let record_path = folder.join(format!("{}.json", adapter.library));
    let Ok(text) = std::fs::read_to_string(&record_path) else {
        return Ok(Library::Missing);
    };
    let Ok(record) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(Library::Missing);
    };
    let field = |name: &str| record.get(name).and_then(|v| v.as_str()).map(str::to_string);
    let (Some(version), Some(file), Some(sha)) = (field("version"), field("file"), field("sha256"))
    else {
        return Ok(Library::Missing);
    };
    let path = folder.join(&file);
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(Library::Missing);
    };
    let refuse = |reason: String| Ok(Library::Refused(path.clone(), reason));
    let pinned = pinned_version();
    if version != pinned {
        return refuse(format!("grammar version {version} is not {pinned}"));
    }
    if hex(&Sha256::digest(&bytes)) != sha {
        return refuse("content hash differs from its record".to_string());
    }
    let library = match stapel_index::Library::open(&path) {
        Ok(l) => l,
        Err(reason) => return refuse(reason),
    };
    if let Err(reason) = stapel_index::extract(adapter, &library, b"") {
        if reason == "tree-sitter refused its ABI version" {
            return refuse(reason);
        }
    }
    Ok(Library::Loaded(library))
}

/// The top folder of the working tree.
fn toplevel() -> Result<PathBuf, Fail> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| Fail::failed(format!("git did not start: {e}")))?;
    if !out.status.success() {
        return Err(Fail::failed("not a git working tree"));
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim_end_matches('\n'),
    ))
}

/// Tracked files present in the tree and untracked files git does not ignore.
fn file_list(top: &Path) -> Result<Vec<String>, Fail> {
    let out = Command::new("git")
        .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard"])
        .current_dir(top)
        .output()
        .map_err(|e| Fail::failed(format!("git did not start: {e}")))?;
    if !out.status.success() {
        return Err(Fail::failed("git ls-files failed"));
    }
    let mut files: Vec<String> = out
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .filter_map(|p| String::from_utf8(p.to_vec()).ok())
        .filter(|p| !p.starts_with(".stapel/index/"))
        .filter(|p| top.join(p).symlink_metadata().is_ok())
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}

/// The result of one rebuild of the index.
struct Built {
    rust_line: Option<(usize, usize)>,
    skipped: usize,
    erroneous: usize,
    stored: serde_json::Map<String, serde_json::Value>,
    library: Option<Library>,
    /// Files parsed in this run, and files of the old index that left the list.
    parsed: usize,
    removed: usize,
}

impl Built {
    /// The library line as a refusal (exit 2), when a Rust file was skipped for the library.
    fn library_failure(&self) -> Result<(), Fail> {
        match &self.library {
            Some(Library::Missing) => Err(Fail::refused(
                "no grammar library for rust: run stapel grammar build",
            )),
            Some(Library::Refused(path, reason)) => Err(Fail::refused(format!(
                "grammar library {} refused: {reason}; run stapel grammar build",
                path.display()
            ))),
            _ => Ok(()),
        }
    }
}

/// The files of the stored index of `top`, when it is readable and made by this version.
fn previous(top: &Path) -> serde_json::Map<String, serde_json::Value> {
    let Ok(text) = std::fs::read_to_string(top.join(".stapel/index/index.json")) else {
        return Default::default();
    };
    let Ok(document) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Default::default();
    };
    if document["stapel"] != env!("CARGO_PKG_VERSION") {
        return Default::default();
    }
    document["files"].as_object().cloned().unwrap_or_default()
}

/// Brings the index of the tree at `top` up to date and stores it; with `incremental` a file whose
/// content hash is the stored one is not parsed again. Definitions of a language without a usable
/// library are not stored.
fn build(top: &Path, incremental: bool) -> Result<Built, Fail> {
    let files = file_list(top)?;
    let old = if incremental {
        previous(top)
    } else {
        Default::default()
    };
    let removed = old
        .keys()
        .filter(|k| files.binary_search(k).is_err())
        .count();
    let mut parsed_now = 0usize;
    let adapter = &stapel_index::adapters()[0];
    let owns = |path: &str| {
        Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| adapter.extensions.contains(&e))
    };
    let candidate = |path: &str| {
        let full = top.join(path);
        owns(path)
            && full
                .symlink_metadata()
                .map(|m| m.file_type().is_file() && m.len() <= 1_048_576)
                .unwrap_or(false)
            && std::fs::File::open(&full).is_ok()
    };
    let library = if files.iter().any(|f| candidate(f)) {
        Some(load(adapter)?)
    } else {
        None
    };
    let loaded = match &library {
        Some(Library::Loaded(l)) => Some(l),
        _ => None,
    };

    let mut parsed_files = 0usize;
    let mut symbols = 0usize;
    let mut skipped = 0usize;
    let mut erroneous = 0usize;
    let mut stored = serde_json::Map::new();
    for file in &files {
        let path = top.join(file);
        let is_file = path
            .symlink_metadata()
            .map(|m| m.file_type().is_file() && m.len() <= 1_048_576)
            .unwrap_or(false);
        let Some(library) = loaded.filter(|_| is_file && owns(file)) else {
            skipped += 1;
            continue;
        };
        let Ok(source) = std::fs::read(&path) else {
            skipped += 1;
            continue;
        };
        let sha = hex(&Sha256::digest(&source));
        if let Some(entry) = old.get(file).filter(|e| e["sha256"] == sha.as_str()) {
            parsed_files += 1;
            symbols += entry["definitions"].as_array().map_or(0, |d| d.len());
            if entry["errors"] == true {
                erroneous += 1;
            }
            stored.insert(file.clone(), entry.clone());
            continue;
        }
        let Ok(found) = stapel_index::extract(adapter, library, &source) else {
            skipped += 1;
            continue;
        };
        parsed_now += 1;
        parsed_files += 1;
        symbols += found.definitions.len();
        if found.has_errors {
            erroneous += 1;
        }
        let definitions: Vec<serde_json::Value> = found
            .definitions
            .iter()
            .map(|d| serde_json::json!({"line": d.line, "kind": d.kind, "name": d.name}))
            .collect();
        stored.insert(
            file.clone(),
            serde_json::json!({
                "sha256": sha,
                "errors": found.has_errors,
                "definitions": definitions,
            }),
        );
    }

    let folder = top.join(".stapel/index");
    std::fs::create_dir_all(&folder)
        .map_err(|e| Fail::failed(format!("cannot make {}: {e}", folder.display())))?;
    let document = serde_json::json!({
        "stapel": env!("CARGO_PKG_VERSION"),
        "files": stored,
    });
    write_replacing(
        &folder.join("index.json"),
        serde_json::to_string(&document).unwrap().as_bytes(),
    )?;

    let rust_line = loaded.map(|_| (parsed_files, symbols));
    Ok(Built {
        rust_line,
        skipped,
        erroneous,
        stored,
        library,
        parsed: parsed_now,
        removed,
    })
}

/// `stapel index`: rebuilds the whole index and prints the counts.
pub fn run() -> Result<(), Fail> {
    let top = toplevel()?;
    let built = build(&top, false)?;
    if let Some((files, symbols)) = built.rust_line {
        say!("rust: {files} files, {symbols} symbols");
    }
    say!("skipped: {} files", built.skipped);
    say!("syntax errors: {} files", built.erroneous);
    built.library_failure()
}

/// `stapel q symbol NAME`: one line per definition named exactly NAME.
pub fn symbol(name: &str, json: bool) -> Result<(), Fail> {
    let top = toplevel()?;
    let built = build(&top, true)?;
    if built.parsed > 0 || built.removed > 0 {
        warn!("index: {} parsed, {} removed", built.parsed, built.removed);
    }
    let mut found = 0usize;
    let mut objects: Vec<serde_json::Value> = Vec::new();
    // The map keeps the paths in byte order.
    for (path, entry) in &built.stored {
        let Some(definitions) = entry["definitions"].as_array() else {
            continue;
        };
        let mut lines: Vec<(u64, &str)> = definitions
            .iter()
            .filter(|d| d["name"] == name)
            .map(|d| (d["line"].as_u64().unwrap_or(0), d["kind"].as_str().unwrap_or("")))
            .collect();
        lines.sort();
        for (line, kind) in lines {
            if json {
                objects.push(serde_json::json!({
                    "path": path, "line": line, "kind": kind, "name": name
                }));
            } else {
                say!("{path}:{line} {kind} {name}");
            }
            found += 1;
        }
    }
    if json {
        say!("{}", serde_json::to_string(&objects).unwrap());
    }
    built.library_failure()?;
    if found == 0 {
        return Err(Fail::failed(format!("no definition of {name}")));
    }
    Ok(())
}
