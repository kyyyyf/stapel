//! The stapel repository a command runs in.

use stapel_core::config::Config;
use stapel_core::guard::stapel_root;
use std::path::PathBuf;

/// The repository root above the current folder and its validated `stapel.toml`.
pub fn open() -> Result<(PathBuf, Config), String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let root = stapel_root(&cwd)
        .ok_or("no .stapel/stapel.toml here or above; run stapel init in the repository first")?;
    let path = root.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let config = Config::parse_at(&text, &root).map_err(|e| format!(".stapel/stapel.toml: {e}"))?;
    Ok((root, config))
}
