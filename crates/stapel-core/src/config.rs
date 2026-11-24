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
        text.push_str("[check]\nrunner = \"cargo\"\ntimeout_secs = 3600\n");
        text.push_str(PROJECT_COMMENT);
    } else {
        text.push_str(
            "# [check]\n# runner = \"junit\"\n# command = \"npx jest {file} --ci --reporters=jest-junit\"\n# report = \"junit.xml\"\n# tests = [\"**/*.test.*\"]\n",
        );
        text.push_str(PROJECT_COMMENT);
    }
    text.push_str(PROCESS_COMMENT);
    text.push_str(GUARDS_COMMENT);
    text
}

const GUARDS_COMMENT: &str = "\n# [guards]\n# dir = \"guards\"   # a write into this folder asks the person\n#\n# [[guards.check]]\n# name = \"no-todo\"\n# command = \"grep -rn TODO src\"   # each output line is a violation\n# baseline = \"no-todo.txt\"   # read as <dir>/<baseline>; stapel check removes fixed lines\n";

const PROJECT_COMMENT: &str = "# project = [\"npx prettier --check {files}\"]   # shell commands run after the suite; {files} is the ticket's changed files\n";

const PROCESS_COMMENT: &str = "\n# [process]\n# dialogs = \"all\"   # \"close\": a light ticket asks for one stapel ok before the close, after the first\n\n# [worktree]\n# setup = \"...\"   # a shell command that `stapel new` runs in the new worktree\n";

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
    /// `[roles]`: agent type to token role (STP-26); absent means the default rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles: Option<BTreeMap<String, String>>,
    /// `[repository]` (STP-31 AC-4); absent means the shared mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<Repository>,
    /// `[process]` (STP-36 AC-7); absent means the level `all`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,
    /// `[worktree]` (STP-36 AC-10); absent means no setup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<Worktree>,
    /// `[guards]` (STP-41 AC-1): machine checks in a folder the agent cannot write unseen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guards: Option<Guards>,
}

/// `[guards]`: the folder of guard files and the guard entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guards {
    /// A relative folder of the repository outside `.stapel`, `.git` and `.claude`.
    pub dir: String,
    /// `[[guards.check]]`; absent means none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub check: Vec<GuardEntry>,
}

/// One `[[guards.check]]` entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardEntry {
    pub name: String,
    /// Run with `sh -c`.
    pub command: String,
    /// A relative path inside `dir`; absent means an exit-code guard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<String>,
}

/// A relative path: no empty, `.` or `..` part, no leading `/`, no backslash, no NUL.
fn is_relative_path(path: &str) -> bool {
    !path.contains(['\\', '\0']) && path.split('/').all(|p| !matches!(p, "" | "." | ".."))
}

/// `[worktree]`: what `stapel new` does in a new worktree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Worktree {
    /// A shell command run in the new worktree; absent means none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
}

/// `[process]`: how often the process turns to the person.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Process {
    /// The dialog level: `all` (the default) or `close`.
    #[serde(default = "default_dialogs")]
    pub dialogs: String,
}

fn default_dialogs() -> String {
    "all".to_string()
}

/// `[repository]`: where the process files live in git history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    pub history: History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum History {
    Local,
    Shared,
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
    /// The junit runner (STP-28 AC-4): the command template with one word `{file}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The junit runner: the XML report path, relative to the check copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
    /// The junit runner: test file globs; the default is filled in at load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests: Option<Vec<String>>,
    /// The junit runner: build input globs; the default is filled in at load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<String>>,
    /// The level of `stapel check` (STP-35 AC-7): `steps`, `head` or `full`; absent means `head`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// Where `stapel next` demands a check (STP-35 AC-13): `step`, `review`, `close`; absent means
    /// all three.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<String>>,
    /// The cargo runner (STP-35 AC-12): `--skip` filters for the local suite below `full`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suite_exclude: Option<Vec<String>>,
    /// Project commands (STP-37 AC-1): shell commands run in the check copy after the suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<Vec<String>>,
}

pub const DEFAULT_TEST_GLOBS: [&str; 3] = ["**/*.test.*", "**/*.spec.*", "**/__tests__/**"];
pub const DEFAULT_INPUT_GLOBS: [&str; 10] = [
    "**/package.json",
    "**/package-lock.json",
    "**/yarn.lock",
    "**/pnpm-lock.yaml",
    "**/jest.config.*",
    "**/tsconfig*.json",
    "**/babel.config.*",
    "**/.babelrc",
    "**/.swcrc",
    "**/jest.setup.*",
];

/// Whether the glob matches the repository path (STP-28 AC-6): `*` and `?` stay inside one part;
/// `**` as a part matches any parts, none included, but a trailing `/**` needs at least one.
pub fn glob_matches(glob: &str, path: &str) -> bool {
    let g: Vec<&str> = glob.split('/').collect();
    let p: Vec<&str> = path.split('/').collect();
    parts_match(&g, &p)
}

fn parts_match(g: &[&str], p: &[&str]) -> bool {
    match g.split_first() {
        None => p.is_empty(),
        Some((&"**", rest)) => {
            if rest.is_empty() {
                return !p.is_empty();
            }
            (0..=p.len()).any(|n| parts_match(rest, &p[n..]))
        }
        Some((head, rest)) => match p.split_first() {
            Some((part, tail)) => part_matches(head, part) && parts_match(rest, tail),
            None => false,
        },
    }
}

fn part_matches(glob: &str, part: &str) -> bool {
    let g: Vec<char> = glob.chars().collect();
    let s: Vec<char> = part.chars().collect();
    // reach[j]: the first i glob chars match the first j part chars.
    let mut reach = vec![false; s.len() + 1];
    reach[0] = true;
    for c in &g {
        let mut next = vec![false; s.len() + 1];
        if *c == '*' {
            let mut any = false;
            for j in 0..=s.len() {
                any |= reach[j];
                next[j] = any;
            }
        } else {
            for j in 1..=s.len() {
                next[j] = reach[j - 1] && (*c == '?' || *c == s[j - 1]);
            }
        }
        reach = next;
    }
    reach[s.len()]
}

fn validate_glob(key: &str, glob: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("check.{key} \"{glob}\" {why}"));
    if glob.is_empty() {
        return bad("is empty");
    }
    if glob.starts_with('/') {
        return bad("is absolute: use a path relative to the repository");
    }
    if glob.split('/').any(|part| part == "..") {
        return bad("holds ..");
    }
    if glob.contains(['{', '}', '[', ']', '!', '\\']) {
        return bad("holds one of { } [ ] ! \\, which are not supported: use * ? **");
    }
    Ok(())
}

fn validate_report(report: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("check.report \"{report}\" {why}"));
    if report.is_empty() {
        return bad("is empty");
    }
    if report.starts_with('/') {
        return bad("is absolute: use a path relative to the check copy");
    }
    if report.split('/').any(|part| part == "..") {
        return bad("holds ..");
    }
    Ok(())
}

fn validate_command(command: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("check.command \"{command}\" {why}"));
    let words: Vec<&str> = command.split(' ').filter(|w| !w.is_empty()).collect();
    let marks = command.matches("{file}").count();
    if marks == 0 {
        return bad("has no word {file}");
    }
    if marks > 1 {
        return bad("holds {file} twice");
    }
    let Some(at) = words.iter().position(|w| *w == "{file}") else {
        return bad("holds {file} inside a longer word: make it a word of its own");
    };
    if at == 0 {
        return bad("has no program before {file}");
    }
    if at > 1 && words[at - 1].starts_with('-') {
        return bad(
            "puts {file} after a word that starts with -: put {file} right after the program",
        );
    }
    Ok(())
}

fn default_runner() -> String {
    "cargo".into()
}

fn default_timeout() -> u64 {
    3600
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
    /// The `permissionMode` of the role's agent file; absent means the default of the role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
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
        let mut config: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        for entry in &config.guard.always_writable {
            validate_writable(entry)?;
        }
        config.validate()?;
        if let Some(check) = config.check.as_mut().filter(|c| c.runner == "junit") {
            let own = |globs: &[&str]| globs.iter().map(|g| g.to_string()).collect();
            check.tests.get_or_insert_with(|| own(&DEFAULT_TEST_GLOBS));
            check
                .inputs
                .get_or_insert_with(|| own(&DEFAULT_INPUT_GLOBS));
        }
        Ok(config)
    }

    /// Checks that sections, dependencies, the key pattern and `build.requires` fit together.
    fn validate(&self) -> Result<(), String> {
        if let Some(process) = &self.process {
            if process.dialogs != "all" && process.dialogs != "close" {
                return Err(format!(
                    "process.dialogs must be all or close: {}",
                    process.dialogs
                ));
            }
        }
        if let Some(guards) = &self.guards {
            let dir = guards.dir.strip_suffix('/').unwrap_or(&guards.dir);
            let first = dir.split('/').next().unwrap_or("").to_lowercase();
            if !is_relative_path(dir) || matches!(first.as_str(), ".stapel" | ".git" | ".claude") {
                return Err(format!(
                    "guards.dir must be a relative folder of the repository outside .stapel, .git and .claude: {}",
                    guards.dir
                ));
            }
            if self.check.is_none() {
                return Err("guards need [check]".into());
            }
            let mut names = std::collections::HashSet::new();
            for entry in &guards.check {
                if entry.name.trim().is_empty() || entry.command.trim().is_empty() {
                    return Err("guards.check entries need a name and a command".into());
                }
                if !names.insert(entry.name.as_str()) {
                    return Err(format!("guards.check name is used twice: {}", entry.name));
                }
                if let Some(base) = &entry.baseline {
                    if !is_relative_path(base) {
                        return Err(format!(
                            "guards.check baseline must be a relative path inside guards.dir: {base}"
                        ));
                    }
                }
            }
        }
        if let Some(setup) = self.worktree.as_ref().and_then(|w| w.setup.as_ref()) {
            if setup.trim().is_empty() {
                return Err("worktree.setup is empty: name a shell command".into());
            }
        }
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
            if section.title.trim().eq_ignore_ascii_case("description")
                && !section.id.eq_ignore_ascii_case("description")
            {
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
        if let Some(roles) = &self.roles {
            for (key, value) in roles {
                for text in [key, value] {
                    if text.is_empty() || text.len() > 64 || text.chars().any(char::is_control) {
                        return Err(format!(
                            "roles: {key} must be 1 to 64 bytes without control characters"
                        ));
                    }
                }
            }
        }
        if let Some(check) = &self.check {
            match check.runner.as_str() {
                "cargo" => {
                    for (key, set) in [
                        ("command", check.command.is_some()),
                        ("report", check.report.is_some()),
                        ("tests", check.tests.is_some()),
                        ("inputs", check.inputs.is_some()),
                    ] {
                        if set {
                            return Err(format!(
                                "check.{key} is a junit key: set runner = \"junit\" or remove it"
                            ));
                        }
                    }
                }
                "junit" => {
                    if check.cargo.is_some() {
                        return Err(
                            "check.cargo belongs to the cargo runner: remove it for junit".into(),
                        );
                    }
                    let Some(command) = &check.command else {
                        return Err("check.command is needed for the junit runner".into());
                    };
                    validate_command(command)?;
                    let Some(report) = &check.report else {
                        return Err("check.report is needed for the junit runner".into());
                    };
                    validate_report(report)?;
                    if check.tests.as_ref().is_some_and(|t| t.is_empty()) {
                        return Err("check.tests is empty: name at least one glob".into());
                    }
                    for (key, globs) in [("tests", &check.tests), ("inputs", &check.inputs)] {
                        for glob in globs.iter().flatten() {
                            validate_glob(key, glob)?;
                        }
                    }
                }
                other => {
                    return Err(format!(
                        "check.runner \"{other}\" is not supported: use cargo or junit"
                    ));
                }
            }
            if let Some(level) = &check.level {
                if !matches!(level.as_str(), "steps" | "head" | "full") {
                    return Err(format!("check.level must be steps, head or full: {level}"));
                }
            }
            if let Some(points) = &check.points {
                let known = points
                    .iter()
                    .all(|p| matches!(p.as_str(), "step" | "review" | "close"));
                if !known || !points.iter().any(|p| p == "close") {
                    return Err("check.points must hold close and only step, review, close".into());
                }
            }
            if let Some(exclude) = &check.suite_exclude {
                if check.runner == "junit" && !exclude.is_empty() {
                    return Err("check.suite_exclude needs the cargo runner".into());
                }
                if exclude.iter().any(|e| e.trim().is_empty()) {
                    return Err("check.suite_exclude has an empty entry".into());
                }
            }
            if let Some(project) = &check.project {
                if project.iter().any(|c| c.trim().is_empty()) {
                    return Err("check.project entries must not be empty".into());
                }
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
