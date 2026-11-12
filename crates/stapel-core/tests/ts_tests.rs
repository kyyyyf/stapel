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

// STP-28 step 14 (AC-7): a describe with a modifier other than `.only` or `.skip` hides its calls.
#[test]
fn describe_modifiers_hide_calls() {
    for head in [
        "describe.each([1, 2])('t', () => {",
        "describe.todo('t', () => {",
        "describe.concurrent('t', () => {",
        "describe.skip.each([1])('t', () => {",
    ] {
        let src = format!("{head} it('hidden', () => {{}}); }});\ntest('seen', () => {{}});");
        assert_eq!(names(&src), vec!["seen"], "{head}");
    }
    for (head, want) in [
        ("describe.skip('t', () => {", "t hidden"),
        ("describe.only('t', () => {", "t hidden"),
    ] {
        let src = format!("{head} it('hidden', () => {{}}); }});");
        assert_eq!(names(&src), vec![want], "{head}");
    }
}

// STP-28 step 14 (AC-22): the `.only` forms.
#[test]
fn has_only_call_cases() {
    use stapel_core::ts_tests::has_only_call;
    for src in [
        "test.only('a', () => {});",
        "it.only ('a', () => {});",
        "describe.only('a', () => {});",
        "test.only.each([1])('a', () => {});",
        "test.only`a ${1}`('a', () => {});",
    ] {
        assert!(has_only_call(src), "{src}");
    }
    for src in [
        "describe.skip('a', () => {});",
        "// test.only('a', () => {});",
        "const s = \"test.only('a')\";",
        "const r = /test.only(/;",
        "foo.test.only('a', () => {});",
    ] {
        assert!(!has_only_call(src), "{src}");
    }
}

// STP-28 step 23 (AC-7): an `it` under `describe.each` with a tagged template table is no test call.
#[test]
fn describe_each_template_hides_calls() {
    let src = "describe.each`\n  a | b\n  ${1} | ${2}\n`('adds $a', ({ a, b }) => {\n  it('hidden', () => {});\n});\ntest('seen', () => {});";
    assert_eq!(names(src), vec!["seen"]);
}

// STP-28 step 23 (AC-7): hidden and titled `describe` scopes nest and follow each other.
#[test]
fn describe_scope_cases() {
    // A hidden describe inside a titled one.
    let src = "describe('a', () => {\n  describe.each([1])('h', () => { it('hidden', () => {}); });\n  it('kept', () => {});\n});";
    assert_eq!(names(src), vec!["a kept"]);

    // A titled describe inside a hidden one.
    let src = "describe.todo('h', () => {\n  describe('b', () => { it('hidden', () => {}); });\n});\ntest('top', () => {});";
    assert_eq!(names(src), vec!["top"]);

    // A titled describe after a hidden one.
    let src = "describe.each([1])('h', () => { it('hidden', () => {}); });\ndescribe('c', () => { it('d', () => {}); });";
    assert_eq!(names(src), vec!["c d"]);
}
