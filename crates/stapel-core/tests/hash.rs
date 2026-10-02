//! STP-2 AC-8: the normal form (version 1) of a section and its hash.

use proptest::prelude::*;
use stapel_core::hash::{NORMAL_FORM, normal_form, section_hash};

const BASE: &str = "Criterion one: caf\u{e9} works.\n\nSecond paragraph.\n";

#[test]
fn ignores_non_content_differences() {
    let variants = [
        BASE.replace('\n', "\r\n"),
        BASE.replace('\n', "\r"),
        format!("\u{feff}{BASE}"),
        BASE.replace('\u{e9}', "e\u{301}"),
        BASE.replace("works.", "works. \t "),
        format!("\n\n  \n{BASE}\n\n\t\n"),
    ];
    let expected = section_hash(BASE);
    for v in variants {
        assert_eq!(section_hash(&v), expected, "{v:?}");
    }
    assert_eq!(NORMAL_FORM, 1);
    assert!(expected.starts_with("sha256:"));
    assert_eq!(expected.len(), "sha256:".len() + 64);
    assert!(expected["sha256:".len()..].chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn detects_content_changes() {
    let changed = [
        BASE.replace("one:", "one: "),
        BASE.replace("Criterion one", "Criterion  one"),
        BASE.replace("works", "fails"),
        BASE.replace("Criterion", "criterion"),
        BASE.replace("\n\n", "\n"),
        BASE.replace("Second", "\tSecond"),
        format!("{BASE}More.\n"),
    ];
    let base = section_hash(BASE);
    for c in changed {
        assert_ne!(section_hash(&c), base, "{c:?}");
    }
}

proptest! {
    #[test]
    fn normal_form_is_idempotent(s in "\\PC*") {
        let once = normal_form(&s);
        prop_assert_eq!(normal_form(&once), once.clone());
    }

    #[test]
    fn any_inner_change_changes_hash(s in "[a-z ]{1,40}( [a-z]{1,10}\n){0,5}[a-z]{1,10}", at in 0usize..200) {
        let body = normal_form(&s);
        prop_assume!(!body.is_empty());
        let chars: Vec<char> = body.chars().collect();
        let i = at % (chars.len() + 1);
        let mut changed: String = chars[..i].iter().collect();
        changed.push('X');
        changed.extend(&chars[i..]);
        prop_assert_ne!(section_hash(&changed), section_hash(&body));
    }
}
