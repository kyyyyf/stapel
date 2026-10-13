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
2. the command does all its writes to files and to git that it does with an open pipe;
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

## Spec

Terms:

- **Closed pipe**: standard output or standard error of the `stapel` process is the write end of a pipe whose
  read end no process holds; every write to it fails with `EPIPE` (Rust ignores `SIGPIPE`, so no signal ends
  the process).
- **Open run** and **closed run** of a command: the same command line, in the same starting repository and
  with the same standard input, once with both outputs open and once with the named output a closed pipe.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN standard output is a closed pipe THE `stapel` binary, for each case of the test's table (every subcommand of `main.rs`), prints no panic message on standard error and exits with the code of the open run. | `closed_pipe::stdout_closed_keeps_exit_code` |
| AC-2 | WHEN standard error is a closed pipe THE `stapel` binary exits with the code of the open run, for a failing command (1), a hook deny (2) and a command that warns and succeeds (0). | `closed_pipe::stderr_closed_keeps_exit_code` |
| AC-3 | WHEN standard output or standard error is a closed pipe THE commands that write leave the same tree as the open run, `.git/` aside: other files byte for byte, `.jsonl` files line for line with `id` and `at` dropped from each record. | `closed_pipe::closed_output_keeps_every_write` |
| AC-4 | IF a `.rs` file under `crates/stapel-cli/src/` (any depth) other than `out.rs`, with `//` comments removed, matches `print!`, `println!`, `eprint!`, `eprintln!`, `dbg!`, `io::stdout(` or `io::stderr(` THEN the test fails naming the file and line. | `closed_pipe::no_print_macros_outside_out` |

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

Planned: a module `crates/stapel-cli/src/out.rs` with macros `say!` (a line to standard output), `say_raw!`
(no newline) and `warn!` (a line to standard error). Each writes through a locked handle, drops a
`BrokenPipe` error and panics on any other error with the text of `println!` (Decision of question 2), as
today. A function `out::flush()` serves `init`: it drops `BrokenPipe` and returns any other error, as today.
Every print site in `crates/stapel-cli/src` uses them. No change to exit codes or to the text of any
message.

## Test plan

CLI tests in a new file `crates/stapel-cli/tests/closed_pipe.rs`. A closed pipe is made with `std::io::pipe()`
(Rust 1.87 or later): the reader is dropped before the child starts, so every write fails, with no race. The
child runs through `std::process::Command` with `assert_cmd::cargo::cargo_bin("stapel")` and the environment
of `common::stapel`. Each case runs twice on two copies of one fixture repository (copied after it is built,
so commit ids agree): the open run and the closed run. Fixtures: `common::repo_with_ticket` with one stale
confirmation; `tests/fixtures/main.jsonl` (two models) for `tokens import`; `tests/fixtures/session.jsonl` for
`tokens session`; a small cargo crate with one RED/GREEN step for `check` and `check --list`. Heavy runs:
`cargo test -j 4 … -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | standard output closed: `init --prefix`, `init` prompt (`STAPEL_ASSUME_TTY=1`, input `STP`), `new`, `ok`, `status` with a stale section, `close`, `tokens`, `tokens add`, `tokens import`, `tokens session`, `check`, `check --list`, `hook pre-tool-use` with a payload that gives `ask` | `closed_pipe::stdout_closed_keeps_exit_code` |
| Negative | standard error closed: unknown ticket key (1), empty title (1), missing transcript (1), a hook deny (2), `tokens add` with a role missing from `stapel.toml` (warning, 0) | `closed_pipe::stderr_closed_keeps_exit_code` |
| Data | writing commands with the output they print to closed: `init` both ways, `new`, `ok`, `close`, `tokens add` (standard error), `tokens import` with two models, `tokens session`, `check` | `closed_pipe::closed_output_keeps_every_write` |
| Regression | a print macro or a direct handle outside `out.rs` | `closed_pipe::no_print_macros_outside_out` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| Standard output | file descriptor 1 | open pipe, closed pipe, file, terminal | closed descriptor: not promised | write error other than `EPIPE`: not promised (Decision of 2026-10-13, question 2) | per write call | `closed_pipe::stdout_closed_keeps_exit_code` |
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

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `closed_pipe::stdout_closed_keeps_exit_code` |
| AC-2 | `closed_pipe::stderr_closed_keeps_exit_code` |
| AC-3 | `closed_pipe::closed_output_keeps_every_write` |
| AC-4 | `closed_pipe::no_print_macros_outside_out` |

## Plan

Route: full (source: `auto`; tags `data`, `interface`).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | `out.rs` macros; every print site of `crates/stapel-cli/src` moved to them; `init` flush | AC-1, AC-2, AC-3, AC-4 | message texts, exit codes and golden files |

## Review

TODO

## Summary

TODO
