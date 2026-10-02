//! One-time grants that carry a person's "yes" from the Claude Code permission dialog to `ok` and
//! `close` (STP-2 AC-16).
//!
//! The guard issues a grant when it asks; the replaced command carries the token. The grant file's
//! name is a hash of the token and of everything the person was asked about, so a file cannot be
//! reused for other content and rewriting its content changes nothing. Files live in the git
//! directory, which git never tracks and the write tools may not touch.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Seconds a grant stays valid after the dialog appears.
pub const LIFETIME: u64 = 30 * 60;

/// What the person is asked to approve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub key: String,
    /// `ok` or `close`.
    pub action: String,
    /// The section id for `ok`, the reason for `close`.
    pub detail: String,
    /// The section hash for `ok`, empty for `close`.
    pub hash: String,
    /// `id=hash` of each dependency, sorted.
    pub dependencies: Vec<String>,
}

pub fn grants_dir(root: &Path) -> Result<PathBuf, String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "stapel/grants"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err("git did not report the repository's git directory".into());
    }
    let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim_end());
    Ok(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

fn file_name(token: &str, facts: &Facts) -> String {
    let mut h = Sha256::new();
    for part in [
        token,
        &facts.key,
        &facts.action,
        &facts.detail,
        &facts.hash,
        &facts.dependencies.join(","),
        env!("CARGO_PKG_VERSION"),
    ] {
        h.update(part.as_bytes());
        h.update([0u8]);
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    format!("{hex}.json")
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn expires_at(path: &Path) -> Option<u64> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_file() || meta.len() > 4096 {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    v["expires_at"].as_u64()
}

/// Removes grants that expired or cannot be read.
fn sweep(dir: &Path) {
    let now = now();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if expires_at(&path).is_none_or(|t| t <= now) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Writes a grant for `facts` and returns its token.
pub fn issue(root: &Path, facts: &Facts) -> Result<String, String> {
    let dir = grants_dir(root)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    sweep(&dir);
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("no randomness for a grant: {e}"))?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let content = serde_json::json!({ "expires_at": now() + LIFETIME }).to_string();
    let path = dir.join(file_name(&token, facts));
    std::fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(token)
}

/// Checks and removes the grant for `token` and `facts`; it is removed whatever the outcome.
pub fn consume(root: &Path, token: &str, facts: &Facts) -> Result<(), String> {
    let path = grants_dir(root)?.join(file_name(token, facts));
    if std::fs::symlink_metadata(&path).is_err() {
        return Err(
            "the grant does not match: the section, a dependency or the stapel build changed \
             since the dialog, or the grant was used already; ask the agent to run the command \
             again"
                .into(),
        );
    }
    let expires = expires_at(&path);
    let _ = std::fs::remove_file(&path);
    sweep(path.parent().expect("grants dir"));
    match expires {
        Some(t) if t > now() => Ok(()),
        _ => Err("the grant expired; ask the agent to run the command again".into()),
    }
}
