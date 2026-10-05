# STP-5 — Measured token usage of subagents

## Description

Source: the close of STP-4 (Summary, "measurement caveat"); `docs/PHASES.md`, phase 0, STP-5. First ticket of
the spec process of `CLAUDE.md` after STP-4 (pilot).

### Frame

**Problem.** `stapel tokens import` takes the usage of each assistant message from its last transcript
line. In subagent transcripts most messages never get a final line: of 77 messages in the 14 subagent files
of STP-4, 8 have one (`stop_reason` set); the rest carry the usage of the first streamed chunk, so output is
4 or 5 tokens where the report was thousands. Input and cache counts of those lines look right; output and
thinking are lost. The STP-4 journal therefore shows 3.6k output tokens for all subagents.

Facts found while framing (2026-10-05, this machine):

- The main transcript's `cost-state` lines (written when a session ends or continues in a new file) hold
  per-model session totals — input, output, thinking, cache read, cache write, cost — that include
  subagents: the STP-1 to STP-3 session shows 197,599 output and 118,515 thinking tokens for the reviewer
  model `claude-fable-5-1`, against about 2,000 imported.
- The notice that ends a subagent carries `subagent_tokens`, which equals the context of its last call plus
  its final output (14 of 14 subagents of STP-4), not the sum over its calls.
- The current session has no `cost-state` line yet.

**Risk tags:** `data` (what a `measured` record means changes for subagents).

**Non-goals.** Cost in money; live metering while a session runs; providers other than Claude Code; changing
how Claude Code writes transcripts.

**Promise (closed list).** For Claude Code transcripts on this machine's format:

1. a subagent record never shows a streamed partial count as `measured`;
2. where only a total is known, it is recorded as such and marked, never split by guess;
3. session totals from `cost-state` lines can be imported per model and compared with the per-message
   records, so the gap is visible in `stapel tokens`.

Not promised: splitting a session total between tickets that share a session by time (only windows that
start and end at `cost-state` lines can be exact); usage of background calls no transcript holds.

**Size.** Small: one importer change, one report change, two or three criteria.

### Decisions

- 2026-10-05: the frame is confirmed by the human.
- 2026-10-05, questions answered as recommended: (1) a message without a final line keeps its input and
  cache as `measured` and loses its output, and the record says how many messages were partial; (2) session
  totals from `cost-state` go to one session journal, not to a ticket, and the report shows per session the
  total, the sum of message records of all tickets, and the gap; (3) a `cost-state` line is the same line
  when its session, `startTime` and `totalDuration` are the same; a later line of a session replaces an
  earlier one in the report, since its totals are cumulative; (4) older subagent records are not rewritten;
  the report recognises them and shows their output as partial.

## Spec

Criteria are short (`CLAUDE.md`, the spec process, item 3); exact output is in golden files.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF a message's last line in a transcript has no `stop_reason` THEN `stapel tokens import` adds its input and cache counts, adds nothing to the output of its model, and the record of that model carries `partial: <n>` (messages without a final line) and no `output` field. | `tokens::partial_messages_lose_only_their_output` |
| AC-2 | WHEN `stapel tokens import` writes a record THE record carries `importer: 2`; IF a v1 `measured` record has a subagent transcript identity (`<session>/<agent>`) and no `importer` field THEN `stapel tokens` shows its output as partial. | `tokens::older_subagent_records_show_partial_output` |
| AC-3 | WHEN `stapel tokens session <transcript>` runs THE command appends to `.stapel/sessions.jsonl` one record of the transcript's last `cost-state` line: `v: 1`, `id`, `at`, `session`, `start`, `duration_ms`, and per model `input`, `output`, `thinking`, `cache_read`, `cache_write`; IF that line is already recorded THEN it appends nothing and prints `already imported: <id>`; IF the transcript has no `cost-state` line THEN it exits 1 and appends nothing. | `tokens::session_totals_are_imported_once`, `tokens::session_without_cost_state_is_refused` |
| AC-4 | WHEN `stapel tokens` runs and `.stapel/sessions.jsonl` has records THE report ends with a sessions table: per session (the latest record of it) and model, the session's output plus thinking, the sum of the output of all tickets' records of that session, and the gap; partial records are counted and named in the row; the layout is that of the golden file `tokens_sessions.txt`. | `tokens::report_shows_session_gaps` |

## Design

### Facts (external contract and states)

| Claim | Source | Mark |
|---|---|---|
| Subagent transcripts keep, for most messages, only streamed lines without `stop_reason` and with the first chunk's `output_tokens`; 8 of 77 messages of STP-4's 14 subagent files have a final line | this machine, 2026-10-05 | verified |
| Main transcripts have a final line for every message (801 of 801 in two sessions) | same | verified |
| A `cost-state` line holds `sessionId`, `startTime` (ms), `totalDuration` (ms) and `modelUsage.<model>` with `inputTokens`, `outputTokens`, `thinkingTokens`, `cacheReadInputTokens`, `cacheCreationInputTokens`, `costUSD`; totals are cumulative over the session and include subagents | same (three lines of one session, growing) | verified |
| A `cost-state` line is written when a session continues in a new file or is resumed, not at every call; the running session has none | same | read |

External states (`.stapel/config/external-states.yml`, `claude-code`): transcripts of a session that
continued in another file (the `cost-state` line is in the old file: handled, the command takes any file);
a session resumed several times (several lines: the latest wins, AC-3); a compacted session (same file:
handled); permission modes, hooks and settings (not read: not applicable).

### Decisions (as built)

To be written once before code review (`CLAUDE.md`, item 6).

## Test plan

| Category | Cases | Tests |
|---|---|---|
| Main path | a subagent fixture with and without final lines; a session journal and the report | `tokens::partial_messages_lose_only_their_output`, `tokens::session_totals_are_imported_once`, `tokens::report_shows_session_gaps` |
| Negative | no `cost-state` line; an older subagent record | `tokens::session_without_cost_state_is_refused`, `tokens::older_subagent_records_show_partial_output` |
| Repeated runs | the same `cost-state` line twice; a later line of the same session | `tokens::session_totals_are_imported_once` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| transcript of `tokens session` | path | STP-3 bounds (16 MiB lines) | no `cost-state`: exit 1 | unreadable line: skipped and counted | — | `tokens::session_without_cost_state_is_refused` |
| `cost-state` counts | JSON integers | 0 / 10^15 | a missing model field: absent | non-integer: the line is unreadable | tokens | `tokens::session_totals_are_imported_once` |

**Review Focus.** Checked: a session file with a `cost-state` line but no messages (handled: totals, gap equals
totals); a model in `cost-state` that no ticket record has (handled: sum 0); `stop_reason` present but
`output_tokens` missing (as STP-3: absent field).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `tokens::partial_messages_lose_only_their_output` |
| AC-2 | `tokens::older_subagent_records_show_partial_output` |
| AC-3 | `tokens::session_totals_are_imported_once`, `tokens::session_without_cost_state_is_refused` |
| AC-4 | `tokens::report_shows_session_gaps` |

## Plan

Each step is a RED/GREEN pair; heavy runs with `-j 4`.

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 1 | Partial messages in import; `importer: 2`; older records shown partial; fixtures get final lines | AC-1, AC-2 | partial output counted |
| 2 | `stapel tokens session` and the session journal | AC-3 | subcommand missing |
| 3 | The sessions table in the report | AC-4 | no table |
| 4 | After the close: import this session's `cost-state` when it exists, and compare with the STP-4 records | manual | — |

## Review

One spec review in order (frame and criteria first); after the build, the three reviewers without quotas.

## Summary

After the close: escaped defects, HIGH and MEDIUM inside-promise findings, size, tokens, time.
