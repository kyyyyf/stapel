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

## Spec

TODO

## Design

TODO

## Proof

TODO

## Plan

TODO

## Review

TODO

## Summary

TODO
