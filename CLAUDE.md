# stapel — rules for Claude Code in this repository

## What this is

`stapel` carries a development ticket from description to merge with a repeatable result. Plan: `docs/PLAN.md`,
phases: `docs/PHASES.md`. Rust, one cargo workspace.

## How work is done here

- Every acceptance criterion has a test that fails first. Commits come in pairs: RED (tests only), then
  GREEN (code). One plan step is one pair. A test that passes before any code is a coverage test and goes
  in a commit of its own.
- Review before merge: fresh code reviewer, external reviewer, drift reviewer. Reviewers work on a copy
  (`git archive`), with no write access to git or to the working tree, and run no builds.
- Every model call is recorded in the ticket's token journal with its role and the mark `measured` or
  `estimate`.
- Agents do not `git push` and do not switch branches in the working tree. The human, or the orchestrator
  after review, pushes. Agents do not write files the guard denies by other means (a script, Bash); they
  hand such changes to the human as a short command.
- Tracked files and commit messages contain no private terms (organisation name, surnames). Commit author
  is `kyyyyf`.
- Heavy test runs use `cargo test -j 4 … -- --test-threads=4` at most; parallel nested builds crashed the
  machine (STP-4).

## The spec process

Goals, in this order of trade-off: complete enough to catch defects before code, simple enough that the
human reads what they confirm, and cheap in tokens. Introduced after STP-4 (2026-10-05); STP-5 is its pilot.
Rules that catch nothing are removed (item 9), so this list must not only grow.

1. **Frame first, one page.** The problem; risk tags (`security`, `guard`, `data`, `migration`,
   `interface`); at least one **non-goal**; the **promise** as a closed list: which inputs and constructions
   the ticket handles, and which it does not promise. The default threat model is a careless agent, not a
   malicious one; deliberate circumvention is not promised unless the ticket is tagged `security`. A size
   estimate: a ticket that outgrows its frame is split, not grown. The human confirms the frame.
2. **Questions before text.** The author asks the human questions from the coverage map — the ten
   categories of `.stapel/config/coverage-taxonomy.yml` and the dimensions of every external system the
   ticket reads, from `.stapel/config/external-states.yml` — one at a time or in a small group, each with
   options and a recommendation. Answers go into the ticket's **Decisions** log with their date; nothing is
   invented where the human has not answered. Prior art (klc, other tools) is studied for tagged tickets.
3. **Short criteria.** One behaviour per criterion, at most about 500 characters, written as
   `WHEN <event> THE <command> <does>` or `IF <unwanted condition> THEN …`, each naming its test. Exact
   output lives in golden files under `crates/*/tests/golden/`; internal mechanics go to Design "as
   built". The test plan has an **Inputs** table (type, bounds, empty, invalid, precision, test) and an
   **External states** table (each dimension: handled with a test, refused with its exit code and test, or
   not promised); both are filled from the catalogue, not from scratch, and a blank row is an error. After
   the test plan, a **Review Focus** line lists up to five input classes or failure modes no planned test
   exercises, each with a test or a refusal; "none" means checked.
4. **One spec review, in order.** A separate model first reads only the frame and the criteria and writes
   its own list of inputs, external states and bypass attempts; then it reads Design, the tables and the
   promise and compares. It returns a Clear/Partial/Missing map over the coverage categories and findings
   tagged **inside** the promise (defects) or **outside** it (proposals to widen it, not defects). A second
   round only when a finding is HIGH. The build starts after that review.
5. **The human confirms a summary.** Before each confirmation the orchestrator shows the decisions, the
   non-goals, the promise and the criteria changed since the last confirmation; the full text stays one
   command away (`stapel status` shows the diff). A spec change during the build is explicit: the GREEN
   commit says `spec change: AC-x, <reason>` and the confirmation goes stale.
6. **During the build.** Design "as built" is written as notes and moved into the ticket once, before code
   review, so the build permit is not lost at every step. A cheap drift review runs after a GREEN step only
   when the step touches a criterion of a tagged ticket; it gets the step's diff and its criteria, nothing
   else. A program checks the rest: `crates/stapel-cli/tests/ticket_drift.rs` fails when a ticket names a
   test that does not exist, a test exists that no ticket names, a criterion quotes an output line not in
   the code, a ticket names a missing path, a tagged ticket lacks its tables or a dated self-check, the
   test lists of criteria, Test plan and Proof disagree, or a CLI option is in no Inputs table. `stapel
   check` shows that every step's tests failed at RED and pass at GREEN and HEAD; `--locked` covers
   `Cargo.lock`.
7. **Code review without quotas.** Each reviewer reports every HIGH and MEDIUM finding and a short summary of
   LOW ones, with no fixed maximum. Each finding gets `catchable_at` (`spec`, `design`, `test-plan`,
   `code`), `promise` (`inside` or `outside`) and `rule`: the item of this process that should have caught
   it, or `none`.
8. **Measures after the close.** The orchestrator imports the measured tokens and compares with the
   previous ticket: **escaped defects** (found after the close: by `stapel check` on history, by later
   tickets, by the human) — the main measure; HIGH and MEDIUM inside-promise findings of code review; the
   size of `ticket.md`; tokens by role; wall time. The summary says what improved and what did not.
9. **Rules are pruned.** `rule` counts per item are kept across tickets in `.stapel/process-ledger.jsonl`. An
   item that caught nothing in three tickets in a row is removed or simplified, with the decision recorded
   there.

## Language

Every artifact is in English: documents, tickets and their sections, journal labels, `stapel.toml`, and every
message the `stapel` binary prints. Only conversation with the human in the chat is in Russian, in full
sentences. Code identifiers and commit messages are in English.

## Layout

```text
crates/stapel-core    tickets, state facts, section hashes, checks, decision log, the PreToolUse guard
crates/stapel-agent   model providers, roles, context packs, token journal
crates/stapel-index   index: language adapters, blast radius
crates/stapel-forge   MR/PR, CI, line comments
crates/stapel-mcp     MCP server for the agent
crates/stapel-lsp     LSP server for Zed
crates/stapel-cli     the `stapel` binary
docs/                 plan, phases, decisions
.stapel/              process and tickets of this repository
```
