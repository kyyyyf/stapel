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
