# STP-19 — Token journal records imported from a file

## Description

Source: `docs/PHASES.md`, phase 0, STP-19 (split from STP-18 at its frame, Decisions of STP-18 of 2026-10-15);
the next-session note `.stapel/scratch/stp19/next-session.md`.

### Frame

**Problem.** A ticket's `tokens.jsonl` takes records from two commands: `stapel tokens import` sums a Claude
Code transcript, and `stapel tokens add` writes one record from its flags. Usage that is in neither form (a
tool other than Claude Code, a file of several model calls) needs one `tokens add` per call, typed by hand.
The guard denies an agent's Edit and Write to `tokens.jsonl`, so such records have no checked path. STP-18
built that path for findings (`stapel findings import FILE [KEY]`, a permission dialog and a one-time grant);
this ticket gives the token journal the same path, and keeps records from a file apart from the measured
records of a transcript.

**Risk tags:** `guard` (a new permission dialog and grant), `data` (records appended to a journal with
history), `interface` (a new command and a changed report).

**Non-goals.** A dialog for `stapel tokens add` or `stapel tokens import` (they stay as they are, also with
measured counts in an agent's shell); records in the legacy (STP-1) shape; checking or rewriting the records
already in `tokens.jsonl`; editing or removing a record; a Bash command that writes `tokens.jsonl` directly
(STP-20).

**Promise (closed list).**

1. `stapel tokens import-records FILE [KEY]` reads `FILE` as JSON lines and appends every record to the
   ticket's `tokens.jsonl` (without `KEY`: the only open ticket; a closed ticket only by an explicit `KEY`),
   all or nothing.
2. A record of `FILE` has `id` (1 to 64 characters, no whitespace or control character), `role`, `model` and
   `source` (`measured` or `estimate`); a `measured` record has `input` and `output` and may have
   `cache_read` and `cache_write`, an `estimate` record has `estimate` and no measured count; counts are whole
   numbers from 0 to 10^15; `step` and `note` are optional, with the rules of `tokens add`; `ticket` is
   optional and equal to the key; `v` is optional and `1`. Any other field (`at`, `transcript`, `importer`,
   `partial`, `from`, `to`, `messages`, `imported_from`, …) is refused. A record that breaks this, a line
   that is not a JSON object, an empty file, more than 100 records or 256 KiB, an `id` repeated in the file or
   already in `tokens.jsonl` is refused with its line number and nothing is written (exit 2). `FILE` as
   `findings import` takes it: a regular UTF-8 file, not `-`, not a machine file.
3. Each appended record is `{"v":1,"id","at":<now, UTC>,"ticket":<folder key>,"role","model","source",
   <counts>,["step"],["note"],"imported_from":<base name of FILE>}`.
4. When an agent runs the command, the guard runs the same checks, issues a one-time grant whose facts name
   the key, the action and a hash of the records, and asks Claude Code to show the permission dialog, as for
   `findings import`. The dialog shows the count, the file's base name, the sums of the measured counts and of
   the estimates, and one entry per record: `id`, `role`, `model`, `source`, its counts, `step` and the start
   of `note` up to 40 characters; at most 30 records go through the dialog. A refusal of the checks is a
   denial with its reason, without a dialog.
5. The command run with the grant appends only when the records still hash to the granted value; in an
   agent's shell without a grant it refuses, as `findings import` does. A person in a terminal runs it
   without a dialog.
6. `stapel tokens [KEY]` shows records with `imported_from` in their own row per role and model, with the
   role cell `<role> (file)`; the other rows and the columns do not change.

Not promised: deliberate circumvention (no `security` tag); a `FILE` that changes between the dialog and the
run other than by refusal; two imports at the same moment; that the counts in `FILE` are true (the dialog
shows them, the person decides).

**Prior art.** `stapel findings import` (STP-18): `records::plan_findings`, `read_input`, `check_findings`,
the dialog in `crates/stapel-core/src/dialog.rs`, the grant in `grant.rs`, the command in
`crates/stapel-cli/src/findings.rs`. `stapel tokens add` (STP-3): the field rules in
`crates/stapel-cli/src/tokens.rs`. The report: `tokens::report`. The three record shapes in the history:
v1 `measured` (with or without `importer`, `partial`), v1 `estimate`, legacy (`ts`, `kind`,
`total_tokens` or `input_tokens` and `output_tokens`).

**Size.** About 350 changed source lines (`crates/stapel-core`: the token record checks in `records.rs`,
`dialog.rs`, `guard.rs`; `crates/stapel-cli`: the command module, `main.rs`, the report row in `tokens.rs`)
and about 1 000 test lines. STP-18's estimate was low by a factor of two; if the spec grows past about 500
source lines, the ticket is split before the spec review. Route: full (tags `guard`, `data`, `interface`).

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-15: Decision (STP-18 frame, not recommended there): records of `tokens.jsonl` imported from a file
  with the dialog of `findings import`; an imported record may be `measured` and gets `imported_from`; the
  report shows such records apart.
- 2026-10-19: Decision (record shape, recommended): `FILE` holds v1 records without the fields the command
  sets: `v`, `at`, `ticket` and `imported_from` are set by the command. Rejected: full v1 records; v1 and
  legacy.
- 2026-10-19: Decision (repeats, recommended): every record carries `id` with the rule of `findings import`;
  an `id` repeated in `FILE` or already in `tokens.jsonl` is a refusal. Rejected: ids made by the command
  (a second import doubles the records); a hash of the file kept in the records.
- 2026-10-19: Decision (fields, recommended): a closed set of fields; the fields of a transcript import and
  `imported_from` in `FILE` are refused. Rejected: other fields allowed and kept.
- 2026-10-19: Decision (`imported_from`, recommended): the base name of `FILE`, because a full path can hold a
  private term (a user name). Rejected: the path as typed; the path from the repository root.
- 2026-10-19: Decision (report, recommended): own row per role and model with the role cell `<role> (file)`.
  Rejected: a new `origin` column; a separate table.
- 2026-10-19: Decision (dialog, recommended): count, file name, sums, and one entry per record with id, role,
  model, source, counts, step and the start of the note; at most 30 records. Rejected: totals only.
- 2026-10-19: Decision (`tokens add`, recommended): a non-goal; `tokens add` and `tokens import` keep running
  without a dialog. Rejected: the same dialog for `tokens add` with measured counts.
- 2026-10-19: Decision (limits and ticket choice, recommended): as STP-18: 100 records and 256 KiB in `FILE`,
  30 in the dialog; a closed ticket only by an explicit `KEY`; the input file rules of `findings import`.
- 2026-10-19: Decision (estimate records, recommended): every record from a file gets `imported_from` and its
  own report row, `estimate` as well as `measured`. Rejected: only `measured`.
- 2026-10-19: Decision (`model`, recommended): required in `FILE`; no default from `stapel.toml`. Rejected:
  the default of `tokens add`.
- 2026-10-19: Decision (frame): the human confirmed the frame in the chat; `stapel ok` has no section for it.
- 2026-10-19: Assumption: `ticket` may be given in `FILE` and must then equal the folder key, as in
  `findings import`; the command writes the folder key in every case.
- 2026-10-19: Assumption: `at` of every appended record is the time of the run, the same for all records of
  one import.

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
