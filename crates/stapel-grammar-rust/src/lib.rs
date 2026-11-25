//! The Rust grammar as a library of its own: `stapel grammar build` builds it, `stapel` loads it.

/// The tree-sitter language of Rust, as the pointer that `tree_sitter::Language` is made from.
#[unsafe(no_mangle)]
pub extern "C" fn stapel_grammar_language() -> *const () {
    // SAFETY: `LANGUAGE` is the function `tree-sitter-rust` exports for this purpose.
    unsafe { tree_sitter_rust::LANGUAGE.into_raw()() }
}
