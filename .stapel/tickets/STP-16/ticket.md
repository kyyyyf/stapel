# STP-16 — A closed output pipe ends a command without a panic

## Description

Source: `docs/PHASES.md`, phase 0, STP-16; finding X-1 of STP-13 (`.stapel/tickets/STP-13/findings.jsonl`).

### Frame

**Problem.** The `stapel` binary prints with `println!`, `print!` and `eprintln!` (about 65 places in
`crates/stapel-cli/src`). When the reader of its output closes the pipe early, as `stapel status STP-13 | head
-4` does, the next print panics: "failed printing to stdout: Broken pipe (os error 32)", a backtrace hint goes
to stderr and the exit code is 101. A script or an agent sees a crash where the command did its work. Worse, a
command that prints between two writes stops half-way: `stapel tokens import` appends one record per model and
prints a line after each, so with two models and a closed pipe the second record is never written.

**Risk tags:** `data` (a write cut short by a print), `interface` (the exit code when the pipe is closed).

**Non-goals.** A new output format or a `--quiet` option; the text of any message; Windows (the binary is
built and tested on Linux and WSL); output that clap prints for `--help`, `--version` and usage errors (clap
writes it itself and ignores a write error).

**Promise (closed list).** For every subcommand of the `stapel` binary, when its standard output or standard
error is a pipe whose reader has closed it:

1. the command does not panic and prints no panic message;
2. the command does all its writes to files outside `.git/` that it does with an open pipe (spec change,
   code review DR-1);
3. the exit code is the one the command gives with an open pipe (Decision of 2026-10-13, question 1).

Not promised: a reader that stops reading without closing the pipe (the command blocks, as any program does);
a full disk or another write error on standard output or standard error that is not a closed pipe (Decision
of 2026-10-13, question 2); output of
child processes (`cargo`, `git`) that `stapel check` passes through.

**Size.** Small: three or four criteria, about 100 changed lines of source and tests. Route: full (Decision of
2026-10-13, question 3).

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-13: Decision: the human asked to add the ticket to `docs/PHASES.md` and to start it.
- 2026-10-13: Decision: the key is STP-16; STP-14 and STP-15 are reserved in `docs/PHASES.md`, and `stapel new`
  takes the largest folder key plus one, so it was run with two empty folders in place for one command.
- 2026-10-13: Decision: the human confirmed the frame.
- 2026-10-13: Decision (question 1, answer (a), recommended): with a closed pipe the command stops
  printing to it, finishes its work and gives the exit code it gives with an open pipe. Rejected: (b) 141 as
  for SIGPIPE (the process ends at once and can leave a write half done); (c) always 0 (hides a failure).
- 2026-10-13: Decision (question 2, answer (a), recommended): a write error on standard output or
  standard error that is not a closed pipe is not promised; it keeps today's behaviour.
- 2026-10-13: Decision (question 3, answer (a), recommended): tags `data` and `interface` stay; route
  full.
- 2026-10-13, spec review round 1 (SR1-1 to SR1-8, two HIGH; round 2 follows), fixed in the text: SR1-1 (AC-3
  compares JSON lines without `id` and `at`: `tokens add`, `ok` and `close` take random ids), SR1-2 (closed
  standard error before a write: `tokens add` with a role missing from `stapel.toml`), SR1-3 (AC-2 without a
  fixed code; a hook deny with standard error closed keeps 2; the stdout case of the hook uses an `ask`
  payload), SR1-4 (one full `check` run in AC-3), SR1-5 (AC-3 compares the whole tree but `.git/`), SR1-6
  (AC-4 rule: recursive, comments stripped, `dbg!` and `io::stdout()`/`io::stderr()` too), SR1-7 (`init`
  prompt path with `STAPEL_ASSUME_TTY`), SR1-8 (`status` with a stale confirmation). LOW: Rust 1.87 floor for
  `std::io::pipe`, `std::process::Command` in the test, `--version` named as a non-goal, decided questions
  no longer called open.
- 2026-10-14, code review round 1 (R, E, DR; no HIGH), spec change, needs a new confirmation: promise item 2
  names files outside `.git/` (DR-1); AC-4 also scans `crates/stapel-core/src` (E-1); AC-2, AC-3 and AC-4
  gain coverage tests (R-1, DR-2, R-2, E-1). Plan step 2.
- 2026-10-13, spec review round 2 (SR2-1 to SR2-3, one HIGH), fixed in the text; no round 3 (CLAUDE.md item 4
  allows one more round): SR2-1 (`.json` files compared as parsed values without `id` and `at`: `ok` and
  `close` write times into `state.json`), SR2-2 (the full `check` runs once open and once closed, in one test,
  with `CARGO_BUILD_JOBS=2`), SR2-3 (AC-4 also matches `write!`, `writeln!`, `stdout(`, `stderr(`, `Stdout`,
  `Stderr`; `out.rs` exposes no handle). LOW: the stale fixture named, comment stripping rule named.

## Spec

Terms:

- **Closed pipe**: standard output or standard error of the `stapel` process is the write end of a pipe whose
  read end no process holds; every write to it fails with `EPIPE` (Rust ignores `SIGPIPE`, so no signal ends
  the process).
- **Open run** and **closed run** of a command: the same command line, in the same starting repository and
  with the same standard input, once with both outputs open and once with the named output a closed pipe.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN standard output is a closed pipe THE `stapel` binary, for each case of the test's table (every subcommand of `main.rs`; the full `check` is asserted in AC-3's test), prints no panic message on standard error and exits with the code of the open run. | `closed_pipe::stdout_closed_keeps_exit_code` |
| AC-2 | WHEN standard error is a closed pipe THE `stapel` binary exits with the code of the open run, for a failing command (1), a hook deny (2) and a command that warns and succeeds (0). | `closed_pipe::stderr_closed_keeps_exit_code`, `closed_pipe::check_with_stderr_closed_keeps_exit_code` |
| AC-3 | WHEN standard output or standard error is a closed pipe THE commands that write leave the same tree as the open run, `.git/` aside: `.json` files as parsed values and `.jsonl` files line for line as parsed records, with every `id` and `at` key dropped at any depth; other files byte for byte. For each pair the open run prints to the stream the closed run closes; on standard error the pairs are `tokens add` with a role missing from `stapel.toml`, `ok` with a spaced `user.name` and `init` with a foreign `pre-push` hook (warnings before a write); `new`, `close` and `tokens session` print nothing there. | `closed_pipe::closed_output_keeps_every_write`, `closed_pipe::open_runs_print_to_the_closed_stream` |
| AC-4 | IF a `.rs` file, with `//` comments removed, under `crates/stapel-cli/src/` (any depth) other than `out.rs` matches `print!`, `println!`, `eprint!`, `eprintln!`, `dbg!`, `write!`, `writeln!`, `stdout(`, `stderr(`, `Stdout` or `Stderr`, or under `crates/stapel-core/src/` matches `print!`, `println!`, `eprint!`, `eprintln!`, `dbg!`, `io::stdout(` or `io::stderr(` (spec change, code review E-1; core writes to files and formatters and sets child outputs) THEN the test fails naming the file and line. | `closed_pipe::no_print_macros_outside_out`, `closed_pipe::no_print_macros_in_stapel_core` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-4. AC-4 keeps it for commands added later.
- **Not promised:** the Frame's list; the exact set of subcommands in the test's table beyond those that exist
  at the close of this ticket (AC-4 covers new ones by construction).

## Design

### Facts (external contract and states)

- F-1: the Rust runtime sets `SIGPIPE` to ignored before `main`, so a write to a closed pipe returns
  `ErrorKind::BrokenPipe`; `println!` and `eprintln!` panic on any write error ("failed printing to stdout").
- F-2: child processes do not inherit the outputs: `git` runs through `.output()`, `cargo` in `stapel check`
  through piped outputs (`stapel-core/src/runner.rs`); only `kill` inherits standard output and prints nothing.
- F-3: `init` flushes standard output after its prompt (reached without `--prefix`, with a terminal or
  `STAPEL_ASSUME_TTY`) and returns the flush error as a command error.
- F-6: `stapel_core::journal::new_id` takes random bits, so `tokens add`, `ok` and `close` write different ids
  in two runs; `tokens import` and `tokens session` derive their ids from the input.
- F-7: writes that follow a print today: `tokens import` (one record per model), `tokens add` after a warning
  on standard error, `check` (report first, then `runs.jsonl`), `init` after its prompt.
- F-4: clap prints `--help` and usage errors itself and ignores the write result (non-goal).
- F-5: prior art: ripgrep and fd treat `BrokenPipe` on standard output as a quiet end with code 0; git and
  coreutils die by `SIGPIPE` (141). Neither keeps work running after the pipe closes; this ticket does, for
  promise item 2.

### Decisions (as built)

#### Step 1

- `crates/stapel-cli/src/out.rs` holds the macros `say!`, `say_raw!` and `warn!`; they call two hidden
  functions `out::stdout(args, newline)` and `out::stderr(args)` that take `fmt::Arguments`, write through a
  locked handle, drop `BrokenPipe` and panic on any other error with the text of `println!`. No handle leaves
  the module. `#[macro_use] mod out;` is the first module of `main.rs`.
- `out::flush()` drops `BrokenPipe` and returns any other error; `init` calls it after its prompt.
- Every print site of `crates/stapel-cli/src` moved to the macros; no message text or exit code changed.
- The test `closed_output_keeps_every_write` runs each writing case with standard output closed and with
  standard error closed, asserts that the open run succeeds and writes, and runs the full `check` once open and
  once closed. `tokens import` cases pass `--until`, because a fresh checkout gives the fixture a new
  modification time (B1-1).

#### Step 2

- Coverage only, no code: three tests added at the end of `closed_pipe.rs`. In
  `open_runs_print_to_the_closed_stream` the pairs of `new`, `close`, `tokens session` and the plain `init` cases
  with standard error closed are left out, because these commands print nothing to standard error; `ok` with a
  user name that has a space and `init` with a foreign pre-push hook cover warnings before a write instead.
- `no_print_macros_in_stapel_core` uses narrower patterns than the CLI scan (print macros, `dbg!`,
  `io::stdout(`, `io::stderr(`), because the core writes to files with `write!` and sets `Command::stdout`.

## Test plan

CLI tests in a new file `crates/stapel-cli/tests/closed_pipe.rs`. A closed pipe is made with `std::io::pipe()`
(Rust 1.87 or later): the reader is dropped before the child starts, so every write fails, with no race. The
child runs through `std::process::Command` with `assert_cmd::cargo::cargo_bin("stapel")` and the environment
of `common::stapel`. Each case runs twice on two copies of one fixture repository (copied after it is built,
so commit ids agree): the open run and the closed run. Fixtures: `common::repo_with_ticket`, made stale by `ok` on a
section and then `common::set_section` on it; `tests/fixtures/main.jsonl` (two models) for `tokens import`; `tests/fixtures/session.jsonl` for
`tokens session`; a small cargo crate with one RED/GREEN step for `check` and `check --list`. Heavy runs:
`cargo test -j 4 … -- --test-threads=3`. The full `check` runs once open and once closed, in
`closed_output_keeps_every_write` only, with `CARGO_BUILD_JOBS=2`; that test asserts both the exit code and
the tree, so AC-1's table holds `check --list` and not `check` (SR2-2). AC-4 strips a comment from `//` to the
end of the line when `//` is outside a string literal; `/* */` comments are not used in the crate.

| Category | Cases | Tests |
|---|---|---|
| Main path | standard output closed: `init --prefix`, `init` prompt (`STAPEL_ASSUME_TTY=1`, input `STP`), `new`, `ok`, `status` with a stale section, `close`, `tokens`, `tokens add`, `tokens import`, `tokens session`, `check --list`, `hook pre-tool-use` with a payload that gives `ask` | `closed_pipe::stdout_closed_keeps_exit_code` |
| Negative | standard error closed: unknown ticket key (1), empty title (1), missing transcript (1), a hook deny (2), `tokens add` with a role missing from `stapel.toml` (warning, 0) | `closed_pipe::stderr_closed_keeps_exit_code` |
| Coverage (step 2) | `check` with an unknown key and standard error closed (refusal, 2; R-1, DR-2); each writing case of AC-3 prints to the stream it closes in the open run, with the three warning-before-write cases on standard error, tree and code compared (R-2, R-3); no print site in `crates/stapel-core/src`, narrower patterns (E-1) | `closed_pipe::check_with_stderr_closed_keeps_exit_code`, `closed_pipe::open_runs_print_to_the_closed_stream`, `closed_pipe::no_print_macros_in_stapel_core` |
| Data | writing commands with the output they print to closed: `init` both ways, `new`, `ok`, `close`, `tokens add` (standard error), `tokens import` with two models, `tokens session`, `check` (exit code too) | `closed_pipe::closed_output_keeps_every_write` |
| Regression | a print macro or a direct handle outside `out.rs` | `closed_pipe::no_print_macros_outside_out` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| Standard output | file descriptor 1 | open pipe, closed pipe; a file or a terminal is unchanged and not tested (DR-3) | closed descriptor: not promised | write error other than `EPIPE`: not promised (Decision of 2026-10-13, question 2) | per write call | `closed_pipe::stdout_closed_keeps_exit_code` |
| Standard error | file descriptor 2 | open pipe, closed pipe | closed descriptor: not promised | as standard output | per write call | `closed_pipe::stderr_closed_keeps_exit_code` |
| Subcommand | CLI | every variant of `Command`, `TokensAction` and `HookCommand` in `main.rs`, and `check --list` | no subcommand: clap usage (non-goal) | unknown: clap error (non-goal) | — | `closed_pipe::stdout_closed_keeps_exit_code` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| os | processes: signals (`SIGPIPE` ignored by the Rust runtime) | a closed pipe gives `EPIPE`, dropped by `out.rs` | `closed_pipe::stdout_closed_keeps_exit_code` |
| os | processes: children | no child inherits the outputs (F-2) | — |
| os | case-insensitive file systems; symlinks; permissions; time; WSL | not touched by this ticket | — |
| git | every dimension | not touched: `git` output is captured | — |
| cargo | every dimension | not touched: `cargo` output is captured | — |
| claude-code | hook output read by Claude Code | a deny (standard error, exit 2) keeps 2 with standard error closed; an `ask` answer (standard output) keeps 0 with standard output closed | `closed_pipe::stderr_closed_keeps_exit_code`, `closed_pipe::stdout_closed_keeps_exit_code` |

**Review Focus:** a print between two writes (`tokens import`, `check`, `init`); a warning on standard error
before a write (`tokens add`); the `init` prompt and flush; the hook's deny and `ask` paths; a `print!` without
a newline (`status` with a stale section). Each has a case in the named tests.

**Author self-check:** 2026-10-13, before spec review: every promise item has a criterion and a test; the
Inputs and External states rows come from the catalogue; no open question is left.
2026-10-13, after spec review round 1: every SR1 finding applied; AC-3 renamed; round 2 for the two HIGH.
2026-10-13, after spec review round 2: SR2 findings applied; AC-1 covers `check` by its exit code in AC-3's test.
2026-10-14, after step 1: design as built moved into the ticket; drift findings D1-1 to D1-7 have no defect to fix.
2026-10-14, after step 2: as built of step 2 added; `stapel check` passes at `e7f70c8` (384 tests).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `closed_pipe::stdout_closed_keeps_exit_code` |
| AC-2 | `closed_pipe::stderr_closed_keeps_exit_code`, `closed_pipe::check_with_stderr_closed_keeps_exit_code` |
| AC-3 | `closed_pipe::closed_output_keeps_every_write`, `closed_pipe::open_runs_print_to_the_closed_stream` |
| AC-4 | `closed_pipe::no_print_macros_outside_out`, `closed_pipe::no_print_macros_in_stapel_core` |

## Plan

Route: full (source: `auto`; tags `data`, `interface`).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | `out.rs` macros; every print site of `crates/stapel-cli/src` moved to them; `out::flush()` for `init` | AC-1, AC-2, AC-3, AC-4 | message texts, exit codes and golden files |
| 2 | Coverage after code review round 1: R-1/DR-2, R-2, E-1 (tests only, no code) | AC-2, AC-3, AC-4 | `crates/stapel-cli/src`, the four tests of step 1 |

## Review

Round 1 (2026-10-14), on a read-only copy at `c0f1a1e` with the as-built notes: fresh reviewer (Sonnet, R-1 to
R-8), external reviewer (Fable, E-1 to E-7), drift reviewer (Sonnet, DR-1 to DR-6). No HIGH, so no round 2.
Six MEDIUM, five inside the promise, none in the code: R-1 and DR-2 (`check` with standard error closed), R-2
(AC-3 cases that may not print to the stream they close) and E-1 (AC-4 blind to `stapel-core`) get coverage
tests in plan step 2; DR-1 narrows promise item 2 to files outside `.git/`; E-2 (no test of a non-pipe write
error) is rejected as outside the promise (question 2). D1-2's fate changed from a note to the fix of R-1.

## Summary

TODO
