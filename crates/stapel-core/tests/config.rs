//! STP-1 AC-3: the starter `stapel.toml` written by `init` is readable by the core.

use stapel_core::config::{Config, default_toml};

#[test]
fn default_config_roundtrips() {
    let text = default_toml("ABC");
    let config = Config::parse(&text).expect("starter config parses");
    assert_eq!(config.tickets.key, "ABC-{n}");

    let again = Config::parse(&config.to_toml()).expect("serialized config parses");
    assert_eq!(again, config);
}

#[test]
fn default_config_has_roles_and_sections() {
    let config = Config::parse(&default_toml("STP")).unwrap();

    let sections: Vec<&str> = config.sections.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        sections,
        ["spec", "design", "proof", "plan", "review", "summary"]
    );

    let roles: Vec<&str> = config.models.keys().map(String::as_str).collect();
    let mut expected = [
        "author", "builder", "reviewer", "external", "drift", "cheap",
    ];
    expected.sort();
    assert_eq!(roles, expected);

    let external = &config.models["external"];
    assert_eq!(external.model, "claude-fable-5-1");
    assert!(!external.required);
    assert!(config.models["builder"].required);
    assert_eq!(config.models["author"].model, "claude-opus-5-5");

    for path in [".stapel/", "docs/"] {
        assert!(
            config.guard.always_writable.iter().any(|p| p == path),
            "{path} must be always writable"
        );
    }
}

#[test]
fn config_rejects_missing_ticket_key() {
    let text = default_toml("ABC").replace("key = \"ABC-{n}\"", "");
    let err = Config::parse(&text).unwrap_err();
    assert!(err.to_string().contains("key"), "{err}");
}

// E-8: an always_writable entry that would cover the whole repository or leave it is refused.
#[test]
fn config_rejects_bad_always_writable() {
    for bad in ["", "/", "/etc/", "../x/", "docs/../", "./"] {
        let text = default_toml("ABC").replace(
            "always_writable = [\".stapel/\", \"docs/\"]",
            &format!("always_writable = [\".stapel/\", {bad:?}]"),
        );
        let err = Config::parse(&text).expect_err(bad);
        assert!(
            err.to_string().contains("always_writable"),
            "{bad:?}: {err}"
        );
    }
}

// STP-2 AC-13: sections, dependencies, the key pattern and build.requires are validated.
#[test]
fn rejects_bad_sections() {
    let base = default_toml("ABC");
    let cases: [(&str, String); 9] = [
        ("duplicate", base.replace("id = \"design\"", "id = \"SPEC\"")),
        ("duplicate", base.replace("title = \"Design\"", "title = \"spec\"")),
        ("Description", base.replace("title = \"Design\"", "title = \"Description\"")),
        ("depends_on", base.replace("depends_on = [\"spec\"]", "depends_on = [\"nope\"]")),
        ("depends_on", base.replace("depends_on = [\"spec\"]", "depends_on = [\"design\"]")),
        ("{n}", base.replace("key = \"ABC-{n}\"", "key = \"ABC\"")),
        ("build.requires", base.replace("requires = [\"spec\", \"design\", \"proof\"]", "requires = []")),
        ("build.requires", base.replace("requires = [\"spec\", \"design\", \"proof\"]", "requires = [\"nope\"]")),
        ("build.requires", base.replace("requires = [\"spec\", \"design\", \"proof\"]", "requires = [\"review\"]")),
    ];
    for (word, text) in cases {
        assert_ne!(text, base, "case for {word} did not change the config");
        let err = Config::parse(&text).expect_err(word);
        assert!(err.contains(word), "{word}: {err}");
    }
}

// STP-2 AC-13: without a [build] table the permit needs spec, design and proof.
#[test]
fn build_requires_defaults() {
    let base = default_toml("ABC");
    let start = base.find("[build]").expect("starter config has a [build] table");
    let end = base[start..].find("\n\n").map(|i| start + i).unwrap_or(base.len());
    let without = format!("{}{}", &base[..start], &base[end..]);
    let config = Config::parse(&without).unwrap();
    assert_eq!(config.build.requires, ["spec", "design", "proof"]);
    assert_eq!(Config::parse(&base).unwrap().build.requires, ["spec", "design", "proof"]);
}
