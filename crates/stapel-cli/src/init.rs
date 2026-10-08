//! `stapel init`: the `.stapel/` layout in the repository root.

use serde_json::{Value, json};
use stapel_core::config::{Config, starter_toml, validate_prefix};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const CONFIG: &str = ".stapel/stapel.toml";
const ALLOWLIST: &str = "# Reviewer false positives that were decided not to fix.\n";
const IGNORE_LINE: &str = "/.stapel/index/";
const SETTINGS: &str = ".claude/settings.json";
const HOOK_COMMAND: &str = "stapel hook pre-tool-use";
/// The installed hook line. Claude Code treats any exit code but 2 as "go ahead", so a missing
/// binary would silently disable the guard; this line turns that case into a denial.
const HOOK_LINE: &str = "command -v stapel >/dev/null 2>&1 || { echo 'stapel: the stapel binary is not on PATH; the call is denied' >&2; exit 2; }; stapel hook pre-tool-use";
const HOOK_MATCHER: &str = "Bash|Write|Edit|MultiEdit|NotebookEdit";
/// Seconds Claude Code waits for the hook; the guard answers in milliseconds.
const HOOK_TIMEOUT: u64 = 10;
/// Marks the pre-push hook as ours.
const PRE_PUSH_MARKER: &str = "# stapel: pre-push guard";
/// Second layer against agent pushes: Claude Code sets CLAUDECODE in the shell it gives the
/// agent, so this refuses a push however it was started (scripts, interpreters, gh).
const PRE_PUSH: &str = r#"#!/bin/sh
# stapel: pre-push guard, installed by `stapel init`.
# Claude Code sets CLAUDECODE (and CLAUDE_CODE_ENTRYPOINT) in the shell it gives the agent;
# pushes from there are refused.
if [ -n "${CLAUDECODE:-}" ] || [ -n "${CLAUDE_CODE_ENTRYPOINT:-}" ]; then
  echo "stapel: push from an agent session is not allowed; the human pushes from their own terminal" >&2
  exit 1
fi
exit 0
"#;

/// Test-only switch: treat stdin as a terminal so the prefix prompt can be driven from a pipe.
const ASSUME_TTY: &str = "STAPEL_ASSUME_TTY";

pub fn run(prefix: Option<String>) -> Result<(), String> {
    let root = repo_root()?;

    // Read and check everything first, so a refusal leaves the repository untouched.
    let mut plan = Vec::new();
    let config_path = root.join(CONFIG);
    if config_path.exists() {
        let text = std::fs::read_to_string(&config_path).map_err(|e| format!("{CONFIG}: {e}"))?;
        let config =
            Config::parse_at(&text, &root).map_err(|e| format!("{CONFIG} cannot be read: {e}"))?;
        if prefix.is_some() {
            println!(
                "{CONFIG} already exists, the ticket key stays {}; edit the file to change it",
                config.tickets.key
            );
        }
    } else {
        let prefix = match prefix {
            Some(p) => p,
            None => ask_prefix()?,
        };
        validate_prefix(&prefix)?;
        let cargo = root.join("Cargo.toml").is_file();
        plan_create(&root, CONFIG, starter_toml(&prefix, cargo), &mut plan);
    }
    plan_create(&root, ".stapel/allowlist.toml", ALLOWLIST.into(), &mut plan);
    plan_create(&root, ".stapel/tickets/.gitkeep", String::new(), &mut plan);
    plan_ignore_index(&root, &mut plan)?;
    plan_hooks(&root, &mut plan)?;
    plan_pre_push(&root, &mut plan)?;

    for step in &plan {
        let path = root.join(&step.rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, &step.content).map_err(|e| format!("{}: {e}", step.rel))?;
        if step.executable {
            make_executable(&path).map_err(|e| format!("{}: {e}", step.rel))?;
        }
    }

    if !on_path("stapel") {
        eprintln!(
            "warning: stapel is not on PATH; until it is, the hook denies every tool call it \
             checks"
        );
    }
    if plan.is_empty() {
        println!("already set up: nothing changed");
    }
    for step in plan {
        println!("{}", step.label);
    }
    Ok(())
}

/// One file `init` is about to write.
struct Step {
    rel: String,
    content: String,
    label: String,
    executable: bool,
}

fn repo_root() -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err("not a git repository: run stapel init inside a repository".into());
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim_end(),
    ))
}

fn ask_prefix() -> Result<String, String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() && std::env::var_os(ASSUME_TTY).is_none() {
        return Err(
            "ticket key prefix not given: pass --prefix, e.g. stapel init --prefix STP".into(),
        );
    }
    print!("Ticket key prefix (2-8 uppercase Latin letters, e.g. STP): ");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok(line.trim().to_string())
}

/// Plans `rel` only when it does not exist yet, so a second run and hand edits leave it alone.
fn plan_create(root: &Path, rel: &'static str, content: String, plan: &mut Vec<Step>) {
    if !root.join(rel).exists() {
        plan.push(Step {
            rel: rel.into(),
            content,
            label: format!("created: {rel}"),
            executable: false,
        });
    }
}

fn plan_ignore_index(root: &Path, plan: &mut Vec<Step>) -> Result<(), String> {
    let mut text = match std::fs::read(root.join(".gitignore")) {
        Ok(bytes) => String::from_utf8(bytes)
            .map_err(|_| ".gitignore is not UTF-8; the file was not touched".to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!(".gitignore: {e}")),
    };
    if text.lines().any(|l| l.trim_end() == IGNORE_LINE) {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(IGNORE_LINE);
    text.push('\n');
    plan.push(Step {
        rel: ".gitignore".into(),
        content: text,
        label: "appended: .gitignore".into(),
        executable: false,
    });
    Ok(())
}

/// Plans the stapel PreToolUse hook in `.claude/settings.json`, keeping every other key and hook.
fn plan_hooks(root: &Path, plan: &mut Vec<Step>) -> Result<(), String> {
    let path = root.join(SETTINGS);
    let existed = path.exists();
    let mut settings: Value = if existed {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{SETTINGS}: {e}"))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("{SETTINGS} is not valid JSON; the file was not touched: {e}"))?
    } else {
        json!({})
    };

    let pre = settings
        .as_object_mut()
        .ok_or(format!("{SETTINGS}: expected a JSON object"))?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(format!("{SETTINGS}: the hooks field must be an object"))?
        .entry("PreToolUse")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or(format!(
            "{SETTINGS}: the hooks.PreToolUse field must be an array"
        ))?;

    // An entry with the bare command was written by an earlier, fail-open version: upgrade it.
    let mut upgraded = false;
    for hook in pre
        .iter_mut()
        .filter_map(|entry| entry["hooks"].as_array_mut())
        .flatten()
    {
        if hook["command"] == HOOK_COMMAND {
            hook["command"] = json!(HOOK_LINE);
            hook["timeout"] = json!(HOOK_TIMEOUT);
            upgraded = true;
        }
    }
    let installed = pre.iter().any(|entry| {
        entry["hooks"].as_array().is_some_and(|hooks| {
            hooks.iter().any(|h| {
                h["command"]
                    .as_str()
                    .is_some_and(|c| c.contains(HOOK_COMMAND))
            })
        })
    });
    if installed && !upgraded {
        return Ok(());
    }
    if !installed {
        pre.push(json!({
            "matcher": HOOK_MATCHER,
            "hooks": [{ "type": "command", "command": HOOK_LINE, "timeout": HOOK_TIMEOUT }]
        }));
    }

    let mut content = serde_json::to_string_pretty(&settings).expect("JSON value serializes");
    content.push('\n');
    plan.push(Step {
        rel: SETTINGS.into(),
        content,
        label: if upgraded {
            format!("updated: {SETTINGS} (stapel hook line)")
        } else if existed {
            format!("appended: {SETTINGS} (stapel hook)")
        } else {
            format!("created: {SETTINGS}")
        },
        executable: false,
    });
    Ok(())
}

/// Plans the git pre-push hook in the hooks directory git uses (`core.hooksPath` included).
/// A pre-push hook that is not ours is kept, with a warning.
fn plan_pre_push(root: &Path, plan: &mut Vec<Step>) -> Result<(), String> {
    let out = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks/pre-push"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err("git did not report the repository hooks path".into());
    }
    let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim_end());
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    let Ok(rel) = path.strip_prefix(root).map(|p| p.display().to_string()) else {
        // A shared hooks directory would put the hook into every repository that uses it.
        eprintln!(
            "warning: the git hooks directory {} is outside the repository (core.hooksPath); \
             pre-push was not installed, so the second layer against agent pushes is off",
            path.parent().unwrap_or(&path).display()
        );
        return Ok(());
    };

    match std::fs::read(&path) {
        Ok(bytes) if String::from_utf8_lossy(&bytes).contains(PRE_PUSH_MARKER) => {
            if !is_executable(&path) {
                plan.push(Step {
                    rel: path.display().to_string(),
                    content: String::from_utf8_lossy(&bytes).into_owned(),
                    label: format!("fixed: {rel} (pre-push is executable again)"),
                    executable: true,
                });
            }
            Ok(())
        }
        Ok(_) => {
            eprintln!(
                "warning: {rel} exists and is not from stapel; the second layer against agent \
                 pushes was not installed. Add a check of the CLAUDECODE variable to it by hand"
            );
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            plan.push(Step {
                rel: path.display().to_string(),
                content: PRE_PUSH.into(),
                label: format!("created: {rel} (pre-push: second layer against agent pushes)"),
                executable: true,
            });
            Ok(())
        }
        Err(e) => Err(format!("{rel}: {e}")),
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 == 0o111)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}
