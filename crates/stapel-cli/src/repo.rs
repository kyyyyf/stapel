//! The stapel repository a command runs in.

use stapel_core::config::Config;
use stapel_core::guard::stapel_root;
use std::path::PathBuf;

/// The repository root above the current folder and its validated `stapel.toml`.
pub fn open() -> Result<(PathBuf, Config), String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    // A `stapel.toml` that exists but is no regular file (a folder, a dangling link) is a broken
    // config with its load fault, not an absent one (STP-30 AC-10).
    let root = cwd
        .ancestors()
        .find(|dir| dir.join(".stapel/stapel.toml").symlink_metadata().is_ok())
        .map(std::path::Path::to_path_buf)
        .or_else(|| stapel_root(&cwd))
        .ok_or("no .stapel/stapel.toml here or above; run stapel init in the repository first")?;
    let config = load(&root)?;
    Ok((root, config))
}

/// The validated `stapel.toml` of the repository at `root`.
pub fn load(root: &std::path::Path) -> Result<Config, String> {
    let path = root.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Config::parse_at(&text, root).map_err(|e| format!(".stapel/stapel.toml: {e}"))
}
