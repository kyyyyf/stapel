//! STP-28 step 5: the JUnit report reader behind the junit runner.

use stapel_core::junit::{read_report, Outcome, Suite};

fn real(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/data/junit/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn read(text: &str) -> Result<Vec<Suite>, String> {
    read_report(text.as_bytes())
}

fn cases(suite: &Suite) -> Vec<(&str, Outcome)> {
    suite.cases.iter().map(|c| (c.name.as_str(), c.outcome)).collect()
}

#[test]
fn reads_test_cases() {
    let suites = read_report(&real("a.xml")).expect("a.xml reads");
    assert_eq!(suites.len(), 1);
    assert_eq!(suites[0].name, "outer");
    assert_eq!(
        cases(&suites[0]),
        vec![
            ("outer inner & \"quoted\" <tag> passes", Outcome::Passed),
            ("outer inner & \"quoted\" <tag> fails", Outcome::Failed),
            ("outer is skipped", Outcome::Skipped),
            ("outer is todo", Outcome::Passed),
            (" top level", Outcome::Passed),
        ]
    );
    assert_eq!(suites[0].cases[0].classname, "outer inner & \"quoted\" <tag> passes");

    let suites = read_report(&real("all.xml")).expect("all.xml reads");
    assert_eq!(suites.len(), 2);
    assert_eq!(suites[0].name, "undefined");
    assert_eq!(
        cases(&suites[0]),
        vec![(" only one", Outcome::Passed), (" other", Outcome::Skipped)]
    );
    assert_eq!(suites[1].cases.len(), 5);
    assert_eq!(suites[1].cases[1].outcome, Outcome::Failed);

    // the other real reports
    assert_eq!(read_report(&real("none.xml")).unwrap().len(), 0);
    assert_eq!(read_report(&real("b.xml")).unwrap().len(), 0);
    let suites = read_report(&real("b2.xml")).unwrap();
    assert_eq!(suites.len(), 1);
    assert_eq!(
        cases(&suites[0]),
        vec![
            ("src/b.test.js", Outcome::Failed),
            ("src/b.test.js", Outcome::Failed)
        ]
    );
    assert_eq!(suites[0].cases[0].classname, "Test suite failed to run");
    let suites = read_report(&real("c.xml")).unwrap();
    assert_eq!(suites.len(), 1);
    assert_eq!(suites[0].cases.len(), 2);

    // character references, the five entities and CDATA
    let xml = "<testsuites><testsuite name=\"s\">\
        <testcase classname=\"c\" name=\"caf&#233; &#x41; &lt;&gt;&amp;&quot;&apos;\"/>\
        <testcase classname=\"c\" name=\"d\"><failure><![CDATA[<not> a & tag]]></failure></testcase>\
        </testsuite></testsuites>";
    let suites = read(xml).unwrap();
    assert_eq!(
        cases(&suites[0]),
        vec![("caf\u{e9} A <>&\"'", Outcome::Passed), ("d", Outcome::Failed)]
    );

    // `error` fails like `failure`; `skipped` with content; other elements are skipped with content
    let xml = "<?xml version=\"1.0\"?>\n<testsuites>\n<testsuite name=\"s\">\
        <properties><property name=\"p\" value=\"v\"/></properties>\
        <testcase classname=\"c\" name=\"e\"><error message=\"m\">boom</error></testcase>\
        <testcase classname=\"c\" name=\"s\"><skipped>why</skipped></testcase>\
        <testcase classname=\"c\" name=\"o\"><system-out><![CDATA[<testcase name=\"x\"><failure/>]]></system-out>\
        <system-err>a &lt;b</system-err></testcase>\
        <system-out>text <b>bold</b> </system-out>\
        </testsuite></testsuites>";
    let suites = read(xml).unwrap();
    assert_eq!(suites.len(), 1);
    assert_eq!(
        cases(&suites[0]),
        vec![("e", Outcome::Failed), ("s", Outcome::Skipped), ("o", Outcome::Passed)]
    );

    // comments are skipped
    let xml = "<testsuites><!-- c <testcase> --><testsuite name=\"s\"/></testsuites>";
    assert_eq!(read(xml).unwrap().len(), 1);
}

#[test]
fn refuses_malformed_reports() {
    let refused = |what: &str, bytes: &[u8]| {
        let err = read_report(bytes).expect_err(what);
        assert!(!err.is_empty(), "{what}: the reason is empty");
    };
    refused("empty file", b"");
    refused("blank file", b"  \n");
    refused("text", b"this is not a report");
    refused(
        "doctype",
        b"<?xml version=\"1.0\"?><!DOCTYPE testsuites [<!ENTITY x \"y\">]><testsuites/>",
    );
    refused(
        "unknown entity",
        b"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"&foo;\"/></testsuite></testsuites>",
    );
    refused(
        "unknown entity in text",
        b"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"a\"><failure>&foo;</failure></testcase></testsuite></testsuites>",
    );
    refused(
        "testcase not closed",
        b"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"a\"></testsuite></testsuites>",
    );
    refused(
        "stray close tag",
        b"<testsuites><testsuite name=\"s\"></testsuite></testsuite></testsuites>",
    );
    refused("not closed at the end", b"<testsuites><testsuite name=\"s\">");
    refused(
        "invalid utf-8",
        b"<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"\xff\xfe\"/></testsuite></testsuites>",
    );
    refused("a bad character reference", b"<testsuites><testsuite name=\"&#xZZ;\"/></testsuites>");

    // the reader refuses what a refusal reads as the real report plus a damage
    let mut cut = real("a.xml");
    cut.truncate(cut.len() - 20);
    refused("a report cut short", &cut);

    // over 16 MiB
    let mut big = real("none.xml");
    big.extend(std::iter::repeat(b' ').take(16 * 1024 * 1024 + 1));
    refused("over 16 MiB", &big);
}

#[test]
fn refuses_bad_character_references() {
    let name_of = |reference: &str| {
        read(&format!("<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"a{reference}b\"/></testsuite></testsuites>"))
    };
    for bad in [
        "&#0;", "&#x1;", "&#xFFFE;", "&#+65;", "&#-65;", "&#x+41;", "&#x-41;", "&#8;", "&#xB;",
        "&#xD800;", "&#xDFFF;", "&#xFFFF;", "&#x110000;", "&#;", "&#x;",
    ] {
        let err = name_of(bad).expect_err(bad);
        assert!(!err.is_empty(), "{bad}: the reason is empty");
    }
    // in text as well
    let xml = "<testsuites><testsuite name=\"s\"><testcase classname=\"c\" name=\"a\"><failure>&#x1;</failure></testcase></testsuite></testsuites>";
    assert!(read(xml).is_err());

    for (good, expect) in [
        ("&#9;", "a\tb"),
        ("&#x10FFFF;", "a\u{10FFFF}b"),
        ("&#xA;", "a\nb"),
        ("&#13;", "a\rb"),
        ("&#x20;", "a b"),
        ("&#xD7FF;", "a\u{D7FF}b"),
        ("&#xE000;", "a\u{E000}b"),
        ("&#xFFFD;", "a\u{FFFD}b"),
        ("&#x10000;", "a\u{10000}b"),
    ] {
        let suites = name_of(good).unwrap_or_else(|e| panic!("{good}: {e}"));
        assert_eq!(suites[0].cases[0].name, expect, "{good}");
    }
}

#[test]
fn refuses_misplaced_elements() {
    let refused = |what: &str, bytes: &[u8]| -> String { read_report(bytes).expect_err(what) };
    let place = "in a place the reader does not know";
    assert!(refused(
        "nested testsuite",
        b"<testsuites><testsuite name=\"a\"><testsuite name=\"b\"/></testsuite></testsuites>"
    )
    .contains(place));
    assert!(refused(
        "nested testsuite under a root testsuite",
        b"<testsuite name=\"a\"><testsuite name=\"b\"/></testsuite>"
    )
    .contains(place));
    assert!(refused("testcase under testsuites", b"<testsuites><testcase name=\"x\"/></testsuites>")
        .contains(place));
    assert!(refused(
        "failure under testsuite",
        b"<testsuites><testsuite><failure/></testsuite></testsuites>"
    )
    .contains(place));
    assert!(refused("second root", b"<testsuites/><testsuites/>").contains("more than one root"));
    assert!(refused("two suites as roots", b"<testsuite name=\"a\"/><testsuite name=\"b\"/>")
        .contains("more than one root"));
    assert!(refused("text after the root", b"<testsuites/>junk").contains("text outside"));
    assert!(refused("text before the root", b"junk<testsuites/>").contains("text outside"));
    assert!(refused("unknown root", b"<report><testsuite name=\"a\"/></report>")
        .contains("no <testsuites>"));
    assert!(refused("invalid utf-8", b"<testsuites name=\"\xff\"/>").contains("UTF-8"));
    assert!(refused("empty file", b"").contains("empty"));
    let mut big = b"<testsuites/>".to_vec();
    big.extend(std::iter::repeat(b' ').take(16 * 1024 * 1024 + 1));
    assert!(refused("over the limit", &big).contains("16 MiB"));
}

#[test]
fn refusals_name_their_cause() {
    let cause = |what: &str, bytes: &[u8], needle: &str| {
        let err = read_report(bytes).expect_err(what);
        assert!(err.contains(needle), "{what}: `{err}` lacks `{needle}`");
    };
    cause("blank", b"  \n", "empty");
    cause("text", b"this is not a report", "text outside");
    cause(
        "doctype",
        b"<?xml version=\"1.0\"?><!DOCTYPE testsuites [<!ENTITY x \"y\">]><testsuites/>",
        "DOCTYPE",
    );
    cause(
        "unknown entity",
        b"<testsuites><testsuite name=\"&foo;\"/></testsuites>",
        "`&foo;` is not known",
    );
    cause(
        "unknown entity in text",
        b"<testsuites><testsuite name=\"s\"><testcase name=\"a\"><failure>&foo;</failure></testcase></testsuite></testsuites>",
        "`&foo;` is not known",
    );
    cause(
        "mismatched close",
        b"<testsuites><testsuite name=\"s\"><testcase name=\"a\"></testsuite></testsuites>",
        "does not match <testcase>",
    );
    cause(
        "stray close",
        b"<testsuites><testsuite name=\"s\"></testsuite></testsuite></testsuites>",
        "does not match <testsuites>",
    );
    cause("close with no open", b"</testsuites>", "has no open element");
    cause("not closed", b"<testsuites><testsuite name=\"s\">", "<testsuite> is not closed");
    cause("bad reference", b"<testsuites><testsuite name=\"&#xZZ;\"/></testsuites>", "is not known");
    cause("control character", b"<testsuites><testsuite name=\"&#x1;\"/></testsuites>", "is not known");
    cause("signed reference", b"<testsuites><testsuite name=\"&#+65;\"/></testsuites>", "is not known");
    cause("unterminated comment", b"<testsuites><!-- never closed", "a comment is not closed");
    cause(
        "unterminated CDATA",
        b"<testsuites><testsuite name=\"s\"><system-out><![CDATA[x</system-out></testsuite></testsuites>",
        "a CDATA section is not closed",
    );
    cause(
        "unterminated processing instruction",
        b"<testsuites><?php never closed",
        "a processing instruction is not closed",
    );
    // UTF-16 with a byte order mark is not UTF-8
    let mut utf16 = vec![0xFF, 0xFE];
    for unit in "<testsuites/>".encode_utf16() {
        utf16.extend(unit.to_le_bytes());
    }
    cause("UTF-16", &utf16, "not valid UTF-8");
    cause("invalid UTF-8", b"<testsuites name=\"\xff\"/>", "not valid UTF-8");
    let mut big = b"<testsuites/>".to_vec();
    big.extend(std::iter::repeat(b' ').take(16 * 1024 * 1024 + 1));
    cause("over 16 MiB", &big, "over 16 MiB");
}

#[test]
fn reads_unusual_valid_reports() {
    // a BOM, single quotes, spaces around `=`, a foreign encoding declaration, a processing instruction
    let xml = "\u{feff}<?xml version='1.0' encoding='ISO-8859-1'?><?stylesheet href='x'?>\n\
        <testsuites >\n<testsuite  name = 'a b'  tests='1'>\
        <testcase classname = 'c' name = 'one' />\
        </testsuite></testsuites>\n";
    let suites = read(xml).unwrap();
    assert_eq!(suites.len(), 1);
    assert_eq!(suites[0].name, "a b");
    assert_eq!(cases(&suites[0]), vec![("one", Outcome::Passed)]);

    // a testcase inside an unknown element is skipped; mixed outcomes; a missing name
    let xml = "<testsuites><testsuite name=\"s\">\
        <extra><testcase name=\"hidden\"><failure/></testcase></extra>\
        <testcase classname=\"c\" name=\"p\"/>\
        <testcase classname=\"c\" name=\"f\"><failure message=\"m\"/></testcase>\
        <testcase classname=\"c\" name=\"e\"><error/></testcase>\
        <testcase classname=\"c\" name=\"k\"><skipped/></testcase>\
        <testcase classname=\"c\"/>\
        </testsuite><testsuite><testcase name=\"n\"/></testsuite></testsuites>";
    let suites = read(xml).unwrap();
    assert_eq!(suites.len(), 2);
    assert_eq!(
        cases(&suites[0]),
        vec![
            ("p", Outcome::Passed),
            ("f", Outcome::Failed),
            ("e", Outcome::Failed),
            ("k", Outcome::Skipped),
            ("", Outcome::Passed),
        ]
    );
    assert_eq!(suites[1].name, "");
    assert_eq!(cases(&suites[1]), vec![("n", Outcome::Passed)]);

    // a single testsuite as the root
    let suites = read("<testsuite name=\"root\"><testcase name=\"t\"/></testsuite>").unwrap();
    assert_eq!(suites.len(), 1);
    assert_eq!(cases(&suites[0]), vec![("t", Outcome::Passed)]);

    assert_eq!(read("<testsuites/>").unwrap().len(), 0);
}

// AC-38: each cause of a refused character reference has its own reason.
#[test]
fn character_reference_causes() {
    let reason = |reference: &str| {
        read(&format!("<testsuites><testsuite name=\"a{reference}b\"/></testsuites>"))
            .expect_err(reference)
    };
    let groups: [(&[&str], &str); 4] = [
        (&["&#0;", "&#xFFFE;"], "is not an XML 1.0 character"),
        (&["&#+65;", "&#x+41;"], "has a sign"),
        (&["&#;", "&#x;"], "has no digits"),
        (&["&#x110000;"], "is above 10FFFF"),
    ];
    for (references, cause) in groups {
        for reference in references {
            let err = reason(reference);
            assert!(err.contains(cause), "{reference}: `{err}` lacks `{cause}`");
            assert!(err.contains(reference), "{reference}: `{err}` does not name it");
            for (_, other) in groups.iter().filter(|(_, o)| *o != cause) {
                assert!(!err.contains(other), "{reference}: `{err}` holds `{other}`");
            }
        }
    }
}

#[test]
fn refuses_unknown_roots() {
    for (text, name) in [("<foo/>", "foo"), ("<foo/><testsuites/>", "foo"), ("<a/><b/>", "a")] {
        let err = read(text).expect_err(text);
        assert!(err.contains("unknown root"), "{text}: `{err}` lacks `unknown root`");
        assert!(err.contains(&format!("<{name}>")), "{text}: `{err}` does not name <{name}>");
    }
}

#[test]
fn refuses_second_roots() {
    for text in ["<testsuites/><foo/>", "<testsuite/><testsuite/>"] {
        let err = read(text).expect_err(text);
        assert!(
            err.contains("more than one root element"),
            "{text}: `{err}` lacks `more than one root element`"
        );
    }
}

#[test]
fn refuses_cdata_outside_root() {
    for text in [
        "<![CDATA[x]]><testsuites/>",
        "<testsuites/><![CDATA[x]]>",
        "<![CDATA[x]]><testsuites/><![CDATA[y]]>",
    ] {
        let err = read(text).expect_err(text);
        assert!(err.contains("text outside"), "{text}: `{err}` lacks `text outside`");
    }
}
