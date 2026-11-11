//! STP-28 step 4: the TypeScript lexer behind the junit step tests.

use stapel_core::ts_tests::test_calls;

fn names(src: &str) -> Vec<String> {
    test_calls(src).into_iter().map(|(n, _)| n).collect()
}

#[test]
fn finds_test_calls_with_describe_titles() {
    let src = "describe('a', () => { it('b', () => {}) })";
    assert_eq!(names(src), vec!["a b"]);

    let src = "describe('a', () => {\n  describe(\"b\", () => {\n    test('c', () => {});\n  });\n  test(`d`, () => {});\n});\n";
    assert_eq!(names(src), vec!["a b c", "a d"]);

    let src = "test.only('x', () => {});\nit.skip('y', () => {});\ntest('top', () => {});";
    assert_eq!(names(src), vec!["x", "y", "top"]);

    let src = "test('it\\'s', () => {});\ntest('a\\nb', () => {});\ntest('caf\\u{e9}', () => {});";
    assert_eq!(names(src), vec!["it's", "a\nb", "caf\u{e9}"]);

    let src = "test('', () => {});";
    assert_eq!(names(src), vec![""]);

    // the text of a test call ends at its `)`
    let src = "test('t', () => { f(1); }); const after_marker = 1;";
    let calls = test_calls(src);
    assert_eq!(calls.len(), 1);
    assert!(calls[0].1.contains("f(1)"), "{:?}", calls);
    assert!(!calls[0].1.contains("after_marker"), "{:?}", calls);
}

#[test]
fn skips_comments_strings_and_regexes() {
    let src = "re.test(s);\n\
               const a = \"test('x')\";\n\
               // test('x')\n\
               /* it('y') */\n\
               const r = /test\\('z'/;\n\
               const t = `${test('q')}`;\n\
               test('real', () => {});\n";
    assert_eq!(names(src), vec!["real"]);

    let src = "const v = a / b / c;\ntest('after division', () => {});";
    assert_eq!(names(src), vec!["after division"]);

    for before in ["f(", "x = ", "return "] {
        let src = format!("{before}/test\\('z'/);\ntest('kept', () => {{}});");
        assert_eq!(names(&src), vec!["kept"], "{src}");
    }
}

#[test]
fn titles_not_promised_are_skipped() {
    let src = "const x = 1;\n\
               test(`a ${x}`, () => {});\n\
               test('a ' + x, () => {});\n\
               test.todo('t');\n\
               test(name, () => {});\n\
               test.each([1])('e', () => {});\n\
               describe(`d ${y}`, () => { it('hidden', () => {}); });\n\
               test('seen', () => {});\n";
    assert_eq!(names(src), vec!["seen"]);
}
