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
