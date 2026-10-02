# stapel — overall plan

Status: plan at the start of the project, 2026-10-02. Changed by decisions recorded in tickets.

## 1. What it is

`stapel` is a tool that takes a development ticket from description to merge so that the result is
repeatable, the process is understandable without reading the internals, and token spending is measured from
day one. It is built for a team of three roles — product, engineer, QA — who may be the same person. It
works on any repository that has a `.stapel/` folder. Interfaces: command line, Zed, Claude Code in the Zed
agent panel, a merge request in GitLab or GitHub.

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

Five actions and one query:

```text
stapel new <key|link>      create a ticket from a description or a task in an external tracker
stapel ask                 the agent asks questions with a recommendation, the human answers
stapel ok <section>        confirm a section: spec | design | proof | review
stapel build               build step by step, each step RED → GREEN
stapel ship                merge the MR, record the summary
stapel q <question>        query the index: symbol | refs | callers | tests | history | impact
```

Auxiliary: `status`, `inbox` (what is waiting for me), `guide` (step-by-step procedure), `init`, `index`.

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
the external reviewer). In phase 0 the orchestrator role is played by Claude Code in an interactive session;
from phase 1 it is `stapel` itself.

## 6. Integrations

- **Forge.** One ticket — one branch — one MR. `build` opens the MR as a draft at the first step. Reviewers'
  findings and people's comments are visible as MR line comments; the source of truth is `findings.jsonl`.
  A human's comment in the MR or in Zed becomes a finding of kind `human-review`, and the builder must
  assess it.
- **CI.** The proof of a test is a record "command, output, commit, source". The source is CI or a local
  run. Both are acceptable, and the difference is visible in the ticket summary.
- **Tracker.** An adapter from day one: key, link, status sent outward. Jira is connected later.
- **Zed.** `stapel-lsp` for `ticket.md` and code (buttons, highlighting, hints), `inbox` in the status bar,
  Claude Code in the agent panel through ACP, Zed tasks and hotkeys as a quick start.
- **Claude Code hooks.** Installed by `init`: no writes to code before the build permit, no `git push` for
  agents, reviewers without write access to git.

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

## 10. Phases

In detail — in `docs/PHASES.md`.

```text
0  bootstrap         state core, sections, confirmations, RED→GREEN, findings, tokens        ~1 week
1  self-service      stapel runs its own tickets from new to ship                            ~2 weeks
2  index             language adapters, blast radius, MCP, context packages with a budget    ~2–3 weeks
3  Zed               LSP, inbox, status bar, guide                                           ~2 weeks
4  verification      3–5 klc tickets through stapel against klc; a decision on klc's fate    ~1–2 weeks
```
