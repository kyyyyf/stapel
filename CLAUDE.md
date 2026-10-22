# stapel — rules for Claude Code in this repository

## What this is

`stapel` carries a development ticket from description to merge with a repeatable result. Plan: `docs/PLAN.md`,
phases: `docs/PHASES.md`. Rust, one cargo workspace.

## The process

The process (RED/GREEN, the spec process, build and review) is the skill `.claude/skills/stapel/SKILL.md`,
installed by `stapel init` with the agents under `.claude/agents/`. Change it in
`crates/stapel-cli/assets/` and run `stapel init`; `init_process::this_repository_has_the_current_files`
fails until the installed files match.

## Rules of this repository

- Tracked files and commit messages contain no private terms (organisation name, surnames). Commit author
  is `kyyyyf`.
- Heavy test runs use `cargo test -j 4 … -- --test-threads=4` at most, never two at once; parallel nested
  builds crashed the machine (STP-4).

## The plan

The plan is `docs/PHASES.md` (`docs/decisions/0004`). Tickets are taken in the order of its items; a ticket's
Frame names its plan item (`1.3`) and its promise stays inside that item. While a ticket builds, the frame and
spec of the next item may be written in a second git worktree, with no cargo, no builder and no `stapel ok`
there until the current ticket closes (ledger 2026-10-22). An item is added, moved, split or widened only by a
dated `decision` record in `.stapel/process-ledger.jsonl` that names the reason, made with the human in the
chat; an agent never does it on its own. A ticket gets its `STP-<n>` key from `stapel new` when it is taken;
the keys in the phase-0 list that have no folder are labels.

## Language

Every artifact is in English: documents, tickets and their sections, journal labels, `stapel.toml`, and every
message the `stapel` binary prints. Only conversation with the human in the chat is in Russian, in full
sentences. Code identifiers and commit messages are in English.

Chat replies follow ASD-STE100 (Simplified Technical English) about 80 % of the way, in Russian: one idea per
sentence, sentences of about 20 words at most, active voice, one term for one meaning every time, no idioms or
filler, steps as numbered lists with one action each, facts first and the request last. Code names, quotes and
needed technical terms stay as they are; a longer sentence is allowed when splitting it would hide a condition.

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
