# 0002 — Build and review: what stapel takes from BMAD

Date: 2026-10-07. Proposed by the orchestrator after a targeted study of BMAD-METHOD's `skills/bmad-build` and
`skills/bmad-code-review` (main branch; about 22 fetches; the fetch tool returns summaries, so quoted wording and
numbers may differ slightly from the source). The human asked to add the proposals to the plan.
Background: `docs/20260107_klc_bmad_comparation.md`, `docs/decisions/0001-spec-process.md`.

## Context

stapel already has the deterministic part that BMAD lacks: `stapel check` runs each step's tests at its RED
commit, the drift test, the token journal, a model per role. What it lacks is the organisation of the build
and the review: the orchestrator writes all code in one long session (STP-4: 280k output, 82M cache read
tokens); three reviewers run on every ticket; findings have only the fates `fix` and `accepted`, without a
verdict or evidence (STP-5's step-1 finding D1-1 was false, and only the orchestrator's check showed it).

## Decisions

### Process text now (`CLAUDE.md`, applied by the human)

1. **Review by risk.** An untagged ticket gets the drift test and one fresh reviewer; a ticket with risk tags
   gets the three reviewers. The ticket records the reviewers that ran and the review round.
2. **Three review tasks for the existing reviewers** (BMAD's edge-case and verification-gap lenses):
   deletion check (what did removed code guarantee, and what replaces or retires it); claims check (try to
   falsify each checkable claim of the criteria and of Design "as built" against the code); verification gap
   (for each behaviour change, would an existing test fail if the change were wrong). `stapel check` covers the
   new tests of a step; the verification gap covers regressions in code the step did not test.
3. **A test that did not run counts as missing**, for reviewers and the builder.
4. **The builder stops when the request leaves out something the human would notice**; that becomes an
   `intent_gap`, not a guess.
5. **Route after the investigation**, written in the ticket with its source (`auto` or pinned by the human):
   light for about 100 changed lines or fewer, mechanical work and no risk tags (no spec review, no builder,
   one reviewer; RED/GREEN and `stapel check` stay); full otherwise.
6. **Builder subagent per step, tried by hand first.** It receives only the ticket path, the step and the
   base commit, and returns a fixed report: files changed, the test command and its result, what is left. The
   orchestrator coordinates and records the builder's tokens. Its effect is measured against the
   orchestrator's tokens of STP-4 and STP-5.

### Tickets (`docs/PHASES.md`)

7. **Triage of findings (STP-8):** a verdict per finding (`high`, `medium`, `low`, `false` with a refutation,
   `maybe-false` with what would settle it) and its evidence; grouping by root cause; the fates `patch`
   (the builder fixes, tests run again), `bad_plan` (revert the step, amend the plan, rebuild), `intent_gap`
   (the human decides, a spec change) and `defer` (kept with its evidence); at most two review rounds, then
   the human decides. `bad_plan` maps to `catchable_at: design`, `intent_gap` to `spec`.
8. **Size limit of the Spec section (STP-9):** about 1600 tokens, checked by `ticket_drift.rs`; beyond it the
   ticket is split (BMAD's plan limit of 900–1600 tokens; STP-4 reached 54 KB).
9. **The builder in code (phase 1):** a builder role in `stapel-agent` and `stapel build`.

## Not taken

- BMAD's skill rendering (`render_skill.py`, `customize.toml` layers): stapel has `stapel.toml`.
- The Blind Hunter's quota of findings: stapel removed review quotas (decision 0001).
- Review on the live working tree: the `git archive` copy without write access stays; a diff file may be
  given as a second input.
- A light route with no review at all: even the light route keeps the drift test and one reviewer.
- A free-text executor report: stapel needs a fixed report for the journal.
- One commit at the end: RED/GREEN pairs and `stapel check` stay.

## Still to study

How BMAD writes specs and work plans (`skills/bmad-spec`, its PRD and architecture validation checklists, the
plan template of `skills/bmad-build`), before STP-9. Decision 0001 already took from `bmad-spec` the silent
dimension rule, the non-goal rule and the review order.
