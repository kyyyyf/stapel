# 0001 — The spec process after STP-4

Date: 2026-10-05. Decided by the human in the chat; written down by the orchestrator. The rules themselves are in
`CLAUDE.md`, "The spec process"; this note keeps the reasons and the sources.

## Context

From STP-1 to STP-4 the "Before the build" rules grew by one or two items per ticket, and none was ever removed.
STP-4's ticket reached 54 KB; 35 spec-review findings and 54 per-step drift findings came before code, yet code
review still found 38. Code review found 30 to 38 findings on every ticket from STP-2 to STP-4, whatever the
spec effort, because each reviewer was allowed up to 15 and used the quota. The measure "share of code-review
findings catchable before code" (60 % on STP-3, 84 % on STP-4, 81 % without the deliberate-circumvention
findings) therefore measured the reviewers' quotas more than the quality of the spec. Tokens were spent mostly
by the orchestrator (280k output, 82M cache read on STP-4), not by the reviewers.

## Goals

In this order of trade-off: (1) complete enough, (2) simple enough that the human reads what they confirm,
(3) defects caught early, (4) tokens spent where they catch something.

## Decisions

1. **Rethink the whole spec process, not only the narrow fix.** The six narrow fixes found after STP-4 (an
   external-states table, the promise as a closed list with a non-goal, the review order, a coverage map with
   multiple-choice questions, a Review Focus line, mechanical checks of criteria) are folded into the process
   instead of being added as six more rules.
2. **The process** is the nine items of `CLAUDE.md`: frame first; questions before text; short criteria with an
   Inputs and an External states table filled from catalogues; one spec review in a fixed order; the human
   confirms a summary; Design "as built" written once before code review; code review without quotas; measures
   after the close; rules pruned.
3. **Main measure: escaped defects** (`docs/PLAN.md` §8), with HIGH and MEDIUM inside-promise code-review
   findings, the size of `ticket.md`, tokens and time beside it.
4. **Threat model:** a careless agent, not a malicious one. Findings are tagged by the promise: inside (a
   defect) or outside (a proposal to widen it, not counted). This replaces the "adversarial" mark tried for a
   day, because an agent's intent cannot be tested, while an input or a construction can.
5. **Rules are pruned:** each code-review finding names the rule that should have caught it; a rule that
   catches nothing for three tickets is removed or simplified (`.stapel/process-ledger.jsonl`).
6. **One confirmation for several sections** becomes an option in `stapel.toml` (STP-7), for repositories where
   one person owns every section; the default stays per section.
7. **Hardening of `stapel check` against deliberate circumvention** is a ticket of its own (STP-6), outside
   the default threat model.
8. **STP-5 is the pilot**; it is compared with STP-3 and STP-4 by the measures of decision 3.

## Sources and what was taken

From a study of five tools (2026-10-05; links verified by reading the source unless marked):

| Tool | Taken | Not taken, and why |
|---|---|---|
| superpowers (github.com/obra/superpowers) | Review Focus: the input classes a spec implies but no test exercises; "an empty section means checked" | its spec reviewer is lenient by design ("approve unless serious") |
| Spec Kit (github.com/github/spec-kit) | the ten-category coverage map with Clear/Partial/Missing; questions with options and a recommendation; a dated Clarifications log | its volume: one measured report (Scott Logic, 2025, secondary) found ten times the time and still a missed bug |
| OpenSpec (github.com/Fission-AI/OpenSpec) | one behaviour per requirement, about 500 characters; open questions only where they change nothing | living capability specs: a large redesign for now |
| Kiro (kiro.dev) | EARS form (`WHEN … THE … SHALL`, `IF … THEN`); later, if needed, an ambiguity probe by comparing several restatements of a criterion | SMT formalisation |
| BMAD (github.com/bmad-code-org/BMAD-METHOD) | "a dimension left silent is a finding"; at least one non-goal; the reviewer reads the author's narrative only after its own pass | the role ceremony; the "at least ten findings" quota |

Also: requirement smells (Femmer et al., 2017) as a later word-list check with waivers.

## First result (STP-5, in progress)

The ordered spec review produced all eight of its HIGH and MEDIUM round-1 findings in its own first pass, before
reading the author's tables. Code review still found 17 findings, several of them breaches of the new rules by
the author (criteria over 500 characters, prose instead of the External states table, a frame outgrown without
a decision); the drift reviewer caught them. The pilot's measures go into STP-5's summary.
