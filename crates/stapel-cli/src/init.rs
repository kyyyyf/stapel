//! `stapel init`: the `.stapel/` layout in the repository root.

use serde_json::{Value, json};
use stapel_core::config::{Config, History, starter_toml, validate_prefix};
use std::io::{BufRead, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::Command;

const CONFIG: &str = ".stapel/stapel.toml";
const ALLOWLIST: &str = "# Reviewer false positives that were decided not to fix.\n";
const IGNORE_LINES: [&str; 3] = [
    "/.stapel/index/",
    "/.claude/worktrees/",
    "/.stapel/tickets/*/page.md",
];
const UNION_LINES: [&str; 2] = [
    ".stapel/process-ledger.jsonl merge=union",
    ".stapel/sessions.jsonl merge=union",
];
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
/// The token hooks: event, hook name. No matcher; the entry waits up to 30 seconds because
/// `SessionEnd` has a timeout of 1.5 seconds by default.
const TOKEN_HOOKS: [(&str, &str); 3] = [
    ("SubagentStop", "subagent-stop"),
    ("Stop", "stop"),
    ("SessionEnd", "session-end"),
];
const TOKEN_HOOK_TIMEOUT: u64 = 30;
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

/// The pre-push hook of the local mode: the agent check of `PRE_PUSH`, then a refusal of new
/// commits that carry `.stapel/`.
const LOCAL_PRE_PUSH: &str = r#"#!/bin/sh
# stapel: pre-push guard, installed by `stapel init`.
# Claude Code sets CLAUDECODE (and CLAUDE_CODE_ENTRYPOINT) in the shell it gives the agent;
# pushes from there are refused.
if [ -n "${CLAUDECODE:-}" ] || [ -n "${CLAUDE_CODE_ENTRYPOINT:-}" ]; then
  echo "stapel: push from an agent session is not allowed; the human pushes from their own terminal" >&2
  exit 1
fi
# stapel: local history check
while read -r lref lsha rref rsha extra; do
  [ -n "$lref" ] && [ -n "$lsha" ] && [ -n "$rref" ] && [ -n "$rsha" ] && [ -z "$extra" ] || continue
  case "$lsha" in *[!0]*) ;; *) continue ;; esac
  if [ -n "$(git rev-list -n 1 --full-history "$lsha" --not --remotes -- .stapel 2>/dev/null)" ]; then
    echo "stapel: $lref carries .stapel/; push the branch that stapel export writes" >&2
    exit 1
  fi
done
exit 0
"#;

/// Test-only switch: treat stdin as a terminal so the prefix prompt can be driven from a pipe.
const ASSUME_TTY: &str = "STAPEL_ASSUME_TTY";

pub fn run(prefix: Option<String>, local: bool) -> Result<(), String> {
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
        let is_local = config
            .repository
            .as_ref()
            .is_some_and(|r| r.history == History::Local);
        if local && !is_local {
            return Err(format!(
                "{CONFIG} is in shared mode; --local applies only to the first stapel init. To \
                 switch, add [repository] with history = \"local\" to it, remove stapel's lines \
                 from .gitignore and .gitattributes, and run stapel init"
            ));
        }
        if prefix.is_some() {
            say!(
                "{CONFIG} already exists, the ticket key stays {}; edit the file to change it",
                config.tickets.key
            );
        }
        if let Some(new_text) = add_roles_rows(&text, &config) {
            plan.push(Step {
                label: format!("updated: {CONFIG}"),
                rel: CONFIG.into(),
                content: new_text,
                executable: false,
            });
        }
        config
    } else {
        let prefix = match prefix {
            Some(p) => p,
            None => ask_prefix()?,
        };
        validate_prefix(&prefix)?;
        let cargo = root.join("Cargo.toml").is_file();
        let mut text = starter_toml(&prefix, cargo);
        if local {
            text.push_str("\n[repository]\nhistory = \"local\"\n");
        }
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
    let is_local = config
        .repository
        .as_ref()
        .is_some_and(|r| r.history == History::Local);
    let hook_rel = plan_pre_push(&root, is_local, &mut plan)?;
    let mut ignore: Vec<String> = IGNORE_LINES.iter().map(|l| l.to_string()).collect();
    if let (true, Some(rel)) = (is_local, hook_rel) {
        // A pre-push in a tracked hooks folder (`.husky/`) must not reach a merge request.
        if !Path::new(&rel).starts_with(".git") {
            ignore.push(format!("/{rel}"));
        }
    }
    plan_ignore_index(&root, is_local, &ignore, &mut plan)?;
    plan_hooks(&root, &mut plan)?;
    let mut kept = false;
    plan_process_files(&root, &config, &mut plan, &mut kept)?;
    let mut junit_note = false;
    if !is_local {
        check_workflow_path(&root)?;
        junit_note = plan_workflow(&config, &root, &mut plan, &mut kept);
    }

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
    if junit_note {
        say!("note: fill in the step Setup of .github/workflows/stapel-check.yml; the job fails until you do");
    }
    if new_folders {
        say!("note: restart Claude Code so that it loads the new agents and skill");
    }
    if is_local {
        say!("{}", LOCAL_CHECKLIST.trim_end());
    }
    Ok(())
}

const WORKFLOW: &str = ".github/workflows/stapel-check.yml";

/// Refuses when the workflow's path, `.github` or `.github/workflows` is not a place to write a
/// file: a folder at the file path, a file or a symlink at a folder path. A missing path is fine.
fn check_workflow_path(root: &Path) -> Result<(), String> {
    for rel in [".github", ".github/workflows"] {
        if let Ok(meta) = std::fs::symlink_metadata(root.join(rel)) {
            if !meta.is_dir() {
                return Err(format!("{rel} is not a directory; nothing was written"));
            }
        }
    }
    if std::fs::symlink_metadata(root.join(WORKFLOW)).is_ok_and(|m| m.is_dir()) {
        return Err(format!("{WORKFLOW} is a directory; nothing was written"));
    }
    Ok(())
}

/// The prefix of a key `<PREFIX>-{n}` with 2 to 8 uppercase Latin letters.
fn key_prefix(key: &str) -> Option<&str> {
    let prefix = key.strip_suffix("-{n}")?;
    let ok = (2..=8).contains(&prefix.len()) && prefix.bytes().all(|b| b.is_ascii_uppercase());
    ok.then_some(prefix)
}

/// Plans the workflow: created when missing, updated when it is an unchanged file of stapel, kept
/// with a warning otherwise. Nothing is planned without a prefix or a clean build commit.
/// The lines `plan_create_text` prints for it, spelled out so that the ticket lint finds them:
/// `created: .github/workflows/stapel-check.yml`, `updated: .github/workflows/stapel-check.yml`,
/// `warning: .github/workflows/stapel-check.yml is not the file stapel wrote; kept. Delete it and run stapel init to take stapel's text`.
fn plan_workflow(config: &Config, root: &Path, plan: &mut Vec<Step>, kept: &mut bool) -> bool {
    let key = &config.tickets.key;
    let Some(prefix) = key_prefix(key) else {
        warn!(
            "warning: the ticket key {key} is not <PREFIX>-{{n}}, so {WORKFLOW} was not written"
        );
        return false;
    };
    let commit = match crate::build_commit::build_commit() {
        crate::build_commit::BuildCommit::Clean(commit) => commit,
        crate::build_commit::BuildCommit::Dirty(value) => {
            warn!(
                "warning: this stapel was built from a dirty tree ({value}), so {WORKFLOW} was \
                 not written; commit and push the change, then reinstall stapel"
            );
            return false;
        }
        crate::build_commit::BuildCommit::Missing => {
            // One line, so that the ticket lint finds the quoted text.
            #[rustfmt::skip]
            warn!("warning: this stapel has no build commit, so .github/workflows/stapel-check.yml was not written; reinstall stapel from a clean checkout as docs/install.md says");
            return false;
        }
    };
    let junit = config.check.as_ref().is_some_and(|c| c.runner == "junit");
    let template = if junit {
        include_str!("../assets/stapel-check-junit.yml")
    } else {
        include_str!("../assets/stapel-check.yml")
    };
    let text = template
        .replace("{PREFIX}", prefix)
        .replace("{COMMIT}", &commit);
    use sha2::{Digest, Sha256};
    let hash: String = Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let text = format!("# stapel: {} sha256:{hash}\n{text}", env!("CARGO_PKG_VERSION"));
    let before = plan.len();
    let equal = std::fs::symlink_metadata(root.join(WORKFLOW)).is_ok_and(|m| m.is_file())
        && std::fs::read_to_string(root.join(WORKFLOW)).is_ok_and(|old| old == text);
    plan_create_text(root, WORKFLOW.to_string(), text, plan, kept, unchanged_workflow_version);
    junit && (plan.len() > before || equal)
}

/// The marker version of an unchanged workflow: the first line is the marker, its hash is the
/// SHA-256 of the rest.
fn unchanged_workflow_version(text: &str) -> Option<(u64, u64, u64)> {
    use sha2::{Digest, Sha256};
    let (marker, body) = text.split_once('\n')?;
    let rest = marker.strip_prefix("# stapel: ")?;
    let (version, hash) = rest.split_once(" sha256:")?;
    let version = parse_version(version)?;
    let digest: String = Sha256::digest(body.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    (hash.len() == 64 && digest == hash).then_some(version)
}

/// The person's manual steps after `init` in the local mode.
const LOCAL_CHECKLIST: &str = "checklist:
1. Commit the installed files on the base branch with these two lines:
   git add .stapel
   git add -f .claude/skills/stapel .claude/agents/stapel-*.md .claude/settings.json
2. Fill in [check] command in .stapel/stapel.toml.
3. Restart Claude Code and see \"stapel next\" at session start.
4. Run stapel init again after anything changes core.hooksPath (husky does on npm install).
5. Push only the export/<KEY> branches that stapel export writes.
";

/// The text of an existing `stapel.toml` with the `[roles]` row of each installed agent that the
/// table lacks, every other line kept; `None` when there is no `[roles]` table or no row is missing.
fn add_roles_rows(text: &str, config: &Config) -> Option<String> {
    let roles = config.roles.as_ref()?;
    let missing: Vec<String> = AGENT_FILES
        .iter()
        .filter(|a| !roles.contains_key(&format!("stapel-{}", a.name)))
        .map(|a| format!("stapel-{} = \"{}\"", a.name, a.role))
        .collect();
    if missing.is_empty() {
        return None;
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let at = lines.iter().position(|l| l.trim() == "[roles]")?;
    let mut last = at;
    for (i, l) in lines.iter().enumerate().skip(at + 1) {
        let t = l.trim();
        if t.starts_with('[') {
            break;
        }
        if !t.is_empty() && !t.starts_with('#') {
            last = i;
        }
    }
    let mut out = String::new();
    for (i, l) in lines.iter().enumerate() {
        out.push_str(l);
        if i == last {
            if !l.ends_with('\n') {
                out.push('\n');
            }
            for row in &missing {
                out.push_str(row);
                out.push('\n');
            }
        }
    }
    Some(out)
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

const AGENT_FILES: [AgentFile; 5] = [
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
        description: "Checks the diff of one step, or the pack.md of the whole ticket, against its criteria and reports where the code and the criteria differ.",
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
    AgentFile {
        name: "depends",
        role: "depends",
        description: "Compares two parallel tickets before the build and reports a dependency of kind spec or merge without changing anything.",
        tools: READ_ONLY_TOOLS,
        default_mode: "default",
        body: include_str!("../assets/agents/depends.md"),
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
const AGENT_ROLES: [&str; 4] = ["builder", "drift", "reviewer", "depends"];

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
    plan_create_text(root, SKILL_PATH.to_string(), skill, plan, kept, unchanged_version);
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
            unchanged_version,
        );
    }
    plan_elaboration_files(root, plan, kept);
    Ok(())
}

const ELABORATION_DESCRIPTION: &str = "Turns the intent for a piece of work larger than one ticket into requirements with ids and a breakdown into tickets.";

/// Plans the four files of the skill `stapel-elaborate` by the rules of the other process files.
fn plan_elaboration_files(root: &Path, plan: &mut Vec<Step>, kept: &mut bool) {
    use sha2::{Digest, Sha256};
    let skill = process_text(
        &[
            "name: stapel-elaborate".to_string(),
            format!("description: \"{ELABORATION_DESCRIPTION}\""),
        ],
        include_str!("../assets/elaborate/skill.md"),
    );
    let line_marked = |body: &str| {
        let hash: String = Sha256::digest(body.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        format!(
            "<!-- stapel: {} sha256:{hash} -->\n{body}",
            env!("CARGO_PKG_VERSION")
        )
    };
    let files = [
        ("SKILL.md", skill, unchanged_version as fn(&str) -> _),
        (
            "template.md",
            line_marked(include_str!("../assets/elaborate/template.md")),
            line_marker_version,
        ),
        (
            "checklist.md",
            line_marked(include_str!("../assets/elaborate/checklist.md")),
            line_marker_version,
        ),
        (
            "NOTICE.md",
            line_marked(include_str!("../assets/elaborate/notice.md")),
            line_marker_version,
        ),
    ];
    for (name, content, version_of) in files {
        let rel = format!(".claude/skills/stapel-elaborate/{name}");
        plan_create_text(root, rel, content, plan, kept, version_of);
    }
}

/// The marker version of an unchanged file whose first line is
/// `<!-- stapel: <version> sha256:<hex> -->` and whose hash is that of the text after that line.
/// `None` for any other text.
fn line_marker_version(text: &str) -> Option<(u64, u64, u64)> {
    use sha2::{Digest, Sha256};
    let (first, rest) = text.split_once('\n')?;
    let inner = first
        .strip_prefix("<!-- stapel: ")?
        .strip_suffix(" -->")?;
    let (version, hash) = inner.split_once(" sha256:")?;
    let version = parse_version(version)?;
    let valid_hash = hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let digest: String = Sha256::digest(rest.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    (valid_hash && digest == hash).then_some(version)
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
    version_of: fn(&str) -> Option<(u64, u64, u64)>,
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
                .and_then(|old| version_of(&old).map(|v| (old, v)))
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

fn plan_ignore_index(
    root: &Path,
    local: bool,
    ignore: &[String],
    plan: &mut Vec<Step>,
) -> Result<(), String> {
    let ignore: Vec<&str> = ignore.iter().map(String::as_str).collect();
    if local {
        let exclude = git_path(root, "info/exclude")?;
        let attributes = git_path(root, "info/attributes")?;
        plan_append_lines(root, &exclude, &ignore, plan)?;
        return plan_append_lines(root, &attributes, &UNION_LINES, plan);
    }
    plan_append_lines(root, ".gitignore", &ignore, plan)?;
    plan_append_lines(root, ".gitattributes", &UNION_LINES, plan)
}

/// The path git prints for `--git-path name`, as written relative to `root` or absolute when it
/// lies outside `root`.
fn git_path(root: &Path, name: &str) -> Result<String, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--git-path", name])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err(format!("git could not name the path {name}"));
    }
    let printed = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    let abs = root.join(&printed);
    Ok(match abs.strip_prefix(root) {
        Ok(rel) => rel.display().to_string(),
        Err(_) => abs.display().to_string(),
    })
}

/// Plans appending the lines of `lines` that `rel` lacks; a file that is not UTF-8 stays untouched.
fn plan_append_lines(
    root: &Path,
    rel: &str,
    lines: &[&str],
    plan: &mut Vec<Step>,
) -> Result<(), String> {
    let mut existed = true;
    let mut text = match std::fs::read(root.join(rel)) {
        Ok(bytes) => String::from_utf8(bytes)
            .map_err(|_| format!("{rel} is not UTF-8; the file was not touched"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            existed = false;
            String::new()
        }
        Err(e) => return Err(format!("{rel}: {e}")),
    };
    let mut changed = false;
    for line in lines {
        if text.lines().any(|l| l.trim_end() == *line) {
            continue;
        }
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(line);
        text.push('\n');
        changed = true;
    }
    if changed {
        plan.push(Step {
            rel: rel.into(),
            content: text,
            label: format!("{}: {rel}", if existed { "appended" } else { "created" }),
            executable: false,
        });
    }
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
    if root_object.get("permissions").is_some_and(|p| {
        p.as_object()
            .is_none_or(|o| o.get("allow").is_some_and(|a| !a.is_array()))
    }) {
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
        "SubagentStop",
        "Stop",
        "SessionEnd",
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
    let mut token_added = false;
    for (event, name) in TOKEN_HOOKS {
        let entries = hook_array(hooks, event);
        let command = format!("stapel hook {name}");
        if !has_command(entries, &command) {
            let line = format!(
                "command -v stapel >/dev/null 2>&1 || exit 0; {command} 2>/dev/null || exit 0"
            );
            entries.push(json!({
                "hooks": [{ "type": "command", "command": line, "timeout": TOKEN_HOOK_TIMEOUT }]
            }));
            token_added = true;
        }
    }
    let session_added = session_added || token_added;
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

/// The Pre-push path: what `git rev-parse --git-path hooks/pre-push` prints, made absolute.
fn pre_push_path(root: &Path) -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks/pre-push"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err("git did not report the repository hooks path".into());
    }
    let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim_end());
    Ok(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

/// In Local mode: the Pre-push path, relative to `root`, when the file there is missing, not
/// executable or not the Local-mode text, and `init` could write it (inside the work tree).
/// A hook without stapel's marker is not ours and gives no fact. `git` runs in the main worktree
/// (the first entry of `git worktree list --porcelain`), where a relative hooks path is settled.
pub(crate) fn stale_local_pre_push(root: &Path) -> Option<String> {
    let main = main_worktree(root).unwrap_or_else(|| root.to_path_buf());
    let path = pre_push_path(&main).ok()?;
    let rel = path.strip_prefix(&main).ok()?.display().to_string();
    let fresh = match std::fs::read(&path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            if !text.contains(PRE_PUSH_MARKER) {
                return None;
            }
            text == LOCAL_PRE_PUSH && is_executable(&path)
        }
        Err(_) => false,
    };
    (!fresh).then_some(rel)
}

/// The first worktree that `git worktree list --porcelain` names.
fn main_worktree(root: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.strip_prefix("worktree ")?;
    Some(PathBuf::from(first))
}

/// Plans the git pre-push hook in the hooks directory git uses (`core.hooksPath` included).
/// A pre-push hook that is not ours is kept, with a warning.
/// Returns the hook path relative to `root` when a stapel hook is there.
fn plan_pre_push(root: &Path, local: bool, plan: &mut Vec<Step>) -> Result<Option<String>, String> {
    let wanted = if local { LOCAL_PRE_PUSH } else { PRE_PUSH };
    // In Local mode the path is settled in the main worktree, as `stale_local_pre_push` does.
    let base = if local {
        main_worktree(root).unwrap_or_else(|| root.to_path_buf())
    } else {
        root.to_path_buf()
    };
    let root = base.as_path();
    let path = pre_push_path(root)?;
    let Ok(rel) = path.strip_prefix(root).map(|p| p.display().to_string()) else {
        // A shared hooks directory would put the hook into every repository that uses it.
        warn!(
            "warning: the git hooks directory {} is outside the repository (core.hooksPath); \
             pre-push was not installed, so the second layer against agent pushes is off",
            path.parent().unwrap_or(&path).display()
        );
        return Ok(None);
    };

    match std::fs::read(&path) {
        Ok(bytes) if String::from_utf8_lossy(&bytes).contains(PRE_PUSH_MARKER) => {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            if text != wanted {
                plan.push(Step {
                    rel: path.display().to_string(),
                    content: wanted.into(),
                    label: format!("updated: {rel} (pre-push)"),
                    executable: true,
                });
            } else if !is_executable(&path) {
                plan.push(Step {
                    rel: path.display().to_string(),
                    content: text,
                    label: format!("fixed: {rel} (pre-push is executable again)"),
                    executable: true,
                });
            }
            Ok(Some(rel))
        }
        Ok(_) => {
            warn!(
                "warning: {rel} exists and is not from stapel; the second layer against agent \
                 pushes was not installed. Add a check of the CLAUDECODE variable to it by hand"
            );
            Ok(None)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            plan.push(Step {
                rel: path.display().to_string(),
                content: wanted.into(),
                label: format!("created: {rel} (pre-push: second layer against agent pushes)"),
                executable: true,
            });
            Ok(Some(rel))
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
