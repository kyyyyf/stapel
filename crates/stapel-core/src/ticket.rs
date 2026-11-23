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
    crate::state::read_bounded(path)
}

/// One line of a text as `parse` sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scanned {
    /// The line without its line break.
    pub line: String,
    /// The level (1 to 6) and title of an ATX heading outside fences and HTML comment blocks.
    pub heading: Option<(usize, String)>,
    /// The line opens, closes or lies inside a fenced code block or an HTML comment block.
    pub in_block: bool,
    /// The line lies inside a fenced code block, or inside an HTML comment block that began on an
    /// earlier line; the fence lines and the first line of a comment are not inside.
    pub inside: bool,
}

/// The lines of `text` with the heading and block rules of `parse`.
pub fn scan(text: &str) -> Vec<Scanned> {
    scan_state(text).0
}

/// Whether `text` ends inside a fence or an HTML comment block that no line closes.
pub fn ends_open(text: &str) -> bool {
    scan_state(text).1
}

fn scan_state(text: &str) -> (Vec<Scanned>, bool) {
    // Lone CR (old Mac editors) ends a line too.
    let text = &text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut in_comment = false;

    for line in text.split('\n') {
        let line_nl = line.strip_suffix('\r').unwrap_or(line);
        let mut in_block = true;
        let mut inside = false;
        let heading = if let Some((ch, len)) = fence {
            inside = !closes_fence(line_nl, ch, len);
            if !inside {
                fence = None;
            }
            None
        } else if in_comment {
            inside = true;
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
            in_block = false;
            atx_heading(line_nl)
        };
        out.push(Scanned {
            line: line.to_string(),
            heading,
            in_block,
            inside,
        });
    }
    (out, fence.is_some() || in_comment)
}

pub fn parse(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for scanned in scan(text) {
        match scanned.heading {
            Some((2, title)) => sections.push(Section {
                title,
                body: String::new(),
            }),
            _ => {
                if let Some(current) = sections.last_mut() {
                    current.body.push_str(&scanned.line);
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

fn atx_heading(line: &str) -> Option<(usize, String)> {
    let rest = indented_at_most_3(line);
    let level = rest.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &rest[level..];
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
    (!title.is_empty()).then(|| (level, title.to_string()))
}

impl<'a> Lookup<'a> {
    /// The body when the section was found exactly once.
    pub fn found(&self) -> Option<&'a str> {
        match self {
            Lookup::Found(body) => Some(body),
            _ => None,
        }
    }
}
