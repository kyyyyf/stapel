# stapel — overall plan

Status: plan at the start of the project, 2026-10-02. Changed by decisions recorded in tickets and in
`docs/decisions/`; the product direction and the interface changed on 2026-10-19 (`docs/decisions/0004`).

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
