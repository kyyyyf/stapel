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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub tickets: Tickets,
    pub sections: Vec<Section>,
    pub models: BTreeMap<String, ModelBinding>,
    pub guard: Guard,
    #[serde(default)]
    pub build: Build,
}

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
