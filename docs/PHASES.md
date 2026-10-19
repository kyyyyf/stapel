# stapel — phase plan

Every phase is described the same way: goal, what is included, acceptance criteria, exit criterion, how the
work is run. The acceptance criteria here are deliberately short; the full criteria and tests will appear in
the `ticket.md` of each ticket. Time estimates are for a single maintainer who runs the work through agents.

## How the work is run in each phase

| Phase | Who is the orchestrator | Process |
|---|---|---|
| 0 | Claude Code in an interactive session | the same process we are building, but carried out by hand: a `ticket.md` per ticket, RED→GREEN commits, three reviewer agents on a `git archive` of the current commit, findings in a single format, a token journal from the agents' reports |
| 1 onward | Claude Code in an interactive session, following `stapel next` | the same process, installed by `stapel init` as a skill and agent definitions; `stapel` computes the next action, briefs, packs, checks and the MR; the person answers questions and dialogs only (`docs/decisions/0004`) |

Rules in all phases: agents do not run `git push`; reviewers work only on a copy and without write access to
git; no private terms in tracked files and commit messages; for every acceptance criterion there is a test
that fails first.

---

## Phase 0 — bootstrap (about a week)

**Closed on 2026-10-19 with STP-19** (eleven tickets: STP-1 to STP-6, STP-13, STP-14, STP-16 to STP-19). The tickets listed below and not closed move to phases 1 to 3 as noted at each entry; see "How tickets are named from phase 1 on".

**Goal.** A minimal core with which a ticket can be run by hand, but already by the tool's rules.

**What is included.**

- A cargo workspace: `stapel-core`, `stapel-cli`; the other packages are empty stubs.
- `stapel init`: creates `.stapel/`, `stapel.toml` with starting sections, roles and models, installs the
  Claude Code hooks (a ban on `git push` for agents, a ban on writing to code before the build permit).
- `stapel new`: a `ticket.md` with sections and a `state.json`; the ticket key by the rule from
  `stapel.toml` (`STP-<n>`), a field for a link to an external tracker.
- Section hashes and confirmations: `stapel ok <section>` writes the fact "who, when, hash"; a change in a
  section makes the dependent confirmations stale; `stapel status` shows this.
- The decision log `decisions.jsonl`: answers to questions, choice of an option, confirmations, returns with
  a reason.
- The RED→GREEN check: from the git history and the test command in the plan, the tool confirms that the
  step's test failed before the change and passes after it, on the final commit.
- Accepting findings: `stapel review take --role <role> --file <json>` with validation of the single
  format, a record in `findings.jsonl`, a refusal that keeps the raw answer.
- The token journal `tokens.jsonl` and the `stapel tokens` command: records marked "measured" or
  "estimate".
- MR through `gh` or `glab`: `stapel mr open` as a draft, the link in `state.json`.

**Added during STP-2.** `stapel close` writes a `closed` fact (the phase-0 stand-in for `ship`). People never type
`ok` or `close`: in Claude Code the guard answers `stapel ok` with the permission dialog and a one-time grant;
outside an agent's shell (a terminal, a Zed task) the commands run directly. `status` prints
`waiting for: <section> (owner: <owner>)`. Confirmations are kept in `state.json`; `decisions.jsonl` comes with STP-3.

**Added during STP-3.** `decisions.jsonl` holds confirmations and closings, written by `stapel ok` and `stapel
close`; answers to questions stay in the ticket text, bound by the spec confirmation's hash, and `stapel decide`
comes with `ask` in phase 1. Token records come from `stapel tokens add` or from importing Claude Code transcripts.

**Interim comparison with klc (added during STP-4).** Before phase 4, `stapel` and klc each take the same
next ticket from the same branch of a TypeScript project that klc already runs, and the results are compared
by the metrics of `docs/PLAN.md` §8. It starts only with the human's explicit consent. STP-4 adds the `[check] runner`
key with `cargo` as its only value; another runner is a later ticket and a precondition of the comparison.

**STP-3 baseline for the comparison.** Spec review: 32 findings over two rounds. Code review: 30 findings,
18 of them (60%) `catchable_at` earlier than `code` (target: under half; not met). The 18 fall into three
groups: ticket text behind the code (9), input bounds (6), meaning of external data and identity (3).
Measured tokens (output / cache read): orchestrator 151k / 76M, research 1.5k / 4.8M, external 1.6k / 2.9M,
reviewer 0.2k / 1.2M, drift 0.5k / 1.0M.

**Acceptance criteria (abridged).**

1. `stapel init` in an empty repository creates `.stapel/` and the hooks; running it again changes nothing.
2. `stapel new` creates a ticket; `stapel status` on it shows "waiting: spec".
3. After `stapel ok spec` and an edit of the spec, `status` shows "spec confirmation is stale" and, as a
   diff, what changed.
4. For a plan step with a test command, the tool distinguishes: the test did not fail before the change;
   failed and now passes; does not pass. The check runs on the final commit.
5. A reviewer's answer in the wrong format is rejected, the raw text is saved, the reason is named.
6. Every model call recorded through `stapel tokens add` is visible in `stapel tokens` with its role and
   mark.
7. The whole binary builds with one command and works in someone else's repository.

**Exit criterion.** One phase 0 ticket is taken through these commands entirely, with three reviewers and a
token record, even if some steps were still performed by the orchestrator by hand.

**First tickets.**

- STP-1 workspace, `init`, `stapel.toml`, hooks.
- STP-2 `new`, `ticket.md`, `state.json`, hashes and confirmations, `status`.
- STP-3 decision log and token journal.
- STP-4 RED→GREEN check on the final commit. Its first steps extend the drift check (CLAUDE.md items 5
  and 9: Inputs table, abuse table and self-check, matching test lists); `Cargo.lock` is checked by `--locked`
  in every run, and item 11 stays a manual review. After its close it is
  compared with STP-3: tokens by role, and findings by stage and `catchable_at`.
- STP-5 measured token usage of subagents (their transcripts keep only the first streamed chunk of a
  message; found at the close of STP-4).
- STP-6 hardening of `stapel check` against deliberate circumvention (environment, cargo configuration,
  test targets; the adversarial findings of STP-4's code review).
- STP-7 one confirmation for several sections, as an option in `stapel.toml` for repositories where one
  person owns every section. Done by STP-17 (`stapel ok KEY`).
- STP-8 (item 1.5) accepting findings and `findings.jsonl`, with the triage of `docs/decisions/0002`: a verdict
  (`high`, `medium`, `low`, `false` with a refutation, `maybe-false` with what settles it) and evidence per
  finding, findings grouped by root cause, the fates `patch`, `bad_plan`, `intent_gap` and `defer`, at most
  two review rounds before the human decides; each finding names the process rule that should have caught it,
  and `.stapel/process-ledger.jsonl` counts the catches per rule (`CLAUDE.md`, spec process items 7 and 9).
- STP-9 (folded into item 1.6) mechanical spec checks in `ticket_drift.rs` (`docs/decisions/0001`, the six narrow fixes, and
  `docs/decisions/0002`): one behaviour per criterion, at most about 500 characters; the Spec section within
  about 1600 tokens, else the ticket is split; an External states table whenever the ticket has an External
  contract, every row handled, refused or not promised; a Review Focus line; for every Inputs and External
  states row an `IF … THEN` criterion or a golden file; a word list of vague terms with an inline waiver; a
  frame with at least one non-goal and a closed promise; the Frame within about 300 tokens; typed lines in the
  Decisions log (Decision, Assumption, Open question); references that exist, no placeholders, no
  contradiction with the Decisions log (`docs/decisions/0003`). Before it: a targeted study of how BMAD writes specs
  and work plans (`skills/bmad-spec`, the PRD and architecture validation checklists, the plan template of
  `skills/bmad-build`), on top of what `docs/decisions/0001` and `0002` already took.
- STP-10 (item 1.2) the builder brief (`docs/decisions/0003`): a command that assembles, for one plan step, the Frame,
  the step's criteria, its Inputs and External states rows, its "must not change" line and the step itself,
  in 900–1300 tokens and at most 1600; stable fact ids (`F-n`) cited by id. Tried by hand first, in the builder
  pilot on the remaining pair of STP-5.
- STP-11 (item 3.6) ticket types `bug` (reproduction, cause hypothesis, no fix) and `spike` (a question with a time box),
  with templates; ticket status outside the confirmed text.
- STP-12 MR through `gh`/`glab`. Folded into item 1.7.
- STP-13 (split from STP-6, taken right after it) hardening of `stapel check`, part two: dependency
  redirection (`path`, `git`, `package`, registries, `[patch]`, `[replace]`), `[features]`, `[profile]`,
  `[lints]`, `edition`, dev-dependencies added after a RED, non-`.rs` files under `crates/*/tests/` after a
  RED; in a RED: a dev-dependency redirect, a `Cargo.toml`, link, `.cargo` path or `rust-toolchain` file under
  `crates/*/tests/`, a file named by `build` or a target `path`.
- STP-14 (after STP-13) `stapel check --history`: the outcomes that need no test run (`unpaired`, `duplicate`,
  `no-tests`, `red-changes-code`, `tests-changed`, `build-input-changed`) from git history alone, in seconds,
  with no record in `runs.jsonl`; run after every builder commit, later from a hook, so a builder's mistake is
  fixed by amending its last commit instead of rewriting a chain (found during STP-6).
- STP-15 (split from STP-13; after the pilot, when a catch demands it) hardening of `stapel check`, part three: `include*!` with a
  non-literal argument or a path outside the repository, includes followed through another included file, a
  later RED that rewrites a helper or included file an earlier step relies on, a config planted inside the
  check's worktree between runs, a run-time read of a file outside `crates/*/tests/`.
- STP-16 (found after the close of STP-13, finding X-1; taken before STP-14) a closed output pipe: `stapel
  status STP-13 | head -4` panics with "failed printing to stdout: Broken pipe" and exits 101 (an escaped
  defect of STP-2 step 7); every command of the `stapel` binary that prints is checked.
- STP-17 (asked by the human during STP-16; after STP-16) fewer confirmation commands: `stapel ok KEY` with no
  section confirms every stale section and prints their list first; `stapel ok` shows the diff of a section
  since its last confirmation, not the whole text. A spec change after a review round is made as one change, so
  one round needs one confirmation.
- STP-18 (asked by the human at the close of STP-14; next, before STP-15) machine records through `stapel`:
  `stapel findings import FILE` appends review findings to a ticket's `findings.jsonl` (fields checked, a
  repeated id refused) and `stapel ledger add` appends a record to `.stapel/process-ledger.jsonl`; an agent
  runs them and the guard shows the permission dialog with the records, as for `stapel ok`. In STP-14 the
  human typed nine `cat … >> findings.jsonl` commands and one for the ledger.
- STP-19 (split from STP-18 at its frame; after STP-18) token journal records imported from a file: `stapel
  tokens import-records FILE [KEY]` appends checked records to a ticket's `tokens.jsonl` through the same
  permission dialog; every imported record (`measured` or `estimate`) gets the field `imported_from`, and the
  token report shows such records apart as `<role> (file)`.
- STP-20 (item 1.9; asked by the human at the close of STP-18) the guard denies Bash commands that write into
  `crates/` (`>`, `>>`, `tee`, `sed -i`, `perl -i`, a heredoc into a file): seven of eleven builders of STP-14
  and STP-18 wrote repository files through Bash although their briefs named Edit and Write only.
- STP-21 (item 1.10; asked by the human at the close of STP-18) the guard denies a compound command, a pipe or a
  redirection that holds `stapel ok`, `close`, `findings import`, `ledger add` or `tokens import-records`,
  with "run it as one plain
  command"; today it allows the call and the command refuses in the agent's shell without a dialog (seen at
  the frame of STP-18 and three times in STP-14).
- STP-22 (item 1.11; asked by the human at the close of STP-18; finding E-3 of STP-14) `stapel check` and the commit hook
  walk only `base..HEAD` of the ticket, so the hook stays fast on a long first-parent history.

**To discuss with the human:** a feature or epic level with its own human-facing spec and "Done when", with
tickets limited in size and written for an agent (`docs/decisions/0003`, open question).

**Process changes without a ticket** (`docs/decisions/0002`, text in `CLAUDE.md`): review by risk (an untagged
ticket gets the drift test and one fresh reviewer; a tagged one gets three); three review tasks for the
existing reviewers (deletion check, claims check, verification gap); "a test that did not run counts as
missing"; a route chosen after the investigation and written in the ticket (light: up to about 100 changed
lines, mechanical, no risk tags); a builder subagent per step, tried by hand from the next build on and
measured against the orchestrator's tokens, with a fixed report (files, test command and result, what is left)
and the stop rule "stop when the request leaves out something the human would notice"; it receives a brief of
900–1300 tokens (`docs/decisions/0003`), and every plan step gets a "must not change" line.

---

## How tickets are named from phase 1 on

A ticket is planned as a **plan item** (`1.1`, `2.3`) and gets its `STP-<n>` key only when `stapel new`
creates it; the plan entry is then updated with the key. The keys STP-7 to STP-12, STP-15 and STP-20 to
STP-22 in the phase-0 list are planning labels of the same kind (decision 0004 §5, item 7). Items are taken
in the order listed; an item is added, moved or widened only by a dated decision in
`.stapel/process-ledger.jsonl` that names the reason, and the ticket's Frame names its plan item
(`CLAUDE.md`, "The plan").

---

## Phase 1 — the backbone for the chat (about three weeks)

Re-planned on 2026-10-19 (`docs/decisions/0004`). The earlier plan made `stapel build` an orchestrator of its
own that called `claude -p`; the person would have had two orchestrators and a command line they do not use.
Now the Claude Code main agent stays the orchestrator, as in phase 0, and `stapel` is the backbone it calls:
it computes the next action, assembles briefs and review packs, checks history, guards writes, journals
tokens and opens the MR. A person types no stapel command. One ticket asks the person for one confirmation
dialog (the human page) and one push dialog; everything else is a question in the chat or runs alone.

**Goal.** A technical person in the Claude Code chat takes a `stapel` ticket from "I want X" to a merge
request while answering questions and two dialogs; the orchestrator follows `stapel next`; no machine file is
written by hand; the orchestrator's tokens per ticket fall by half against STP-19.

**What is included.**

- `stapel next`: the next action from the facts (confirmations, plan steps, git history, runs, findings),
  printed for the orchestrator with the command or the file to use. The orchestrator's skill is one rule:
  run `stapel next`, do what it says, run it again.
- The builder brief per step as a command and a review pack per round (the `git archive` copy, the diff,
  the criteria, the three review checks).
- The process as installed data: `stapel init` writes the orchestrator skill, the agent definitions
  (builder, drift reviewer, code reviewers, spec reviewer) with their tools, models and permission mode from
  `stapel.toml`, and the hooks; a new stapel version updates them. The spec process leaves `CLAUDE.md` of
  this repository and becomes the skill text.
- Tokens from hooks: `SubagentStop` records each subagent's measured usage by its agent type; the session
  tail is recorded at `Stop`. No transcript import by a person.
- The ticket lint as a command for any repository, so `stapel next` refuses to advance on an inconsistent
  ticket, and the findings triage.
- `stapel ship`: the preconditions, the push and the MR through one dialog, the review map in the MR body.
- One confirmation dialog per ticket: `stapel ok KEY` shows the human page (the Frame, the criteria and the
  Decisions lines with their diff); Design, Proof and Plan are confirmed by the same dialog and shown as one
  line each. Findings are recorded without a dialog; an `intent_gap` is a question in the chat. `close`
  goes through the dialog like `ok`.
- The guard items planned in phase 0 (labels STP-20, STP-21) and `base..HEAD` walking (label STP-22).
- The `junit` runner for `stapel check`, so phase 2 can start the day phase 1 ends.

**Acceptance criteria (abridged).**

1. A `stapel` ticket goes through `new → next … → ship` on `stapel` itself with the orchestrator running only
   what `stapel next` names; the person answers questions and two dialogs and types no command.
2. `stapel next` on a ticket with a stale confirmation, a failed check, a lint error or an open HIGH finding
   names that fact and no build action.
3. Every builder and reviewer subagent run leaves a `measured` token record with its role, written by a hook.
4. A reviewer agent cannot write to the working tree or to git; a builder agent cannot push.
5. `stapel ship` refuses while a confirmation is stale, the check is not current at HEAD, a HIGH finding has
   no fate, or a private term is found; else it opens the MR with the review map and records the link.
6. `stapel init` in an empty repository installs the skill, the agents and the hooks; running it again
   changes nothing; a hand-edited agent file survives.
7. `stapel check` on a jest step shows the test failing at RED and passing at GREEN and HEAD from the JUnit
   report.

**Exit criterion.** Three `stapel` tickets in a row are taken through `stapel next` with one confirmation
dialog each and without manual intervention in the machine files, and the orchestrator's measured cache
read per ticket is under half of STP-19 (39.5M).

**Items, in order.**

- 1.1 `stapel next [KEY]`: the next action computed from `state.json`, the Plan steps, `check --history`,
  `runs.jsonl` and `findings.jsonl`; one line of action plus the command or the path (`--json` for the
  skill); the facts that block an action named first (stale, failed, unpaired, lint). First, because it
  replaces the orchestrator re-reading `CLAUDE.md` and a 50 KB ticket on every turn (ledger 2026-10-08: the
  orchestrator's context is the main cost).
- 1.2 the builder brief as a command, `stapel brief KEY STEP`, 900 to 1 300 tokens, assembled from the
  Frame, the step's criteria, its Inputs and External states rows, its "must not change" line and the step
  (`docs/decisions/0003`; label STP-10); `next` prints its path.
- 1.3 the process as installed data: `stapel init` writes `.claude/skills/stapel/SKILL.md` (the spec
  process, today `CLAUDE.md` items 1 to 9, as the skill text), `.claude/agents/stapel-builder.md`,
  `stapel-drift.md`, `stapel-reviewer.md`, `stapel-spec-reviewer.md` (tools, `model` from `[models]`,
  `permissionMode`; reviewers read-only; builders with Edit, Write and Bash under the guard), and the hooks;
  files carry a version marker, a re-run updates stapel's files and leaves the person's. Prior art: the
  briefs and reviewer prompts of STP-5 to STP-19 under `.stapel/scratch/`.
- 1.4 tokens from hooks: a `SubagentStop` hook runs `stapel hook subagent-stop`, which reads
  `agent_transcript_path` and `agent_type` from the hook input, maps the type to a role through
  `stapel.toml`, and appends a `measured` record (the import of STP-5 and STP-19 reused); a `Stop` hook
  records the main session's tail for the open ticket with the role `orchestrator`. The transcript format is
  documented as internal: a record the parser cannot read is written as `estimate` with the reason.
- 1.5 the review pack and the triage: `stapel review start KEY --round N` makes the `git archive` copy of
  HEAD under a temporary folder, writes the pack (diff `base..HEAD`, the ticket, the criteria of the round,
  the three review checks of `docs/decisions/0002`) and names the reviewers by route; `findings import`
  runs from the orchestrator without a dialog and records the importer; the triage of `docs/decisions/0002`
  (verdict, evidence, root cause, the fates `patch`, `bad_plan`, `intent_gap`, `defer`; two rounds at most)
  (label STP-8).
- 1.6 `stapel lint [KEY]`: the checks of `crates/stapel-cli/tests/ticket_drift.rs` that any repository
  can run (a named test exists, a named path exists, a tagged ticket has its tables and self-check, the test
  lists agree, a criterion is one behaviour of at most about 500 characters, the Frame within about 300
  tokens and names its plan item, a CLI option is in an Inputs table, no placeholder, no contradiction with
  the Decisions log); test lookup by the `[check]` runner's language (label STP-9). `ticket_drift.rs` keeps
  the stapel-only checks (quoted output lines in Rust code).
- 1.7 `stapel ship KEY`: refuses on a stale confirmation, a check not current at HEAD, a HIGH finding
  without a fate, a private term, a dirty tree; else, after the push dialog, pushes the ticket branch and
  opens the MR or PR as a draft through `gh` or `glab`; the body holds the review map (files outside the
  steps' named files and "must not change" lines, the Review Focus items, deferred findings, imprecise
  places), the test evidence (`runs.jsonl`) and the token cost; the link goes to `state.json` (label
  STP-12). The merge stays with the person in the forge; `close` follows the merge.
- 1.8 one dialog per ticket: `stapel ok KEY` shows the human page (the Frame, the criteria, the Decisions
  lines, with their diff since the last confirmation) and confirms every section; Design, Proof and Plan
  appear as one line each (`design: 42 lines changed`), in full only with `--full`; `stapel close` through
  the dialog with its grant, as `ok` (today it needs the person's terminal).
- 1.9 the guard denies Bash commands that write into `crates/` (label STP-20, text in the phase-0 list).
- 1.10 the guard denies a compound command, a pipe or a redirection that holds a confirm or record command
  (label STP-21).
- 1.11 `stapel check` and the commit hook walk only `base..HEAD` (label STP-22; the pilot repository has a
  long history).
- 1.12 the `junit` runner: `[check] runner = "junit"`, `command` (a template with `{tests}`), `report`
  (the XML path), a timeout; test identity `<file>::<name>`; the outcomes of STP-4 and STP-14 unchanged;
  `lint` finds TypeScript tests by name.

Not taken from the old phase 1: `stapel build` as an orchestrator with `claude -p` (the chat is the
orchestrator); the Anthropic API provider (every role runs as a Claude Code subagent on the subscription;
the external reviewer runs as a subagent with another model); `build --at-own-risk` (a stale confirmation
stops `next`, and the person confirms in one dialog); one dialog per section (STP-7 is done by STP-17, and
1.8 goes further).

---

## Phase 2 — pilot on a foreign repository (two to three weeks)

Moved forward from phase 4 (`docs/decisions/0004`): the product claim is tested on a repository that is not
stapel's own before the guards and the index are built, and its measurements decide what phases 3 and 4
contain.

**How the pilot is run (decided by the orchestrator on the human's request, 2026-10-19).**

- The repository: the TypeScript project next to this one that klc already runs (jest with `jest-junit`);
  never named in tracked files. The human is the one technical person and the only reviewer in the forge.
- Preconditions, checked on the day before: phase 1 closed; `stapel init` there with the human's explicit
  consent at that moment; the binary on PATH, `gh` or `glab` logged in, Claude Code on the subscription.
- Three tickets, one after another, each the next real ticket of that project, each about 300 changed
  lines or fewer; the first one is the comparison with klc: the same ticket from the same base commit in two
  branches, stapel first, klc second, by the same person; the human chooses which branch merges and records
  why. Tickets two and three go through stapel only.
- The person's part per ticket: the request in the chat, the frame and coverage questions, one confirmation
  dialog, one push dialog, the merge in the forge. The author of stapel is not in the chat; what the person
  had to ask the author is a finding against the skill text.
- Measures per ticket, written into the ticket's summary and into `docs/`: the metrics of `docs/PLAN.md` §8;
  questions asked and dialogs shown; wall time from the request to the MR; escaped defects counted four
  weeks after the merge. The comparison table and the decision on klc (freeze, carry over the remainder,
  or bring the lessons back) close the phase.
- Ticket state lives on the ticket's branch next to the code (one ticket, one branch, one MR); a separate
  state branch stays an option in `stapel.toml` for a later team.

**Goal.** The three pilot tickets merged with the person answering questions and two dialogs per ticket;
the comparison with klc recorded.

**Acceptance criteria (abridged).**

1. The person types no stapel command during the three tickets; the decisions log shows every answer.
2. The MRs carry the review map, and the human reviewer's comments on them are recorded as findings.
3. Escaped defects per ticket are counted four weeks after each merge and written into the summary.
4. The comparison table is published in `docs/` of both repositories without the project's name.

**Exit criterion.** Three tickets merged on the pilot repository; the comparison table published; the
decision on klc recorded; the list of what phases 3 and 4 must contain updated from the pilot's findings.

**Items.**

- 2.1 `stapel init` on a foreign repository: no `[check] cargo`, the skill and the agents, `context.md` with
  the sections of phase 3 empty, a smoke ticket that proves the hooks and the junit runner; the person's
  checklist printed at the end.
- 2.2 pilot ticket one and the comparison record with klc (both branches, the metrics, the merge decision).
- 2.3 pilot tickets two and three; the comparison table; the decision on klc; the findings against the
  skill text and the plan of phases 3 and 4 updated.

---

## Phase 3 — project context, guards and autonomy levels (three to four weeks)

**Why this phase comes before the index.** The goals name one pain that stapel is for: software written by
chatting with an agent is an MVP, because the agent knows nothing of what the organisation keeps in people's
heads, and because each change looks right alone while their sum erodes the system. Phase 1 makes the
process repeatable; phase 2 shows it on a real repository; neither gives the agent the organisation's
knowledge or stops the erosion. Phase 3 does both: a project context the agent reads at every frame, and
checks the program runs that the agent cannot weaken (`docs/20260107_arc_integrity.md`, untracked note:
"the earlier and harder the check, the more reliable it is"; a reviewer model is the weakest layer). The
index (phase 4) saves tokens and sharpens the review map; it does not change what is true about the result.
The goals rank quality and determinism before token efficiency, and the measurements of phase 0 say the
orchestrator's context, not file reading, is the main cost, which `stapel next` already attacks. So phase 3
is the second half of the product claim and phase 4 is an optimisation measured against the pilot's numbers.

**Goal.** A ticket that breaks a project rule is stopped by a check the agent cannot weaken, not by a
reviewer; the person's involvement is a configured level, from "every question and both dialogs" down to
"one page, one dialog".

**What is included.**

- `.stapel/context.md`: domain terms, conventions, non-functional budgets, a "never" list, architecture
  rules; read by the orchestrator at the frame and by the builder brief; an answer that holds for the whole
  repository goes there instead of the ticket's Decisions log, through a question with that option.
- Machine guards out of the agent's reach under `.stapel/guards/`: dependency rules as tests, ratchet
  baselines ("no worse than now": existing violations recorded, new ones fail, a fixed one leaves the
  baseline for good), coverage of the diff, a public API snapshot, a new dependency only through a dialog;
  `stapel check` runs them at every step; the guard denies agent writes under `.stapel/guards/`; CI runs
  the same set and decides.
- Autonomy levels in `stapel.toml` (the founder parameter of decision 0004, second segment): which of the
  moments of `docs/decisions/0004` §4 ask the person. Level 1: every frame and coverage question, the
  confirmation dialog, the push dialog (phase 1 default). Level 2: only questions the orchestrator cannot
  answer from `context.md` and the code, one dialog. Level 3: the human page in the person's language,
  generated from the Frame and the criteria and confirmed instead of them; the criteria stay in English
  for the agents; a change in either makes the other stale.
- Ticket types `bug` (reproduction, cause hypothesis, no fix) and `spike` (a question with a time box), with
  templates; ticket status outside the confirmed text (label STP-11).

**Acceptance criteria (abridged).**

1. A builder that adds a forbidden import or a new dependency is stopped before the GREEN commit, with a
   message that names the rule and the fix.
2. A ratchet baseline shrinks when a violation is fixed and never grows without a dialog.
3. At level 3 the human page and the criteria are confirmed by one dialog; a change in either makes the
   other stale.
4. A pilot ticket at level 2 asks the person fewer questions than the same kind of ticket at level 1, and
   the answers it takes from `context.md` are listed in the ticket.

**Exit criterion.** One pilot ticket stopped by a guard; one ticket shipped at level 2 on the pilot
repository; the level-3 page produced for one ticket and read by a person who reads no tests.

**Items.** 3.1 `context.md` and the question option "holds for the repository"; 3.2 the guards folder, the
ratchet and `check` running them; 3.3 the dependency dialog and the API snapshot; 3.4 autonomy levels 1 and
2; 3.5 level 3, the human page; 3.6 `bug` and `spike`. Details are fixed at the end of phase 2 from the
pilot's findings.

---

## Phase 4 — index and selective tests (two to three weeks)

Unchanged in content from the earlier phase 2, moved after the pilot: the pilot's token measurements say
where the index pays. Included: `stapel-index` with adapters for TypeScript and Rust; `stapel q` (`symbol`,
`refs`, `callers`, `tests`, `impact`); blast radius, which also feeds the review map of `ship`; selective
test runs per step with a full run before `ship`; the MCP server with the same queries; context packs
assembled within a budget. Acceptance criteria as before: `impact` matches a manual sample, imprecise
answers marked, incremental re-indexing, selective and full runs in `runs.jsonl`, savings measured on three
tickets.

**Exit criterion.** The builder agent on a pilot ticket uses MCP queries instead of reading whole files,
and this is visible in `tokens.jsonl`.

---

## Phase 5 — Zed and more languages (after phase 4; not sized)

Decided by the human on 2026-10-19: Zed and a couple of other languages are the stage after these. Zed:
`stapel-lsp` for `ticket.md` and code, where each button calls one existing command, and the inbox in the
status bar. Languages: Python and Go adapters for the index and their test runners for `check` (pytest and
`go test` with JUnit output); C++ later. Nothing in phases 1 to 4 depends on this phase.

## What stays out of scope until phase 5 has run

Jira (the adapter exists, the connection comes later), semantic search on embeddings, multi-user locks more
complex than "the section is held by whoever confirms", a hosted service, a web UI, agents other than Claude
Code, deploy and operations after the merge, a separate observation stage.
