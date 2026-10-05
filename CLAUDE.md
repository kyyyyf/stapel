# stapel — rules for Claude Code in this repository

## What this is

`stapel` carries a development ticket from description to merge with a repeatable result. Plan: `docs/PLAN.md`,
phases: `docs/PHASES.md`. Rust, one cargo workspace.

## How work is done here

- Every acceptance criterion has a test that fails first. Commits come in pairs: RED (tests only), then
  GREEN (code). One plan step is one pair.
- Review before merge: fresh code reviewer, external reviewer, drift reviewer. Reviewers work on a copy
  (`git archive`), with no write access to git or to the working tree.
- Every model call is recorded in the ticket's token journal with its role and the mark `measured` or
  `estimate`.
- Agents do not `git push` and do not switch branches in the working tree. The human, or the orchestrator
  after review, pushes.
- Tracked files and commit messages contain no private terms (organisation name, surnames). Commit author
  is `kyyyyf`.

## Before the build: catch defects early

Most review findings of STP-1 could have been caught before any code was written. So, for every ticket:

1. **Scope and risk tags first.** The ticket names its risk tags (`security`, `guard`, `data`, `migration`,
   `interface`). A tag makes items 2–5 mandatory; a ticket without tags may keep them to one line each.
2. **Study klc first.** Before writing `ticket.md`, the author studies the matching part of klc (`../klc`,
   list in `docs/PLAN.md` §7): how it solved the same problem, what its reviews found, which edge cases it
   handles. Findings go into the Design section: what we take, what we do differently, and why. klc code
   is not copied; ideas, data and found defects are.
3. **Spec questions from the coverage taxonomy.** Questions are checked against the ten categories of klc's
   `config/coverage-taxonomy.yml` (taken as data), including the adversarial/abuse sub-check. Tickets
   tagged `security` or `guard` have a **Guarantees** section: what is promised, against whom, and what is
   explicitly not promised. Unbounded wording ("any form", "always", "never" without a list) is not allowed
   in criteria. **Threat model by default:** the agent is careless, not malicious. A ticket promises
   protection against mistakes; deliberate circumvention (an agent that hides code from a check on purpose)
   is listed under "Not promised" unless the ticket is tagged `security` and says otherwise.
4. **External contract as facts.** Before the design, every claim about an external system (Claude Code,
   git, `gh`, the OS) is written down with its source and the mark `verified`, `read` or `assumption`.
5. **Test plan before code, by category:** main path; negative cases; abuse (a table of bypass attempts);
   robustness (size, depth, time, panics); environment (symlinks, case, encodings, `PATH`); repeated runs;
   integration with the real external system. Parsers get property tests (no panic, bounded time).
   The test plan has an **Inputs** table: every input the ticket adds (CLI flag or argument, file field,
   environment variable) with its type, smallest and largest value, empty value, invalid value and precision
   (for times: the unit), each row with its test. Sums of counts say what happens on overflow.
6. **Spec review before the build.** The external reviewer reads only `ticket.md` (spec, design, test plan)
   with one task: find how the ticket's promises can be broken. The task always includes two questions:
   what counts as the same thing (identity), and what happens on a repeat or a double entry; and what
   exactly each number or field taken from an external system means. The build starts after that review.
7. **Author self-check before code review,** against the abuse table of the test plan.
8. **Stage metric.** Each finding in `findings.jsonl` gets `catchable_at`: `spec`, `design`, `test-plan` or
   `code`, and `adversarial: true` when it needs deliberate circumvention beyond the ticket's threat model
   (item 3); adversarial findings are counted apart and do not enter the target. The ticket summary counts
   them. The target, first checked on STP-2: far fewer code-review
   findings than STP-1, and fewer than half of them `catchable_at` earlier than `code`. After the close, the
   orchestrator imports the ticket's measured token usage and compares two things with the previous ticket:
   tokens by role, and findings by stage and `catchable_at`, with the verdict "target met" or "not met" and
   the group that missed it.

9. **Every exact detail lives in one place, and a program checks the rest.** Acceptance criteria state
   observable behaviour (what a person or agent sees, exit codes, what is written to disk). Exact output
   lives in golden files under `crates/*/tests/golden/`, and a criterion names the file instead of retyping
   the text. Internal mechanics (temporary file names, the order of `fsync`, parser rules) belong in the
   Design section, marked "as built" and updated by the GREEN commit. The test
   `crates/stapel-cli/tests/ticket_drift.rs` runs with every `cargo test` and fails when a ticket names a
   test that does not exist, a test exists that no ticket names, an acceptance criterion quotes an output
   line (`label: text`) that is not in the code, or a ticket names a missing `crates/` or `docs/` path. The
   drift reviewer then reviews meaning, not wording. The same test also fails when a ticket tagged
   `security` or `guard` has no abuse table or no self-check record, when the test lists of the criteria,
   the Test plan and the Proof disagree, or when a CLI flag the ticket adds is missing from its Inputs table.
   A commit that changes a `Cargo.toml` contains the matching `Cargo.lock`.
10. **A spec change during the build is explicit.** When the code shows a criterion is wrong, the GREEN
    commit changes the ticket too and says `spec change: AC-x, <reason>` in its message; the confirmation of
    the spec goes stale (STP-2), so the person sees the diff in `stapel status`.
11. **Drift review after every GREEN step.** A cheap drift reviewer compares the step's diff with the ticket
    right after the GREEN commit; the "as built" Design and the criteria are fixed before the next step, so
    no gap between text and code piles up for the code review.

In phase 0 the orchestrator runs these checks by hand where no test does them; the tool takes them over from
STP-2/STP-4 on.

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
