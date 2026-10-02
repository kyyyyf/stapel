//! STP-2 AC-9: finding sections of ticket.md by their level-2 heading.

use proptest::prelude::*;
use stapel_core::ticket::{Lookup, find, parse, read};

const TICKET: &str = "# T-1 title\n\n## Description\n\nWhat.\n\n## Spec\n\nCriteria.\n### Sub\nMore.\n\n## Design\n\nChoice.\n";

fn body<'a>(sections: &'a [stapel_core::ticket::Section], title: &str) -> &'a str {
    match find(sections, title) {
        Lookup::Found(s) => s,
        other => panic!("{title}: {other:?}"),
    }
}

#[test]
fn finds_sections_by_title() {
    let s = parse(TICKET);
    assert_eq!(body(&s, "spec").trim(), "Criteria.\n### Sub\nMore.");
    assert_eq!(body(&s, "SPEC").trim(), "Criteria.\n### Sub\nMore.");
    assert_eq!(body(&s, "design").trim(), "Choice.");
    assert_eq!(body(&s, "Description").trim(), "What.");
}

#[test]
fn ignores_headings_in_code_blocks() {
    let text = "## Spec\n\n```\n## Design\n```\nafter\n\n## Design\n\nreal\n";
    let s = parse(text);
    assert!(body(&s, "spec").contains("## Design\n```\nafter"));
    assert_eq!(body(&s, "design").trim(), "real");
}

#[test]
fn fence_variants() {
    // A fence closes only with the same character, at least as long.
    let text = "## Spec\n\n~~~~\n```\n## Design\n~~~\n## Proof\n~~~~\n\n## Design\n\nreal\n";
    let s = parse(text);
    assert!(matches!(find(&s, "proof"), Lookup::Missing));
    assert_eq!(body(&s, "design").trim(), "real");
    let text = "## Spec\n\n````md\n```\n## Design\n```\n````\n\n## Design\n\nreal\n";
    let s = parse(text);
    assert_eq!(body(&s, "design").trim(), "real");
}

#[test]
fn setext_and_comments_are_not_headings() {
    let text = "## Spec\n\nSpec\n----\n\n    ## Design\n\n<!--\n## Design\n-->\n\n##Design\n\n### Design\n\n   ## Design ##\n\nreal\n";
    let s = parse(text);
    assert_eq!(body(&s, "design").trim(), "real");
    assert!(body(&s, "spec").contains("<!--\n## Design\n-->"));
}

#[test]
fn reports_missing_and_duplicate_sections() {
    let s = parse("## Spec\n\na\n\n## Spec\n\nb\n");
    assert!(matches!(find(&s, "spec"), Lookup::Duplicated));
    assert!(matches!(find(&s, "design"), Lookup::Missing));
}

#[test]
fn order_does_not_matter() {
    let s = parse("## Design\n\nd\n\n## Spec\n\ns\n");
    assert_eq!(body(&s, "spec").trim(), "s");
    assert_eq!(body(&s, "design").trim(), "d");
}

#[test]
fn non_utf8_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ticket.md");
    std::fs::write(&path, b"## Spec\n\n\xff\xfe\n").unwrap();
    let err = read(&path).unwrap_err();
    assert!(err.contains("UTF-8"), "{err}");
    std::fs::remove_file(&path).unwrap();
    let err = read(&path).unwrap_err();
    assert!(err.contains("ticket.md"), "{err}");
}

#[test]
fn large_ticket_parses_quickly() {
    let mut text = String::new();
    while text.len() < 1 << 20 {
        text.push_str("## Section\n```\n## not\n```\n<!--\n## not\n-->\nline of text here\n");
    }
    let started = std::time::Instant::now();
    let s = parse(&text);
    assert!(!s.is_empty());
    assert!(
        started.elapsed() < std::time::Duration::from_millis(200),
        "{:?}",
        started.elapsed()
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_input_never_panics(s in "(\\PC|\n|\r|#|`|~|<!--|-->| ){0,400}") {
        let sections = parse(&s);
        for section in &sections {
            let _ = find(&sections, &section.title);
        }
    }
}

// F-8: lone CR line endings (old Mac editors) still give headings.
#[test]
fn lone_cr_line_endings_parse() {
    let s = parse("## Spec\r\rText.\r\r## Design\r\rChoice.\r");
    assert_eq!(body(&s, "spec").trim(), "Text.");
    assert_eq!(body(&s, "design").trim(), "Choice.");
}
