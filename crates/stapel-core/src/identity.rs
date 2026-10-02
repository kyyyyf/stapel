//! Who is confirming, and whether the call comes from an agent's shell.

use std::path::Path;

/// Variables Claude Code sets in the shell it gives an agent; present with any value, empty
/// included, means an agent's shell.
pub const AGENT_VARIABLES: [&str; 2] = ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"];

pub fn in_agent_shell() -> bool {
    AGENT_VARIABLES
        .iter()
        .any(|v| std::env::var_os(v).is_some())
}

/// `git config user.name` read in `root`, trimmed; an error when unset or blank.
pub fn user_name(root: &Path) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(["config", "user.name"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || name.is_empty() {
        return Err(
            "git config user.name is not set; set it to the name confirmations are recorded under"
                .into(),
        );
    }
    Ok(name)
}
