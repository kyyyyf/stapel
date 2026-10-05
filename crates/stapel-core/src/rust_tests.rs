//! Test functions of a Rust test file and their text as tokens (STP-4, "Step test").
//!
//! A test is a top-level function with the attribute `#[test]`, also inside a top-level
//! `proptest! { … }`. Its text runs from its first attribute to its closing brace as tokens:
//! comments are dropped, whitespace between tokens becomes one space, and string, char and raw
//! literals are kept byte for byte. The file is never compiled; this is a lexer, so a `}` or `//`
//! inside a literal does not end anything.

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Punct(char),
    Word,
    Literal,
}

#[derive(Debug, Clone)]
struct Token {
    kind: Kind,
    text: String,
}

fn lex(src: &str) -> Vec<Token> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let word = |ch: char| ch.is_alphanumeric() || ch == '_';
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            let mut depth = 0usize;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if let Some(end) = raw_string_end(&c, i) {
            out.push(Token {
                kind: Kind::Literal,
                text: c[i..end].iter().collect(),
            });
            i = end;
        } else if ch == '"' || (ch == 'b' && c.get(i + 1) == Some(&'"')) {
            let start = i;
            i += if ch == 'b' { 2 } else { 1 };
            while i < c.len() && c[i] != '"' {
                i += if c[i] == '\\' { 2 } else { 1 };
            }
            i = (i + 1).min(c.len());
            out.push(Token {
                kind: Kind::Literal,
                text: c[start..i].iter().collect(),
            });
        } else if ch == '\'' {
            let start = i;
            if c.get(i + 1) == Some(&'\\') {
                // The quote, the backslash and the escaped character, then up to the closing quote.
                i += 3;
                while i < c.len() && c[i] != '\'' {
                    i += 1;
                }
                i = (i + 1).min(c.len());
                out.push(Token {
                    kind: Kind::Literal,
                    text: c[start..i].iter().collect(),
                });
            } else if c.get(i + 2) == Some(&'\'') {
                i += 3;
                out.push(Token {
                    kind: Kind::Literal,
                    text: c[start..i].iter().collect(),
                });
            } else {
                // A lifetime or a label.
                i += 1;
                while i < c.len() && word(c[i]) {
                    i += 1;
                }
                out.push(Token {
                    kind: Kind::Word,
                    text: c[start..i].iter().collect(),
                });
            }
        } else if word(ch) {
            let start = i;
            while i < c.len() && word(c[i]) {
                i += 1;
            }
            out.push(Token {
                kind: Kind::Word,
                text: c[start..i].iter().collect(),
            });
        } else {
            out.push(Token {
                kind: Kind::Punct(ch),
                text: ch.to_string(),
            });
            i += 1;
        }
    }
    out
}

/// The end of a raw string literal (`r"…"`, `r#"…"#`, `br"…"`) starting at `i`, if one does.
fn raw_string_end(c: &[char], i: usize) -> Option<usize> {
    let mut j = i;
    if c.get(j) == Some(&'b') {
        j += 1;
    }
    if c.get(j) != Some(&'r') {
        return None;
    }
    if i > 0 && (c[i - 1].is_alphanumeric() || c[i - 1] == '_') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while c.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if c.get(j) != Some(&'"') {
        return None;
    }
    j += 1;
    while j < c.len() {
        if c[j] == '"' && (1..=hashes).all(|k| c.get(j + k) == Some(&'#')) {
            return Some(j + 1 + hashes);
        }
        j += 1;
    }
    Some(c.len())
}

fn is(t: &Token, ch: char) -> bool {
    t.kind == Kind::Punct(ch)
}

/// The index after the `]` that closes the `[` at `open`.
fn close_bracket(tokens: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    for (k, t) in tokens.iter().enumerate().skip(open) {
        if is(t, '[') {
            depth += 1;
        } else if is(t, ']') {
            depth -= 1;
            if depth == 0 {
                return k + 1;
            }
        }
    }
    tokens.len()
}

/// The index after the `}` that closes the `{` at `open`.
fn close_brace(tokens: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    for (k, t) in tokens.iter().enumerate().skip(open) {
        if is(t, '{') {
            depth += 1;
        } else if is(t, '}') {
            depth -= 1;
            if depth == 0 {
                return k + 1;
            }
        }
    }
    tokens.len()
}

fn join(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Ranges of test functions among the items of `tokens[start..end]`.
fn scan(
    tokens: &[Token],
    start: usize,
    end: usize,
    top: bool,
    out: &mut Vec<(String, usize, usize)>,
) {
    let mut i = start;
    while i < end {
        let t = &tokens[i];
        if top
            && t.kind == Kind::Word
            && t.text == "proptest"
            && tokens.get(i + 1).is_some_and(|t| is(t, '!'))
            && tokens.get(i + 2).is_some_and(|t| is(t, '{'))
        {
            let close = close_brace(tokens, i + 2);
            scan(tokens, i + 3, close.saturating_sub(1), false, out);
            i = close;
            continue;
        }
        if is(t, '#') {
            // A run of attributes, then the item they belong to.
            let attrs_start = i;
            let mut test = false;
            while i < end && is(&tokens[i], '#') {
                let mut open = i + 1;
                if tokens.get(open).is_some_and(|t| is(t, '!')) {
                    open += 1;
                }
                if !tokens.get(open).is_some_and(|t| is(t, '[')) {
                    break;
                }
                let after = close_bracket(tokens, open);
                let inner = &tokens[open + 1..after.saturating_sub(1).max(open + 1)];
                if inner.len() == 1 && inner[0].kind == Kind::Word && inner[0].text == "test" {
                    test = true;
                }
                i = after;
            }
            if i == attrs_start {
                i += 1;
                continue;
            }
            let mut k = i;
            while k < end
                && tokens[k].kind == Kind::Word
                && ["pub", "async", "unsafe", "const", "extern"].contains(&tokens[k].text.as_str())
            {
                k += 1;
                if tokens.get(k).is_some_and(|t| is(t, '(')) {
                    while k < end && !is(&tokens[k], ')') {
                        k += 1;
                    }
                    k += 1;
                }
            }
            if test
                && k + 1 < end
                && tokens[k].kind == Kind::Word
                && tokens[k].text == "fn"
                && tokens[k + 1].kind == Kind::Word
            {
                let name = tokens[k + 1].text.clone();
                // The body is the first `{` outside parentheses and brackets; a `;` there means
                // no body.
                let mut b = k + 2;
                let mut depth = 0usize;
                while b < end {
                    let t = &tokens[b];
                    if is(t, '(') || is(t, '[') {
                        depth += 1;
                    } else if is(t, ')') || is(t, ']') {
                        depth = depth.saturating_sub(1);
                    } else if depth == 0 && (is(t, '{') || is(t, ';')) {
                        break;
                    }
                    b += 1;
                }
                let close = if b < end && is(&tokens[b], '{') {
                    close_brace(tokens, b)
                } else {
                    b + 1
                };
                out.push((name, attrs_start, close.min(tokens.len())));
                i = close;
            }
            continue;
        }
        if is(t, '{') {
            i = close_brace(tokens, i);
            continue;
        }
        i += 1;
    }
}

/// The test functions of a file, in file order, as (name, text).
pub fn test_functions(src: &str) -> Vec<(String, String)> {
    let tokens = lex(src);
    let mut ranges = Vec::new();
    scan(&tokens, 0, tokens.len(), true, &mut ranges);
    ranges
        .into_iter()
        .map(|(name, a, b)| (name, join(&tokens[a..b])))
        .collect()
}

/// The file's text as tokens without its test functions: the helpers and other items.
pub fn helper_text(src: &str) -> String {
    let tokens = lex(src);
    let mut ranges = Vec::new();
    scan(&tokens, 0, tokens.len(), true, &mut ranges);
    let mut keep = vec![true; tokens.len()];
    for (_, a, b) in ranges {
        keep[a..b].iter_mut().for_each(|k| *k = false);
    }
    let kept: Vec<Token> = tokens
        .into_iter()
        .zip(keep)
        .filter_map(|(t, k)| k.then_some(t))
        .collect();
    join(&kept)
}
