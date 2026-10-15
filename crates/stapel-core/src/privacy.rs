//! Private-data patterns that the checks of test files look for.

/// Top-level domains reserved for documentation and tests (RFC 2606, RFC 6761).
const RESERVED_TLDS: [&str; 4] = ["invalid", "example", "test", "localhost"];

/// An e-mail address: `@`, a host of letters, digits, `-` and `.` with at least two labels, a non-empty
/// first label and a last label of two or more ASCII letters. An address whose last label is a reserved
/// top-level domain (`invalid`, `example`, `test`, `localhost`) is not one.
pub fn has_email(text: &str) -> bool {
    text.match_indices('@').any(|(i, _)| {
        let after = &text[i + 1..];
        let host: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '.')
            .collect();
        let parts: Vec<&str> = host.split('.').collect();
        parts.len() >= 2
            && !parts[0].is_empty()
            && parts.last().is_some_and(|tld| {
                tld.len() >= 2
                    && tld.chars().all(|c| c.is_ascii_alphabetic())
                    && !RESERVED_TLDS.contains(&tld.to_ascii_lowercase().as_str())
            })
    })
}
