//! `stapel init`: the `.stapel/` layout in the repository root.

use serde_json::{json, Value};
use stapel_core::config::{starter_toml, validate_prefix, Config};
use std::io::{BufRead, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::Command;

const CONFIG: &str = ".stapel/stapel.toml";
const ALLOWLIST: &str = "# Reviewer false positives that were decided not to fix.\n";
const IGNORE_LINE: &str = "/.stapel/index/";
const SETTINGS: &str = ".claude/settings.json";
/// The narrow allow rules of the dialog-free commands, in the order `init` appends them.
const ALLOW_RULES: [&str; 4] = [
    "Bash(stapel findings import:*)",
    "Bash(stapel findings triage:*)",
    "Bash(stapel ledger add --kind finding:*)",
    "Bash(stapel ledger add --kind catches:*)",
];
const HOOK_COMMAND: &str = "stapel hook pre-tool-use";
/// The installed hook line. Claude Code treats any exit code but 2 as "go ahead", so a missing
/// binary would silently disable the guard; this line turns that case into a denial.
const HOOK_LINE: &str = "command -v stapel >/dev/null 2>&1 || { echo 'stapel: the stapel binary is not on PATH; the call is denied' >&2; exit 2; }; stapel hook pre-tool-use";
const HOOK_MATCHER: &str = "Bash|Write|Edit|MultiEdit|NotebookEdit";
/// Seconds Claude Code waits for the hook; the guard answers in milliseconds.
const HOOK_TIMEOUT: u64 = 10;
const POST_HOOK_COMMAND: &str = "stapel hook post-tool-use";
/// The installed PostToolUse and PostToolUseFailure line: advice, not a guard, so a missing,
/// older or failing binary lets the call stand and reaches no agent.
const POST_HOOK_LINE: &str =
    "command -v stapel >/dev/null 2>&1 || exit 0; stapel hook post-tool-use 2>/dev/null || exit 0";
/// The hook events of the commit hook.
const POST_HOOK_EVENTS: [&str; 2] = ["PostToolUse", "PostToolUseFailure"];
const POST_HOOK_MATCHER: &str = "Bash";
const SESSION_HOOK_COMMAND: &str = "stapel hook session-start";
/// The installed SessionStart line: context, not a guard, so a missing, older or failing binary
/// prints nothing.
const SESSION_HOOK_LINE: &str =
    "command -v stapel >/dev/null 2>&1 || exit 0; stapel hook session-start 2>/dev/null || exit 0";
const SESSION_HOOK_MATCHER: &str = "startup|clear|compact";
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
    let new_folders = [".claude/agents", ".claude/skills"]
        .iter()
        .any(|d| std::fs::symlink_metadata(root.join(d)).is_err());
    let mut plan = Vec::new();
    let config_path = root.join(CONFIG);
    let config = if config_path.exists() {
        let text = std::fs::read_to_string(&config_path).map_err(|e| format!("{CONFIG}: {e}"))?;
        let config =
            Config::parse_at(&text, &root).map_err(|e| format!("{CONFIG} cannot be read: {e}"))?;
        if prefix.is_some() {
            say!(
                "{CONFIG} already exists, the ticket key stays {}; edit the file to change it",
                config.tickets.key
            );
        }
        config
    } else {
        let prefix = match prefix {
            Some(p) => p,
            None => ask_prefix()?,
        };
        validate_prefix(&prefix)?;
        let cargo = root.join("Cargo.toml").is_file();
        let text = starter_toml(&prefix, cargo);
        let config = Config::parse_at(&text, &root)
            .map_err(|e| format!("the starter configuration cannot be read: {e}"))?;
        plan_create(&root, CONFIG, text, &mut plan);
        config
    };
    plan_create(&root, ".stapel/allowlist.toml", ALLOWLIST.into(), &mut plan);
    plan_create(&root, ".stapel/tickets/.gitkeep", String::new(), &mut plan);
    plan_create(
        &root,
        ".stapel/config/coverage-taxonomy.yml",
        include_str!("../assets/config/coverage-taxonomy.yml").into(),
        &mut plan,
    );
    plan_create(
        &root,
        ".stapel/config/external-states.yml",
        include_str!("../assets/config/external-states.yml").into(),
        &mut plan,
    );
    plan_ignore_index(&root, &mut plan)?;
    plan_hooks(&root, &mut plan)?;
    plan_pre_push(&root, &mut plan)?;
    let mut kept = false;
    plan_process_files(&root, &config, &mut plan, &mut kept)?;

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
        warn!(
            "warning: stapel is not on PATH; until it is, the hook denies every tool call it \
             checks"
        );
    }
    if plan.is_empty() && !kept {
        say!("already set up: nothing changed");
    }
    for step in plan {
        say!("{}", step.label);
    }
    if new_folders {
        say!("note: restart Claude Code so that it loads the new agents and skill");
    }
    Ok(())
}

const SKILL_PATH: &str = ".claude/skills/stapel/SKILL.md";
const SKILL_DESCRIPTION: &str =
    "Carries a development ticket from description to merge with the stapel process.";

/// One agent file: its name, role in `[models]`, text, tools and default permission mode.
struct AgentFile {
    name: &'static str,
    role: &'static str,
    description: &'static str,
    tools: &'static str,
    default_mode: &'static str,
    body: &'static str,
}

const READ_ONLY_TOOLS: &str = "Read, Grep, Glob";

const AGENT_FILES: [AgentFile; 4] = [
    AgentFile {
        name: "builder",
        role: "builder",
        description: "Builds one plan step of a stapel ticket: writes the tests or the code of one commit and reports what is left.",
        tools: "Read, Edit, Write, Bash, Grep, Glob",
        default_mode: "acceptEdits",
        body: include_str!("../assets/agents/builder.md"),
    },
    AgentFile {
        name: "drift",
        role: "drift",
        description: "Checks the diff of one step against its criteria and reports where the code and the criteria differ.",
        tools: READ_ONLY_TOOLS,
        default_mode: "default",
        body: include_str!("../assets/agents/drift.md"),
    },
    AgentFile {
        name: "reviewer",
        role: "reviewer",
        description: "Reviews the code of a ticket on a copy of the repository and reports findings without changing anything.",
        tools: READ_ONLY_TOOLS,
        default_mode: "default",
        body: include_str!("../assets/agents/reviewer.md"),
    },
    AgentFile {
        name: "spec-reviewer",
        role: "reviewer",
        description: "Reviews the frame and the criteria of a ticket and reports findings without changing anything.",
        tools: READ_ONLY_TOOLS,
        default_mode: "default",
        body: include_str!("../assets/agents/spec-reviewer.md"),
    },
];

/// Stapel's text of a process file: the frontmatter lines (without the marker), the marker line
/// with the hash of everything else, and the body.
fn process_text(lines: &[String], body: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut rest = String::from("---\n");
    for line in lines {
        rest.push_str(line);
        rest.push('\n');
    }
    rest.push_str("---\n");
    rest.push_str(body);
    let hash: String = Sha256::digest(rest.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let marker = format!("# stapel: {} sha256:{hash}\n", env!("CARGO_PKG_VERSION"));
    // The marker is the last frontmatter line; the hash is over the file without it.
    let at = rest.len() - body.len() - "---\n".len();
    rest.insert_str(at, &marker);
    rest
}

const ALLOWED_MODES: [&str; 6] = [
    "default",
    "acceptEdits",
    "auto",
    "dontAsk",
    "plan",
    "manual",
];
const AGENT_ROLES: [&str; 3] = ["builder", "drift", "reviewer"];

/// Refuses a binding of an agent role whose mode or model cannot go into an agent file.
fn check_bindings(config: &Config) -> Result<(), String> {
    for role in AGENT_ROLES {
        let Some(binding) = config.models.get(role) else {
            continue;
        };
        match binding.permission_mode.as_deref() {
            Some("bypassPermissions") => {
                return Err(format!(
                    "[models] {role}: permission_mode bypassPermissions is not allowed: it turns \
                     off the permission dialog for every call the guard lets through"
                ));
            }
            Some(mode) if !ALLOWED_MODES.contains(&mode) => {
                return Err(format!(
                    "[models] {role}: permission_mode \"{mode}\" is not one of {}",
                    ALLOWED_MODES.join(", ")
                ));
            }
            _ => {}
        }
        let model = &binding.model;
        let first_ok = model
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric());
        let rest_ok = model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".:_-/[]".contains(c));
        if !first_ok || !rest_ok {
            return Err(format!(
                "[models] {role}: model \"{model}\" cannot be written to an agent file"
            ));
        }
    }
    Ok(())
}

/// Plans the skill and the four agent files: each is created when missing, updated when it is an
/// unchanged file of stapel, and kept with a warning otherwise.
fn plan_process_files(
    root: &Path,
    config: &Config,
    plan: &mut Vec<Step>,
    kept: &mut bool,
) -> Result<(), String> {
    check_bindings(config)?;
    let paths = std::iter::once(SKILL_PATH.to_string()).chain(
        AGENT_FILES
            .iter()
            .map(|a| format!(".claude/agents/stapel-{}.md", a.name)),
    );
    for rel in paths {
        if std::fs::symlink_metadata(root.join(&rel)).is_ok_and(|m| m.is_dir()) {
            return Err(format!("{rel} is a directory; nothing was written"));
        }
    }
    let skill = process_text(
        &[
            "name: stapel".to_string(),
            format!("description: \"{SKILL_DESCRIPTION}\""),
        ],
        include_str!("../assets/skill.md"),
    );
    plan_create_text(root, SKILL_PATH.to_string(), skill, plan, kept);
    for agent in AGENT_FILES {
        let binding = config.models.get(agent.role);
        let mut lines = vec![
            format!("name: stapel-{}", agent.name),
            format!("description: \"{}\"", agent.description),
            format!("tools: {}", agent.tools),
        ];
        if let Some(b) = binding {
            lines.push(format!("model: \"{}\"", b.model));
        }
        let mode = binding
            .and_then(|b| b.permission_mode.as_deref())
            .unwrap_or(agent.default_mode);
        lines.push(format!("permissionMode: {mode}"));
        let text = process_text(&lines, agent.body);
        plan_create_text(
            root,
            format!(".claude/agents/stapel-{}.md", agent.name),
            text,
            plan,
            kept,
        );
    }
    Ok(())
}

/// Plans `rel` when nothing is at its path, not even a symlink; plans an update when the file is
/// an unchanged file of a version not greater than the binary's whose text differs.
/// Warns and sets `kept` for a file that is left alone with a warning.
fn plan_create_text(
    root: &Path,
    rel: String,
    content: String,
    plan: &mut Vec<Step>,
    kept: &mut bool,
) {
    let path = root.join(&rel);
    let not_stapels = |kept: &mut bool| {
        *kept = true;
        warn!(
            "warning: {rel} is not the file stapel wrote; kept. Delete it and run stapel init to \
             take stapel's text"
        );
    };
    let label = match std::fs::symlink_metadata(&path) {
        Err(_) => format!("created: {rel}"),
        Ok(meta) if meta.is_file() => {
            let Some(version) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|old| unchanged_version(&old).map(|v| (old, v)))
            else {
                not_stapels(kept);
                return;
            };
            let (old, version) = version;
            let own = env!("CARGO_PKG_VERSION");
            if version > parse_version(own).unwrap_or_default() {
                *kept = true;
                warn!(
                    "warning: {rel} was written by stapel {}.{}.{}, newer than this stapel {own}; \
                     kept",
                    version.0, version.1, version.2
                );
                return;
            }
            if old == content {
                return;
            }
            format!("updated: {rel}")
        }
        Ok(_) => {
            not_stapels(kept);
            return;
        }
    };
    plan.push(Step {
        label,
        rel,
        content,
        executable: false,
    });
}

/// `a.b.c` as numbers; `None` when it has another form.
fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.split('.');
    let mut next = || {
        let p = parts.next()?;
        (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse::<u64>().ok())
            .flatten()
    };
    let version = (next()?, next()?, next()?);
    parts.next().is_none().then_some(version)
}

/// The marker version of an unchanged file: a valid marker whose hash is the SHA-256 of the text
/// without the marker line and its newline. `None` for any other text.
fn unchanged_version(text: &str) -> Option<(u64, u64, u64)> {
    use sha2::{Digest, Sha256};
    let mut lines = text.split_inclusive('\n');
    if lines.next()?.trim_end_matches('\n') != "---" {
        return None;
    }
    let mut marker = None;
    let mut closed = false;
    let mut without = String::from("---\n");
    for line in lines.by_ref() {
        let bare = line.trim_end_matches('\n');
        if bare == "---" {
            closed = true;
            without.push_str(line);
            break;
        }
        if bare.starts_with("# stapel:") {
            if marker.replace(bare).is_some() {
                return None;
            }
        } else {
            without.push_str(line);
        }
    }
    if !closed {
        return None;
    }
    without.extend(lines);
    let rest = marker?.strip_prefix("# stapel: ")?;
    let (version, hash) = rest.split_once(" sha256:")?;
    let version = parse_version(version)?;
    let valid_hash = hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let digest: String = Sha256::digest(without.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    (valid_hash && digest == hash).then_some(version)
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
    say_raw!("Ticket key prefix (2-8 uppercase Latin letters, e.g. STP): ");
    crate::out::flush().map_err(|e| e.to_string())?;
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

/// Plans the stapel PreToolUse, PostToolUse, PostToolUseFailure and SessionStart hooks in
/// `.claude/settings.json` as one write, keeping every other key and hook.
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

    let root_object = settings
        .as_object_mut()
        .ok_or(format!("{SETTINGS}: expected a JSON object"))?;
    // Check the allow list before changing anything, so a refusal leaves the value as read.
    if root_object
        .get("permissions")
        .is_some_and(|p| !p.is_object() || p["allow"].as_array().is_none() && !p["allow"].is_null())
    {
        return Err(format!(
            "{SETTINGS}: the permissions.allow field must be an array"
        ));
    }
    let allow_added = {
        let allow = root_object
            .entry("permissions")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("checked to be an object")
            .entry("allow")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("checked to be an array");
        let mut added = false;
        for rule in ALLOW_RULES {
            if !allow.iter().any(|v| v.as_str() == Some(rule)) {
                allow.push(json!(rule));
                added = true;
            }
        }
        added
    };
    let hooks = root_object
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(format!("{SETTINGS}: the hooks field must be an object"))?;
    // Check all four arrays before changing anything, so a refusal leaves the value as read.
    for event in [
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "SessionStart",
    ] {
        if hooks.get(event).is_some_and(|v| !v.is_array()) {
            return Err(format!(
                "{SETTINGS}: the hooks.{event} field must be an array"
            ));
        }
    }
    let (pre_upgraded, pre_added) = plan_pre_hook(hook_array(hooks, "PreToolUse"));
    let mut post_added = false;
    for event in POST_HOOK_EVENTS {
        let entries = hook_array(hooks, event);
        if !has_command(entries, POST_HOOK_COMMAND) {
            entries.push(json!({
                "matcher": POST_HOOK_MATCHER,
                "hooks": [{ "type": "command", "command": POST_HOOK_LINE, "timeout": HOOK_TIMEOUT }]
            }));
            post_added = true;
        }
    }
    let session_added = {
        let entries = hook_array(hooks, "SessionStart");
        let missing = !has_command(entries, SESSION_HOOK_COMMAND);
        if missing {
            entries.push(json!({
                "matcher": SESSION_HOOK_MATCHER,
                "hooks": [{ "type": "command", "command": SESSION_HOOK_LINE, "timeout": HOOK_TIMEOUT }]
            }));
        }
        missing
    };
    if !pre_upgraded && !pre_added && !post_added && !session_added && !allow_added {
        return Ok(());
    }

    let mut content = serde_json::to_string_pretty(&settings).expect("JSON value serializes");
    content.push('\n');
    let label = if pre_upgraded {
        format!("updated: {SETTINGS} (stapel hook line)")
    } else if !existed {
        format!("created: {SETTINGS}")
    } else if !pre_added && !post_added && !session_added {
        format!("appended: {SETTINGS} (stapel allow rules)")
    } else if !pre_added && !post_added {
        format!("appended: {SETTINGS} (stapel session hook)")
    } else if pre_added && post_added {
        format!("appended: {SETTINGS} (stapel hooks)")
    } else if post_added {
        format!("appended: {SETTINGS} (stapel commit hooks)")
    } else {
        format!("appended: {SETTINGS} (stapel hook)")
    };
    plan.push(Step {
        rel: SETTINGS.into(),
        content,
        label,
        executable: false,
    });
    Ok(())
}

/// The `hooks.<event>` array, created empty when missing; `plan_hooks` has checked its type.
fn hook_array<'a>(
    hooks: &'a mut serde_json::Map<String, Value>,
    event: &str,
) -> &'a mut Vec<Value> {
    hooks
        .entry(event)
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .expect("checked to be an array")
}

/// Whether any hook of `entries` runs a command that contains `command`.
fn has_command(entries: &[Value], command: &str) -> bool {
    entries.iter().any(|entry| {
        entry["hooks"].as_array().is_some_and(|hooks| {
            hooks
                .iter()
                .any(|h| h["command"].as_str().is_some_and(|c| c.contains(command)))
        })
    })
}

/// Upgrades a bare PreToolUse entry or adds a new one; returns (upgraded, added).
fn plan_pre_hook(pre: &mut Vec<Value>) -> (bool, bool) {
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
    let installed = has_command(pre, HOOK_COMMAND);
    if !installed {
        pre.push(json!({
            "matcher": HOOK_MATCHER,
            "hooks": [{ "type": "command", "command": HOOK_LINE, "timeout": HOOK_TIMEOUT }]
        }));
    }
    (upgraded, !installed)
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
        warn!(
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
            warn!(
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
