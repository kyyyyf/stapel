//! The permission dialog for `stapel ok` and `stapel close` run by an agent (STP-2 AC-15, AC-16).
//!
//! The guard runs the same checks the command will run, so a "yes" does not end in a refusal,
//! issues a one-time grant, and asks Claude Code to show the person what they approve.

use crate::config::Config;
use crate::confirm::{Prepared, prepare};
use crate::grant::{Facts, issue};
use crate::guard::{Decision, Invocation, stapel_root};
use crate::hash::normal_form;
use crate::identity::user_name;
use crate::ticket::{find, parse};
use crate::tickets::{Status, resolve};
use std::path::Path;

/// Permission modes in which Claude Code shows the dialog for an `ask` answer.
pub const DIALOG_MODES: [&str; 4] = ["default", "acceptEdits", "auto", "plan"];

pub fn ok_facts(p: &Prepared) -> Facts {
    Facts {
        key: p.key.clone(),
        action: "ok".into(),
        detail: p.section.clone(),
        hash: p.hash.clone(),
        dependencies: p
            .depends_on
            .iter()
            .map(|(id, hash)| format!("{id}={hash}"))
            .collect(),
    }
}

pub fn close_facts(key: &str, reason: &str) -> Facts {
    Facts {
        key: key.into(),
        action: "close".into(),
        detail: reason.into(),
        hash: String::new(),
        dependencies: Vec::new(),
    }
}

pub fn ask(inv: &Invocation, cwd: &Path, mode: Option<&str>) -> Decision {
    match prepare_ask(inv, cwd, mode) {
        Ok(decision) => decision,
        Err(reason) => Decision::Deny(reason),
    }
}

fn prepare_ask(inv: &Invocation, cwd: &Path, mode: Option<&str>) -> Result<Decision, String> {
    if crate::identity::in_agent_tool_shell() {
        return Err(format!(
            "this hook call comes from an agent's shell ({} is set), not from Claude Code's hook \
             runner; only Claude Code asks the person",
            crate::identity::TOOL_SHELL_MARKER
        ));
    }
    let mode = mode.unwrap_or("");
    if !DIALOG_MODES.contains(&mode) {
        return Err(format!(
            "stapel {} needs the permission dialog, which Claude Code does not show in permission \
             mode \"{mode}\"; switch to one of: {}",
            inv.action,
            DIALOG_MODES.join(", ")
        ));
    }
    let root = stapel_root(cwd).ok_or("not inside a stapel repository")?;
    let text = std::fs::read_to_string(root.join(".stapel/stapel.toml"))
        .map_err(|e| format!(".stapel/stapel.toml: {e}"))?;
    let config = Config::parse(&text).map_err(|e| format!(".stapel/stapel.toml: {e}"))?;
    user_name(&root)?;

    let (facts, reason, tail) = if inv.action == "ok" {
        let positional = only_positional(inv)?;
        let (key, section) = match positional.as_slice() {
            [section] => (None, section.as_str()),
            [key, section] => (Some(key.as_str()), section.as_str()),
            _ => return Err("usage: stapel ok [KEY] <section>".into()),
        };
        let p = prepare(&root, &config, key, section)?;
        let reason = format!(
            "stapel: confirm section `{}` of {} — {}…, {}, {}; runs: {}. Answer Yes or No; do \
             not choose \"don't ask again\".",
            p.section,
            p.key,
            &p.hash[..p.hash.len().min("sha256:".len() + 12)],
            lines(&p.text),
            against_head(&root, &config, &p),
            program(inv).join(" ")
        );
        let tail = format!("ok {} {}", quote(&p.key), quote(&p.section));
        (ok_facts(&p), reason, tail)
    } else {
        let (key, reason_text) = close_args(inv)?;
        let ticket = resolve(&root, key.as_deref())?;
        if !matches!(ticket.status, Status::Open(_)) {
            return Err(format!("ticket {} is not open", ticket.key));
        }
        let check = crate::checkstate::current(&root, &config, &ticket.dir);
        let reason = format!(
            "stapel: close ticket {} with the reason \"{reason_text}\"; its confirmations stop \
             permitting code writes; {check}; runs: {}. Answer Yes or No; do not choose \"don't \
             ask again\".",
            ticket.key,
            program(inv).join(" ")
        );
        let tail = format!(
            "close {} {}",
            quote(&ticket.key),
            quote(&format!("--reason={reason_text}"))
        );
        (close_facts(&ticket.key, &reason_text), reason, tail)
    };
    let command_without_grant = format!("{} {tail}", program(inv).join(" "));
    let with_placeholder = format!("{command_without_grant} --grant {}", "0".repeat(32));
    let rule =
        allow_rule(&root, &command_without_grant).or_else(|| allow_rule(&root, &with_placeholder));
    if let Some(rule) = rule {
        return Err(format!(
            "the permission allow rule \"{rule}\" would let stapel {} run without the dialog; \
             remove it from the Claude Code settings",
            inv.action
        ));
    }
    let token = issue(&root, &facts)?;
    Ok(Decision::Ask {
        reason,
        command: format!("{command_without_grant} --grant {token}"),
    })
}

fn only_positional(inv: &Invocation) -> Result<Vec<String>, String> {
    if let Some(flag) = inv.args.iter().find(|a| a.starts_with('-')) {
        return Err(format!(
            "unexpected option {flag} for stapel {}",
            inv.action
        ));
    }
    Ok(inv.args.clone())
}

fn close_args(inv: &Invocation) -> Result<(Option<String>, String), String> {
    let mut key = None;
    let mut reason = None;
    let mut it = inv.args.iter();
    while let Some(a) = it.next() {
        if a == "--reason" {
            reason = it.next().cloned();
        } else if let Some(v) = a.strip_prefix("--reason=") {
            reason = Some(v.to_string());
        } else if a.starts_with('-') {
            return Err(format!("unexpected option {a} for stapel close"));
        } else if key.is_none() {
            key = Some(a.clone());
        } else {
            return Err("usage: stapel close [KEY] --reason <text>".into());
        }
    }
    let reason = reason.map(|r| r.trim().to_string()).unwrap_or_default();
    if reason.is_empty() {
        return Err("a closing reason is required: --reason \"<why the ticket is done>\"".into());
    }
    Ok((key, reason))
}

fn lines(text: &str) -> String {
    match text.lines().count() {
        1 => "1 line".into(),
        n => format!("{n} lines"),
    }
}

/// `same as HEAD`, `differs from HEAD (+a -b lines)` or `not in HEAD`.
fn against_head(root: &Path, config: &Config, p: &Prepared) -> String {
    let rel = p
        .dir
        .strip_prefix(root)
        .map(|d| d.join("ticket.md"))
        .unwrap_or_default();
    let out = std::process::Command::new("git")
        .arg("show")
        .arg(format!("HEAD:{}", rel.display()))
        .current_dir(root)
        .output();
    let Some(out) = out.ok().filter(|o| o.status.success()) else {
        return "not in HEAD".into();
    };
    let head = String::from_utf8_lossy(&out.stdout).into_owned();
    let title = &config.section(&p.section).expect("validated").title;
    let Some(old) = find(&parse(&head), title).found().map(normal_form) else {
        return "not in HEAD".into();
    };
    if old == p.text {
        return "same as HEAD".into();
    }
    let diff = similar::TextDiff::from_lines(&old, &p.text);
    let (mut added, mut removed) = (0, 0);
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => added += 1,
            similar::ChangeTag::Delete => removed += 1,
            similar::ChangeTag::Equal => {}
        }
    }
    format!("differs from HEAD (+{added} -{removed} lines)")
}

/// A word for `sh`: unchanged when it is plain, else single-quoted.
fn quote(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:=@%+-".contains(c));
    if plain {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// The program words of the replaced command: the installed `stapel` (the build that ran these
/// checks), or the `cargo run ... --` form as written.
fn program(inv: &Invocation) -> Vec<String> {
    let first = inv.program.first().map(String::as_str).unwrap_or("stapel");
    if first.rsplit('/').next() == Some("stapel") {
        vec!["stapel".into()]
    } else {
        inv.program.iter().map(|w| quote(w)).collect()
    }
}

/// A Bash permission allow rule, in the project or user settings, that matches `command` and so
/// would let it run without the dialog. Rules follow Claude Code's forms: `Bash`, `Bash(*)`,
/// `Bash(prefix:*)`, and patterns with `*`.
fn allow_rule(root: &Path, command: &str) -> Option<String> {
    let mut files = vec![
        root.join(".claude/settings.json"),
        root.join(".claude/settings.local.json"),
    ];
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        let home = std::path::PathBuf::from(home);
        files.push(home.join(".claude/settings.json"));
        files.push(home.join(".claude/settings.local.json"));
    }
    for file in files {
        let Some(text) = crate::stage::read_bounded(&file) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        for rule in v["permissions"]["allow"].as_array().into_iter().flatten() {
            let Some(rule) = rule.as_str() else { continue };
            if bash_rule_matches(rule.trim(), command) {
                return Some(rule.to_string());
            }
        }
    }
    None
}

fn bash_rule_matches(rule: &str, command: &str) -> bool {
    if rule == "Bash" {
        return true;
    }
    let Some(pattern) = rule.strip_prefix("Bash(").and_then(|r| r.strip_suffix(')')) else {
        return false;
    };
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(":*") {
        return command == prefix || command.starts_with(&format!("{prefix} "));
    }
    glob(pattern, command)
}

/// `*` matches any run of characters; everything else matches itself.
fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !text.starts_with(first) || !text[first.len()..].ends_with(last) {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(i) => rest = &rest[i + part.len()..],
            None => return false,
        }
    }
    true
}
