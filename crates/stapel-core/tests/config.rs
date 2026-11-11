//! STP-1 AC-3: the starter `stapel.toml` written by `init` is readable by the core.

use stapel_core::config::{Config, default_toml, starter_toml};

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
        [
            "description",
            "spec",
            "design",
            "proof",
            "plan",
            "review",
            "summary"
        ]
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
        (
            "duplicate",
            base.replace("id = \"design\"", "id = \"SPEC\""),
        ),
        (
            "duplicate",
            base.replace("title = \"Design\"", "title = \"spec\""),
        ),
        (
            "Description",
            base.replace("title = \"Design\"", "title = \"Description\""),
        ),
        (
            "depends_on",
            base.replace("depends_on = [\"spec\"]", "depends_on = [\"nope\"]"),
        ),
        (
            "depends_on",
            base.replace("depends_on = [\"spec\"]", "depends_on = [\"design\"]"),
        ),
        ("{n}", base.replace("key = \"ABC-{n}\"", "key = \"ABC\"")),
        (
            "build.requires",
            base.replace(
                "requires = [\"description\", \"spec\", \"design\", \"proof\"]",
                "requires = []",
            ),
        ),
        (
            "build.requires",
            base.replace(
                "requires = [\"description\", \"spec\", \"design\", \"proof\"]",
                "requires = [\"nope\"]",
            ),
        ),
        (
            "build.requires",
            base.replace(
                "requires = [\"description\", \"spec\", \"design\", \"proof\"]",
                "requires = [\"review\"]",
            ),
        ),
    ];
    for (word, text) in cases {
        assert_ne!(text, base, "case for {word} did not change the config");
        let err = Config::parse(&text).expect_err(word);
        assert!(err.contains(word), "{word}: {err}");
    }
    // STP-25 AC-1: the title Description is accepted for the id description only.
    let config = Config::parse(&base).unwrap();
    assert!(config.section("description").is_some());
}

// STP-2 AC-13: without a [build] table the permit needs spec, design and proof.
#[test]
fn build_requires_defaults() {
    let base = default_toml("ABC");
    let start = base
        .find("[build]")
        .expect("starter config has a [build] table");
    let end = base[start..]
        .find("\n\n")
        .map(|i| start + i)
        .unwrap_or(base.len());
    let without = format!("{}{}", &base[..start], &base[end..]);
    let config = Config::parse(&without).unwrap();
    assert_eq!(config.build.requires, ["spec", "design", "proof"]);
    assert_eq!(
        Config::parse(&base).unwrap().build.requires,
        ["description", "spec", "design", "proof"]
    );
}

/// STP-4 AC-7: `[check]` has one runner, a bounded time limit and no other keys.
#[test]
fn check_section_is_validated() {
    let base = default_toml("ABC");
    assert!(Config::parse(&base).unwrap().check.is_none());

    let with = |body: &str| Config::parse(&format!("{base}\n[check]\n{body}\n"));
    let check = with("runner = \"cargo\"").unwrap().check.unwrap();
    assert_eq!(check.runner, "cargo");
    assert_eq!(check.timeout_secs, 900);
    assert_eq!(with("").unwrap().check.unwrap().runner, "cargo");
    for ok in ["10", "86400"] {
        let c = with(&format!("timeout_secs = {ok}"))
            .unwrap()
            .check
            .unwrap();
        assert_eq!(c.timeout_secs.to_string(), ok);
    }
    for bad in [
        "runner = \"npm\"",
        "timeout_secs = 9",
        "timeout_secs = 86401",
        "timeout_secs = 0",
        "timeout_secs = -1",
        "timeout_secs = \"900\"",
        "timeout_secs = 1.5",
        "command = \"cargo test\"",
    ] {
        assert!(with(bad).is_err(), "{bad} was accepted");
    }

    let cargo = starter_toml("ABC", true);
    assert_eq!(
        Config::parse(&cargo).unwrap().check.unwrap().runner,
        "cargo"
    );
    let other = starter_toml("ABC", false);
    assert!(Config::parse(&other).unwrap().check.is_none());
    assert!(other.lines().any(|l| l.starts_with("# [check]")), "{other}");
}

/// STP-6 AC-10: `[check] cargo` is an absolute path to an executable regular file outside the
/// repository folder (links resolved); a bad value fails the load and names the key.
#[test]
fn check_cargo_key_is_validated() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let repo = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = repo.path().canonicalize().unwrap();
    let out = outside.path().canonicalize().unwrap();
    let make = |path: &std::path::Path, mode: u32| {
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    };
    make(&out.join("cargo"), 0o755);
    make(&out.join("plain"), 0o644);
    make(&root.join("cargo"), 0o755);
    std::fs::create_dir(out.join("folder")).unwrap();
    symlink(root.join("cargo"), out.join("link-into-repo")).unwrap();
    symlink(out.join("cargo"), out.join("link-ok")).unwrap();

    let base = default_toml("ABC");
    let load = |value: &str| {
        let value = value.replace('\\', "\\\\").replace('"', "\\\"");
        Config::parse_at(&format!("{base}\n[check]\ncargo = \"{value}\"\n"), &root)
    };
    let path = |p: std::path::PathBuf| p.to_string_lossy().into_owned();

    for ok in [path(out.join("cargo")), path(out.join("link-ok"))] {
        let config = load(&ok).unwrap_or_else(|e| panic!("{ok} was refused: {e}"));
        assert_eq!(config.check.unwrap().cargo.as_deref(), Some(ok.as_str()));
    }
    // Without the key nothing changes.
    let none = Config::parse_at(&format!("{base}\n[check]\n"), &root).unwrap();
    assert_eq!(none.check.unwrap().cargo, None);

    for bad in [
        "cargo".to_string(),
        "./cargo".to_string(),
        "~/cargo".to_string(),
        String::new(),
        path(out.join("missing")),
        path(out.join("folder")),
        path(out.join("plain")),
        path(root.join("cargo")),
        path(out.join("link-into-repo")),
        path(
            root.join("..")
                .join(root.file_name().unwrap())
                .join("cargo"),
        ),
    ] {
        let e = load(&bad).expect_err(&format!("{bad:?} was accepted"));
        assert!(e.contains("check.cargo"), "{bad:?}: {e}");
    }
}

// STP-26 AC-3: a `[roles]` key or value of 1 to 64 bytes without control characters is accepted.
#[test]
fn rejects_bad_roles() {
    let base = default_toml("STP");
    // The starter file has the `[roles]` table (AC-12); the rows under test join its rows.
    let parse =
        |rows: &str| Config::parse(&base.replace("[roles]\n", &format!("[roles]\n{rows}\n")));
    let long = "k".repeat(65);
    for rows in [
        "\"\" = \"x\"".to_string(),
        "a = \"\"".to_string(),
        format!("\"{long}\" = \"x\""),
        "a = \"x\\ty\"".to_string(),
    ] {
        let e = parse(&rows).expect_err(&format!("{rows:?} was accepted"));
        assert!(
            e.contains("must be 1 to 64 bytes without control characters") && e.contains("roles:"),
            "{rows:?}: {e}"
        );
    }
    let ok = "k".repeat(64);
    parse(&format!("\"{ok}\" = \"{ok}\"")).expect("64 bytes are accepted");
}

fn with_check(rows: &str) -> Result<Config, String> {
    Config::parse(&format!("{}\n[check]\n{rows}\n", default_toml("STP")))
}

// STP-28 AC-4: the junit `[check]` keys and their defaults; a cargo `[check]` reads as before.
#[test]
fn junit_check_keys() {
    let minimal =
        with_check("runner = \"junit\"\ncommand = \"npx jest {file}\"\nreport = \"junit.xml\"")
            .unwrap();
    let check = minimal.check.unwrap();
    assert_eq!(check.runner, "junit");
    assert_eq!(check.command.as_deref(), Some("npx jest {file}"));
    assert_eq!(check.report.as_deref(), Some("junit.xml"));
    assert_eq!(check.timeout_secs, 900);
    assert_eq!(
        check.tests.as_deref().unwrap(),
        ["**/*.test.*", "**/*.spec.*", "**/__tests__/**"]
    );
    assert_eq!(
        check.inputs.as_deref().unwrap(),
        [
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
        ]
    );

    let full = with_check(
        "runner = \"junit\"\ncommand = \"jest {file}\"\nreport = \"out/r.xml\"\ntimeout_secs = 60\n\
         tests = [\"t/**\"]\ninputs = []",
    )
    .unwrap()
    .check
    .unwrap();
    assert_eq!(full.timeout_secs, 60);
    assert_eq!(full.tests.as_deref().unwrap(), ["t/**"]);
    assert_eq!(full.inputs.as_deref().unwrap(), &[] as &[String]);

    let cargo = with_check("runner = \"cargo\"\ntimeout_secs = 900")
        .unwrap()
        .check
        .unwrap();
    assert_eq!(
        (&cargo.command, &cargo.report, &cargo.tests, &cargo.inputs),
        (&None, &None, &None, &None)
    );
}

// STP-28 AC-5: a bad junit `[check]` fails the load with a message that names the key.
#[test]
fn rejects_bad_junit_keys() {
    let junit = |extra: &str| {
        let mut rows = String::from("runner = \"junit\"\n");
        for key in ["command", "report"] {
            if !extra.contains(&format!("{key} =")) {
                let v = if key == "command" {
                    "jest {file}"
                } else {
                    "r.xml"
                };
                rows.push_str(&format!("{key} = \"{v}\"\n"));
            }
        }
        with_check(&format!("{rows}{extra}"))
    };
    junit("").expect("the base case loads");

    for (extra, key) in [
        ("command = \"jest\"", "check.command"),
        ("command = \"jest {file} {file}\"", "check.command"),
        ("command = \"jest x{file}\"", "check.command"),
        ("command = \"jest {file}y\"", "check.command"),
        ("command = \"jest --x={file}\"", "check.command"),
        (
            "command = \"jest --testPathPattern {file}\"",
            "check.command",
        ),
        ("report = \"\"", "check.report"),
        ("report = \"/x\"", "check.report"),
        ("report = \"../x\"", "check.report"),
        ("tests = []", "check.tests"),
        ("tests = [\"a/../b\"]", "check.tests"),
        ("tests = [\"\"]", "check.tests"),
        ("tests = [\"/a\"]", "check.tests"),
        ("tests = [\"{a,b}\"]", "check.tests"),
        ("tests = [\"[ab]\"]", "check.tests"),
        ("tests = [\"!x\"]", "check.tests"),
        ("tests = [\"a\\\\b\"]", "check.tests"),
        ("inputs = [\"a/../b\"]", "check.inputs"),
        ("inputs = [\"x}\"]", "check.inputs"),
        ("cargo = \"/usr/bin/cargo\"", "check.cargo"),
    ] {
        let e = junit(extra).expect_err(&format!("{extra} was accepted"));
        assert!(e.contains(key), "{extra}: {e}");
    }
    let e = junit("command = \"jest --testPathPattern {file}\"").unwrap_err();
    assert!(e.contains("right after the program"), "{e}");

    for rows in [
        "runner = \"cargo\"\ncommand = \"jest {file}\"",
        "runner = \"cargo\"\nreport = \"r.xml\"",
        "runner = \"cargo\"\ntests = [\"a\"]",
        "runner = \"cargo\"\ninputs = [\"a\"]",
        "runner = \"cargo\"\ncargo = \"/usr/bin/cargo\"\ncommand = \"x {file}\"",
        "command = \"jest {file}\"",
    ] {
        let e = with_check(rows).expect_err(&format!("{rows:?} was accepted"));
        assert!(
            [
                "check.command",
                "check.report",
                "check.tests",
                "check.inputs"
            ]
            .iter()
            .any(|k| e.contains(k)),
            "{rows:?}: {e}"
        );
    }
    let e = with_check("runner = \"junit\"\ncommand = \"jest {file}\"\nreport = \"r.xml\"\ncargo = \"/usr/bin/cargo\"")
        .unwrap_err();
    assert!(e.contains("check.cargo"), "{e}");
    let e = with_check("runner = \"jest\"").unwrap_err();
    assert!(e.contains("check.runner"), "{e}");
    // The junit runner needs both keys.
    assert!(
        with_check("runner = \"junit\"\ncommand = \"jest {file}\"")
            .unwrap_err()
            .contains("check.report")
    );
    assert!(
        with_check("runner = \"junit\"\nreport = \"r.xml\"")
            .unwrap_err()
            .contains("check.command")
    );
}

// STP-28 AC-6: the glob matcher.
#[test]
fn globs_match_paths() {
    use stapel_core::config::glob_matches as m;
    assert!(m("**/package.json", "package.json"));
    assert!(m("**/package.json", "a/b/package.json"));
    assert!(!m("**/package.json", "a/xpackage.json"));
    assert!(!m("*.ts", "a/b.ts"));
    assert!(m("*.ts", "b.ts"));
    assert!(m("**/*.test.*", "a/b.test.ts"));
    assert!(m("**/__tests__/**", "__tests__/a.ts"));
    assert!(m("**/__tests__/**", "x/__tests__/y/z.json"));
    assert!(!m("**/__tests__/**", "x/__tests__"));
    assert!(!m("a/**", "a"));
    assert!(m("a/**", "a/b"));
    assert!(m("a/**", "a/b/c/d"));
    assert!(m("a/**/b", "a/b"));
    assert!(m("a/**/b", "a/x/y/b"));
    assert!(!m("a/**/b", "a/xb"));
    assert!(m("a?c", "abc"));
    assert!(!m("a?c", "a/c"));
    assert!(!m("a*c", "a/c"));
    assert!(m("tsconfig*.json", "tsconfig.json"));
    assert!(m("tsconfig*.json", "tsconfig.build.json"));
}
