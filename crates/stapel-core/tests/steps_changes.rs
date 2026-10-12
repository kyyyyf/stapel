//! STP-13 AC-5 (R-3): the changes of a commit fail closed on a record that is malformed or short.

use stapel_core::steps::parse_changes;

#[test]
fn malformed_or_short_records_fail_closed() {
    let ok =
        parse_changes(b":100644 100644 a b M\0x.txt\0:100644 100644 a b R100\0old\0new\0").unwrap();
    assert_eq!(ok.len(), 2);
    assert_eq!(ok[1].old.as_deref(), Some("old"));
    assert_eq!(ok[1].new.as_deref(), Some("new"));
    for raw in [
        &b"no colon\0x.txt\0"[..],
        b":100644 100644 a b M\0",
        b":100644 100644 a b R100\0old\0",
        b":100644 100644\0x.txt\0",
    ] {
        assert!(parse_changes(raw).is_err(), "{raw:?}");
    }
}
