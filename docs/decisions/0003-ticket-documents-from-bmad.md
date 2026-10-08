# 0003 — Ticket documents: what stapel takes from BMAD

Date: 2026-10-08. Proposed by the orchestrator after a study of BMAD-METHOD's `skills/bmad-spec`, `bmad-prd`,
`bmad-architecture`, `bmad-ticket` (templates and references), `bmad-project-context` and the plan template of
`bmad-build` (main branch, about 27 fetches; the fetch tool returns summaries, so fine print may be missing).
The human asked to record what to take and the plan; epics and features are discussed later.

## Findings

- BMAD separates readers. The human reviews short documents: a PRD of one or two pages for small work, an
  epic's outcome and "Done when" (three to six checks), and the frozen intent of the build plan. Agents read
  machine-facing documents: `bmad-spec` is a "canonical contract" derived from a decision log, and a story is
  deliberately short (one sentence, one `Verify:` line, a Boundaries line, references).
- The rule is "the reader has only the ticket": a builder can write the acceptance criteria without further
  information, and a story fits one agent session.
- No 300-token intent or 1500-token story limit was found in the current version. The nearest limits are the
  build plan's 900–1300 token target and its 1600-token split point; the intent is one or two sentences.
- "What not to do" exists in narrow forms: a story's "Must not change: adjacent behaviour", the spec's non-goals,
  the plan's Never list.
- Ticket types initiative, epic, story, bug and spike have templates; a bug has a reproduction and a cause
  hypothesis, never a fix; a spike is a question with a time box. Status lives in a plan file, not in the ticket.

## Decisions

stapel keeps one `ticket.md` per ticket, but serves two readers from it:

| Reader | Part | Limit |
|---|---|---|
| Human | Frame (problem, risk tags, non-goals, closed promise, size) | about 300 tokens |
| Human | Spec criteria (WHEN/IF, each naming a test) | about 1600 tokens (STP-9) |
| Human | Decisions log with typed lines: Decision, Assumption, Open question | one line each |
| Builder | A brief per step, assembled from the Frame, the step's criteria, its Inputs and External states rows, its "must not change" line and the step itself | 900–1300 tokens, 1600 at most |
| Reviewers | The whole ticket | none |

Take, in this order:

1. **A brief per step for the builder** (900–1300 tokens, at most 1600), assembled from the ticket, not written
   by hand. First by hand in the builder pilot (decision 0002) on the remaining pair of STP-5; then as a
   command.
2. **A "must not change" line per plan step**, naming adjacent behaviour, not files; next to the Frame's
   non-goals.
3. **Typed lines in the Decisions log** (Decision, Assumption, Open question), so the human sees what is not
   yet confirmed.
4. **A validation checklist in the drift test:** references exist, no placeholders, no contradiction with the
   Decisions log; the Frame within about 300 tokens.
5. **Stable ids for facts** (`F-3`) so a brief cites a fact by id instead of copying it.
6. **Ticket types `bug` and `spike`** with their own templates, and **status outside the confirmed text**, so a
   status change never looks like a spec change.

Not taken now: epics and the initiative hierarchy, a board, tracker sync, a decision log from which the spec is
derived by a model (`.memlog.md`), `AGENTS.md` (CLAUDE.md does this).

## Open question for the human (to discuss)

The human's direction: a spec for a feature or an epic is written for a human; a ticket's spec is limited in
size and written for an agent. Decide later: whether stapel gets a feature/epic level with its own human-facing
spec and "Done when", and how tickets point to it (BMAD: `covers` ids and references by id, not copies).
