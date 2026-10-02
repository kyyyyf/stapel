//! The PreToolUse guard behind `stapel hook pre-tool-use`.

use crate::config::Config;
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

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

/// The repository root at or above `start` that has `.stapel/stapel.toml`, if any.
pub fn stapel_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(".stapel/stapel.toml").is_file())
        .map(Path::to_path_buf)
}

/// Files under `.stapel/tickets/<key>/` that only `stapel` itself writes.
const MACHINE_FILES: [&str; 5] = [
    "state.json",
    "decisions.jsonl",
    "findings.jsonl",
    "runs.jsonl",
    "tokens.jsonl",
];

/// Decides on a tool call. `cwd` is where the agent runs; `project_dir` is the Claude Code
/// project (`CLAUDE_PROJECT_DIR`), which still counts when the agent has `cd`-ed elsewhere.
pub fn decide(input: &HookInput, cwd: &Path, project_dir: Option<&Path>) -> Decision {
    let cwd = input.cwd.as_deref().unwrap_or(cwd);
    match input.tool_name.as_str() {
        "Bash" => {
            let in_stapel_repo =
                stapel_root(cwd).is_some() || project_dir.and_then(stapel_root).is_some();
            let command = input.tool_input["command"].as_str().unwrap_or_default();
            if in_stapel_repo && is_git_push(command) {
                return Decision::Deny(
                    "git push агентам запрещён: пушит человек или оркестратор после ревью".into(),
                );
            }
            Decision::Allow
        }
        "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => {
            let field = if input.tool_name == "NotebookEdit" {
                "notebook_path"
            } else {
                "file_path"
            };
            let Some(path) = input.tool_input[field].as_str() else {
                return Decision::Deny(format!("вход хука без поля tool_input.{field}"));
            };
            decide_write(&resolve(&cwd.join(path)))
        }
        _ => Decision::Allow,
    }
}

/// `target` is absolute with symlinks resolved; the repository is the one that contains it.
fn decide_write(target: &Path) -> Decision {
    let Some(root) = stapel_root(target) else {
        return Decision::Allow;
    };
    let rel = target
        .strip_prefix(&root)
        .expect("root is an ancestor of target");

    if is_machine_file(rel) {
        return Decision::Deny(format!(
            "файл {} пишет только stapel, не инструмент записи агента",
            rel.display()
        ));
    }
    let writable = match std::fs::read_to_string(root.join(".stapel/stapel.toml"))
        .ok()
        .and_then(|text| Config::parse(&text).ok())
    {
        Some(config) => config.guard.always_writable,
        // A broken config must stay fixable, and must not open anything else.
        None => vec![".stapel/".to_string()],
    };
    if writable
        .iter()
        .any(|prefix| rel.starts_with(prefix.trim_end_matches('/')))
    {
        return Decision::Allow;
    }
    if build_allowed(&root) {
        return Decision::Allow;
    }
    Decision::Deny(format!(
        "запись в {} запрещена: сборка не разрешена ни у одного тикета \
         (нужно \"build\": {{\"allowed\": true}} в .stapel/tickets/<ключ>/state.json)",
        rel.display()
    ))
}

fn is_machine_file(rel: &Path) -> bool {
    if rel == Path::new(".stapel/stapel.toml") {
        return true;
    }
    let parts: Vec<_> = rel.components().collect();
    matches!(
        parts.as_slice(),
        [Component::Normal(a), Component::Normal(b), Component::Normal(_), Component::Normal(f)]
            if *a == ".stapel" && *b == "tickets" && MACHINE_FILES.iter().any(|m| f == m)
    )
}

/// True when some ticket's `state.json` has `build.allowed = true`.
fn build_allowed(root: &Path) -> bool {
    let Ok(tickets) = std::fs::read_dir(root.join(".stapel/tickets")) else {
        return false;
    };
    tickets.flatten().any(|ticket| {
        std::fs::read_to_string(ticket.path().join("state.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some_and(|state| state["build"]["allowed"] == true)
    })
}

/// Canonicalizes the longest existing ancestor of `path`, so symlinks on the way are followed,
/// and appends the not-yet-existing rest with `.` and `..` resolved lexically.
fn resolve(path: &Path) -> PathBuf {
    let existing = path
        .ancestors()
        .find(|a| !a.as_os_str().is_empty() && a.exists());
    let (mut out, rest) = match existing.and_then(|a| Some((a.canonicalize().ok()?, a))) {
        Some((canonical, ancestor)) => (canonical, path.strip_prefix(ancestor).unwrap_or(path)),
        None => (PathBuf::new(), path),
    };
    for component in rest.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// True when any simple command in the shell text pushes with git.
pub fn is_git_push(command: &str) -> bool {
    let (commands, nested) = lex(command);
    commands.iter().any(|words| runs_git_push(words)) || nested.iter().any(|s| is_git_push(s))
}

/// Reserved words that may stand before the program of a simple command.
const RESERVED: [&str; 12] = [
    "{", "}", "!", "if", "then", "elif", "else", "fi", "do", "done", "while", "until",
];
/// Programs that run another program given later on their command line.
const WRAPPERS: [&str; 15] = [
    "env", "command", "exec", "nohup", "time", "sudo", "doas", "nice", "ionice", "timeout",
    "stdbuf", "setsid", "chronic", "xargs", "unbuffer",
];
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
/// Git subcommands, and their `git-<name>` programs, that send objects to a remote.
const PUSH_SUBCOMMANDS: [&str; 3] = ["push", "send-pack", "http-push"];

fn runs_git_push(words: &[String]) -> bool {
    let mut rest = words;
    while let Some(w) = rest.first() {
        if RESERVED.contains(&w.as_str()) || is_assignment(w) {
            rest = &rest[1..];
        } else {
            break;
        }
    }
    let Some((program, args)) = rest.split_first() else {
        return false;
    };
    let name = program.rsplit('/').next().unwrap_or(program);

    if name == "eval" {
        return is_git_push(&args.join(" "));
    }
    if WRAPPERS.contains(&name) {
        // Wrapper options may take values (`sudo -u me`, `timeout 30`); rather than know each
        // wrapper's syntax, look for a push starting at any later word.
        return (0..args.len()).any(|i| runs_git_push(&args[i..]));
    }
    if SHELLS.contains(&name) {
        let script = args
            .iter()
            .position(|a| a.starts_with('-') && !a.starts_with("--") && a.contains('c'))
            .and_then(|i| args.get(i + 1));
        return script.is_some_and(|s| is_git_push(s));
    }
    if let Some(sub) = name.strip_prefix("git-") {
        return PUSH_SUBCOMMANDS.contains(&sub);
    }
    if name != "git" {
        return false;
    }
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        if GIT_VALUE_OPTIONS.contains(&arg.as_str()) {
            if arg == "-c" && args.get(i + 1).is_some_and(|v| is_push_alias(v)) {
                return true;
            }
            i += 2;
        } else if arg.starts_with('-') {
            i += 1;
        } else {
            return PUSH_SUBCOMMANDS.contains(&arg.as_str());
        }
    }
    false
}

/// `alias.<name>=<value>` whose value pushes, e.g. `alias.p=push` or `alias.p=!git push`.
fn is_push_alias(config: &str) -> bool {
    let Some((key, value)) = config.split_once('=') else {
        return false;
    };
    if !key.starts_with("alias.") {
        return false;
    }
    match value.strip_prefix('!') {
        Some(script) => is_git_push(script),
        None => value
            .split_whitespace()
            .next()
            .is_some_and(|sub| PUSH_SUBCOMMANDS.contains(&sub)),
    }
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
            && !name.starts_with(|c: char| c.is_ascii_digit())
    })
}

/// Splits shell text into simple commands of unquoted words, plus the scripts nested in
/// double quotes as `$(...)` or backticks. Not a full shell parser: it knows quotes,
/// backslashes, the separators `; & | ( )`, backticks and newline, and drops redirections,
/// which is enough to find each program and its arguments. Heredoc bodies are read as
/// commands, which can only deny more, never less.
fn lex(text: &str) -> (Vec<Vec<String>>, Vec<String>) {
    let mut commands = Vec::new();
    let mut nested = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut skip_next_word = false;
    let mut chars = text.chars().peekable();

    fn end_word(word: &mut String, in_word: &mut bool, words: &mut Vec<String>, skip: &mut bool) {
        if *in_word {
            let w = std::mem::take(word);
            if *skip {
                *skip = false;
            } else {
                words.push(w);
            }
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
                        '$' if chars.peek() == Some(&'(') => {
                            chars.next();
                            let mut depth = 1;
                            let mut script = String::new();
                            for s in chars.by_ref() {
                                match s {
                                    '(' => depth += 1,
                                    ')' => {
                                        depth -= 1;
                                        if depth == 0 {
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                                script.push(s);
                            }
                            nested.push(script);
                        }
                        '`' => {
                            let script: String = chars.by_ref().take_while(|&s| s != '`').collect();
                            nested.push(script);
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
            '>' | '<' => {
                // `2>&1`, `>log`, `<<EOF`: drop a leading fd number and the target word.
                if in_word && word.chars().all(|d| d.is_ascii_digit()) {
                    word.clear();
                    in_word = false;
                }
                end_word(&mut word, &mut in_word, &mut words, &mut skip_next_word);
                while chars.peek().is_some_and(|&n| n == '>' || n == '<') {
                    chars.next();
                }
                if chars.peek() == Some(&'&') {
                    chars.next();
                    while chars
                        .peek()
                        .is_some_and(|n| n.is_ascii_digit() || *n == '-')
                    {
                        chars.next();
                    }
                } else {
                    skip_next_word = true;
                }
            }
            ';' | '&' | '|' | '(' | ')' | '`' | '\n' => {
                end_word(&mut word, &mut in_word, &mut words, &mut skip_next_word);
                if !words.is_empty() {
                    commands.push(std::mem::take(&mut words));
                }
            }
            c if c.is_whitespace() => {
                end_word(&mut word, &mut in_word, &mut words, &mut skip_next_word)
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    end_word(&mut word, &mut in_word, &mut words, &mut skip_next_word);
    if !words.is_empty() {
        commands.push(words);
    }
    (commands, nested)
}
