//! Sections of `ticket.md` (STP-2 AC-9).
//!
//! A section heading is an ATX level-2 heading (`## Title`, up to three leading spaces, an optional
//! closing run of `#`) outside fenced code blocks and HTML comment blocks. Setext headings and
//! indented code are not headings. A section runs to the next level-2 heading.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub title: String,
    /// The text between this heading and the next level-2 heading, without the heading line.
    pub body: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Lookup<'a> {
    Found(&'a str),
    Missing,
    Duplicated,
}

/// Reads `ticket.md`; a missing or non-UTF-8 file is an error that names the file.
pub fn read(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{} is not valid UTF-8", path.display()))
}

pub fn parse(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut in_comment = false;

    for line in text.split('\n') {
        let line_nl = line.strip_suffix('\r').unwrap_or(line);
        let heading = if let Some((ch, len)) = fence {
            if closes_fence(line_nl, ch, len) {
                fence = None;
            }
            None
        } else if in_comment {
            if line_nl.contains("-->") {
                in_comment = false;
            }
            None
        } else if let Some(open) = opens_fence(line_nl) {
            fence = Some(open);
            None
        } else if let Some(rest) = indented_at_most_3(line_nl).strip_prefix("<!--") {
            in_comment = !rest.contains("-->");
            None
        } else {
            heading_title(line_nl)
        };

        match heading {
            Some(title) => sections.push(Section {
                title,
                body: String::new(),
            }),
            None => {
                if let Some(current) = sections.last_mut() {
                    current.body.push_str(line);
                    current.body.push('\n');
                }
            }
        }
    }
    sections
}

/// The section whose heading equals `title`, compared without case.
pub fn find<'a>(sections: &'a [Section], title: &str) -> Lookup<'a> {
    let mut matches = sections
        .iter()
        .filter(|s| s.title.trim().eq_ignore_ascii_case(title.trim()));
    match (matches.next(), matches.next()) {
        (None, _) => Lookup::Missing,
        (Some(s), None) => Lookup::Found(&s.body),
        (Some(_), Some(_)) => Lookup::Duplicated,
    }
}

/// The line without up to three leading spaces, or `""` when it is indented four or more.
fn indented_at_most_3(line: &str) -> &str {
    let spaces = line.len() - line.trim_start_matches(' ').len();
    if spaces > 3 { "" } else { &line[spaces..] }
}

fn opens_fence(line: &str) -> Option<(char, usize)> {
    let rest = indented_at_most_3(line);
    let ch = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let len = rest.chars().take_while(|c| *c == ch).count();
    // A backtick fence's info string may not contain backticks.
    let info = &rest[len..];
    (len >= 3 && !(ch == '`' && info.contains('`'))).then_some((ch, len))
}

fn closes_fence(line: &str, ch: char, len: usize) -> bool {
    let rest = indented_at_most_3(line);
    let run = rest.chars().take_while(|c| *c == ch).count();
    run >= len && rest[run..].trim().is_empty()
}

fn heading_title(line: &str) -> Option<String> {
    let rest = indented_at_most_3(line).strip_prefix("##")?;
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let title = rest.trim_matches([' ', '\t']);
    let without_closing = title.trim_end_matches('#').trim_end_matches([' ', '\t']);
    let title = if without_closing.is_empty() {
        title
    } else {
        without_closing
    };
    (!title.is_empty()).then(|| title.to_string())
}
