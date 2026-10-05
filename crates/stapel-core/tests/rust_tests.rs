//! STP-4 step 3 drift review: the lexer behind "Step test" and its text.

use stapel_core::rust_tests::{helper_text, test_functions};

fn names(src: &str) -> Vec<String> {
    test_functions(src).into_iter().map(|(n, _)| n).collect()
}

fn text_of(src: &str, name: &str) -> String {
    test_functions(src)
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no test {name} in {src}"))
        .1
}

// D3-1: a `;` inside the signature does not end the function.
#[test]
fn signature_with_semicolon_keeps_the_body() {
    let src = "#[test]\nfn t() -> [u8; 4] {\n    let body_marker = [1u8; 4];\n    body_marker\n}\n";
    assert!(
        text_of(src, "t").contains("body_marker"),
        "{:?}",
        test_functions(src)
    );
}

// D3-2: an escaped quote char literal is one literal; what follows is read normally.
#[test]
fn escaped_char_literals_do_not_swallow_code() {
    let src = "#[test]\nfn a() {\n    let q = '\\'';\n    let b = '}';\n    let s = \"x\";\n}\n\n#[test]\nfn b() {}\n";
    assert_eq!(names(src), ["a", "b"]);
    let a = text_of(src, "a");
    assert!(
        a.contains("'\\''") && a.contains("'}'") && a.contains("\"x\""),
        "{a}"
    );
}
// D3-3: the rules of the Terms: literals byte for byte, comments (doc comments too) dropped,
// raw strings, nested comments, lifetimes, attribute runs, `proptest!`, nested and other tests.
#[test]
fn lexer_follows_the_terms() {
    let src = r####"
/// A doc comment.
fn helper<'a>(x: &'a str) -> &'a str { x }

#[test]
#[ignore]
fn raw() {
    let r = r#"} // not a comment "#;
    let b = b"}";
    /* outer /* inner } */ still comment } */
    let c = '"';
}

#[tokio::test]
async fn not_a_step_test() {}

mod nested {
    #[test]
    fn inner() {}
}

proptest! {
    #[test]
    fn property(x in 0..10u32) { prop_assert!(x < 10); }
}
"####;
    assert_eq!(names(src), ["raw", "property"]);
    let raw = text_of(src, "raw");
    assert!(raw.starts_with("# [ test ] # [ ignore ] fn raw"), "{raw}");
    assert!(raw.contains("r#\"} // not a comment \"#"), "{raw}");
    assert!(raw.contains("b\"}\""), "{raw}");
    assert!(!raw.contains("inner") && !raw.contains("still"), "{raw}");
    assert!(raw.contains("'\"'"), "{raw}");
    assert!(
        helper_text(src).contains("helper < 'a >"),
        "{}",
        helper_text(src)
    );

    let docs_changed = src.replace("/// A doc comment.", "/// Another doc comment.");
    assert_eq!(helper_text(src), helper_text(&docs_changed));
    let spaced = src.replace("let c = '\"';", "let   c =\n '\"' ;");
    assert_eq!(text_of(src, "raw"), text_of(&spaced, "raw"));
    let literal = src.replace("not a comment", "not  a comment");
    assert_ne!(text_of(src, "raw"), text_of(&literal, "raw"));
}
