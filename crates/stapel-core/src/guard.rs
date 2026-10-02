//! The PreToolUse guard behind `stapel hook pre-tool-use`.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The part of a Claude Code PreToolUse payload the guard looks at.
#[derive(Debug, Deserialize)]
pub struct HookInput {
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: serde_json::Value,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny(String),
}

impl HookInput {
    pub fn parse(text: &str) -> Result<HookInput, String> {
        serde_json::from_str(text).map_err(|e| format!("вход хука не разобран: {e}"))
    }
}

/// The repository root above `start` that has `.stapel/stapel.toml`, if any.
pub fn stapel_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(".stapel/stapel.toml").is_file())
        .map(Path::to_path_buf)
}

/// Decides on a tool call; `root` is the stapel repository it happens in, if any.
pub fn decide(input: &HookInput, root: Option<&Path>) -> Decision {
    if root.is_none() {
        return Decision::Allow;
    }
    if input.tool_name == "Bash" {
        let command = input.tool_input["command"].as_str().unwrap_or_default();
        if is_git_push(command) {
            return Decision::Deny(
                "git push агентам запрещён: пушит человек или оркестратор после ревью".into(),
            );
        }
    }
    Decision::Allow
}

/// True when any simple command in the shell text runs `git push`.
pub fn is_git_push(command: &str) -> bool {
    simple_commands(command).iter().any(|words| runs_git_push(words))
}

/// Wrappers that run the rest of the line as a command.
const WRAPPERS: [&str; 6] = ["env", "command", "exec", "nohup", "time", "sudo"];
const SHELLS: [&str; 5] = ["sh", "bash", "zsh", "dash", "ksh"];
/// Git global options that take the next word as their value.
const GIT_VALUE_OPTIONS: [&str; 7] = [
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--super-prefix",
    "--config-env",
];

fn runs_git_push(words: &[String]) -> bool {
    let mut rest = words;
    loop {
        match rest.first() {
            Some(w) if is_assignment(w) => rest = &rest[1..],
            Some(w) if WRAPPERS.contains(&w.as_str()) => {
                rest = &rest[1..];
                while rest.first().is_some_and(|w| w.starts_with('-')) {
                    rest = &rest[1..];
                }
            }
            _ => break,
        }
    }
    let Some((program, args)) = rest.split_first() else {
        return false;
    };
    let name = program.rsplit('/').next().unwrap_or(program);

    if SHELLS.contains(&name) {
        let script = args
            .iter()
            .position(|a| a.starts_with('-') && !a.starts_with("--") && a.contains('c'))
            .and_then(|i| args.get(i + 1));
        return script.is_some_and(|s| is_git_push(s));
    }
    if name != "git" {
        return false;
    }
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        if GIT_VALUE_OPTIONS.contains(&arg.as_str()) {
            i += 2;
        } else if arg.starts_with('-') {
            i += 1;
        } else {
            return arg == "push";
        }
    }
    false
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
            && !name.starts_with(|c: char| c.is_ascii_digit())
    })
}

/// Splits shell text into simple commands of unquoted words. Not a full shell parser: it knows
/// quotes, backslashes, and the separators `; & | ( )` and newline, which is enough to find the
/// program and its arguments.
fn simple_commands(text: &str) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = text.chars();

    fn end_word(word: &mut String, in_word: &mut bool, words: &mut Vec<String>) {
        if *in_word {
            words.push(std::mem::take(word));
            *in_word = false;
        }
    }

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    word.push(q);
                }
            }
            '"' => {
                in_word = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        '\\' => {
                            if let Some(e) = chars.next() {
                                word.push(e);
                            }
                        }
                        _ => word.push(q),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(e) = chars.next() {
                    word.push(e);
                }
            }
            ';' | '&' | '|' | '(' | ')' | '\n' => {
                end_word(&mut word, &mut in_word, &mut words);
                if !words.is_empty() {
                    commands.push(std::mem::take(&mut words));
                }
            }
            c if c.is_whitespace() => end_word(&mut word, &mut in_word, &mut words),
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    end_word(&mut word, &mut in_word, &mut words);
    if !words.is_empty() {
        commands.push(words);
    }
    commands
}
