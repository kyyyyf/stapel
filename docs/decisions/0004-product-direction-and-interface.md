# 0004 — Product direction and the interface

Date: 2026-10-19. Proposed by the orchestrator after the human stated the product goals in the chat and asked
for an assessment, a re-planned phase 1 and the tickets. Status: **confirmed by the human in the chat on
2026-10-19** with these answers: the founder moves to the second stage as a configurable autonomy level; the
goals stay as written and target technical people only for now; one confirmation dialog per ticket; the
pilot's form and the ticket numbering are the orchestrator's decisions (§5 records them); Zed and a couple of
other languages are the stage after phase 4. The interface decision (§3 item 3) was taken earlier the same day.

## 1. The goals as stated

- A tool and a process for companies: founders may build software themselves with agents without building a
  shack; small and medium companies, or one department, build software they could not afford before (hiring
  or outsourcing). Not for people who check hypotheses fast: an agent alone is enough for them.
- Pain 1: a task that would make a department work better never starts, because there is no time and no
  people. Pain 2: software written by chatting with an agent is an MVP, not an extensible and maintainable
  system within its non-functional requirements, because the agent reads a task its own way and knows
  nothing of what the organisation keeps in people's heads.
- Money: free during verification, then a subscription for a period that includes consultations.
- Left empty: why not an agent alone; what competitors have; what is not done at the start; the signal that
  the direction is right.

## 2. Assessment

What is strong:

1. **An explicit non-target.** "Not for fast hypothesis checks" is a real non-goal; it says what stapel may
   be slow at. Every later feature can be tested against it.
2. **Pain 2 is exact and is what stapel already does.** Every phase-0 principle (agreements as tests, checks
   by the program, state as facts, people make only decisions, tokens measured) is an answer to "the agent
   optimises one task and nobody checks the sum". The eleven closed tickets measure this: code review found
   30 to 38 findings per ticket before the spec process and 1 to 4 inside-promise MEDIUM after it.
3. **Honest pricing.** Free for verification, then a subscription with consultations: a process needs a
   person to install it; the goals say so instead of promising self-service on day one.

What is weak:

1. **Two segments pull apart.** A founder who codes alone wants speed and few questions and reads no tests.
   A department with one technical person reads tests, runs CI and reviews an MR. The phase-0 process (RED
   and GREEN commits, criteria that name tests, a 50 KB ticket) serves the second. The decision of
   2026-10-19 (first segment: one technical person) belongs in the goals; the founder is the second segment
   and needs the human page of the two-level spec first.
2. **Pain 1 is not stapel's.** "No time to start" is removed by any agent; stapel adds steps at the start.
   What stapel removes is "no one to keep it alive": the second change and the tenth cost the same, the
   result is checked the same way, and the cost is known. The claim should be "stays changeable", not
   "starts fast".
3. **The empty sections are the product.** "Why not an agent alone" is the whole pitch. Proposal, from the
   measurements of phase 0: an agent gives one answer; stapel gives (a) checks the program runs and the agent
   cannot weaken (the guard, `stapel check`, the ticket lint), (b) a person who makes only decisions, each
   one a dialog with a diff, (c) a cost and a defect count per ticket. Competitors (Spec Kit, Kiro, OpenSpec,
   BMAD, superpowers; studied in decisions 0001 to 0003) are prompt and template packs that the agent itself
   runs; none checks RED to GREEN from history, guards writes, journals tokens or keeps state as facts. Kiro
   is an IDE with metered credits; the others leave enforcement to the agent. The nearest thing is klc, the
   human's own earlier tool.
4. **"What the organisation keeps in people's heads" has no artifact.** A ticket's Decisions log keeps
   answers for one ticket; `CLAUDE.md` and the two catalogues under `.stapel/config/` keep process knowledge.
   Domain terms, conventions, non-functional budgets and architecture rules of the product being built have
   no place yet. Pain 2 names them; phase 3 adds them (a project context document and machine guards).
5. **No signal of success.** Proposal: (a) three tickets merged on a foreign repository with the person
   answering only questions and dialogs; (b) escaped defects per ticket on that repository, counted four
   weeks after each merge; (c) a person who did not build stapel takes a ticket from `stapel init` to an MR
   in one working day without the author in the chat; (d) the pilot team runs its fifth ticket without the
   author present; (e) tokens per merged ticket inside a stated budget.
6. **"Not done at the start" is empty, so the plan grows.** Proposal: no hosted service, no web UI, no Zed,
   no multi-user locks, no Jira sync, no deploy or operations after the merge, no agent other than Claude
   Code, no language beyond TypeScript and Rust, no semantic index, no founder without a technical person.
7. **Platform risk is unnamed.** The product runs inside Claude Code: hooks, the permission dialog, custom
   agents, transcripts. The hooks and agents are documented and stable; the transcript format is documented
   as internal and may change, and `stapel tokens import` reads it today. The risk is accepted; the defence
   is the `estimate` mark and a second source of usage (the `--output-format json` result of headless runs).

## 3. Decisions (proposed)

1. **First segment:** a company or department with one technical person, who works in Claude Code. The
   founder who codes alone is the second stage, served by a configurable **autonomy level** (phase 3): how
   much the orchestrator does alone with an idea, from every question and two dialogs down to one page and
   one dialog.
2. **The claim:** "stays changeable at a known cost", not "starts fast". The measure is escaped defects
   per ticket and tokens per ticket (`docs/PLAN.md` §8).
3. **The interface (decided 2026-10-19):** the person works only in the Claude Code chat: "run ticket X" or
   "I want a CSV export". The orchestrator (the Claude Code main agent, with the stapel skill) asks the
   questions, writes the ticket, drives builders and reviewers as subagents, and confirms with the person
   through the permission dialog. A person never types a stapel command. The command line is the backbone the
   agent calls: state, the next action, briefs, checks, the guard, journals, the MR. Zed is optional and
   waits for a pilot person who asks for it; every button would call one command, so nothing is blocked.
4. **Phase 1 re-planned** as the backbone for that interface (`docs/PHASES.md`): `stapel next`, briefs,
   the process installed as data by `init` (skill, agent definitions), tokens from hooks, the review copy,
   the ticket lint, `ship` with the MR review map, one confirmation dialog that shows the human page, the
   junit runner. Phase 2 is the pilot on a foreign TypeScript repository, moved forward from phase 4. Phase
   3 is the project context, machine guards and the autonomy levels (pain 2). Phase 4 is the index. Phase 5
   is Zed and more languages.
5. **One confirmation dialog per ticket.** `stapel ok KEY` confirms every section at once and shows the
   human page (Frame, criteria, Decisions lines) with its diff; the other sections appear as one line each.
   The push to the forge is the second and last dialog, because it leaves the machine and the pre-push hook
   refuses pushes from an agent's shell. Findings are recorded without a dialog; an `intent_gap` is a
   question in the chat.
6. **The empty sections** of the goals are filled with the proposals of §2 items 3, 5 and 6; the human
   keeps the goals text as written and targets technical people only for now.
7. **Agents do not deviate from the plan.** `docs/PHASES.md` names items in order (`1.1`, `2.3`); a ticket's
   Frame names its item and the promise stays inside it; an item changes only by a dated ledger decision
   made with the human; `stapel lint` (item 1.6) checks that the Frame names an existing item. The rule is in
   `CLAUDE.md`, "The plan".

## 4. What the person sees in one ticket

1. The person writes what they want, or names a ticket. The orchestrator creates the ticket and investigates
   the code.
2. The orchestrator asks the frame questions and the coverage-map questions, one or a few at a time, each
   with options and a recommendation (the `AskUserQuestion` tool; the main agent has it, subagents do not).
3. The person confirms one page: the Frame and the criteria, with the diff since the last confirmation, in
   one permission dialog, the only confirmation of the ticket. Design, Proof and Plan are confirmed by the
   same dialog and shown as one line each unless the person opens them.
4. Builders, the drift reviewer, the code reviewers and the checks run without the person. The person is
   asked only for an `intent_gap` (something the request left out) and for a finding the orchestrator could
   not settle.
5. `stapel ship` pushes and opens the MR through one dialog. The MR body carries the review map: where a
   human reviewer should look (files the plan did not name, the Review Focus items, deferred findings,
   imprecise places), the test evidence and the token cost.
6. After the merge the ticket is closed; the summary holds time, tokens by role, findings and escaped
   defects found later.

Autonomy levels (how many of these moments ask the person) are configured in phase 3; phase 1 builds the
level above, which is the one the first segment needs.

## 5. The questions and their answers (2026-10-19)

1. The founder segment: the second stage, as a configurable autonomy level (the human).
2. Pain 1 wording: the goals stay as written; the claim of §3 item 2 is the orchestrator's reading, not a
   change to the goals (the human: the goals are written; technical people only for now).
3. The pilot: decided by the orchestrator on the human's request; the form is in `docs/PHASES.md`, phase 2:
   after phase 1 (which ends with the junit runner), three tickets one after another on the TypeScript
   project next to this one, the first one also through klc from the same base commit, the human as the
   one technical person, the author of stapel out of the chat, escaped defects counted four weeks after
   each merge.
4. Confirmation: one dialog per ticket (the human: "no sense in pressing several times").
5. Findings: recorded without a dialog; the person sees `intent_gap` questions in the chat and the review
   map in the MR (follows from answer 4).
6. Ticket state: on the ticket's branch next to the code; a state branch stays an option (orchestrator).
7. Numbering: no new code. A ticket is planned as a plan item (`1.1`) and gets its `STP-<n>` key from
   `stapel new` when it is taken; the plan entry is then updated with the key; the keys STP-7 to STP-12,
   STP-15 and STP-20 to STP-22 in the phase-0 list are labels (orchestrator).
