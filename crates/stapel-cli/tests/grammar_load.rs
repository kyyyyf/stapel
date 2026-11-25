//! STP-44: the Rust grammar library parses a sample file, and the adapter table is data. The library
//! comes from the dev-dependency `stapel-grammar-rust`, which cargo builds next to the test binaries;
//! no test runs a nested cargo build.

use std::path::{Path, PathBuf};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/grammar")
        .join(name)
}

/// The library file that cargo leaves in the `deps` folder next to the test executable.
fn built_library() -> PathBuf {
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let path = deps.join(format!("libstapel_grammar_rust.{ext}"));
    assert!(path.is_file(), "no library at {}", path.display());
    path
}

#[test]
fn sample_symbols() {
    let source = std::fs::read(data("sample.rs")).unwrap();
    let expected = std::fs::read_to_string(data("sample.expected")).unwrap();
    let adapter = &stapel_index::adapters()[0];
    let library = stapel_index::Library::open(&built_library()).expect("the library loads");
    let parsed = stapel_index::extract(adapter, &library, &source).expect("the sample parses");
    assert!(!parsed.has_errors, "the sample has no syntax errors");
    let found: String = parsed
        .definitions
        .iter()
        .map(|d| format!("{} {} {}\n", d.line, d.kind, d.name))
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn all_kinds() {
    let source = std::fs::read(data("kinds.rs")).unwrap();
    let expected = std::fs::read_to_string(data("kinds.expected")).unwrap();
    let adapter = &stapel_index::adapters()[0];
    let library = stapel_index::Library::open(&built_library()).expect("the library loads");
    let parsed = stapel_index::extract(adapter, &library, &source).expect("the sample parses");
    assert!(!parsed.has_errors, "the sample has no syntax errors");
    let found: String = parsed
        .definitions
        .iter()
        .map(|d| format!("{} {} {}\n", d.line, d.kind, d.name))
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn adapter_table_is_data() {
    let table = stapel_index::adapters();
    assert_eq!(table.len(), 1);
    let adapter = &table[0];
    assert_eq!(adapter.language, "rust");
    assert_eq!(adapter.extensions, ["rs"]);
    assert_eq!(adapter.library, "rust");
    let file = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../stapel-index/queries/rust.scm"),
    )
    .unwrap();
    assert_eq!(adapter.query, file);
}

#[test]
fn binary_links_no_grammar() {
    fn walk(manifest: &Path, seen: &mut Vec<PathBuf>, names: &mut Vec<String>) {
        let manifest = manifest.canonicalize().unwrap();
        if seen.contains(&manifest) {
            return;
        }
        seen.push(manifest.clone());
        let text = std::fs::read_to_string(&manifest).unwrap();
        let doc: toml::Table = text.parse().unwrap();
        for section in ["dependencies", "build-dependencies"] {
            let Some(deps) = doc.get(section).and_then(|d| d.as_table()) else {
                continue;
            };
            for (name, spec) in deps {
                names.push(name.clone());
                if let Some(path) = spec.get("path").and_then(|p| p.as_str()) {
                    walk(
                        &manifest.parent().unwrap().join(path).join("Cargo.toml"),
                        seen,
                        names,
                    );
                }
            }
        }
    }
    let mut seen = Vec::new();
    let mut names = Vec::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
        &mut seen,
        &mut names,
    );
    assert!(names.iter().any(|n| n == "tree-sitter"), "{names:?}");
    // `tree-sitter-language` is the shared ABI type crate, not a grammar.
    let grammars: Vec<&String> = names
        .iter()
        .filter(|n| n.starts_with("tree-sitter-") && *n != "tree-sitter-language")
        .collect();
    assert!(grammars.is_empty(), "grammar crates linked: {grammars:?}");
}
