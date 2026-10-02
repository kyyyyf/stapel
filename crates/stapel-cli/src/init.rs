//! `stapel init`: the `.stapel/` layout in the repository root.

use serde_json::{Value, json};
use stapel_core::config::{Config, default_toml, validate_prefix};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const CONFIG: &str = ".stapel/stapel.toml";
const ALLOWLIST: &str = "# Ложные срабатывания ревьюеров, которые решено не исправлять.\n";
const IGNORE_LINE: &str = "/.stapel/index/";
const SETTINGS: &str = ".claude/settings.json";
const HOOK_COMMAND: &str = "stapel hook pre-tool-use";
const HOOK_MATCHER: &str = "Bash|Write|Edit|MultiEdit|NotebookEdit";

/// Test-only switch: treat stdin as a terminal so the prefix prompt can be driven from a pipe.
const ASSUME_TTY: &str = "STAPEL_ASSUME_TTY";

pub fn run(prefix: Option<String>) -> Result<(), String> {
    let root = repo_root()?;
    let mut created = Vec::new();

    let config_path = root.join(CONFIG);
    if config_path.exists() {
        let text = std::fs::read_to_string(&config_path).map_err(|e| format!("{CONFIG}: {e}"))?;
        let config = Config::parse(&text).map_err(|e| format!("{CONFIG} не читается: {e}"))?;
        if prefix.is_some() {
            println!(
                "{CONFIG} уже есть, ключ тикетов остаётся {}; чтобы сменить, правьте файл",
                config.tickets.key
            );
        }
    } else {
        let prefix = match prefix {
            Some(p) => p,
            None => ask_prefix()?,
        };
        validate_prefix(&prefix)?;
        create(&root, CONFIG, &default_toml(&prefix), &mut created)?;
    }
    create(&root, ".stapel/allowlist.toml", ALLOWLIST, &mut created)?;
    create(&root, ".stapel/tickets/.gitkeep", "", &mut created)?;
    ignore_index(&root, &mut created)?;
    install_hooks(&root, &mut created)?;

    if !on_path("stapel") {
        eprintln!(
            "предупреждение: stapel не найден в PATH; пока его там нет, хуки Claude Code не срабатывают"
        );
    }

    if created.is_empty() {
        println!("уже готово: ничего не изменено");
    }
    for line in created {
        println!("{line}");
    }
    Ok(())
}

fn repo_root() -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("не удалось запустить git: {e}"))?;
    if !out.status.success() {
        return Err("не git-репозиторий: запустите stapel init внутри репозитория".into());
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim_end(),
    ))
}

fn ask_prefix() -> Result<String, String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() && std::env::var_os(ASSUME_TTY).is_none() {
        return Err(
            "префикс ключа не задан: укажите --prefix, например stapel init --prefix STP".into(),
        );
    }
    print!("Префикс ключа тикетов (2–8 заглавных латинских букв, например STP): ");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok(line.trim().to_string())
}

/// Writes `rel` only when it does not exist yet, so a second run and hand edits leave it alone.
fn create(root: &Path, rel: &str, content: &str, created: &mut Vec<String>) -> Result<(), String> {
    let path = root.join(rel);
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(&path, content).map_err(|e| format!("{rel}: {e}"))?;
    created.push(format!("создано: {rel}"));
    Ok(())
}

fn ignore_index(root: &Path, created: &mut Vec<String>) -> Result<(), String> {
    let path = root.join(".gitignore");
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    if text.lines().any(|l| l.trim_end() == IGNORE_LINE) {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(IGNORE_LINE);
    text.push('\n');
    std::fs::write(&path, text).map_err(|e| format!(".gitignore: {e}"))?;
    created.push("дописано: .gitignore".into());
    Ok(())
}

/// Adds the stapel PreToolUse hook to `.claude/settings.json`, keeping every other key and hook.
fn install_hooks(root: &Path, created: &mut Vec<String>) -> Result<(), String> {
    let path = root.join(SETTINGS);
    let existed = path.exists();
    let mut settings: Value = if existed {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{SETTINGS}: {e}"))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("{SETTINGS} не разбирается как JSON, файл не тронут: {e}"))?
    } else {
        json!({})
    };

    let pre = settings
        .as_object_mut()
        .ok_or(format!("{SETTINGS}: ожидался JSON-объект"))?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(format!("{SETTINGS}: поле hooks должно быть объектом"))?
        .entry("PreToolUse")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or(format!(
            "{SETTINGS}: поле hooks.PreToolUse должно быть массивом"
        ))?;

    let installed = pre.iter().any(|entry| {
        entry["hooks"]
            .as_array()
            .is_some_and(|hooks| hooks.iter().any(|h| h["command"] == HOOK_COMMAND))
    });
    if installed {
        return Ok(());
    }
    pre.push(json!({
        "matcher": HOOK_MATCHER,
        "hooks": [{ "type": "command", "command": HOOK_COMMAND }]
    }));

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let mut text = serde_json::to_string_pretty(&settings).expect("JSON value serializes");
    text.push('\n');
    std::fs::write(&path, text).map_err(|e| format!("{SETTINGS}: {e}"))?;
    created.push(if existed {
        format!("дописано: {SETTINGS} (хук stapel)")
    } else {
        format!("создано: {SETTINGS}")
    });
    Ok(())
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}
