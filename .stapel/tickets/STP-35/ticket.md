# STP-35 — The check: faster fixtures, levels, points and the step cache

## Description

Source: `docs/PLAN.md`, phase 3b, ticket 3b-1 (items 3.2a and 3.2b); ledger 2026-11-20 (the ten items of
the first cut grouped into four tickets).

### Frame

**Problem.** A local `stapel check` here takes 30 to 80 minutes, and `stapel next` demands it again
after most commits. Most tests of `tests/check.rs` compile a fixture crate with real cargo; every check
runs every step at RED, GREEN and HEAD and the suite, even steps an earlier passing check ran at the same
commits; `check.timeout_secs` bounds each cargo run apart, so a check is not bounded (1800 s did not stop
one in STP-34); a commit of `.stapel/tickets/` makes a check stale.

**Plan item:** 3b-1 (3.2a, 3.2b).

**Risk tags:** `interface` (`[check]` keys `level`, `points`, `suite_exclude`; the flag `--level`;
actions of `stapel next`; the CI workflow), `data` (check record fields that a later check reads).

**Non-goals.** Reading the CI result (waits for `stapel-forge`); a cache across tickets or machines;
parallel steps; a suite cache; other test files; a limit per cargo run; items 3.5 and 3.6.

**Promise (closed list).**

1. Fixtures: the tests of `tests/check.rs` whose subject is the check's logic run with a fake cargo that
   compiles nothing; real cargo stays only in tests of the cargo boundary. No behaviour loses its test.
2. Time limit: `check.timeout_secs` bounds the whole check; at the end the running program and its
   descendants, nested process groups included, are killed, what is left is `unverified`, the record says
   `fail`, exit 1.
3. Step cache: a step whose RED, GREEN, test list, lock hash, toolchain and stapel version match a passed step of a
   passing record of the ticket is not run at RED and GREEN; its line says `cached`.
4. Levels `steps`, `head`, `full` in `[check] level` and `--level` (flag wins, default `head`); see AC-8
   to AC-10.
5. Points: `[check] points` (`step`, `review`, `close`; `close` required) say where `stapel next`
   demands a check; the close needs a current pass of level `head` or `full`.
6. Validity: changes only under this ticket's `.stapel/tickets/<KEY>/`, its Plan unchanged, keep a
   check current.
7. `[check] suite_exclude`: `--skip` filters for the local suite below `full`.
8. CI: the workflow of `stapel init` runs `--level full`.
9. Measures (not tested): `tests/check.rs` and a local check before and after; the cache on a closed
   ticket of the pilot's jest repository; in the Review section.

Not promised: a suite cache under junit; a process that leaves its parent's tree on purpose; a
hand-written record in `runs.jsonl`.

**Prior art.** STP-4 (`runs.jsonl`), STP-6 (build inputs), STP-27 (cargo lock), STP-28 (junit, machine
files), STP-33 (CI workflow).

**Size.** About 450 source lines (`check.rs`, `runner.rs`, `config.rs`, `checkstate.rs`, `next.rs`,
`close.rs`, the workflow asset), about 600 test lines plus the move to the fake cargo. 12 Plan steps, the
limit; split past 600 source lines. Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-20: Decision (split, the human): keep items 3.2a and 3.2b in one ticket of at most 12 Plan
  steps. Rejected: cutting the points, `suite_exclude` and validity out into the next ticket (recommended);
  3.2a only.
- 2026-11-20: Decision (time limit, the human, recommended): `check.timeout_secs` bounds the whole check,
  and the kill reaches nested process groups. Rejected: a limit per cargo run; two keys.
- 2026-11-20: Decision (close gate, the human, recommended): the close needs a current passing local check
  of level `head` or `full`; the CI workflow runs `--level full`; stapel does not read the CI result in
  this ticket. Rejected: a local `full` check at the close.
- 2026-11-20: Decision (jest measure, the human, recommended): measure the cache on the pilot's
  TypeScript repository, on the history of one closed pilot ticket. Rejected: a jest fixture; later on the
  foreign repository.

## Spec

Terms:

- **Check record**: a line of `runs.jsonl` with `"kind": "check"`. A **passing record** has
  `"result": "pass"`.
- **Step key**: of one step, its RED commit, its GREEN commit, the **test hash** (SHA-256 of the sorted
  test ids it runs, one per line, as `<dir>/<file>::<name>`), the **lock hash** (SHA-256 of the lock files
  at HEAD: `Cargo.lock` under the cargo runner; `package-lock.json`, `yarn.lock` and `pnpm-lock.yaml` at
  any depth under the junit runner; the word `none` when there is none), the **toolchain** (the record field
  `toolchain`: the cargo path and version under the cargo runner, `[check] command` under the junit runner) and
  the stapel version.
- **Cached step**: a step whose key equals the key of a step with outcome `pass` or `cached` in a passing
  record of the same ticket.
- **Level**: `steps` < `head` < `full`. A record without `level` was written before this ticket and
  counts as `full`.
- **Point**: a place where `stapel next` demands a current check: `step`, `review` or `close`.
- **Fake cargo**: the test script of `tests/common/` that answers `cargo --version` and `cargo test` from
  markers in the fixture's sources, with libtest's output lines, and compiles nothing.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN one fixture ticket with a passing step, a step with no RED and a step that is not green is checked with the real cargo and with the fake cargo THE two reports are equal except the `cargo:` line. | `check::fake_cargo_matches_real_cargo` |
| AC-2 | WHEN the tests of this repository run THE `stapel check` runs of `tests/check.rs` all go through one helper that takes `Cargo::Fake` or `Cargo::Real`, and the tests that pass `Cargo::Real` are exactly those named in the file's list `REAL_CARGO`, at most 30 (the boundary tests). | `check::real_cargo_only_at_the_boundary` |
| AC-3 | IF `check.timeout_secs` runs out while a program of the check runs THEN the check kills that program and every process it started, nested process groups included and its own group excluded, prints every step not yet judged and not cached and the suite as `unverified: the time limit of <n> s for the check ran out`, writes its record with result `fail` and exits 1 within `<n>` + 10 seconds of its start, the start being after the wait for the machine-wide cargo lock, and no checkout runs after the limit; the range of the key stays 10 to 86400, and the default that `stapel init` writes becomes 3600. | `check::time_limit_bounds_the_whole_check`, `check::time_limit_kills_nested_groups`, `check::time_limit_stops_the_checkouts` |
| AC-4 | WHEN a step is a cached step and the level is not `full` THE check runs none of its tests at RED and GREEN, prints `<label>: cached from <record id>`, and records the outcome `cached` and `from`: the id of the record that ran the step, passed on unchanged when the source step was itself cached. | `check_cache::reuses_a_passing_step` |
| AC-5 | IF one part of the step key differs, the record failed, the step there was neither `pass` nor `cached`, or the record lacks a key field THEN the step is not cached and runs at RED, GREEN and HEAD. | `check_cache::a_changed_key_runs_the_step`, `check_cache::a_changed_lock_file_runs_the_step` |
| AC-6 | WHEN a check writes its record THE record has `level`, `lock`, `toolchain`, `plan_hash` and `suite_exclude` (the configured entries below level `full`, `[]` at `full`), and each step has `tests_hash`. | `check_cache::record_fields` |
| AC-7 | IF `[check] level` is not `steps`, `head` or `full` THEN loading `stapel.toml` fails with `check.level must be steps, head or full: <value>`; IF `--level` is another word THEN `stapel check` exits 2 with `stapel: --level must be steps, head or full: <value>` and runs no cargo; IF `--level` comes with `--list` or `--history` THEN it exits 2 with `stapel: --level does not go with --list or --history`. | `check_levels::refuses_an_unknown_level` |
| AC-8 | WHEN the level is `steps` THE check runs the steps that are not cached at RED, GREEN and HEAD, runs nothing of a cached step, runs no suite, prints `suite: skipped: level steps`, records the suite outcome `skipped`, and its result is `pass` when every step passes or is cached. | `check_levels::steps_level` |
| AC-9 | WHEN the level is `head` THE check does what `steps` does, runs the tests of each cached step at HEAD, and runs the suite; a cached step whose tests fail at HEAD runs whole and is not `cached`. | `check_levels::head_level`, `check_cache::a_cached_step_failing_at_head_runs_whole` |
| AC-10 | WHEN the level is `full` THE check runs every step at RED, GREEN and HEAD and the whole suite, and reads neither the cache nor `suite_exclude`. | `check_levels::full_level` |
| AC-11 | WHEN neither `--level` nor `[check] level` is given THE level is `head`; WHEN both are given THE flag wins; THE report line `level: <level>` follows the `cargo:` line. | `check_levels::flag_wins_over_the_key` |
| AC-12 | WHEN `[check] suite_exclude` lists entries and the level is `steps` or `head` THE suite runs with `--skip <entry>` for each entry and the suite line ends with `(left out: <entries>)`; IF the key holds an entry under the junit runner (an empty list is accepted), or an entry is empty or only whitespace, THEN loading `stapel.toml` fails with `check.suite_exclude needs the cargo runner` or `check.suite_exclude has an empty entry`. | `check_levels::suite_exclude` |
| AC-13 | WHEN `[check] points` is absent THE points are `step`, `review` and `close`; IF it lacks `close` or holds a word other than `step`, `review` and `close` THEN loading `stapel.toml` fails with `check.points must hold close and only step, review, close`. | `check_levels::points_need_close` |
| AC-14 | WHEN `step` is a point, the tree is clean, the newest step commit is a GREEN, a later Plan step is open and the check is not a current pass THE `stapel next` prints `run-check` with `stapel check <KEY> --level steps` in place of `build-step`; WHEN the tree is dirty THE next prints `commit` first; WHEN `step` is no point THE next prints `build-step`. | `next_points::step_point`, `next_points::without_the_step_point` |
| AC-15 | WHEN every Plan step is done and `review` is a point THE `stapel next` prints `run-check` with `stapel check <KEY>` (no flag) before `code-review` unless the current check passes at the level of `[check] level` (default `head`) or higher; a pass below that level, made with a lower `--level`, does not count; WHEN `review` is no point THE next prints `code-review` without a check. | `next_points::review_point` |
| AC-16 | WHEN no current check passes at level `head` or `full` (none, stale, or a `steps` pass) THE `stapel next` before `write-summary` and before `close` prints `run-check` with `stapel check <KEY> --level head`, whatever the other points; a failing check keeps the action `fix-check`; `status` prints `check: pass at <sha> (level steps)` for a `steps` pass and the other lines as today; `stapel close` refuses a `steps` pass with `stapel: the close needs a check of level head or full; run stapel check <KEY> --level head` and exit 1. | `next_points::close_needs_head_or_full` |
| AC-17 | WHEN the only changes since the checked commit are under `.stapel/tickets/<KEY>/` of this ticket and the hash of its Plan section equals the record's `plan_hash` THE check stays current for `status`, `next` and `close`; a changed Plan section, a change of another ticket's folder, of a path that a junit test or input glob matches, or of another path outside `docs/` and the machine files still makes it stale. | `check::tickets_folder_keeps_the_check` |
| AC-18 | WHEN `stapel init` writes the CI workflow THE check line is `stapel check --level full "$BRANCH"`, for the cargo and the junit runner. | `init_workflow::workflow_lines`, `init_workflow::writes_the_workflow` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-1 and AC-2, item 2 by AC-3, item 3 by AC-4 to
  AC-6, item 4 by AC-7 to AC-11, item 5 by AC-13 to AC-16, item 6 by AC-17, item 7 by AC-12, item 8 by
  AC-18; item 9 is measured, not tested. Unchanged: the step outcomes and their reasons; the history
  rules; the report lines other than `level:`, `cached` and the suite line of AC-8 and AC-12; the
  junit runner other than the cache, the levels and the workflow line.
- **Not promised:** the Frame's list; the speed of a test file other than `tests/check.rs`; a cache hit
  after `git gc` removed a commit (the step key no longer resolves and the step runs).

## Test plan

Integration tests in `crates/stapel-cli/tests/`: `check.rs` (fixtures, time limit, validity), the new
files `check_cache.rs`, `check_levels.rs` and `next_points.rs`, and `init_workflow.rs`. The fake cargo
lives in `tests/common/` so that the new files share it; the tests of the time limit use a fake cargo that
sleeps and starts a child in its own process group. Light tests: `stapel cargo test -j 4 -p stapel-cli
--test <file> -- --test-threads=4`; before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Fake cargo | pass, no RED, not green, with the same report as real cargo | `check::fake_cargo_matches_real_cargo` |
| Boundary list | a check test without the fake cargo outside `REAL_CARGO`; the list over 30 | `check::real_cargo_only_at_the_boundary` |
| Time limit | three steps of a fake cargo that sleeps 6 s each, limit 10 s: exit 1 within 20 s, later steps and suite `unverified`; a grandchild in its own group is gone after the exit | `check::time_limit_bounds_the_whole_check`, `check::time_limit_kills_nested_groups`, `check::time_limit_stops_the_checkouts` |
| Cache hit | a second check after a passing one: no cargo run at RED or GREEN of the step (the fake cargo logs its runs), the `cached from` line, the outcome `cached` and `from` in the record; a third check whose source step was cached keeps the first `from` | `check_cache::reuses_a_passing_step` |
| Cache miss | each of: a new GREEN commit, a test added, `Cargo.lock` changed at HEAD, a nested `package-lock.json` changed under junit, another `cargo` text, another `[check] command` under junit, another version in `tool`, a failing record, a step outcome `no-red`, a record without `tests_hash`, a record of another ticket | `check_cache::a_changed_key_runs_the_step`, `check_cache::a_changed_lock_file_runs_the_step` |
| Record | `level`, `lock`, `toolchain`, `plan_hash`, `suite_exclude`, `tests_hash`; the `level` is `head` with no key and no flag | `check_cache::record_fields` |
| Level values | `fast`, empty, upper case `HEAD` in the key (load error) and the flag (exit 2); `--level` with `--list` and with `--history` | `check_levels::refuses_an_unknown_level` |
| Levels | `steps`, `head`, `full` against the fake cargo's run log; a cached step whose test fails at HEAD | `check_levels::steps_level`, `check_levels::head_level`, `check_levels::full_level`, `check_cache::a_cached_step_failing_at_head_runs_whole` |
| Precedence | no key and no flag; key `steps` and flag `head` | `check_levels::flag_wins_over_the_key` |
| Suite exclude | two entries at `head`; the same at `full`; an entry under junit; `[]` under junit; an entry `""` and `" "` | `check_levels::suite_exclude` |
| Points | default; key `full` with a `head` pass at the review point; `["close"]`; `["step", "close"]`; `[]`; `["review"]`; `["close", "deploy"]`; an untracked file after a GREEN with the step point | `check_levels::points_need_close`, `next_points::step_point`, `next_points::without_the_step_point`, `next_points::review_point` |
| Close gate | no check, a fail, a stale check, a `steps` pass, a `head` pass, an old record without `level`, each with points `["close"]`; the `status` line of a `steps` pass | `next_points::close_needs_head_or_full` |
| Validity | a commit of this ticket's `ticket.md` outside the Plan only; a new Plan step; of its `runs.jsonl` only; of another ticket's `ticket.md`; of `src/lib.rs`; under junit, a path of this ticket's folder that a test glob matches | `check::tickets_folder_keeps_the_check` |
| Workflow | the cargo and the junit golden files | `init_workflow::workflow_lines`, `init_workflow::writes_the_workflow` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `--level` | `steps`, `head`, `full` | — | absent: the key | another word, upper case, with `--list` or `--history`: exit 2 | exact | `check_levels::refuses_an_unknown_level`, `check_levels::flag_wins_over_the_key` |
| `[check] level` | string | — | absent: `head` | another word: load error | exact | `check_levels::refuses_an_unknown_level` |
| `[check] points` | list of strings | `["close"]` / three words | absent: all three; `[]`: refused | without `close`, another word: load error | exact, repeats allowed | `check_levels::points_need_close` |
| `[check] suite_exclude` | list of strings | 0 / any | absent or `[]`: nothing left out | an entry under junit, an empty or whitespace entry: load error | passed to `--skip` as given | `check_levels::suite_exclude` |
| `check.timeout_secs` | integer seconds | 10 / 86400 | absent: the default | outside the range: load error as today | whole seconds, for the whole check | `check::time_limit_bounds_the_whole_check` |
| Check records of `runs.jsonl` | JSON lines | none / the journal limit | no record: nothing cached | a record without key fields, a failing record, a malformed line: not a cache source | exact match of every key field | `check_cache::a_changed_key_runs_the_step` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| cargo | output format of libtest | the fake cargo prints the same lines as libtest; AC-1 compares | `check::fake_cargo_matches_real_cargo` |
| cargo | `Cargo.lock` changed at HEAD | part of the step key | `check_cache::a_changed_lock_file_runs_the_step` |
| cargo | `--skip` filters of libtest | given as entries; matching is libtest's | `check_levels::suite_exclude` |
| cargo | workspaces, custom targets, config files, environment, offline | unchanged; covered by the boundary tests kept on real cargo | — |
| git | a commit of the step key removed by `git gc` or a rewritten history | the key does not match; the step runs | `check_cache::a_changed_key_runs_the_step` |
| git | paths under `.stapel/tickets/<KEY>/` and other tickets' folders | AC-17 | `check::tickets_folder_keeps_the_check` |
| git | object format, worktrees, submodules, other dimensions | unchanged by this ticket | — |
| os | processes: process groups, children that leave the group | the kill walks the children of every process the check started; a process that calls `setsid` and then forks away from its parent's tree is not promised | `check::time_limit_kills_nested_groups` |
| os | pid reuse between the walk and the kill | not promised | — |
| os | time: clock skew | the limit uses a monotonic clock | — |
| claude-code | every dimension | not read by this ticket | — |

**Review Focus:** two checks of the same ticket in two worktrees writing `runs.jsonl` at once (the
journal's append is one line; not tested); a cache record whose commit is no longer an ancestor of HEAD
after a rebase (a key of commits that exist still matches; the review judges it); `suite_exclude` entries
that skip every test (the suite passes with 0 tests; not refused); the step point after a `fix` commit
that is not a GREEN (no check demanded; not tested).

## Design

### Facts

- F-1: `check.rs::run` sets `timeout = check.timeout_secs` and passes it to every `run_cargo` and
  `run_program` call; each call gets the whole limit (`check.rs:390`, `runner.rs:358`).
- F-2: `runner.rs::run_child` puts the child in its own process group and kills that group; a test
  binary that runs `stapel check` puts its own cargo in a new group, which the kill does not reach.
- F-3: `check.rs::run` writes one check record with `head`, `tool`, `result`, `steps` (label, red,
  green, outcome, reason, tests count, names) and `suite`; nothing reads the step entries back.
- F-4: `checkstate::current` drops machine files and `docs/` from the changed paths;
  `.stapel/tickets/<KEY>/ticket.md` is not a machine file.
- F-5: `next.rs::after_the_steps` demands `run-check` once every Plan step is done and the check is not a
  current pass; nothing demands a check between steps.
- F-6: the check copy builds into `<check dir>/check-target`, a new folder per check, and each fixture of
  `tests/check.rs` lies in its own temporary folder; cargo's artifacts of a path package carry its path
  in their metadata, so a target folder shared between fixtures reuses no fixture artifact.

### Decisions (before the build)

- 2026-11-20, agent: the shared `CARGO_TARGET_DIR` of plan item 3.2a is not built (F-6): it saves no
  fixture build; the gain comes from the fake cargo. Step 1 measures `tests/check.rs` before any change,
  so the Review shows the gain.
- The deadline is one `Instant` made at the start of `run`; each run gets the time left. The kill walks
  the process tree from the child (`ps -A -o pid=,ppid=`) and kills each process group it finds, then
  the child's own group.
- The cache lookup reads `runs.jsonl` once, keeps the passing records of the ticket, and matches a step
  by its six key fields; the newest match wins and its id goes into `from`.
- `CheckLine::Pass` carries the level; `close.rs` and `next.rs` compare it.
- 2026-11-20, agent (SR1-4): the default of `[check] points` is all three, as plan item 3.2b says; the
  step point is cheap because the cache runs only the new step. Rejected: `["review", "close"]`.
- 2026-11-20, agent (SR1-5): only this ticket's folder keeps a check current; another ticket's folder
  and a junit glob match still make it stale.
- 2026-11-20, the human (SR2-1, SR2-3): the range 10 to 86400 of `timeout_secs` stays; the default
  `stapel init` writes becomes 3600; this repository's key goes from 1800 to 5400 by a command the
  person runs.
- 2026-11-20, the human (SR2-2, ledger): steps 1 and 2 are test-refactor steps with no RED and no GREEN.
- 2026-11-20, agent (SR1-10): the kill reads `ps -A -o pid=,ppid=,pgid=` and never signals the group of
  stapel itself.
- The step point is judged in `next.rs::history_action` before `build-step` of a step that is not the
  first.

### Decisions (as built)

- Steps 1 and 2 (the fake cargo and the move) are on main as `086bcaf` and `2f94673`; the ticket branch
  merges main (`5bab3a1`), and the word commits `0bc96a8` and `ebb9b23` close the two steps (ledger
  2026-11-20, the human: a test helper changed by a non-RED commit of the range blocks every later step).
- The step 3 RED first changed the existing test `closed_ticket_build_inputs_are_read_at_its_end`, which
  passed before the code, so the check gave step 3 `no-red`. That change is on main as `4c61839`, and the
  branch was rebuilt on it (ledger 2026-11-21, the human).
- 2026-11-21, agent: `time_limit_kills_nested_groups` failed at its GREEN in the check, because the RED
  run panicked before its `pkill` and left `sleep 303` alive, and `pgrep` searched the whole machine. The
  step 3 RED was amended: the sleeps run under a path of the test only, and a guard kills them on every
  exit; the branch was rebuilt again.
- The fake cargo is `tests/common/fake_cargo.sh` (sh and awk). A test body marker `// fake: needs <text>`
  makes the test pass only when a file under `crates/*/src` holds the text; `// fake: item <text>` makes
  the test target compile only when the text is there; `this does not compile` in a source breaks the
  library. `tests/check.rs` runs every check through `check_command(dir, Cargo)`; `add_test` adds a marker
  from the assertion. `REAL_CARGO` holds 14 tests: ignored tests, compile errors, child output, time
  limits, planted configuration, `include_str!`, the target folder and the environment.
- 2026-11-20, agent: the default of an absent `timeout_secs` is 3600 too, as the template; the check.rs
  helpers rewrite any `timeout_secs = <n>` line; `outcome_unverified_on_timeout_and_stale_lock` asserts
  the time-limit text for step 2 and keeps the stale-lock case in a second check without the hanging
  step.
- `runner.rs::kill_tree` reads `ps -A -o pid=,ppid=,pgid=`, kills the descendants and their groups except
  stapel's own, in up to 5 passes; `Runner` keeps the deadline and gives each run the time left, at the
  four run sites (cargo step, cargo suite, junit file, junit suite).
- `check.rs::cached_from` reads `runs.jsonl` and takes the newest passing record of the ticket with the
  same `lock`, `toolchain` and `tool` whose step is `pass` or `cached` with the same `red`, `green` and
  `tests_hash`; malformed and keyless records are skipped. At `head` a cached step still runs at HEAD and
  runs whole when it fails there; at `steps` it runs nothing. `outcomes::passing` accepts `cached`.
- `lock` under cargo is the SHA-256 of `Cargo.lock` at the checked commit; `plan_hash` is the
  `section_hash` of the Plan (`sha256:` prefix) or `none`. At `steps` the record's `suite_exclude` holds
  the configured entries although no suite ran.
- `level:` is printed under junit too. The golden reports `check_report.txt` and
  `check_report_build_inputs.txt` carry the line (step 7's RED was rebuilt to hold them).
- `checkstate::current` drops paths under `.stapel/tickets/<KEY>/` when the Plan hash equals the record's
  `plan_hash`; a record without it drops nothing. 2026-11-21, agent: the existing test
  `machine_files_keep_a_check_current` now expects `pass` for a file of the ticket's own folder and
  `stale` for another ticket's folder.
- `CheckLine::Pass` carries the level. 2026-11-21, agent: `stapel next` demands the `head` check before
  the final `close` action too, so it never prints `close` while the gate refuses. 2026-11-21, agent
  (spec change of AC-16): a failing check keeps the earlier `fix-check` action, which names
  `runs.jsonl`, instead of `run-check --level head`.
- DR-1: the CI workflow of this repository installs stapel at `6b59ae1` (the step 12 GREEN), which has
  `--level`; the orchestrator built that commit in a clean copy with `STAPEL_BUILD_COMMIT` and ran its
  `init` (`438a2cc`). CI can install it once the branch is pushed.
- Pre-existing failures on this Mac with the default TMPDIR, at the base `e0da738` too: five
  `check_junit::absolute_script_*`, `status::refuses_ambiguous_case_variant_keys` and
  `next::refuses_a_wrong_call`.
- This repository's `timeout_secs` is 5400 (`fcebc9d`).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check::fake_cargo_matches_real_cargo` |
| AC-2 | `check::real_cargo_only_at_the_boundary` |
| AC-3 | `check::time_limit_bounds_the_whole_check`, `check::time_limit_kills_nested_groups`, `check::time_limit_stops_the_checkouts` |
| AC-4 | `check_cache::reuses_a_passing_step` |
| AC-5 | `check_cache::a_changed_key_runs_the_step`, `check_cache::a_changed_lock_file_runs_the_step` |
| AC-6 | `check_cache::record_fields` |
| AC-7 | `check_levels::refuses_an_unknown_level` |
| AC-8 | `check_levels::steps_level` |
| AC-9 | `check_levels::head_level`, `check_cache::a_cached_step_failing_at_head_runs_whole` |
| AC-10 | `check_levels::full_level` |
| AC-11 | `check_levels::flag_wins_over_the_key` |
| AC-12 | `check_levels::suite_exclude` |
| AC-13 | `check_levels::points_need_close` |
| AC-14 | `next_points::step_point`, `next_points::without_the_step_point` |
| AC-15 | `next_points::review_point` |
| AC-16 | `next_points::close_needs_head_or_full` |
| AC-17 | `check::tickets_folder_keeps_the_check` |
| AC-18 | `init_workflow::workflow_lines`, `init_workflow::writes_the_workflow` |

## Plan

Route: full (tags `interface`, `data`), auto.

1. Fake cargo (AC-1; test `check::fake_cargo_matches_real_cargo`): a test-refactor step (ledger 2026-11-20): first measure the time of the light test run
   of `--test check` and note it; then add the fake cargo to `tests/common/` and the test in one commit
   `STP-35 step 1 tests: ...`. Must not change: any source file.
2. Logic tests on the fake cargo (AC-2; test `check::real_cargo_only_at_the_boundary`): a test-refactor step:
   one helper for every check run, `REAL_CARGO`, the test, and the move of every test whose subject is
   not the cargo boundary, in one commit `STP-35 step 2 tests: ...`; measure the file's time again.
   Must not change: what each moved test asserts (the code review's deletion check reads the move).
3. Time limit (AC-3; tests `check::time_limit_bounds_the_whole_check`,
   `check::time_limit_kills_nested_groups`, which fail at the RED because each run gets the whole limit
   and the kill stops at the child's group): the GREEN adds the deadline, the tree kill that spares
   stapel's own group, and the template default 3600. Must not change: the outcome of a run that ends in time.
4. Config keys (AC-13, the load errors of AC-7; tests `check_levels::points_need_close`,
   `check_levels::refuses_an_unknown_level`, which fail at the RED because the keys are unknown): the
   GREEN adds `level` and `points` to `config.rs::Check` and the flag `--level`. Must not
   change: the other keys.
5. Record fields (AC-6; test `check_cache::record_fields`, which fails at the RED because the record has
   no `level`, `lock`, `toolchain`, `suite_exclude` or `tests_hash`): the GREEN writes them, the level
   from step 4's key and flag. Must not change: the other record fields.
6. The cache (AC-4, AC-5; tests `check_cache::reuses_a_passing_step`,
   `check_cache::a_changed_key_runs_the_step`): the GREEN reads the passing records, skips a cached
   step at RED and GREEN, and writes `cached` and `from`. Must not change: a step that is not cached.
7. Levels (AC-8 to AC-11; tests `check_levels::steps_level`, `check_levels::head_level`,
   `check_levels::full_level`, `check_levels::flag_wins_over_the_key`): the GREEN applies the level to
   the steps and the suite and prints the `level:` line. Must not change: the step outcomes.
8. Suite exclude (AC-12; test `check_levels::suite_exclude`): the GREEN adds the key with its load
   errors, the `--skip` arguments and the suite line. Must not change: the suite at level `full`.
9. Validity (AC-17; test `check::tickets_folder_keeps_the_check`): the GREEN drops the paths of
   this ticket's folder in `checkstate::current` unless a junit glob matches or the Plan hash differs. Must not change: the staleness of any other path.
10. Step point (AC-14; tests `next_points::step_point`, `next_points::without_the_step_point`): the GREEN
    demands `commit` on a dirty tree, then `run-check --level steps`, before the next `build-step`. Must
    not change: the first step.
11. Review and close points (AC-15, AC-16; tests `next_points::review_point`,
    `next_points::close_needs_head_or_full`): the GREEN reads `points` in `after_the_steps`, demands a
    `head` check before `write-summary`, carries the level in `CheckLine::Pass`, prints the level of a `steps` pass in `status`, and refuses it in
    `close.rs`. Must not change: a project without `[check]`.
12. CI workflow (AC-18; tests `init_workflow::workflow_lines`, `init_workflow::writes_the_workflow`,
    which fail at the RED with the new golden files): the GREEN changes the workflow assets and runs
    `stapel init` in the worktree. Must not change: the other workflow lines.
13. Real key changes (XR2-1, AC-5; test `check_cache::a_changed_lock_file_runs_the_step`): a coverage
    commit `STP-35 step 13 tests: ...`: after a passing check, a later RED that changes `Cargo.lock`
    makes the earlier steps run again; under junit a changed nested `package-lock.json` does the same.
    If the test fails, it becomes a RED and the GREEN fixes `lock_hash`. Must not change: any source file.
14. Cached step failing at HEAD (XR2-2, AC-9; test `check_cache::a_cached_step_failing_at_head_runs_whole`):
    a coverage commit `STP-35 step 14 tests: ...`: a step cached by an earlier check whose test fails at
    HEAD is not reported `cached` and the result is `fail`. If the test fails, it becomes a RED and the
    GREEN fixes `Runner::step`. Must not change: any source file.
15. No checkout after the limit (XR2-4, AC-3; test `check::time_limit_stops_the_checkouts`, which fails at
    the RED because a post-checkout hook that sleeps runs for every unjudged step after the limit): the
    GREEN reads the time left in `Runner::at` before the checkout. Must not change: a run that ends in
    time.

## Review

TODO

## Summary

TODO
