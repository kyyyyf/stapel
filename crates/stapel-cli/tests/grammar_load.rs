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
