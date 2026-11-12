# STP-28 — junit runner and base..HEAD walk

## Description

Source: `docs/PHASES.md`, stage A item A1 (plan items 1.11 and 1.12); ledger decision 2026-11-10; the
investigation in the chat of 2026-11-11.

### Frame

**Problem.** `stapel check` runs only cargo tests, so the TypeScript pilot repository cannot be checked;
every check and commit hook walks the whole first-parent history (1035 commits here, more in the pilot).

**Plan item:** A1 and A1b (items 1.11, 1.12 and the lint part of 1.6; ledger decision of 2026-11-12).

**Risk tags:** `guard` (the check is the RED/GREEN boundary; a wrong report read lets untested code pass),
`interface` (new `[check]` keys, a new command).

**Non-goals.** The other checks of item 1.6; `stapel init` on a foreign repository (A2); attention numbers
(A3); check in CI; deliberate circumvention of the check.

**Promise (closed list).**

1. `[check] runner = "junit"` with `command` (a template with `{file}`), `report` (the XML path), the
   timeout, and globs for test files and build inputs with node defaults; the cargo runner is unchanged.
2. A step's tests are the `test`/`it` calls the RED commit adds or changes, found by a TypeScript lexer;
   identity `<file>::<name>`, the name joined from the `describe` titles and the test title.
3. The outcomes of STP-4 and STP-14 keep their meaning under junit.
4. `stapel check`, the commit hook and `stapel next` walk only `base..HEAD`; a ticket without a base keeps
   the walk from the root.
5. A commit that touches only `docs/`, the process ledger or a ticket's `tokens.jsonl` does not make a
   check stale (the ledger and `tokens.jsonl` already hold; `docs/` is new).
6. `stapel lint [KEY]` for one ticket, with two checks for both runners: a test the ticket names does not
   exist; a step test the ticket does not name.

Not promised: titles that are not string literals (`${…}`, variables, `test.each`); report formats other
than JUnit XML; languages other than TypeScript and JavaScript.

**Size.** About 900 source lines and 1100 test lines, 11 to 12 Plan steps; over 12, the `base..HEAD` walk
is split off by a ledger decision. Route: full (tags), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-11: Decision (step tests, the human, recommended): a TypeScript lexer reads the `test`/`it`
  calls with their `describe` titles that the RED commit adds or changes, as the Rust lexer does for
  `#[test]`. Rejected: the test names of the Plan step (a forgotten name goes unchecked).
- 2026-11-11: Decision (file kinds, the human, recommended): two `[check]` keys, test file globs and build
  input globs, with node defaults (`**/*.test.ts`, `**/*.spec.ts`, `**/__tests__/**`; `package.json`, the
  lock file, `jest.config.*`, `tsconfig*.json`). Rejected: fixed jest rules without keys.
- 2026-11-11: Decision (lint, the human): this ticket adds `stapel lint [KEY]` with only the two test-name
  checks, for cargo and junit; the other checks of item 1.6 stay a gate candidate (ledger 2026-11-10).
  Rejected: the whole item 1.6 here (over 12 steps); 1.6 as a separate ticket in stage A.
- 2026-11-11: Decision (identity, the human, recommended): one run of the command per test file, with the
  placeholder `{file}` in place of `{tests}`; the file is known from the run, the name is the test case's
  `name` (the jest-junit default: `describe` titles and the test title joined by a space). Rejected: one run
  of all files with a `file` attribute in the report (a reporter setting for the person).
- 2026-11-11: Decision (dependencies, the human, recommended): each check copy links the working tree's
  `node_modules`; a step whose range changes a build input is `unverified`. Rejected: a `setup` command such
  as `npm ci` in each copy (about a minute per copy, needs the network); a link with an optional `setup`.
- 2026-11-11: Assumption (HEAD suite): the suite at HEAD runs `command` once with the word `{file}` left
  out; no separate key.
- 2026-11-11: Assumption (command): `command` is split on spaces and run without a shell; a first word
  with `/` is a path in the check copy, any other is looked up in the absolute `PATH` entries.
- 2026-11-11: Assumption (defaults): the default globs match at any depth (`**/package.json`), "the lock
  file" means `package-lock.json`, `yarn.lock` and `pnpm-lock.yaml`; only the root `node_modules` is linked.
- 2026-11-11: Assumption (test calls): `.only`, `.skip` and `.todo` forms of `test` and `it`, and `.only`
  and `.skip` of `describe`, count; names are compared trimmed, escapes in titles decoded.
- 2026-11-11: Assumption (report): one run gives one `testsuite`; a suite without a test case after a
  failed run means the file did not load, which counts as a cargo compile error of the step's target.
- 2026-11-11: Assumption (lint): without KEY, `lint` takes the branch ticket or the one open ticket; an
  open ticket's named test must exist once the step that names it has a GREEN commit.
- 2026-11-11: Decision (inputs, the human, recommended): the default `inputs` also hold
  `**/babel.config.*`, `**/.babelrc`, `**/.swcrc` and `**/jest.setup.*` (SR1-11). Rejected: leaving them
  to the person's `inputs`.
- 2026-11-11: Assumption (config source): `command`, `report` and the globs come from the working tree's
  `stapel.toml` when the check starts; a change of `stapel.toml` inside the Walk is a build input (SR1-8).
- 2026-11-11: Decision (report fixtures, the human, recommended): real jest-junit reports (jest 29.7.0,
  jest-junit 16.0.0) are the reader's fixtures, committed by the step that adds the reader, and a manual
  smoke check on the pilot repository is named in Proof (SR2-5). Rejected: hand-made reports only.
- 2026-11-11: Decision (`{file}` place, the human, recommended): `{file}` after a word that starts with
  `-` stays refused, and the message says to put `{file}` right after the program, as in `jest {file}
  --ci`, because jest reads the file as the value of an array option before it (SR2-10). Rejected: an
  optional `suite` key.
- 2026-11-11: Assumption (real runs, replaces parts of the test calls and report lines): `.todo` is no
  test call (jest-junit reports it as passed); a file that does not load gives no suite unless
  `JEST_JUNIT_REPORT_TEST_SUITE_ERRORS=true`, which the junit run sets; default test globs are
  `**/*.test.*`, `**/*.spec.*`, `**/__tests__/**`, limited by the Test file names.
- 2026-11-11: Decision (plan size, the orchestrator under the human's leave in the chat): fix steps 13 to
  19 after code review round 1; the ticket is not split because the fixes touch the code and tests of
  steps 3 to 12. Updated 2026-11-12: steps 20 to 23 follow the merge of main (M-1) and take the findings
  deferred to after code review round 2, so the Plan has 23 steps and the person confirms once.
- 2026-11-12: Decision (base merge, the human, recommended, M-1): under both runners a build input that only
  a merge of the base branch brings (its content at the merge equals that at the merge's second parent,
  and that parent is on the base branch) blocks no step and not the suite; a merge that resolves the file
  to other content still blocks. Rejected: the STP-13 rule unchanged, so every ticket that merges main
  with a new test file cannot close.
- 2026-11-12: Decision (scope and follow-up, the human, ledger): STP-28 keeps promise items 1 to 6, so A1b
  is done inside it; it has 23 Plan steps and closes after code review round 2 with text fixes only; the
  code defects and the coverage of round 2 go to a follow-up ticket A1c before A2. Rejected: moving items
  4 to 6 to A1b; fix steps 24 to 26 and a third review round.

## Spec

Terms:

- **Walk**: the first-parent commits of `base..HEAD` (`base` of `state.json`; without it, from the root);
  a merge counts by its diff to its first parent.
- **Glob**: a repository path pattern; `*` and `?` match inside one path part, `**` any parts. **Test
  globs** are `[check] tests`, **input globs** `[check] inputs`.
- **Test file**: a file matching a test glob, named `*.ts`, `*.tsx`, `*.js`, `*.jsx`, `*.mts`, `*.cts`,
  `*.mjs` or `*.cjs`.
- **Test call**: a call of `test` or `it` (also `.only`, `.skip`; not `.todo`) not after a `.`, whose
  whole first argument is one string or one template without `${`. Its **name** joins the titles of the enclosing `describe` calls and
  its own by a space (a `describe` without such a title hides its calls); its **text** runs to its `)`.
- **Step test**: under junit, `<test file>::<name>` of a test call that the RED commit adds or whose text
  it changes; under cargo, as STP-4.
- **Base merge input**: any change that a merge commit of the Walk brings whose content at the merge
  equals its content at the merge's second parent, where that parent is an ancestor of the `base_branch`
  of `state.json`; under junit `.stapel/stapel.toml` and the command's script are never one.
- **Per-file run**: `command` with `{file}` replaced by one test file's path, run in the check copy
  after its old `report` file is removed.
- **Named test**: backtick text holding `::` in a table column headed `Test` or `Tests`, or in a Plan
  step's test list; its file part is a known cargo form with `::` (`core::config`), else its first
  segment, and the rest is the name. A **Checked test**: for `lint`, each Named test of a closed ticket, or
  of an open ticket's Plan step with a GREEN.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the ticket has a `base` THE `stapel check` and `stapel check --history` read step commits, their order and the build-input range only from the Walk, so a commit before `base` whose subject names a step of the ticket counts for nothing; without `base` it still counts. | `check_history::walk_starts_at_the_base` |
| AC-2 | WHEN the commit hook or `stapel next` reads the steps of a ticket with a `base` THE same Walk applies: a step commit before `base` gives no hook fault and no `next` fact or action. | `hook_post::hook_walks_from_the_base`, `next::next_walks_from_the_base` |
| AC-3 | WHEN the commits and changes after a check's HEAD touch only paths under `docs/`, the process ledger or a ticket's machine files (`tokens.jsonl` among them) THE check stays current in `status`, `next` and `close`; any other path makes it stale as before. | `check::docs_commits_keep_a_check_current` |
| AC-4 | WHEN `[check]` has `runner = "junit"` THE config reads `command` and `report` (both needed), `timeout_secs` as for cargo, `tests` (default `**/*.test.*`, `**/*.spec.*`, `**/__tests__/**`) and `inputs` (default `**/package.json`, the three lock files, `**/jest.config.*`, `**/tsconfig*.json`, `**/babel.config.*`, `**/.babelrc`, `**/.swcrc`, `**/jest.setup.*`); a cargo `[check]` reads as before. | `core::config::junit_check_keys` |
| AC-5 | IF `command` has no program before `{file}`, lacks the word `{file}`, holds it twice, holds it inside a longer word or right after a word that starts with `-`, `report` or a glob is empty, absolute or holds `..`, a glob holds `{`, `}`, `[`, `]`, `!` or a backslash, `tests` is empty, a junit key stands with cargo, `cargo` stands with junit, or the runner is another word THEN loading `stapel.toml` fails with a message that names the key; after a `-` word the message says to put `{file}` right after the program. | `core::config::rejects_bad_junit_keys`, `core::config::rejects_command_without_program` |
| AC-6 | WHEN a glob is matched THE leading `**/` also matches no folder (`**/package.json` matches `package.json`), `*` and `?` never match `/`, a trailing `/**` matches every path below at any depth but not the folder itself, and a middle `/**/` also matches no folder. | `core::config::globs_match_paths` |
| AC-7 | WHEN a test file is read THE test calls and their names come from a lexer that skips comments, string, template and regular-expression literals; `re.test(s)`, `test(` inside a string, comment or regular expression, a call whose first argument is not one literal (`test('a ' + x, …)`, `${…}`, a variable), `test.todo`, an `each` form, and a call under a `describe` without a literal title or with a modifier other than `.only` or `.skip` (`describe.each`, `.todo`, `.concurrent`, `.skip.each`) are no test calls. | `ts_tests::finds_test_calls_with_describe_titles`, `ts_tests::skips_comments_strings_and_regexes`, `ts_tests::titles_not_promised_are_skipped`, `ts_tests::describe_modifiers_hide_calls`, `ts_tests::describe_each_template_hides_calls`, `ts_tests::describe_scope_cases` |
| AC-8 | WHEN a report is read THE reader takes the elements `testsuites`, `testsuite`, `testcase`, `failure`, `error` and `skipped`, skips other elements with their content, and decodes the five XML entities, character references and CDATA. | `junit_report::reads_test_cases`, `junit_report::reads_unusual_valid_reports` |
| AC-9 | WHEN the runner is junit THE step tests of a RED commit are its added or changed test calls of test files; a test call whose text is unchanged is none, also when other lines of its file change. | `check_junit::step_tests_from_the_red_commit` |
| AC-10 | WHEN the runner is junit THE check makes one Per-file run per test file of the step at RED, GREEN and HEAD, with the `[check]` of the working tree's `stapel.toml`, its `node_modules` linked, and the cargo run's environment plus `JEST_JUNIT_REPORT_TEST_SUITE_ERRORS=true`. | `check_junit::red_to_green_passes` |
| AC-11 | WHEN a Per-file run's report is read THE step test takes the state of the one test case whose name equals its name, both trimmed: failed with `failure` or `error`, ignored with `skipped`, passed otherwise. | `check_junit::red_to_green_passes`, `check_junit::not_green_and_removed`, `check_junit::step_name_with_edge_space` |
| AC-12 | WHEN the states of the step tests at RED, GREEN and HEAD are read THE outcomes `no-red`, `not-green`, `removed` and `pass` keep their STP-4 meaning under junit. | `check_junit::not_green_and_removed` |
| AC-13 | WHEN the runner is junit THE code of `red-changes-code` is any path that matches no test glob and is not under `docs/` or `.stapel/` and is not a `.md` file. | `check_junit::history_outcomes_under_junit` |
| AC-14 | WHEN the runner is junit THE outcomes `unpaired`, `duplicate`, `no-tests`, `retired`, `interleaved` and the note `superseded by` keep their STP-4 and STP-14 meaning in `check` and `check --history`, and `tests-changed` protects each step test's text. | `check_junit::history_outcomes_under_junit` |
| AC-15 | WHEN every test case of a Per-file run's report has the classname `Test suite failed to run` THE file did not load, and its step tests fail there as a compile error of their cargo target does. | `check_junit::file_that_does_not_load_fails`, `check_junit::load_failure_edge_cases` |
| AC-16 | IF a Per-file run's report holds no test suite THEN the step tests of that file are `unverified` with the reason that the report of the file holds no test suite, whatever the exit code. | `check_junit::report_shape_is_unverified` |
| AC-17 | IF a Per-file run's report holds more than one test suite THEN the step tests of that file are `unverified` with the count of suites in the reason. | `check_junit::report_shape_is_unverified` |
| AC-18 | IF a step test has no test case or two in its Per-file run's report THEN the step is `unverified` with the name and the count in the reason. | `check_junit::report_shape_is_unverified` |
| AC-19 | IF a Per-file run exits 0 with a failed test case, or exits other than 0 with test cases none of which failed THEN the step is `unverified` with the exit code and the count of failures in the reason. | `check_junit::exit_code_must_agree`, `check_junit::exit_code_edge_cases` |
| AC-20 | IF the command of a Per-file run cannot start, runs out of time or dies by a signal THEN the step is `unverified` with the reason. | `check_junit::run_faults_are_unverified` |
| AC-21 | IF a Per-file run leaves no report, or one that is not a regular file, is over 16 MiB on disk or is refused by the reader THEN the step is `unverified` with the reason; kind and size are checked before the read. | `check_junit::run_faults_are_unverified`, `check_junit::report_size_checked_before_read`, `check_junit::report_must_be_a_regular_file` |
| AC-22 | IF any test file of the step holds a `test`, `it` or `describe` call with `.only` (also `.only.each` and a tagged template after `.only`) at the RED or the GREEN commit THEN the whole step is `unverified` with a reason that names the file, `.only` and the commit, and no command runs for it. | `check_junit::only_call_is_unverified`, `ts_tests::has_only_call_cases`, `check_junit::only_call_in_another_file` |
| AC-23 | IF the working tree has no `node_modules` folder THEN under junit the steps that run and the suite are `unverified` with the reason `node_modules is missing in the working tree`. | `check_junit::node_modules_missing_is_unverified` |
| AC-36 | IF the runner is junit and a path under `docs/` after a check's HEAD matches a test or input glob THEN that path makes the check stale. | `check_junit::docs_path_matching_a_glob_is_stale` |
| AC-24 | WHEN a commit of the Walk, a merge commit by its first-parent diff, changes a file that matches an input glob, and it is no Base merge input, THE later steps read `unverified` with the reason `build-input-changed: <path> at <sha>` as under cargo, and no command runs for them. | `check_junit::build_input_change_is_unverified`, `check_junit::merge_brings_a_build_input` |
| AC-25 | WHEN the runner is junit and a commit of the Walk after the ticket's first RED changes `.stapel/stapel.toml` THE steps whose RED lies before that commit read `build-input-changed` for it; a change at or before the first RED blocks no step. | `check_junit::config_change_is_unverified`, `check_junit::config_change_after_first_red` |
| AC-26 | WHEN the runner is junit THE suite at HEAD removes the old report, runs `command` once with only the word `{file}` left out, reads every test suite of the report and sums them; it passes when the exit code is 0 and the sums hold a test case and no failure or error, and its detail counts passed, failed and ignored. | `check_junit::suite_runs_the_whole_command`, `check_junit::suite_sums_every_suite` |
| AC-27 | WHEN the runner is junit THE outcome `tests-changed` also protects the rest of a test file whose rest the RED changed, and the other files the RED changed that match a test glob, in `check`, `check --history` and the commit hook; the gate that decides whether the RED changed the rest ignores whitespace and `;`, the protected rest is then compared exactly, and a deleted protected file is a change; a RED that changes only test calls of a file leaves its rest unprotected, as the cargo helper rule does. | `check_junit::tests_changed_under_junit`, `check_junit::hook_faults_under_junit`, `check_junit::same_file_helper_is_not_protected` |
| AC-28 | WHEN `stapel lint [KEY]` runs THE ticket is KEY; without KEY it is the branch ticket, else the one open ticket. | `lint::lint_exit_codes_and_order` |
| AC-29 | IF a Checked test does not exist at HEAD, its file missing included THEN lint prints `<KEY> names <test>, which does not exist`. | `lint::names_a_missing_test`, `lint::named_tests_from_test_columns`, `lint::rows_without_header_name_no_test` |
| AC-30 | IF a step test of the ticket's RED commits on the Walk exists at HEAD and no ticket of the checkout names it in a table row THEN lint prints `<test> is a step test of <KEY> and no ticket names it`. | `lint::step_test_named_by_no_ticket` |
| AC-31 | WHEN lint compares names THE cargo forms are `<file stem>::<name>`, `<crate suffix>::<file stem>::<name>` and `<dir>/<file stem>::<name>`, where the name may hold `::` (`x::m::t` is test `m::t` of file `x`), and the junit form is `<test file>::<name>`. | `lint::step_test_named_by_no_ticket`, `lint::lint_under_junit`, `lint::module_path_names` |
| AC-32 | WHEN lint ends THE missing tests come first, then the unnamed step tests, each group sorted by name, and the exit code is 1; with none it prints `lint: <KEY> clean` and exits 0; IF KEY is unknown, or without KEY there is no branch ticket and not exactly one open ticket THEN it refuses with a message and exit 2. | `lint::lint_exit_codes_and_order` |
| AC-33 | WHEN the runner is junit THE step test is `removed` when its file is missing at HEAD or the lexer finds no test call of its name there, before any HEAD run. | `check_junit::not_green_and_removed` |
| AC-34 | IF the suite run's command cannot start, runs out of time or dies by a signal, or leaves no report, or one that is not a regular file, is over 16 MiB on disk or is refused by the reader THEN the suite is `unverified` with the reason; kind and size are checked before the read. | `check_junit::suite_runs_the_whole_command`, `check_junit::report_must_be_a_regular_file` |
| AC-35 | WHEN a junit check ends THE record in `runs.jsonl` has `"runner": "junit"` and no cargo version, and a passing record is current in `status` and `next` as a cargo one is. | `check_junit::junit_record_is_current` |
| AC-37 | WHEN the ticket has a `base` THE `check`, `check --history`, the commit hook, `next` and `lint` run no `git log` or `rev-list` over the first-parent history beyond `base..HEAD`, the check of `base` included. | `check_history::walk_reads_only_the_range`, `check_history::base_edges` |
| AC-38 | IF a report is empty, not valid UTF-8, holds a DOCTYPE, an unknown entity, unbalanced tags, a second root, text outside the root, an unknown root, a known element outside its parent (a nested `testsuite`, a `testcase` under `testsuites`), or a character reference outside the XML 1.0 characters, with a sign, without digits or above `10FFFF` THEN the reader refuses it with a reason that names that cause, one reason per cause. | `junit_report::refuses_malformed_reports`, `junit_report::refuses_misplaced_elements`, `junit_report::refusals_name_their_cause`, `junit_report::refuses_bad_character_references`, `junit_report::character_reference_causes` |
| AC-39 | WHEN the first word of `command` is a repository path THE script runs as it is at the step's commit, and a change of it blocks a step only between that step's RED and GREEN. | `check_junit::config_change_is_unverified`, `check_junit::input_window_edges` |
| AC-40 | WHEN a junit check runs THE check and its suite take the machine-wide cargo lock as a cargo check does. | `cargo_lock::junit_check_takes_the_lock`, `cargo_lock::junit_lock_edges` |
| AC-41 | WHEN a build input or configuration change blocks a junit step THE commit hook and `next` name that change's commit as the step's fault commit. | `check_junit::fault_commit_is_the_blocking_change`, `check_junit::config_fault_commit` |
| AC-42 | WHEN the runner is junit and a GREEN or later commit changes, adds or deletes a file that matches a test glob, is no test file and lies in the folder of a step test's file THE step reads `tests-changed`. | `check_junit::fixture_beside_a_test_is_protected`, `check_junit::fixture_added_or_deleted_is_protected` |
| AC-43 | WHEN a change on the Walk is a Base merge input THE change blocks no step and not the suite, under cargo and junit; a merge that resolves it to other content, or a ticket without `base_branch`, blocks as before. | `check_history::merge_of_the_base_branch_blocks_nothing`, `check_junit::merge_of_the_base_branch_blocks_nothing` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-43; the cargo runner, its outcomes and its record are
  unchanged apart from the Walk; tickets STP-1 to STP-12 without `base` are checked as before.
- **Not promised:** the Frame's list; test calls under `test.each`, `describe.each`, `xit`, `fit` and
  aliases; JSX text that holds a quote or a backtick; a `node_modules` other than the root one (workspaces);
  a reporter whose test case `name` is not `<describe titles> <title>`; a `node_modules` tracked by git;
  a command that needs a shell (pipes, `&&`, quoting, variables); a report written after the command exits;
  a `base` that a rebase took off the first-parent line (the check refuses as before); a bound on the total
  check time (three runs per test file and step, under the machine-wide lock); `lint` for test names that
  hold `|` or a backtick; under junit a helper, `beforeEach` or `describe` title of a test file whose rest
  the RED did not change (the cargo helper rule has the same limit); a run script that is not the first word of `command` as a build input; under
  cargo, a `docs/` file that a test reads (here only this repository's lint reads `docs/`); an agent that
  merges its own side branch into the local base branch before merging that branch (subagents do not
  switch branches and only the orchestrator merges); the round-2 defects and coverage left to A1c (ledger
  2026-11-12): CR2-4, CR2-5, CR2-7, CR2-8, CR2-11, d20-2, d20-3, d21-1, d21-2, d22-1.

## Test plan

CLI tests in the new files `crates/stapel-cli/tests/check_junit.rs` and `lint.rs`; core tests in the new
files `crates/stapel-core/tests/ts_tests.rs` and `junit_report.rs`. A new helper `common::junit_repo`
(added in the RED of step 6) makes a repository with `[check] runner = "junit"`, `command = "sh
<stand-in> {file}"`, `report = "junit.xml"` and an ignored `node_modules`; the stand-in (sh and awk) lies
outside the repository, logs its runs and writes one test case per `it("…"` line, failed while a path the
line `needs` is missing; fault cases write their report by hand. No node or jest runs. Light tests: `stapel cargo test -j 4 -p
stapel-cli --test <file> -- --test-threads=3`, `-p stapel-core --test <file>`; before each GREEN also
`--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Walk in check | a commit `ABC-1 1 RED: …` before `base` and a real step 1 after it: `check` and `--history` show one pass, no `duplicate`; the same history without `base`: `duplicate`; a merge of main into the branch after `base` | `check_history::walk_starts_at_the_base` |
| Walk in hook, next | the same old commit: the hook after the next commit prints no fault for it; `next` names no fault and no action from it | `hook_post::hook_walks_from_the_base`, `next::next_walks_from_the_base` |
| Staleness | a passing check, then commits of a.md under `docs/`, the ledger and `tokens.jsonl`: `check: pass`; a commit of `src/x.rs`: stale; `docs/` plus `src/`: stale | `check::docs_commits_keep_a_check_current` |
| Staleness under junit | a passing junit check, then a commit of a.test.ts under `docs/`: stale; of site/package.json under `docs/`: stale; of a.md under `docs/`: current | `check_junit::docs_path_matching_a_glob_is_stale` |
| Config | the minimal junit `[check]` and its thirteen default globs; every key set; cargo `[check]` unchanged | `core::config::junit_check_keys` |
| Bad config | no `{file}`; `{file}` twice; `--x={file}`; `--testPathPattern {file}` (the message names the place); a glob with `{a,b}`, `[ab]`, `!x` or a backslash; `report = ""`, `"/x"`, `"../x"`; `tests = []`; a glob `"a/../b"`; `command` with cargo; `cargo` with junit; `runner = "jest"`: each refused naming its key | `core::config::rejects_bad_junit_keys` |
| Globs | `**/package.json` against `package.json` and `a/b/package.json`; `*.ts` against `a/b.ts` (no); `**/__tests__/**` against `__tests__/a.ts` and `x/__tests__/y/z.json`; `a/**` against `a` (no); `a/**/b` against `a/b`; `?` against `/` (no) | `core::config::globs_match_paths` |
| Lexer | `describe('a', () => { it('b', …) })`: `a b`; nested describes; `test.only`, `it.skip`; double, single and plain template titles; escapes `\'`, `\n`, `\u{e9}` decoded; a test call's text ends at its `)` | `ts_tests::finds_test_calls_with_describe_titles` |
| Lexer skips | `re.test(s)`; `"test('x')"`; `// test('x')`; `/* it('y') */`; `/test\('z'/`; a template literal holding `${test('q')}`; a division `a / b / c` beside a test; a regular expression after `(`, `=`, `return` | `ts_tests::skips_comments_strings_and_regexes` |
| Lexer not promised | a template title with `${x}`; `test('a ' + x, …)`; `test.todo('t')`; `test(name, …)`; `test.each(…)`; an `it` under a `describe` whose template title holds `${y}`: no test call | `ts_tests::titles_not_promised_are_skipped` |
| Report | the real `a.xml` (nested describe, pass, fail, skip, todo, a top-level test named ` top level`, titles with `&`, `"`, `<`, `>`) and `all.xml` (two suites); `&#233;`, CDATA; `system-out` and `properties` skipped | `junit_report::reads_test_cases` |
| Bad report | empty file; text; a DOCTYPE; `&foo;`; `<testcase>` not closed; a stray `</testsuite>`; invalid UTF-8: refused with a reason | `junit_report::refuses_malformed_reports` |
| Step tests | a RED that adds one test and edits the title of another; a RED that only edits a helper line of the file: `no-tests` | `check_junit::step_tests_from_the_red_commit` |
| Red to green | RED fails, GREEN and HEAD pass: `pass`; a path with a space is one word; the copy's `node_modules` is a link; the stand-in sees `PATH`, `HOME` and the jest-junit variable; the working tree's `command` runs; a name with a leading space matches | `check_junit::red_to_green_passes` |
| Not green, removed | GREEN still failing: `not-green`; a test passing at RED: `no-red`; `it.skip` at RED: `unverified`; the test call deleted later, and the whole file deleted: `removed`, no run at HEAD for it | `check_junit::not_green_and_removed` |
| History | no RED; two REDs; RED changes `src/lib.ts`: `red-changes-code`; RED also changes `README.md`: no fault; GREEN edits the step test call: `tests-changed`; a later RED edits it: `superseded by`; retired by `spec change:`; interleaved | `check_junit::history_outcomes_under_junit` |
| Not loaded | RED report shaped as the real `b2.xml` (two cases of classname `Test suite failed to run`), exit 1: `pass`, `RED 0 assert, 1 compile`; the same at GREEN: `not-green` | `check_junit::file_that_does_not_load_fails` |
| Report shape | no suite (the real `none.xml` and `b.xml`) with exit 1 and with exit 0; two suites; a name with two test cases; a name with none | `check_junit::report_shape_is_unverified` |
| Exit code | exit 0 with a failure; exit 1 with all passed | `check_junit::exit_code_must_agree` |
| Run faults | no program; a 10 s timeout on a sleeping stand-in; `kill -9 $$` in the stand-in; no report; an old report left in the copy and the stand-in writing none; 17 MiB; a refused report | `check_junit::run_faults_are_unverified` |
| Only | `test.only` at RED (as the real `c.xml`); `describe.only` added at GREEN: `unverified`, the stand-in not run for that file | `check_junit::only_call_is_unverified` |
| Dependencies | no `node_modules` in the working tree: the AC-23 reason; a GREEN that changes `package.json`: `build-input-changed`, the stand-in not run | `check_junit::node_modules_missing_is_unverified`, `check_junit::build_input_change_is_unverified` |
| Merge input | a merge of main after step 1 that brings `jest.config.js`: step 2 reads `build-input-changed` at the merge; step 1 passes | `check_junit::merge_brings_a_build_input` |
| Config change | a GREEN that changes `.stapel/stapel.toml`; `command = "./run.sh {file}"` and a GREEN that edits `run.sh`: `build-input-changed` | `check_junit::config_change_is_unverified` |
| Suite | the HEAD suite run without `{file}`: `suite: pass` with counts; a failing case: `suite: fail`; a committed passing report and a stand-in that writes none: `unverified`; a timeout; a signal; a refused report: `unverified` | `check_junit::suite_runs_the_whole_command` |
| Record | a junit pass: `"runner": "junit"`, no cargo version; `status` and `next` show the check current | `check_junit::junit_record_is_current` |
| Suite sums | a suite report with two suites of 2 and 3 cases: `5 passed`; one failure in the second: `suite: fail` | `check_junit::suite_sums_every_suite` |
| Protected files | GREEN edits a helper line of the test file; GREEN edits a fixture `__tests__/data.json` the RED added: `tests-changed` | `check_junit::tests_changed_under_junit` |
| Hook | a GREEN that edits a helper line of the test file: the hook prints the `tests-changed` fault | `check_junit::hook_faults_under_junit` |
| Lint missing | a closed ticket naming `x::gone` in a table row; an open ticket whose step 1 has a GREEN and names a missing test: reported; step 2 without GREEN: not reported | `lint::names_a_missing_test` |
| Lint unnamed | a step test no ticket names: reported; named as `x::t`, `core::x::t` or `stapel-core/x::t`: not reported | `lint::step_test_named_by_no_ticket` |
| Lint junit | `src/a.test.ts::a b` named in a row: clean; unnamed: reported | `lint::lint_under_junit` |
| Lint exit | two missing and two unnamed: that order, sorted, exit 1; clean: exit 0; on branch `ABC-2` without KEY: `ABC-2`; `lint ABC-9` unknown, no ticket, and two open tickets on `main`: exit 2 with a message; a Named test in a Plan step only | `lint::lint_exit_codes_and_order` |
| Walk speed | `GIT_TRACE` to a file for `check`, `check --history`, the hook, `next` and `lint` of a ticket with a base: no `rev-list` or `log` over the first-parent history without `base..` | `check_history::walk_reads_only_the_range` |
| No program | `command = "{file}"`; `command = " {file}"`: refused naming `command` | `core::config::rejects_command_without_program` |
| Describe forms | an `it` under `describe.each([…])('t', …)`, `describe.todo`, `describe.concurrent`, `describe.skip.each`: no test call; under `describe.skip` and `describe.only`: a test call | `ts_tests::describe_modifiers_hide_calls` |
| Only forms | `test.only(`, `it.only (`, `describe.only(`, `test.only.each(`, `test.only` followed by a tagged template, `describe.skip(`: only the first five; `.only` in a comment, string or regular expression and `foo.test.only(`: none | `ts_tests::has_only_call_cases` |
| Reader edges | BOM; single quotes; spaces around `=`; a foreign encoding declaration; processing instructions; a `testcase` inside an unknown element (skipped); mixed outcomes; a missing `name` | `junit_report::reads_unusual_valid_reports` |
| Misplaced | a nested `testsuite`; a `testcase` under `testsuites`; a second root; text outside the root; an unknown root; invalid UTF-8; an empty file; 16 MiB plus one byte | `junit_report::refuses_misplaced_elements` |
| Causes | each refusal of `refuses_malformed_reports` and `refuses_misplaced_elements` asserts its own cause text; an unterminated comment and CDATA; UTF-16 | `junit_report::refusals_name_their_cause` |
| Character references | `&#0;`, `&#x1;`, `&#xFFFE;`, `&#+65;`, `&#-65;`: refused; `&#9;`, `&#x10FFFF;`: read | `junit_report::refuses_bad_character_references` |
| Edge space | a title `"a "`: the report's `name` ` a ` matches; `pass` | `check_junit::step_name_with_edge_space` |
| Load edges | a load-failure report with exit 0: `unverified` (AC-19 first); a load-failure case beside normal cases: read as normal cases | `check_junit::load_failure_edge_cases` |
| Exit edges | exit 1 with only skipped cases; exit 1 with one suite of zero cases: `unverified` | `check_junit::exit_code_edge_cases` |
| Size first | a sparse 17 MiB report without read permission: the over-16-MiB reason, not a read error | `check_junit::report_size_checked_before_read` |
| Only elsewhere | a step with two test files, `.only` in the second at GREEN: the whole step `unverified`, no run | `check_junit::only_call_in_another_file` |
| Lock | a held machine-wide lock: a junit `stapel check` waits with the wait line and runs after the release | `cargo_lock::junit_check_takes_the_lock` |
| Config window | `stapel.toml` changed before the first RED: no block; after a step's GREEN, between steps and after the last GREEN: every step whose RED lies before it reads `build-input-changed` | `check_junit::config_change_after_first_red` |
| Input window | a plain input change before RED, inside RED, between RED and GREEN, in GREEN, after GREEN; a script change inside and outside its step's window | `check_junit::input_window_edges` |
| Fault commit | an input change in step 2's GREEN after a passing step 1: the hook and `next` name that commit, not an earlier one | `check_junit::fault_commit_is_the_blocking_change` |
| Same-file helper | a RED that only adds a test call; a GREEN that edits a helper of that file: `--history` gives `needs-run`; a RED that only reformats the rest (spaces, `;`): the same | `check_junit::same_file_helper_is_not_protected` |
| Fixture beside | `__tests__/data.json` beside the step test file, untouched by the RED, edited by the GREEN: `tests-changed`; a fixture in another folder: no fault | `check_junit::fixture_beside_a_test_is_protected` |
| Test columns | a closed ticket naming `x::t` in a `Tests` column after `tests/x.rs` was deleted: reported; a code name such as check::run_check in a `Design` cell: ignored; a Plan step's test list counts | `lint::named_tests_from_test_columns` |
| Module paths | a cargo test `m::t` in `mod m` of `tests/x.rs` named `x::m::t`: found; as a step test: named, no fault; a name of the core config test file still resolves to that file | `lint::module_path_names` |
| Base merge | cargo: a ticket with `base_branch` main merges main, which added a test file new.rs under the tests folder of a crate: every step and the suite run, no `build-input-changed`; the same merge resolving the file to other content: blocked at the merge | `check_history::merge_of_the_base_branch_blocks_nothing` |
| Base merge junit | a merge of main that brings `jest.config.js` unchanged: no block; resolved to other content: blocked; the same merge in a ticket without `base_branch`: blocked | `check_junit::merge_of_the_base_branch_blocks_nothing` |
| Reference causes | `&#0;` and `&#xFFFE;` (not an XML character), `&#+65;` (a sign), `&#;` and `&#x;` (no digits), `&#x110000;` (too large): four different reasons, each asserted | `junit_report::character_reference_causes` |
| Report kind | a FIFO, a folder and a symbolic link at the report path, for a Per-file run and for the suite: `unverified` with the kind in the reason, no read blocks | `check_junit::report_must_be_a_regular_file` |
| Fixture added or deleted | a GREEN that adds `__tests__/data.json` beside the step test file; a later commit that deletes one the RED left alone: `tests-changed` | `check_junit::fixture_added_or_deleted_is_protected` |
| Header-less rows | a ticket whose only table has no separator line and names a missing test: no fault; the old lint fixtures get header rows and such a row each | `lint::rows_without_header_name_no_test` |
| Base edges | a `base` after HEAD: refused; HEAD equal to `base` (an empty Walk): accepted | `check_history::base_edges` |
| Each template | an `it` under `describe.each` written with a tagged template table: no test call | `ts_tests::describe_each_template_hides_calls` |
| Scope | a hidden `describe` inside a titled one and the reverse; a titled `describe` after a hidden one: names as the scopes say | `ts_tests::describe_scope_cases` |
| Lock edges | the holder line names the junit check; the held mark reaches the Per-file and suite commands; a junit check with the mark set takes no lock; the lock is free after a refused check | `cargo_lock::junit_lock_edges` |
| Config fault | a `stapel.toml` change after step 2's RED: the hook and `next` name that commit | `check_junit::config_fault_commit` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `runner` | string | `cargo` or `junit` | absent: `cargo` | other word: refused | exact | `core::config::rejects_bad_junit_keys` |
| `command` | string of words | one word plus `{file}` / any | refused | `{file}` missing, twice, inside a word or after a word starting with `-`: refused, the message names the place after the program | split on spaces, no shell; the suite run drops only the word `{file}` | `core::config::rejects_bad_junit_keys` |
| `report` | relative path | one part / any | refused | absolute or `..`: refused | as given | `core::config::rejects_bad_junit_keys` |
| `tests`, `inputs` | globs | one / any | `tests = []` refused; `inputs = []` allowed | empty, absolute, `..`, `{`, `}`, `[`, `]`, `!` or backslash: refused | `*`, `?`, `**` | `core::config::globs_match_paths` |
| `timeout_secs` | seconds | 10 / 86400 | absent: 900 | out of range: refused as before | whole seconds, per run | `check_junit::run_faults_are_unverified` |
| test title | string or template literal | `''` / any | an empty name matches an empty trimmed `name` | `${…}`, a variable: no test call | escapes decoded; compared trimmed | `ts_tests::finds_test_calls_with_describe_titles` |
| test file path | repository path | — | — | a space: passed as one word; non-UTF-8: not promised | as committed | `check_junit::red_to_green_passes` |
| report file | XML | — / 16 MiB | refused | AC-8 and AC-21 refusals | exact names | `junit_report::refuses_malformed_reports` |
| `lint` KEY | ticket key | as `STP-2` | absent: branch ticket or the one open ticket | unknown or none: exit 2 | as for `check` | `lint::lint_exit_codes_and_order` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | history shape: merge commits in `base..HEAD` | the first-parent Walk passes the merge of main; its first-parent diff counts | `check_history::walk_starts_at_the_base`, `check_junit::merge_brings_a_build_input` |
| git | history shape: rebase, squash, amend | `base` off the first-parent line: the refusal of STP-13; otherwise not promised | — |
| git | ticket without `base` (STP-1 to STP-12) | the walk from the root | `check_history::walk_starts_at_the_base` |
| git | shallow clone, SHA-256, submodules | not promised | — |
| git | linked worktrees | the check copy as before | — |
| git | paths with spaces, glob characters, non-UTF-8 | a space handled; non-UTF-8 not promised | `check_junit::red_to_green_passes` |
| git | dirty tree | `node_modules` and `stapel.toml` come from the working tree | `check_junit::red_to_green_passes` |
| git | commit messages with control bytes; configuration | as before | — |
| junit | command exit code | checked against the failures; a signal or timeout: `unverified` | `check_junit::exit_code_must_agree`, `check_junit::run_faults_are_unverified` |
| junit | report missing, empty, malformed, over 16 MiB; left by an earlier run or committed | `unverified`; an old report is removed before each run | `check_junit::run_faults_are_unverified`, `junit_report::refuses_malformed_reports` |
| junit | `skipped`, `error`, `failure` test cases; XML-escaped titles; a top-level name with a leading space | ignored, failed, failed; decoded; compared trimmed | `junit_report::reads_test_cases` |
| junit | a file that does not load: no suite by default; with `JEST_JUNIT_REPORT_TEST_SUITE_ERRORS=true` cases of classname `Test suite failed to run` | the run sets the variable; such cases fail the file's step tests (AC-15) | `check_junit::file_that_does_not_load_fails` |
| junit | no suite (no test found, e.g. a path with regular-expression characters taken as a pattern, or a load failure without the variable); several suites per Per-file run; duplicate test case names | `unverified` (AC-16 to AC-18) | `check_junit::report_shape_is_unverified` |
| junit | several suites in the HEAD suite run | summed (AC-26) | `check_junit::suite_sums_every_suite` |
| junit | name templates other than `<describe titles> <title>` | not promised | — |
| node | `node_modules` missing in the working tree | `unverified` (AC-23) | `check_junit::node_modules_missing_is_unverified` |
| node | `package.json`, lock files, jest, babel, swc and TypeScript config changed in the range | `build-input-changed` | `check_junit::build_input_change_is_unverified` |
| node | jest is a node script | `PATH` and `HOME` kept | `check_junit::red_to_green_passes` |
| node | `test.todo` reported as passed | `.todo` is no test call | `ts_tests::titles_not_promised_are_skipped` |
| node | workspaces with nested `node_modules`; a stale `node_modules` | not promised | — |
| os | symlinks | the `node_modules` link; a test file that is a link: not promised | `check_junit::red_to_green_passes` |
| os | processes: process group, children that leave it | the group killed at the timeout as for cargo; leavers not promised | `check_junit::run_faults_are_unverified` |
| os | case-insensitive file systems; WSL Windows `node` on `PATH` | not promised | — |
| cargo | every dimension | as before | — |

### Abuse

| Attempt of a careless agent | Outcome | Test |
|---|---|---|
| RED adds `it.skip(…)` | `unverified: ignored at RED` | `check_junit::not_green_and_removed` |
| RED adds only `test.todo(…)` | `no-tests` | `check_junit::history_outcomes_under_junit` |
| RED adds `test.only(…)`, so other step tests of the file are skipped | `unverified` (AC-22) | `check_junit::only_call_is_unverified` |
| RED also changes `src/lib.ts` | `red-changes-code` | `check_junit::history_outcomes_under_junit` |
| GREEN edits the test call | `tests-changed` | `check_junit::history_outcomes_under_junit` |
| GREEN edits a helper or `describe` title of a test file whose rest the RED changed | `tests-changed` | `check_junit::tests_changed_under_junit` |
| GREEN edits a helper of a file where the RED only added a test call | not protected, `needs-run` (AC-27 limit) | `check_junit::same_file_helper_is_not_protected` |
| GREEN edits a fixture beside the step test file | `tests-changed` | `check_junit::fixture_beside_a_test_is_protected` |
| GREEN adds the fixture its RED test reads | `tests-changed` | `check_junit::fixture_added_or_deleted_is_protected` |
| merges main and resolves a merged build input to other content | `build-input-changed` at the merge | `check_junit::merge_of_the_base_branch_blocks_nothing` |
| GREEN adds `test.only.each` in another test file of the step | `unverified` (AC-22) | `check_junit::only_call_in_another_file` |
| changes `command` in `stapel.toml` after the steps are built | `build-input-changed` for the earlier steps | `check_junit::config_change_after_first_red` |
| GREEN adds `passWithNoTests` to `jest.config.js` | `build-input-changed` | `check_junit::build_input_change_is_unverified` |
| GREEN changes `command` in `stapel.toml` or edits the run script | `build-input-changed` | `check_junit::config_change_is_unverified` |
| commits a passing `junit.xml` | removed before the run | `check_junit::run_faults_are_unverified` |
| writes a test with a `${…}` title only | `no-tests` | `check_junit::history_outcomes_under_junit` |
| deletes the step test after GREEN | `removed` | `check_junit::not_green_and_removed` |
| adds a test that no ticket names | `lint` exits 1 | `lint::step_test_named_by_no_ticket` |
| names a test in the ticket that is never written | `lint` exits 1 once the step has a GREEN | `lint::names_a_missing_test` |

**Review Focus:** JSX text with an apostrophe in a `.tsx` test file (the lexer may read a string to the next
quote; not promised, not tested); a regular expression right after `)` (`if (x) /a/.test(s)`), read as a
division; the trimmed comparison of names that differ only in inner spaces; a file that matches both a test
glob and an input glob (the input rule comes first); a `stapel.toml` edited in the working tree between two
checks (the second check runs the new command for old commits).

**Author self-check:** 2026-11-11, before spec review, again after rounds 1 and 2, code review round 1 and, on 2026-11-12, the merge of main: promise item 1 maps to
AC-4 to AC-6, AC-10, AC-23 to AC-26, AC-34, AC-35; item 2 to AC-7 and AC-9; item 3 to AC-8, AC-11 to
AC-22, AC-27, AC-33, AC-38 to AC-43; item 4 to
AC-1, AC-2 and AC-37; item 5 to AC-3; item 6 to AC-28 to AC-32; each Inputs and External states row has a test or
"not promised"; readers of the changed configuration (`[check]` with junit keys) are `config.rs`,
`check.rs`, `checkstate.rs`, `next.rs`, `init` (the starter text stays cargo) and the commit hook.

## Design

### Facts

- F-1: `Check` (`crates/stapel-core/src/config.rs`, `deny_unknown_fields`) has `runner`, `timeout_secs`
  and `cargo`; `validate` accepts only `runner = "cargo"`.
- F-2: `check.rs` `Runner` is cargo-only: `at` groups step tests by file into `cargo test --locked -p
  <package> --test <file> -- --exact <names>`; `suite` runs `cargo test --workspace --locked`.
- F-3: `runner.rs` `run_cargo` runs one program in its own process group with an empty environment plus
  `ENV_ALLOW`, a timeout and a 16 MiB output cap; `parse_run` gives `TestState` and `Parsed`.
- F-4: `steps.rs` `TestId {dir, file, name}` fixes the path to `crates/<dir>/tests/<file>.rs`;
  `step_tests` uses `rust_tests::test_functions`.
- F-5: `outcomes.rs` `analyse` and `is_code_stp4`/`under_tests` are Rust-layout specific; `judge_red`,
  the protected units and `touching` carry the STP-4 and STP-14 outcomes.
- F-6: full first-parent walks: `steps::step_commits`, `outcomes.rs` order map,
  `build_inputs::check_base` and `range`, `check::hook_faults`, `next.rs` subjects; `State.base` exists
  since STP-13.
- F-7: `checkstate::current` judges staleness by `git diff --name-only <head>` minus
  `worktree::is_machine_file`, which `stapel new` and the dirty test also use.
- F-8: the workspace has no XML or glob crate; `stapel check` builds with `--locked`.
- F-10: real jest-junit 16.0.0 runs (`.stapel/scratch/junit-fixtures/`): a file that does not load gives
  no suite by default and exit 1, as a path that matches nothing; `test.todo` is a passed case; a top-level
  test is named ` <title>`; titles are XML-escaped; with `.only` the other tests are `skipped`, exit 0.
- F-9: `build_inputs::analyse` reads each commit of the range by its first-parent diff, so a merge of main
  shows the files it brings.

### Decisions (before the build)

- The JUnit reader is our own (`crates/stapel-core/src/junit.rs`, about 200 lines) for the subset of
  AC-8, not `quick-xml`: no new dependency or network for `--locked` builds, and an own strict reader
  fails closed on what it does not know. The glob matcher is our own too (`config.rs`, `*`, `?`, `**`).
- The lexer is `crates/stapel-core/src/ts_tests.rs` with `test_calls(text) -> Vec<(name, text)>` and
  `rest_text(text)` (the file without its test calls), as `rust_tests` has for Rust. A `/` starts a
  regular expression after start, `(`, `,`, `=`, `:`, `[`, `!`, `&`, `|`, `?`, `{`, `}`, `;`, an operator
  or `return`, `typeof`, `case`, `in`, `of`; else it is a division. It also reports `.only` calls.
- `TestId` keeps `dir`, `file`, `name`; a junit test has an empty `dir` and the repository path as
  `file` (as built), so cargo prints `dir/file::name` as before and junit `path::name`.
- `run_cargo` becomes `run_program` with the same environment, group, timeout and cap; `Runner` holds a
  kind (cargo or junit). The reader returns every suite; the Per-file run applies AC-15 to AC-19 to it, the
  suite run sums it (SR1-1).
- The run's environment is the cargo run's `ENV_ALLOW` (it holds `PATH` and `HOME`, which jest as a node
  script needs) plus `JEST_JUNIT_REPORT_TEST_SUITE_ERRORS=true`; the variable is jest-junit's and other
  reporters ignore it.
- The real reports of F-10 are copied to `crates/stapel-core/tests/data/junit/` by the RED of step 5.
- `[check]` is read once from the working tree's `stapel.toml` when the check starts; the commits under
  test cannot change it, and a change of it inside the Walk is a build input (AC-25, SR1-8).
- Under junit `stapel check` takes the machine-wide cargo lock of STP-27 too, because the runs are as
  heavy (as built by step 12 it took none: d6-1, fixed in step 16); the check record has `"runner":
  "junit"` and no cargo version.
- Until step 10 the junit suite reads `unverified` with the reason that the suite is not built; step 10
  replaces it.
- `step_commits`, `analyse`, `build_inputs::range` and `hook_faults` take the base and walk `base..HEAD`;
  `check_base` checks that the oldest commit of the Walk has `base` as first parent, without a full walk
  (as built by step 12 `check_base`, `range` and the position maps of `analyse_in` and `hook_faults`
  still read the whole first-parent history: d1-1, d1-2, CR1-2, fixed in step 13).
- `checkstate::current` drops `docs/` paths besides machine files; `is_machine_file` stays, so `stapel new`
  and the guard keep their rules.
- `stapel lint` is `crates/stapel-cli/src/lint.rs`; `ticket_drift.rs` stays this repository's full lint.

### Design as built (steps 1 to 23)

- `outcomes.rs`: `Mode::{Cargo, Junit(&[String])}` picks the rules; `analyse_in` takes it. Under junit
  `is_junit_code` judges the RED (AC-13), step tests come from `ts_tests::test_calls`, and the protected
  units add `Unit::Rest` and `Unit::File`. The gate compares the rest before and at the RED with
  whitespace and `;` removed; the protected rest is then compared exactly at each later commit. For each
  folder of a step test file, every non-test file matching a test glob, also one a later commit adds or
  deletes, is a `Unit::File`; a deletion counts as a change (AC-42, AC-27).
- `build_inputs.rs`: `check_base` reads only `base..HEAD` and checks that `base` is the first parent of
  its oldest commit (an empty range needs HEAD equal to `base`); `range` and the position maps read the
  same range. `base_merge_parent` gives a merge's second parent when it is an ancestor of
  `refs/heads/<base_branch>`; `same_as_parent` compares a path there and at the merge. Under cargo every
  hit of such a merge is skipped when all its paths are unchanged; `analyse_junit` skips an input-glob
  path so, but never `.stapel/stapel.toml` or the script (AC-43).
- `check.rs`: `junit_block` blocks a step by the first `walk_changes` entry: an input at or before its
  GREEN; `.stapel/stapel.toml` anywhere after its RED; the script after its RED and not after its GREEN.
  The hook and `next` take that change as the fault commit (AC-41). `at_junit` and `suite_junit` run
  `run_program` with `JEST_JUNIT_REPORT_TEST_SUITE_ERRORS=true`, remove the old report first and read it
  through `read_report_file`, which refuses a symbolic link, a folder, a FIFO or another non-regular file
  and a size over 16 MiB from `symlink_metadata` before the read. Both names are trimmed before the
  compare. `link_modules` links the working tree's `node_modules`. A check under either runner takes the
  machine-wide cargo lock and sets the held mark for its children (AC-40).
- `junit.rs`: `read_report` is a single-pass reader with a stack of open elements; a character reference
  is refused with its own cause: a sign, no digits, above `10FFFF`, or no XML 1.0 character (AC-38).
- `ts_tests.rs`: a token lexer; a `describe` scope ends at its closing token; a `describe` with a
  modifier other than `.only` or `.skip`, or an `each` form, hides its calls; `has_only_call` counts
  `.only` followed by a call, `.each` or a tagged template.
- `lint.rs`: Named tests come only from columns headed `Test` or `Tests` of tables with a separator line,
  and from the test lists of Plan steps; the file part is a known cargo form or the first segment, a
  name may hold `::` (`nested_test` follows `mod` blocks), and a name whose file is missing is reported.
- Tests: `common::junit_repo` and its stand-in outside the repository (`JunitRepo::runs` reads its log);
  the real reports of F-10 lie in the core crate's `tests/data/junit/`.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check_history::walk_starts_at_the_base` |
| AC-2 | `hook_post::hook_walks_from_the_base`, `next::next_walks_from_the_base` |
| AC-3 | `check::docs_commits_keep_a_check_current` |
| AC-4 | `core::config::junit_check_keys` |
| AC-5 | `core::config::rejects_bad_junit_keys`, `core::config::rejects_command_without_program` |
| AC-6 | `core::config::globs_match_paths` |
| AC-7 | `ts_tests::finds_test_calls_with_describe_titles`, `ts_tests::skips_comments_strings_and_regexes`, `ts_tests::titles_not_promised_are_skipped`, `ts_tests::describe_modifiers_hide_calls`, `ts_tests::describe_each_template_hides_calls`, `ts_tests::describe_scope_cases` |
| AC-8 | `junit_report::reads_test_cases`, `junit_report::reads_unusual_valid_reports` |
| AC-9 | `check_junit::step_tests_from_the_red_commit` |
| AC-10 | `check_junit::red_to_green_passes` |
| AC-11 | `check_junit::red_to_green_passes`, `check_junit::not_green_and_removed`, `check_junit::step_name_with_edge_space` |
| AC-12 | `check_junit::not_green_and_removed` |
| AC-13 | `check_junit::history_outcomes_under_junit` |
| AC-14 | `check_junit::history_outcomes_under_junit` |
| AC-15 | `check_junit::file_that_does_not_load_fails`, `check_junit::load_failure_edge_cases` |
| AC-16 | `check_junit::report_shape_is_unverified` |
| AC-17 | `check_junit::report_shape_is_unverified` |
| AC-18 | `check_junit::report_shape_is_unverified` |
| AC-19 | `check_junit::exit_code_must_agree`, `check_junit::exit_code_edge_cases` |
| AC-20 | `check_junit::run_faults_are_unverified` |
| AC-21 | `check_junit::run_faults_are_unverified`, `check_junit::report_size_checked_before_read`, `check_junit::report_must_be_a_regular_file` |
| AC-22 | `check_junit::only_call_is_unverified`, `ts_tests::has_only_call_cases`, `check_junit::only_call_in_another_file` |
| AC-23 | `check_junit::node_modules_missing_is_unverified` |
| AC-24 | `check_junit::build_input_change_is_unverified`, `check_junit::merge_brings_a_build_input` |
| AC-36 | `check_junit::docs_path_matching_a_glob_is_stale` |
| AC-25 | `check_junit::config_change_is_unverified`, `check_junit::config_change_after_first_red` |
| AC-26 | `check_junit::suite_runs_the_whole_command`, `check_junit::suite_sums_every_suite` |
| AC-27 | `check_junit::tests_changed_under_junit`, `check_junit::hook_faults_under_junit`, `check_junit::same_file_helper_is_not_protected` |
| AC-28 | `lint::lint_exit_codes_and_order` |
| AC-29 | `lint::names_a_missing_test`, `lint::named_tests_from_test_columns`, `lint::rows_without_header_name_no_test` |
| AC-30 | `lint::step_test_named_by_no_ticket` |
| AC-31 | `lint::step_test_named_by_no_ticket`, `lint::lint_under_junit`, `lint::module_path_names` |
| AC-32 | `lint::lint_exit_codes_and_order` |
| AC-33 | `check_junit::not_green_and_removed` |
| AC-34 | `check_junit::suite_runs_the_whole_command`, `check_junit::report_must_be_a_regular_file` |
| AC-35 | `check_junit::junit_record_is_current` |
| AC-37 | `check_history::walk_reads_only_the_range`, `check_history::base_edges` |
| AC-38 | `junit_report::refuses_malformed_reports`, `junit_report::refuses_misplaced_elements`, `junit_report::refusals_name_their_cause`, `junit_report::refuses_bad_character_references`, `junit_report::character_reference_causes` |
| AC-39 | `check_junit::config_change_is_unverified`, `check_junit::input_window_edges` |
| AC-40 | `cargo_lock::junit_check_takes_the_lock`, `cargo_lock::junit_lock_edges` |
| AC-41 | `check_junit::fault_commit_is_the_blocking_change`, `check_junit::config_fault_commit` |
| AC-42 | `check_junit::fixture_beside_a_test_is_protected`, `check_junit::fixture_added_or_deleted_is_protected` |
| AC-43 | `check_history::merge_of_the_base_branch_blocks_nothing`, `check_junit::merge_of_the_base_branch_blocks_nothing` |

Manual smoke check before the close: `stapel check` of one step on the pilot repository with jest 29 and
jest-junit 16 (SR2-5).

## Plan

Route: full (tags `guard`, `interface`), auto.

1. The Walk (AC-1, AC-2; tests `check_history::walk_starts_at_the_base`,
   `hook_post::hook_walks_from_the_base`, `next::next_walks_from_the_base`): `steps.rs`, `outcomes.rs`,
   `build_inputs.rs`, `check.rs`, `next.rs`. Must not change: outcomes of a ticket without `base`.
2. `docs/` keeps a check current (AC-3; test `check::docs_commits_keep_a_check_current`): `checkstate.rs`.
   Must not change: `is_machine_file` and its other callers.
3. junit keys and globs (AC-4, AC-5, AC-6; tests `core::config::junit_check_keys`,
   `core::config::rejects_bad_junit_keys`, `core::config::globs_match_paths`): `config.rs`. Must not
   change: cargo `[check]` and the starter text.
4. The lexer (AC-7; tests `ts_tests::finds_test_calls_with_describe_titles`,
   `ts_tests::skips_comments_strings_and_regexes`, `ts_tests::titles_not_promised_are_skipped`):
   `ts_tests.rs`. Must not change: `rust_tests.rs`.
5. The report reader (AC-8; tests `junit_report::reads_test_cases`,
   `junit_report::refuses_malformed_reports`): `junit.rs`; the RED adds the real reports. Must not change: `parse_run`.
6. The junit run (AC-9 to AC-12, AC-33; tests `check_junit::step_tests_from_the_red_commit`,
   `check_junit::red_to_green_passes`, `check_junit::not_green_and_removed`); the RED adds
   `common::junit_repo`. Each test fails at the RED: `check` has no junit run yet. Must not change: cargo
   output (its goldens); the protected units (step 11).
7. Report shape and exit code (AC-15 to AC-19; tests `check_junit::file_that_does_not_load_fails`,
   `check_junit::report_shape_is_unverified`, `check_junit::exit_code_must_agree`): each test asserts a
   reason or a compile count that step 6 does not give. Must not change: step 6.
8. Run faults and `.only` (AC-20, AC-21, AC-22; tests `check_junit::run_faults_are_unverified`,
   `check_junit::only_call_is_unverified`). Must not change: steps 6 and 7.
9. Dependencies and build inputs (AC-23, AC-24, AC-25, AC-36; tests
   `check_junit::node_modules_missing_is_unverified`, `check_junit::build_input_change_is_unverified`,
   `check_junit::merge_brings_a_build_input`, `check_junit::config_change_is_unverified`,
   `check_junit::docs_path_matching_a_glob_is_stale`): `build_inputs.rs`, `checkstate.rs` (the `docs/`
   exemption of step 2 yields to the junit globs). Must not change: cargo build inputs.
10. The suite and the record (AC-26, AC-34, AC-35; tests `check_junit::suite_runs_the_whole_command`,
    `check_junit::suite_sums_every_suite`, `check_junit::junit_record_is_current`): `Runner::suite`, the
    record; the record test fails at the RED because no junit check passes before the suite. Must not change: the cargo suite.
11. History outcomes under junit (AC-13, AC-14, AC-27; tests `check_junit::history_outcomes_under_junit`,
    `check_junit::tests_changed_under_junit`, `check_junit::hook_faults_under_junit`): each fails at the
    RED, as step 6 protects no test text and each edits a test call, a helper line or a fixture after the
    RED. Must not change: cargo outcomes.
12. `stapel lint` (AC-28 to AC-32; tests `lint::names_a_missing_test`, `lint::step_test_named_by_no_ticket`,
    `lint::lint_under_junit`, `lint::lint_exit_codes_and_order`): `lint.rs`, `main.rs`. Must not change:
    `ticket_drift.rs`.
13. Walk speed (AC-37; test `check_history::walk_reads_only_the_range`): `check_base` asks whether `base`
    is the first parent of the oldest Walk commit; `range` and the position maps of `analyse_in` and
    `hook_faults` read `base..HEAD`. Must not change: outcomes, and the walk of a ticket without `base`.
14. Config and lexer (AC-5, AC-7, AC-22; tests `ts_tests::describe_modifiers_hide_calls`,
    `ts_tests::has_only_call_cases`): describe modifiers and `each` forms hide calls; `.only.each` and a
    tagged template count. A coverage commit before the RED adds `core::config::rejects_command_without_program`
    (the refusal already holds). Must not change: `test_calls` names of steps 4 to 12.
15. Reader (AC-38; test `junit_report::refuses_bad_character_references`): character references outside
    XML 1.0 or with a sign are refused. A coverage commit adds `junit_report::refuses_misplaced_elements`,
    `junit_report::refusals_name_their_cause` and `junit_report::reads_unusual_valid_reports` (AC-8,
    AC-38; the behaviour holds). Must not change: the reading of the real reports.
16. Run and report (AC-11, AC-21, AC-40; tests `check_junit::step_name_with_edge_space`,
    `check_junit::report_size_checked_before_read`, `cargo_lock::junit_check_takes_the_lock`): both names
    trimmed, the size read from metadata (the dead signal branch goes), the lock taken under junit. A
    coverage commit adds `check_junit::exit_code_edge_cases`, `check_junit::load_failure_edge_cases` and
    `check_junit::only_call_in_another_file`, whose criteria steps 7 and 8 built. Must not change: the reason texts of
    steps 7 and 8.
17. Config window and fault commit (AC-25, AC-41; tests `check_junit::config_change_after_first_red`,
    `check_junit::fault_commit_is_the_blocking_change`): `junit_block` and the hook and `next` fault commit.
    A coverage commit adds `check_junit::input_window_edges` (AC-39). Must not change: the input window of
    AC-24.
18. Protected files (AC-42; test `check_junit::fixture_beside_a_test_is_protected`): the folder of each
    step test file under junit. A coverage commit adds `check_junit::same_file_helper_is_not_protected`
    (AC-27; the limit holds). Must not change: `Unit::Rest` and cargo units.
19. Lint names (AC-29, AC-31; tests `lint::named_tests_from_test_columns`, `lint::module_path_names`):
    Test columns and Plan test lists only, the file part as the Named test term says, a missing file
    reported. Must not change: the exit codes and order of AC-32.
20. Base merge input (AC-43, AC-24; tests `check_history::merge_of_the_base_branch_blocks_nothing`,
    `check_junit::merge_of_the_base_branch_blocks_nothing`): `build_inputs.rs` for both runners, with the
    `base_branch` of `state.json`. Must not change: `check_junit::merge_brings_a_build_input`, which merges
    a side branch in a ticket without `base_branch` and still blocks.
21. Reader and report file (AC-21, AC-34, AC-38; tests `junit_report::character_reference_causes`,
    `check_junit::report_must_be_a_regular_file`): one reason per reference cause; one read helper for
    the Per-file and suite reports that checks the file kind and size before the read. Must not change:
    the other reader reasons.
22. Protected fixtures and lint headers (AC-42, AC-29; tests `check_junit::fixture_added_or_deleted_is_protected`,
    `lint::rows_without_header_name_no_test`): added and deleted fixtures; the header-less fallback goes.
    The RED gives the old lint fixtures header rows and a header-less row naming a missing test each, so
    each changed old test fails at the RED. Must not change: the Named test term.
23. Coverage, its own commit, no GREEN (AC-7, AC-37, AC-40, AC-41; tests
    `check_history::base_edges`, `ts_tests::describe_each_template_hides_calls`,
    `ts_tests::describe_scope_cases`, `cargo_lock::junit_lock_edges`, `check_junit::config_fault_commit`):
    the behaviour holds. Must not change: any source file.

## Review

- Spec review: two rounds by `stapel-spec-reviewer` (round 1 with one HIGH, round 2 without HIGH).
- Drift: `stapel-drift` after every GREEN step (steps 1 to 23) and after the merge of main.
- Code review: round 1 and round 2, each with `stapel-reviewer` on Sonnet, an external `stapel-reviewer`
  on Fable and a whole-ticket drift review.
- HIGH and MEDIUM findings inside the promise by stage (`findings.jsonl`): spec review 1 HIGH, 13 MEDIUM;
  step drift 3 HIGH (d9-1, d12-1, M-1), 24 MEDIUM; code review round 1 0 HIGH, 2 MEDIUM; code review
  round 2 1 HIGH (CR2-1, the plan text, fixed by the ledger decision of 2026-11-12), 5 MEDIUM.
- Round 2 left to A1c (ledger 2026-11-12): a base merge against `tests-changed` (CR2-4), an absolute
  script path in `command` (CR2-5), reader roots, CDATA outside the root and entities in skipped elements
  (CR2-7, CR2-11), the silent cargo fallback when `stapel.toml` fails to load (CR2-8), the read between
  metadata and content (d21-1, d21-2), and coverage d20-2, d20-3, d22-1.

## Summary

- Built: promise items 1 to 6 by 43 criteria in 23 Plan steps: the `junit` runner with its keys, lexer,
  report reader, Per-file runs, suite and outcomes; the `base..HEAD` walk; the `docs/` staleness rule;
  `stapel lint [KEY]`. The final check passes with 912 tests.
- Improved: the spec named its external states from real jest-junit runs, which corrected two remembered
  facts before the build; each fix step had a RED test that failed first.
- Not improved: the ticket outgrew its 12-step frame (23 steps by the human's leave) and its size; drift
  found three HIGH defects after the spec review; round 2 still found code defects, left to A1c.
- Process problems seen:
  - Builders wrote files with shell scripts seven times despite the rule.
  - One builder ran `pkill -f` on a pattern that could match other sessions' wait loops.
  - One builder ran `cargo fmt` over 29 files (reverted before the commit).
  - `stapel check` refused at once, four times, while another worktree's check held the repository-wide
    check lock, instead of waiting.
  - After the merge of main, `stapel next` blocked the fix of its own block (fix-history pointed at the
    steps the base-merge fix had to repair).
  - The installed binary was older than the branch, so the hook printed stale faults until the close.
  - Two sessions changed the plan of the same item in parallel (CR2-1).
