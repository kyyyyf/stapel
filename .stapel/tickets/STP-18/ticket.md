# STP-18 — Machine records through stapel: findings import and ledger add

## Description

Source: `docs/PHASES.md`, phase 0, STP-18 (asked by the human at the close of STP-14); the next-session note
`.stapel/scratch/stp18/next-session.md`.

### Frame

**Problem.** Review findings and process-ledger records are machine records: JSON lines with history, read by
the measures after a close. The guard denies an agent's Edit and Write to a ticket's `findings.jsonl`, so in
STP-14 the human typed nine `cat … >> findings.jsonl` commands and one for the ledger. Nothing checked those
lines: the history holds `round` as a number and as a string, about 80 forms of `fate`, and records without
`stage` or `promise`. `.stapel/process-ledger.jsonl` is not protected at all: an agent's Edit or Write to it
is allowed today.

**Risk tags:** `guard` (new permission dialogs and grants; the ledger becomes a machine file), `data` (records
appended to files with history), `interface` (two new commands).

**Non-goals.** Importing token journal records from a file (split to its own ticket, Decisions); writes to
`runs.jsonl`, `state.json` and `decisions.jsonl`; a Bash command that writes these files directly (`cat >>`,
`tee`, a script), which the guard does not see today; checking or rewriting the records already in the files;
editing or removing a record.

**Promise (closed list).**

1. `stapel findings import FILE [KEY]` reads `FILE` as JSON lines and appends every record to the ticket's
   `findings.jsonl` (without `KEY`: the only open ticket), all or nothing.
2. Each record must have `id`, `ticket` (equal to the key), `stage`, `round` (a number or a string), `role`,
   `severity` (`HIGH`, `MEDIUM`, `LOW`), `promise` (`inside`, `outside`), `claim`, `catchable_at` (`spec`,
   `design`, `test-plan`, `code`) and `fate` (non-empty text); `stage` and `role` come from closed lists;
   `rule` is optional text; other fields are allowed and kept. A record that breaks this, a line that is not a
   JSON object, an empty file, an `id` repeated in the file or already in `findings.jsonl` is refused with its
   line number and nothing is written (exit 2).
3. `stapel ledger add --kind KIND --note TEXT [KEY]` appends one record `{"v": 1, "at": <today, UTC>, "kind",
   "ticket", "note"}` to `.stapel/process-ledger.jsonl`; `KIND` is `finding`, `catches` or `decision`; the
   note must not be empty.
4. When an agent runs either command, the guard runs the same checks, issues a one-time grant whose facts name
   the key, the action and a hash of the records, and asks Claude Code to show the permission dialog, as for
   `stapel ok`. The dialog shows the count, the totals by severity, and one line per record: `id`, `severity`,
   `promise`, and the start of `claim` up to 80 characters (for the ledger: kind, ticket and the note). A
   refusal of the checks is a denial with its reason, without a dialog.
5. The command run with the grant appends only when the records still hash to the granted value; in an agent's
   shell without a grant it refuses, as `stapel ok` does. A person in a terminal runs it without a dialog.
6. `.stapel/process-ledger.jsonl` becomes a machine file: the guard denies an agent's Edit and Write to it.

Not promised: deliberate circumvention (no `security` tag); a `FILE` that changes between the dialog and the
run other than by refusal; two imports at the same moment; records with a `ticket` field in another case
than the folder.

**Prior art.** `stapel ok KEY` (STP-17): the guard prepares, issues a grant over the hashes and asks
(`crates/stapel-core/src/dialog.rs`, `grant.rs`); `stapel tokens import` appends to a journal with
`stapel_core::journal::append`. klc keeps findings in a file the reviewer writes directly.

**Size.** About 430 changed source lines (`crates/stapel-core`: a records module for the checks, `dialog.rs`,
`guard.rs`; `crates/stapel-cli`: two command modules, `main.rs`) and about 1 000 test lines. With the token
import it was about 580 and 1 400, so it was split. Route: full (tags `guard`, `data`, `interface`).

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-15: Decision (record check, recommended): the required fields strictly, closed lists for
  `severity`, `promise`, `catchable_at`, `role` and `stage`, `round` a number or a string, `fate` free text,
  other fields allowed. Rejected: an exact field set; presence only.
- 2026-10-15: Decision (dialog for many records, recommended): count, totals by severity, one line per record
  with id, severity, promise and the start of the claim. Rejected: totals only; the full JSON.
- 2026-10-15: Decision (ledger command, recommended): flags `--kind` and `--note`; `v` and `at` set by the
  command. Rejected: a file of records; both forms.
- 2026-10-15: Decision (token journal, not recommended): records of `tokens.jsonl` imported from a file with
  the same dialog; an imported record may be `measured` and gets the field `imported_from` with the file name;
  the report shows such records apart. Rejected: leave it out; only `estimate`.
- 2026-10-15: Decision (split, recommended): the token record import is its own ticket after STP-18, because
  the promise grew to about 580 source and 1 400 test lines. Rejected: one ticket.
- 2026-10-15: Decision (ledger file, recommended): `.stapel/process-ledger.jsonl` becomes a machine file.
  Rejected: only the new command.
- 2026-10-15: Assumption: `claim` is required, because all 669 records in the history have it.
- 2026-10-15: Assumption: the `stage` list for new records is `spec-review`, `step-drift`, `code-review`,
  `after-close`; the round goes in `round`, not in the stage name (the history has `spec-review-2`).
- 2026-10-15: Assumption: the `role` list is `external`, `drift`, `reviewer`, `fresh-code`, `builder`,
  `orchestrator`, `human` (the first six are in the history).
- 2026-10-15: Assumption: `ledger add` without `KEY` takes the only open ticket, as `tokens import` does; a
  record without `ticket` (two in the history) is not promised.

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
