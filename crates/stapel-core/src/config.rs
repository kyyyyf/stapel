//! The process definition stored in `.stapel/stapel.toml`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path};

const TEMPLATE: &str = include_str!("stapel.toml");
const PREFIX_PLACEHOLDER: &str = "@PREFIX@";

/// The starter `stapel.toml` with ticket keys of the form `<prefix>-<n>`.
pub fn default_toml(prefix: &str) -> String {
    TEMPLATE.replace(PREFIX_PLACEHOLDER, prefix)
}

const CHECK_COMMENT: &str = "\n# RED to GREEN check of ticket steps (STP-4): `stapel check` runs each step's tests at its\n# RED and GREEN commits and at HEAD; with this section, `stapel close` needs a passing check.\n";

/// The starter `stapel.toml` for `init`: with `[check]` in a cargo repository, with a commented
/// example otherwise (STP-4 AC-7).
pub fn starter_toml(prefix: &str, cargo: bool) -> String {
    let mut text = default_toml(prefix);
    text.push_str(CHECK_COMMENT);
    if cargo {
        text.push_str("[check]\nrunner = \"cargo\"\ntimeout_secs = 900\n");
    } else {
        text.push_str("# [check]\n# runner = \"cargo\"\n# timeout_secs = 900\n");
    }
    text
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub tickets: Tickets,
    pub sections: Vec<Section>,
    pub models: BTreeMap<String, ModelBinding>,
    pub guard: Guard,
    #[serde(default)]
    pub build: Build,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<Check>,
}

/// `[check]`: how `stapel check` runs a ticket's tests (STP-4 AC-7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    #[serde(default = "default_runner")]
    pub runner: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// The cargo program (STP-6 AC-9): an absolute path outside the repository; absent means the
    /// first `cargo` in an absolute `PATH` entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo: Option<String>,
}

fn default_runner() -> String {
    "cargo".into()
}

fn default_timeout() -> u64 {
    900
}

pub const TIMEOUT_RANGE: std::ops::RangeInclusive<u64> = 10..=86_400;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Build {
    /// Sections whose fresh confirmations together permit code writes.
    #[serde(default = "default_requires")]
    pub requires: Vec<String>,
}

impl Default for Build {
    fn default() -> Self {
        Build {
            requires: default_requires(),
        }
    }
}

fn default_requires() -> Vec<String> {
    ["spec", "design", "proof"].map(String::from).to_vec()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tickets {
    /// Key pattern for new tickets; `{n}` is replaced by the next number.
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub title: String,
    pub owner: String,
    /// Sections whose change makes a confirmation of this one stale.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelBinding {
    pub provider: String,
    pub model: String,
    /// When false, a missing key or subscription is reported and the role is skipped.
    #[serde(default = "required_by_default")]
    pub required: bool,
}

fn required_by_default() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guard {
    /// Path prefixes, relative to the repository root, that agents may write without a build permit.
    pub always_writable: Vec<String>,
}

impl Config {
    /// Like `parse`, and `[check] cargo` is tested against the repository folder `root`
    /// (STP-6 AC-10): an absolute path to an executable regular file that, with links resolved,
    /// lies outside `root`.
    pub fn parse_at(text: &str, root: &std::path::Path) -> Result<Config, String> {
        let config = Config::parse(text)?;
        if let Some(cargo) = config.check.as_ref().and_then(|c| c.cargo.as_deref()) {
            validate_cargo(cargo, root)?;
        }
        Ok(config)
    }

    pub fn parse(text: &str) -> Result<Config, String> {
        let config: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        for entry in &config.guard.always_writable {
            validate_writable(entry)?;
        }
        config.validate()?;
        Ok(config)
    }

    /// Checks that sections, dependencies, the key pattern and `build.requires` fit together.
    fn validate(&self) -> Result<(), String> {
        if !self.tickets.key.contains("{n}") {
            return Err(format!(
                "tickets.key \"{}\" has no {{n}} for the ticket number",
                self.tickets.key
            ));
        }
        let mut ids = std::collections::HashSet::new();
        let mut titles = std::collections::HashSet::new();
        for section in &self.sections {
            if !ids.insert(section.id.to_lowercase()) {
                return Err(format!("sections: duplicate id \"{}\"", section.id));
            }
            if section.title.trim().eq_ignore_ascii_case("description") {
                return Err(
                    "sections: the title Description is reserved for the ticket description".into(),
                );
            }
            if !titles.insert(section.title.trim().to_lowercase()) {
                return Err(format!("sections: duplicate title \"{}\"", section.title));
            }
        }
        for section in &self.sections {
            for dep in &section.depends_on {
                if dep.eq_ignore_ascii_case(&section.id) {
                    return Err(format!("sections: {} depends_on itself", section.id));
                }
                if self.section(dep).is_none() {
                    return Err(format!(
                        "sections: {} depends_on unknown section \"{dep}\"",
                        section.id
                    ));
                }
            }
        }
        if self.build.requires.is_empty() {
            return Err("build.requires is empty: name the sections the build needs".into());
        }
        for id in &self.build.requires {
            match self.section(id) {
                None => return Err(format!("build.requires names unknown section \"{id}\"")),
                Some(s) if s.owner == "generated" => {
                    return Err(format!(
                        "build.requires names \"{id}\", which is generated and cannot be confirmed"
                    ));
                }
                Some(_) => {}
            }
        }
        if let Some(check) = &self.check {
            if check.runner != "cargo" {
                return Err(format!(
                    "check.runner \"{}\" is not supported: the only runner is cargo",
                    check.runner
                ));
            }
            if !TIMEOUT_RANGE.contains(&check.timeout_secs) {
                return Err(format!(
                    "check.timeout_secs {} is out of range: use {} to {} seconds",
                    check.timeout_secs,
                    TIMEOUT_RANGE.start(),
                    TIMEOUT_RANGE.end()
                ));
            }
        }
        Ok(())
    }

    /// The section with this id, compared without case.
    pub fn section(&self, id: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.id.eq_ignore_ascii_case(id))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("config types always serialize")
    }
}

/// A ticket key prefix is 2 to 8 uppercase Latin letters.
pub fn validate_prefix(prefix: &str) -> Result<(), String> {
    let ok = (2..=8).contains(&prefix.len()) && prefix.bytes().all(|b| b.is_ascii_uppercase());
    if ok {
        Ok(())
    } else {
        Err(format!(
            "ticket key prefix \"{prefix}\" is not valid: use 2 to 8 uppercase Latin letters, e.g. STP"
        ))
    }
}

/// An `always_writable` entry is a relative path inside the repository, such as `docs/`.
fn validate_writable(entry: &str) -> Result<(), String> {
    let path = Path::new(entry.trim_end_matches('/'));
    let inside = !entry.starts_with('/')
        && path.components().next().is_some()
        && path.components().all(|c| matches!(c, Component::Normal(_)));
    if inside {
        Ok(())
    } else {
        Err(format!(
            "guard.always_writable: \"{entry}\" is not valid: use a relative path inside the \
             repository without . and .., e.g. docs/"
        ))
    }
}

/// `[check] cargo`: an absolute path to an executable regular file outside `root`.
/// Tests `[check] cargo` against the repository folder `root`; `check` runs it again before the
/// first run, since a link may change after the load.
pub fn validate_cargo(cargo: &str, root: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let bad = |why: &str| Err(format!("check.cargo \"{cargo}\" {why}"));
    let path = std::path::Path::new(cargo);
    if !path.is_absolute() {
        return bad("is not an absolute path");
    }
    let Ok(real) = path.canonicalize() else {
        return bad("does not exist");
    };
    let Ok(meta) = std::fs::metadata(&real) else {
        return bad("cannot be read");
    };
    if !meta.is_file() {
        return bad("is not a regular file");
    }
    if meta.permissions().mode() & 0o111 == 0 {
        return bad("is not executable");
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if real.starts_with(&root) {
        return bad("lies inside the repository");
    }
    Ok(())
}

/// A `cargo` found on `PATH` must not lie under the repository folder once links are resolved
/// (STP-6 AC-9).
pub fn validate_path_cargo(cargo: &std::path::Path, root: &std::path::Path) -> Result<(), String> {
    let real = cargo.canonicalize().unwrap_or_else(|_| cargo.to_path_buf());
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if real.starts_with(&root) {
        return Err(format!(
            "cargo {} found on PATH lies inside the repository",
            cargo.display()
        ));
    }
    Ok(())
}
