# STP-20 — stapel next: the next action from the facts

## Description

Source: `docs/PHASES.md`, plan item 1.1 (`docs/decisions/0004`); the next-session note
`.stapel/scratch/next/next-session.md`.

### Frame

**Problem.** The orchestrator finds the next action of a ticket by re-reading `CLAUDE.md`, the 50 KB ticket,
`stapel status`, `stapel check --history` and the journals on every turn; the ledger of 2026-10-08 names the
orchestrator's context as the main cost. Every fact needed is already on disk or in git. `stapel next [KEY]`
computes the action from them, so the skill of item 1.3 can be one rule: run `stapel next`, do what it says,
run it again.

**Plan item:** 1.1. **Risk tags:** `interface` (a new command; its `--json` output is what the skill reads).

**Non-goals.** Writing any file or record (the review record is item 1.5); the builder brief or its path (item
1.2); the ticket lint (item 1.6); findings fates and triage (item 1.5: today no command sets a fate after the
import, so `next` reads no fate); the drift review after a GREEN step; choosing reviewers by tags; reading
`docs/PHASES.md`; running cargo.

**Promise (closed list).**

1. `stapel next [KEY] [--json]` resolves the ticket as `stapel status` does; with no `KEY` and no open
   ticket it names the action `new-ticket` with `stapel new "<title>"`. It writes nothing.
2. It reads only: `state.json` (closed, confirmations), the sections of `ticket.md`, the `Route:` line and
   the numbered step items of the Plan, the first-parent history `base..HEAD` (step commits and the
   history analysis of `stapel check --history`, without cargo), the newest check record of `runs.jsonl`,
   and the `stage` of the records in `findings.jsonl`.
3. It prints the blocking facts first (each confirmation missing or stale, each history fault, a failing
   current check), then exactly one action, the first that applies of: `closed`; `write-frame` (Description
   is empty, `TODO` or has no `### Frame`); `write-section` (the first configured, not `generated` section
   that is empty or `TODO`); `spec-review` (route not `light`, no confirmation ever, no `spec-review`
   finding); `confirm` (`stapel ok KEY`); `fix-history` (the step, its outcome and commit); `fix-check`;
   `green-step` or `build-step` (the first Plan step not done, in Plan order); `run-check` (`stapel check
   KEY`, when `[check]` is set and the check is not a pass at HEAD); `code-review` (Review is `TODO`);
   `write-summary`; `close` (`stapel close KEY`, by the person).
4. A Plan step `N.` or `Nx.` (a letter suffix) has the label `step N` or `step Nx`. It is done when
   history has its GREEN, or a commit `<KEY> step <label> <word>: …` where the word is not `RED`
   (coverage, cleanup); with a RED and no GREEN the action is `green-step`.
5. `--json` prints one JSON object with the same action, its command or path and the facts.

Not promised: a step in history that the Plan does not name; a Plan written in another form; review rounds
after the first (no fact tells them apart until item 1.5); a review that found nothing and left the Review
section `TODO`; deliberate circumvention.

**Prior art.** `stapel status` (`crates/stapel-cli/src/status.rs`: `waiting_for`, `freshness`,
`checkstate::current`); `stapel check --history` (`crates/stapel-cli/src/check.rs`, `outcomes::analyse`,
`history_outcome`); `steps::parse_subject`; `tickets::resolve`. klc has no next-action command.

**Size.** About 450 changed source lines (`crates/stapel-core`: a `next` module with the rules and the Plan
parse; `crates/stapel-cli`: the command, `main.rs`) and about 1 500 test lines, from STP-19's actual 524 and
2 000 against its estimate of 350. Past about 650 source lines the ticket is split before the spec review.
Route: full (tag `interface`), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-19: Decision (review facts, not recommended): a record "review round N done" is wanted, but it
  comes with item 1.5 (`stapel review start`); in this ticket `next` reads indirect facts: `spec-review`
  findings or an earlier confirmation for the spec review, the Review section for the code review. Rejected:
  widening item 1.1 by a ledger decision; moving item 1.5 forward.
- 2026-10-19: Decision (route and drift, recommended): `next` reads the Plan's `Route:` line and names no
  spec review on `light`; the drift review after a step stays the orchestrator's rule, since a clean drift
  review leaves no fact.
- 2026-10-19: Decision (no ticket, recommended): with no open ticket `next` names `stapel new "<title>"` and
  does not read `docs/PHASES.md`, which a foreign repository does not have.
- 2026-10-19: Decision (history, recommended): `next` runs the history analysis of `check --history`
  itself: git only, no cargo, no record in `runs.jsonl`.
- 2026-10-19: Assumption: findings fates stay out (no command sets a fate after the import, so a HIGH
  finding without a fate could block `next` forever); item 1.5 adds them to `next`.

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
