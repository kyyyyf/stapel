# STP-46 — The guards folder checks after STP-43

## Description

Source: `docs/PLAN.md`, phase 5, item 5.2b (ledger decisions of 2026-11-25 and 2026-11-27); the deferred
findings STP-43 SR1-9 and XR2-8.

### Frame

**Problem.** A `[guards] dir` named `scripts`, `tools`, `pkg`, `cmd`, `internal` or `include` loads, and then
every builder write under it asks the person, because builders write there as work. In a multi-line guard
command, the word after a newline is not a command start, so a script that the second line runs gives no
reach warning.

**Plan item:** 5.2b.

**Risk tags:** `guard` (what `[guards] dir` may be, the reach warning of a guard), `interface` (a refusal
of `stapel.toml`, the warning lines of `stapel check` and `stapel init`).

**Non-goals.** Work folders other than the six of the plan item; a work folder found from the project's
tracked files; a change of the reach warning's text; a shell parser (here-documents, quotes that span
lines, `if … then`); the deferred findings of STP-43 other than SR1-9 and XR2-8.

**Promise (closed list).**

1. Work folders: loading `stapel.toml` fails, with the refusal of STP-43 AC-1, when `[guards] dir` is one
   folder part from the list `scripts`, `tools`, `pkg`, `cmd`, `internal`, `include` (any letter case, with
   or without a trailing `/`), in addition to the nine folders of STP-43. A nested folder such as
   `tools/guards` loads. An agent's edit of `stapel.toml` that sets such a `dir` is denied, as in STP-43 AC-2.
2. Newline: in a guard command, a newline separates commands like `;`, so the first word after it is a
   command start for the reach warning of STP-43 (`stapel check` and `stapel init`); a newline right
   after an unescaped `\` is a continuation and separates nothing.

Not promised: what STP-43 did not promise (a path in a variable, a glob, a wrapper, and the rest of its
list); a carriage return alone as a separator; a newline inside quotes; the meaning of a word after a
newline that a shell would read as data (a here-document body).

**Prior art.** STP-41 (the guards folder and its load checks); STP-43 (the work-folder refusal, the reach
words, `command_words` and `guard_reach` in `crates/stapel-cli/src/check.rs`); the list is in
`crates/stapel-core/src/config.rs`.

**Size.** About 20 source lines and 80 test lines; 2 Plan steps. Route: full, auto (risk tag `guard`).

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-27: Decision (order, the human): 5.2b is taken now, before the rest of 5.2 (ledger 2026-11-27).
- 2026-11-27: Decision (backslash, the human, recommended): a newline right after `\` continues the
  command, as in a shell; the word after it is not a command start. Rejected: every newline separates.
- 2026-11-27: Decision (folders, the human, recommended): exactly the six folders of the plan item.
  Rejected: also `bin`, `benches`, `examples`, `vendor`, `build`.

## Spec

Terms (the other terms are those of STP-43: covers, reach word, outside file, reach warning):

- **Work folders**: the nine of STP-43 (`src`, `tests`, `test`, `docs`, `lib`, `crates`, `app`,
  `packages`, `spec`) and the six new ones `scripts`, `tools`, `pkg`, `cmd`, `internal`, `include`.
- **Command word**: as in STP-43, and in addition a newline (`\n`) splits the command like `;`, so the
  word after it is a command start; a newline right after an odd number of `\` is a **continuation**:
  the last `\` and the newline are white space, so the next word keeps the state of the word before the
  `\` (a command start after `&&` or `;`, the script of an interpreter after `sh`); after an even number
  of `\` (an escaped `\`) the newline separates.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF `[guards] dir` is one part that is a new work folder (`scripts`, `tools`, `pkg`, `cmd`, `internal`, `include`; any letter case, with or without a trailing `/`) THEN loading `stapel.toml` fails with the message of STP-43 AC-1, `guards.dir <dir> is a folder that agents write: name a folder of its own, such as guards`; `<dir>` as written (`Tools/`, `INTERNAL`); a `dir` with an empty part (`tools//`) keeps the lexical refusal of STP-41; WHEN `dir` has two or more parts (`tools/guards`, `scripts/x`) or another name (`toolsx`) THE it loads as now, with the same stderr as `dir = "guards"`. | `check_guards::six_more_work_folders_refused` |
| AC-2 | IF an agent's edit of `stapel.toml` sets `dir` to a new work folder THEN the hook denies it with `.stapel/stapel.toml would not load: ` and the AC-1 message. | `hook_guards::a_new_work_folder_edit_is_denied` |
| AC-3 | WHEN a guard command holds a newline that is not a continuation THE `stapel check` takes the first word after it as a command start, so a script outside `dir` that the next line runs (`sh scripts/cov.sh`, `scripts/cov.sh`, `node scripts/cov.js`) gives the reach warning; empty lines and a `;` before the newline change nothing. | `check_guards::newline_starts_a_command` |
| AC-4 | IF a newline is a continuation THEN the `\` and the newline start no command and no word, so the word after them is a reach word exactly when it would be one with the `\` and the newline replaced by a space: `cargo fmt && \` then `scripts/cov.sh` and `sh \` then `scripts/cov.sh` warn, `cargo test \` then `scripts/cov.sh` does not; IF the newline follows an escaped `\` (`echo a\\`) THEN it separates as in AC-3. | `check_guards::backslash_newline_continues` |
| AC-5 | WHEN `stapel init` runs in a repository whose guard command runs an outside script on its second line THE it prints the reach warning for it, as in STP-43 AC-4. | `init_process::guard_reach_after_newline` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-1 and AC-2, item 2 by AC-3 to AC-5. Unchanged:
  the nine work folders of STP-43 (`check_guards::all_work_folders_refused`); the reach warnings of a
  one-line command (`check_guards::warns_about_a_file_outside_dir`, `check_guards::reach_word_coverage`,
  `check_guards::interpreter_only_at_command_start`); the outcome and record of every guard. Changed on
  purpose: a `stapel.toml` with `dir` set to one of the six folders that loaded before now fails in every
  command, and the person recovers by a hand edit of `dir` (the hook lets an edit through when the edited
  text loads); a multi-line guard command can give reach warnings it did not give before.
- **Not promised:** the Frame's list; a newline inside quotes; a carriage return alone; a `\` followed by
  `\r\n` (the newline then separates); a continuation inside a word (`scripts/\` then a newline then
  `cov.sh` is two words here, one path in a shell).

**Author self-check (item 7).** Done on 2026-11-27: each promise item maps to a criterion; the hook
already loads the edited text (STP-38), so AC-2 needs no hook code; `guard_reach` is shared by check and
init, so AC-5 needs no init code; a TOML basic multi-line string (`"""`) removes a line-ending `\` and the
newline before stapel reads the command, so a continuation reaches stapel only from a literal string
(three single quotes) or an escaped `\\n`; no fixture of this repository sets one of the six folders as `dir`.
After the spec review (2026-11-27): today a lone `\` is pushed as a word and takes the command start or
the interpreter's script place, so the `&& \` and `sh \` cases fail at RED and pass after the change
(SR1-1, SR1-2); an escaped `\` before a newline separates (SR1-3).

## Test plan

Integration tests in `crates/stapel-cli/tests/`: the existing `check_guards.rs` (fixtures `stapel_repo`,
`add_config`, `guards_repo`), `hook_guards.rs` and `init_process.rs`. Light tests: `stapel cargo test -j 4
-p stapel-cli --test check_guards -- --test-threads=4` (and `--test hook_guards`, `--test
init_process`); before each GREEN also `--test ticket_drift` (the ticket lint) and `--test fixtures`
(the private-data test).

| Category | Cases | Tests |
|---|---|---|
| New folders | each of the six refused as written, upper case and with a trailing `/` (`Tools/`, `INTERNAL` among them), the message naming `dir` as written; `tools//`: the lexical refusal of STP-41, not the work-folder one; `tools/guards`, `scripts/x`, `toolsx` load with the same stderr as `guards` | `check_guards::six_more_work_folders_refused` |
| Hook edit | the main agent in a dialog mode edits `stapel.toml` to `dir = "scripts"`: denied with the load message; to `dir = "tools/guards"`: asks as now | `hook_guards::a_new_work_folder_edit_is_denied` |
| Newline | `cargo fmt --check` then a newline then `sh scripts/cov.sh`: one warning; the second line `scripts/cov.sh` alone and `node scripts/cov.js`: one warning each; an empty line and `true;` before the newline: the same warning; a `\r\n` line end: the same warning; a second line `grep -w sh scripts/x.sh`: none | `check_guards::newline_starts_a_command` |
| Continuation | `cargo fmt && \` then a newline then `scripts/cov.sh`: one warning; `sh \` then a newline then `scripts/cov.sh`: one warning; `echo a\\` then a newline then `scripts/cov.sh`: one warning; `cargo test \` then a newline then `scripts/cov.sh`: none; `cargo clippy \` then a newline then `--config clippy.toml`: warns for `clippy.toml`; a `\` that ends the whole command: none; no `\r` in this test | `check_guards::backslash_newline_continues` |
| Init | a guard `cargo fmt --check` then a newline then `sh scripts/cov.sh`: init prints the warning and writes its files | `init_process::guard_reach_after_newline` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[guards] dir` | TOML string | one part / any parts | refused as now (STP-41) | a work folder: load error | exact parts, lower case | `check_guards::six_more_work_folders_refused` |
| `[[guards.check]] command` | TOML string | one line / any lines | refused as now (STP-41) | none: only outside files warn | exact words; `\n` and a `\` before it | `check_guards::newline_starts_a_command` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| os | a file system with case-sensitive names and a `dir` that differs only in case | compared in lower case, as STP-43 does | `check_guards::six_more_work_folders_refused` |
| os | line ends of the command: `\n`, `\r\n`, `\r` | `\n` separates (a `\r` before it is white space); a `\r` alone is white space, not promised as a separator | `check_guards::newline_starts_a_command` |
| claude-code | an agent edit of `stapel.toml` | denied when the text would not load | `hook_guards::a_new_work_folder_edit_is_denied` |
| shell | a newline inside quotes, a here-document, `if … then` over lines | not promised | — |

### Abuse

| Attempt of a careless agent | Outcome | Test |
|---|---|---|
| sets `dir = "scripts"` in `stapel.toml` through Edit | denied: the text would not load | `hook_guards::a_new_work_folder_edit_is_denied` |
| writes a two-line guard command whose second line runs `scripts/cov.sh`, then edits the script | the person sees the reach warning at each check and at init; the script edit is not stopped (not promised, as in STP-43) | `check_guards::newline_starts_a_command` |

**Review Focus:** a newline inside quotes (not promised, Guarantees); a `\` that ends the whole command
(no newline after it: a word `\`, never a file, so no warning; `check_guards::backslash_newline_continues`
adds this case); a work folder of another name (not promised, Frame).

## Design

### Facts

- F-1: `Config::validate` in `crates/stapel-core/src/config.rs` refuses a `dir` that is one of the nine
  work folders, compared lower-cased without a trailing `/`, after the lexical check and before
  `guards need [check]` (STP-43 step 1).
- F-2: the hook loads an edited `stapel.toml` and denies it with `.stapel/stapel.toml would not load:
  <error>` (`crates/stapel-core/src/guard.rs`, STP-38).
- F-3: `command_words` in `crates/stapel-cli/src/check.rs` splits at white space (a newline included)
  and at `; | & ( ) < >` and a backtick; only `; | & (` and a backtick start a command.
- F-4: `guard_reach` (same file) is used by `stapel check` and `stapel init` (STP-43 AC-3, AC-4).

### Shape

- `config.rs`: the six folders join the list of the `matches!` in `validate`.
- `check.rs`, `command_words`: a `\n` starts a command like `;`, unless the piece so far ends in an odd
  number of `\`; then the last `\` is dropped and the newline is white space, and when the piece is then
  empty no word is pushed, so the start and interpreter states of the word before stay (AC-4).

### Decisions (before the build)

None.

### Decisions (as built)

None yet.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check_guards::six_more_work_folders_refused` |
| AC-2 | `hook_guards::a_new_work_folder_edit_is_denied` |
| AC-3 | `check_guards::newline_starts_a_command` |
| AC-4 | `check_guards::backslash_newline_continues` |
| AC-5 | `init_process::guard_reach_after_newline` |

Changed by this ticket: no existing test.

## Plan

Route: full (tag `guard`), auto.

1. Work folders (AC-1, AC-2; tests `check_guards::six_more_work_folders_refused`,
   `hook_guards::a_new_work_folder_edit_is_denied`): the RED adds the two tests; the GREEN adds the six
   folders to the list in `Config::validate`.
2. Newline (AC-3 to AC-5; tests `check_guards::newline_starts_a_command`,
   `check_guards::backslash_newline_continues`, `init_process::guard_reach_after_newline`): the RED adds
   the three tests; the GREEN makes a newline start a command in `command_words`, except after an odd
   number of `\`, where the last `\` is dropped and the state of the word before it stays (Design shape).

## Review

TODO

<!-- stapel:review:begin -->
Generated by stapel; edits between the markers are overwritten.

Findings:

| Stage | Round | Roles | HIGH | MEDIUM | LOW | False | Fates |
|---|---|---|---|---|---|---|---|
| spec-review | 1 | reviewer | 2 | 2 | 3 | 0 | defer 1, open 1, patch 5 |
| spec-review | 2 | reviewer | 0 | 0 | 3 | 0 | open 3 |

Check runs: none
<!-- stapel:review:end -->

## Summary

TODO
