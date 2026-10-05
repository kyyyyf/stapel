# STP-3 — Decision log and token journal as commands

## Description

Source: `docs/PHASES.md`, phase 0, third ticket; phase-0 criterion 6: "every model call recorded through
`stapel tokens add` is visible in `stapel tokens` with its role and mark". No external tracker.

Today the journals of STP-1 and STP-2 are written by hand and the orchestrator's own tokens are estimates
that turned out wrong by orders of magnitude. Claude Code keeps a transcript of every session and subagent
with the real `usage` of each model call. This ticket makes both journals tool-written: confirmations and
closings go into `decisions.jsonl` by themselves, model calls go into `tokens.jsonl` by `stapel tokens add`
or by importing a Claude Code transcript, and `stapel tokens` shows totals by role, never mixing measured
and estimated numbers.

**Risk tags:** `data` (two new line formats, append-only files several writers touch), `interface` (new
commands and their output). No `guard` change: both files are already denied to the write tools (STP-1).

## Spec

### Acceptance criteria

Criteria state behaviour; exact output lives in the golden files named here (CLAUDE.md item 9).

| № | Criterion | Test |
|---|---|---|
| AC-1 | `stapel ok` and `stapel close` · each append one line to the ticket's `decisions.jsonl`, after `state.json` is written: `{v: 1, id, at, by, action, section?, hash?, via?, reason?}`; `id` is unique within the file; nothing is appended when the command refuses. | `decisions::ok_and_close_append_a_decision`, `decisions::refusal_appends_nothing` |
| AC-2 | `stapel tokens add` · appends one line to the ticket's `tokens.jsonl`: `{v: 1, id, at, ticket, role, model, step?, source, input?, output?, cache_read?, cache_write?, note?}` with `source` = `measured` (at least input and output given) or `estimate` (only `--estimate <n>` given); an unknown value is absent, never 0; negative numbers, non-numbers, an unknown role (roles of `stapel.toml` plus `orchestrator`) or both measured and estimate options refuse with exit 1 and append nothing. | `tokens::add_measured`, `tokens::add_estimate`, `tokens::add_refuses_bad_input` |
| AC-3 | `stapel tokens import <transcript.jsonl> --ticket <KEY> --role <role> [--since <ts>] [--until <ts>]` · reads a Claude Code transcript, sums `message.usage` per `message.id` (the last line for an id wins) within the time window, and appends one `measured` record per model with input, output, cache read and cache write; the record id is derived from the transcript path, the window and the model, so importing the same thing twice adds nothing and says so; a transcript modified less than 60 seconds ago is refused as still being written. | `tokens::import_sums_usage_per_message`, `tokens::import_is_idempotent`, `tokens::import_refuses_live_transcript`, `tokens::import_respects_window` |
| AC-4 | `stapel tokens [KEY]` · prints totals by role and model for one ticket, or for all tickets without a key, with measured and estimated numbers in separate columns and records without input/output split counted apart, in the layout of the golden file `tokens_report.txt` under the CLI tests' `golden` folder; a column with no data prints `—`, never 0. | `tokens::report_matches_golden`, `tokens::report_never_mixes_sources` |
| AC-5 | Both journals · are append-only: a line is written with one `write` call in append mode; a corrupt or truncated line is reported by `stapel tokens` and by `stapel status` as `problem: <file>:<line>` and skipped, and no command ever rewrites or drops it; lines written before this ticket (no `v`) are read as `legacy` records and shown apart. | `journal::corrupt_line_is_reported_and_kept`, `journal::legacy_lines_are_read`, `journal::concurrent_appends_keep_every_line` |
| AC-6 | The `ticket_drift` check (CLAUDE.md item 9) · checks that named tests and `crates/` or `docs/` paths exist for closed tickets always, and for open tickets only when `STAPEL_DRIFT_ALL` is set (the orchestrator sets it before code review and before closing), since an open ticket names tests and files its build has yet to create; quoted output and "every test is named by some ticket" stay checked always. | `ticket_drift::open_tickets_are_plans` |

### Questions

Questions with a recommendation. Until answered, the build follows the recommendation.

1. **Import from Claude Code transcripts in this ticket.** It makes the orchestrator's and the subagents'
   tokens measured instead of estimated, which is the point of the phase-0 criterion. Recommendation: yes
   (AC-3). Answer: —
2. **How imported tokens are attributed to a ticket.** A session spans several tickets. Recommendation: by
   explicit `--ticket` and an optional time window only; no guessing from text (klc KLC-172 matched `AC-2`
   and `UTF-8` as ticket keys). Answer: —
3. **What goes into `decisions.jsonl` now.** Recommendation: confirmations and closings, written by `ok`
   and `close`. Answers to questions stay in the ticket text, which the spec confirmation already binds by
   hash; a separate `stapel decide` command comes with `ask` in phase 1. Answer: —
4. **Cost.** Transcripts carry no cost; computing it from a price table invents numbers. Recommendation:
   tokens only; cost comes with the API provider in phase 1, copied as the provider reports it. Answer: —
5. **Old hand-written records of STP-1 and STP-2.** Recommendation: keep them, read them as `legacy`, and
   in the last plan step import this session's transcripts so that STP-1..3 also get measured records.
   Answer: —

### Out of scope

- `stapel decide`, returns of a section with a reason, "won't fix because" (phase 1 and STP-5).
- Cost in money; budgets per role.
- Test results inside the confirmed Proof section (found at the close of STP-2): the RED→GREEN check of
  STP-4 keeps results in `runs.jsonl` and shows them through `status`.
- Guessing the ticket of a transcript; importing transcripts automatically.
- Locks between processes beyond append-mode single writes.

## Design

### What klc does, and what we take

From a study of `../klc` (token journal entry `author-research`):

| klc | Take / do differently | Why |
|---|---|---|
| No decision log: picks in a rewritten `phase_history`, decisions as free-text admonitions with colliding ids; acks without who or why | One append-only `decisions.jsonl`, written only by the commands that make the decision | Principle 5 |
| One writer per store; record ids assigned by the writer; deterministic ids for imports | Take | Re-imports are idempotent (AC-3) |
| 196 of 207 token records store 0 for "not measured" | Absent, never 0 (AC-2, AC-4) | Numbers that look measured but are not |
| Sources `provider/transcript/signal/estimated`; reports never mix sources | Take, as `measured` and `estimate` | Principle 8 |
| Transcript import: sum per `message.id`, last line wins; skip transcripts written in the last 10 minutes | Take; 60 seconds | Live transcripts freeze partial totals (KLC-172) |
| Ticket inferred from the first key-shaped word | Do differently: explicit `--ticket` | KLC-172 false matches |
| Journal compaction rewrote the file and dropped corrupt lines; exceptions swallowed while the source was deleted | Never rewrite; report and keep corrupt lines | Data loss (KLC-119, KLC-133) |
| No schema version; old shapes guessed | `v: 1` on every line; lines without it are `legacy` | AC-5 |

### External contract (facts)

| Claim | Source | Mark |
|---|---|---|
| A Claude Code session keeps a transcript at `~/.claude/projects/<project>/<session>.jsonl`, and each subagent one at `<session>/subagents/agent-<id>.jsonl` with a `.meta.json` holding `agentType` and `description` | this session's files, 2026-10-05 | verified |
| An assistant line holds `message.id`, `message.model` and `message.usage` with `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`; one message may appear on several lines | this session's files; klc `token_import.py` | verified |
| Transcripts carry no cost | klc study; this session's files | verified |
| Model lines may carry the model `<synthetic>` with zero usage | this session's main transcript | verified |
| `O_APPEND` writes of one buffer are not interleaved with other appends on a local file system | POSIX `write(2)` for regular files | read |

### Decisions

- **Formats.** One JSON object per line, `v: 1`, keys as in AC-1 and AC-2, numbers as JSON integers.
- **Writing.** Open with `append(true)`, build the whole line including `\n`, one `write_all`; no read
  before write, so concurrent writers never lose a line.
- **Reading.** Line by line; a line that is not a JSON object with `v: 1` and the required keys is `legacy`
  when it has no `v` and parses, else a problem with its line number.
- **Ids.** `d-<12 hex>` and `t-<12 hex>` from random bytes for written records; imported records use
  `t-` plus the first 12 hex of SHA-256 over the transcript path, the window and the model.
- **Where.** `stapel-core` gets `journal` (append, read, problems) and `tokens` (records, import, report);
  `ok` and `close` call `journal::append` for decisions.

### Risks

| Risk | Test |
|---|---|
| R-1 Two writers at once lose or merge a line | `journal::concurrent_appends_keep_every_line` |
| R-2 An estimate shown as a measurement, or unknown shown as 0 | `tokens::report_never_mixes_sources`, `tokens::report_matches_golden` |
| R-3 Importing twice doubles the numbers | `tokens::import_is_idempotent` |
| R-4 A corrupt line loses data | `journal::corrupt_line_is_reported_and_kept` |

## Test plan

| Category | Cases | Tests |
|---|---|---|
| Main path | ok and close write decisions; add measured and estimate; import a transcript; report by role | `decisions::ok_and_close_append_a_decision`, `tokens::add_measured`, `tokens::add_estimate`, `tokens::import_sums_usage_per_message`, `tokens::report_matches_golden` |
| Negative | refused ok writes no decision; bad numbers, unknown role, both sources; live transcript | `decisions::refusal_appends_nothing`, `tokens::add_refuses_bad_input`, `tokens::import_refuses_live_transcript` |
| Abuse | the journals are machine files for the write tools (STP-1); a hand-edited line with a forged `source` still shows as what it says — the journal is a record, not a proof | STP-1 `hook::denies_machine_file_writes_even_with_build` |
| Robustness | corrupt middle line, truncated last line, a 50 MiB transcript, numbers as strings, booleans, negatives | `journal::corrupt_line_is_reported_and_kept`, `tokens::import_sums_usage_per_message`, `tokens::add_refuses_bad_input` |
| Environment | ten processes appending at once; non-ASCII notes; legacy lines from STP-1 and STP-2 | `journal::concurrent_appends_keep_every_line`, `journal::legacy_lines_are_read` |
| Repeated runs | the same import twice | `tokens::import_is_idempotent` |
| Integration | the real binary; a transcript fixture cut from a real Claude Code transcript (no private text) | all CLI tests |

## Proof

| Criterion or risk | Test | Result |
|---|---|---|
| AC-1 | `decisions::ok_and_close_append_a_decision`, `decisions::refusal_appends_nothing` | see `runs.jsonl` |
| AC-2 | `tokens::add_measured`, `tokens::add_estimate`, `tokens::add_refuses_bad_input` | see `runs.jsonl` |
| AC-3, R-3 | `tokens::import_sums_usage_per_message`, `tokens::import_is_idempotent`, `tokens::import_refuses_live_transcript`, `tokens::import_respects_window` | see `runs.jsonl` |
| AC-4, R-2 | `tokens::report_matches_golden`, `tokens::report_never_mixes_sources` | see `runs.jsonl` |
| AC-5, R-1, R-4 | `journal::corrupt_line_is_reported_and_kept`, `journal::legacy_lines_are_read`, `journal::concurrent_appends_keep_every_line` | see `runs.jsonl` |
| AC-6 | `ticket_drift::open_tickets_are_plans` | see `runs.jsonl` |

## Plan

Each step is a pair of commits, RED then GREEN, with `cargo fmt` before RED. Check command:
`cargo test --workspace`, which includes the `ticket_drift` check. Until step 1 is built, that check fails on this
very draft (it names tests that do not exist yet); this is expected and is what AC-6 changes.

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 1 | The drift check treats open tickets as plans | AC-6 | this draft is flagged |
| 2 | `journal`: append, read, problems, legacy | AC-5 | module missing |
| 3 | Decisions from `ok` and `close` | AC-1 | no decisions file |
| 4 | `stapel tokens add` | AC-2 | subcommand missing |
| 5 | `stapel tokens import` | AC-3 | subcommand missing |
| 6 | `stapel tokens` report and its golden file | AC-4 | subcommand missing |
| 7 | Import this session's transcripts into STP-1, STP-2 and STP-3; record the output in `runs.jsonl` | manual | — |

## Review

Spec review before the build by the external reviewer; after the build, the three reviewers. Each finding gets
`catchable_at`.

## Summary

Generated after merge.
