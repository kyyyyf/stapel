//! The normal form of a section's text and its hash (STP-2 AC-8).
//!
//! The normal form ignores what editors change without changing content: a byte-order mark, line
//! endings, the Unicode composition form, trailing spaces and tabs, and blank lines at the start and
//! end. Everything else is content. The heading line is not part of the text given here.

use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

/// Version of the normal form; stored with each confirmation so that a change of the rules is
/// reported instead of looking like a change of content.
pub const NORMAL_FORM: u32 = 1;

pub fn normal_form(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let text: String = text.nfc().collect();
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.trim_end_matches([' ', '\t']))
        .collect();
    let start = lines.iter().position(|l| !l.is_empty());
    let end = lines.iter().rposition(|l| !l.is_empty());
    match (start, end) {
        (Some(s), Some(e)) => lines[s..=e].join("\n"),
        _ => String::new(),
    }
}

/// `sha256:<hex>` of the normal form.
pub fn section_hash(text: &str) -> String {
    let digest = Sha256::digest(normal_form(text).as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}
