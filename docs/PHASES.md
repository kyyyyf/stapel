# stapel — phase plan

Every phase is described the same way: goal, what is included, acceptance criteria, exit criterion, how the
work is run. The acceptance criteria here are deliberately short; the full criteria and tests will appear in
the `ticket.md` of each ticket. Time estimates are for a single maintainer who runs the work through agents.

## How the work is run in each phase

| Phase | Who is the orchestrator | Process |
|---|---|---|
| 0 | Claude Code in an interactive session | the same process we are building, but carried out by hand: a `ticket.md` per ticket, RED→GREEN commits, three reviewer agents on a `git archive` of the current commit, findings in a single format, a token journal from the agents' reports |
| 1 onward | `stapel` | every `stapel` ticket goes through `stapel` |

Rules in all phases: agents do not run `git push`; reviewers work only on a copy and without write access to
git; no private terms in tracked files and commit messages; for every acceptance criterion there is a test
that fails first.

---

## Phase 0 — bootstrap (about a week)

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
- STP-4 RED→GREEN check on the final commit.
- STP-5 accepting findings and `findings.jsonl`.
- STP-6 MR through `gh`/`glab`.

---

## Phase 1 — self-service (about two weeks)

**Goal.** `stapel` runs its own tickets from `new` to `ship` without a manual orchestrator.

**What is included.**

- `stapel-agent`: the Claude Code provider (headless `claude -p`, usage from JSON) and the Anthropic API
  provider; roles from `stapel.toml`; a context package for a role with a token budget.
- `stapel ask`: the author asks questions by the coverage map with a recommendation and an example; answers
  go to the decision log and into the spec; criteria checking: the form "who · what · with what · under what
  condition", vague words, "Out of scope"; every criterion gets a test stub.
- Design options and risks; every risk is a test; a choice with a reason.
- The step plan and `stapel build`: the builder goes through the steps; the tool checks RED→GREEN and the
  scope; a stop is a semantic check with a clear reason.
- `stapel review`: a fresh reviewer, an external reviewer, a drift reviewer on a `git archive`; the review
  map "required / desirable / can be skipped"; findings in the MR as line comments.
- `stapel ship`: all HIGH findings closed, confirmations current, CI or a local run green, no private terms
  found — merge and the ticket summary (time, tokens by role, rounds, what the review found).
- Three kinds of checks: a list of semantic ones, a list of self-fixing ones, the rest are advice.
- Choosing where the state lives: a separate branch `stapel-state` (default) or the ticket's branch.

**Acceptance criteria (abridged).**

1. A `stapel` ticket goes through `new → ask → ok → build → review → ship` on `stapel` itself; the human
   performs only decisions.
2. A criterion without a test stops `ok proof`, naming the criterion.
3. A change to the spec after confirmations makes them stale and stops `build` until `ok` is repeated;
   `build --at-own-risk` is allowed with a mark in the decision log.
4. A HIGH finding without a fate stops `ship`.
5. The ticket summary shows tokens by role, and no "estimate" number is presented as "measured".
6. Hooks: the builder agent cannot run `git push`; a reviewer cannot write to the working tree.

**Exit criterion.** Three `stapel` tickets in a row are taken through `stapel` without manual intervention
in the machine files.

---

## Phase 2 — index and context (two to three weeks)

**Goal.** The index answers the questions of the agent and the human and measurably saves tokens.

**What is included.**

- `stapel-index`: the core plus adapters for Python, TypeScript/JavaScript, Go, Rust. Layers: structure and
  file skeleton (tree-sitter), definitions and references (SCIP, a fallback on ripgrep marked "imprecise"),
  the test map (statically by imports and, where available, by coverage), history (git).
- Incremental update by file hash; the cache in `.stapel/index/`, not in git.
- `stapel q`: `symbol`, `refs`, `callers`, `tests`, `history`, `outline`, `impact`.
- Blast radius: for a set of symbols or files — who calls them, which tests, which past tickets and
  rollbacks; imprecise places are marked.
- Selective test run: `build` runs the tests from the map for the changed files, a full run before `ship`.
- `stapel-mcp`: the same queries as MCP tools for the agent.
- Context packages: the program assembles a role's context from index queries within a token budget; the
  composition of the package is recorded in `runs.jsonl`.
- Measuring the savings: the same ticket with and without the index.

**Acceptance criteria (abridged).**

1. On the `stapel` repository and on the klc repository, `impact` for a function returns calls and tests
   that match a manual check on a sample.
2. Imprecise answers (ripgrep) are marked as imprecise.
3. Re-indexing without changes does not re-read files; a change to one file updates only that file.
4. `build` on a ticket runs only the tests from the map and then a full run before `ship`; both results are
   in `runs.jsonl`.
5. On three tickets the token savings with the index against without it are measured and recorded in the
   summary.

**Exit criterion.** The builder agent on a `stapel` ticket uses MCP queries instead of reading whole files,
and this is visible in `tokens.jsonl`.

---

## Phase 3 — Zed (about two weeks)

**Goal.** All of a human's actions are available in Zed without a terminal.

**What is included.**

- `stapel inbox`: the queue of decisions across all tickets; output for the status bar.
- Zed tasks and hotkeys for `ok`, `ask`, `inbox`, the ticket diff — as a quick start.
- `stapel-lsp` for `ticket.md`: buttons above sections (confirm, return with a reason, choose an option, as
  recommended), diagnostics (stale, no test, vague word), hover hints.
- `stapel-lsp` for code: review findings above lines with the buttons "fix / do not fix + reason"; a human's
  comment by selection → a `human-review` finding.
- A Zed extension that starts the LSP and declares the `ticket.md` language.
- `stapel guide`: step-by-step procedures for live operations with output cross-checking and hashes before
  and after.
- Notifications: a Claude Code hook sends a system notification when the queue reaches a human.

**Acceptance criteria (abridged).**

1. Scenarios Z-1..Z-4 from the analysis run in Zed without terminal commands.
2. Each button calls exactly one `stapel` command, and the same command works from the terminal.
3. Diagnostics in the editor match what would stop `ok` or `ship`.
4. `guide` stops when the output differs from the expected one and writes the transcript into the ticket.

**Exit criterion.** One ticket is taken entirely from Zed by one person in all roles.

---

## Phase 4 — verification on klc (one to two weeks)

**Goal.** Compare `stapel` and klc on real tasks and decide klc's fate.

**What is included.**

- `stapel init` in the klc repository; the Python adapter already exists.
- Tickets KLC-147, KLC-148, KLC-152, KLC-163, KLC-170 (klc reliability defects) are taken through `stapel`.
- Tickets of similar scope go through klc.
- Comparison by the metrics from the overall plan: human hours, stops for no good reason, review rounds and
  defects, tokens by role, test minutes.

**Acceptance criteria (abridged).**

1. All five tickets are merged into klc through `stapel`, and their MRs contain review findings as comments.
2. The comparison table by metrics is published in `docs/` of both repositories.
3. A decision is recorded: freeze klc, carry over the remainder, or bring the lessons back into klc.

**Exit criterion.** The decision is made and recorded.

---

## What stays out of scope until phase 4

Jira (the adapter exists, the connection comes later), C++ (the adapter comes later), semantic search on
embeddings (after measuring the benefit), multi-user locks more complex than "the section is held by whoever
confirms", a separate observation stage (it exists as a "closed" record with no actions).
