//! Code index: language adapters, blast radius of a change.

use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

/// One language: the file extensions it owns, the library that parses it and its built-in query.
pub struct Adapter {
    pub language: &'static str,
    pub extensions: &'static [&'static str],
    pub library: &'static str,
    pub query: &'static str,
}

static ADAPTERS: [Adapter; 1] = [Adapter {
    language: "rust",
    extensions: &["rs"],
    library: "rust",
    query: include_str!("../queries/rust.scm"),
}];

/// The adapter table: a language is one entry here.
pub fn adapters() -> &'static [Adapter] {
    &ADAPTERS
}

/// A grammar library loaded from a file; it keeps the file mapped while the language is in use.
pub struct Library {
    language: Language,
    _handle: libloading::Library,
}

impl Library {
    /// Loads the library file and takes the language of its `stapel_grammar_language` function.
    pub fn open(path: &Path) -> Result<Library, String> {
        // SAFETY: loading a library runs its initialisers; the file is one that `stapel grammar build`
        // made.
        let handle = unsafe { libloading::Library::new(path) }
            .map_err(|e| format!("cannot load: {e}"))?;
        // SAFETY: the symbol has the signature of the function the grammar crate exports.
        let make: libloading::Symbol<unsafe extern "C" fn() -> *const ()> =
            unsafe { handle.get(b"stapel_grammar_language\0") }
                .map_err(|_| "no symbol stapel_grammar_language".to_string())?;
        let function: unsafe extern "C" fn() -> *const () = *make;
        // SAFETY: the function returns the pointer of a `TSLanguage` that lives as long as the library.
        let language = Language::from(unsafe { tree_sitter_language::LanguageFn::from_raw(function) });
        Ok(Library {
            language,
            _handle: handle,
        })
    }
}

/// A definition found in a file: the line (from 1) of its name, its kind and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub line: usize,
    pub kind: String,
    pub name: String,
}

/// What one parse found.
pub struct Extract {
    pub definitions: Vec<Definition>,
    pub has_errors: bool,
}

/// Parses `source` with the library and runs the adapter's query over the tree.
pub fn extract(adapter: &Adapter, library: &Library, source: &[u8]) -> Result<Extract, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&library.language)
        .map_err(|_| "tree-sitter refused its ABI version".to_string())?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "the parse gave no tree".to_string())?;
    let query = Query::new(&library.language, adapter.query).map_err(|e| e.to_string())?;
    let names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    let mut definitions: Vec<Definition> = Vec::new();
    let mut seen = std::collections::HashMap::<usize, usize>::new();
    while let Some(m) = matches.next() {
        let mut name = None;
        let mut kind = None;
        for capture in m.captures {
            let label = names[capture.index as usize];
            if label == "name" {
                name = Some(capture.node);
            } else {
                kind = Some(label);
            }
        }
        if let (Some(name), Some(kind)) = (name, kind) {
            let at = name.start_byte();
            // A `fn` in an `impl` or `trait` body matches both the general and the method pattern;
            // the method wins.
            if let Some(&i) = seen.get(&at) {
                if kind == "method" {
                    definitions[i].kind = "method".to_string();
                }
                continue;
            }
            seen.insert(at, definitions.len());
            definitions.push(Definition {
                line: name.start_position().row + 1,
                kind: kind.to_string(),
                name: String::from_utf8_lossy(&source[name.byte_range()]).into_owned(),
            });
        }
    }
    definitions.sort_by(|a, b| a.line.cmp(&b.line));
    Ok(Extract {
        definitions,
        has_errors: tree.root_node().has_error(),
    })
}
