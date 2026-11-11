//! The JUnit XML report reader of the junit runner (STP-28, AC-8).
//!
//! It reads the subset jest-junit writes: `testsuites`, `testsuite`, `testcase`, `failure`, `error`
//! and `skipped`. Other elements are skipped with their content. It decodes the five XML entities,
//! character references and CDATA, and it refuses what it does not know: a DOCTYPE, an unknown entity,
//! unbalanced tags, invalid UTF-8, an empty file and a file over 16 MiB.

/// The largest report the reader takes.
pub const MAX_REPORT: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Debug)]
pub struct Case {
    pub classname: String,
    pub name: String,
    pub outcome: Outcome,
}

#[derive(Clone, Debug)]
pub struct Suite {
    pub name: String,
    pub cases: Vec<Case>,
}

/// Reads a report and returns its test suites in file order, or the reason it is refused.
pub fn read_report(bytes: &[u8]) -> Result<Vec<Suite>, String> {
    if bytes.len() > MAX_REPORT {
        return Err(format!("the report is over {} MiB", MAX_REPORT / 1024 / 1024));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "the report is not valid UTF-8".to_string())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Err("the report is empty".into());
    }
    Reader::default().run(text)
}

#[derive(Default)]
struct Reader {
    stack: Vec<String>,
    /// Open elements that are not ours; everything inside them is skipped.
    unknown: usize,
    roots: usize,
    suites: Vec<Suite>,
    suite: Option<Suite>,
    case: Option<Case>,
}

impl Reader {
    fn run(mut self, text: &str) -> Result<Vec<Suite>, String> {
        let mut rest = text;
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix('<') {
                rest = self.markup(after)?;
            } else {
                let end = rest.find('<').unwrap_or(rest.len());
                let (chunk, tail) = rest.split_at(end);
                decode(chunk)?;
                if self.stack.is_empty() && !chunk.trim().is_empty() {
                    return Err("the report has text outside its root element".into());
                }
                rest = tail;
            }
        }
        if let Some(open) = self.stack.last() {
            return Err(format!("the element <{open}> is not closed"));
        }
        if self.roots == 0 {
            return Err("the report has no <testsuites> element".into());
        }
        Ok(self.suites)
    }

    /// `after` follows a `<`; returns the text after the markup.
    fn markup<'a>(&mut self, after: &'a str) -> Result<&'a str, String> {
        if let Some(r) = after.strip_prefix("?") {
            return skip_to(r, "?>", "a processing instruction");
        }
        if let Some(r) = after.strip_prefix("!--") {
            return skip_to(r, "-->", "a comment");
        }
        if let Some(r) = after.strip_prefix("![CDATA[") {
            return skip_to(r, "]]>", "a CDATA section");
        }
        if after.starts_with('!') {
            return Err("the report holds a DOCTYPE or another declaration".into());
        }
        if let Some(r) = after.strip_prefix('/') {
            let end = r.find('>').ok_or("a closing tag has no `>`")?;
            let name = r[..end].trim();
            self.close(name)?;
            return Ok(&r[end + 1..]);
        }
        let (name, attrs, empty, tail) = open_tag(after)?;
        self.open(name, &attrs)?;
        if empty {
            self.close(name)?;
        }
        Ok(tail)
    }

    fn open(&mut self, name: &str, attrs: &[(String, String)]) -> Result<(), String> {
        let attr = |key: &str| {
            attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default()
        };
        let parent = self.stack.last().map(String::as_str);
        let known = if self.unknown > 0 {
            false
        } else {
            match (name, parent) {
                ("testsuites" | "testsuite", None) => {
                    self.roots += 1;
                    if self.roots > 1 {
                        return Err("the report has more than one root element".into());
                    }
                    true
                }
                ("testsuite", Some("testsuites")) => true,
                ("testcase", Some("testsuite")) => true,
                ("failure" | "error" | "skipped", Some("testcase")) => true,
                ("testsuites" | "testsuite" | "testcase" | "failure" | "error" | "skipped", _) => {
                    return Err(format!(
                        "the element <{name}> is in a place the reader does not know"
                    ));
                }
                _ => false,
            }
        };
        if known {
            match name {
                "testsuite" => {
                    self.suite = Some(Suite { name: attr("name"), cases: Vec::new() });
                }
                "testcase" => {
                    self.case = Some(Case {
                        classname: attr("classname"),
                        name: attr("name"),
                        outcome: Outcome::Passed,
                    });
                }
                "failure" | "error" => {
                    if let Some(c) = self.case.as_mut() {
                        c.outcome = Outcome::Failed;
                    }
                }
                "skipped" => {
                    if let Some(c) = self.case.as_mut() {
                        if c.outcome == Outcome::Passed {
                            c.outcome = Outcome::Skipped;
                        }
                    }
                }
                _ => {}
            }
        } else {
            self.unknown += 1;
        }
        self.stack.push(name.to_string());
        Ok(())
    }

    fn close(&mut self, name: &str) -> Result<(), String> {
        match self.stack.pop() {
            None => return Err(format!("the closing tag </{name}> has no open element")),
            Some(open) if open != name => {
                return Err(format!("the closing tag </{name}> does not match <{open}>"));
            }
            Some(_) => {}
        }
        if self.unknown > 0 {
            self.unknown -= 1;
            return Ok(());
        }
        match name {
            "testcase" => {
                if let (Some(c), Some(s)) = (self.case.take(), self.suite.as_mut()) {
                    s.cases.push(c);
                }
            }
            "testsuite" => {
                if let Some(s) = self.suite.take() {
                    self.suites.push(s);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn skip_to<'a>(text: &'a str, end: &str, what: &str) -> Result<&'a str, String> {
    text.find(end)
        .map(|i| &text[i + end.len()..])
        .ok_or_else(|| format!("{what} is not closed"))
}

/// Reads `name attr="v" ... (/)>` after a `<`; returns the name, the decoded attributes, whether the
/// tag is empty (`/>`), and the text after the tag.
#[allow(clippy::type_complexity)]
fn open_tag(after: &str) -> Result<(&str, Vec<(String, String)>, bool, &str), String> {
    let name_end = after
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .ok_or("a tag is not closed")?;
    let name = &after[..name_end];
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || "_-.:".contains(c)) {
        return Err(format!("`<{}` is not a tag", &after[..name_end.min(20)]));
    }
    let mut rest = &after[name_end..];
    let mut attrs = Vec::new();
    loop {
        rest = rest.trim_start();
        if let Some(r) = rest.strip_prefix("/>") {
            return Ok((name, attrs, true, r));
        }
        if let Some(r) = rest.strip_prefix('>') {
            return Ok((name, attrs, false, r));
        }
        let eq = rest.find('=').ok_or_else(|| format!("the tag <{name}> is malformed"))?;
        let key = rest[..eq].trim();
        if key.is_empty() || key.contains(|c: char| c.is_whitespace() || "<>/\"'".contains(c)) {
            return Err(format!("the tag <{name}> has a bad attribute"));
        }
        let value = rest[eq + 1..].trim_start();
        let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'');
        let quote = quote.ok_or_else(|| format!("an attribute of <{name}> has no quotes"))?;
        let body = &value[1..];
        let close = body.find(quote).ok_or_else(|| format!("an attribute of <{name}> is not closed"))?;
        if body[..close].contains('<') {
            return Err(format!("an attribute of <{name}> holds `<`"));
        }
        attrs.push((key.to_string(), decode(&body[..close])?));
        rest = &body[close + 1..];
    }
}

/// Decodes the five XML entities and character references; any other entity is refused.
fn decode(s: &str) -> Result<String, String> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        let end = tail.find(';').ok_or("an `&` has no `;`")?;
        let body = &tail[..end];
        match body {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = if let Some(hex) = body.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = body.strip_prefix('#') {
                    dec.parse::<u32>().ok()
                } else {
                    None
                };
                match code.and_then(char::from_u32) {
                    Some(c) => out.push(c),
                    None => return Err(format!("the entity `&{body};` is not known")),
                }
            }
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}
