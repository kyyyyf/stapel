//! The git commit the binary was built from, named by `stapel --version`.

#[derive(Debug, PartialEq, Eq)]
pub enum BuildCommit {
    /// 40 or 64 lowercase hex digits.
    Clean(String),
    /// Such digits followed by `-dirty`.
    Dirty(String),
    Missing,
}

pub fn parse(value: Option<&str>) -> BuildCommit {
    let Some(value) = value else {
        return BuildCommit::Missing;
    };
    let is_sha = |s: &str| {
        (s.len() == 40 || s.len() == 64)
            && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    };
    if is_sha(value) {
        BuildCommit::Clean(value.to_string())
    } else if value.strip_suffix("-dirty").is_some_and(is_sha) {
        BuildCommit::Dirty(value.to_string())
    } else {
        BuildCommit::Missing
    }
}

pub fn build_commit() -> BuildCommit {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var("STAPEL_TEST_BUILD_COMMIT") {
        return parse(Some(&value));
    }
    parse(option_env!("STAPEL_BUILD_COMMIT"))
}

/// The text after `stapel ` in `stapel --version`.
pub fn version_text() -> &'static str {
    let version = env!("CARGO_PKG_VERSION");
    let text = match build_commit() {
        BuildCommit::Clean(v) | BuildCommit::Dirty(v) => format!("{version} ({v})"),
        BuildCommit::Missing => version.to_string(),
    };
    Box::leak(text.into_boxed_str())
}
