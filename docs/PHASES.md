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

**Items, in order.** Order changed on 2026-10-21 (ledger): 1.3, 1.5, 1.8, 1.4, 1.6, 1.7, 1.9 to 1.12, so
that the items that cut the person's approval dialogs come first; the numbers stay as labels. Changed on
2026-10-22 (ledger): 1.9 and 1.10 are one ticket; 1.11 takes the light route or joins 1.12; the frame and
spec of the next item may be written in a second worktree while the current ticket builds. Changed on
2026-11-02 (ledger): the new item 1.13 is taken before 1.6, and it replaces the second-worktree rule.
Changed on 2026-11-10 (ledger): after 1.13 only 1.11 with 1.12 (one ticket) and the new item 1.14 are taken;
1.6, 1.7, 1.9 and 1.10 leave the plan and become candidates ("The plan after STP-27" below).

- 1.1 (STP-20) `stapel next [KEY]`: the next action computed from `state.json`, the Plan steps, `check --history`,
  `runs.jsonl` and `findings.jsonl`; one line of action plus the command or the path (`--json` for the
  skill); the facts that block an action named first (stale, failed, unpaired, lint). First, because it
  replaces the orchestrator re-reading `CLAUDE.md` and a 50 KB ticket on every turn (ledger 2026-10-08: the
  orchestrator's context is the main cost).
- 1.2 (STP-21) the builder brief as a command, `stapel brief KEY STEP`, 900 to 1 300 tokens, assembled from the
  Frame, the step's criteria, its Inputs and External states rows, its "must not change" line and the step
  (`docs/decisions/0003`; label STP-10); `next` prints its command. Changed on 2026-10-20 (ledger): the brief
  goes to stdout; it also carries a header, the Spec terms its criteria use and the findings the step names;
  over 1 600 tokens it refuses.
- 1.3 (STP-22) the process as installed data: `stapel init` writes `.claude/skills/stapel/SKILL.md` (the spec
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
  (label STP-8). Split on 2026-10-22 (ledger): part 1 (STP-23) the import without a dialog and the
  triage; part 2 `review start`, the pack and the reviewers by route.
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
  appear as one line each (`design: 42 lines changed`); `stapel close` through the dialog with its grant,
  as `ok` (today it needs the person's terminal). Narrowed on 2026-10-30 (ledger, STP-25): no `--full`;
  the dialog holds a short page of at most 22 lines, the terminal form the whole page.
- 1.9 the guard denies Bash commands that write into `crates/` (label STP-20, text in the phase-0 list).
- 1.10 the guard denies a compound command, a pipe or a redirection that holds a confirm or record command
  (label STP-21). Taken with 1.9 as one guard ticket (ledger 2026-10-22).
- 1.11 `stapel check` and the commit hook walk only `base..HEAD` (label STP-22; the pilot repository has a
  long history); a commit that touches only `docs/`, the process ledger or a ticket's `tokens.jsonl` does
  not make a check stale (ledger 2026-10-22). Light route when its investigation shows about 100 mechanical
  lines and no risk tag; else taken with 1.12.
- 1.12 the `junit` runner: `[check] runner = "junit"`, `command` (a template with `{tests}`), `report`
  (the XML path), a timeout; test identity `<file>::<name>`; the outcomes of STP-4 and STP-14 unchanged;
  `lint` finds TypeScript tests by name.
- 1.13 two ticket builds at once (ledger 2026-11-02): one ticket per session, each on its own branch and
  worktree; keys that do not repeat across worktrees and branches; commands and hooks without KEY take the
  ticket of the current branch; the build permit only for that ticket; one machine-wide cargo lock with a
  queue; independence checked by stapel, not by the person: named and changed files, a dependency agent
  before the build, a re-run of `stapel check` after the other ticket merges; a dependency found stops the
  later ticket until the earlier one closes; after `stapel close` the orchestrator merges the branch into
  main locally, with no push.

Not taken from the old phase 1: `stapel build` as an orchestrator with `claude -p` (the chat is the
orchestrator); the Anthropic API provider (every role runs as a Claude Code subagent on the subscription;
the external reviewer runs as a subagent with another model); `build --at-own-risk` (a stale confirmation
stops `next`, and the person confirms in one dialog); one dialog per section (STP-7 is done by STP-17, and
1.8 goes further).

---

## The plan after STP-27 (ledger 2026-11-10)

Cut on 2026-11-10 after a review of the code and the business model. Every ticket so far ran on this
repository, tickets grew (STP-27: 50 steps in 8 days), and the product claim, "the tenth change costs what
the first did", is untested. The pilot needs only the junit runner, so everything else waits for the pilot's
numbers. The earlier phases 2 to 5 are replaced by the stages below; their text is in the git history.

Two process rules from the same decision:

- A ticket over 12 Plan steps or two code review rounds stops and is split by a ledger decision.
- The guard is frozen: new guard work starts only after a real bypass seen in the pilot.

### Stage A — the road to the pilot (about a week)

Items. Changed on 2026-11-11 (ledger, two decisions): A1 is split, A3 runs in parallel with A1, and the
`base..HEAD` walk leaves the plan (measured: `stapel next` walks the 1035 commits of this repository in
0.05 s, and a ticket made by `stapel new` has a base that bounds its range). Each ticket has its own session.

- A1 (item 1.12, STP-28): the `junit` runner, with the text of that item above without its `lint` clause.
- A3 (item 1.14), in parallel with A1: see below.
- A1b (the rest of items 1.11 and 1.6 that the pilot needs; after A1 closes): a commit that touches only
  `docs/` does not make a check stale (the process ledger and a ticket's `tokens.jsonl` are machine files
  already); `stapel lint [KEY]` with two checks for both runners: a test the ticket names does not exist,
  and a step test no ticket names.
- A2 (item 2.1, widened; depends on A1): `stapel init` on a foreign repository: no `[check] cargo`, the
  skill and the agents, a smoke ticket that proves the hooks and the junit runner, the person's checklist
  printed at the end. Widened: the files `init` installs stay out of the shared git history of that
  repository (for example through `.git/info/exclude`), also when its git hooks live in a tracked folder.
- A3 (new item 1.14): attention numbers per ticket in the token report: dialogs shown, questions asked, the
  human's minutes, Plan steps, wall time from the request to the merge request. Also: a `SubagentStop`
  whose transcript file does not exist writes no `estimate` record (312 of 545 records of STP-27).
  Widened on 2026-11-10 (ledger, second decision) by two pilot measures: the time from a confirmation
  request to its answer, and the count of questions that concern the feature, not the ticket.

### Stage B — the pilot (two to three weeks)

- The repository: the TypeScript project next to this one (jest with `jest-junit`, git hooks in a tracked
  folder); never named in tracked files. Two local clones of its branch are made after A1 and A2 close and
  the human confirms the base commit: one for stapel, one for a plain agent. Their `origin` is the local
  repository, so nothing leaves the machine by accident.
- 2.2 pilot ticket one, two ways from the same base commit: through stapel, and through a plain Claude Code
  agent without the process. The ticket: stream highlighting driven by data, with a hover cost that does not
  depend on the row count. The comparison with a plain agent replaces the comparison with klc.
- 2.3 pilot tickets two and three, through stapel only: collapsed columns with windowing (a UI ticket that
  is hard to test in jsdom), then the verification ticket of that group (e2e, performance baseline,
  documents; almost no RED→GREEN).
- The person's part per ticket: the request in the chat, the questions, one confirmation dialog, the push
  and the merge request by hand. The author of stapel is not in the chat; what the person had to ask the
  author is a finding against the skill text.
- Measures per ticket: the attention numbers of A3, tokens by role, review rounds and findings, escaped
  defects counted four weeks after the merge.

### Stage C — the gate (item 2.4)

stapel continues as planned only if all three hold on the pilot's numbers:

1. The human's minutes per ticket are not above a plain agent plus the review of its code.
2. Escaped defects are fewer than the plain agent's.
3. The third ticket costs no more than the first.

Otherwise stapel narrows to `check` and the token journal for CI. The decision is a ledger record. If it
narrows, phases 3 to 5 below are not taken.

Also decided at the gate (ledger 2026-11-10, third decision): the kind `bug`. Until then a bug goes through
the current process unchanged; the RED test is the reproduction. Every escaped defect of a pilot ticket
becomes a bug ticket, and its attention numbers (item 1.14) show what a bug costs. If that cost is too high
for a two-step fix, the kind `bug` enters phase 4 by a ledger decision.

### After the gate — phases 3 to 5 (ledger 2026-11-10, second decision)

Taken only if the gate (2.4) passes, in this order, chosen by the human. The pilot's findings may reorder
them only by a ledger decision. The numbers 3 to 5 are new; they do not mean the earlier phases 3 to 5.
Each item is one ticket under the rule of 12 Plan steps.

**Phase 3 — one ticket made lighter.**

- 3.1 the human page as a file. `stapel` writes the page (the Frame, the criteria, the Decisions lines, one
  line each for Design, Proof and Plan) to `.stapel/tickets/KEY/page.md`; the orchestrator names the path in
  the chat and the person reads it in the editor; changed lines are marked after an edit. The dialog of
  `stapel ok KEY` shows a short summary (title, criteria count, changes, the page's hash) and refuses when
  the page changed after it was written. No editor extension and no LSP.
- 3.2 `stapel check` in the CI of the repository: the same check on the merge request, so the boundary
  decides, not the agent's shell. This is what lets the guard stay frozen.

**Phase 4 — the feature level.** Optional: a ticket without a feature works as before. Two levels only,
feature and ticket.

- 4.1 the elaboration skill. The questions, the template and the checklists for elaborating a feature are
  ported from the method studied in decisions 0001 to 0003 into the skill `stapel init` installs. Its
  licence is MIT (checked 2026-11-10): the copyright and permission notice is kept; the method's name is a
  trademark and appears only in a truthful compatibility statement. Not taken: tracker stores, personas,
  Python tooling. The ticket's investigation chooses the snapshot. A report per release of the method lists
  ideas worth taking.
- 4.2 the kind `feature`: a ticket with a description, requirements with ids, a Decisions log and a
  breakdown into tickets with dependencies; confirmed by the same dialog and page as a ticket; no build and
  no status of its own. Child tickets are created from the breakdown and name the requirements they cover.
- 4.3 staleness from requirements to tickets. A ticket records the hash of each requirement it covers. A
  changed requirement makes an unbuilt ticket wait for a new confirmation with the diff, stops a ticket in
  build, and marks a closed ticket "built against the old text" until the human decides: a new ticket, or
  "no change needed, because".
- 4.4 computed readiness. A feature is done when its tickets are closed and each requirement is proved by a
  named test; `stapel next` on a feature names the next ticket and what blocks it.

- Candidate for this phase, decided at the gate: the kind `bug`, on the ticket-kind mechanism of 4.2. A
  short form of three lines (the reproduction, the cause hypothesis found by an investigation before the
  Frame, the source ticket), one RED, one GREEN, one dialog. The link to the source ticket counts escaped
  defects per ticket without a hand-written note. A bug has no level above it; when its fix changes a
  requirement, the staleness of 4.3 applies.

The quality of a feature spec is measured, per ticket, by three numbers: `intent_gap` findings and
questions (enough information), brief size and builder tokens (not too much), dialogs and the human's
minutes (attention). A rule of the elaboration skill that catches nothing is removed as in the spec process.

**Phase 5 — the sum of changes.**

- 5.1 the guards folder: machine checks out of the agent's reach with ratchet baselines ("no worse than
  now": existing violations recorded, new ones fail, a fixed one leaves the baseline for good); `stapel
  check` and the CI run them.
- 5.2 a new dependency only through a dialog, and a public API snapshot.

### Candidates without a place in the order

Not a plan; each needs a ledger decision to enter it. The `base..HEAD` walk of item 1.11 (taken when a
measurement shows a slow walk); `stapel lint` (item 1.6); `stapel ship` with the
review map (item 1.7); guard items 1.9 and 1.10; an import from the method or from a tracker when a real
user asks; the index; autonomy levels; an editor extension; more languages; removal of the five empty
crates; one short plan document in place of `docs/PLAN.md` and this file.

## What stays out of scope

Jira sync, semantic search on embeddings, multi-user locks, a hosted service, a web UI, agents other than
Claude Code, deploy and operations after the merge, a planning method of stapel's own beyond the feature
level above, more than two levels of work items.
