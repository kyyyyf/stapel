# stapel — the plan as it was written

Superseded on 2026-11-20 by the single plan document `docs/PLAN.md` (ledger decision of that day). This
file keeps the earlier texts unchanged, because tickets, decisions and ledger records name them by their old
paths and section numbers: `docs/PLAN.md` (the overall plan of 2026-10-02 with its later notes) and
`docs/PHASES.md` (the phase plan with phases 0 and 1, the pilot, the gate and the plan after it as it stood
on 2026-11-20). Nothing here is current; the current plan and its order are in `docs/PLAN.md`.

---

# Part 1. The overall plan (`docs/PLAN.md` until 2026-11-20)

Status: plan at the start of the project, 2026-10-02. Changed by decisions recorded in tickets and in
`docs/decisions/`; the product direction and the interface changed on 2026-10-19 (`docs/decisions/0004`).
The plan was cut to the pilot on 2026-11-10 (ledger): phases 2 to 5 below are replaced by the stages in
`docs/PHASES.md`, "The plan after STP-27"; §8 compares with a plain agent, not with klc.
The feature level is stapel's own (ledger 2026-11-10, second decision): the method studied in decisions
0001 to 0003 is a source of ideas and a possible input, not the upper layer of stapel.

## 1. What it is

`stapel` is a tool that takes a development ticket from description to merge so that the result is
repeatable, the process is understandable without reading the internals, and token spending is measured from
day one. It is built for a team of three roles — product, engineer, QA — who may be the same person. It
works on any repository that has a `.stapel/` folder.

**For whom (2026-10-19, `docs/decisions/0004`).** Companies and departments that build software they could
not afford before, and keep it changeable: the first segment is a company or department with one technical
person; the second stage is a founder who builds alone, served by a configurable autonomy level (phase 3).
Not for fast hypothesis checks: an agent alone serves those. The claim is "stays changeable at a known cost", measured by escaped defects and tokens per
ticket (§8).

**Interface.** The person works in the Claude Code chat and types no stapel command: the orchestrator (the
Claude Code main agent with the stapel skill) asks questions, writes the ticket, runs builders and reviewers
as subagents, and confirms with the person through the permission dialog. `stapel` is the backbone the agent
calls. The merge request in GitLab or GitHub is the second interface, for the reviewers. Zed is optional.

The name means a slipway: the place where a ship is built step by step before it is launched.

## 2. Principles

1. **Agreements become verifiable artifacts.** An acceptance criterion is a test. A design risk is a test.
   An agent's claim is marked "verified", "read" or "assumption".
2. **Determinism comes from code.** Everything a program can check, the program checks: tests, the RED→GREEN
   order, "criterion → test", the diff against the plan. The model is responsible only for judgment.
3. **State is facts, the phase is computed.** What is stored: confirmations with a section hash, green steps
   on a commit, open findings, the release. "Whose decision is needed now" is derived from the facts.
4. **Three kinds of checks.** Semantic ones stop the process. Format errors the tool fixes itself. Advice is
   visible and does not block.
5. **People make only decisions.** Answering a question, choosing an option, confirming, "do not fix,
   because". Every decision is recorded: who, when, why.
6. **One live author session per ticket, independent agents only for review.** The prompt cache works, and
   independence is where it is needed.
7. **The index answers questions instead of guessing files.** Its product is the blast radius of a change
   and a selective test run.
8. **Tokens are counted from day one**, by role, marked "measured" or "estimate".
9. **One binary.** No environments and no dependencies on the developer's machine.
10. **The process is data.** Definitions of sections, checks, roles and models live in TOML and Markdown;
    the core executes them.

## 3. What it looks like for the team

For the person, one ticket is six moments (`docs/decisions/0004` §4): the request in the chat; the frame
and coverage questions, each with options and a recommendation; one dialog that confirms the human page
(the Frame and the criteria, with their diff) and with it every section; a question only for an
`intent_gap` or a finding the orchestrator could not settle; one dialog for the push and the MR; the
summary after the merge. Which of these moments ask the person is the autonomy level (phase 3).

For the orchestrator, the backbone is a few commands, each one deterministic:

```text
stapel new <title>         create a ticket; a link to an external tracker as an option
stapel next [KEY]          the next action from the facts, with its command or file (phase 1)
stapel brief KEY STEP      the builder brief for one plan step, 900 to 1 300 tokens (phase 1)
stapel ok KEY              confirm, through the permission dialog; shows the diff since the last confirmation
stapel check [KEY]         every step's tests failed at RED and pass at GREEN and HEAD; --history in seconds
stapel lint [KEY]          the ticket is consistent: tests and paths exist, tables present, lists agree (phase 1)
stapel review start KEY    the git archive copy and the review pack for a round (phase 1)
stapel ship KEY            preconditions, push and the MR with the review map, through the dialog (phase 1)
stapel close KEY           close after the merge, through the dialog
stapel q <question>        query the index: symbol | refs | callers | tests | impact (phase 4)
```

Records: `findings import`, `findings triage` and `ledger add --kind finding|catches` without a dialog (STP-23);
`tokens import-records` and `ledger add --kind decision` through the dialog; `tokens` for the report. Auxiliary: `status`, `init`, `hook` (the entry points Claude Code calls). The orchestrator's skill
is one rule: run `stapel next`, do what it says, run it again.

One file for people — `ticket.md`:

| Section | Owner | Contents |
|---|---|---|
| Description | customer | the original task, a link to the tracker |
| Spec | product | acceptance criteria (each one is a test), questions with answers, "Out of scope" |
| Design | engineer | options, the choice with a reason, risks (each one is a test) |
| Proof | QA | a table "criterion or risk → test → result" (the tool fills in the result) |
| Plan | engineer | build steps: tests, the check command, the expectation |
| Review | generated | a map "required / desirable / can be skipped", findings and their fate |
| Summary | generated | time, tokens by role, review rounds, what the review found, three lines from a human |

Machine files sit next to it, and only the tool writes them: `state.json` (facts), `decisions.jsonl`,
`findings.jsonl`, `runs.jsonl`, `tokens.jsonl`.

Roles work in parallel on drafts. A confirmation is tied to the section hash: if the spec changed, the
confirmations of the design and the proof become stale, and the tool shows exactly what changed. The build
is permitted when the three confirmations refer to the same version of the spec; the engineer may start
earlier with a mark "at own risk".

The scope is determined by rules (how many modules are touched, whether an interface or a data format
changes, a migration, security); a human may raise or lower it, with a reason. A small task goes the same
way, only shorter: its sections consist of one line or are created automatically.

## 4. Architecture

Rust, one cargo workspace with several packages:

| Package | Responsibility |
|---|---|
| `stapel-core` | tickets, state facts, section hashes, checks, the decision log, process definitions from TOML |
| `stapel-agent` | model providers, roles, context packages with a budget, the token journal, running the author and the reviewers |
| `stapel-index` | the index core and language adapters: structure (tree-sitter), references (SCIP), tests, git history; blast radius |
| `stapel-forge` | MR/PR, CI, line comments: GitLab and GitHub through `glab`/`gh` or the API |
| `stapel-mcp` | MCP server: index queries and ticket state for the agent |
| `stapel-lsp` | LSP server for Zed: buttons above sections, highlighting, hints |
| `stapel-cli` | the `stapel` binary |

Layout in the project repository:

```text
.stapel/
  stapel.toml          process: sections, checks, roles, models, scope rules
  tickets/STP-12/      ticket.md, state.json, decisions.jsonl, findings.jsonl, runs.jsonl, tokens.jsonl
  index/               index cache (not in git)
  allowlist.toml       reviewers' false positives
```

Ticket state lives in git: either in the ticket's own branch next to the code, or in a separate state
branch (as in klc). The choice is a phase 1 decision; by default a separate branch `stapel-state`.

A language adapter is four things: a tree-sitter grammar, a source of references (a SCIP indexer or a
fallback on ripgrep marked "imprecise"), test recognition, module boundaries. The first adapters: Python,
TypeScript/JavaScript, Go, Rust. C++ is added later as one more adapter (scip-clang); the core does not
change.

## 5. Models and providers

Providers: Claude Code on a subscription (usage is taken from its JSON output), the Anthropic API with a key
(the `usage` field), room for others. A role is bound to a model in `stapel.toml`; the token journal brings
records to one form and marks them "measured" or "estimate".

Starting values, to be changed by measurements:

| Role | Model | Why |
|---|---|---|
| spec and design author (`ask`, options) | claude-opus-5-5 | judgment, questions, design |
| builder (`build`) | claude-sonnet-5-5 | a lot of mechanical work by the plan; cheaper |
| fresh code reviewer | claude-sonnet-5-5 | an independent look at the diff against the criteria |
| external reviewer | claude-fable-5-1 | the strongest model and a different one from the author's: real independence |
| drift reviewer (code ↔ plan ↔ spec) | claude-sonnet-5-5 | cross-checking, not creative work |
| cheap judgments (scope, finding classification) | claude-haiku-4-5 | where a deterministic rule was not enough |

```toml
# .stapel/stapel.toml — fragment
[models]
author   = { provider = "claude-code", model = "claude-opus-5-5" }
builder  = { provider = "claude-code", model = "claude-sonnet-5-5" }
reviewer = { provider = "claude-code", model = "claude-sonnet-5-5" }
external = { provider = "anthropic-api", model = "claude-fable-5-1", required = false }
drift    = { provider = "claude-code", model = "claude-sonnet-5-5" }
cheap    = { provider = "claude-code", model = "claude-haiku-4-5" }
```

If a role has no key or subscription, the tool says so and works with what it has (`required = false` for
the external reviewer). The orchestrator role is played by Claude Code in an interactive session in every
phase (`docs/decisions/0004`); from phase 1 it follows `stapel next`, and every other role runs as a Claude
Code subagent whose definition `stapel init` writes from `[models]`. The `anthropic-api` provider stays an
option for a role that must run outside the subscription.

## 6. Integrations

- **Forge.** One ticket — one branch — one MR. `build` opens the MR as a draft at the first step. Reviewers'
  findings and people's comments are visible as MR line comments; the source of truth is `findings.jsonl`.
  A human's comment in the MR or in Zed becomes a finding of kind `human-review`, and the builder must
  assess it.
- **CI.** The proof of a test is a record "command, output, commit, source". The source is CI or a local
  run. Both are acceptable, and the difference is visible in the ticket summary.
- **Tracker.** An adapter from day one: key, link, status sent outward. Jira is connected later.
- **Claude Code.** Installed by `init` as data: the hooks (no writes to code before the build permit, no
  push for agents, no writes to machine files, the permission dialog for `ok`, `close`, `ship` and the
  record commands, token records at `SubagentStop` and `Stop`), the orchestrator skill (the spec process),
  and the agent definitions (builder, drift reviewer, code reviewers, spec reviewer) with tools, model and
  permission mode; reviewers are read-only. The person's own edits to these files survive a re-run.
- **Zed.** Optional, after a pilot person asks for it: `stapel-lsp` buttons would each call one existing
  command. Not in the phases (`docs/decisions/0004`).

## 7. What is carried over from klc

As ideas and data, not as code: the finding format and accepting answers with one command; the reviewer
roles and their prompts; questions with a recommendation and a coverage map; criteria checking (the form
"who · what · with what · under what condition", vague words, "Out of scope"); the provenance of claims;
the code ↔ plan ↔ spec cross-check; repeating the checks on the final commit; the reason for a return; the
allowlist of false positives; review on an isolated copy; hashes before and after live operations and a
rehearsal on a copy (`guide`); scope checking through the blast radius; simple section locks; protecting
the public mirror from private terms; hooks against writes by agents.

Not carried over: the separate learn and manual stages, tracks as different sets of stages, ranking files
for a ticket, dashboards drawn by a model.

## 8. How success is measured

On the same tickets through `stapel` and through klc (phase 4):

| Metric | How it is counted |
|---|---|
| human hours per ticket | from the decision log: the time between decisions that required a human |
| stops for no good reason | how many times the process stopped for a reason other than a code defect |
| review rounds and defects found | from `findings.jsonl` |
| tokens per ticket, by role | from `tokens.jsonl`, measurements only |
| index savings | the same ticket with and without the index: tokens and turns to the first correct change |
| CI minutes | selective run against a full run |
| escaped defects | defects found after a ticket closed: by `stapel check` on its history, by later tickets, by the human |

**Added after STP-4.** Escaped defects are the main quality measure, and the per-ticket spec process is
judged by them, by the HIGH and MEDIUM inside-promise findings of code review, by the size of `ticket.md`
and by tokens — not by the share of review findings catchable before code. That share measured mostly the
reviewers' quotas: code review found 30 to 38 findings on every ticket from STP-2 to STP-4, while the spec
work grew to 54 KB and 89 findings before code on STP-4. The spec process is in `CLAUDE.md`, "The spec
process"; rules that catch nothing for three tickets are removed (`.stapel/process-ledger.jsonl`).

## 9. Risks

- **It will grow complicated again.** Defense: the process is data; every new check must be semantic or
  self-fixing; once per phase, a revision of the commands and checks with the numbers of their use.
- **Rust will slow down early iterations.** Defense: the core is small, everything changeable is in TOML and
  Markdown.
- **SCIP is not for every language and is not always up to date.** Defense: a fallback path on tree-sitter
  and ripgrep with an honest mark "imprecise"; the review map sends people to look at exactly the imprecise
  places.
- **Subagent tokens inside an interactive Claude Code session are hard to measure.** Defense: subagent logs;
  until there is a number, the mark "estimate", never presenting it as a measurement.
- **A single maintainer.** Defense: the tool must run its own tickets as early as possible (phase 1), so
  that the process does not depend on one person's memory.
- **The product runs inside Claude Code.** Hooks, the permission dialog, custom agents and skills are
  documented; the transcript format is documented as internal and may change. Defense: the `estimate` mark
  when a transcript cannot be read, and the usage of headless runs as a second source.
- **The process stays heavier than the segment accepts.** Phase 0 tickets reached 50 KB and 2 500 changed
  lines for small commands. Defense: the human page and the engagement levels (phase 3), the ticket lint's
  size limits, and the pilot's measurements before the index is built.

## 10. Phases

In detail — in `docs/PHASES.md`.

```text
0  bootstrap         state core, sections, confirmations, RED→GREEN, findings, tokens;      closed 2026-10-19
                     guard, dialog, check on history, records through the dialog
1  backbone          next, briefs, the process installed by init, tokens from hooks,         ~3 weeks
                     review packs, lint, ship with the review map, one dialog, the junit runner
2  pilot             a foreign TypeScript repository: init there, three tickets,             ~2–3 weeks
                     the first one also through klc, the comparison
3  context & guards  project context, machine guards out of the agent's reach,               ~3–4 weeks
                     autonomy levels, the human page, bug and spike tickets
4  index             TypeScript and Rust adapters, blast radius, selective tests, MCP         ~2–3 weeks
5  Zed & languages   stapel-lsp buttons, inbox; Python and Go adapters and runners           not sized
```


---

# Part 2. The phase plan (`docs/PHASES.md` until 2026-11-20)

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
- 1.12 the `junit` runner: `[check] runner = "junit"`, `command` (a template with `{file}`, one run per test file; STP-28), `report`
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

Changed on 2026-11-12 (ledger, the human): STP-28 had built the `base..HEAD` walk, the `docs/` rule and
`stapel lint` before the decisions of 2026-11-11 reached it, so A1 keeps them, A1b is done inside A1, and
the code defects of its second code review go to A1c.

- A1 (items 1.11, 1.12 and the `lint` part of 1.6, STP-28): the `junit` runner; the `base..HEAD` walk; a
  commit that touches only `docs/` does not make a check stale; `stapel lint [KEY]` with two checks for
  both runners: a test the ticket names does not exist, and a step test no ticket names. A1b is done here.
- A3 (item 1.14), in parallel with A1: see below.
- A1c (follow-up of STP-28, before A2): the defects and coverage its second code review left: a merge of
  the base branch that brings a fixture still gives `tests-changed`; an absolute script path in `command`
  runs the working tree's script; the report reader's roots, CDATA outside the root and entities in
  skipped elements; a silent cargo fallback when `stapel.toml` fails to load; the report read between its
  metadata and its content; tests for build inputs, negatives and cargo fixtures brought by a base merge
  (CR2-4, CR2-5, CR2-7, CR2-8, CR2-11, d20-2, d20-3, d21-1, d21-2, d22-1).
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

Verdict after three pilot tickets (ledger 2026-11-19, with the human; the reports are in
`.stapel/scratch/pilot/`): point 1 fails by its letter, because the person of this pilot does not read the
agent's code, so the term "plus the review of its code" is zero and stapel's 51 to 88 minutes stand against
the plain agent's 14; point 2 cannot be counted, because the plain agent's branch was never merged; point 3
passes on every measure (returns 10 against 24, output tokens 369 k against 639 k). The human decided to go
on with phases 3 to 5 and replaced points 1 and 2 by measures that fit a person who does not read code: the
returns to the person (dialogs, question dialogs, typed messages) and the minutes of interaction against a
plain agent on the next foreign repository, and escaped defects counted by the project's CI after the merge
and by a mutation run on the diff (phase 5). The order of the phases changed the same day (below).

Also decided at the gate (ledger 2026-11-10, third decision): the kind `bug`. Until then a bug goes through
the current process unchanged; the RED test is the reproduction. Every escaped defect of a pilot ticket
becomes a bug ticket, and its attention numbers (item 1.14) show what a bug costs. If that cost is too high
for a two-step fix, the kind `bug` enters phase 4 by a ledger decision.

### After the gate — phases 3 to 5 (ledger 2026-11-10, second decision)

Taken in this order, chosen by the human: phase 3, then phase 3b, then phase 5, then phase 4 (ledger
2026-11-19; before that day the order was 3, 4, 5). The pilot's findings may reorder them only by a ledger
decision. The numbers 3 to 5 are new; they do not mean the earlier phases 3 to 5. Each item is one ticket
under the rule of 12 Plan steps. Item 4.1 was started on 2026-11-19 before the reorder and finishes; the
next ticket after it is 3.2a, then 3.2b, then 3.3.

Why this order: the pilot showed that stapel's value lies in the checks at the boundary (the stand, RED
before GREEN, the project's CI) and its cost in the mechanics of the process (dialogs per review round, the
grant lifetime, the history rules). Phase 3b removes the cost, phase 5 adds the checks and gives the
mutation run that stands in for the uncountable escaped defects; the feature level of phase 4 plans larger
work, which no pilot ticket needed.

**Phase 3 — one ticket made lighter.**

- 3.1 the human page as a file. `stapel` writes the page (the Frame, the criteria, the Decisions lines, one
  line each for Design, Proof and Plan) to `.stapel/tickets/KEY/page.md`; the orchestrator names the path in
  the chat and the person reads it in the editor; changed lines are marked after an edit. The dialog of
  `stapel ok KEY` shows a short summary (title, criteria count, changes, the page's hash) and refuses when
  the page changed after it was written. No editor extension and no LSP.
- 3.2 `stapel check` in the CI of the repository: the same check on the merge request, so the boundary
  decides, not the agent's shell. This is what lets the guard stay frozen.

**Phase 3b — the mechanics after the pilot (ledger 2026-11-19).** Seven small tickets: 3.2a and 3.2b from the
local check time, the others each from a pilot finding named in `.stapel/scratch/pilot/report-ticket-2.md` and `report-ticket-3.md`.

- 3.2a faster fixtures of `tests/check.rs` (ledger 2026-11-19, after STP-34; taken before 3.3): the 175 tests
  run real cargo on the fixture crate `tiny` one at a time, each with a cold target folder, so a local
  `stapel check` takes 30 to 100 minutes. A shared `CARGO_TARGET_DIR` for the fixture builds, and a fake
  cargo for the tests whose subject is the logic of `stapel check`; real cargo stays in a few tests of the
  cargo boundary. Measured: the time of `tests/check.rs` and of the whole local check, before and after.
  Also: `check.timeout_secs` did not stop a suite run of more than 77 minutes.
- 3.2b check levels, points and the step cache (ledger 2026-11-20): `stapel check` keeps the results it
  has already proved. A step whose RED commit, GREEN commit, test list, lock file hash and stapel version
  match a passing record of `runs.jsonl` is not run again, because the commits cannot change; the HEAD run
  and the suite are what a new run adds. Three levels, in `[check] level` and as a flag: `steps` runs the
  new steps at RED and GREEN and their tests at HEAD, without the suite; `head` runs every step's tests at
  HEAD and the suite; `full` runs everything as today, cache ignored. The points at which `stapel next`
  demands a check are the list `[check] points`, by default `["step", "review", "close"]`: after each
  builder commit at `steps`, before code review and at the close at `head`; `full` runs in the CI of 3.2,
  and the close needs it green (3.5). The list can drop `step`; `close` cannot be dropped, and a RED run is
  never skipped. A check stays valid when the commits after it touch only `.stapel/tickets/` (finding of
  STP-32). `[check] suite_exclude` names test binaries or files the local suite leaves out; the CI's `full`
  run never leaves them out. Measured: the time of the `steps` check after one step and of the `head` check
  before the close on this repository and on a jest project.
- 3.3 the grant lifetime (finding G-6): a grant of `stapel ok` and `stapel close` lives 24 hours instead of 30
  minutes, so that an answer after a pause does not cost a second dialog; the sweep still removes expired
  grants.
- 3.4 fix steps without a dialog per round (the structure of the returns): the fix and coverage steps that a
  review round adds are built on a provisional permit and confirmed together by the `stapel ok` before the
  close; a fix step that widens the promise or changes a criterion's meaning still stops for its own dialog.
  Expected: three dialogs per ticket (after the spec review, before code review, close).
- 3.5 the close needs the project's own checks (finding G-12): `stapel check` runs the project's check command
  under the Node version the project pins (`.nvmrc`, `engines`), and the close runs the project's pre-commit
  checks on the whole diff (for the pilot: prettier, eslint, tsc), so that nothing is left to the CI after the
  merge. In this repository the CI of 3.2 is that check.
- 3.6 softer history rules (finding G-10): a GREEN commit may change a step's test file and a build input may
  change after a RED when the commit message says so (`history: <reason>`), and `stapel check` judges the
  result (the tests fail at RED and pass at GREEN and HEAD), not the shape of the history. The four history
  rewrites of pilot ticket three were its largest cost.
- 3.7 small findings in one ticket: `stapel review start` accepts any round number (G-7); `stapel close` writes
  the rule 9 ledger records `catches` and `finding` from the ticket's findings (G-11); an agent edit of
  `.stapel/stapel.toml` goes through a dialog like `stapel ok` instead of being denied (G-9).

**Phase 5 — the sum of changes, and the index (ledger 2026-11-20).** The index answers the questions an
agent asks about a codebase it does not know, so that a feature, a bug fix or a refactoring names every
place it must change before the build, instead of reading the codebase or guessing. The plan of
`docs/PLAN.md` (principle 7, the package `stapel-index`) is taken here; the human asked for it for new and
existing projects alike.

- 5.1 the guards folder: machine checks out of the agent's reach with ratchet baselines ("no worse than
  now": existing violations recorded, new ones fail, a fixed one leaves the baseline for good); `stapel
  check` and the CI run them.
- 5.2 the structural index and `stapel q`. A tree-sitter parse of every tracked TypeScript and Rust file
  gives the symbols of a file, the import graph and the test files of a module (by import); it lives in
  `.stapel/index/`, outside git. The queries: `symbol` (where a name is defined), `refs` (who imports or
  names it), `tests` (the tests that reach a file) and `impact` (the blast radius of a set of files: every
  file that depends on them through the graph, and the tests of those). How the index stays current: it
  trusts the working tree, not git events. Every query first compares the content hash of each tracked file
  with the hash it indexed and re-parses the changed files before it answers; a parse is milliseconds, so
  the cost is paid by the query, and a change made outside a ticket, by hand or by another tool, is seen by
  the next query like any other. `stapel index` rebuilds everything, and a stapel version change does that
  on its own. Nothing in the index is confirmed by the person: it is a cache of facts, and a wrong answer is
  a bug of stapel.
- 5.3 the blast radius in the process. The investigation before the Frame runs `impact` on the files the
  request names and puts the result in the Frame as the list of places the ticket changes; the brief of a
  step carries the places of that step; the review pack and the drift pack carry the blast radius of the
  diff, so a reviewer sees what should have changed with the code; the Review Focus names the tests of the
  radius that the plan does not run. The agent queries the index as a tool through `stapel-mcp`, not by
  reading files. Measured as `docs/PLAN.md` says: the same ticket with and without the index, tokens and
  turns to the first correct change.
- 5.4 a new dependency only through a dialog, and a public API snapshot. The snapshot is computed from the
  index (the exported symbols of the package and their signatures) and kept in the guards folder; a change
  of it is a ratchet failure until the person confirms it.
- 5.5 precise references. Where a SCIP indexer exists (scip-typescript, rust-analyzer), `refs` and `impact`
  use it; it is slower, so it runs at commit boundaries (a post-commit hook, `stapel check`, `stapel review
  start`) and the answer says which commit it is current for and lists the files changed since, for which
  the structural index of 5.2 answers instead, marked "imprecise".

Candidates on the index for phase 4, on the ticket-kind mechanism of 4.2: the kind `bug`, whose cause
hypothesis names the callers and the tests around the failing place from the index; and the kind
`refactor`, whose promise is "behaviour unchanged" and whose proof is characterisation tests on the blast
radius, the API snapshot of 5.4 and the full list of call sites from the index, so that no site is left
unchanged.

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

### Candidates without a place in the order

Not a plan; each needs a ledger decision to enter it. The other checks of `stapel lint` (item 1.6; its two
test-name checks are built in STP-28); `stapel ship` with the
review map (item 1.7); guard items 1.9 and 1.10; an import from the method or from a tracker when a real
user asks; autonomy levels; an editor extension; more languages; removal of the five empty
crates; one short plan document in place of `docs/PLAN.md` and this file.

## What stays out of scope

Jira sync, semantic search on embeddings, multi-user locks, a hosted service, a web UI, agents other than
Claude Code, deploy and operations after the merge, a planning method of stapel's own beyond the feature
level above, more than two levels of work items.
