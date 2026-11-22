# STP-37 — The close and the history

## Description

Source: `docs/PLAN.md`, phase 3b, ticket 3b-3 (items 3.5, 3.6, 3.7b); ledger 2026-11-20 (the ten items of
the first cut grouped into four tickets).

### Frame

**Problem.** Part of what must be true at the close is left to the CI. In the pilot the project's checks
(prettier, eslint, tsc) failed in CI after a green `stapel check`, and jest ran under a Node version other
than the pinned one. The history rules judge the shape, not the result: a GREEN that touches its step's
test file, or a build input changed after a RED, blocks the step; in STP-35 three test-only commits went to
main apart and the branch was rebuilt each time. `stapel review start` refuses round 3, and the rule 9
records `catches` and `finding` are written by hand, or not at all.

**Plan item:** 3b-3 (3.5, 3.6, 3.7b).

**Risk tags:** `interface` (the `[check]` key `project`, the lines of `stapel check`, the `history:` line
of a commit message, the `--round` values, the output of `stapel close`), `data` (ledger records that
`stapel ledger rules` of 3b-4 will read; check records the close reads).

**Non-goals.** Reading CI or pre-commit configuration files; switching or installing a Node version;
version ranges (`engines`), aliases (`lts/*`) and other version managers' files (`.tool-versions`,
`volta`); reading the CI result; `stapel ledger rules` and the generated Review section (3b-4); the items
of STP-36 (the close's own dialog rules); the open items XR2-3 and XR2-7 of STP-35.

**Promise (closed list).**

1. Project checks: `stapel check` at level `head` and `full` runs the shell commands of `[check] project`
   in the check copy after the suite; `{files}` becomes the ticket's changed files. A failed command fails
   the check and so stops the close.
2. Node pin: under the junit runner, `stapel check` compares the version in `.nvmrc` or `.node-version`
   with `node --version` and refuses a mismatch, a pin that is not a version, or a missing `node`, exit 2.
3. History reason: a non-RED commit with a line `history: <reason>` may change its step's test file (which
   must still fail at RED) or a build input after a RED; the step is judged by its runs instead of
   `tests-changed` or the build input block; the report and the record list these commits.
4. Review rounds: `stapel review start --round N` accepts any whole N of 1 or more after round N−1.
5. Ledger at the close: `stapel close` appends one `catches` record (counted findings by stage and `rule`)
   and one `finding` record per deferred finding; a close that fails writes none.

Not promised: a project command that leaves processes after its exit; file names with a newline in
`{files}`; a `history:` line that a later commit reverts; deduplication against hand-made ledger records.

**Prior art.** STP-6 (build inputs), STP-14 (history outcomes), STP-24 (`review start`), STP-28 (junit),
STP-35 (levels, time limit; its test-only commits), the hand-written `catches` records of STP-25, STP-26.

**Size.** About 500 source lines (`check.rs`, `config.rs`, `outcomes.rs`, `build_inputs.rs`, `review.rs`,
`close.rs`, `records.rs`, the skill and the starter `stapel.toml`), about 700 test lines. 12 Plan steps,
the limit; split past 12. Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-22: Decision (project checks, the human, recommended): an explicit list of commands in
  `stapel.toml`, with `{files}` for the changed files; a failure stops the close. Rejected: reading the CI
  and pre-commit files; moving 3.5 to a later ticket.
- 2026-11-22: Decision (where the project checks run, agent): inside `stapel check` at level `head` and
  `full`, in the check copy, so the close reads one record and the grant of `stapel close` does not wait
  for long commands. Rejected: running them inside `stapel close`.
- 2026-11-22: Decision (Node pin, the human, recommended): compare the pin with `node --version` and refuse
  on a mismatch; no version manager. Rejected: a wrapper command; not promised.
- 2026-11-22: Decision (ledger, the human, recommended): one `catches` record and one `finding` record per
  deferred finding. Rejected: `catches` only; a `finding` record per HIGH and MEDIUM finding.
- 2026-11-22: Decision (history reason, the human, recommended): the step is judged by its test runs, its
  line names the reason, and the Summary lists the commits (from the check report: `stapel export` writes
  no Summary before 3b-4). Rejected: a ledger decision per reason; the reason in the commit message only.
- 2026-11-22: Assumption: STP-36 changes `close.rs` in parallel; `stapel depends` runs before the build.
- 2026-11-22: Decision (build order, the human): steps 1 to 10 are built before STP-36 is on main; the
  merge dependencies DP-1/DP-2 (and the same DP2-x, DP3-x of later rounds) are lowered to LOW, and steps
  11 and 12 are built after main with STP-36 is merged into this branch (74200d1).

## Spec

Terms:

- **Project command**: one entry of `[check] project`, a shell command run with `sh -c`.
- **Changed files**: the paths that `git diff-tree -z --no-renames --name-only` gives for each non-merge
  commit of the ticket's first-parent range as the check finds it (with or without `base`), that exist at
  the checked commit and do not lie under `.stapel/`; sorted, unique, each passed as one shell-quoted
  word, with `./` before a path that starts with `-`.
- **Pin**: the text of `.nvmrc` or `.node-version` at the root of the checked commit. A pin is a
  **version** when it is one non-empty line of one to three dot-separated numbers with an optional
  leading `v`, surrounding whitespace ignored. It **matches** `node --version` when each given number
  equals the same number of the node version. Two pins **disagree** when their lists of numbers differ.
- **History line**: a line of a commit message, the subject included, that starts with `history: `
  followed by a non-empty reason; the first such line of a commit counts. A **history commit** is a
  non-RED commit of the ticket's first-parent range with a history line.
- **Counted finding**: a finding of `findings.jsonl` whose severity after its triage verdicts
  (`triage::state_of`) is HIGH or MEDIUM, with `promise` `inside`, not marked false, and whose newest fate
  is not `no-change`.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF an entry of `[check] project` is empty or whitespace THEN loading `stapel.toml` fails with `check.project entries must not be empty`. | `check_project::refuses_an_empty_command` |
| AC-2 | WHEN the level is `head` or `full` and `[check] project` has entries THE check runs each in order with `sh -c` in the check copy, with the environment of its other runs, after the suite and prints `project: <command>: pass`, or `project: <command>: fail (exit <n>)` and the last 20 lines of its output; a fail makes the result `fail`, exit 1, and the close refuses it as any failing check. | `check_project::runs_the_commands`, `check_project::a_failing_command_fails_the_check` |
| AC-3 | WHEN a whitespace-separated word of a project command is exactly `{files}` THE word becomes the changed files; IF there is none THEN the command does not run and its line is `project: <command>: skipped: no changed files`, which counts as passing. | `check_project::files_are_the_changed_files`, `check_project::no_changed_files_skips` |
| AC-4 | IF the time limit of the check runs out during a project command, or before the first one THEN the running command and its descendants are killed as other runs, and its line and the lines of the commands not run say `unverified: time limit`; the result is `fail`. | `check_project::time_limit_stops_a_command` |
| AC-5 | WHEN the level is `steps` THE check runs no project command and prints `project: skipped: level steps`. | `check_project::steps_level_skips` |
| AC-6 | WHEN the runner is junit and the checked commit has a pin that is a version THE check, at every level, runs `node --version` in the check copy under its time limit before any test, prints `node: <version> (pin <pin> in <file>)` after the `level:` line, and goes on when the pin matches; under the cargo runner and in `--history` mode the pin files are not read. | `check_node::a_matching_pin_runs` |
| AC-7 | IF the pin does not match THEN the check exits 2 with `stapel: node <version> does not match the pin <pin> in <file>` and runs no test. | `check_node::a_mismatch_refuses` |
| AC-8 | IF a pin is not a version (`.nvmrc` judged first), `.nvmrc` and `.node-version` disagree, or `node --version` cannot run or prints no version THEN the check exits 2 with `stapel: <file>: not a version: <text>`, `stapel: .nvmrc and .node-version disagree: <a>, <b>` or `stapel: node --version failed: <reason>`, and runs no test. | `check_node::refuses_a_bad_pin` |
| AC-9 | WHEN a history commit changes a protected test in a test file of a step THE step is not `tests-changed`: the test text of that commit is the protected text for later commits, and the note `history by <label>: <reason>` follows the step's line, in both modes; a test inside a source file and a deleted test are judged as today. | `check_history_reason::green_may_change_its_test` |
| AC-10 | IF a step's test file was changed by a history commit THEN its RED run takes that file from the newest such commit on the RED commit's code, and the step is `no-red` unless that run fails as a RED is judged today, a compile failure included. | `check_history_reason::a_weakened_test_is_no_red` |
| AC-11 | WHEN a history commit changes a build input under the cargo runner, or an input, a configuration file or the command's script under the junit runner THE change blocks no step and no run, in both modes; a later non-RED commit without a history line still blocks, as today, and the text of the history commit is the baseline it is compared with. | `check_history_reason::build_input_with_a_reason`, `check_history_reason::junit_input_with_a_reason` |
| AC-12 | IF a commit has `history:` with an empty or whitespace reason, or the history line is in a RED commit THEN the commit is judged as one without a history line. | `check_history_reason::an_empty_reason_counts_as_none` |
| AC-13 | WHEN the range has history commits THE report prints `history: <short id> <reason>` for each, oldest first, before the suite line (in `--history` mode after the step lines), and the check record has `history`. | `check_history_reason::the_record_lists_the_reasons` |
| AC-14 | WHEN a check writes `project` or `history` into its record THE entries are `{command, outcome}` with the outcome `pass`, `fail`, `skipped` or `unverified`, and `{commit, reason}` with the full commit id; a command or reason is cut to 200 characters, and past 20 entries the rest are left out with `truncated: true`. | `check_history_reason::the_record_lists_the_reasons`, `check_project::runs_the_commands` |
| AC-15 | WHEN `stapel review start --round N` gets a whole N of 3 or more and `findings.jsonl` has code review findings of round N−1 THE round starts as round 2 does, its pack carries the patch findings of round N−1; IF it has none THEN it refuses with `round <N> needs the code review findings of round <N-1> in .stapel/tickets/<KEY>/findings.jsonl`; IF N is 0 or not a whole number THEN it refuses with `--round must be a whole number of 1 or more, or merge`; exit 2. | `review::any_round_number`, `review::a_round_needs_the_previous_round` |
| AC-16 | WHEN `stapel close` closes a ticket THE ledger gets, in one write before `state.json` is saved, one `catches` record with the note `At the close: counted findings: <stage>/<rule>=<n>; …` (sorted by stage, then rule; `;` and `=` in a rule written `_`), or `At the close: counted findings: none`, and one `finding` record per finding not marked false whose newest fate is `defer`, with the note `At the close, deferred: <id> <severity> <stage> round <n>: <claim>` (severity after triage, claim cut to 500 characters); the close prints `ledger: <n> records`. | `close_ledger::writes_catches_and_deferred`, `close_ledger::no_findings` |
| AC-17 | IF `stapel close` refuses, `findings.jsonl` or the ledger has a malformed line, or the append fails THEN the ledger and `state.json` are unchanged, the close names the cause, and a later close after the fix writes the records; a failed append spends the grant, so the later close has its own dialog. | `close_ledger::a_refused_close_writes_nothing`, `close_ledger::bad_journals_refuse` |
| AC-18 | WHEN `stapel init` writes the starter `stapel.toml` and the skill THE starter names `project` with `{files}` in a comment under `[check]`, and the skill names the history line, the round numbers and the ledger records of the close. | `init_process::starter_names_project_and_history` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-1 to AC-5 and AC-14, item 2 by AC-6 to AC-8, item
  3 by AC-9 to AC-14, item 4 by AC-15, item 5 by AC-16 and AC-17; AC-18 tells the process about them.
  Unchanged: the step outcomes of commits without a history line; the cache key (project commands are never
  cached); the golden reports of a check without `project` and without a pin; rounds 1, 2 and `merge`;
  `stapel next`, which names no round after 2 (the orchestrator names round 3, as the skill says).
- **Not promised:** the Frame's list; the order of records when two closes append at once.

## Test plan

Integration tests in `crates/stapel-cli/tests/`: the new files `check_project.rs`, `check_node.rs`,
`check_history_reason.rs` and `close_ledger.rs`, and the existing `review.rs` and `init_process.rs`. The
check tests use the fake cargo of `tests/common/`; the node tests put a fake `node` script first on an
absolute `PATH` and use the junit fixtures of `check_junit.rs`. Light tests: `stapel cargo test -j 4 -p
stapel-cli --test <file> -- --test-threads=4`; before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Project key | `""`, `"  "`; a list of two commands loads | `check_project::refuses_an_empty_command` |
| Project run | two passing commands in order (a log file shows the order and the copy's path); one that exits 3 with 30 output lines: the last 20 printed, result `fail`, exit 1, `stapel close` refuses; 22 commands, one of 300 characters: the record cut and `truncated` | `check_project::runs_the_commands`, `check_project::a_failing_command_fails_the_check` |
| Changed files | a changed file, a file with a space, a UTF-8 name (`é.ts`), a file named `-x.ts`, a deleted file, a renamed file, a file under `.stapel/`, a file changed only before the base, a file brought only by a merge of the base branch; `{files}` twice in one command; `--x={files}` and `"{files}"` left as written; no changed file outside `.stapel/` | `check_project::files_are_the_changed_files`, `check_project::no_changed_files_skips` |
| Time limit | a command that sleeps past the limit and starts a child; a second command not run; a suite that uses the whole limit before the first command | `check_project::time_limit_stops_a_command` |
| Level | `--level steps` with two commands | `check_project::steps_level_skips` |
| Node match | pin `20`, `v20.11`, `20.11.1` with node `v20.11.1`, at level `steps` too; under cargo a `.nvmrc` of `lts/*` is ignored; `--history` with a mismatching pin is not refused | `check_node::a_matching_pin_runs` |
| Node refusal | pin `18` with node `v20.11.1`; pin `20.1` with `v20.11.1`; `lts/*`, empty, two lines, `20.x`; `.nvmrc` `20` and `.node-version` `18`; `.nvmrc` `lts/*` and `.node-version` `18` (the alias is named); `20` and `v20` agree; no `node` on `PATH`; a `node` that exits 1; a `node` that prints `hello` | `check_node::a_mismatch_refuses`, `check_node::refuses_a_bad_pin` |
| History tests | a GREEN with a history line that edits its step's test; a later fix commit with a history line that edits step 1's test; the same without the line (`tests-changed` as today); a history commit that deletes a test (as today); a test inside a source file (as today); `--history` mode | `check_history_reason::green_may_change_its_test` |
| Weakened test | a history GREEN that edits its step's test so that it passes on the RED code: `no-red`; a history GREEN that fixes a typo of the test, which still fails on the RED code: `pass`; a history GREEN whose test does not compile on the RED code: `pass` | `check_history_reason::a_weakened_test_is_no_red` |
| History inputs | a history commit that changes `tests/common/mod.rs` between steps; then a commit without the line that changes `Cargo.toml`; a later commit without the line that reverts the history commit's input (blocks); under junit a history commit that changes `jest.config.js` and one that changes the command's script; both modes | `check_history_reason::build_input_with_a_reason`, `check_history_reason::junit_input_with_a_reason` |
| History line | `history:` with no reason; `history:   `; a RED with a history line; `History: x` (another case: no line); each next to a valid history commit in the same fixture, which is excused; the line in the subject (counts); two lines (the first reason) | `check_history_reason::an_empty_reason_counts_as_none` |
| History record | two history commits: report lines oldest first, the `history` field of the record with full ids; a reason of 300 characters cut; `--history` mode places the lines after the steps | `check_history_reason::the_record_lists_the_reasons` |
| Rounds | round 3 after round 2 findings, with a round 2 patch finding in its pack; round 7 after round 6; round 3 without round 2 findings; round 0, `-1` and `x` with the new text; the existing `refuses_bad_rounds` and precedence tests changed from round 3 | `review::any_round_number`, `review::a_round_needs_the_previous_round` |
| Ledger | findings of two stages and three rules, a LOW finding, a HIGH regraded `low` by a verdict, a finding marked false, an outside finding, a `no-change` finding, two deferred findings, a deferred finding marked false (no record), a deferred HIGH regraded `low` (named `LOW`), a rule with `;`, a claim of 600 characters; a ticket without findings | `close_ledger::writes_catches_and_deferred`, `close_ledger::no_findings` |
| Ledger failures | a close refused for a failing check and for a missing reason, then a close that succeeds and writes; a malformed line of `findings.jsonl`; a malformed line of the ledger; a ledger file that is read-only; each leaves `state.json` open | `close_ledger::a_refused_close_writes_nothing`, `close_ledger::bad_journals_refuse` |
| Process files | the starter `stapel.toml` and the skill | `init_process::starter_names_project_and_history` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[check] project` | list of strings | 0 / any; 20 in the record | absent or `[]`: no project line | an empty or whitespace entry: load error | run as given by `sh -c`; cut to 200 characters in the record | `check_project::refuses_an_empty_command`, `check_project::runs_the_commands` |
| `{files}` in a command | whole word | 0 / every changed file | no changed file: skipped | a file name with a newline: not promised; `{files}` inside another word: left as written | each path one shell-quoted word | `check_project::files_are_the_changed_files` |
| `.nvmrc`, `.node-version` | text file | `1` / `v99.99.99` | empty: refused | alias, range, several lines, `20.x`: refused | each given number, exact | `check_node::refuses_a_bad_pin` |
| History line | line of a commit message | one character of reason / the message | empty reason: no line | another case of `history:`: no line | the first line, trimmed; cut to 200 characters in the record | `check_history_reason::an_empty_reason_counts_as_none` |
| `--round N` | whole number or `merge` | 1 / the largest `u64` | absent: as today | 0, negative, not a number: refused with the new text | exact | `review::any_round_number` |
| Findings of `findings.jsonl` | JSON lines | none / the journal limit | none: `catches` says `none` | a malformed line: the close refuses with its place | state after triage; claim cut to 500 characters | `close_ledger::writes_catches_and_deferred`, `close_ledger::bad_journals_refuse` |
| `.stapel/process-ledger.jsonl` | JSON lines | absent / the journal limit | absent: created | a malformed line, a write error: the close refuses | one write of all records | `close_ledger::bad_journals_refuse` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | paths with spaces, glob characters and UTF-8 in `{files}`; deletions; renames (`--no-renames`: the new path; the old one does not exist) | read with `-z`; quoted words; deletions left out | `check_project::files_are_the_changed_files` |
| git | paths with newlines or non-UTF-8 bytes in `{files}` | not promised | — |
| git | commit messages with control bytes in a history line | the reason is printed with control bytes replaced, as the report does today | `check_history_reason::the_record_lists_the_reasons` |
| git | history shape: merges of the base branch, amended and rebuilt history | the first-parent range: commits brought by a merge are outside it; a history line of the merge commit itself counts; files brought only by a merge are not changed files | `check_history_reason::build_input_with_a_reason`, `check_project::files_are_the_changed_files` |
| git | shallow clone, object format, worktrees, submodules, dirty tree, configuration | unchanged by this ticket | — |
| node | `node --version` output: `v20.11.1`, extra text, nothing | the first `v<n>.<n>.<n>` token, else refused | `check_node::refuses_a_bad_pin` |
| node | no `node` on `PATH`; `node` exits non-zero; `node` that hangs | refused, exit 2; a hang ends at the check's time limit | `check_node::refuses_a_bad_pin` |
| node | version managers that change `node` per folder (nvm, fnm, volta shims) | the `node` the check copy finds is compared; not switched | — |
| cargo | every dimension | unchanged; project commands are shell commands, not cargo runs | — |
| os | processes of a project command: groups, children that leave the group | the kill of the check's time limit, as for other runs | `check_project::time_limit_stops_a_command` |
| os | `sh` missing | not promised | — |
| os | case-insensitive file systems: `.NVMRC` | not read; only the exact names | — |
| claude-code | every dimension | not read by this ticket | — |

**Review Focus:** a project command that changes files of the check copy (the copy is removed after the
check; not tested); a `{files}` list longer than the system's argument limit (not promised, `sh` fails and
the command fails); two closes of different tickets appending to the ledger at once (one line each; not
tested); a history line inside a commit that is both a GREEN and a merge (judged as a GREEN; the review
judges it); a pin file present at HEAD but absent at a RED commit (the pin of the checked commit is read
once; not tested).

## Design

### Facts

- F-1: `outcomes.rs` (about line 700 to 750) walks later commits for each protected unit of a step; a
  non-RED commit that changes it makes the step `tests-changed`; a deletion with `spec change:` retires
  a test. The message is already read there.
- F-2: `build_inputs.rs::BuildInputs::changed` is the first non-RED commit of the range that changes a
  build input; `check.rs` (about line 464) turns it into `build-input-changed: <path> at <id>` and blocks
  every later step without a cargo run. Under junit the block comes from `build_inputs.rs::analyse_junit`
  (`walk_changes`, every commit of the range, no message read) and `check.rs::junit_block`; the
  `--history` mode (`check.rs` about line 1393) is a third site.
- F-3: `review.rs:62` refuses a round outside 1 and 2; `review.rs:83` to `:97` checks round 1 findings
  before round 2; `write_pack` gives findings only to round 2 and `patch_findings` reads round 1; the
  tests `review::refuses_bad_rounds` and the precedence tests use `--round 3` as a bad round.
- F-8: `triage::state_of` gives a finding's severity after its verdicts and marks false findings;
  `is_blocking` and `patch_findings` leave false findings out.
- F-9: `check.rs` trims only `steps` until the record fits `MAX_RECORD` (about line 627).
- F-4: `close.rs` checks the current check line and the merge review, consumes the grant, saves
  `state.json` and writes a decision record; nothing touches the ledger. `records::check_ledger` builds a
  ledger line and `journal::append_all` appends lines.
- F-5: the check copy links the working tree's `node_modules` (`check.rs:867`); each run gets the time
  left of the whole check (STP-35).
- F-6: `stapel export` writes no Summary; the orchestrator writes it.
- F-7: `check.rs::refuse` prints `stapel: <message>` and exits 2.

### Decisions (before the build)

- 2026-11-22, agent: `{files}` leaves out paths under `.stapel/`, because the ticket files are not project
  code and prettier would judge `ticket.md`.
- 2026-11-22, agent: a project command runs after the suite even when a step failed, so one check shows
  every failure.
- 2026-11-22, agent: a counted finding leaves out false findings and the newest fate `no-change`, as the
  hand-written records of STP-25 and STP-26 did (SR1-5).
- 2026-11-22, agent (SR1-6): the close builds the ledger records before it consumes the grant and appends
  them in one write before `state.json` is saved; a failed append refuses the close. A failed save after
  the append leaves records of an open ticket; that case is not promised.
- 2026-11-22, agent (SR1-9): a history commit may change a step's test only so that the test still fails
  on the RED code; the RED run takes the test file from the newest history commit that changed it.
- 2026-11-22, agent (SR1-8): the round N pack carries the patch findings of round N−1; `stapel next` is
  not changed.
- The project commands run through the runner's program run with the deadline, in the check copy as the
  current directory. The record cut follows F-9: `project` and `history` are cut first to 20 entries.
- The pin is read from the check copy at the checked commit; `node` is found as the junit runner finds its
  command today.
- The history line is read in `steps.rs` with the commit message, so that `outcomes.rs` and
  `build_inputs.rs` share one parser.

### Decisions (as built)

All dated 2026-11-22, mark `agent`; the person may overrule them at the close.

- AC-14's `truncated: true` is a top-level `project_truncated: true` and `history_truncated: true` in the
  record, after the `steps_truncated` precedent. Commands and reasons are cut by characters (`cut_chars`).
- The printed `history:` line is cut to 200 characters as the record is, and a control character in it
  becomes a space; the record keeps the reason raw apart from the cut.
- Besides the time limit, a project command reads `unverified: <reason>` when it cannot start or ends by a
  signal, and when the changed files cannot be read from git (then every later command too); each fails the
  check (DR-1).
- The first `history: ` line of a message with a non-empty reason counts (`steps.rs::history_reason`).
- The RED run of a step overwrites its test files with their text at the newest history commit
  (`Analysis::red_files`); the GREEN and HEAD runs take the commit as it is.
- A history commit's build-input changes are still recorded as touched but set no `changed`
  (`build_inputs.rs::history_commit`); `history_commits` lists them for the report and record.
- The round refusal keeps round 2's text with the full path; AC-15 quotes it. The pack heading is
  `Findings of round <N-1> with the fate patch`; `--round` accepts hyphen values so `-1` gets the refusal.
- The close builds its ledger lines (`stapel-core/src/close_ledger.rs`) after STP-36's `close_refusal`, the
  main-merge, check and merge-review gates and before the grant is consumed, appends them in one write
  after it and before `state.json` is saved, and prints `ledger: <n> records`. A finding without `rule`
  counts under `none`. `records::closed_ticket_by_key_only` now counts the `catches` line.
- The starter `stapel.toml` comment for `project` is a list: `# project = ["npx prettier --check {files}"]`.
- Step 2's RED was rebuilt once (history rewrite before any review) because `runs_the_commands` depended on
  the length of `TMPDIR`; the code did not change.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check_project::refuses_an_empty_command` |
| AC-2 | `check_project::runs_the_commands`, `check_project::a_failing_command_fails_the_check` |
| AC-3 | `check_project::files_are_the_changed_files`, `check_project::no_changed_files_skips` |
| AC-4 | `check_project::time_limit_stops_a_command` |
| AC-5 | `check_project::steps_level_skips` |
| AC-6 | `check_node::a_matching_pin_runs` |
| AC-7 | `check_node::a_mismatch_refuses` |
| AC-8 | `check_node::refuses_a_bad_pin` |
| AC-9 | `check_history_reason::green_may_change_its_test` |
| AC-10 | `check_history_reason::a_weakened_test_is_no_red` |
| AC-11 | `check_history_reason::build_input_with_a_reason`, `check_history_reason::junit_input_with_a_reason` |
| AC-12 | `check_history_reason::an_empty_reason_counts_as_none` |
| AC-13 | `check_history_reason::the_record_lists_the_reasons` |
| AC-14 | `check_history_reason::the_record_lists_the_reasons`, `check_project::runs_the_commands` |
| AC-15 | `review::any_round_number`, `review::a_round_needs_the_previous_round` |
| AC-16 | `close_ledger::writes_catches_and_deferred`, `close_ledger::no_findings` |
| AC-17 | `close_ledger::a_refused_close_writes_nothing`, `close_ledger::bad_journals_refuse` |
| AC-18 | `init_process::starter_names_project_and_history` |

## Plan

Route: full (tags `interface`, `data`), auto.

1. Project key (AC-1; test `check_project::refuses_an_empty_command`, which fails at the RED because the
   key is unknown): the GREEN adds `project` to `config.rs::Check` with its load error. Must not change:
   the other keys.
2. Project commands (AC-2, AC-4, AC-5, the `project` part of AC-14; tests
   `check_project::runs_the_commands`, `check_project::a_failing_command_fails_the_check`,
   `check_project::time_limit_stops_a_command`, `check_project::steps_level_skips`): the GREEN runs the
   commands after the suite with the deadline, prints their lines and writes `project` into the record.
   Must not change: a check without the key.
3. Changed files (AC-3; tests `check_project::files_are_the_changed_files`,
   `check_project::no_changed_files_skips`): the GREEN replaces the word `{files}` and skips a command
   with no changed file. Must not change: a command without `{files}`.
4. Node pin (AC-6, AC-7; tests `check_node::a_matching_pin_runs`, `check_node::a_mismatch_refuses`): the
   GREEN reads the pin under junit, runs `node --version`, prints the `node:` line and refuses a mismatch.
   Must not change: the cargo runner and `--history`.
5. Bad pins (AC-8; test `check_node::refuses_a_bad_pin`): the GREEN refuses an alias, two disagreeing
   files and a failing `node`. Must not change: a matching pin.
6. History line for tests (AC-9, AC-12; tests `check_history_reason::green_may_change_its_test`,
   `check_history_reason::an_empty_reason_counts_as_none`): the GREEN parses the history line in
   `steps.rs` and lets a history commit change a protected test in `outcomes.rs`. Must not change: a
   commit without the line.
7. The RED run of a changed test (AC-10; test `check_history_reason::a_weakened_test_is_no_red`): the
   GREEN runs the step's RED with the test file of the newest history commit. Must not change: a step
   without a history commit.
8. History line for build inputs (AC-11; tests `check_history_reason::build_input_with_a_reason`,
   `check_history_reason::junit_input_with_a_reason`): the GREEN skips history commits in
   `BuildInputs::changed`, `analyse_junit` and the `--history` mode. Must not change: a later commit
   without the line.
9. History record (AC-13, the `history` part of AC-14; test
   `check_history_reason::the_record_lists_the_reasons`): the GREEN prints the `history:` lines and writes
   `history` into the record. Must not change: the golden reports without history commits.
10. Review rounds (AC-15; tests `review::any_round_number`, `review::a_round_needs_the_previous_round`;
    the RED also changes `review::refuses_bad_rounds` and the precedence tests from round 3): the GREEN
    accepts any whole round of 1 or more, checks round N−1 and puts its patch findings into the pack.
    Must not change: rounds 1, 2 and `merge`.
11. Ledger at the close (AC-16, AC-17; tests `close_ledger::writes_catches_and_deferred`,
    `close_ledger::no_findings`, `close_ledger::a_refused_close_writes_nothing`,
    `close_ledger::bad_journals_refuse`): the GREEN builds the records before the grant and appends them
    before the save. Must not change: the close's other refusals.
12. Process files (AC-18; test `init_process::starter_names_project_and_history`): the GREEN changes the
    starter `stapel.toml` and `skill.md` in `crates/stapel-cli/assets/` and runs `stapel init` in the
    worktree. Must not change: the other process files.

## Review

TODO

## Summary

TODO
