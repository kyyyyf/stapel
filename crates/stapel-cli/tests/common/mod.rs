#![allow(dead_code)]

use assert_cmd::Command;
use std::path::Path;
use tempfile::TempDir;

/// A temporary directory that git does not see as part of any enclosing repository.
pub fn bare_dir() -> TempDir {
    tempfile::tempdir().unwrap()
}

/// A temporary directory with a fresh `git init`.
pub fn git_repo() -> TempDir {
    let dir = bare_dir();
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    git_config(dir.path(), "user.name", "test-user");
    dir
}

/// Sets a local git config value in the repository at `dir`.
pub fn git_config(dir: &Path, key: &str, value: &str) {
    let status = std::process::Command::new("git")
        .args(["config", key, value])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

/// `stapel` running in `dir`, with stdin piped so it is never a terminal.
pub fn stapel(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("stapel").unwrap();
    cmd.current_dir(dir)
        .env(
            "GIT_CEILING_DIRECTORIES",
            std::env::temp_dir().canonicalize().unwrap(),
        )
        .env_remove("STAPEL_ASSUME_TTY")
        .env_remove("CLAUDE_PROJECT_DIR")
        // The tests run the same inside and outside an agent's shell, and never see the
        // developer's global git identity.
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .env_remove("CLAUDE_CODE_EXECPATH")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .write_stdin("");
    cmd
}

pub fn read(dir: &Path, rel: &str) -> String {
    std::fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Every file under `dir` except `.git/`, with its content and modification time.
pub fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>, std::time::SystemTime)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>, std::time::SystemTime)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().display().to_string();
                let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
                out.push((rel, std::fs::read(&path).unwrap(), mtime));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// A git repository with `stapel init --prefix ABC` done.
pub fn stapel_repo() -> TempDir {
    let repo = git_repo();
    stapel(repo.path())
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    drop_description_section(repo.path());
    repo
}

/// Removes the template's `description` entry and its `requires` item from the `stapel.toml` that
/// `init` wrote, so the fixtures keep the old section list (STP-25 step 5).
fn drop_description_section(dir: &Path) {
    let path = dir.join(".stapel/stapel.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let entry = "[[sections]]\nid = \"description\"\ntitle = \"Description\"\nowner = \"product\"\n\n";
    let text = text
        .replacen(entry, "", 1)
        .replacen("[\"description\", \"spec\"", "[\"spec\"", 1);
    std::fs::write(&path, text).unwrap();
}

pub fn state_json(dir: &Path, key: &str) -> serde_json::Value {
    serde_json::from_str(&read(dir, &format!(".stapel/tickets/{key}/state.json"))).unwrap()
}

/// Replaces the body of the section titled `title` in a ticket's ticket.md.
pub fn set_section(dir: &Path, key: &str, title: &str, body: &str) {
    let path = dir.join(format!(".stapel/tickets/{key}/ticket.md"));
    let text = std::fs::read_to_string(&path).unwrap();
    let heading = format!("## {title}\n");
    let start = text
        .find(&heading)
        .unwrap_or_else(|| panic!("no {heading}"))
        + heading.len();
    let end = text[start..]
        .find("\n## ")
        .map(|i| start + i + 1)
        .unwrap_or(text.len());
    let new = format!(
        "{}\n{}\n\n{}",
        &text[..start],
        body.trim_end(),
        &text[end..]
    );
    std::fs::write(&path, new).unwrap();
}

/// A stapel repository with one ticket ABC-1 whose spec, design and proof have text.
pub fn repo_with_ticket() -> TempDir {
    let repo = stapel_repo();
    let dir = repo.path();
    stapel(dir).args(["new", "First"]).assert().success();
    set_section(dir, "ABC-1", "Spec", "The spec.");
    set_section(dir, "ABC-1", "Design", "The design.");
    set_section(dir, "ABC-1", "Proof", "The proof.");
    repo
}

/// Runs the PreToolUse hook on a Bash command with the given permission mode.
pub fn hook_bash(dir: &Path, command: &str, mode: &str) -> assert_cmd::assert::Assert {
    let payload = serde_json::json!({
        "tool_name": "Bash",
        "cwd": dir,
        "permission_mode": mode,
        "tool_input": { "command": command, "description": "test" },
    });
    stapel(dir)
        .args(["hook", "pre-tool-use"])
        .write_stdin(payload.to_string())
        .assert()
}

/// The hook's answer for a command it asks about: (reason, replaced command).
pub fn ask(dir: &Path, command: &str) -> (String, String) {
    let out = hook_bash(dir, command, "auto")
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap_or_else(|e| {
        panic!(
            "no JSON answer for {command:?}: {e}: {}",
            String::from_utf8_lossy(&out)
        )
    });
    let h = &v["hookSpecificOutput"];
    assert_eq!(h["hookEventName"], "PreToolUse");
    assert_eq!(h["permissionDecision"], "ask", "{v}");
    (
        h["permissionDecisionReason"].as_str().unwrap().to_string(),
        h["updatedInput"]["command"].as_str().unwrap().to_string(),
    )
}

/// The token after `--grant` in a replaced command.
pub fn token_of(command: &str) -> String {
    command
        .split_whitespace()
        .skip_while(|w| *w != "--grant")
        .nth(1)
        .unwrap_or_else(|| panic!("no --grant in {command}"))
        .to_string()
}

/// Runs git in `dir` without the global configuration and returns its stdout.
pub fn git_in(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Writes `spec` confirmation time `at` into the `state.json` of `key` in `dir`; none removes every
/// confirmation.
pub fn set_spec_time(dir: &Path, key: &str, at: Option<&str>) {
    let path = dir.join(format!(".stapel/tickets/{key}/state.json"));
    let mut state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    state["confirmations"] = match at {
        Some(at) => serde_json::json!([{
            "section": "spec",
            "by": "test-user",
            "at": at,
            "hash": "h",
            "normal_form": 1,
        }]),
        None => serde_json::json!([]),
    };
    std::fs::write(&path, state.to_string()).unwrap();
}

/// Two ticket worktrees of one repository (STP-27): `a` is ABC-1, `b` is ABC-2.
pub struct Pair {
    pub repo: TempDir,
    pub a: std::path::PathBuf,
    pub b: std::path::PathBuf,
}

/// A repository with one commit (`src/shared.txt`, `src/main.rs`, `Cargo.lock`, a ledger with union
/// merge), two ticket worktrees ABC-1 and ABC-2 made by `stapel new`, and spec confirmations at
/// 10:00 and 11:00 on 2026-01-01.
pub fn two_ticket_worktrees() -> Pair {
    let repo = stapel_repo();
    let dir = repo.path();
    let put = |rel: &str, text: String| {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    let lines = |n: u32| (1..=n).map(|i| format!("line {i}\n")).collect::<String>();
    put("src/shared.txt", lines(10));
    put("src/main.rs", lines(20));
    put(
        "Cargo.lock",
        "# lock\nname = \"x\"\nversion = \"1\"\nchecksum = \"c\"\n".to_string(),
    );
    put(".stapel/process-ledger.jsonl", "{\"n\":0}\n".to_string());
    git_in(dir, &["config", "user.email", "test-user.invalid"]);
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "-m", "base"]);
    stapel(dir).args(["new", "First"]).assert().success();
    stapel(dir).args(["new", "Second"]).assert().success();
    let a = dir.join(".claude/worktrees/ABC-1");
    let b = dir.join(".claude/worktrees/ABC-2");
    set_spec_time(&a, "ABC-1", Some("2026-01-01T10:00:00Z"));
    set_spec_time(&b, "ABC-2", Some("2026-01-01T11:00:00Z"));
    Pair { repo, a, b }
}

/// A junit repository (STP-28): the repository `repo` with ticket ABC-1 and `[check] runner =
/// "junit"`, and the folder `tools` that holds the test stand-in and its log.
pub struct JunitRepo {
    pub repo: TempDir,
    pub tools: TempDir,
}

impl JunitRepo {
    pub fn dir(&self) -> &Path {
        self.repo.path()
    }

    /// The lines `<commit> <test file>` of the stand-in's runs, oldest first.
    pub fn runs(&self) -> Vec<String> {
        std::fs::read_to_string(self.tools.path().join("runs.log"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }
}

/// The awk program of the stand-in: one test case per `it(` or `test(` line with a double-quoted
/// title, named with a leading space as jest-junit names a top-level test. `.skip` is skipped; a
/// line with `needs <path>` fails while that path is missing in the copy.
const STAND_IN_AWK: &str = r#"
BEGIN { out = "junit.xml"; print "<?xml version=\"1.0\"?>\n<testsuites>\n<testsuite name=\"s\">" > out }
/(^|[^.A-Za-z_])(it|test)(\.skip)?\("/ {
  line = $0
  skip = (line ~ /\.skip\(/)
  match(line, /\("[^"]*"/)
  title = substr(line, RSTART + 2, RLENGTH - 3)
  failed = 0
  if (match(line, /needs [^ ]+/)) {
    path = substr(line, RSTART + 6, RLENGTH - 6)
    if (system("test -e " path) != 0) failed = 1
  }
  body = ""
  if (skip) body = "<skipped/>"
  else if (failed) { body = "<failure message=\"missing\"/>"; bad = 1 }
  printf "<testcase classname=\"\" name=\" %s\">%s</testcase>\n", title, body > out
}
END { print "</testsuite>\n</testsuites>" > out; exit bad }
"#;

/// A git repository with `stapel init --prefix ABC`, ticket ABC-1, a gitignored `node_modules`
/// folder and `[check] runner = "junit"` whose command is a shell stand-in for jest kept outside the
/// repository. The stand-in needs one argument (a Per-file run) or none (the suite run over every
/// `__tests__/*.test.ts`, not logged), `PATH`, `HOME`,
/// `JEST_JUNIT_REPORT_TEST_SUITE_ERRORS=true` and a `node_modules` link with `marker` inside;
/// else it leaves no report. Everything is committed; the base test file is `__tests__/base.test.ts`.
pub fn junit_repo() -> JunitRepo {
    let repo = git_repo();
    let tools = bare_dir();
    let dir = repo.path();
    git_config(dir, "user.email", "test-user.invalid");
    let put = |rel: &str, text: &str| {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    put(".gitignore", "node_modules/\n");
    put("src/index.ts", "export const one = 1;\n");
    put(
        "__tests__/base.test.ts",
        "const helper = 1;\nit(\"base\", () => {});\nit(\"keeps\", () => { /* needs src/index.ts */ });\n",
    );
    put("node_modules/marker", "x\n");
    let script = tools.path().join("stand-in.sh");
    let log = tools.path().join("runs.log");
    std::fs::write(
        &script,
        format!(
            "[ \"$#\" -le 1 ] || exit 5\n\
             [ -n \"$PATH\" ] && [ -n \"$HOME\" ] && [ \"$JEST_JUNIT_REPORT_TEST_SUITE_ERRORS\" = true ] || exit 3\n\
             [ -L node_modules ] && [ -e node_modules/marker ] || exit 4\n\
             if [ \"$#\" = 0 ]; then set -- __tests__/*.test.ts; exec awk '{1}' \"$@\"; fi\n\
             echo \"$(git rev-parse HEAD) $1\" >> '{0}'\n\
             exec awk '{1}' \"$1\"\n",
            log.display(),
            STAND_IN_AWK
        ),
    )
    .unwrap();
    stapel(dir)
        .args(["init", "--prefix", "ABC"])
        .assert()
        .success();
    stapel(dir).args(["new", "First"]).assert().success();
    let path = dir.join(".stapel/stapel.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&format!(
        "\n[check]\nrunner = \"junit\"\ncommand = \"sh {} {{file}}\"\nreport = \"junit.xml\"\ntimeout_secs = 60\n",
        script.display()
    ));
    std::fs::write(&path, text).unwrap();
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-q", "-m", "initial"]);
    JunitRepo { repo, tools }
}

/// The grants folder inside the git directory of `dir`.
pub fn grants_dir(dir: &Path) -> std::path::PathBuf {
    dir.join(".git/stapel/grants")
}
