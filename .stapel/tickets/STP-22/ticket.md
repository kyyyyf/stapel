# STP-22 — stapel init installs the process: skill, agent definitions, hooks

## Description

Source: `docs/PHASES.md`, plan item 1.3; the ledger decision of 2026-10-21 (approval dialogs, part 2: rule 5
of the spec process moves into the skill text).

### Frame

**Problem.** The process lives in `CLAUDE.md` and hand-written briefs; a foreign repository gets the
guard from `stapel init` but no process. `init` installs it as data.

**Plan item:** 1.3.

**Risk tags:** `interface` (files Claude Code reads), `guard` (agent tools and permission modes),
`migration` (a re-run updates installed files and keeps hand edits).

**Non-goals.** Items 1.4, 1.5, 1.8, 1.9, 1.10; `--force` or `.new` files; a plugin; `~/.claude`;
removing files; agents other than Claude Code; making the model follow the text.

**Promise (closed list).**

1. `init` writes the skill `.claude/skills/stapel/SKILL.md` and the agents `stapel-builder`,
   `stapel-drift`, `stapel-reviewer`, `stapel-spec-reviewer` under `.claude/agents/`, each with a marker
   `# stapel: <version> sha256:<hash>` in its frontmatter; when missing, it also writes the two catalogues
   `.stapel/config/coverage-taxonomy.yml` and `external-states.yml` that the skill reads.
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
- 2026-10-22: Decision (loading the skill, the human, recommended; its output refined by the hook output
  decision below): a SessionStart hook prints one line that names the skill and `stapel next`, because Claude Code cannot be made to load a skill and `init`
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
- 2026-10-22: Decision (after spec review round 1, recommended): 13 inside findings are patched or kept
  with a reason (`.stapel/scratch/stp22/findings-sr1.jsonl`); no HIGH finding, so no second round. The
  restart note stays keyed to `.claude/agents/` and `.claude/skills/`, because Claude Code picks up a new
  skill in a skills folder it already watches (SR1-2); the STP-14 labels stay when the session hook is
  added with another stapel hook (SR1-7).
- 2026-10-22: Decision (provider, SR1-11, the human): the model of a role is written whatever its
  provider. Rejected: only `claude-code` bindings; a refusal.
- 2026-10-22: Decision (the local hook, SR1-12, the human): in step 6 the orchestrator gives the human a
  command that removes this repository's local SessionStart hook from `.claude/settings.local.json`, so the
  context is not printed twice. Rejected: keep both hooks.
- 2026-10-22: Decision (after code review round 1, the human): `init` also creates the two catalogues of
  `.stapel/config/` when they are missing, since the skill and the spec reviewer read them (E-3; the promise
  widens, Frame item 1). The ticket stays one ticket although its tests grew to about 1 500 lines against 700;
  the source stays at about 400 lines, under the split point (DR-1). Recommended and taken: quoted YAML values
  (R-1), the dropped rules back in the skill (R-2, R-3, R-4, E-1, E-2), the long lines rewrapped (R-7), the text contradictions fixed (D5-2, D8-2,
  D8-3, E-6). Not promised: a symlinked parent folder (R-6), CRLF (E-9), files left after a failed write (E-5).
- 2026-10-22: Decision (after the drift review of step 9b, the human): the wording findings are fixed now, not
  deferred (D9B-1, D9B-3, D9B-4, D9B-5; AC-25, step 9d); the lint runs before each commit in the skill and the
  builder text; D9B-2 and D9B-6 are accepted without change.

## Spec

Terms:

- **Process file**: `.claude/skills/stapel/SKILL.md`, then `.claude/agents/stapel-<agent>.md` for the
  agents `builder`, `drift`, `reviewer`, `spec-reviewer`, in this order, under the repository root.
- **Agent file**: a process file under `.claude/agents/`.
- **Frontmatter**: the lines after a first line `---` up to the next line `---`.
- **Marker line**: a frontmatter line that starts with `# stapel:`.
- **Valid marker**: exactly one marker line, of the form `# stapel: <a>.<b>.<c> sha256:<h>` with `a`, `b`,
  `c` decimal numbers and `h` 64 lowercase hex digits; the version `<a>.<b>.<c>` compares by its numbers.
- **Unchanged file**: a regular file, not a symlink, read as UTF-8, with a valid marker whose `h` is the
  SHA-256 of the file's bytes without the marker line and its `\n`.
- **Stapel's text**: the text this binary writes to a process file: its frontmatter, the marker line of
  the binary's version (`CARGO_PKG_VERSION`) with the hash of the rest, and its body.
- **Role binding**: the `[models]` entry of the agent's role (`builder`, `drift`, or `reviewer` for both
  reviewers) in `.stapel/stapel.toml`, or in the starter text when `init` creates it.
- **Allowed mode**: one of `default`, `acceptEdits`, `auto`, `dontAsk`, `plan`, `manual`.
- **Session hook matcher**: startup|clear|compact (the literal value, no quotes).
- **Session hook line**: `command -v stapel >/dev/null 2>&1 || exit 0; stapel hook session-start 2>/dev/null || exit 0`.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel init` runs and no file and no symlink is at a process file's path THE command writes stapel's text to it, creating its folders, and prints `created: <path>`. | `init_process::creates_the_process_files` |
| AC-2 | WHEN `init` writes an agent file THE frontmatter holds, in this order, `name: stapel-<agent>`, `description: "<its text>"`, `tools: <list>`, `model: "<model of the role binding>"` (left out when the role has no binding; both values as double-quoted YAML strings), `permissionMode: <mode>` and the marker line; the tools are `Read, Edit, Write, Bash, Grep, Glob` for the builder and `Read, Grep, Glob` for the others. Output: `golden/init_agent_frontmatter.txt`. | `init_process::agent_frontmatter`, `init_process::model_from_the_role`, `init_process::marker_holds_the_binary_version` |
| AC-3 | WHEN the role binding has no `permission_mode` THE mode is `acceptEdits` for the builder and `default` for the other agents; WHEN it holds an allowed mode THE mode is that value. | `init_process::permission_mode_of_the_role`, `init_process::reviewer_mode_reaches_both_reviewers`, `init_process::model_change_updates_both_reviewers` |
| AC-4 | IF the binding of `builder`, `drift` or `reviewer` has `permission_mode` `bypassPermissions` THEN `init` prints `stapel: [models] <role>: permission_mode bypassPermissions is not allowed: it turns off the permission dialog for every call the guard lets through`; IF another value that is not an allowed mode THEN `stapel: [models] <role>: permission_mode "<value>" is not one of default, acceptEdits, auto, dontAsk, plan, manual`; exit 1, nothing written. | `init_process::refuses_a_bad_permission_mode`, `init_process::refusals_on_every_checked_role` |
| AC-5 | IF the `model` of such a binding is empty, does not start with an ASCII letter or digit, or holds a character other than ASCII letters, digits, `.`, `_`, `-`, `:`, `/`, `[`, `]` THEN `init` prints `stapel: [models] <role>: model "<value>" cannot be written to an agent file`, exits 1 and writes nothing. | `init_process::refuses_a_bad_model` |
| AC-6 | WHEN `init` writes the skill THE frontmatter holds `name: stapel`, `description: "<its text>"` and the marker line, in this order. | `init_process::skill_frontmatter` |
| AC-7 | WHEN a process file is an unchanged file whose version is not greater than the binary's and its text differs from stapel's text THE command writes stapel's text and prints `updated: <path>`; WHEN the texts are equal it leaves the file and prints nothing for it. | `init_process::updates_an_unchanged_file`, `init_process::second_run_changes_nothing`, `init_process::hash_computed_by_hand` |
| AC-8 | IF an existing process file is not an unchanged file (no marker line, a marker that is not valid, a hash that does not match, a symlink, dangling or not, not UTF-8) THEN `init` leaves it as it is and prints on stderr `warning: <path> is not the file stapel wrote; kept. Delete it and run stapel init to take stapel's text`; after such a warning the line `already set up: nothing changed` is not printed. | `init_process::keeps_an_edited_file`, `init_process::keeps_a_file_without_a_valid_marker`, `init_process::keeps_a_symlink_and_non_utf8`, `init_process::newer_file_with_a_wrong_hash` |
| AC-9 | IF a process file is an unchanged file whose version is greater than the binary's THEN `init` leaves it and prints on stderr `warning: <path> was written by stapel <version>, newer than this stapel <own version>; kept`; after it the line `already set up: nothing changed` is not printed. | `init_process::keeps_a_newer_file`, `init_process::newer_file_alone`, `init_process::versions_compare_by_numbers` |
| AC-10 | IF a process file's path is a directory THEN `init` prints `stapel: <path> is a directory; nothing was written` and exits 1 before any write. | `init_process::refuses_a_directory`, `init_process::refuses_a_directory_at_any_process_path` |
| AC-11 | WHEN `init` creates the folder `.claude/agents/` or `.claude/skills/` THE last line on stdout is `note: restart Claude Code so that it loads the new agents and skill`. | `init_process::restart_note`, `init_process::restart_note_for_one_missing_folder` |
| AC-12 | WHEN no hook command of `.claude/settings.json` contains `stapel hook session-start` THE command adds to `hooks.SessionStart` the entry with the session hook matcher, the session hook line and timeout 10, keeping every other key and entry; when this is the only change to the file the label is `appended: .claude/settings.json (stapel session hook)`, otherwise the labels of STP-14 stay. IF `hooks.SessionStart` is not an array THEN exit 1 with `stapel: .claude/settings.json: the hooks.SessionStart field must be an array`. | `init_process::installs_the_session_hook`, `init_process::session_hook_next_to_others`, `init_process::session_hook_inside_a_wrapper`, `init_process::session_hook_labels_with_other_changes`, `init_process::session_hook_keeps_every_event` |
| AC-13 | WHEN `stapel hook session-start` runs and `.stapel/stapel.toml` is a file in the repository of `CLAUDE_PROJECT_DIR`, else of the working folder, THE stdout is the line `This repository follows the stapel process: load the skill stapel and follow it.`, then, when `stapel next` succeeds there, the line `stapel next:` and its text output, and when it fails (an unreadable `stapel.toml` included) nothing more; exit 0. | `hook_session::prints_the_skill_line_and_next`, `hook_session::next_fails`, `hook_session::unreadable_config` |
| AC-14 | IF no such file is found, or the hook panics before its first line, THEN it prints nothing and exits 0; the session hook line with no `stapel` on PATH prints nothing and exits 0. | `hook_session::silent_outside_a_stapel_repository`, `init_process::session_hook_line_without_stapel`, `init_process::session_hook_line_with_a_failing_stapel` |
| AC-15 | WHEN the skill is written THE body has the headings `## The loop`, `## How work is done`, `## The spec process`, `## Build and review`, the nine rules of the spec process as a numbered list, the sentence of AC-16, and the sentence `Before the spec review, run stapel brief on every Plan step and split the steps it refuses.` | `init_process::skill_text_holds_the_process` |
| AC-16 | WHEN the skill is written THE body holds `Spec changes inside the promise during the build are collected and confirmed by one stapel ok before code review; a change that widens the promise or changes a criterion's meaning stops the build until it is confirmed.` | `init_process::skill_text_holds_the_process` |
| AC-17 | WHEN an agent file is written THE body holds the role lines of the cases table (row "Agent texts"). | `init_process::agent_texts_hold_their_rules`, `init_process::read_only_agents_say_so` |
| AC-18 | WHEN `stapel init` runs in a new repository with this repository's `.stapel/stapel.toml` THE five process files it writes equal byte for byte the process files of this repository. | `init_process::this_repository_has_the_current_files` |
| AC-19 | WHEN this repository's `CLAUDE.md` is read THE file has no heading `## The spec process` and no heading `## Build and review`, and holds `.claude/skills/stapel/SKILL.md` and `crates/stapel-cli/tests/ticket_drift.rs`. | `init_process::claude_md_names_the_skill`, `init_process::claude_md_names_the_ticket_lint` |
| AC-20 | WHEN the skill is written THE body holds `An untagged ticket gets the ticket lint and one stapel-reviewer; a tagged ticket gets stapel-reviewer, an external reviewer, and stapel-drift after each GREEN step that touches a criterion.`, `stapel ok runs only as one plain command whose permission dialog the person answers; stapel close is run by the person.`, `During the build you do not read the brief; before the spec review you run stapel brief only to check each step's size.` and `A ticket tagged guard or security has an Abuse table: each attempt of a careless agent and its outcome.`, and no longer holds `Review before merge:`. | `init_process::skill_text_is_consistent` |
| AC-21 | WHEN the builder agent file is written THE body holds `Never run git reset --hard, git stash or git checkout -- <file>.` | `init_process::builder_text_forbids_history_rewrites` |
| AC-22 | WHEN `stapel init` runs and `.stapel/config/coverage-taxonomy.yml` or `.stapel/config/external-states.yml` does not exist THE command writes stapel's copy of that catalogue (equal to this repository's file) and prints `created: <path>`; an existing catalogue is never changed. | `init_process::installs_the_catalogues` |
| AC-23 | WHEN the skill is written THE body holds `A GREEN commit that changes a criterion's meaning says spec change: AC-<n>, <reason> in its message; stapel check reads it.`, `For a tagged ticket, study the prior art (earlier tickets, other tools) before the questions.`, `stapel check runs the tests with the lock file as it is, so a stale lock file fails the check.`, `The other actions of stapel next name their own command or file; do what they name.` and a sentence that starts `The ticket lint fails when`. | `init_process::skill_text_keeps_the_old_rules` |
| AC-24 | WHEN the builder agent file is written THE body holds `For green-step the RED exists: make only the GREEN.` and no longer holds `Run only the test command`. | `init_process::builder_text_for_green_step` |
| AC-25 | WHEN the skill and the builder file are written THE skill body holds `They type no stapel command; the one exception is stapel close, which the person runs in their own terminal until it goes through a permission dialog.`, `The ticket lint fails when a ticket names a test that does not exist, a test exists that no ticket names, a criterion quotes an output line that is not in the code, a ticket names a missing path, a tagged ticket lacks its tables or a dated self-check, a ticket tagged guard or security lacks its Abuse table, the test lists of criteria, Test plan and Proof disagree, or a CLI option is in no Inputs table.` and `The builder runs the ticket lint before each commit.`, and the builder body holds `For green-step the RED exists: make only the GREEN. Skip item 1.` after its line that starts `2. GREEN` and holds `before each commit`. | `init_process::texts_agree_on_close_lint_and_green_step` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-25; every file `init` wrote before this ticket, and its
  labels, stay as they were (STP-14 labels included); a second run with nothing to change and no warning
  prints `already set up: nothing changed`; a new stapel version changes every marker, so this repository
  needs `stapel init` after a version bump, which AC-18 enforces; `permission_mode` is read only by `init`, so no other command reads or
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
starter configuration, each marker version written as `<version>` and each hash as `<hash>`. Every test runs the binary and asserts a
`created:` line, a refusal text or a hook output, so no test passes before its code. Light tests:
`cargo test -j 4 -p stapel-cli --test init_process -- --test-threads=3` (and `--test hook_session`,
`--test init`, `--test hooks_install`); before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Main path | `init --prefix ABC` in an empty repository: five `created:` lines, each file with a valid marker whose hash matches | `init_process::creates_the_process_files` |
| Frontmatter | the starter configuration: the golden file, version and hash replaced | `init_process::agent_frontmatter` |
| Model | `reviewer` model `claude-opus-5-5[1m]`: both reviewer files hold it; no `drift` binding: no `model:` line | `init_process::model_from_the_role` |
| Mode | no key: `acceptEdits` and `default`; `drift` with `plan`, `builder` with `manual`: those values | `init_process::permission_mode_of_the_role` |
| Bad mode | `bypassPermissions` on `builder`; `yolo` on `reviewer`; `Default` (case) on `drift`: exit 1, the text, no process file and no `.claude/settings.json` change; `bypassPermissions` on `author`: accepted | `init_process::refuses_a_bad_permission_mode` |
| Bad model | `""`, `[1m]`, `a b`, `x\npermissionMode: plan`, `x#y`: exit 1, the text, nothing written | `init_process::refuses_a_bad_model` |
| Skill frontmatter | `name: stapel`, a `description:` line, a valid marker, in order | `init_process::skill_frontmatter` |
| Update | a file written by `init`, then its body changed and its marker hash recomputed with version `0.0.1`: `updated: <path>` and stapel's text again | `init_process::updates_an_unchanged_file` |
| Second run | `init` twice: `already set up: nothing changed`, files unchanged | `init_process::second_run_changes_nothing` |
| Edited | one byte of the body changed: the warning on stderr, the file unchanged, the other files written; a second run with only that warning: no `already set up` line | `init_process::keeps_an_edited_file` |
| No valid marker | no marker line; `# stapel: 0.1 sha256:<h>`; uppercase hex; two marker lines; a valid marker line in the body only: each kept with the warning | `init_process::keeps_a_file_without_a_valid_marker` |
| Symlink, encoding | a symlink to a copy of stapel's text; a dangling symlink to a path outside the repository (nothing is created there); a file with a `0xff` byte: kept with the warning | `init_process::keeps_a_symlink_and_non_utf8` |
| Newer | a valid hash with version `999.0.0`: kept with the newer warning; version `0.0.10` with a valid hash and other text: `updated:` | `init_process::keeps_a_newer_file` |
| Directory | `.claude/agents/stapel-builder.md/` a folder: exit 1, the text, no other process file and no `.stapel/` written | `init_process::refuses_a_directory` |
| Restart | a first run: the last stdout line is the note; a run with both folders present and one file missing: no note | `init_process::restart_note` |
| Session hook | no settings: the entry under `SessionStart`, matcher, line, timeout; a second run adds none | `init_process::installs_the_session_hook` |
| Session hook next to others | settings with a foreign `SessionStart` entry and the STP-14 hooks: both kept, the label `appended: .claude/settings.json (stapel session hook)`; `SessionStart` an object: exit 1 | `init_process::session_hook_next_to_others` |
| Hook line without stapel | the installed line run by `sh` with a PATH without `stapel`: empty stdout, exit 0 | `init_process::session_hook_line_without_stapel` |
| Hook output | an initialised repository with an open ticket: the first line, `stapel next:`, the lines `ticket:` and `next:`; with `CLAUDE_PROJECT_DIR` set and the working folder elsewhere: the same | `hook_session::prints_the_skill_line_and_next` |
| Next fails | `.stapel/stapel.toml` present, `next` refusing (two open tickets); a `stapel.toml` that does not parse: only the first line, exit 0 | `hook_session::next_fails` |
| Outside | a git repository with no `.stapel/stapel.toml`; a folder outside any repository: empty stdout, exit 0 | `hook_session::silent_outside_a_stapel_repository` |
| Skill text | the headings, nine numbered rules under `## The spec process`, the sentences of AC-15 and AC-16 | `init_process::skill_text_holds_the_process` |
| Agent texts | builder: `stapel brief`, `GREEN never changes files under tests/`, `Write files with Edit or Write, never with sed, cat or a redirection`, `Never git push`, `never switch branches`, `never run stapel ok or stapel close`; reviewer: `You work on a copy`, `run no builds`, `catchable_at`, `deletion check`, `claims check`, `verification gap`; drift: `only the step's diff and its criteria`; spec reviewer: `first read only the Frame and the criteria`, `Clear, Partial or Missing`, `inside` and `outside` | `init_process::agent_texts_hold_their_rules` |
| This repository | a new repository with this repository's `.stapel/stapel.toml`, `init`: each process file equals this repository's | `init_process::this_repository_has_the_current_files` |
| Marker version | the marker of a written file holds `CARGO_PKG_VERSION` of the binary (D1-1) | `init_process::marker_holds_the_binary_version` |
| Reviewer mode | `reviewer` with `plan`: both reviewer files hold `permissionMode: plan` (D1-2) | `init_process::reviewer_mode_reaches_both_reviewers` |
| Every role | `bypassPermissions`, `yolo` and the model `x#y` on `drift` and on `reviewer`: the texts of AC-4 and AC-5 (D2-1) | `init_process::refusals_on_every_checked_role` |
| Any path | a folder at `.claude/skills/stapel/SKILL.md`, then at `stapel-spec-reviewer.md`: exit 1, `common::snapshot` equal before and after (D2-2, D2-3) | `init_process::refuses_a_directory_at_any_process_path` |
| One folder | `.claude/skills/` present and `.claude/agents/` missing, and the reverse: the note is the last line, after the `created:` lines (D3-4) | `init_process::restart_note_for_one_missing_folder` |
| Hash by hand | a file whose marker hash the test computes with `sha2` over the bytes without the marker line: `updated:` when its version is `0.0.1` and its body differs (D3-5) | `init_process::hash_computed_by_hand` |
| Newer alone | only one file differs, an unchanged file of version `999.0.0`: the newer warning, no `already set up` line (D3B-2) | `init_process::newer_file_alone` |
| Number order | an unchanged file of version `0.09.0` (newer by numbers, older as text): the newer warning (D3B-3) | `init_process::versions_compare_by_numbers` |
| Newer, wrong hash | version `999.0.0` with a hash that does not match: the AC-8 warning, not the newer one (D3B-4) | `init_process::newer_file_with_a_wrong_hash` |
| Wrapper | a foreign `SessionStart` command `my-wrapper; stapel hook session-start`: nothing added, no session label (D4-2) | `init_process::session_hook_inside_a_wrapper` |
| Mixed labels | the bare PreToolUse line and no `SessionStart`: `updated: .claude/settings.json (stapel hook line)`; no settings: `created: .claude/settings.json` (D4-3) | `init_process::session_hook_labels_with_other_changes` |
| Every event | settings with all four stapel events and a foreign entry in each: all kept in place; `.claude/settings.local.json` byte-equal after the run (D4-4, D4-6) | `init_process::session_hook_keeps_every_event` |
| Failing stapel | the installed line with a fake `stapel` that prints to stderr and exits 3: empty stdout and stderr, exit 0 (D4-7) | `init_process::session_hook_line_with_a_failing_stapel` |
| Unreadable config | `.stapel/stapel.toml` a file with a `0xff` byte (not UTF-8): only the first line, exit 0; a folder at that path is no file, so the hook is silent (AC-14) (D4B-3) | `hook_session::unreadable_config` |
| Read-only texts | the drift and spec reviewer bodies hold `no write access` and `run no builds`; the reviewer body too (D5-5) | `init_process::read_only_agents_say_so` |
| Consistent skill | the four sentences of AC-20 are present and `Review before merge:` is absent (D5-1, D5-2) | `init_process::skill_text_is_consistent` |
| Builder history | the sentence of AC-21 in the builder body (D5-3) | `init_process::builder_text_forbids_history_rewrites` |
| Quoted values | the golden file holds `description: "…"` and `model: "…"`; a description with `: ` stays one string (R-1) | `init_process::agent_frontmatter` |
| Catalogues | an empty repository: both catalogues created, equal to this repository's files; a second run with an edited catalogue: unchanged, no label (E-3) | `init_process::installs_the_catalogues` |
| Model change | `init`, then the `reviewer` model changed in `stapel.toml`, `init` again: `updated:` for both reviewer files, the new model in each (E-4) | `init_process::model_change_updates_both_reviewers` |
| Old rules | the sentences of AC-23 in the skill body (R-2, R-3, R-4, E-1, E-2) | `init_process::skill_text_keeps_the_old_rules` |
| Green step | the sentences of AC-24 in the builder body (E-6, D8-2) | `init_process::builder_text_for_green_step` |
| Texts agree | the sentences of AC-25; the green-step sentence after `2. GREEN` in the builder body (D9B-1, D9B-3, D9B-4, D9B-5) | `init_process::texts_agree_on_close_lint_and_green_step` |
| Ticket lint | this repository's `CLAUDE.md` names `crates/stapel-cli/tests/ticket_drift.rs` (E-2) | `init_process::claude_md_names_the_ticket_lint` |
| CLAUDE.md | this repository's `CLAUDE.md`: no `## The spec process`, no `## Build and review`, the skill path present | `init_process::claude_md_names_the_skill` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `--prefix` of `init` | 2 to 8 uppercase Latin letters | as STP-1 | absent with a config: ignored | as STP-1 | exact | `init::rejects_bad_prefix` |
| `[models].<role>.model` | string | one character | `""`: exit 1; no binding: no `model:` line | a first character that is not a letter or digit, a character outside the list of AC-5: exit 1 | written as is | `init_process::refuses_a_bad_model`, `init_process::model_from_the_role` |
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

### Abuse table

Attempts of a careless agent against "the process files say what stapel wrote, and the agents keep their limits".

| Attempt | Outcome |
|---|---|
| The agent sets `permission_mode = "bypassPermissions"` on a role to stop the dialogs | `init` refuses, exit 1, AC-4 |
| The agent puts a second line into a `model` value (`x\npermissionMode: plan`) | `init` refuses, exit 1, AC-5 |
| The agent edits an agent file (adds `Write` to a reviewer's tools) | the hash no longer matches: kept with the warning, AC-8; the edit stays the person's file, not promised to be undone |
| The agent deletes the marker line so that `init` leaves the file alone | kept with the warning, AC-8; not undone |
| The agent replaces a process file with a symlink to a file outside the repository | kept with the warning, never written through, AC-8 |
| The agent runs `stapel hook session-start` in its own shell | the guard denies it (STP-14: `stapel hook` is run only by Claude Code) |
| An old `stapel` on PATH runs `init` over newer files | kept with the newer warning, AC-9 |

**Review Focus:** a process file with CRLF line ends (reads as edited; not promised); a marker line inside
the body only (kept; `init_process::keeps_a_file_without_a_valid_marker`); a model `inherit` (letters,
accepted; `init_process::model_from_the_role` does not cover it, accepted by the character rule); a
`.stapel/stapel.toml` without `[models]` (refused at the parse with exit 1, as `init` does today; not tested
here); the hook run
while `next` is slow (it runs `next` in the process; `next` reads no network; not tested).

**Author self-check:** 2026-10-22, before spec review: every promise item has a criterion and a test
(item 1: AC-1, AC-6; item 2: AC-2 to AC-5; item 3: AC-7 to AC-11; item 4: AC-12 to AC-14; item 5: AC-15 to
AC-17; item 6: AC-18, AC-19); each input has its empty and invalid case; the rows were checked against
`init.rs`, `hook.rs`, `config.rs` and the catalogue; every test runs the binary and asserts new output, or
reads a file that this ticket changes; no open question is left.
2026-10-22, after spec review round 1 (external): 0 HIGH, 6 MEDIUM, 9 LOW; 13 inside, 2 outside, both
decided by the human. One change: the hook is silent only without `stapel.toml` (SR1-1); a dangling symlink
is kept (SR1-6); a model starts with a letter or digit (SR1-13); the warning suppresses `already set up`
(SR1-15); the matcher is a term (SR1-14); `second_run_changes_nothing` moves to step 1 (SR1-4); step 6 fixes
when the human applies `CLAUDE.md` (SR1-5) and removes the local hook (SR1-12); the golden hides the version
(SR1-8); Review Focus and Design corrected (SR1-3, SR1-9).
2026-10-22, during the build, before code review: the drift reviews of steps 1 to 5 gave 35 findings (0 HIGH);
one change: the Abuse table, which a `guard` ticket needs and the spec missed (`ticket_drift` found it after
step 6); AC-9 suppresses `already set up` too (D3B-1); coverage steps 7, 7b and 7c; text fixes in step 8 with
AC-20 and AC-21 (D5-1 to D5-3).
2026-10-22, after code review round 1 (reviewer, external, drift): 0 HIGH, 9 MEDIUM, 17 LOW; one change: AC-2
and AC-6 quote their values, AC-22 installs the catalogues, AC-23 and AC-24 put the dropped rules back and
fix the builder text, AC-19 names the lint file; steps 9, 9b and 9c; Design as built moved in.
2026-10-22, after the drift review of step 9b: 6 findings on the wording (3 MEDIUM); the human chose to fix them
now; AC-25 and step 9d.

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
- `stapel hook session-start` runs `next` in the same process through a function that takes the
  repository root and returns its text instead of printing it, so no second binary is started and the root
  found from `CLAUDE_PROJECT_DIR` is the one `next` reads.
- The texts are drafted by the orchestrator in `.stapel/scratch/stp22/texts/` from `CLAUDE.md`, the ledger
  decision of 2026-10-21 and the briefs of STP-5 to STP-21 before step 5; the builder of step 5 copies them
  into `assets/` and adjusts them only to make its tests pass.

### Decisions (as built)

- `crates/stapel-cli/src/init.rs`: `plan_process_files` plans the five files after the pre-push hook, in
  the order of the term **Process file**; `check_bindings` checks `builder`, `drift` and `reviewer` (mode
  first, then model) before any write; a directory at any process path refuses before any write.
- The frontmatter is built in code; the marker is the last frontmatter line; its hash is the SHA-256 of the
  text without the marker line and its `\n`; the version is `CARGO_PKG_VERSION`.
- `plan_create_text` decides per file with `symlink_metadata`: nothing at the path, created; a symlink,
  dangling or not, kept with the AC-8 warning; a regular file read as UTF-8 with a valid marker and a
  matching hash, updated when its version is not greater and its text differs; greater, kept with the
  AC-9 warning; any other file, kept with the AC-8 warning. A warning sets `kept`, and `run` prints
  `already set up: nothing changed` only when the plan is empty and nothing was kept.
- `unchanged_version` and `parse_version` read the marker; versions compare as numbers, and the newer
  warning prints the parsed version (a marker `0.09.0` prints `0.9.0`).
- The restart note is printed last when `.claude/agents` or `.claude/skills` was missing before planning.
- `plan_hooks` adds the SessionStart entry in the same read and write as the other stapel hooks; a command
  that contains `stapel hook session-start` counts as installed; the label
  `appended: .claude/settings.json (stapel session hook)` only when the settings file existed and the
  session hook is its only change.
- `hook::session_start` takes the start folder from a non-empty `CLAUDE_PROJECT_DIR`, else the working
  folder (an empty value counts as unset, D4B-1), finds the root with `stapel_root`, prints the first line
  when `.stapel/stapel.toml` is a file there, then `stapel next:` and `next::text_at(root)` when that
  succeeds; stdin is not read; a panic is caught; exit 0 always.
- `next::run` prints the lines of a new `lines()`; `next::text_at(root)` serves the hook; `repo::load(root)`
  is shared by `repo::open`. `next.rs` has one print site.
- The bodies are `crates/stapel-cli/assets/skill.md` and `assets/agents/*.md`, taken in with
  `include_str!`; this repository's installed copies are kept equal by AC-18.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `init_process::creates_the_process_files` |
| AC-2 | `init_process::agent_frontmatter`, `init_process::model_from_the_role`, `init_process::marker_holds_the_binary_version` |
| AC-3 | `init_process::permission_mode_of_the_role`, `init_process::reviewer_mode_reaches_both_reviewers`, `init_process::model_change_updates_both_reviewers` |
| AC-4 | `init_process::refuses_a_bad_permission_mode`, `init_process::refusals_on_every_checked_role` |
| AC-5 | `init_process::refuses_a_bad_model` |
| AC-6 | `init_process::skill_frontmatter` |
| AC-7 | `init_process::updates_an_unchanged_file`, `init_process::second_run_changes_nothing`, `init_process::hash_computed_by_hand` |
| AC-8 | `init_process::keeps_an_edited_file`, `init_process::keeps_a_file_without_a_valid_marker`, `init_process::keeps_a_symlink_and_non_utf8`, `init_process::newer_file_with_a_wrong_hash` |
| AC-9 | `init_process::keeps_a_newer_file`, `init_process::newer_file_alone`, `init_process::versions_compare_by_numbers` |
| AC-10 | `init_process::refuses_a_directory`, `init_process::refuses_a_directory_at_any_process_path` |
| AC-11 | `init_process::restart_note`, `init_process::restart_note_for_one_missing_folder` |
| AC-12 | `init_process::installs_the_session_hook`, `init_process::session_hook_next_to_others`, `init_process::session_hook_inside_a_wrapper`, `init_process::session_hook_labels_with_other_changes`, `init_process::session_hook_keeps_every_event` |
| AC-13 | `hook_session::prints_the_skill_line_and_next`, `hook_session::next_fails`, `hook_session::unreadable_config` |
| AC-14 | `hook_session::silent_outside_a_stapel_repository`, `init_process::session_hook_line_without_stapel`, `init_process::session_hook_line_with_a_failing_stapel` |
| AC-15 | `init_process::skill_text_holds_the_process` |
| AC-16 | `init_process::skill_text_holds_the_process` |
| AC-17 | `init_process::agent_texts_hold_their_rules`, `init_process::read_only_agents_say_so` |
| AC-18 | `init_process::this_repository_has_the_current_files` |
| AC-19 | `init_process::claude_md_names_the_skill`, `init_process::claude_md_names_the_ticket_lint` |
| AC-20 | `init_process::skill_text_is_consistent` |
| AC-21 | `init_process::builder_text_forbids_history_rewrites` |
| AC-22 | `init_process::installs_the_catalogues` |
| AC-23 | `init_process::skill_text_keeps_the_old_rules` |
| AC-24 | `init_process::builder_text_for_green_step` |
| AC-25 | `init_process::texts_agree_on_close_lint_and_green_step` |

## Plan

Route: full (tags `interface`, `guard`, `migration`), auto.

1. The five process files are created with their frontmatter and marker, the bodies a placeholder line
   each (AC-1, AC-2, AC-3, AC-6; tests `init_process::creates_the_process_files`,
   `init_process::agent_frontmatter`, `init_process::model_from_the_role`,
   `init_process::permission_mode_of_the_role`, `init_process::skill_frontmatter`,
   `init_process::second_run_changes_nothing`). Must not change: the other files of `init` and their labels (`tests/init.rs`, `tests/hooks_install.rs`).
2. Refusals of a role binding and of a directory (AC-4, AC-5, AC-10; tests
   `init_process::refuses_a_bad_permission_mode`, `init_process::refuses_a_bad_model`,
   `init_process::refuses_a_directory`). Must not change: the files of step 1.
3. A re-run updates stapel's unchanged files, and the restart note (AC-7, AC-11; tests
   `init_process::updates_an_unchanged_file`, `init_process::restart_note`). Must not change: a first run's files.
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
   `init_process::this_repository_has_the_current_files`, `init_process::claude_md_names_the_skill`). After the
   RED commit the human applies the `CLAUDE.md` change with a script the orchestrator writes under
   `.stapel/scratch/stp22/`, and removes the local SessionStart hook with a command the orchestrator gives;
   then the builder runs `stapel init` here and the GREEN commit holds `CLAUDE.md` and the new files. Must not change: the rules of `CLAUDE.md` that stay
   (the plan, language, layout, private terms, thread limits, commit author).
7. Coverage after the drift reviews of steps 1 and 2 (D1-1, D1-2, D2-1, D2-2, D2-3; AC-2, AC-3, AC-4, AC-10):
   new `#[test]` functions `init_process::marker_holds_the_binary_version`,
   `init_process::reviewer_mode_reaches_both_reviewers`, `init_process::refusals_on_every_checked_role`,
   `init_process::refuses_a_directory_at_any_process_path`. Tests only, each helper nested inside its
   function; a test that fails goes to step 8.
7b. Coverage after the drift reviews of steps 3 and 3b (D3-4, D3-5, D3B-2, D3B-3, D3B-4; AC-7 to AC-9,
   AC-11): new `#[test]` functions `init_process::restart_note_for_one_missing_folder`,
   `init_process::hash_computed_by_hand`, `init_process::newer_file_alone`,
   `init_process::versions_compare_by_numbers`, `init_process::newer_file_with_a_wrong_hash`. Tests only, each
   helper nested inside its function; a test that fails goes to step 8.
7c. Coverage after the drift reviews of steps 4 to 5 (D4-2, D4-3, D4-4, D4-6, D4-7, D4B-3, D5-5; AC-12 to
   AC-14, AC-17): new `#[test]` functions `init_process::session_hook_inside_a_wrapper`,
   `init_process::session_hook_labels_with_other_changes`, `init_process::session_hook_keeps_every_event`,
   `init_process::session_hook_line_with_a_failing_stapel`, `hook_session::unreadable_config`,
   `init_process::read_only_agents_say_so`. Tests only, each helper nested inside its function; a test that
   fails goes to step 8.
8. Text fixes after the drift review of step 5 (D5-1, D5-2, D5-3; AC-20, AC-21; tests
   `init_process::skill_text_is_consistent`, `init_process::builder_text_forbids_history_rewrites`): the assets
   change, and the GREEN runs the built `stapel init` here so that AC-18 holds. Must not change: the
   frontmatter, the sentences of AC-15 to AC-17.
9. Fixes after code review round 1 in code (R-1, E-3, E-7, E-10; AC-2, AC-6, AC-22; tests
   `init_process::agent_frontmatter`, `init_process::skill_frontmatter`, `init_process::installs_the_catalogues`): quoted
   `description` and `model`, the two catalogues as assets under `crates/stapel-cli/assets/config/`, the stale
   comments of `init.rs`; the RED changes the golden file; the GREEN runs the built `stapel init` here so that
   AC-18 holds. Must not change: the marker rule and every other label.
9b. Fixes after code review round 1 in the texts (the findings are listed in the Decisions log; AC-19,
   AC-23, AC-24; tests `init_process::skill_text_keeps_the_old_rules`, `init_process::builder_text_for_green_step`,
   `init_process::claude_md_names_the_ticket_lint`): the drafts in `.stapel/scratch/stp22/texts/`; after the RED
   the human applies the `CLAUDE.md` line with a script; the GREEN runs the built `stapel init` here. Must not
   change: the sentences of AC-15, AC-16, AC-20 and AC-21.
9c. Coverage after code review round 1 (E-4; AC-3): new `#[test]` function
   `init_process::model_change_updates_both_reviewers`. Tests only, each helper nested inside its function.
9d. Fixes after the drift review of step 9b (AC-25; test `init_process::texts_agree_on_close_lint_and_green_step`):
   the drafts in `.stapel/scratch/stp22/texts/`; the GREEN runs the built `stapel init` here. Must not change:
   the sentences of AC-15, AC-16, AC-20, AC-21, AC-23 and AC-24.

## Review

Spec review: one round by an external model (15 findings, 0 HIGH, 6 MEDIUM; 13 inside, 2 outside, both
decided by the human). Drift review after every GREEN step of the tagged ticket (steps 1 to 9d: 54 records,
0 HIGH); verification gaps became the coverage steps 7, 7b, 7c and 9c, text contradictions the fix steps 8,
9b and 9d. After step 6 `ticket_drift` found the missing Abuse table (B-3). Code review round 1 by three
reviewers (stapel-reviewer, an external model, stapel-drift on the whole ticket): 20 records, 0 HIGH, 9
MEDIUM (7 inside, 2 outside); the human widened the promise to install the catalogues (E-3) and kept the
ticket whole (DR-1); fixes in steps 9 to 9d. Code review round 2 by one stapel-reviewer on the diff since
round 1: every patch fate done, one MEDIUM (R2-1, deferred to item 1.6). Deferred: D9D-1 (the builder's
commit rule against a RED, to items 1.9 and 1.10), R2-1, B-2 (the Test plan is in no confirmed hash, to
item 1.8), B-4 (builders' heredocs and scripts, to items 1.9 and 1.10). From step 7 on the steps were built
by the installed `stapel-builder` and reviewed by `stapel-drift`; their text gaps (commit subjects, the
whole-file rule) were found that way and fixed in steps 8 and 9b. Every finding with its fate is in
`findings.jsonl`.

## Summary

`stapel init` installs the process as data: the skill `.claude/skills/stapel/SKILL.md`, the agents
`stapel-builder`, `stapel-drift`, `stapel-reviewer` and `stapel-spec-reviewer` with their tools, model and
permission mode, the two catalogues of `.stapel/config/`, and a SessionStart hook that names the skill and
prints `stapel next`. Each file carries a marker with a hash; a re-run updates stapel's unchanged files and
keeps edited, unmarked or newer ones with a warning. This repository runs on the installed files, and
`CLAUDE.md` keeps only its own rules.

Measures against STP-21 (the skill, rule 8):

- Escaped defects of STP-21 found after its close: none known.
- Code review HIGH and MEDIUM inside the promise: 8 over two rounds (STP-21: 5).
- `ticket.md`: 51.7 KB (STP-21: 47.3 KB).
- Size: about 416 source, 1 647 test and 277 text lines against an estimate of 350, 700 and 250 (DR-1); kept
  whole by the human.
- Tokens, measured: orchestrator output 208k, cache read 76.9M (STP-21: 82M) — far from the phase 1 target
  of under 19.75M; builder 66k / 6.6M (STP-21: 83k / 13.0M); drift 12k / 0.3M; external 8k / 1.9M;
  reviewer 5k / 0.2M. A `stapel-builder` run took about 20k to 40k tokens against about 60k for a
  builder with a hand brief.
- Wall time: 2026-10-21 about 15:20 UTC (`stapel new`) to 2026-10-22 about 11:30 UTC, with a night.

Approval dialogs: about 14 (STP-21: 27) — `stapel ok` 5, `stapel findings import` 6 (one dialog shows at
most 30 records, so 65 records took three), `stapel ledger add` 3, two scripts the human ran for
`CLAUDE.md`. Two were avoidable: a ledger record withdrawn the same day, and a `stapel ok` for a brief that
grew after the findings import.

What improved: the installed agents work with a three-line request; the builder's tokens halved; dialogs
fell by about half. What did not: the orchestrator's context stays the main cost; builders still wrote
files through heredocs or scripts (four times); text reviews looped on wording for three rounds.
