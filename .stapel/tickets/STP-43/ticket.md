# STP-43 — The guards folder and the external reviewer after their pilots

## Description

Source: `docs/PLAN.md`, phase 5, item 5.1b (ledger decisions of 2026-11-24); the deferred findings
STP-41 SR1-7, SR2-10 and XR1-5, STP-42 XR1-3 and CR1-4.

### Frame

**Problem.** A `[guards] dir` such as `tests` loads, and then every builder write under it is denied. A
guard can read a script outside the folder, which an agent edits with no dialog. The external reviewer's
findings say the role `reviewer`. The starter binds `external` to `anthropic-api`, though the model goes
into a Claude Code agent header.

**Plan item:** 5.1b.

**Risk tags:** `guard` (what `[guards] dir` may be, the warning about a guard's reach), `interface` (a
refusal of `stapel.toml`, a warning line of `stapel check` and `stapel init`, a refusal of `stapel init`).

**Non-goals.** Reading the files a command opens at run time (implicit configs such as `.eslintrc` that a
linter reads unnamed); a format of guard files; moving the guards of a project; a second body text for the
external reviewer; a provider other than Claude Code for an agent role; a change of the hook's write rules.

**Promise (closed list).**

1. Overlap: loading `stapel.toml` fails when `[guards] dir` is one folder part from the list `src`,
   `tests`, `test`, `docs`, `lib`, `crates`, `app`, `packages`, `spec` (any letter case, with or without a
   trailing `/`). A nested folder such as `tests/guards` loads.
2. Reach: `stapel check` (at the levels that run guards) and `stapel init` print, for each guard, one
   warning line per distinct script or config of its command that is an existing regular file of the
   working tree outside `dir`; the run goes on. The skill and the starter comment say that guard scripts and their configs
   belong in `dir`.
3. Role: `stapel init` writes the agent's own role into the finding template of the reviewer body, so
   `stapel-reviewer` says `reviewer` and `stapel-external-reviewer` says `external`; one body text in the
   assets.
4. Provider: the starter binds `external` with `provider = "claude-code"`; `stapel init` refuses, before
   it writes anything, an agent role (`builder`, `drift`, `reviewer`, `depends`, `external`) bound to
   another provider, and names the line to change. This repository's `stapel.toml` is changed to match.

Not promised: a path in a variable, a glob or a command substitution of a guard's command; an absolute
path or a path with an empty, `.` or `..` part; a script that a named script calls; a file or folder
named as the subject of a check (`grep -rn TODO src`, `grep TODO README.md`); a config named other than
by an option whose name holds `config`; a script behind a wrapper (`FOO=1 x.sh`, `env`, `time`, `exec`,
`xargs`, `if … then`); a first word naming a root file (`make`) warns, which is accepted; one file
spelled in two letter cases; control characters in `<word>` and `<dir>`; the content of the check copy at HEAD when it differs from the
working tree; the roles of findings already written; a provider check of roles that have no agent file
(`author`, `cheap` and others).

**Prior art.** STP-41 (the guards folder, its write rule and its load checks); STP-42 (the agent
`stapel-external-reviewer`, `AGENT_ROLES`, `check_bindings`); STP-26 (`[roles]`); STP-22 (the model of
a role written whatever its provider).

**Size.** About 150 source lines and 300 test lines; 5 Plan steps, 11 after the fix and coverage steps. Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-24: Decision (overlap, the human, recommended): refuse at load a `dir` that is one folder of a
  fixed list; a nested folder loads. Rejected: a warning only; an overlap computed from the `[check]` test
  globs and the tracked files.
- 2026-11-24: Decision (overlap, agent, SR1-1): the rule "a `dir` that covers an `always_writable` entry"
  of the first answer is dropped: the guards dialog already wins over `always_writable`, `docs` is a work
  folder, and the tests of STP-41 AC-12 set `dir = "checks"` with `checks/` writable on purpose.
- 2026-11-24: Decision (reach, the human, recommended): a warning in `stapel check` and `stapel init` for a
  file outside `dir` that a command names, and a line in the skill and the starter. Rejected: the text
  only; a refusal at load.
- 2026-11-24: Decision (reach, agent, SR1-3): only reach words warn (a script or a config position), so a
  file named as the subject of a check gives no lasting warning.
- 2026-11-24: Decision (role, the human, recommended): `stapel init` fills the role into one reviewer body.
  Rejected: a second body file.
- 2026-11-24: Decision (provider, the human, recommended): `stapel init` refuses an agent role bound to a
  provider other than `claude-code`. Rejected: a warning. This supersedes the decision of STP-22 (SR1-11,
  2026-10-22) that wrote the model of a role whatever its provider; the refusal names the line to change.
- 2026-11-24: Assumption: a refusal at load also refuses an agent's edit of `stapel.toml` that sets such a
  `dir`, because the hook loads the edited text before its dialog (STP-38).

## Spec

Terms:

- **Work folders**: the list `src`, `tests`, `test`, `docs`, `lib`, `crates`, `app`, `packages`, `spec`.
- **Covers**: folder A covers path B when the parts of A, compared in lower case, are the first parts of
  B; a trailing `/` of either is ignored.
- **Command word**: a piece of a guard's command after it is split at white space and at each of
  `; | & ( ) < >` and a backtick, with every `'` and `"` then removed from it (`--config="clippy.toml"`
  is the word `--config=clippy.toml`); an `&` right after `>` or `<` (`2>&1`) is part of the
  redirection and starts no word.
- **Reach word**: a command word that is (a) the first word of the command or the first word after `;`,
  `|`, `&`, `(` or a backtick; (b) the first word not starting with `-` after a word of kind (a) that is
  one of the interpreters `sh`, `bash`, `zsh`, `node`, `python`, `python3`, `ruby`, `perl` (`grep -w sh
  x.sh` has none); or (c) the value of an option whose
  name holds `config`, given as `--<name>=<value>` or as the next word.
- **Outside file**: a reach word, with a leading `./` removed, that is a relative path (no empty, `.` or
  `..` part, no leading `/`, no `$`, `*`, `?`, `~`, `{`), names a regular file under the root of the git
  working tree of the current folder (links followed; a path whose metadata cannot be read is no file),
  and is not covered by `[guards] dir`.
- **Reach warning**: `warning: guard <name> reads <word> outside <dir>; a guard's scripts and configs belong in <dir>`,
  with `<name>` printed as stapel prints names (control characters escaped), `<word>` without its
  leading `./` and `<dir>` as written in `stapel.toml`; one line per distinct
  outside file of each guard, in command order, guards in their order.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF `[guards] dir` is one part that is a work folder (any letter case, with or without a trailing `/`) THEN loading `stapel.toml` fails with `guards.dir <dir> is a folder that agents write: name a folder of its own, such as guards`, `<dir>` as written; the lexical check of STP-41 runs first and the `[check]` check after; WHEN `dir` has two or more parts (`tests/guards`, `src/x`) or another name THE it loads as now. | `check_guards::refuses_an_overlapping_dir`, `check_guards::all_work_folders_refused` |
| AC-2 | IF an agent's edit of `stapel.toml` sets `dir` to a work folder THEN the hook denies it with `.stapel/stapel.toml would not load: ` and the AC-1 message. | `hook_guards::an_overlapping_dir_edit_is_denied` |
| AC-3 | WHEN `stapel check` runs its guards (levels `head` and `full`) THE it prints the reach warnings on standard error and the run goes on with the same outcomes and record; at level `steps` it prints none; a command with no outside file prints none. | `check_guards::warns_about_a_file_outside_dir`, `check_guards::reach_word_coverage`, `check_guards::interpreter_only_at_command_start` |
| AC-4 | WHEN `stapel init` runs in a repository whose `stapel.toml` has guards THE, after every refusal check and before it writes, it prints the reach warnings on standard error and writes as before; IF init refuses (a binding, a directory at a process path) THEN no reach warning is printed. | `init_process::guard_reach_warning` |
| AC-5 | WHEN `stapel init` writes the starter and the skill THE the starter's line `# [guards]` ends with the comment `# a guard's scripts and configs belong in dir: an agent edits a file outside it with no dialog`, so the starter stays valid TOML when its commented lines are uncommented, and the skill's line on the guards folder holds the sentence `A guard's scripts and configs belong in the guards folder; an agent edits a file outside it with no dialog.` | `init_process::guard_reach_text` |
| AC-6 | WHEN `stapel init` writes the reviewer body for an agent THE the finding template holds `"role":"<role>"` with the agent's role: `reviewer` for `stapel-reviewer`, `external` for `stapel-external-reviewer`; the two bodies differ only there; the asset holds one body. | `init_process::reviewer_role_in_the_finding_template`, `init_process::external_reviewer_agent` |
| AC-7 | IF an agent role (`builder`, `drift`, `reviewer`, `depends`, `external`) of `[models]` has a provider other than `claude-code` THEN `stapel init` exits 1 with `stapel: [models] <role>: provider "<p>" is not claude-code: an agent role runs as a Claude Code agent; set provider = "claude-code"` and writes nothing; the provider of a role is checked before its mode and model, and the roles in the order of that list; a role without an agent file (`author`, `cheap`) is not checked. | `init_process::refuses_a_foreign_provider`, `init_process::first_foreign_role_is_named` |
| AC-8 | WHEN `stapel init` writes a new `stapel.toml` THE its `external` binding has `provider = "claude-code"`. | `init_process::starter_binds_claude_code` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-1 and AC-2, item 2 by AC-3 to AC-5, item 3 by
  AC-6, item 4 by AC-7 and AC-8. Unchanged: the other load checks of `[guards]`
  (`check_guards::refuses_a_bad_dir`, `check_guards::refuses_bad_entries`); the write rules of the hook
  for the guards folder (`hook_guards::guards_folder_edit_asks`); the outcome and record of every guard;
  the bodies of the other agents. Changed on purpose: a `stapel.toml` with such a `dir` that loaded
  before now fails in every command; a starter of an older stapel with `external` on `anthropic-api` now
  makes `stapel init` refuse until the line is changed; this repository's `[models] external` gets
  `provider = "claude-code"`.
- **Not promised:** the Frame's list; a work folder of another name (`source`, `__tests__`, `scripts`); a
  word quoted with a space inside it; a file created after the warning is printed.

**Author self-check (item 7).** Done on 2026-11-24: each promise item maps to a criterion; the pilot's
`tests/guards` has two parts and loads (AC-1); the hook already loads the edited text (STP-38), so AC-2
needs no hook code; no existing test sets a work folder as `dir`.

## Test plan

Integration tests in `crates/stapel-cli/tests/`: the existing `check_guards.rs` (fixtures `stapel_repo`,
`add_config`, `guards_repo`), `hook_guards.rs` and `init_process.rs` (fixtures `repo_with_config`,
`replace_line`, `refused_binding`). Light tests: `stapel cargo test -j 4 -p stapel-cli --test
check_guards -- --test-threads=4` (and `--test hook_guards`, `--test init_process`); before each GREEN
also `--test ticket_drift` and `--test fixtures`.

| Category | Cases | Tests |
|---|---|---|
| Overlap | `tests`, `Tests/`, `src`, `docs`, `crates`, `packages` refused with the message naming `dir` as written; `tests/guards`, `src/x`, `guards`, `testing` load; `dir = "tests"` without `[check]`: the AC-1 message | `check_guards::refuses_an_overlapping_dir` |
| Hook edit | the main agent in a dialog mode edits `stapel.toml` to `dir = "tests"`: denied with the load message; to `dir = "tests/guards"`: asks as now | `hook_guards::an_overlapping_dir_edit_is_denied` |
| Reach in check | `sh scripts/cov.sh`: one warning; `./scripts/cov.sh`: warns for `scripts/cov.sh`; `cargo clippy --config=clippy.toml`, `--config="clippy.toml"` and `--config clippy.toml`: warn for `clippy.toml`; the same file twice in one command: one line; the same file in two guards: one line each; `sh guards/run.sh`: none; `grep -rn TODO src` (a folder) and `grep TODO README.md` (a subject file): none; a missing file, a dangling link, `$HOME/x`, `*.rs`, `/abs/x`, `../x`, `scripts/./cov.sh`: none; a link `scripts/l.sh` to a file: warns; level `steps`: none; outcomes and the record are those without the warning | `check_guards::warns_about_a_file_outside_dir` |
| Reach in init | a guard reading `scripts/cov.sh`: init prints the warning and writes its files; the same repository with `external` on `anthropic-api`, and with a directory at `.claude/skills/stapel/SKILL.md`: the refusal and no warning | `init_process::guard_reach_warning` |
| Texts | the starter's `# [guards]` line with its comment, and the skill sentence, installed and in the assets | `init_process::guard_reach_text` |
| Role | the installed `stapel-reviewer.md` holds `"role":"reviewer"`, `stapel-external-reviewer.md` holds `"role":"external"`, and the two bodies are equal after that one replacement | `init_process::reviewer_role_in_the_finding_template`, `init_process::external_reviewer_agent` |
| Provider | `external` and `builder` with `anthropic-api`: refused, nothing written; `reviewer` with `anthropic-api` and a bad mode: the provider message; `author` with `anthropic-api`: init runs | `init_process::refuses_a_foreign_provider` |
| Starter | a new `stapel.toml`: the `external` line holds `provider = "claude-code"` and `model = "claude-opus-5-5"` | `init_process::starter_binds_claude_code` |
| All folders | each of the nine work folders refused; `tests/guards`, `src/x`, `guards` load with no error at all | `check_guards::all_work_folders_refused` |
| Reach coverage | a script after `\|`, `&&`, `(` and a backtick warns; `node`, `python`, `ruby` scripts warn; `sh -e scripts/cov.sh` warns; `sh src` (a folder) gives none; the guard part of the `runs.jsonl` record equals that of a run without outside files | `check_guards::reach_word_coverage` |
| Interpreter place | `grep -w sh scripts/x.sh` and `grep TODO 2>&1` with a root file `1`: no warning | `check_guards::interpreter_only_at_command_start` |
| Role order | `builder` and `external` both on `anthropic-api`: the message names `builder`; `drift` alone and `depends` alone: refused | `init_process::first_foreign_role_is_named` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[guards] dir` | TOML string | one part / any parts | refused as now (STP-41) | a work folder: load error | exact parts, lower case | `check_guards::refuses_an_overlapping_dir` |
| `[[guards.check]] command` | TOML string | one word / any | refused as now (STP-41) | none: only outside files warn | exact words | `check_guards::warns_about_a_file_outside_dir` |
| `[models] <role> provider` | TOML string | — | absent: a TOML error as now | not `claude-code` for an agent role: init refuses | exact | `init_process::refuses_a_foreign_provider` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| os | a reach word naming an existing file, a folder, a missing path, a link to a file, or a dangling link | warns for a file and a link to a file, not for the rest | `check_guards::warns_about_a_file_outside_dir` |
| os | a path whose metadata cannot be read (no permission, a link loop) | no file: no warning, the run goes on | not promised beyond the dangling link case |
| os | a file system with case-sensitive names and a `dir` that differs only in case | compared in lower case, as the hook does | `check_guards::refuses_an_overlapping_dir` |
| git | a script in the working tree and not at HEAD, or the reverse | the working tree decides; the check copy is not read | not promised |
| claude-code | an agent edit of `stapel.toml` | denied when the text would not load | `hook_guards::an_overlapping_dir_edit_is_denied` |
| claude-code | the agent file of a role bound to another provider | refused by init | `init_process::refuses_a_foreign_provider` |
| shell | variables, globs, command substitution, scripts called by a script | not promised | — |

### Abuse

| Attempt of a careless agent | Outcome | Test |
|---|---|---|
| sets `dir = "tests"` in `stapel.toml` through Edit | denied: the text would not load | `hook_guards::an_overlapping_dir_edit_is_denied` |
| writes a guard command that runs `scripts/cov.sh` outside the folder, then edits the script | the person sees the reach warning at each check and at init; the script edit is not stopped (not promised) | `check_guards::warns_about_a_file_outside_dir` |
| binds `external` to `anthropic-api` and runs `stapel init` | refused, nothing written | `init_process::refuses_a_foreign_provider` |

**Review Focus:** a guard command with a path in a variable (not promised, Frame); a work folder of
another name (not promised, Guarantees); a path whose metadata cannot be read (handled as no file, no
test but the dangling link); `stapel init` in a repository whose `stapel.toml` fails loading (unchanged:
init refuses, `init_process::refuses_a_bad_permission_mode` covers the path).

## Design

### Facts

- F-1: `Config::validate` in `crates/stapel-core/src/config.rs` checks `[guards] dir` lexically and
  refuses `.stapel`, `.git` and `.claude`, then refuses guards without `[check]`.
- F-2: the hook loads an edited `stapel.toml` with `Config::parse_at` and denies it with
  `.stapel/stapel.toml would not load: <error>` (`crates/stapel-core/src/guard.rs`, STP-38).
- F-3: `crates/stapel-cli/src/check.rs` runs the guards after the project commands at levels `head` and
  `full` through `runner.guards(dir, entries)`, in a check copy whose tracked files are those of HEAD.
- F-4: `crates/stapel-cli/src/init.rs` (`AGENT_FILES`) gives `reviewer` and `external-reviewer` the same
  body `crates/stapel-cli/assets/agents/reviewer.md`, whose template line says `"role":"reviewer"`;
  `check_bindings` refuses bad modes and models of `AGENT_ROLES` before anything is written.
- F-5: the field `provider` of a binding is read by no code; the starter
  `crates/stapel-core/src/stapel.toml` binds `external` to `anthropic-api`.
- F-6: `crates/stapel-cli/tests/hook_guards.rs` sets `dir = "checks"` with `checks/` in
  `always_writable` (STP-41 AC-12); no test sets a work folder as `dir`.

### Shape

- `stapel-core::config`: `WORK_FOLDERS` and a check in `validate` between the lexical one and the
  `[check]` one.
- `check.rs`: a private `command_words` and a `pub(crate)` function `guard_reach(root, dir, entries) ->
  Vec<String>` that returns the reach warnings, used by `check.rs` and `init.rs`.
- `check.rs`: before the guards run, print each line of `guard_reach` for the root of the working tree.
- `init.rs`: print each line of `guard_reach` after the whole plan is checked, right before the first
  write; `plan_process_files` replaces
  `@ROLE@` in the body with `agent.role`; `check_bindings` refuses a provider other than `claude-code`
  first for each role.
- The reviewer asset: `"role":"@ROLE@"`; the skill asset and `GUARDS_COMMENT`: the sentences of AC-5;
  the starter: `external = { provider = "claude-code", … }`.

### Decisions (before the build)

- 2026-11-24, agent: the reach warning goes to standard error, so the check's standard output and record
  stay as they are.
- 2026-11-24, agent (SR1-2): the reach is computed on the working tree of the current folder, before the
  check copy, because the warning is for the person who edits that tree.
- 2026-11-24, agent: this repository's `.stapel/stapel.toml` gets `provider = "claude-code"` for
  `external` from the orchestrator through its dialog before the GREEN of step 5, because a builder may
  not edit it and `stapel init` refuses without it.

### Decisions (as built)

- Step 1 (defb165, 58bbe4b): the work-folder check sits in `Config::validate` after the lexical check
  and before `guards need [check]`; it compares the lower-cased `dir` without its trailing `/`.
- Step 2 (6b0d3cb, 14be84b): `command_words` (private) and `guard_reach(root, dir, entries)`
  (`pub(crate)`) live in `crates/stapel-cli/src/check.rs`, not in `stapel-core`, because only the binary
  uses them; check prints the lines with `warn!` before the guards run.
- 2026-11-24, agent: the link cases of `check_guards::warns_about_a_file_outside_dir` live in the
  ignored `target/` folder, because `stapel check` refuses an untracked link in the working tree; the
  link guard runs `sh target/l.sh || true`, as ignored files are absent from the check copy.
- Step 3 (f56326e, 01cf32f): init prints the reach warnings after every refusal check, right before its
  write loop.
- 2026-11-24, agent: at step 3 `init_process::guard_reach_warning` tested the refusal with
  `permission_mode bypassPermissions`, because the provider check came only in step 5; the step 5 RED
  added the `anthropic-api` case with a `history:` line.
- Step 4 (0ef3ada, 0d05333): `"role":"@ROLE@"` in the reviewer asset, replaced with `agent.role` in
  `plan_process_files`.
- 9319e25: this repository's `[models] external` gets `provider = "claude-code"`.
- Step 5 (d4b6688, 96b6926): `check_bindings` checks the provider of each role before its mode and
  model.
- 2026-11-24, agent: the check of 96b6926 failed the suite test `init::writes_commented_check_otherwise`:
  it uncomments every starter line after `# [check]`, and the own line of step 3 then is no TOML. AC-5
  now puts the comment at the end of the line `# [guards]` (step 6).
- Step 6 (1154bf5, 8e0549f): `GUARDS_COMMENT` ends its `# [guards]` line with the AC-5 comment.
- Step 7 (58960ba, 5cacf2a): `guard_reach` lower-cases both the reach word and `dir` before the Covers
  test.
- 2026-11-24, agent (code review round 2): rule (b) now needs the interpreter at a command start, and
  `2>&1` starts no word (step 11, CR2-1, XR2-6, XR2-7); the test gaps of the round are coverage steps 8
  to 10.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check_guards::refuses_an_overlapping_dir`, `check_guards::all_work_folders_refused` |
| AC-2 | `hook_guards::an_overlapping_dir_edit_is_denied` |
| AC-3 | `check_guards::warns_about_a_file_outside_dir`, `check_guards::reach_word_coverage`, `check_guards::interpreter_only_at_command_start` |
| AC-4 | `init_process::guard_reach_warning` |
| AC-5 | `init_process::guard_reach_text` |
| AC-6 | `init_process::reviewer_role_in_the_finding_template`, `init_process::external_reviewer_agent` |
| AC-7 | `init_process::refuses_a_foreign_provider`, `init_process::first_foreign_role_is_named` |
| AC-8 | `init_process::starter_binds_claude_code` |

## Plan

Route: full (tags `guard`, `interface`), auto.

1. Overlap (AC-1, AC-2; tests `check_guards::refuses_an_overlapping_dir`,
   `hook_guards::an_overlapping_dir_edit_is_denied`): the RED adds both tests; the GREEN adds the check
   to `Config::validate`. Must not change: `check_guards::refuses_a_bad_dir`,
   `hook_guards::guards_folder_edit_asks`.
2. Reach in check (AC-3; test `check_guards::warns_about_a_file_outside_dir`): the RED adds the test; the
   GREEN adds `guard_reach` and prints its lines in `check.rs`. Must not change: the guard outcomes and
   records.
3. Reach in init and the texts (AC-4, AC-5; tests `init_process::guard_reach_warning`,
   `init_process::guard_reach_text`): the RED adds both tests; the GREEN prints the lines in `init.rs`,
   changes the starter comment and the skill asset, and runs `stapel init`. Must not change:
   `init_process::starter_names_guards`.
4. Role (AC-6; tests `init_process::reviewer_role_in_the_finding_template`,
   `init_process::external_reviewer_agent`): the RED adds the first test and changes the body assertion
   of the second, which then fails; the GREEN puts `@ROLE@` into the asset, replaces it in `init.rs` and
   runs `stapel init`. Must not change: the other agent bodies.
5. Provider (AC-7, AC-8; tests `init_process::refuses_a_foreign_provider`,
   `init_process::starter_binds_claude_code`): the RED adds both tests; the orchestrator changes this
   repository's binding before the GREEN; the GREEN adds the check to `check_bindings` and the starter
   line. Must not change: `init_process::refusals_on_every_checked_role`.
6. Fix of the starter line (the suite test `init::writes_commented_check_otherwise`; AC-5; test
   `init_process::guard_reach_text`): the RED changes the test to expect the comment at the end of the
   line `# [guards]`, with a `history:` line; the GREEN moves it there in `GUARDS_COMMENT`. Must not
   change: `init::writes_commented_check_otherwise`, `init_process::starter_names_guards`.
7. Case of `dir` (DR1-1; AC-3; test `check_guards::warns_about_a_file_outside_dir`): the RED adds
   `dir = "Guards"` with `sh guards/run.sh`: no warning (`history:` line); the GREEN compares in lower
   case.
8. Coverage of the work folders (CR2-3, XR2-4; AC-1; test `check_guards::all_work_folders_refused`): a
   coverage commit only.
9. Coverage of the reach words (DR2-1, XR2-1, XR2-5; AC-3; test `check_guards::reach_word_coverage`): a
   coverage commit only.
10. Coverage of the role order (CR2-2, DR2-2, XR2-5; AC-7; test
   `init_process::first_foreign_role_is_named`): a coverage commit only.
11. Interpreter place (CR2-1, XR2-6, XR2-7; AC-3; test `check_guards::interpreter_only_at_command_start`):
   the RED adds the test; the GREEN changes `command_words` and `guard_reach`.

## Review

<!-- stapel:review:begin -->
Generated by stapel; edits between the markers are overwritten.

Findings:

| Stage | Round | Roles | HIGH | MEDIUM | LOW | False | Fates |
|---|---|---|---|---|---|---|---|
| spec-review | 1 | external | 1 | 4 | 4 | 0 | defer 1, patch 8 |
| spec-review | 2 | external | 0 | 1 | 5 | 0 | patch 6 |
| code-review | 1 | drift | 0 | 0 | 1 | 0 | patch 1 |
| code-review | 2 | drift, external, reviewer | 0 | 2 | 14 | 0 | defer 2, no-change 1, patch 13 |

Check runs:

| Level | Result | Commit | At | Steps |
|---|---|---|---|---|
| head | pass | 5cacf2a | 2026-11-24T20:53:28Z | pass 7 |
<!-- stapel:review:end -->

## Summary

TODO
