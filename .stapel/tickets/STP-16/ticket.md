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
built and tested on Linux and WSL); output that clap prints for `--help` and usage errors (clap writes it
itself and ignores a write error).

**Promise (closed list).** For every subcommand of the `stapel` binary, when its standard output or standard
error is a pipe whose reader has closed it:

1. the command does not panic and prints no panic message;
2. the command does all its writes to files and to git that it does with an open pipe;
3. the exit code is the one the command gives with an open pipe (Open question 1).

Not promised: a reader that stops reading without closing the pipe (the command blocks, as any program does);
a full disk or another write error on standard output that is not a closed pipe (Open question 2); output of
child processes (`cargo`, `git`) that `stapel check` passes through.

**Size.** Small: three or four criteria, about 100 changed lines of source and tests. Route: light if the
human drops the tags (Open question 3), else full.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-13: Decision: the human asked to add the ticket to `docs/PHASES.md` and to start it.
- 2026-10-13: Decision: the key is STP-16; STP-14 and STP-15 are reserved in `docs/PHASES.md`, and `stapel new`
  takes the largest folder key plus one, so it was run with two empty folders in place for one command.
- 2026-10-13: Decision: the human confirmed the frame.
- 2026-10-13: Decision (Open question 1, answer (a), recommended): with a closed pipe the command stops
  printing to it, finishes its work and gives the exit code it gives with an open pipe. Rejected: (b) 141 as
  for SIGPIPE (the process ends at once and can leave a write half done); (c) always 0 (hides a failure).
- 2026-10-13: Decision (Open question 2, answer (a), recommended): a write error on standard output or
  standard error that is not a closed pipe is not promised; it keeps today's behaviour.
- 2026-10-13: Decision (Open question 3, answer (a), recommended): tags `data` and `interface` stay; route
  full.

## Spec

Terms:

- **Closed pipe**: standard output or standard error of the `stapel` process is the write end of a pipe whose
  read end no process holds; every write to it fails with `EPIPE` (Rust ignores `SIGPIPE`, so no signal ends
  the process).
- **Open run** and **closed run** of a command: the same command line, in the same starting repository and
  with the same standard input, once with both outputs open and once with the named output a closed pipe.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN standard output is a closed pipe THE `stapel` binary, for each subcommand in the table of the test, prints no panic message on standard error and exits with the code of the open run. | `closed_pipe::stdout_closed_keeps_exit_code` |
| AC-2 | WHEN standard error is a closed pipe and the command fails THE `stapel` binary exits with the code of the open run (1), not 101. | `closed_pipe::stderr_closed_keeps_exit_code` |
| AC-3 | WHEN standard output is a closed pipe THE commands that write (`init`, `new`, `ok`, `close`, `tokens add`, `tokens import` with two models, `tokens session`) leave the same files under `.stapel/` as the open run, the `at` times aside. | `closed_pipe::closed_stdout_keeps_every_write` |
| AC-4 | IF a source file under `crates/stapel-cli/src/` other than `out.rs` calls `println!`, `print!`, `eprintln!` or `eprint!` THEN the test fails naming the file and line. | `closed_pipe::no_print_macros_outside_out` |

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
- F-3: `init` flushes standard output after its prompt and returns the flush error as a command error.
- F-4: clap prints `--help` and usage errors itself and ignores the write result (non-goal).
- F-5: prior art: ripgrep and fd treat `BrokenPipe` on standard output as a quiet end with code 0; git and
  coreutils die by `SIGPIPE` (141). Neither keeps work running after the pipe closes; this ticket does, for
  promise item 2.

### Decisions (as built)

Planned: a module `crates/stapel-cli/src/out.rs` with macros `say!` (a line to standard output), `say_raw!`
(no newline) and `warn!` (a line to standard error). Each writes through a locked handle, drops a
`BrokenPipe` error and panics on any other error with the text of `println!` (Decision of Open question 2).
Every print site in `crates/stapel-cli/src` uses them; `init`'s flush drops `BrokenPipe` as well. No change
to exit codes or to the text of any message.

## Test plan

CLI tests in a new file `crates/stapel-cli/tests/closed_pipe.rs`. A closed pipe is made with `std::io::pipe()`:
the reader is dropped before the child starts, so every write fails, with no race. Each case runs twice on two
copies of one fixture repository (`common::repo_with_ticket`, a cargo crate for `check --list`, a small
transcript for `tokens import` and `tokens session`): the open run and the closed run. Heavy runs: `cargo test
-j 4 … -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | every subcommand with output: `init --prefix`, `new`, `ok`, `status`, `close`, `tokens`, `tokens add`, `tokens import`, `tokens session`, `check --list`, `hook pre-tool-use` | `closed_pipe::stdout_closed_keeps_exit_code` |
| Negative | failing commands with standard error closed: unknown ticket key, empty title, missing transcript | `closed_pipe::stderr_closed_keeps_exit_code` |
| Data | writing commands; `tokens import` with two models, so the second record follows a print | `closed_pipe::closed_stdout_keeps_every_write` |
| Regression | a print macro outside `out.rs` | `closed_pipe::no_print_macros_outside_out` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| Standard output | file descriptor 1 | open pipe, closed pipe, file, terminal | closed descriptor: not promised | write error other than `EPIPE`: not promised (Open question 2) | per write call | `closed_pipe::stdout_closed_keeps_exit_code` |
| Standard error | file descriptor 2 | open pipe, closed pipe | closed descriptor: not promised | as standard output | per write call | `closed_pipe::stderr_closed_keeps_exit_code` |
| Subcommand | CLI | every subcommand of `stapel --help` at the close | no subcommand: clap usage (non-goal) | unknown: clap error (non-goal) | — | `closed_pipe::stdout_closed_keeps_exit_code` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| os | processes: signals (`SIGPIPE` ignored by the Rust runtime) | a closed pipe gives `EPIPE`, dropped by `out.rs` | `closed_pipe::stdout_closed_keeps_exit_code` |
| os | processes: children | no child inherits the outputs (F-2) | — |
| os | case-insensitive file systems; symlinks; permissions; time; WSL | not touched by this ticket | — |
| git | every dimension | not touched: `git` output is captured | — |
| cargo | every dimension | not touched: `cargo` output is captured | — |
| claude-code | hook output read by Claude Code | `hook pre-tool-use` with a closed pipe keeps its exit code, so a deny stays a deny | `closed_pipe::stdout_closed_keeps_exit_code` |

**Review Focus:** a print between two writes (`tokens import`, `close`, `init`); the `init` flush; a command
that prints to both outputs; the hook's exit code. Each has a case in the named tests.

**Author self-check:** 2026-10-13, before spec review: every promise item has a criterion and a test; the
Inputs and External states rows come from the catalogue; no open question is left.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `closed_pipe::stdout_closed_keeps_exit_code` |
| AC-2 | `closed_pipe::stderr_closed_keeps_exit_code` |
| AC-3 | `closed_pipe::closed_stdout_keeps_every_write` |
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
