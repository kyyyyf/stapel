//! Test calls of a TypeScript or JavaScript test file (STP-28, "Test call").
//!
//! A test call is `test(…)` or `it(…)` (also `.only`, `.skip`; not `.todo`), not after a `.`, whose
//! whole first argument is one string or one template literal without `${`. Its name joins the titles
//! of the enclosing `describe` calls and its own by a space; a `describe` without such a title hides
//! its calls. Its text runs to its `)`. The file is never run; this is a lexer, so `test(` inside a
//! comment, a string, a template or a regular expression is no call.

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Punct(char),
    Word,
    /// A string or template literal; the decoded value, or `None` for a template with `${`.
    Str(Option<String>),
    Regex,
}

#[derive(Debug, Clone)]
struct Token {
    kind: Kind,
    text: String,
    start: usize,
    end: usize,
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

/// Reads an escape after the backslash at `c[i]`; returns the next index.
fn escape(c: &[char], i: usize, out: &mut String) -> usize {
    let mut i = i + 1;
    let Some(&e) = c.get(i) else { return i };
    i += 1;
    match e {
        'n' => out.push('\n'),
        't' => out.push('\t'),
        'r' => out.push('\r'),
        'b' => out.push('\u{8}'),
        'f' => out.push('\u{c}'),
        'v' => out.push('\u{b}'),
        '0' => out.push('\0'),
        '\n' => {}
        'x' => {
            let hex: String = c.iter().skip(i).take(2).collect();
            if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                out.push(ch);
                i += 2;
            }
        }
        'u' => {
            if c.get(i) == Some(&'{') {
                let mut j = i + 1;
                let mut hex = String::new();
                while j < c.len() && c[j] != '}' {
                    hex.push(c[j]);
                    j += 1;
                }
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
                i = j + 1;
            } else {
                let hex: String = c.iter().skip(i).take(4).collect();
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                    i += 4;
                }
            }
        }
        other => out.push(other),
    }
    i
}

/// Reads a quoted string starting at the quote `c[i]`; returns the value and the next index.
fn read_string(c: &[char], i: usize) -> (String, usize) {
    let quote = c[i];
    let mut i = i + 1;
    let mut out = String::new();
    while i < c.len() && c[i] != quote {
        if c[i] == '\\' {
            i = escape(c, i, &mut out);
        } else if c[i] == '\n' {
            break;
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    (out, (i + 1).min(c.len().max(i)))
}

/// Skips a `${ … }` expression starting at the `$` at `c[i]`; returns the next index.
fn skip_expr(c: &[char], i: usize) -> usize {
    let mut i = i + 2;
    let mut depth = 1usize;
    while i < c.len() && depth > 0 {
        match c[i] {
            '{' => {
                depth += 1;
                i += 1;
            }
            '}' => {
                depth -= 1;
                i += 1;
            }
            '\'' | '"' => i = read_string(c, i).1,
            '`' => i = read_template(c, i).1,
            '/' if c.get(i + 1) == Some(&'/') => {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            '/' if c.get(i + 1) == Some(&'*') => i = skip_block_comment(c, i),
            _ => i += 1,
        }
    }
    i
}

/// Reads a template starting at the backtick `c[i]`; the value is `None` when it holds `${`.
fn read_template(c: &[char], i: usize) -> (Option<String>, usize) {
    let mut i = i + 1;
    let mut out = String::new();
    let mut plain = true;
    while i < c.len() && c[i] != '`' {
        if c[i] == '\\' {
            i = escape(c, i, &mut out);
        } else if c[i] == '$' && c.get(i + 1) == Some(&'{') {
            plain = false;
            i = skip_expr(c, i);
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    ((plain).then_some(out), (i + 1).min(c.len().max(i)))
}

fn skip_block_comment(c: &[char], i: usize) -> usize {
    let mut i = i + 2;
    while i < c.len() && !(c[i] == '*' && c.get(i + 1) == Some(&'/')) {
        i += 1;
    }
    (i + 2).min(c.len())
}

/// A `/` starts a regular expression unless the previous token ends an operand.
fn regex_allowed(prev: Option<&Token>) -> bool {
    match prev {
        None => true,
        Some(t) => match &t.kind {
            Kind::Punct(ch) => !matches!(ch, ')' | ']'),
            Kind::Word => matches!(t.text.as_str(), "return" | "typeof" | "case" | "in" | "of"),
            Kind::Str(_) | Kind::Regex => false,
        },
    }
}

fn lex(c: &[char]) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let start = i;
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            i = skip_block_comment(c, i);
        } else if ch == '\'' || ch == '"' {
            let (value, next) = read_string(c, i);
            i = next;
            out.push(Token {
                kind: Kind::Str(Some(value)),
                text: String::new(),
                start,
                end: i,
            });
        } else if ch == '`' {
            let (value, next) = read_template(c, i);
            i = next;
            out.push(Token {
                kind: Kind::Str(value),
                text: String::new(),
                start,
                end: i,
            });
        } else if ch == '/' && regex_allowed(out.last()) {
            i += 1;
            let mut class = false;
            while i < c.len() && c[i] != '\n' {
                match c[i] {
                    '\\' => i += 1,
                    '[' => class = true,
                    ']' => class = false,
                    '/' if !class => break,
                    _ => {}
                }
                i += 1;
            }
            i = (i + 1).min(c.len());
            while i < c.len() && is_word(c[i]) {
                i += 1;
            }
            out.push(Token {
                kind: Kind::Regex,
                text: String::new(),
                start,
                end: i,
            });
        } else if is_word(ch) {
            while i < c.len() && is_word(c[i]) {
                i += 1;
            }
            out.push(Token {
                kind: Kind::Word,
                text: c[start..i].iter().collect(),
                start,
                end: i,
            });
        } else {
            i += 1;
            out.push(Token {
                kind: Kind::Punct(ch),
                text: String::new(),
                start,
                end: i,
            });
        }
    }
    out
}

fn is_punct(t: Option<&Token>, ch: char) -> bool {
    matches!(t, Some(t) if t.kind == Kind::Punct(ch))
}

/// For a call word at `i` (`name`, `name.only`, `name.skip`) returns the index of its `(`.
fn call_open(toks: &[Token], i: usize, name: &[&str], modifiers: &[&str]) -> Option<usize> {
    let t = &toks[i];
    if t.kind != Kind::Word || !name.contains(&t.text.as_str()) {
        return None;
    }
    if i > 0 && toks[i - 1].kind == Kind::Punct('.') {
        return None;
    }
    let mut j = i + 1;
    if is_punct(toks.get(j), '.') {
        match toks.get(j + 1) {
            Some(m) if m.kind == Kind::Word && modifiers.contains(&m.text.as_str()) => j += 2,
            _ => return None,
        }
    }
    is_punct(toks.get(j), '(').then_some(j)
}

/// The literal title when the whole first argument of the call opened at `open` is one literal.
fn literal_title(toks: &[Token], open: usize) -> Option<String> {
    match (toks.get(open + 1), toks.get(open + 2)) {
        (Some(Token { kind: Kind::Str(Some(s)), .. }), Some(next))
            if next.kind == Kind::Punct(',') || next.kind == Kind::Punct(')') =>
        {
            Some(s.clone())
        }
        _ => None,
    }
}

struct Call {
    name: String,
    start: usize,
    end: usize,
}

/// The index of the `)` that closes the `(` at `open`, or the last token.
fn match_close(toks: &[Token], open: usize) -> usize {
    let mut level = 0usize;
    for (k, u) in toks.iter().enumerate().skip(open) {
        match u.kind {
            Kind::Punct('(') => level += 1,
            Kind::Punct(')') => {
                level -= 1;
                if level == 0 {
                    return k;
                }
            }
            _ => {}
        }
    }
    toks.len().saturating_sub(1)
}

/// For a `describe` word at `i` with a modifier other than `.only` or `.skip` (`each`, `todo`,
/// `concurrent`, `skip.each`, …) returns the index where its whole call chain ends: every call of
/// `describe.each([…])('t', …)` and a tagged template form belong to it.
fn hidden_describe_end(toks: &[Token], i: usize) -> Option<usize> {
    let t = &toks[i];
    if t.kind != Kind::Word || t.text != "describe" {
        return None;
    }
    if i > 0 && toks[i - 1].kind == Kind::Punct('.') {
        return None;
    }
    let mut j = i + 1;
    let mut mods: Vec<&str> = Vec::new();
    while is_punct(toks.get(j), '.') {
        match toks.get(j + 1) {
            Some(m) if m.kind == Kind::Word => mods.push(m.text.as_str()),
            _ => return None,
        }
        j += 2;
    }
    if mods.is_empty() || mods == ["only"] || mods == ["skip"] {
        return None;
    }
    let mut end = None;
    loop {
        match toks.get(j) {
            Some(Token { kind: Kind::Str(_), .. }) => j += 1,
            Some(Token { kind: Kind::Punct('('), .. }) => {
                let close = match_close(toks, j);
                end = Some(close);
                j = close + 1;
            }
            _ => break,
        }
    }
    end
}

fn calls(c: &[char]) -> Vec<Call> {
    let toks = lex(c);
    let mut out = Vec::new();
    // (index of the token that ends the describe's call, its title; `None` hides the calls below it)
    let mut stack: Vec<(usize, Option<String>)> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        while stack.last().is_some_and(|(end, _)| *end < i) {
            stack.pop();
        }
        match &t.kind {
            Kind::Word => {
                if let Some(end) = hidden_describe_end(&toks, i) {
                    stack.push((end, None));
                } else if let Some(open) = call_open(&toks, i, &["describe"], &["only", "skip"]) {
                    stack.push((match_close(&toks, open), literal_title(&toks, open)));
                } else if let Some(open) = call_open(&toks, i, &["test", "it"], &["only", "skip"]) {
                    let Some(title) = literal_title(&toks, open) else {
                        continue;
                    };
                    let mut parts: Vec<&str> = Vec::new();
                    let mut hidden = false;
                    for (_, d) in &stack {
                        match d {
                            Some(d) => parts.push(d),
                            None => hidden = true,
                        }
                    }
                    if hidden {
                        continue;
                    }
                    parts.push(&title);
                    let mut level = 0usize;
                    let mut end = None;
                    for u in &toks[open..] {
                        match u.kind {
                            Kind::Punct('(') => level += 1,
                            Kind::Punct(')') => {
                                level -= 1;
                                if level == 0 {
                                    end = Some(u.end);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    out.push(Call {
                        name: parts.join(" "),
                        start: t.start,
                        end: end.unwrap_or(c.len()),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// The test calls of the file as `(name, text)`, in source order.
pub fn test_calls(text: &str) -> Vec<(String, String)> {
    let c: Vec<char> = text.chars().collect();
    calls(&c)
        .into_iter()
        .map(|k| (k.name, c[k.start..k.end].iter().collect()))
        .collect()
}

/// `base` with the test calls named in `names` replaced by their text in `newer` (STP-37 AC-10).
/// A named call missing in `newer` is left as it is in `base`.
pub fn with_calls_from(base: &str, newer: &str, names: &[String]) -> String {
    let c: Vec<char> = base.chars().collect();
    let n: Vec<char> = newer.chars().collect();
    let from = calls(&n);
    let mut out = String::new();
    let mut at = 0;
    for k in calls(&c) {
        if !names.contains(&k.name) || k.start < at {
            continue;
        }
        if let Some(f) = from.iter().find(|f| f.name == k.name) {
            out.extend(&c[at..k.start]);
            out.extend(&n[f.start..f.end]);
            at = k.end;
        }
    }
    out.extend(&c[at..]);
    out
}

/// Whether the file holds a `test`, `it` or `describe` call with `.only` (not after a `.`).
pub fn has_only_call(text: &str) -> bool {
    let c: Vec<char> = text.chars().collect();
    let toks = lex(&c);
    (0..toks.len()).any(|i| {
        let t = &toks[i];
        t.kind == Kind::Word
            && ["test", "it", "describe"].contains(&t.text.as_str())
            && !(i > 0 && toks[i - 1].kind == Kind::Punct('.'))
            && is_punct(toks.get(i + 1), '.')
            && toks.get(i + 2).is_some_and(|m| m.kind == Kind::Word && m.text == "only")
            && match toks.get(i + 3) {
                Some(n) => matches!(n.kind, Kind::Punct('(' | '.') | Kind::Str(_)),
                None => false,
            }
    })
}

/// The file without its test calls.
pub fn rest_text(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut at = 0;
    for k in calls(&c) {
        if k.start < at {
            continue;
        }
        out.extend(&c[at..k.start]);
        at = k.end;
    }
    out.extend(&c[at..]);
    out
}
