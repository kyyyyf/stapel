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
| AC-1 | IF a message's last line in a transcript has no `stop_reason` THEN `stapel tokens import` counts its input and cache, adds nothing to its model's output, and the record of that model carries `partial: <n>` (such messages, written only when n > 0); the record's `output` is the sum over the messages that have a final line, and is absent only when none has one; the report shows a partial output as a lower bound (`≥<n>`). `<synthetic>` and API error messages stay skipped (STP-3). | `tokens::partial_messages_lose_only_their_output` |
| AC-2 | WHEN `stapel tokens import` writes a record THE record carries `importer: 2`; IF a v1 `measured` record has a subagent transcript identity (`<session>/<agent>`) and no `importer` field THEN `stapel tokens` shows its output as partial. | `tokens::older_subagent_records_show_partial_output` |
| AC-3 | WHEN `stapel tokens session <transcript>` runs THE command appends to `.stapel/sessions.jsonl` one record of the transcript's last `cost-state` line: `v: 1`, `id` (`s-` and 12 hex of SHA-256 over session, `start`, `duration_ms`), `at` (the import time), `session`, `start`, `duration_ms`, and per model `input`, `output`, `thinking`, `cache_read`, `cache_write`; IF that line is already recorded THEN it appends nothing, prints `already imported: <id>` and exits 0; IF the transcript has no `cost-state` line, or its last line that starts as one cannot be read, THEN it exits 1, names the line, and appends nothing. | `tokens::session_totals_are_imported_once`, `tokens::session_without_cost_state_is_refused`, `tokens::truncated_cost_state_is_refused` |
| AC-4 | WHEN `stapel tokens` runs and `.stapel/sessions.jsonl` has records THE report ends with a sessions table: per session (its record with the greatest `start + duration_ms`; records are deduplicated by `id`) and model, the session's output, its thinking (part of the output, shown apart), the sum of the output of all tickets' `measured` records whose transcript identity equals the session or starts with `<session>/` (each session id stands alone: a continuation is another session), the count of partial records (AC-1 and AC-2 alike) and of duplicates (two records with one transcript identity and model whose `from`..`to` ranges overlap; one of them counts — the one with `importer: 2`, else the later `at` — while records with disjoint windows all count), and the signed gap (session minus records); sums saturate; unreadable lines are skipped and named after the table; the layout is that of the golden file `tokens_sessions.txt`. | `tokens::report_shows_session_gaps`, `tokens::report_counts_a_duplicate_import_once` |

## Design

### Facts (external contract and states)

| Claim | Source | Mark |
|---|---|---|
| Subagent transcripts keep, for most messages, only streamed lines without `stop_reason` and with the first chunk's `output_tokens`; 8 of 77 messages of STP-4's 14 subagent files have a final line | this machine, 2026-10-05 | verified |
| Main transcripts have a final line for every message (801 of 801 in two sessions) | same | verified |
| A `cost-state` line holds `sessionId`, `startTime` (ms), `totalDuration` (ms) and `modelUsage.<model>` with `inputTokens`, `outputTokens`, `thinkingTokens`, `cacheReadInputTokens`, `cacheCreationInputTokens`, `costUSD`; totals are cumulative over the session and include subagents | same (three lines of one session, growing) | verified |
| `outputTokens` includes `thinkingTokens`: between two lines of one session the main transcript's message output was 368,947, the line delta 393,927 and the thinking delta 95,633, so thinking is not on top | same | verified (from deltas) |
| A session that continues in a new file gets a new `sessionId` there (the continuation of `d8463ea3` writes `104ede7a`), and ticket records carry that id as the first part of their transcript identity | same | verified |
| Whether the continuation's `cost-state` totals start from zero or carry the predecessor's | — | assumption; checked at plan step 4, and until then each session id stands alone |
| A `cost-state` line is written when a session continues in a new file or is resumed, not at every call; the running session has none | same | read |

External states (`.stapel/config/external-states.yml`, `claude-code`): transcripts of a session that
continued in another file (the `cost-state` line is in the old file: handled, the command takes any file);
a session resumed several times (several lines: the latest wins, AC-3); a compacted session (same file:
handled); permission modes, hooks and settings (not read: not applicable).

### Decisions (as built)

- Step 1: `stapel-core::tokens` marks a message `complete` when its last line has a string `stop_reason`; a later line of the same message replaces usage and completeness. `tokens import` counts input and cache of every message, output only of complete ones; `partial` is the number of incomplete messages (written when > 0); `output` is absent when no message of the model is complete. The report's row output shows `≥<sum>` when any of its records is partial (field `partial`, or a subagent identity without `importer`).
- Step 2: `stapel-core::tokens::last_cost_state` streams the transcript (16 MiB lines); a line counts as a cost-state line when its first 200 characters, without whitespace, contain `"type":"cost-state"`; the last such line must parse with `sessionId`, `startTime`, `totalDuration` and `modelUsage`, else exit 1 naming its line number. `tokens session` writes `models.<model>` with `input`, `output`, `thinking`, `cache_read`, `cache_write` (absent when the line lacks them); `id` = `s-` + 12 hex of SHA-256 over session, start and duration with NUL separators; the journal is `.stapel/sessions.jsonl` through the STP-3 journal.
- Step 3: the sessions table is printed only for the whole report (`stapel tokens` without a key), since one ticket's records cannot be compared with a session total; the measured records of all tickets are collected while the ticket blocks are built; a sessions line without `session`, `start`, `duration_ms` or a `models` object, or not `v: 1`, is a problem line; models are the union of the session's and the records'; duplicates: records of one transcript identity and model whose `from`..`to` overlap, the one with `importer` (then the later `at`) kept; the gap is signed (`i128`); the table uses the ticket table's column widths. Step 1 drift review D1 ("≥0" when no message of a model is complete) matches the Inputs row of AC-2 and is kept.

## Test plan

| Category | Cases | Tests |
|---|---|---|
| Main path | a subagent fixture with and without final lines; a session journal and the report | `tokens::partial_messages_lose_only_their_output`, `tokens::session_totals_are_imported_once`, `tokens::report_shows_session_gaps` |
| Negative | no `cost-state` line; an older subagent record | `tokens::session_without_cost_state_is_refused`, `tokens::older_subagent_records_show_partial_output` |
| Repeated runs | the same `cost-state` line twice; a later line of the same session imported before an earlier one; one transcript in two tickets with disjoint windows (summed) and with overlapping windows (counted once) | `tokens::session_totals_are_imported_once`, `tokens::report_counts_a_duplicate_import_once` |
| Robustness | a half-written last `cost-state` line; a negative gap | `tokens::truncated_cost_state_is_refused`, `tokens::report_shows_session_gaps` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `.stapel/sessions.jsonl` lines | JSON, `v: 1` | STP-3 journal bounds | no file: no sessions table | unreadable or unknown `v`: skipped and named; a duplicate `id`: counted once | — | `tokens::report_shows_session_gaps` |
| `partial` and `importer` of a ticket record | integers | `partial` 1 / 10^15 (0 is never written) | absent: complete (or partial by AC-2); `partial` without `output` shows `≥0` | not an integer: the line is a problem (STP-3) | messages | `tokens::older_subagent_records_show_partial_output` |
| transcript of `tokens session` | path | STP-3 bounds (16 MiB lines) | no `cost-state`: exit 1 | an unreadable line before the last `cost-state` line is skipped; the last one unreadable, or lacking `sessionId`, `startTime` or `totalDuration`, refuses (AC-3) | — | `tokens::session_without_cost_state_is_refused` |
| `cost-state` counts | JSON integers | 0 / 10^15 | a missing model field: absent | non-integer: the line is unreadable | tokens | `tokens::session_totals_are_imported_once` |

**Review Focus.** Checked: two `tokens session` runs at once (both append through the single-write journal; the report dedupes by `id`); a session file with a `cost-state` line but no messages (handled: totals, gap equals
totals); a model in `cost-state` that no ticket record has (handled: sum 0); `stop_reason` present but
`output_tokens` missing (as STP-3: absent field).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `tokens::partial_messages_lose_only_their_output` |
| AC-2 | `tokens::older_subagent_records_show_partial_output` |
| AC-3 | `tokens::session_totals_are_imported_once`, `tokens::session_without_cost_state_is_refused`, `tokens::truncated_cost_state_is_refused` |
| AC-4 | `tokens::report_shows_session_gaps`, `tokens::report_counts_a_duplicate_import_once` |

## Plan

Each step is a RED/GREEN pair; heavy runs with `-j 4`.

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 1 | Partial messages in import; `importer: 2`; older records shown partial; fixtures get final lines | AC-1, AC-2 | partial output counted |
| 2 | `stapel tokens session` and the session journal | AC-3 | subcommand missing |
| 3 | The sessions table in the report | AC-4 | no table |
| 4 | After the close: import this session's `cost-state` when it exists, and compare with the STP-4 records | manual | — |

## Review

Spec review in order (2026-10-05): round 1, all eight HIGH and MEDIUM findings from the first pass, before the
author's tables, all inside the promise, applied; round 2, six closed, S-3 and S-8 partly, one new HIGH
(duplicates by window), applied. After the build, the three reviewers without quotas.

## Summary

After the close: escaped defects, HIGH and MEDIUM inside-promise findings, size, tokens, time.
