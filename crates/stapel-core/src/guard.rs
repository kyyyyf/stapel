//! The PreToolUse guard behind `stapel hook pre-tool-use`.
//!
//! The push check reads only the literal command text. It is the first layer; the second is the
//! git `pre-push` hook that `stapel init` installs, which refuses any push from a shell where
//! Claude Code has set `CLAUDECODE`, however the push was started.

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
    /// Claude Code's permission mode, e.g. `default`, `acceptEdits`, `auto`, `plan`.
    #[serde(default)]
    pub permission_mode: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny(String),
    /// Show the person the permission dialog with `reason`; on "yes" run `command` instead.
    Ask {
        reason: String,
        command: String,
    },
}

impl HookInput {
    pub fn parse(text: &str) -> Result<HookInput, String> {
        serde_json::from_str(text).map_err(|e| format!("hook input could not be parsed: {e}"))
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

const PUSH_REASON: &str =
    "git push is not allowed for agents: the human or the orchestrator pushes after review";
const BYPASS_REASON: &str = "the command may switch off a second layer of protection (the git \
                             pre-push hook, or the Claude Code variables the hooks rely on); agents may \
                             not do that. To read a hook, use the Read tool";
const UNREADABLE_REASON: &str = "the command is too long, too deeply nested or too costly to \
                                 check for git push; the call is denied";

const INJECT_REASON: &str = "the command can type into the terminal where the permission dialog \
                             is answered; only a person answers it";
const GRANT_REASON: &str = "a --grant comes only from the permission dialog; stapel ok and \
                            stapel close may not carry one of their own or words filled in at \
                            run time";
const HOOK_SELF_REASON: &str = "stapel hook is run only by Claude Code; an agent may not run the \
                                hook entry point itself";

/// Longer commands are denied rather than parsed.
const MAX_COMMAND_LEN: usize = 64 * 1024;
/// Deeper nesting of `$( )`, backticks, `eval` and `sh -c` is denied rather than parsed.
const MAX_DEPTH: usize = 16;

/// Decides on a tool call. `cwd` is where the agent runs; `project_dir` is the Claude Code
/// project (`CLAUDE_PROJECT_DIR`), which still counts when the agent has `cd`-ed elsewhere.
pub fn decide(input: &HookInput, cwd: &Path, project_dir: Option<&Path>) -> Decision {
    let cwd = input.cwd.as_deref().unwrap_or(cwd);
    match input.tool_name.as_str() {
        "Bash" => {
            let in_stapel_repo =
                stapel_root(cwd).is_some() || project_dir.and_then(stapel_root).is_some();
            let command = match &input.tool_input["command"] {
                serde_json::Value::String(command) => command.as_str(),
                serde_json::Value::Null => "",
                _ => {
                    return Decision::Deny("hook input has a non-string tool_input.command".into());
                }
            };
            if !in_stapel_repo {
                return Decision::Allow;
            }
            if let Some(reason) = check_command(command) {
                return Decision::Deny(reason.into());
            }
            match plain_invocation(command) {
                Some(invocation) => {
                    crate::dialog::ask(&invocation, cwd, input.permission_mode.as_deref())
                }
                None => Decision::Allow,
            }
        }
        "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => {
            let field = if input.tool_name == "NotebookEdit" {
                "notebook_path"
            } else {
                "file_path"
            };
            let Some(path) = input.tool_input[field].as_str() else {
                return Decision::Deny(format!("hook input has no tool_input.{field}"));
            };
            let target = resolve(&cwd.join(path));
            let in_stapel_session =
                stapel_root(cwd).is_some() || project_dir.and_then(stapel_root).is_some();
            if in_stapel_session && is_global_config(&target) {
                return Decision::Deny(format!(
                    "only stapel or the human writes {}, not an agent's write tool \
                     (global git or Claude Code configuration)",
                    target.display()
                ));
            }
            decide_write(&target)
        }
        _ => Decision::Allow,
    }
}

/// The user's global git and Claude Code configuration: changing it can switch off both layers
/// for every repository at once.
fn is_global_config(target: &Path) -> bool {
    let env_path = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let mut files = Vec::new();
    if let Some(home) = env_path("HOME") {
        files.push(home.join(".gitconfig"));
        files.push(home.join(".config/git/config"));
        files.push(home.join(".claude/settings.json"));
        files.push(home.join(".claude/settings.local.json"));
    }
    if let Some(xdg) = env_path("XDG_CONFIG_HOME") {
        files.push(xdg.join("git/config"));
    }
    if let Some(global) = env_path("GIT_CONFIG_GLOBAL") {
        files.push(global);
    }
    files.iter().any(|f| resolve(f) == target)
}

/// `target` is absolute with symlinks resolved; the repository is the one that contains it.
fn decide_write(target: &Path) -> Decision {
    let Some(root) = stapel_root(target) else {
        return Decision::Allow;
    };
    let rel = target
        .strip_prefix(&root)
        .expect("root is an ancestor of target");

    if is_machine_file(rel) || is_git_or_claude_config(rel) || in_hooks_dir(&root, target) {
        return Decision::Deny(format!(
            "only stapel or the human writes {}, not an agent's write tool \
             (a stapel, git or Claude Code file)",
            rel.display()
        ));
    }
    let config = std::fs::read_to_string(root.join(".stapel/stapel.toml"))
        .ok()
        .and_then(|text| Config::parse(&text).ok());
    let writable = match &config {
        Some(config) => config.guard.always_writable.clone(),
        // A broken config must stay fixable, and must not open anything else.
        None => vec![".stapel/".to_string()],
    };
    if writable
        .iter()
        .any(|prefix| rel.starts_with(prefix.trim_end_matches('/')))
    {
        return Decision::Allow;
    }
    if crate::stage::build_permit(&root, config.as_ref()).is_some() {
        return Decision::Allow;
    }
    let report = match &config {
        Some(config) => crate::stage::permit_report(&root, config),
        None => ".stapel/stapel.toml is invalid, so no confirmation counts".into(),
    };
    Decision::Deny(format!(
        "writing {} is denied: the build is not allowed, because the required sections are \
         not confirmed or are stale — {report}; `stapel status` shows what is waiting",
        rel.display()
    ))
}

/// Machine files, compared without case so that case-insensitive file systems are covered.
fn is_machine_file(rel: &Path) -> bool {
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    match parts.as_slice() {
        [a, b] => {
            (a == ".stapel" && b == "stapel.toml") || (a == ".claude" && b == "settings.json")
        }
        [a, b, _, f] => a == ".stapel" && b == "tickets" && MACHINE_FILES.contains(&f.as_str()),
        _ => false,
    }
}

/// Anything under `.git/` or `.claude/`: git config and hooks, Claude Code settings and hooks.
fn is_git_or_claude_config(rel: &Path) -> bool {
    rel.components().next().is_some_and(|c| {
        let first = c.as_os_str().to_string_lossy().to_lowercase();
        first == ".git" || first == ".claude"
    })
}

/// True when `target` is inside the hooks directory git uses, `core.hooksPath` included.
fn in_hooks_dir(root: &Path, target: &Path) -> bool {
    let Ok(out) = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .current_dir(root)
        .output()
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    let hooks = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim_end());
    let hooks = resolve(&root.join(hooks));
    target.starts_with(&hooks)
}

/// Follows every symlink on the way, dangling ones included, and resolves `.` and `..` in
/// order, so the result is where a write to `path` would land.
fn resolve(path: &Path) -> PathBuf {
    resolve_hops(path, 0)
}

fn resolve_hops(path: &Path, hops: usize) -> PathBuf {
    const MAX_HOPS: usize = 40;
    let components: Vec<Component> = path.components().collect();
    let mut out = PathBuf::new();
    for (i, component) in components.iter().enumerate() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => {
                out.push(other);
                let is_link =
                    std::fs::symlink_metadata(&out).is_ok_and(|m| m.file_type().is_symlink());
                if is_link
                    && hops < MAX_HOPS
                    && let Ok(target) = std::fs::read_link(&out)
                {
                    out.pop();
                    let mut next = out.join(target);
                    for rest in &components[i + 1..] {
                        next.push(rest);
                    }
                    return resolve_hops(&next, hops + 1);
                }
            }
        }
    }
    out
}

/// The reason to deny a shell command, if any.
pub fn check_command(command: &str) -> Option<&'static str> {
    if command.len() > MAX_COMMAND_LEN {
        return Some(UNREADABLE_REASON);
    }
    let mut found = Found {
        budget: WORK_BUDGET,
        ..Found::default()
    };
    scan(command, command, 0, &mut found);
    if found.too_deep {
        Some(UNREADABLE_REASON)
    } else if found.bypass {
        Some(BYPASS_REASON)
    } else if found.hook {
        Some(HOOK_SELF_REASON)
    } else if found.inject {
        Some(INJECT_REASON)
    } else if found.grant {
        Some(GRANT_REASON)
    } else if found.push {
        Some(PUSH_REASON)
    } else {
        None
    }
}

/// True when the shell text pushes with git or cannot be checked.
pub fn is_git_push(command: &str) -> bool {
    check_command(command).is_some()
}

#[derive(Default)]
struct Found {
    push: bool,
    bypass: bool,
    too_deep: bool,
    /// Typing into the terminal, where the permission dialog is answered (AC-17).
    inject: bool,
    /// A `--grant` written by the agent rather than by the dialog (AC-15).
    grant: bool,
    /// The agent runs the hook entry point itself.
    hook: bool,
    /// Units of work left; nested scripts can multiply the work, so it is capped.
    budget: usize,
}

impl Found {
    /// Spends `cost` units; false (and a denial) once the budget is gone.
    fn spend(&mut self, cost: usize) -> bool {
        if self.budget < cost {
            self.budget = 0;
            self.too_deep = true;
            false
        } else {
            self.budget -= cost;
            true
        }
    }
}

/// Roughly a millisecond of work per million units; far above any real command.
const WORK_BUDGET: usize = 1_000_000;

/// Reserved words that may stand before the program of a simple command.
const RESERVED: [&str; 13] = [
    "{", "}", "!", "if", "then", "elif", "else", "fi", "do", "done", "while", "until", "coproc",
];
/// Programs that run another program given later on their command line, here or elsewhere
/// (a container, a remote host, a terminal multiplexer). Any later word may start that
/// program, and any argument with a space may be a whole command line, so wrapper options and
/// their values need no special knowledge.
const WRAPPERS: [&str; 34] = [
    "env",
    "command",
    "builtin",
    "exec",
    "nohup",
    "time",
    "sudo",
    "doas",
    "su",
    "nice",
    "ionice",
    "timeout",
    "stdbuf",
    "setsid",
    "chronic",
    "xargs",
    "unbuffer",
    "find",
    "watch",
    "parallel",
    "busybox",
    "ssh",
    "tmux",
    "screen",
    "script",
    "flock",
    "strace",
    "ltrace",
    "chroot",
    "unshare",
    "nsenter",
    "systemd-run",
    "docker",
    "podman",
];
const SHELLS: [&str; 10] = [
    "sh", "bash", "zsh", "dash", "ksh", "mksh", "ash", "fish", "csh", "tcsh",
];
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
/// Words that push without the pre-push hook, or skip it, in any program: an interpreter's
/// `-c` string is one word, so this catches them inside python, perl and the like too.
const HOOKLESS_PUSH_WORDS: [&str; 3] = ["no-verify", "send-pack", "http-push"];
/// Paths and settings of git hooks; changing them switches off the second layer.
const HOOK_WORDS: [&str; 3] = [".git/hooks", "hooks/pre-push", "core.hookspath"];
/// Git subcommands whose arguments are command lines that git runs.
const RUNS_ARGUMENTS: [&str; 5] = ["submodule", "rebase", "bisect", "filter-branch", "difftool"];

/// `full` is the whole top-level command, used when a shell reads its script from stdin.
fn scan(text: &str, full: &str, depth: usize, found: &mut Found) {
    if depth > MAX_DEPTH {
        found.too_deep = true;
        return;
    }
    if !found.spend(text.len() + 1) {
        return;
    }
    let lexed = lex(text);
    for target in &lexed.targets {
        check_words(std::slice::from_ref(target), found);
    }
    for words in &lexed.commands {
        check_simple(words, full, depth, found);
    }
    for script in &lexed.nested {
        scan(script, full, depth + 1, found);
    }
}

/// Word-level checks that do not depend on where the word stands.
fn check_words(words: &[String], found: &mut Found) {
    for word in words {
        let lower = word.to_ascii_lowercase();
        if HOOK_WORDS.iter().any(|w| lower.contains(w)) {
            found.bypass = true;
        }
        if HOOKLESS_PUSH_WORDS.iter().any(|w| lower.contains(w)) {
            found.push = true;
        }
        if word.contains("TIOCSTI")
            || word.contains("0x5412")
            || word.contains("21522")
            || word.starts_with("/dev/pts/")
            || word == "/dev/tty"
        {
            found.inject = true;
        }
    }
}

/// `stapel hook ...` run by an agent, and `stapel ok|close` with a `--grant` or a run-time word
/// (`$`), whether as `stapel` or `cargo run ... --`.
fn check_stapel(name: &str, args: &[String], found: &mut Found) {
    let rest = match name {
        "stapel" => args,
        "cargo" if args.first().is_some_and(|a| a == "run") => {
            match args.iter().position(|a| a == "--") {
                Some(sep) => &args[sep + 1..],
                None => return,
            }
        }
        _ => return,
    };
    let Some(at) = rest.iter().position(|w| !w.starts_with('-')) else {
        return;
    };
    match rest[at].to_ascii_lowercase().as_str() {
        "hook" => found.hook = true,
        "ok" | "close" => {
            if rest[at + 1..]
                .iter()
                .any(|w| w.starts_with("--grant") || w.contains('$'))
            {
                found.grant = true;
            }
        }
        _ => {}
    }
}

/// Programs and arguments that type into a terminal (AC-17).
fn injects_input(name: &str, args: &[String]) -> bool {
    match name {
        "xdotool" | "ydotool" | "wtype" => true,
        "tmux" => args.iter().any(|a| {
            a.starts_with("send")
                || [
                    "paste-buffer",
                    "pasteb",
                    "load-buffer",
                    "loadb",
                    "set-buffer",
                    "setb",
                ]
                .contains(&a.as_str())
        }),
        "screen" => args.iter().any(|a| a == "-X") && args.iter().any(|a| a == "stuff"),
        _ => false,
    }
}

fn check_simple(words: &[String], full: &str, depth: usize, found: &mut Found) {
    check_words(words, found);

    let mut start = 0;
    while let Some(w) = words.get(start) {
        if w == "function" {
            // `function name { ... }`: the body follows the name.
            start += 2;
        } else if RESERVED.contains(&w.as_str()) {
            start += 1;
        } else if is_assignment(w) {
            if is_alias_config(w) {
                found.push = true;
            }
            if moves_config(w) {
                found.bypass = true;
            }
            start += 1;
        } else {
            break;
        }
    }
    if start < words.len() {
        check_program(&words[start..], full, depth, false, found);
    }
}

/// `words[0]` is the program. Wrappers make every later word a candidate program.
fn check_program(words: &[String], full: &str, depth: usize, under_xargs: bool, found: &mut Found) {
    let Some((program, args)) = words.split_first() else {
        return;
    };
    if !found.spend(words.len()) {
        return;
    }
    let name = base_name(program);

    if clears_claudecode(name, args) {
        found.bypass = true;
    }
    if injects_input(name, args) {
        found.inject = true;
    }
    check_stapel(name, args, found);
    if WRAPPERS.contains(&name) {
        for arg in args.iter().filter(|a| a.contains(char::is_whitespace)) {
            scan(arg, full, depth + 1, found);
        }
        let under_xargs = name == "xargs";
        for i in 0..args.len() {
            // Later wrappers are covered by this loop, so they are not expanded again.
            if !WRAPPERS.contains(&base_name(&args[i])) {
                check_program(&args[i..], full, depth, under_xargs, found);
            } else {
                if clears_claudecode(base_name(&args[i]), &args[i + 1..]) {
                    found.bypass = true;
                }
                if injects_input(base_name(&args[i]), &args[i + 1..]) {
                    found.inject = true;
                }
            }
            if found.too_deep {
                return;
            }
        }
    } else if name == "eval" {
        scan(&args.join(" "), full, depth + 1, found);
    } else if SHELLS.contains(&name) {
        match shell_script(args) {
            Some(script) => scan(script, full, depth + 1, found),
            // A shell with no -c and no script file reads its script from stdin: a pipe or a
            // here-string, whose text is somewhere in the full command.
            None if !args.iter().any(|a| !a.starts_with('-')) => {
                if full.contains("push") {
                    found.push = true;
                }
            }
            None => {}
        }
    } else if let Some(sub) = name.strip_prefix("git-") {
        if PUSH_SUBCOMMANDS.contains(&sub) {
            found.push = true;
        }
    } else if name == "git" {
        check_git(args, full, depth, under_xargs, found);
    }
}

/// Commands that remove CLAUDECODE from what child processes see.
fn clears_claudecode(name: &str, args: &[String]) -> bool {
    // Any Claude Code variable: CLAUDECODE and CLAUDE_CODE_ENTRYPOINT mark the agent's shell for
    // `ok`, `close` and pre-push; CLAUDE_CODE_EXECPATH tells a forged hook call from a real one.
    let names_it = args.iter().any(|a| a.contains("CLAUDE"));
    match name {
        "unset" => names_it,
        "export" | "declare" | "typeset" | "local" => {
            names_it && args.iter().any(|a| a == "-n" || a.starts_with('+'))
                || args.iter().any(|a| moves_config(a))
        }
        "env" => {
            // Only env's own options and assignments, before the program it runs.
            let mut i = 0;
            while let Some(a) = args.get(i) {
                if a == "-u" || a == "--unset" {
                    if args.get(i + 1).is_some_and(|v| v.contains("CLAUDE")) {
                        return true;
                    }
                    i += 2;
                    continue;
                }
                let clears = a == "-"
                    || (a.len() > 3 && "--ignore-environment".starts_with(a.as_str()))
                    || (a.starts_with('-') && !a.starts_with("--") && a.contains('i'))
                    || a.contains("CLAUDE")
                    || moves_config(a);
                if clears {
                    return true;
                }
                if a.starts_with('-') || is_assignment(a) {
                    i += 1;
                } else {
                    return false;
                }
            }
            false
        }
        "exec" => args
            .iter()
            .take_while(|a| a.starts_with('-'))
            .any(|a| !a.starts_with("--") && a.contains('c')),
        _ => false,
    }
}

/// Assignments that take CLAUDECODE away or move where git reads its config and hooks.
fn moves_config(word: &str) -> bool {
    const NAMES: [&str; 9] = [
        "CLAUDECODE",
        "CLAUDE_CODE_ENTRYPOINT",
        "CLAUDE_CODE_EXECPATH",
        "HOME",
        "XDG_CONFIG_HOME",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
        "GIT_DIR",
        "GIT_COMMON_DIR",
    ];
    word.split_once('=')
        .is_some_and(|(name, _)| NAMES.contains(&name))
}

/// The script after `-c` (or a flag cluster with `c`, like `-lc`), skipping a `--`.
fn shell_script(args: &[String]) -> Option<&str> {
    let i = args
        .iter()
        .position(|a| a.starts_with('-') && !a.starts_with("--") && a.contains('c'))?;
    let mut rest = args[i + 1..].iter().map(String::as_str);
    match rest.next()? {
        "--" => rest.next(),
        script => Some(script),
    }
}

fn check_git(args: &[String], full: &str, depth: usize, under_xargs: bool, found: &mut Found) {
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        if let Some(spec) = arg.strip_prefix("--config-env=") {
            if spec.to_ascii_lowercase().starts_with("alias.") {
                found.push = true;
            }
            i += 1;
        } else if arg.len() > 2 && arg.starts_with("-c") {
            check_git_config(&arg[2..], full, depth, found);
            i += 1;
        } else if GIT_VALUE_OPTIONS.contains(&arg.as_str()) {
            let value = args.get(i + 1).map(String::as_str).unwrap_or_default();
            if arg == "--config-env" && value.to_ascii_lowercase().starts_with("alias.") {
                found.push = true;
            }
            if arg == "-c" {
                check_git_config(value, full, depth, found);
            }
            i += 2;
        } else if arg.starts_with('-') {
            i += 1;
        } else {
            check_subcommand(arg, &args[i + 1..], full, depth, found);
            return;
        }
    }
    // `xargs git` takes the subcommand from its input.
    if under_xargs {
        found.push = true;
    }
}

/// A `-c key=value`: a push alias, or a value git may run (pager, editor, ssh command).
fn check_git_config(config: &str, full: &str, depth: usize, found: &mut Found) {
    if config.to_ascii_lowercase().starts_with("alias.") && is_push_alias(config) {
        found.push = true;
    }
    if let Some((_, value)) = config.split_once('=') {
        scan(value.trim_start_matches('!'), full, depth + 1, found);
    }
}

fn check_subcommand(sub: &str, rest: &[String], full: &str, depth: usize, found: &mut Found) {
    // A `$` or `{}` in the subcommand is only filled in at run time.
    if PUSH_SUBCOMMANDS.contains(&sub) || sub.contains('$') || sub.contains("{}") {
        found.push = true;
        return;
    }
    if sub == "subtree" {
        if rest
            .iter()
            .find(|a| !a.starts_with('-'))
            .is_some_and(|a| a == "push")
        {
            found.push = true;
        }
    } else if RUNS_ARGUMENTS.contains(&sub) {
        // Each argument may be a command line, and any later word may start one
        // (`git submodule foreach git push`, `git bisect run git push`).
        for (i, arg) in rest.iter().enumerate() {
            scan(arg.trim_start_matches('-'), full, depth + 1, found);
            check_program(&rest[i..], full, depth + 1, false, found);
        }
    }
}

/// `alias.<name>=<value>` whose value pushes, e.g. `alias.p=push` or `alias.p=!git push`.
fn is_push_alias(config: &str) -> bool {
    let Some((_, value)) = config.split_once('=') else {
        return false;
    };
    match value.strip_prefix('!') {
        Some(script) => is_git_push(script),
        None => value
            .split_whitespace()
            .next()
            .is_some_and(|sub| PUSH_SUBCOMMANDS.contains(&sub)),
    }
}

/// `GIT_CONFIG_PARAMETERS`, `GIT_CONFIG_KEY_<n>` and similar that define an alias: what the
/// alias runs is not visible here.
fn is_alias_config(assignment: &str) -> bool {
    let (name, value) = assignment.split_once('=').unwrap_or_default();
    name.starts_with("GIT_CONFIG") && value.to_ascii_lowercase().contains("alias.")
}

fn base_name(program: &str) -> &str {
    program.rsplit('/').next().unwrap_or(program)
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
            && !name.starts_with(|c: char| c.is_ascii_digit())
    })
}

/// Collects a `( ... )` body after its opening parenthesis, counting nested parentheses.
fn take_balanced(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut depth = 1;
    let mut body = String::new();
    for c in chars.by_ref() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        body.push(c);
    }
    body
}

/// Splits shell text into simple commands of unquoted words, plus the nested scripts found in
/// `$( )`, backticks and process substitution `<( )`, and the targets of redirections. A command substitution leaves the word
/// `$` in its place, since its value is only known at run time. Not a full shell parser: it
/// knows quotes, backslashes and line continuation, the separators `; & | ( )` and newline,
/// and drops redirections, which is enough to find each program and its arguments. Heredoc
/// bodies are read as commands, which can only deny more, never less.
/// What `lex` found: simple commands, nested scripts, and redirection targets.
struct Lexed {
    commands: Vec<Vec<String>>,
    nested: Vec<String>,
    targets: Vec<String>,
}

fn lex(text: &str) -> Lexed {
    let mut commands = Vec::new();
    let mut nested = Vec::new();
    let mut targets = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut skip_next_word = false;
    let mut chars = text.chars().peekable();

    fn end_word(
        word: &mut String,
        in_word: &mut bool,
        words: &mut Vec<String>,
        skip: &mut bool,
        targets: &mut Vec<String>,
    ) {
        if *in_word {
            let w = std::mem::take(word);
            if *skip {
                *skip = false;
                targets.push(w);
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
                        '\\' => match chars.next() {
                            Some('\n') | None => {}
                            Some(e) => word.push(e),
                        },
                        '$' if chars.peek() == Some(&'(') => {
                            chars.next();
                            nested.push(take_balanced(&mut chars));
                            word.push('$');
                        }
                        '`' => {
                            nested.push(chars.by_ref().take_while(|&s| s != '`').collect());
                            word.push('$');
                        }
                        _ => word.push(q),
                    }
                }
            }
            '\\' => match chars.next() {
                // Line continuation: the pair disappears.
                Some('\n') | None => {}
                Some(e) => {
                    in_word = true;
                    word.push(e);
                }
            },
            '$' if chars.peek() == Some(&'(') => {
                chars.next();
                nested.push(take_balanced(&mut chars));
                in_word = true;
                word.push('$');
            }
            '`' => {
                nested.push(chars.by_ref().take_while(|&s| s != '`').collect());
                in_word = true;
                word.push('$');
            }
            '>' | '<' if chars.peek() == Some(&'(') => {
                // Process substitution: a nested script, no redirection target.
                chars.next();
                end_word(
                    &mut word,
                    &mut in_word,
                    &mut words,
                    &mut skip_next_word,
                    &mut targets,
                );
                nested.push(take_balanced(&mut chars));
            }
            '>' | '<' => {
                // `2>&1`, `>log`, `<<EOF`, `<<<word`: drop a leading fd number and the target.
                if in_word && word.chars().all(|d| d.is_ascii_digit()) {
                    word.clear();
                    in_word = false;
                }
                end_word(
                    &mut word,
                    &mut in_word,
                    &mut words,
                    &mut skip_next_word,
                    &mut targets,
                );
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
            ';' | '&' | '|' | '(' | ')' | '\n' => {
                end_word(
                    &mut word,
                    &mut in_word,
                    &mut words,
                    &mut skip_next_word,
                    &mut targets,
                );
                skip_next_word = false;
                if !words.is_empty() {
                    commands.push(std::mem::take(&mut words));
                }
            }
            c if c.is_whitespace() => end_word(
                &mut word,
                &mut in_word,
                &mut words,
                &mut skip_next_word,
                &mut targets,
            ),
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    end_word(
        &mut word,
        &mut in_word,
        &mut words,
        &mut skip_next_word,
        &mut targets,
    );
    if !words.is_empty() {
        commands.push(words);
    }
    Lexed {
        commands,
        nested,
        targets,
    }
}

/// A Bash command that is exactly one `stapel ok|close` call (STP-2 AC-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// `stapel` (as written) or `cargo run [options] --`.
    pub program: Vec<String>,
    /// `ok` or `close`.
    pub action: String,
    /// The words after the action, assignments and redirections dropped.
    pub args: Vec<String>,
}

/// The `stapel ok|close` call when `command` is exactly one simple command, else `None`.
pub fn plain_invocation(command: &str) -> Option<Invocation> {
    let lexed = lex(command);
    if lexed.commands.len() != 1 || !lexed.nested.is_empty() {
        return None;
    }
    let words: Vec<String> = lexed.commands[0]
        .iter()
        .skip_while(|w| is_assignment(w))
        .cloned()
        .collect();
    let (program, rest) = match words.first().map(|w| base_name(w)) {
        Some("stapel") => (words[..1].to_vec(), &words[1..]),
        Some("cargo") if words.get(1).is_some_and(|w| w == "run") => {
            let sep = words.iter().position(|w| w == "--")?;
            (words[..=sep].to_vec(), &words[sep + 1..])
        }
        _ => return None,
    };
    let at = rest.iter().position(|w| !w.starts_with('-'))?;
    let action = rest[at].to_ascii_lowercase();
    if action != "ok" && action != "close" {
        return None;
    }
    Some(Invocation {
        program,
        action,
        args: rest[at + 1..].to_vec(),
    })
}
