# STP-21 — stapel brief: the builder brief of one step

## Description

Source: `docs/PHASES.md`, plan item 1.2 (`docs/decisions/0003-ticket-documents-from-bmad.md`, label STP-10);
the ledger decision of 2026-10-20 that changes the item.

### Frame

**Problem.** Today the orchestrator writes each builder brief by hand from the 50 KB ticket, and a builder
that gets only the ticket path reads all of it. `stapel brief KEY STEP` assembles the brief of one Plan step
from the ticket, so neither reads more than the step needs.

**Plan item:** 1.2.

**Risk tags:** `interface` (a new command whose output a builder reads; `stapel next` names it).

**Non-goals.** Fact ids `F-n` (decision 0003, item 5); the general working rules of a builder (item 1.3,
the agent definition); a test command; Design text; checking confirmations (`next` does); the ticket lint
(item 1.6); writing any file.

**Promise (closed list).**

1. `stapel brief KEY STEP` (`STEP` as `2` or `2c`) prints on stdout, in this order: a header (key, step
   label, base from `state.json`, ticket path, the test names of the step's criteria); the Frame without its
   `**Prior art.**` and `**Size.**` paragraphs; the step item of the Plan; the Spec criteria the step names
   (`AC-n`, `AC-n to AC-m`); the Spec terms those criteria use, with the terms those terms use; the rows of
   the Inputs and External states tables whose Test cell names a test of those criteria; the records of
   `findings.jsonl` whose id the step names (id, severity, claim, fate). It writes nothing.
2. It refuses with exit 1 when the step is not in the Plan, a criterion or finding the step names does not
   exist, or the brief exceeds 1600 tokens estimated as bytes / 4 (it names each part's size).
3. `stapel next` names `stapel brief KEY STEP` for `build-step` and `green-step` when the Plan has the step.

Not promised: steps or tables written in another form; a finding id inside a criterion; a brief between
900 and 1300 tokens (a target, not checked); deliberate circumvention.

**Prior art.** The hand briefs of STP-5 to STP-20 (`.stapel/scratch/stp5-r1-brief.md`); `next::plan_steps`,
`ticket::find`, `tickets::resolve`; BMAD's build plan (900–1300 tokens, split at 1600).

**Size.** About 400 changed source lines (`crates/stapel-core`: a `brief` module; `crates/stapel-cli`: the
command, `main.rs`, the `next` target) and about 1 200 test lines. Past about 600 source lines the ticket is
split before the spec review. Route: full (tag `interface`), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-20: Decision (output, recommended): the brief goes to stdout and the command writes nothing; `next`
  names the command, the builder runs it, the orchestrator does not read the brief. Rejected: a file under
  the ticket with its path (two kinds of target in `next`). Recorded in the ledger as a change of item 1.2.
- 2026-10-20: Decision (size, recommended): over 1600 estimated tokens the command refuses with exit 1 and
  names the size of each part; the author splits the step or shortens the Frame. Rejected: print with a
  warning, which a careless agent skips.
- 2026-10-20: Decision (Frame, recommended): the Frame enters without the Prior art and Size paragraphs.
- 2026-10-20: Decision (contents, recommended): the brief also carries the Spec terms its criteria use, a
  header (key, step, base, ticket path, test names) that replaces the three things CLAUDE.md gives a builder,
  and the findings the step names, because coverage and review steps name no criterion. Not taken: Design
  decisions, the general working rules (item 1.3), a test command (the Test plan line is free text).

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
