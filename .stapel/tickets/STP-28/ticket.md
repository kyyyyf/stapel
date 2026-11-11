# STP-28 — junit runner and base..HEAD walk

## Description

Source: `docs/PHASES.md`, stage A item A1 (plan items 1.11 and 1.12); ledger decision 2026-11-10; the
investigation in the chat of 2026-11-11.

### Frame

**Problem.** `stapel check` runs only cargo tests, so the TypeScript pilot repository cannot be checked;
every check and commit hook walks the whole first-parent history (1035 commits here, more in the pilot).

**Plan item:** A1 (1.11 and 1.12).

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
- **Per-file run**: `command` with `{file}` replaced by one test file's path, run in the check copy
  after its old `report` file is removed.
- **Named test**: backtick text holding `::` in a table cell or a Plan step of a ticket. A **Checked
  test**: for `lint`, each Named test of a closed ticket, or of an open ticket's Plan step with a GREEN.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the ticket has a `base` THE `stapel check` and `stapel check --history` read step commits, their order and the build-input range only from the Walk, so a commit before `base` whose subject names a step of the ticket counts for nothing; without `base` it still counts. | `check_history::walk_starts_at_the_base` |
| AC-2 | WHEN the commit hook or `stapel next` reads the steps of a ticket with a `base` THE same Walk applies: a step commit before `base` gives no hook fault and no `next` fact or action. | `hook_post::hook_walks_from_the_base`, `next::next_walks_from_the_base` |
| AC-3 | WHEN the commits and changes after a check's HEAD touch only paths under `docs/`, the process ledger or a ticket's machine files (`tokens.jsonl` among them) THE check stays current in `status`, `next` and `close`; any other path, and under junit a `docs/` path that matches a test or input glob, makes it stale as before. | `check::docs_commits_keep_a_check_current` |
| AC-4 | WHEN `[check]` has `runner = "junit"` THE config reads `command` and `report` (both needed), `timeout_secs` as for cargo, `tests` (default `**/*.test.*`, `**/*.spec.*`, `**/__tests__/**`) and `inputs` (default `**/package.json`, the three lock files, `**/jest.config.*`, `**/tsconfig*.json`, `**/babel.config.*`, `**/.babelrc`, `**/.swcrc`, `**/jest.setup.*`); a cargo `[check]` reads as before. | `core::config::junit_check_keys` |
| AC-5 | IF `command` lacks the word `{file}`, holds it twice, holds it inside a longer word or right after a word that starts with `-`, `report` or a glob is empty, absolute or holds `..`, a glob holds `{`, `}`, `[`, `]`, `!` or a backslash, `tests` is empty, a junit key stands with cargo, `cargo` stands with junit, or the runner is another word THEN loading `stapel.toml` fails with a message that names the key; after a `-` word the message says to put `{file}` right after the program. | `core::config::rejects_bad_junit_keys` |
| AC-6 | WHEN a glob is matched THE leading `**/` also matches no folder (`**/package.json` matches `package.json`), `*` and `?` never match `/`, a trailing `/**` matches every path below at any depth but not the folder itself, and a middle `/**/` also matches no folder. | `core::config::globs_match_paths` |
| AC-7 | WHEN a test file is read THE test calls and their names come from a lexer that skips comments, string, template and regular-expression literals; `re.test(s)`, `test(` inside a string, comment or regular expression, a call whose first argument is not one literal (`test('a ' + x, …)`, `${…}`, a variable), `test.todo` and a call under a `describe` without a literal title are no test calls. | `ts_tests::finds_test_calls_with_describe_titles`, `ts_tests::skips_comments_strings_and_regexes`, `ts_tests::titles_not_promised_are_skipped` |
| AC-8 | WHEN a report is read THE reader takes the elements `testsuites`, `testsuite`, `testcase`, `failure`, `error` and `skipped`, skips other elements with their content, decodes the five XML entities, character references and CDATA, and refuses a DOCTYPE, an unknown entity and unbalanced tags. | `junit_report::reads_test_cases`, `junit_report::refuses_malformed_reports` |
| AC-9 | WHEN the runner is junit THE step tests of a RED commit are its added or changed test calls of test files; a test call whose text is unchanged is none, also when other lines of its file change. | `check_junit::step_tests_from_the_red_commit` |
| AC-10 | WHEN the runner is junit THE check makes one Per-file run per test file of the step at RED, GREEN and HEAD, with the `[check]` of the working tree's `stapel.toml`, its `node_modules` linked, and the cargo run's environment plus `JEST_JUNIT_REPORT_TEST_SUITE_ERRORS=true`. | `check_junit::red_to_green_passes` |
| AC-11 | WHEN a Per-file run's report is read THE step test takes the state of the one test case whose trimmed `name` is its name: failed with `failure` or `error`, ignored with `skipped`, passed otherwise. | `check_junit::red_to_green_passes`, `check_junit::not_green_and_removed` |
| AC-12 | WHEN the states of the step tests at RED, GREEN and HEAD are read THE outcomes `no-red`, `not-green`, `removed` and `pass` keep their STP-4 meaning under junit. | `check_junit::not_green_and_removed` |
| AC-13 | WHEN the runner is junit THE code of `red-changes-code` is any path that matches no test glob and is not under `docs/` or `.stapel/` and is not a `.md` file. | `check_junit::history_outcomes_under_junit` |
| AC-14 | WHEN the runner is junit THE outcomes `unpaired`, `duplicate`, `no-tests`, `retired`, `interleaved` and the note `superseded by` keep their STP-4 and STP-14 meaning in `check` and `check --history`, and `tests-changed` protects each step test's text. | `check_junit::history_outcomes_under_junit` |
| AC-15 | WHEN every test case of a Per-file run's report has the classname `Test suite failed to run` THE file did not load, and its step tests fail there as a compile error of their cargo target does. | `check_junit::file_that_does_not_load_fails` |
| AC-16 | IF a Per-file run's report holds no test suite THEN the step tests of that file are `unverified` with the reason that the report of the file holds no test suite, whatever the exit code. | `check_junit::report_shape_is_unverified` |
| AC-17 | IF a Per-file run's report holds more than one test suite THEN the step tests of that file are `unverified` with the count of suites in the reason. | `check_junit::report_shape_is_unverified` |
| AC-18 | IF a step test has no test case or two in its Per-file run's report THEN the step is `unverified` with the name and the count in the reason. | `check_junit::report_shape_is_unverified` |
| AC-19 | IF a Per-file run exits 0 with a failed test case, or exits other than 0 with test cases none of which failed THEN the step is `unverified` with the exit code and the count of failures in the reason. | `check_junit::exit_code_must_agree` |
| AC-20 | IF the command of a Per-file run cannot start, runs out of time or dies by a signal THEN the step is `unverified` with the reason. | `check_junit::run_faults_are_unverified` |
| AC-21 | IF a Per-file run leaves no report, a report over 16 MiB or one the reader refuses THEN the step is `unverified` with the reason. | `check_junit::run_faults_are_unverified` |
| AC-22 | IF a test file of the step holds a test call or `describe` with `.only` at the RED or the GREEN commit THEN the step tests of that file are `unverified` with a reason that names the file, `.only` and the commit, and no command runs for it. | `check_junit::only_call_is_unverified` |
| AC-23 | IF the working tree has no `node_modules` folder THEN under junit the steps that run and the suite are `unverified` with the reason `node_modules is missing in the working tree`. | `check_junit::node_modules_missing_is_unverified` |
| AC-24 | WHEN a commit of the Walk, a merge commit by its first-parent diff, changes a file that matches an input glob THE later steps read `unverified` with the reason `build-input-changed: <path> at <sha>` as under cargo, and no command runs for them. | `check_junit::build_input_change_is_unverified`, `check_junit::merge_brings_a_build_input` |
| AC-25 | WHEN the runner is junit and a commit of the Walk changes `.stapel/stapel.toml`, or the first word of `command` when it is a repository path, THE later steps read `build-input-changed` for that path. | `check_junit::config_change_is_unverified` |
| AC-26 | WHEN the runner is junit THE suite at HEAD removes the old report, runs `command` once with only the word `{file}` left out, reads every test suite of the report and sums them; it passes when the exit code is 0 and the sums hold a test case and no failure or error, and its detail counts passed, failed and ignored. | `check_junit::suite_runs_the_whole_command`, `check_junit::suite_sums_every_suite` |
| AC-27 | WHEN the runner is junit THE outcome `tests-changed` also protects the rest of each test file the RED changed and the other files the RED changed that match a test glob, in `check`, `check --history` and the commit hook. | `check_junit::tests_changed_under_junit`, `check_junit::hook_faults_under_junit` |
| AC-28 | WHEN `stapel lint [KEY]` runs THE ticket is KEY; without KEY it is the branch ticket, else the one open ticket. | `lint::lint_exit_codes_and_order` |
| AC-29 | IF a Checked test does not exist at HEAD THEN lint prints `<KEY> names <test>, which does not exist`. | `lint::names_a_missing_test` |
| AC-30 | IF a step test of the ticket's RED commits on the Walk exists at HEAD and no ticket of the checkout names it in a table row THEN lint prints `<test> is a step test of <KEY> and no ticket names it`. | `lint::step_test_named_by_no_ticket` |
| AC-31 | WHEN lint compares names THE cargo forms are `<file stem>::<name>`, `<crate suffix>::<file stem>::<name>` and `<dir>/<file stem>::<name>`, and the junit form is `<test file>::<name>`. | `lint::step_test_named_by_no_ticket`, `lint::lint_under_junit` |
| AC-32 | WHEN lint ends THE missing tests come first, then the unnamed step tests, each group sorted by name, and the exit code is 1; with none it prints `lint: <KEY> clean` and exits 0; IF KEY is unknown, or without KEY there is no branch ticket and not exactly one open ticket THEN it refuses with a message and exit 2. | `lint::lint_exit_codes_and_order` |
| AC-33 | WHEN the runner is junit THE step test is `removed` when its file is missing at HEAD or the lexer finds no test call of its name there, before any HEAD run. | `check_junit::not_green_and_removed` |
| AC-34 | IF the suite run's command cannot start, runs out of time or dies by a signal, or leaves no report, one over 16 MiB or one the reader refuses THEN the suite is `unverified` with the reason. | `check_junit::suite_runs_the_whole_command` |
| AC-35 | WHEN a junit check ends THE record in `runs.jsonl` has `"runner": "junit"` and no cargo version, and a passing record is current in `status` and `next` as a cargo one is. | `check_junit::junit_record_is_current` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-35; the cargo runner, its outcomes and its record are
  unchanged apart from the Walk; tickets STP-1 to STP-12 without `base` are checked as before.
- **Not promised:** the Frame's list; test calls under `test.each`, `describe.each`, `xit`, `fit` and
  aliases; JSX text that holds a quote or a backtick; a `node_modules` other than the root one (workspaces);
  a reporter whose test case `name` is not `<describe titles> <title>`; a `node_modules` tracked by git;
  a command that needs a shell (pipes, `&&`, quoting, variables); a report written after the command exits;
  a `base` that a rebase took off the first-parent line (the check refuses as before); a bound on the total
  check time (three runs per test file and step, under the machine-wide lock); `lint` for test names that
  hold `|` or a backtick; a run script that is not the first word of `command` as a build input; under
  cargo, a `docs/` file that a test reads (here only this repository's lint reads `docs/`).

## Test plan

CLI tests in the new files `crates/stapel-cli/tests/check_junit.rs` and `lint.rs`; core tests in the new
files `crates/stapel-core/tests/ts_tests.rs` and `junit_report.rs`. A new helper `common::junit_repo`
(added in the RED of step 6) makes a repository with `[check] runner = "junit"`, `command = "sh run.sh
{file}"`, `report = "junit.xml"`, a `node_modules` folder and a tracked stand-in `run.sh` (sh and awk):
it writes one test case per `test('…'` line of the file, passed when `src/lib.ts` holds `ok <title>`;
fault cases write their report by hand. No node or jest runs. Light tests: `stapel cargo test -j 4 -p
stapel-cli --test <file> -- --test-threads=3`, `-p stapel-core --test <file>`; before each GREEN also
`--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Walk in check | a commit `ABC-1 1 RED: …` before `base` and a real step 1 after it: `check` and `--history` show one pass, no `duplicate`; the same history without `base`: `duplicate`; a merge of main into the branch after `base` | `check_history::walk_starts_at_the_base` |
| Walk in hook, next | the same old commit: the hook after the next commit prints no fault for it; `next` names no fault and no action from it | `hook_post::hook_walks_from_the_base`, `next::next_walks_from_the_base` |
| Staleness | a passing check, then commits of `docs/a.md`, the ledger and `tokens.jsonl`: `check: pass`; a commit of `src/x.rs`: stale; `docs/` plus `src/`: stale; under junit `docs/a.test.ts`: stale | `check::docs_commits_keep_a_check_current` |
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
| GREEN edits a `describe` title, a helper or a fixture beside the test | `tests-changed` | `check_junit::tests_changed_under_junit` |
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

**Author self-check:** 2026-11-11, before spec review, again after rounds 1 and 2: promise item 1 maps to
AC-4 to AC-6, AC-10, AC-23 to AC-26, AC-34, AC-35; item 2 to AC-7 and AC-9; item 3 to AC-8, AC-11 to
AC-22, AC-27 and AC-33; item 4 to
AC-1 and AC-2; item 5 to AC-3; item 6 to AC-28 to AC-32; each Inputs and External states row has a test or
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
- `TestId` gets a `path` and keeps `dir`, `file` for cargo; cargo prints `dir/file::name` as before,
  junit `path::name`.
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
  heavy; the check record has `"runner": "junit"` and no cargo version.
- Until step 10 the junit suite reads `unverified` with the reason that the suite is not built; step 10
  replaces it.
- `step_commits`, `analyse`, `build_inputs::range` and `hook_faults` take the base and walk `base..HEAD`;
  `check_base` checks that the oldest commit of the Walk has `base` as first parent, without a full walk.
- `checkstate::current` drops `docs/` paths besides machine files; `is_machine_file` stays, so `stapel new`
  and the guard keep their rules.
- `stapel lint` is `crates/stapel-cli/src/lint.rs`; `ticket_drift.rs` stays this repository's full lint.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check_history::walk_starts_at_the_base` |
| AC-2 | `hook_post::hook_walks_from_the_base`, `next::next_walks_from_the_base` |
| AC-3 | `check::docs_commits_keep_a_check_current` |
| AC-4 | `core::config::junit_check_keys` |
| AC-5 | `core::config::rejects_bad_junit_keys` |
| AC-6 | `core::config::globs_match_paths` |
| AC-7 | `ts_tests::finds_test_calls_with_describe_titles`, `ts_tests::skips_comments_strings_and_regexes`, `ts_tests::titles_not_promised_are_skipped` |
| AC-8 | `junit_report::reads_test_cases`, `junit_report::refuses_malformed_reports` |
| AC-9 | `check_junit::step_tests_from_the_red_commit` |
| AC-10 | `check_junit::red_to_green_passes` |
| AC-11 | `check_junit::red_to_green_passes`, `check_junit::not_green_and_removed` |
| AC-12 | `check_junit::not_green_and_removed` |
| AC-13 | `check_junit::history_outcomes_under_junit` |
| AC-14 | `check_junit::history_outcomes_under_junit` |
| AC-15 | `check_junit::file_that_does_not_load_fails` |
| AC-16 | `check_junit::report_shape_is_unverified` |
| AC-17 | `check_junit::report_shape_is_unverified` |
| AC-18 | `check_junit::report_shape_is_unverified` |
| AC-19 | `check_junit::exit_code_must_agree` |
| AC-20 | `check_junit::run_faults_are_unverified` |
| AC-21 | `check_junit::run_faults_are_unverified` |
| AC-22 | `check_junit::only_call_is_unverified` |
| AC-23 | `check_junit::node_modules_missing_is_unverified` |
| AC-24 | `check_junit::build_input_change_is_unverified`, `check_junit::merge_brings_a_build_input` |
| AC-25 | `check_junit::config_change_is_unverified` |
| AC-26 | `check_junit::suite_runs_the_whole_command`, `check_junit::suite_sums_every_suite` |
| AC-27 | `check_junit::tests_changed_under_junit`, `check_junit::hook_faults_under_junit` |
| AC-28 | `lint::lint_exit_codes_and_order` |
| AC-29 | `lint::names_a_missing_test` |
| AC-30 | `lint::step_test_named_by_no_ticket` |
| AC-31 | `lint::step_test_named_by_no_ticket`, `lint::lint_under_junit` |
| AC-32 | `lint::lint_exit_codes_and_order` |
| AC-33 | `check_junit::not_green_and_removed` |
| AC-34 | `check_junit::suite_runs_the_whole_command` |
| AC-35 | `check_junit::junit_record_is_current` |

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
9. Dependencies and build inputs (AC-23, AC-24, AC-25; tests
   `check_junit::node_modules_missing_is_unverified`, `check_junit::build_input_change_is_unverified`,
   `check_junit::merge_brings_a_build_input`, `check_junit::config_change_is_unverified`):
   `build_inputs.rs`. Must not change: cargo build inputs.
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

## Review

TODO

## Summary

TODO
