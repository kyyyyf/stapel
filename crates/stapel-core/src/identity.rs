//! Who is confirming, and whether the call comes from an agent's shell.

use std::path::Path;

/// Variables Claude Code sets in the shell it gives an agent; present with any value, empty
/// included, means an agent's shell.
pub const AGENT_VARIABLES: [&str; 2] = ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"];

/// Set in the shell Claude Code gives the agent's Bash tool and absent in the processes Claude
/// Code starts for hooks (observed 2026-10-02, STP-2 code review round 2). A hook process that
/// carries it was started by the agent, not by Claude Code.
pub const TOOL_SHELL_MARKER: &str = "CLAUDE_CODE_EXECPATH";

pub fn in_agent_tool_shell() -> bool {
    std::env::var_os(TOOL_SHELL_MARKER).is_some()
}

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
