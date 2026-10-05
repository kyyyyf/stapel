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
   in criteria.
4. **External contract as facts.** Before the design, every claim about an external system (Claude Code,
   git, `gh`, the OS) is written down with its source and the mark `verified`, `read` or `assumption`.
5. **Test plan before code, by category:** main path; negative cases; abuse (a table of bypass attempts);
   robustness (size, depth, time, panics); environment (symlinks, case, encodings, `PATH`); repeated runs;
   integration with the real external system. Parsers get property tests (no panic, bounded time).
6. **Spec review before the build.** The external reviewer reads only `ticket.md` (spec, design, test plan)
   with one task: find how the ticket's promises can be broken. The build starts after that review.
7. **Author self-check before code review,** against the abuse table of the test plan.
8. **Stage metric.** Each finding in `findings.jsonl` gets `catchable_at`: `spec`, `design`, `test-plan` or
   `code`. The ticket summary counts them. The target, first checked on STP-2: far fewer code-review
   findings than STP-1, and fewer than half of them `catchable_at` earlier than `code`.

9. **Every exact detail lives in one place, and a program checks the rest.** Acceptance criteria state
   observable behaviour (what a person or agent sees, exit codes, what is written to disk). Exact output
   lives in golden files under `crates/*/tests/golden/`, and a criterion names the file instead of retyping
   the text. Internal mechanics (temporary file names, the order of `fsync`, parser rules) belong in the
   Design section, marked "as built" and updated by the GREEN commit. The test
   `crates/stapel-cli/tests/ticket_drift.rs` runs with every `cargo test` and fails when a ticket names a
   test that does not exist, a test exists that no ticket names, an acceptance criterion quotes an output
   line (`label: text`) that is not in the code, or a ticket names a missing `crates/` or `docs/` path. The
   drift reviewer then reviews meaning, not wording.
10. **A spec change during the build is explicit.** When the code shows a criterion is wrong, the GREEN
    commit changes the ticket too and says `spec change: AC-x, <reason>` in its message; the confirmation of
    the spec goes stale (STP-2), so the person sees the diff in `stapel status`.

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
