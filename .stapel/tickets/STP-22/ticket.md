# STP-22 — stapel init installs the process: skill, agent definitions, hooks

## Description

Source: `docs/PHASES.md`, plan item 1.3; the ledger decision of 2026-10-21 (approval dialogs, part 2: rule 5
of the spec process moves into the skill text).

### Frame

**Problem.** The process lives in this repository's `CLAUDE.md` and in hand-written briefs, so a foreign
repository gets the guard from `stapel init` but no process. `init` installs it as data.

**Plan item:** 1.3.

**Risk tags:** `interface` (files Claude Code reads), `guard` (agent tools and permission modes),
`migration` (a re-run updates installed files and keeps hand edits).

**Non-goals.** Items 1.4, 1.5, 1.8, 1.9, 1.10; `--force` or `.new` files; a plugin; `~/.claude`;
removing files; agents other than Claude Code; making the model follow the text.

**Promise (closed list).**

1. `init` writes the skill `.claude/skills/stapel/SKILL.md` and the agents `stapel-builder`,
   `stapel-drift`, `stapel-reviewer`, `stapel-spec-reviewer` under `.claude/agents/`, each with a marker
   `# stapel: <version> sha256:<hash>` in its frontmatter.
2. Agents get `model` from `[models]`, `permissionMode` from a default or `permission_mode` (bypass and
   unknown values refuse, exit 1); reviewers are read-only, the builder has Edit, Write and Bash.
3. A re-run creates missing files, updates stapel's unchanged ones, keeps edited, unmarked or newer ones
   with a warning.
4. A SessionStart hook names the skill and prints `stapel next`; it is silent when it cannot run.
5. The skill holds the general process of `CLAUDE.md` with rule 5 as changed on 2026-10-21, the `next`
   loop and the brief rule; each agent body holds its role's rules.
6. This repository gets the files, and `CLAUDE.md` keeps only its own rules (the human applies it).

Not promised: CRLF files; hiding the marker from the model; merging a person's `stapel*` file;
deliberate circumvention.

**Prior art.** The builder briefs and reviewer prompts of STP-5 to STP-21 (`.stapel/scratch/stp5-r1-brief.md`,
the agent calls in the transcripts); `init.rs` (`plan_hooks`, `plan_create`, the upgrade of the bare hook
line); the local SessionStart hook of this repository (`.claude/settings.local.json`); the Claude Code
documentation of subagents and skills (fetched 2026-10-21: `model` takes a full id, unknown frontmatter
fields are ignored, a subagent cannot ask the person, files are picked up without a restart).

**Size.** Estimated about 350 changed source lines (`crates/stapel-cli`: `init.rs`, the session-start
hook; `crates/stapel-core`: `permission_mode` in the model binding), about 250 lines of skill and agent
text as files under `crates/stapel-cli/`, and about 700 test lines. Past about 600 source lines the ticket
is split before the spec review. Route: full (tags), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-22: Decision (marker, the human, recommended): a YAML comment line `# stapel: <version>
  sha256:<hex>` in the frontmatter, the hash over the file without that line; a matching hash means
  stapel's unchanged file, which a re-run updates; a mismatch or no marker means the person's file, which
  is kept. Rejected: a version alone (a hand edit is lost on update); never overwrite (a new stapel never
  updates its files).
- 2026-10-22: Decision (loading the skill, the human, recommended): a SessionStart hook prints one line
  that names the skill and `stapel next`, because Claude Code cannot be made to load a skill and `init`
  does not touch the person's `CLAUDE.md`. Rejected: a marked block in `CLAUDE.md`; the skill description
  alone.
- 2026-10-22: Decision (model and permission mode, the human, recommended): `model` from the role in
  `[models]`, the spec reviewer on `reviewer`; `permissionMode` by default `acceptEdits` for the builder
  and `default` for the read-only agents, overridden by a `permission_mode` key of the role. Rejected: a
  new `[agents]` table (two places for a model); values fixed in the code.
- 2026-10-22: Decision (this repository's `CLAUDE.md`, the human, recommended): the last step installs the
  skill here and moves the general process out of `CLAUDE.md`, applied by the human with a script.
  Rejected: keep both texts (they drift); leave this repository without the skill.
- 2026-10-22: Decision (scope of the skill, the human, recommended): the general rules of "How work is
  done here", items 1 to 9 and "Build and review" go into the skill; `CLAUDE.md` keeps the plan, language,
  layout, private terms, thread limits and commit author. Rejected: items 1 to 9 only (a foreign
  repository would lack RED/GREEN and the review rules).
- 2026-10-22: Decision (an edited file, the human, recommended): it is kept and `init` warns with its name;
  to take stapel's new text the person deletes the file and runs `init` again. Rejected: a `.new` file next
  to it; a `--force` flag.
- 2026-10-22: Decision (frame, the human): the Frame above is confirmed in the chat.
- 2026-10-22: Decision (hook output, the human, recommended): `stapel hook session-start` prints the line
  that names the skill and then the text of `stapel next`, which saves the orchestrator one call; when
  `next` fails, only the line. Rejected: the line alone.
- 2026-10-22: Decision (hook events, the human, recommended): matcher `startup|clear|compact`; after a
  compaction the orchestrator may have lost the skill, and a resumed session keeps its context. Rejected:
  every event; `startup|clear` only.
- 2026-10-22: Decision (bypass, the human, recommended): `permission_mode = "bypassPermissions"` refuses
  with exit 1, because it removes the dialog for every call the guard lets through. Rejected: allow it,
  with or without a warning.
- 2026-10-22: Decision (a newer file, the human, recommended): a file with a valid hash whose marker
  version is greater than the binary's is kept with a warning, so an old binary on PATH does not roll
  stapel's files back. Rejected: the running binary's text wins.
- 2026-10-22: Decision (not a regular file, the human, recommended): a directory at a process file's path
  refuses with exit 1 before any write; a symlink or a file that is not UTF-8 is the person's file, kept
  with a warning. Rejected: keep every such path; refuse every such path.
- 2026-10-22: Decision (a broken marker, the human, recommended): a marker line without a valid version
  or hash, two marker lines, or a marker outside the frontmatter count as a hand edit: kept with a warning.
  Rejected: refuse with exit 1.
- 2026-10-22: Decision (restart, the human, recommended): when `init` creates `.claude/agents/` or
  `.claude/skills/`, it prints one line asking for a restart of Claude Code, since Claude Code does not
  see a new folder of agents until then. Rejected: no line.

## Spec

Terms:

- **Process file**: each of `.claude/skills/stapel/SKILL.md`, `.claude/agents/stapel-builder.md`,
  `.claude/agents/stapel-drift.md`, `.claude/agents/stapel-reviewer.md` and
  `.claude/agents/stapel-spec-reviewer.md`, paths relative to the repository root.
- **Agent file**: a process file under `.claude/agents/`; its **agent** is the name after `stapel-`.
- **Frontmatter**: the lines after a first line `---` up to the next line `---`.
- **Marker line**: a frontmatter line that starts with `# stapel:`.
- **Valid marker**: exactly one marker line, of the form `# stapel: <a>.<b>.<c> sha256:<h>` with `a`, `b`,
  `c` decimal numbers and `h` 64 lowercase hex digits; the version `<a>.<b>.<c>` compares by its numbers.
- **Unchanged file**: a regular file, not a symlink, read as UTF-8, with a valid marker whose `h` is the
  SHA-256 of the file's bytes without the marker line and its `\n`.
- **Stapel's text**: the text this binary writes to a process file: its frontmatter, the marker line of
  the binary's version (`CARGO_PKG_VERSION`) with the hash of the rest, and its body.
- **Role binding**: the `[models]` entry of an agent's role in `.stapel/stapel.toml` (the starter text when
  `init` creates it): `builder` for the builder, `drift` for the drift reviewer, `reviewer` for the
  reviewer and the spec reviewer.
- **Allowed mode**: one of `default`, `acceptEdits`, `auto`, `dontAsk`, `plan`, `manual`.
- **Session hook line**: `command -v stapel >/dev/null 2>&1 || exit 0; stapel hook session-start 2>/dev/null || exit 0`.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel init` runs and a process file does not exist THE command writes stapel's text to it, creating its folders, and prints `created: <path>`. | `init_process::creates_the_process_files` |
| AC-2 | WHEN `init` writes an agent file THE frontmatter holds, in this order, `name: stapel-<agent>`, `description: <its text>`, `tools: <list>`, `model: <model of the role binding>` (left out when the role has no binding), `permissionMode: <mode>` and the marker line; the tools are `Read, Edit, Write, Bash, Grep, Glob` for the builder and `Read, Grep, Glob` for the others. Output: `golden/init_agent_frontmatter.txt`. | `init_process::agent_frontmatter`, `init_process::model_from_the_role` |
| AC-3 | WHEN the role binding has no `permission_mode` THE mode is `acceptEdits` for the builder and `default` for the other agents; WHEN it holds an allowed mode THE mode is that value. | `init_process::permission_mode_of_the_role` |
| AC-4 | IF the binding of `builder`, `drift` or `reviewer` has `permission_mode` `bypassPermissions` THEN `init` prints `stapel: [models] <role>: permission_mode bypassPermissions is not allowed: it turns off the permission dialog for every call the guard lets through`; IF another value that is not an allowed mode THEN `stapel: [models] <role>: permission_mode "<value>" is not one of default, acceptEdits, auto, dontAsk, plan, manual`; exit 1, nothing written. | `init_process::refuses_a_bad_permission_mode` |
| AC-5 | IF the `model` of such a binding is empty or holds a character other than ASCII letters, digits, `.`, `_`, `-`, `:`, `/`, `[`, `]` THEN `init` prints `stapel: [models] <role>: model "<value>" cannot be written to an agent file`, exits 1 and writes nothing. | `init_process::refuses_a_bad_model` |
| AC-6 | WHEN `init` writes the skill THE frontmatter holds `name: stapel`, `description: <its text>` and the marker line, in this order. | `init_process::skill_frontmatter` |
| AC-7 | WHEN a process file is an unchanged file whose version is not greater than the binary's and its text differs from stapel's text THE command writes stapel's text and prints `updated: <path>`; WHEN the texts are equal it leaves the file and prints nothing for it. | `init_process::updates_an_unchanged_file`, `init_process::second_run_changes_nothing` |
| AC-8 | IF an existing process file is not an unchanged file (no marker line, a marker that is not valid, a hash that does not match, a symlink, not UTF-8) THEN `init` leaves it as it is and prints on stderr `warning: <path> is not the file stapel wrote; kept. Delete it and run stapel init to take stapel's text`. | `init_process::keeps_an_edited_file`, `init_process::keeps_a_file_without_a_valid_marker`, `init_process::keeps_a_symlink_and_non_utf8` |
| AC-9 | IF a process file is an unchanged file whose version is greater than the binary's THEN `init` leaves it and prints on stderr `warning: <path> was written by stapel <version>, newer than this stapel <own version>; kept`. | `init_process::keeps_a_newer_file` |
| AC-10 | IF a process file's path is a directory THEN `init` prints `stapel: <path> is a directory; nothing was written` and exits 1 before any write. | `init_process::refuses_a_directory` |
| AC-11 | WHEN `init` creates the folder `.claude/agents/` or `.claude/skills/` THE last line on stdout is `note: restart Claude Code so that it loads the new agents and skill`. | `init_process::restart_note` |
| AC-12 | WHEN no hook command of `.claude/settings.json` contains `stapel hook session-start` THE command adds to `hooks.SessionStart` the entry with matcher `startup\|clear\|compact`, the session hook line and timeout 10, keeping every other key and entry; when this is the only change to the file the label is `appended: .claude/settings.json (stapel session hook)`, otherwise the labels of STP-14 stay. IF `hooks.SessionStart` is not an array THEN exit 1 with `stapel: .claude/settings.json: the hooks.SessionStart field must be an array`. | `init_process::installs_the_session_hook`, `init_process::session_hook_next_to_others` |
| AC-13 | WHEN `stapel hook session-start` runs in a repository with `.stapel/stapel.toml` (found from `CLAUDE_PROJECT_DIR`, else the working folder) THE stdout is the line `This repository follows the stapel process: load the skill stapel and follow it.`, then, when `stapel next` succeeds there, the line `stapel next:` and its text output; exit 0. | `hook_session::prints_the_skill_line_and_next`, `hook_session::next_fails` |
| AC-14 | IF the hook runs outside such a repository, or fails or panics THEN it prints nothing and exits 0; the session hook line with no `stapel` on PATH prints nothing and exits 0. | `hook_session::silent_outside_a_stapel_repository`, `init_process::session_hook_line_without_stapel` |
| AC-15 | WHEN the skill is written THE body has the headings `## The loop`, `## How work is done`, `## The spec process`, `## Build and review`, the nine rules of the spec process as a numbered list, the sentence of AC-16, and the sentence `Before the spec review, run stapel brief on every Plan step and split the steps it refuses.` | `init_process::skill_text_holds_the_process` |
| AC-16 | WHEN the skill is written THE body holds `Spec changes inside the promise during the build are collected and confirmed by one stapel ok before code review; a change that widens the promise or changes a criterion's meaning stops the build until it is confirmed.` | `init_process::skill_text_holds_the_process` |
| AC-17 | WHEN an agent file is written THE body holds the role lines of the cases table (row "Agent texts"). | `init_process::agent_texts_hold_their_rules` |
| AC-18 | WHEN `stapel init` runs in a new repository with this repository's `.stapel/stapel.toml` THE five process files it writes equal byte for byte the process files of this repository. | `init_process::this_repository_has_the_current_files` |
| AC-19 | WHEN this repository's `CLAUDE.md` is read THE file has no heading `## The spec process` and no heading `## Build and review`, and holds `.claude/skills/stapel/SKILL.md`. | `init_process::claude_md_names_the_skill` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-19; every file `init` wrote before this ticket, and its
  labels, stay as they were (STP-14 labels included); a second run with nothing to change prints
  `already set up: nothing changed`; `permission_mode` is read only by `init`, so no other command reads or
  refuses it.
- **Not promised:** the Frame's list; a file with CRLF line ends (it reads as edited); a frontmatter that
  YAML would read differently from the line rule above; roles other than `builder`, `drift` and
  `reviewer` (their `permission_mode` is not checked); the main session's mode overriding an agent's
  `permissionMode` (Claude Code does this in its bypass, `acceptEdits` and auto modes); a user-level agent
  with the same name; files left behind when a write fails midway (as `init` today); case-insensitive file
  systems.

## Test plan

CLI tests in two new files, `crates/stapel-cli/tests/init_process.rs` and
`crates/stapel-cli/tests/hook_session.rs`, with `common::git_repo` and `common::stapel`. The golden file
`crates/stapel-cli/tests/golden/init_agent_frontmatter.txt` holds the four agent frontmatters of the
starter configuration, each marker hash written as `<hash>`. Every test runs the binary and asserts a
`created:` line, a refusal text or a hook output, so no test passes before its code. Light tests:
`cargo test -j 4 -p stapel-cli --test init_process -- --test-threads=3` (and `--test hook_session`,
`--test init`, `--test hooks_install`); before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Main path | an empty repository, `init --prefix ABC`: five `created:` lines, each file starts with `---` and has a valid marker whose hash matches | `init_process::creates_the_process_files` |
| Frontmatter | the starter configuration: the four frontmatters equal the golden file after the hash is replaced | `init_process::agent_frontmatter` |
| Model | a configuration whose `reviewer` model is `claude-opus-5-5[1m]`: both reviewer files hold it; a configuration without `drift`: no `model:` line in the drift file | `init_process::model_from_the_role` |
| Mode | no key: `acceptEdits` and `default`; `drift` with `plan`, `builder` with `manual`: those values | `init_process::permission_mode_of_the_role` |
| Bad mode | `bypassPermissions` on `builder`; `yolo` on `reviewer`; `Default` (case) on `drift`: exit 1, the text, no process file and no `.claude/settings.json` change; `bypassPermissions` on `author`: accepted | `init_process::refuses_a_bad_permission_mode` |
| Bad model | `""`, `a b`, `x\npermissionMode: plan`, `x#y`: exit 1, the text, nothing written | `init_process::refuses_a_bad_model` |
| Skill frontmatter | `name: stapel`, a `description:` line, a valid marker, in order | `init_process::skill_frontmatter` |
| Update | a file written by `init`, then its body changed and its marker hash recomputed with version `0.0.1`: `updated: <path>` and stapel's text again | `init_process::updates_an_unchanged_file` |
| Second run | `init` twice: the second prints `already set up: nothing changed`, files unchanged | `init_process::second_run_changes_nothing` |
| Edited | one byte of the body changed: the warning on stderr, the file unchanged, the other files written | `init_process::keeps_an_edited_file` |
| No valid marker | no marker line; `# stapel: 0.1 sha256:<h>`; uppercase hex; two marker lines; a valid marker line in the body only: each kept with the warning | `init_process::keeps_a_file_without_a_valid_marker` |
| Symlink, encoding | a symlink to a copy of stapel's text; a file with a `0xff` byte: kept with the warning | `init_process::keeps_a_symlink_and_non_utf8` |
| Newer | a valid hash with version `999.0.0`: kept with the newer warning; version `0.0.10` with a valid hash and other text: `updated:` | `init_process::keeps_a_newer_file` |
| Directory | `.claude/agents/stapel-builder.md/` a folder: exit 1, the text, no other process file and no `.stapel/` written | `init_process::refuses_a_directory` |
| Restart | a first run: the last stdout line is the note; a run with both folders present and one file missing: no note | `init_process::restart_note` |
| Session hook | no settings: the entry under `SessionStart`, matcher, line, timeout; a second run adds none | `init_process::installs_the_session_hook` |
| Session hook next to others | settings with a foreign `SessionStart` entry and the STP-14 hooks: both kept, the label `appended: .claude/settings.json (stapel session hook)`; `SessionStart` an object: exit 1 | `init_process::session_hook_next_to_others` |
| Hook line without stapel | the installed line run by `sh` with a PATH without `stapel`: empty stdout, exit 0 | `init_process::session_hook_line_without_stapel` |
| Hook output | an initialised repository with an open ticket: the first line, `stapel next:`, the lines `ticket:` and `next:`; with `CLAUDE_PROJECT_DIR` set and the working folder elsewhere: the same | `hook_session::prints_the_skill_line_and_next` |
| Next fails | `.stapel/stapel.toml` present, `next` refusing (two open tickets): only the first line, exit 0 | `hook_session::next_fails` |
| Outside | a folder with no `.stapel/stapel.toml`; an unreadable `stapel.toml`: empty stdout, exit 0 | `hook_session::silent_outside_a_stapel_repository` |
| Skill text | the headings, nine numbered rules under `## The spec process`, the sentences of AC-15 and AC-16 | `init_process::skill_text_holds_the_process` |
| Agent texts | builder: `stapel brief`, `GREEN never changes files under tests/`, `Write files with Edit or Write, never with sed, cat or a redirection`, `Never git push`, `never switch branches`, `never run stapel ok or stapel close`; reviewer: `You work on a copy`, `run no builds`, `catchable_at`, `deletion check`, `claims check`, `verification gap`; drift: `only the step's diff and its criteria`; spec reviewer: `first read only the Frame and the criteria`, `Clear, Partial or Missing`, `inside` and `outside` | `init_process::agent_texts_hold_their_rules` |
| This repository | a new repository with this repository's `.stapel/stapel.toml`, `init`: each process file equals this repository's | `init_process::this_repository_has_the_current_files` |
| CLAUDE.md | this repository's `CLAUDE.md`: no `## The spec process`, no `## Build and review`, the skill path present | `init_process::claude_md_names_the_skill` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `--prefix` of `init` | 2 to 8 uppercase Latin letters | as STP-1 | absent with a config: ignored | as STP-1 | exact | `init::rejects_bad_prefix` |
| `[models].<role>.model` | string | one character | `""`: exit 1; no binding: no `model:` line | a character outside the list of AC-5: exit 1 | written as is | `init_process::refuses_a_bad_model`, `init_process::model_from_the_role` |
| `[models].<role>.permission_mode` | string | — | absent: the default of AC-3 | `bypassPermissions`, any value not an allowed mode (case counts): exit 1 | exact | `init_process::refuses_a_bad_permission_mode`, `init_process::permission_mode_of_the_role` |
| a process file on disk | file | — | missing: created | a directory: exit 1; a symlink, not UTF-8, no valid marker, a wrong hash: kept with a warning | hash over bytes | `init_process::refuses_a_directory`, `init_process::keeps_a_symlink_and_non_utf8`, `init_process::keeps_a_file_without_a_valid_marker` |
| marker version | `<a>.<b>.<c>` decimal | `0.0.0` | — | another form: not valid | numeric per part | `init_process::keeps_a_newer_file` |
| `hooks.SessionStart` of `.claude/settings.json` | JSON array | — | absent: created | not an array: exit 1 | a command containing `stapel hook session-start` counts as installed | `init_process::session_hook_next_to_others` |
| `CLAUDE_PROJECT_DIR` for the hook | path | — | unset: the working folder | a path with no `.stapel/stapel.toml`: nothing printed | as given | `hook_session::prints_the_skill_line_and_next`, `hook_session::silent_outside_a_stapel_repository` |
| hook stdin | JSON of Claude Code | — | — | not read | — | `hook_session::prints_the_skill_line_and_next` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | permission mode, whether the dialog is offered | `permissionMode` written per agent; bypass refused; the main session overriding it not promised | `init_process::refuses_a_bad_permission_mode` |
| claude-code | environment of hook processes versus the agent's Bash shell | the hook reads `CLAUDE_PROJECT_DIR` or the working folder; a missing binary is silent | `hook_session::prints_the_skill_line_and_next`, `init_process::session_hook_line_without_stapel` |
| claude-code | settings at user, project and local level | only the project `.claude/settings.json` is written, other keys and entries kept; local and user settings not touched; a user-level agent of the same name not promised | `init_process::session_hook_next_to_others` |
| claude-code | transcripts; tool call succeeded or failed | not read | — |
| git | dirty tree | `init` writes as today, whatever the tree | `init_process::creates_the_process_files` |
| git | configuration: hooks path | as STP-1 (pre-push), unchanged | `hooks_install::skips_hooks_dir_outside_repo` |
| git | history, shallow, object format, worktrees, submodules, paths, messages | not read by the new code; `next` inside the hook as STP-20 | — |
| cargo | every dimension | not run | — |
| os | symlinks | kept with a warning, never followed for a write | `init_process::keeps_a_symlink_and_non_utf8` |
| os | file permissions | a write error exits 1 with the path, as `init` today; files written before it stay (not promised) | — |
| os | case-insensitive file systems; processes; time; WSL | not promised | — |

**Review Focus:** a process file with CRLF line ends (reads as edited; not promised); a marker line inside
the body only (kept; `init_process::keeps_a_file_without_a_valid_marker`); a model `inherit` (letters,
accepted; `init_process::model_from_the_role` does not cover it, accepted by the character rule); a
`.stapel/stapel.toml` without `[models]` (no `model:` lines; covered by the missing-role case); the hook run
while `next` is slow (it runs `next` in the process; `next` reads no network; not tested).

**Author self-check:** 2026-10-22, before spec review: every promise item has a criterion and a test
(item 1: AC-1, AC-6; item 2: AC-2 to AC-5; item 3: AC-7 to AC-11; item 4: AC-12 to AC-14; item 5: AC-15 to
AC-17; item 6: AC-18, AC-19); each input has its empty and invalid case; the rows were checked against
`init.rs`, `hook.rs`, `config.rs` and the catalogue; every test runs the binary and asserts new output, or
reads a file that this ticket changes; no open question is left.

## Design

### Facts

- F-1: `init::run` plans every write first (`Step` list) and writes after all checks, so a refusal leaves
  the repository untouched; `plan_hooks` reads `.claude/settings.json` once and writes it once with all
  stapel hooks (`crates/stapel-cli/src/init.rs`).
- F-2: `ModelBinding` has no `deny_unknown_fields`, so a `permission_mode` key is ignored today by every
  command (`crates/stapel-core/src/config.rs`).
- F-3: `hook::post_tool_use` shows the fail-open form: a panic is caught, every error prints nothing and
  exits 0 (`crates/stapel-cli/src/hook.rs`).
- F-4: Claude Code (documentation, 2026-10-21): an agent's `model` takes a full id or an alias; unknown
  frontmatter fields are ignored; a subagent cannot ask the person; agent and skill files are picked up
  without a restart except the first file of a new folder; SessionStart stdout becomes context.

### Decisions (before the build)

- The bodies of the five files are text files under `crates/stapel-cli/assets/` (`skill.md`,
  `agents/builder.md`, `agents/drift.md`, `agents/reviewer.md`, `agents/spec-reviewer.md`), taken in with
  `include_str!`; the frontmatter is built in code from the role binding, so a `model` change in
  `stapel.toml` reaches the file at the next `init` (the file is unchanged, its text differs).
- `ModelBinding` gains `permission_mode: Option<String>` (`skip_serializing_if = "Option::is_none"`);
  only `init` checks it.
- The process files are planned after the pre-push hook, in the order of the term **Process file**; the
  folder note is printed after every label.
- `stapel hook session-start` runs `next` in the same process through a function that returns its text
  instead of printing it, so no second binary is started.
- The texts are drafted by the orchestrator in `.stapel/scratch/stp22/texts/` from `CLAUDE.md`, the ledger
  decision of 2026-10-21 and the briefs of STP-5 to STP-21 before step 5; the builder of step 5 copies them
  into `assets/` and adjusts them only to make its tests pass.

### Decisions (as built)

TODO

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `init_process::creates_the_process_files` |
| AC-2 | `init_process::agent_frontmatter`, `init_process::model_from_the_role` |
| AC-3 | `init_process::permission_mode_of_the_role` |
| AC-4 | `init_process::refuses_a_bad_permission_mode` |
| AC-5 | `init_process::refuses_a_bad_model` |
| AC-6 | `init_process::skill_frontmatter` |
| AC-7 | `init_process::updates_an_unchanged_file`, `init_process::second_run_changes_nothing` |
| AC-8 | `init_process::keeps_an_edited_file`, `init_process::keeps_a_file_without_a_valid_marker`, `init_process::keeps_a_symlink_and_non_utf8` |
| AC-9 | `init_process::keeps_a_newer_file` |
| AC-10 | `init_process::refuses_a_directory` |
| AC-11 | `init_process::restart_note` |
| AC-12 | `init_process::installs_the_session_hook`, `init_process::session_hook_next_to_others` |
| AC-13 | `hook_session::prints_the_skill_line_and_next`, `hook_session::next_fails` |
| AC-14 | `hook_session::silent_outside_a_stapel_repository`, `init_process::session_hook_line_without_stapel` |
| AC-15 | `init_process::skill_text_holds_the_process` |
| AC-16 | `init_process::skill_text_holds_the_process` |
| AC-17 | `init_process::agent_texts_hold_their_rules` |
| AC-18 | `init_process::this_repository_has_the_current_files` |
| AC-19 | `init_process::claude_md_names_the_skill` |

## Plan

Route: full (tags `interface`, `guard`, `migration`), auto.

1. The five process files are created with their frontmatter and marker, the bodies a placeholder line
   each (AC-1, AC-2, AC-3, AC-6; tests `init_process::creates_the_process_files`,
   `init_process::agent_frontmatter`, `init_process::model_from_the_role`,
   `init_process::permission_mode_of_the_role`, `init_process::skill_frontmatter`). Must not change: the
   other files of `init` and their labels (`tests/init.rs`, `tests/hooks_install.rs`).
2. Refusals of a role binding and of a directory (AC-4, AC-5, AC-10; tests
   `init_process::refuses_a_bad_permission_mode`, `init_process::refuses_a_bad_model`,
   `init_process::refuses_a_directory`). Must not change: the files of step 1.
3. A re-run updates stapel's unchanged files, and the restart note (AC-7, AC-11; tests
   `init_process::updates_an_unchanged_file`, `init_process::second_run_changes_nothing`,
   `init_process::restart_note`). Must not change: a first run's files.
3b. A re-run keeps a file that is not stapel's unchanged file, or is newer, with a warning (AC-8, AC-9;
   tests `init_process::keeps_an_edited_file`, `init_process::keeps_a_file_without_a_valid_marker`,
   `init_process::keeps_a_symlink_and_non_utf8`, `init_process::keeps_a_newer_file`). Must not change: the
   update and the note of step 3.
4. `init` installs the SessionStart hook (AC-12, AC-14; tests `init_process::installs_the_session_hook`,
   `init_process::session_hook_next_to_others`, `init_process::session_hook_line_without_stapel`). Must not
   change: the PreToolUse, PostToolUse and PostToolUseFailure entries and their labels.
4b. The session-start hook command (AC-13, AC-14; tests
   `hook_session::prints_the_skill_line_and_next`, `hook_session::next_fails`,
   `hook_session::silent_outside_a_stapel_repository`). Must not change: the output of `stapel next`.
5. The skill and agent texts (AC-15, AC-16, AC-17; tests `init_process::skill_text_holds_the_process`,
   `init_process::agent_texts_hold_their_rules`), copied from `.stapel/scratch/stp22/texts/`. Must not
   change: the frontmatter of step 1 except its hash.
6. This repository gets the process files and a shorter `CLAUDE.md` (AC-18, AC-19; tests
   `init_process::this_repository_has_the_current_files`, `init_process::claude_md_names_the_skill`). GREEN:
   the builder runs `stapel init` here; the human applies the `CLAUDE.md` change with a script the
   orchestrator writes under `.stapel/scratch/stp22/`. Must not change: the rules of `CLAUDE.md` that stay
   (the plan, language, layout, private terms, thread limits, commit author).

## Review

TODO

## Summary

TODO
