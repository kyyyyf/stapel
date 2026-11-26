# STP-45 — The observability of stapel check

## Description

Source: `docs/PLAN.md`, phase 5, item 5.2c, widened by the ledger decision of 2026-11-26.

### Frame

**Problem.** Checks 3 and 4 of STP-44 ran out of the 5400 s limit, and the output did not say where the
time went: `stapel check` prints a line only after each step and after the suite, keeps no cargo output,
and on a time-out says only that the limit ran out. Its suite runs `cargo test --workspace` with no `-j`
and no `--test-threads`: about 60 min, against about 11 min with `-j 4 --test-threads=4` by hand.

**Plan item:** 5.2c (the whole item, its later part and its widening included).

**Risk tags:** `interface` (new output lines, new `[check]` keys, new fields of `runs.jsonl`), `data`
(the log files and their removal).

**Non-goals.** The junit runner (it keeps its output and records as now); times of the project commands
and the guards; a change of the time limit or of the cargo slots; a live copy of cargo output on the
screen; a log of a check that `stapel` itself did not end (killed, crashed); the speed of the suite
beyond the two run settings.

**Promise (closed list, cargo runner).**

1. Stage times: the output and the record give the seconds of the copy, of the RED and the GREEN runs of
   each step, of the suite build and of the suite run; a stage that did not run has no time.
2. Slot wait: the seconds spent waiting for a cargo slot are printed on a line of their own and recorded
   apart from the stage times, also when the wait is 0.
3. Progress: while a check runs, a line every `[check] progress_secs` seconds (300 by default) names the
   stage, the test file that runs (the step's file, or the file of the last `Running` line of the suite),
   and the seconds since the check started.
4. Time-out: when the limit runs out, the check names the stage, the running test file and the last 20
   lines of each output stream of that cargo run, then ends as now (`unverified`, exit 1).
5. Log: the full output of every cargo run of the check goes to one file
   `<common folder>/stapel/check-logs/<KEY>-<check id>.log`, made at the first cargo run, whose path
   the check prints then; a check
   keeps the newest 10 logs of its ticket and removes older ones of that ticket only.
6. Slowest files: after a suite run that ran tests, the output and the record name the five test files
   with the longest run time (from libtest's `finished in` part), slowest first.
7. Last pass: the suite line gives the suite time and the suite time of the newest passing record of the
   ticket that has one.
8. Run settings: `[check] jobs` and `[check] test_threads` (whole numbers from 1) give `-j <n>` and
   `--test-threads=<n>` to every cargo test run of the check; absent keys keep the runs as now; this
   repository sets 4 and 4.
9. Captured output (CK1-1, widened on 2026-11-26): the reading of a step run counts only libtest's own
   `running` and `test result:` lines, not those inside the captured output of a failing test.

Not promised: times under a second (whole seconds, rounded down); a test file name that libtest does not
print; a log on a disk that is full or read-only (the check warns and goes on); times of records written
before this ticket.

**Prior art.** STP-4 and STP-6 (the runner and its time limit), STP-35 (the limit of the whole check and
the cache of steps), STP-42 (cargo slots), STP-44 (the time-outs that led to this item).

**Size.** About 500 source and 700 test lines; about 12 Plan steps. Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-26: Decision (scope, the human): the ticket takes the whole item 5.2c, its later part (the five
  slowest test files, the suite time against the last pass) included. Rejected (recommended): the first
  part only.
- 2026-11-26: Decision (run settings, the human, recommended): item 5.2c is widened by a ledger decision:
  `[check]` keys give the jobs and the test threads of every cargo run of the check; this repository sets
  4 and 4. Rejected: a plan item of its own; no change.
- 2026-11-26: Decision (logs, the human, recommended): the newest 10 logs of each ticket are kept.
  Rejected: the newest one only; all logs.
- 2026-11-26: Decision (suite build, agent): the suite runs as two cargo runs, a `--no-run` build and then
  the run, so the build time and the run time are measured, not guessed from the output.
- 2026-11-26: Decision (progress period, agent): the period is a `[check] progress_secs` key, 300 by
  default, so that a test can set it to 1.
- 2026-11-26: Assumption: libtest prints `finished in <s>s` in each `test result:` line, and cargo prints
  a `Running <file>` line on standard error before each test binary; both hold for the toolchains in use.

## Spec

Terms:

- **Check**: a run of `stapel check KEY` with the cargo runner, at any level; the suite criteria (AC-6
  to AC-8, AC-14) apply where the suite runs (levels `head` and `full`).
- **Common folder**: the folder that `git rev-parse --git-common-dir` prints, made absolute.
- **Log folder**: `<common folder>/stapel/check-logs/`.
- **Log**: the file `<log folder>/<KEY>-<id>.log`, where `<id>` is the `id` of the check's record in
  `runs.jsonl` (`c-` and 12 lowercase hex digits); a **log of the ticket** is a file of the log folder
  whose whole name has that form for KEY.
- **Cargo run**: one run of the cargo program by the check, `--version` left out.
- **Start**: the instant right after the check took its cargo slot (or found the held mark).
- **Stage**: the part of the check a cargo run belongs to: `<label> RED`, `<label> GREEN`, `<label> HEAD`
  for a step (`<label>` as the step line prints it, `step 2`), `suite build`, `suite run`.
- **Running file**: for a step run, the test file of that run as `<dir>/<file>` (`tiny/basic`); for a
  suite run, the target of the newest whole `Running` line of its standard error so far, named as the
  `failing:` line names it (STP-41); `-` when there is none.
- **Seconds**: whole seconds, rounded down, of the wall clock; a run cut by the time limit counts until
  it was killed.
- **Target time**: the number `<x>` of `finished in <x>s` in a `test result:` line of the suite run whose
  block ran one or more tests, belonging to the target of its block (the k-th `running` block to the k-th
  `Running` or `Doc-tests` line, as for the `failing:` line); a doc-test target is `doc <crate>`.
- **Tail line**: a line of output with each control character replaced as `printable` does it today
  (STP-37) and cut to 500 characters.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the first cargo run of a check starts THE check makes its log and prints `log: <path>` before anything of that run; the record holds `log`; for each cargo run, the timed-out one included, the log gets `== <stage>: <cargo> <args>` and then the run's kept standard output and standard error. A check with no cargo run makes no log. | `check_times::log_holds_every_run` |
| AC-2 | WHEN a check creates its log THE it removes the logs of its ticket beyond the newest 10 by modification time, ties by name (the new one counted), and leaves every other file of the log folder. | `check_times::keeps_ten_logs_per_ticket` |
| AC-3 | IF the log cannot be made THEN the check prints `warning: no check log: <error>` on standard error, no `log:` line and no `log` field, and no later run tries again; IF a later write fails THEN it prints `warning: check log write failed: <error>` once and writes no more; IF an old log cannot be removed THEN `warning: old check log not removed: <path>: <error>`; in each case the check runs and ends as without the fault. | `check_times::unwritable_log_folder` |
| AC-4 | WHEN a check has made its copy and taken its cargo slot THE it prints `copy: <n> s` and then `cargo slot: waited <n> s` before the `ticket:` line, and the record holds `times.copy` and `slot_wait` in seconds; under the held mark of an outer `stapel cargo` the wait is `0`. | `check_times::copy_and_slot_times` |
| AC-5 | WHEN a step ran one or more cargo runs, whatever its outcome THE the line after its step line (and its notes) is `  times: RED <n> s, GREEN <n> s, HEAD <n> s`, naming only the commits it ran at, the seconds summed over its test files, and the step of the record holds `times` with the keys `red`, `green`, `head` for them; a step that ran none has neither. | `check_times::step_times` |
| AC-6 | WHEN the suite runs THE the check first runs `<cargo> test --workspace --locked --no-run` (with `-j` of AC-11), prints `suite build: <n> s` before the `suite:` line and writes `times.suite_build`; then the run as before, with `times.suite_run`; when the build runs out of the time limit, no run starts, the suite is `unverified` as before, and there is no `times.suite_run`. | `check_times::suite_build_and_run` |
| AC-7 | WHEN the suite run ended `pass` or `fail` THE the `suite:` line ends with `; run <n> s, last pass <m> s`, `<m>` being the `times.suite_run` of the last record of the ticket in `runs.jsonl` (file order) with `result` `pass` and that field, or `; run <n> s, last pass none`; malformed lines are skipped; an `unverified` or `skipped` suite has no such end. | `check_times::last_pass_time` |
| AC-8 | WHEN the suite run gave one or more target times THE the check prints `slowest: <target> <x> s, …` after the `suite:` and `failing:` lines with the five largest, largest first, equal ones in output order, `<x>` as libtest printed it, and the record holds `slowest` as an array of `{"target": <string>, "secs": <number>}`; with no target time there is no line and no field. | `check_times::slowest_files` |
| AC-9 | WHILE a cargo run of the check runs THE the check prints `progress: <stage>, <running file>, <n> s` (`<n>` since the start) each time `[check] progress_secs` seconds (300 when absent) have passed since the start or since its last progress line. | `check_times::progress_lines` |
| AC-10 | IF the time limit runs out during a cargo run THEN the check prints on standard output `time limit: <stage>, <running file>`, then the last 20 tail lines of the kept standard output of the run, each after `  out | `, then the last 20 of its kept standard error, each after `  err | `, and ends as before (`unverified`, `result: fail`, exit 1). | `check_times::limit_names_the_stage` |
| AC-11 | WHEN `[check] jobs = J` is set THE every cargo run of the check gets `-j J` after `--locked`; WHEN `[check] test_threads = T` is set THE every run that runs tests gets `--test-threads=T` right after `--` (before `--exact` or `--skip`; a `--` is added where the run had none), the build of AC-6 none; without the keys the arguments are those of before. | `check_times::run_settings` |
| AC-12 | IF `[check] jobs` or `test_threads` is outside 1 to 1024, or `progress_secs` outside 1 to 86400 THEN every command that loads the config prints `check.<key> must be a whole number from 1 to <max>` and exits 1; IF one of them is set with the junit runner THEN `check.<key> needs the cargo runner`; a value that is not a whole number gives the TOML error, exit 1. | `check_times::refuses_bad_settings` |
| AC-13 | WHEN this repository's `.stapel/stapel.toml` is loaded THE its `[check]` has `jobs = 4` and `test_threads = 4`. | `check_times::this_repository_sets_four` |
| AC-14 | IF the build of AC-6 exits non-zero, prints `error: could not compile` or `because --locked was passed`, or a build input above the copy is found after it THEN no run starts and the suite gets the outcome and detail that the run gave for that output before (`fail: … the workspace does not compile`, `unverified: Cargo.lock is out of date`, `unverified: build-input-outside: …`), with counts 0 and no `times.suite_run`. | `check_times::failed_build_judges_the_suite` |
| AC-15 | WHEN a step run's standard output holds a section `---- <name> stdout ----` of a failing test THE the reading of that run skips every line from the first such heading up to the last line `failures:` of the output, so `running`, `failures:` and `test result:` lines inside a captured output count neither as blocks nor as summaries, and the step is judged by libtest's own lines. | `check::parse_skips_captured_output`, `check::captured_output_holds_a_failures_line` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-4 to AC-6 and AC-14; item 2 by AC-4; item 3 by
  AC-9; item 4 by AC-10; item 5 by AC-1 to AC-3; item 6 by AC-8; item 7 by AC-7; item 8 by AC-11 to
  AC-13; item 9 by AC-15. Unchanged: the junit runner (its tests in `check_junit.rs` keep passing), the suite outcomes of
  before (AC-14), the level `steps` (no suite, no suite lines), `stapel check --list` and `--history`,
  the hook's step lines, the cache of steps.
- **Not promised:** the Frame's list; the log of a run past the 16 MB that the runner keeps of a stream
  (the log holds what was kept); a progress line between two cargo runs; the order of standard output and
  standard error lines inside the log of one run; a log whose name exists already (ids are unique); a
  tail of AC-10 for a stream past those 16 MB (the tail is that of the kept part); a
  log left by a check that ended after its first cargo run but before its record was written (it keeps
  its id, which `runs.jsonl` lacks, and counts in AC-2); the compile of doc-tests, which cargo does in
  the run, not in the `--no-run` build (it counts in `suite run`); a line about the wait for a cargo slot
  while it lasts (SR1-15, deferred).

**Author self-check (item 7).** Done on 2026-11-26: each promise item maps to a criterion; `--no-run`
builds the same test binaries the run then uses, so the run does not build again; the runner already
keeps both streams of each run in memory (`RunResult`), so the log and the time-out tail need no change of
how output is read, only a look at the buffers while the run waits (AC-9, AC-10); the fake cargo of the
tests answers `--no-run`, `-j`, `--test-threads`, a sleep marker and `finished in`, so no test runs a
nested real cargo (STP-4). After the spec review (2026-11-26): the log is made at the first cargo run,
so a check with only cached steps makes none and pushes no real log out (SR1-3); the shared fake answers
`--no-run` with the compile result of the workspace and no tests, so the old tests keep their outcomes
and only those that count `--workspace` runs change (SR1-4).

## Test plan

Integration tests in `crates/stapel-cli/tests/check_times.rs`, on the fixture and fake cargo of
`tests/common/` (`fake_cargo.sh`, extended in the REDs that need it); the golden
`tests/golden/check_report.txt` of `check::report_matches_golden` gains the new lines, with the log path
and the seconds made neutral. Light tests: `stapel cargo test -j 4 -p stapel-cli --test check_times --
--test-threads=4` (and `--test check` for the golden); before each GREEN also `--test ticket_drift` and
`--test fixtures`.

| Category | Cases | Tests |
|---|---|---|
| Log | one step and the suite: the `log:` line before the first step line, the file exists, one `==` header per step run and the `suite run` header (the `suite build` header is checked by `check_times::suite_build_and_run`), the fake's output of each run under it; the record's `log`; level `steps` with every step cached: no `log:` line, no file, no `log` field | `check_times::log_holds_every_run` |
| Retention | 11 logs of the ticket `ABC-1`, made older with set modification times, two of them with one time, 2 logs of `ABC-1-X` (`ABC-1-X-c-<hex>.log`), and a file `notes.txt`: after a check, 10 of the ticket (the oldest gone, the tie by name), the other logs and `notes.txt` stay, and no removal warning | `check_times::keeps_ten_logs_per_ticket` |
| No log | the log folder path taken by a plain file: the warning once, no `log:` line, no `log` field, the check passes | `check_times::unwritable_log_folder` |
| Copy, slot | the two lines in order before `ticket:`, whole numbers, `times.copy` and `slot_wait` in the record; with the held mark set, `waited 0 s` | `check_times::copy_and_slot_times` |
| Step times | a passing step: `times:` with RED, GREEN, HEAD and the record keys; a cached step at the level `head`: HEAD only; a step unverified by the time limit at GREEN: RED and GREEN; at the level `steps` with a cached step: no `times:` line | `check_times::step_times` |
| Suite build | the fake logs its arguments: a `--no-run` run with `-j` of `jobs = 3` before the suite run, its `== suite build:` header in the log; `suite build:` line before `suite:`; both record times; a sleep in the build past the time limit: no suite run, no `suite_run` | `check_times::suite_build_and_run` |
| Last pass | no earlier record: `last pass none`; an earlier passing record with `times.suite_run` 7, a later passing one without it, a failing one with 9 and a malformed line: `last pass 7 s`; an unverified suite: no `; run` end | `check_times::last_pass_time` |
| Slowest | a fake suite with seven targets and given `finished in` times, two equal, and a block of 0 tests with the largest time: the five largest of the blocks with tests in order, the equal ones in output order, the record array with numbers; a suite with no `finished in`: no line | `check_times::slowest_files` |
| Progress | `progress_secs = 1`, a test that sleeps 3 s: at least two `progress:` lines naming the step stage and its file; a suite target that sleeps: `suite run` and the target | `check_times::progress_lines` |
| Time-out | `timeout_secs = 10` (the lowest the config takes), a step test that sleeps 60 s and prints 30 lines on each stream, one with an escape byte: `time limit: step 1 RED, tiny/basic`, the last 20 lines of each stream after `  out | ` and `  err | `, the escape replaced, `result: fail`, exit 1; the log holds the timed-out run | `check_times::limit_names_the_stage` |
| Settings | `jobs = 3`, `test_threads = 2`, with and without `suite_exclude`: the logged arguments of step and suite runs (the build in `check_times::suite_build_and_run`), `--test-threads=2` right after `--`; no keys: the old arguments | `check_times::run_settings` |
| Bad settings | `jobs` 0 and 1025, `test_threads` 0, `progress_secs` 0 and 86401, `jobs` 1024 accepted; `jobs` and `progress_secs` with the junit runner; `jobs = "x"` | `check_times::refuses_bad_settings` |
| This repository | the loaded config of the repository root | `check_times::this_repository_sets_four` |
| Failed build | a workspace that does not compile: one `--no-run` run, no suite run, `fail: … the workspace does not compile`, counts 0, no `suite_run`; a stale lock file: `unverified: Cargo.lock is out of date` | `check_times::failed_build_judges_the_suite` |
| Golden | the full output of the golden fixture with the new lines | `check::report_matches_golden` |
| Captured output | a run output with one failing test whose captured stdout holds two `running 1 test` blocks and their `test result:` lines, then libtest's own `failures:` list and summary: the names are counted from libtest's lines only, not unverified | `check::parse_skips_captured_output` |
| Captured failures list | a captured output that holds a whole failing libtest run of its own (`running 1 test`, `failures:`, a name, `test result: FAILED`) before the real `failures:` list and summary: one block and one summary are counted | `check::captured_output_holds_a_failures_line` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[check] jobs` | whole number | 1 / 1024 | absent: no `-j` | 0 or over 1024: refused; not a number: TOML error; with junit: refused | exact | `check_times::run_settings`, `check_times::refuses_bad_settings` |
| `[check] test_threads` | whole number | 1 / 1024 | absent: no `--test-threads` | 0 or over 1024: refused; not a number: TOML error; with junit: refused | exact | `check_times::run_settings`, `check_times::refuses_bad_settings` |
| `[check] progress_secs` | whole number of seconds | 1 / 86400 | absent: 300 | 0 or over 86400: refused; not a number: TOML error; with junit: refused | whole seconds | `check_times::progress_lines`, `check_times::refuses_bad_settings` |
| earlier records in `runs.jsonl` | JSON lines | none / any | no file or no passing record: `last pass none` | malformed line: skipped; no `times.suite_run`: skipped | whole seconds | `check_times::last_pass_time` |
| `finished in <x>s` of libtest | decimal | 0.00 / any | absent: target left out | not a number: target left out | as printed | `check_times::slowest_files` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| log folder | absent; present; a plain file in its place; read-only | made; used; warning and no log (AC-3); read-only: the same warning, not promised further | `check_times::unwritable_log_folder`, `check_times::log_holds_every_run` |
| older logs | fewer than 10; more than 10; logs of other tickets; other files | kept; the oldest removed; kept; kept | `check_times::keeps_ten_logs_per_ticket` |
| cargo slot | free; held by another process; held mark of an outer holder | `waited 0 s` or more; the wait counted; `waited 0 s` | `check_times::copy_and_slot_times` |
| cargo output | `Running` lines present; absent; `finished in` present; absent | running file and targets named; `-`; times read; no `slowest:` | `check_times::progress_lines`, `check_times::slowest_files` |
| cargo build | compiles; does not compile; lock file stale; build input found above the copy after it | the run follows; AC-14 outcomes, no run | `check_times::failed_build_judges_the_suite` |
| time limit | runs out in a step run; in the suite build; in the suite run; between runs | AC-10 names the stage; the build: no run (AC-6); the run: AC-10; between runs: as before | `check_times::limit_names_the_stage`, `check_times::suite_build_and_run` |

**Review Focus:** a failed removal of an old log (the AC-3 warning; no test); a real cargo run with `--no-run` and then the run (no test runs a real cargo: the
check of this ticket at review is the stand, and its output is quoted in Review); a progress line while
cargo builds and prints nothing (the running file `-`, covered by the suite build case only through the
code review); a log line that holds bytes that are not UTF-8 (written as the runner kept them, lossy;
no test); the clock going back during a check (seconds saturate at 0; no test).

## Design

### Facts

- F-1: `crates/stapel-cli/src/check.rs` holds `run` and the `Runner`; step runs are in `Runner::at`
  (one cargo run per test file, at RED, GREEN and HEAD), the suite in `Runner::suite`.
- F-2: `crates/stapel-core/src/runner.rs` `run_child` reads both streams into shared buffers
  (`read_capped`) and polls the child every 50 ms until it ends or the time limit runs out.
- F-3: the record id comes from `new_id("c")` at the end of `run`; the record is appended to
  `runs.jsonl` of the ticket.
- F-4: `crate::cargo::take` waits for a slot with no time limit and returns `None` under the held mark.
- F-5: `cargo_failing_names` already maps `Running` and `Doc-tests` lines of standard error to target
  names and the `running` blocks of standard output to them.
- F-6: `Check` in `crates/stapel-core/src/config.rs` has `deny_unknown_fields`; `suite_exclude` is
  refused with the junit runner at load.

### Shape

- `runner.rs`: `run_child` takes an optional watcher, called from its poll loop with the elapsed time and
  read access to the two buffers; `run_cargo` passes it through. The watcher of the check prints the
  progress line (AC-9). The run result is unchanged.
- `check.rs`: the record id is made at the start; a `CheckLog` (path, open file or `None` after a
  warning) is made at the first cargo run and written after each cargo run with its stage; the `Runner`
  holds the log, the start instant, the progress period, the current stage and per-step times; `at`
  returns the seconds of its runs with the states; `suite` runs the build then the run and returns both
  times, the target times and the time-out tail; the last pass is read from `runs.jsonl` before the new
  record is appended.
- `config.rs`: `jobs`, `test_threads`, `progress_secs` as `Option<u64>` in `Check`, checked at load as
  `suite_exclude` is.
- `.stapel/stapel.toml` of this repository: `jobs = 4`, `test_threads = 4`.

### Decisions (before the build)

- 2026-11-26, agent: the time-out prints the last 20 lines of each stream, not 20 of both together,
  because the runner keeps the two streams apart and their order across streams is lost.
- 2026-11-26, agent: the step times go on a line of their own after the step line, so the step line that
  the hook and `--history` share stays the same.
- 2026-11-26, agent (SR1-3): the log is made at the first cargo run, so a check that runs no cargo
  makes no log and does not push a real log out of the newest 10.
- 2026-11-26, agent (SR1-5): `jobs` and `test_threads` are bounded by 1024 and `progress_secs` by 86400,
  refused above at load, because TOML allows larger numbers than cargo and `Instant` take.
- 2026-11-26, agent (SR1-12): the seconds of a progress line count from the moment the cargo slot was
  taken, so a long slot wait does not hide in them; the wait has its own line.
- 2026-11-26, agent (SR1-14): `--test-threads` comes right after `--`, before `--exact` and `--skip`.
- 2026-11-26, agent: the last pass compares the suite run time, not the build time, because the build
  time depends on what the target folder of the check holds.

### Decisions (as built)

- 2026-11-26, agent: the RED of step 1 loaded the config through `stapel status`, which exits 1 with no
  open ticket whatever the config; its GREEN (a3519ec) runs `stapel inbox` instead, with a `history:` line.
- 2026-11-26, agent: the removal warning of AC-3 belongs to retention (step 3); `keeps_ten_logs_per_ticket`
  asserts that a normal run prints none, and the failed removal itself has no test (Review Focus).
- 2026-11-26, agent: the RED of step 4 was amended in place (3694851), so the old tests broken by the
  lines before `ticket:` change there and fail at RED.
- 2026-11-26, agent: the test-only commit 7bc1013 makes `closed_pipe::closed_output_keeps_every_write`
  drop `log`, `times` and `slot_wait` when it compares two runs' records.
- 2026-11-26, agent: `timeout_secs` takes 10 at least, so the time-limit tests use 10 s and a fake that
  sleeps 60 s; the fake cargo gained the markers `fake: sleeps <n>`, `fake: noise <n>`,
  `fake: finished <x>`, `finished none`, `fake: build sleeps` (in a source file) and `fake: stale lock`
  (in `Cargo.lock`).
- 2026-11-26, agent: the step run and the suite build and run go through one `Runner::cargo_run`, which
  passes a `Watch` to `run_cargo_watched` in `runner.rs`; `run_cargo`, `run_program` and `cargo_version`
  run as before.
- 2026-11-26, the person: step 12 needs a `stapel` that knows `jobs`, because the guard loads
  `.stapel/stapel.toml` with the installed binary; `stapel` was installed from this branch before the
  GREEN (e0b0f4d). After the merge it is installed again from the pinned checkout.
- 2026-11-26, the person (CK1-1): the check of c-b0421055b3d3 read step 6 as unverified, because the
  failure message of `log_holds_every_run` at RED printed the log, whose fake output holds `running` and
  `test result:` lines; the fix is AC-15 and Plan step 13, and the check runs with `stapel` installed
  from this branch.
- 2026-11-26, agent: the check of c-3e8bef6f581d (level steps) still read step 6 as unverified: the
  captured output held a whole failing fake run with its own `failures:` line, where the skip of step 13
  ended. AC-15 now ends the skip at the last `failures:` line, which libtest prints after every captured
  section; Plan step 14 makes that change.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check_times::log_holds_every_run` |
| AC-2 | `check_times::keeps_ten_logs_per_ticket` |
| AC-3 | `check_times::unwritable_log_folder` |
| AC-4 | `check_times::copy_and_slot_times` |
| AC-5 | `check_times::step_times` |
| AC-6 | `check_times::suite_build_and_run` |
| AC-7 | `check_times::last_pass_time` |
| AC-8 | `check_times::slowest_files` |
| AC-9 | `check_times::progress_lines` |
| AC-10 | `check_times::limit_names_the_stage` |
| AC-11 | `check_times::run_settings` |
| AC-12 | `check_times::refuses_bad_settings` |
| AC-13 | `check_times::this_repository_sets_four` |
| AC-14 | `check_times::failed_build_judges_the_suite` |
| AC-15 | `check::parse_skips_captured_output`, `check::captured_output_holds_a_failures_line` |

Changed by this ticket: `check::report_matches_golden` (its golden gains the new lines of AC-1, AC-4 to
AC-7); `check_levels::head_level`, `check_levels::full_level`, `check_levels::flag_wins_over_the_key` and
`check_levels::suite_exclude`, which count the runs with `--workspace` and now see the build too (AC-6;
`suite_exclude` again at step 8); `check::closed_ticket_is_checked_at_its_last_commit` and
`check::config_toolchain_build_script_and_links_are_unverified` with its golden
`check_report_build_inputs.txt` (AC-4); `check_history::same_lines_as_the_full_check` and
`check_history_reason::a_source_file_test_and_the_cargo_note` (AC-5); `closed_pipe::closed_output_keeps_every_write`
(the record fields, commit 7bc1013).

## Plan

Route: full (tags `interface`, `data`), auto.

1. Settings (AC-11, AC-12; tests `check_times::run_settings`, `check_times::refuses_bad_settings`): the
   RED adds `check_times.rs` and lets the fake cargo log its arguments and accept `-j` and
   `--test-threads`; the GREEN adds the three keys, their checks at load, and the arguments of the step
   and suite runs.
2. Log (AC-1, AC-3; tests `check_times::log_holds_every_run`, `check_times::unwritable_log_folder`,
   `check::report_matches_golden`): the RED adds the tests and the `log:` line to the golden; the GREEN
   makes the id at the start, the log, its header lines and the warning.
3. Retention (AC-2; test `check_times::keeps_ten_logs_per_ticket`): the RED adds the test; the GREEN
   removes the old logs of the ticket.
4. Copy and slot (AC-4; tests `check_times::copy_and_slot_times`, `check::report_matches_golden`): the
   RED adds the test and the two lines to the golden; the GREEN times the copy and the slot wait.
5. Step times (AC-5; tests `check_times::step_times`, `check::report_matches_golden`): the RED adds the
   test and the `times:` line to the golden; the GREEN sums the run times of each step.
6. Suite build (AC-6; tests `check_times::suite_build_and_run`, `check::report_matches_golden`,
   `check_levels::head_level`, `check_levels::full_level`, `check_levels::flag_wins_over_the_key`,
   `check_levels::suite_exclude`): the RED adds the test, the `--no-run` answer and a sleep marker of the
   fake, the `suite build:` line to the golden, and the build run to the counts of the old tests; the
   GREEN runs the build and then the run, whatever the build gave.
7. Failed build (AC-14; test `check_times::failed_build_judges_the_suite`): the RED adds the test; the
   GREEN judges the suite by the build's output when the build fails.
8. Last pass (AC-7; tests `check_times::last_pass_time`, `check::report_matches_golden`): the RED adds
   the test and the end of the `suite:` line to the golden; the GREEN reads the newest passing time.
9. Slowest (AC-8; test `check_times::slowest_files`): the RED adds the test and `finished in` times to
   the fake; the GREEN reads the target times and prints the line.
10. Progress (AC-9; test `check_times::progress_lines`): the RED adds the test; the GREEN adds the
   watcher of `run_child` and the progress line.
11. Time-out (AC-10; test `check_times::limit_names_the_stage`): the RED adds the test; the GREEN prints
    the stage, the running file and the two tails.
12. This repository (AC-13; test `check_times::this_repository_sets_four`): the RED adds the test; the
    GREEN sets `jobs = 4` and `test_threads = 4` in `.stapel/stapel.toml`.
13. Captured output (AC-15; test `check::parse_skips_captured_output`): the RED adds the test; the GREEN
    makes `parse_run` skip the captured output sections of failing tests.
14. Captured failures list (AC-15; test `check::captured_output_holds_a_failures_line`): the RED adds the
    test; the GREEN makes the skip end at the last `failures:` line of the output, not the next one.

## Review

TODO

<!-- stapel:review:begin -->
Generated by stapel; edits between the markers are overwritten.

Findings:

| Stage | Round | Roles | HIGH | MEDIUM | LOW | False | Fates |
|---|---|---|---|---|---|---|---|
| spec-review | 1 | external | 1 | 8 | 7 | 0 | defer 1, no-change 1, patch 14 |
| spec-review | 2 | reviewer | 0 | 3 | 5 | 0 | no-change 1, patch 7 |
| code-review | 1 | reviewer | 1 | 0 | 0 | 0 | patch 1 |

Check runs:

| Level | Result | Commit | At | Steps |
|---|---|---|---|---|
| steps | fail | efae2f9 | 2026-11-26T16:53:32Z | pass 12, unverified 1 |
| head | fail | 4691109 | 2026-11-26T10:59:00Z | pass 11, unverified 1 |
<!-- stapel:review:end -->

## Summary

TODO
