//! The process definition stored in `.stapel/stapel.toml`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub fn parse(text: &str) -> Result<Config, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("config types always serialize")
    }
}
