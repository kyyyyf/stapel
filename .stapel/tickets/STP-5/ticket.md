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

1. `stapel tokens` never shows a streamed partial count as a complete one (older journal lines keep
   `source: measured`; the report marks them, by a rule that also marks the few complete ones);
2. where only a total is known, it is recorded as such and marked, never split by guess;
3. session totals from `cost-state` lines can be imported per model and compared with the per-message
   records, so the gap is visible in `stapel tokens`.

Not promised: splitting a session total between tickets that share a session by time (only windows that
start and end at `cost-state` lines can be exact); usage of background calls no transcript holds.

**Size.** Framed as small (two or three criteria); it grew to a new subcommand, a session journal and a
table (eight criteria after code review). The growth is recorded under Decisions, not split, because the
table is the only way to see the gap the problem is about.

### Decisions

- 2026-10-05: the frame is confirmed by the human.
- 2026-10-05, questions answered as recommended: (1) a message without a final line keeps its input and
  cache as `measured` and loses its output, and the record says how many messages were partial; (2) session
  totals from `cost-state` go to one session journal, not to a ticket, and the report shows per session the
  total, the sum of message records of all tickets, and the gap; (3) a `cost-state` line is the same line
  when its session, `startTime` and `totalDuration` are the same; a later line of a session replaces an
  earlier one in the report, since its totals are cumulative; (4) older subagent records are not rewritten;
  the report recognises them and shows their output as partial.
- 2026-10-06, after code review round 1 (17 findings): every unknown or inexact number is marked, never shown
  as exact (absent session output: `—`; partial records: `≥` and `≤`); a half-written last line refuses;
  criteria split to one behaviour each; the gap of a continued session is marked as possibly including its
  predecessor's totals. Agreed in principle by the human with the build of this process; confirmed with the
  next confirmation.

## Spec

Criteria are short (`CLAUDE.md`, the spec process, item 3); exact output is in golden files.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF a message's last line has no `stop_reason` THEN `stapel tokens import` counts its input and cache but not its output; the record carries `partial: <n>`, its `output` sums the complete messages and is absent when none is complete, and the import line says `partial: <n>`. | `tokens::partial_messages_lose_only_their_output` |
| AC-2 | IF a line without `stop_reason` follows a final line of the same message THEN the final line's usage stays. | `tokens::final_line_is_kept` |
| AC-3 | WHEN `stapel tokens` shows a record with `partial` of 1 or more, or a subagent record (`<session>/<agent>`) without `importer: 2` THE output reads `≥<n>`; records of this importer carry `importer: 2`; IF `partial` or `importer` has another type or value THEN the line is a problem. | `tokens::older_subagent_records_show_partial_output`, `tokens::bad_partial_and_importer_values_are_problems` |
| AC-4 | WHEN `stapel tokens session <transcript>` runs THE command appends one record of the transcript's last `cost-state` line to `.stapel/sessions.jsonl`; IF that line is recorded already THEN it prints `already imported: <id>`, appends nothing and exits 0. | `tokens::session_totals_are_imported_once` |
| AC-5 | IF the transcript has no `cost-state` line, its last non-empty line is not complete JSON, or its last `cost-state` line lacks `sessionId`, `startTime`, `totalDuration` or `modelUsage` or holds a count that is not a non-negative integer THEN `tokens session` exits 1, names the line and appends nothing. | `tokens::session_without_cost_state_is_refused`, `tokens::truncated_cost_state_is_refused`, `tokens::cost_state_with_bad_counts_is_refused` |
| AC-6 | WHEN `stapel tokens` runs without a key and sessions are recorded THE report ends with the sessions table of the golden file `tokens_sessions.txt`: per session and model its output, thinking, the records' output, partial and duplicate counts and the gap; the gap is `—` without a session output and `≤<n>` when a record is partial; a note says a continued session's gap may include its predecessor's totals. | `tokens::report_shows_session_gaps` |
| AC-7 | WHEN two records of one transcript and model have overlapping windows THE table counts one of them and shows the other as a duplicate; records with disjoint windows all count. | `tokens::report_counts_a_duplicate_import_once` |
| AC-8 | IF a sessions line is unreadable, not `v: 1`, or lacks `id`, `session`, `start`, `duration_ms` or `models` THEN the report skips it and prints `problem: .stapel/sessions.jsonl:<n>` after the table; a repeated `id` counts once; `stapel tokens <KEY>` shows no sessions table. | `tokens::sessions_journal_problems_are_named` |

## Design

### Facts (external contract and states)

| Claim | Source | Mark |
|---|---|---|
| Subagent transcripts keep, for most messages, only streamed lines without `stop_reason` and with the first chunk's `output_tokens`; 8 of 77 messages of STP-4's 14 subagent files have a final line | this machine, 2026-10-05 | verified |
| Main transcripts have a final line for every message (801 of 801 in two sessions) | same | verified |
| A `cost-state` line holds `sessionId`, `startTime` (ms), `totalDuration` (ms) and `modelUsage.<model>` with `inputTokens`, `outputTokens`, `thinkingTokens`, `cacheReadInputTokens`, `cacheCreationInputTokens`, `costUSD`; totals are cumulative over the session and include subagents | same (three lines of one session, growing) | verified |
| `outputTokens` includes `thinkingTokens`: between two lines of one session the main transcript's message output was 368,947, the line delta 393,927 and the thinking delta 95,633, so thinking is not on top | same | verified (from deltas) |
| A session that continues in a new file gets a new `sessionId` there (the continuation of `d8463ea3` writes `104ede7a`), and ticket records carry that id as the first part of their transcript identity | same | verified |
| Lines of one session written at its resumes keep `startTime` and a growing `totalDuration` (8,766,812, 8,871,849, 248,039,497 ms in `d8463ea3`), so start plus duration orders them | same | verified |
| Whether a continuation's `cost-state` totals start from zero or carry the predecessor's, and which `sessionId` that line names | — | assumption; the report marks the gap (AC-6) and plan step 4 checks it |
| `modelUsage` keys equal the `message.model` strings of the same session (the four keys of `d8463ea3` match its message models) | same | verified |
| A `cost-state` line is written when a session continues in a new file or is resumed, not at every call; the running session has none | same | read |

External states (`.stapel/config/external-states.yml`):

| System, dimension | Handling | Test |
|---|---|---|
| claude-code: transcripts — streamed lines of one message | the last line decides; a later partial line does not undo a final one | `tokens::partial_messages_lose_only_their_output`, `tokens::final_line_is_kept` |
| claude-code: transcripts — resumed session (several `cost-state` lines) | the latest by start plus duration | `tokens::report_shows_session_gaps` |
| claude-code: transcripts — continued in a new file | another session id; its gap is marked | `tokens::report_shows_session_gaps` |
| claude-code: transcripts — compaction | same file, same rules | not tested (no format change seen) |
| claude-code: permission modes, hook environment, settings | not read | not applicable |
| os: a transcript path that is a directory, unreadable or a symlink | the OS error, exit 1 (STP-3 reader) | not promised |
| os: two `tokens session` runs at once | both may append the same line; the report counts an `id` once | `tokens::sessions_journal_problems_are_named` |

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
| Robustness | a half-written last line (inside the type key and inside the counts); non-integer counts; a negative gap; a later partial line after a final line; bad `partial`, `importer` and sessions lines | `tokens::cost_state_with_bad_counts_is_refused`, `tokens::final_line_is_kept`, `tokens::bad_partial_and_importer_values_are_problems`, `tokens::sessions_journal_problems_are_named`, `tokens::truncated_cost_state_is_refused`, `tokens::report_shows_session_gaps` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `.stapel/sessions.jsonl` lines | JSON, `v: 1` | STP-3 journal bounds | no file: no sessions table | unreadable, unknown `v` or a missing field: skipped and named; a duplicate `id`: counted once | — | `tokens::sessions_journal_problems_are_named` |
| `partial` and `importer` of a ticket record | integers | `partial` 1 / u64; `importer` 2 | absent: complete (or partial by AC-3); `partial` without `output` shows `≥0` | another type or value: the line is a problem | messages | `tokens::bad_partial_and_importer_values_are_problems` |
| transcript of `tokens session` | path | STP-3 bounds (16 MiB lines) | no `cost-state`: exit 1 | an unreadable line before the last `cost-state` line is skipped; the last one unreadable, or lacking `sessionId`, `startTime` or `totalDuration`, refuses (AC-3) | — | `tokens::session_without_cost_state_is_refused` |
| `cost-state` counts | JSON integers | 0 / u64, sums saturate | a missing field: absent | not a non-negative integer: refused (AC-5) | tokens | `tokens::cost_state_with_bad_counts_is_refused` |

**Review Focus.** Checked: a model in the records but not in the session line (gap `—`, `tokens::report_shows_session_gaps`); a session line with no records (sum 0, same test); two runs at once (External states); a complete message without `output_tokens` (its output field absent, as STP-3; AC-1 sums only present counts).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `tokens::partial_messages_lose_only_their_output` |
| AC-2 | `tokens::final_line_is_kept` |
| AC-3 | `tokens::older_subagent_records_show_partial_output`, `tokens::bad_partial_and_importer_values_are_problems` |
| AC-4 | `tokens::session_totals_are_imported_once` |
| AC-5 | `tokens::session_without_cost_state_is_refused`, `tokens::truncated_cost_state_is_refused`, `tokens::cost_state_with_bad_counts_is_refused` |
| AC-6 | `tokens::report_shows_session_gaps` |
| AC-7 | `tokens::report_counts_a_duplicate_import_once` |
| AC-8 | `tokens::sessions_journal_problems_are_named` |

## Plan

Each step is a RED/GREEN pair; heavy runs with `-j 4`.

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 1 | Partial messages in import; `importer: 2`; older records shown partial; fixtures get final lines | AC-1, AC-3 | partial output counted |
| 2 | `stapel tokens session` and the session journal | AC-4, AC-5 | subcommand missing |
| 3 | The sessions table in the report | AC-6, AC-7, AC-8 | no table |
| R1 | Code review round 1: marked unknowns, refusals, sticky final lines, problem lines, split criteria | AC-1 to AC-8 | see `findings.jsonl` |
| 4 | After the close: import this session's `cost-state` when it exists, and compare with the STP-4 records | manual | — |

## Review

Spec review in order (2026-10-05): round 1, all eight HIGH and MEDIUM findings from the first pass, before the
author's tables, all inside the promise, applied; round 2, six closed, S-3 and S-8 partly, one new HIGH
(duplicates by window), applied. After the build, the three reviewers without quotas.

## Summary

After the close: escaped defects, HIGH and MEDIUM inside-promise findings, size, tokens, time.
