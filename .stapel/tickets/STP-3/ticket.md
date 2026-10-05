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

Criteria state behaviour; exact output lives in golden files under the CLI tests' `golden` folder
(CLAUDE.md item 9). Times on the command line are `YYYY-MM-DDTHH:MM:SSZ`, the format `stapel` writes.

| № | Criterion | Test |
|---|---|---|
| AC-1 | `stapel ok` and `stapel close` · append one line to the ticket's `decisions.jsonl` after `state.json` is written: `{v: 1, id, at, by, action}` with `action` `ok` or `close`, plus `section`, the full `hash` and `via` for `ok`, and `reason` for `close`; two consecutive commands write different ids; nothing is appended when the command refuses; when the append fails, the command prints `warning: decision not recorded: <reason>` and exits 1, and the confirmation or closing stands. | `decisions::ok_and_close_append_a_decision`, `decisions::refusal_appends_nothing`, `decisions::append_failure_is_reported` |
| AC-2 | `stapel tokens add [KEY] --role <role> [--model <model>] (--input <n> --output <n> [--cache-read <n>] [--cache-write <n>] \| --estimate <n>) [--step <text>] [--note <text>]` · appends one line `{v: 1, id, at, ticket, role, model, source, …}`: `source` is `measured` with the given counts, or `estimate` with `estimate: <n>`; an option not given is absent from the line, and a given 0 is written as 0; `--model` defaults to the role's model in `stapel.toml` and is required for other roles; a role not in `stapel.toml` is accepted with `warning: role <role> is not in stapel.toml`; open and closed tickets both accept records (a record is a fact, not a decision); it refuses with exit 1 and appends nothing for a negative or non-integer number, input or output alone, both a measured count and `--estimate`, a role, model or step that is empty, over 256 bytes or contains a control character, a note over 4 KiB, or a legacy or unknown ticket. | `tokens::add_measured`, `tokens::add_estimate`, `tokens::add_refuses_bad_input`, `tokens::add_warns_on_unconfigured_role`, `tokens::add_accepts_closed_ticket` |
| AC-3 | `stapel tokens import <transcript.jsonl> [KEY] --role <role> [--since <time>] [--until <time>]` · sums, per model, the usage of the assistant messages of a Claude Code transcript (main or subagent file, sidechain lines included); a message is identified by `message.id` within the file, its time is the timestamp of its first line and its usage that of its last line, since output tokens grow across the lines of one message; a message counts when its time is in the half-open window `[since, until)`; lines marked `isApiErrorMessage` or with model `<synthetic>` are skipped, and so are unreadable lines, each counted in the output (`skipped: <n> error lines`, `skipped: <n> unreadable lines`); a model whose four sums are all zero gets no record. Each record carries `transcript` (the file's session id, plus the agent id for a subagent), `from` and `to` (the first and last counted message time) and `messages`. All records of one import are checked before any is appended: when every model's range equals an existing record of the same transcript and model, it adds nothing and says `already imported: <id>`; when any range overlaps one, it appends nothing, exits 1 and names the record with its `to`; a range whose `from` is after every existing `to` is appended. When the transcript was modified in the last 5 minutes, `--until` is required and must be at least 5 minutes before the time of the import, so a message still being written is not counted. Open and closed tickets both accept imports. | `tokens::import_sums_usage_per_message`, `tokens::import_skips_error_and_unreadable_lines`, `tokens::import_is_idempotent`, `tokens::import_refuses_overlap`, `tokens::import_appends_a_later_range`, `tokens::import_refuses_live_transcript_without_until`, `tokens::import_respects_window` |
| AC-4 | `stapel tokens [KEY]` · reads only `tokens.jsonl` files and prints, per ticket in key order, rows grouped by role and by the exact model string in name order (a transcript's dated model id and a configured id are two rows), with separate columns for measured input, output, cache read and cache write, for estimates, and for legacy records, which show one total (`total_tokens`, else input plus output) and their kind; an absent value prints `—` and a present 0 prints `0`; tickets without a journal are left out; problem lines (AC-5) are printed after the table and make the command exit 1; the layout is that of the golden files `tokens_report.txt` (one ticket) and `tokens_report_all.txt` (several, with the three real legacy shapes and a problem line). | `tokens::report_matches_golden`, `tokens::report_all_matches_golden`, `tokens::report_never_mixes_sources` |
| AC-5 | Both journals · are append-only on local file systems: a line is at most 64 KiB and is written with one `write` call in append mode, a short write is an error, and a newline is written first when the file does not end with one; on reading, empty lines are skipped, a line over 64 KiB is one problem and reading goes on at the next newline, and a line that does not parse is a problem; `stapel tokens` prints `problem: <file>:<line>` with the path relative to the repository root and exits 1, `stapel status` prints `warning: journal line <file>:<line> cannot be read` without changing its exit code, and no command rewrites or drops such a line; lines written before this ticket (no `v`) are read as `legacy`. | `journal::corrupt_line_is_reported_and_kept`, `journal::missing_newline_is_repaired_on_append`, `journal::legacy_lines_are_read`, `journal::concurrent_appends_keep_every_line`, `journal::status_warns_without_failing` |
| AC-6 | The `ticket_drift` check (CLAUDE.md item 9) · checks a closed ticket fully: every named test exists, a named module that is not a test file is drift, and every quoted output line and named `crates/` or `docs/` path exists. For an open ticket it checks each acceptance criterion as soon as all tests that criterion names exist (its GREEN commit has landed); until then that criterion is a plan. "Every test is named by some ticket" is checked always. | `ticket_drift::open_tickets_are_plans` |
| AC-7 | Test fixtures, golden files and test sources · contain no private data: a test fails when any file under the CLI tests' `fixtures` or `golden` folders, or any `crates/*/tests/*.rs`, contains `/home/`, `/Users/`, `C:\\Users\\`, an e-mail address (`@` followed by `<name>.<letters>`), or a term from the untracked `.stapel/config/private.toml` (which lists at least the local user name); without that file the test runs the other checks and prints a notice. Synthetic transcript fixtures keep only `type`, `timestamp`, `isSidechain`, `isApiErrorMessage`, `sessionId`, `agentId` and `message.id`, `.model`, `.usage`. | `fixtures::contain_no_private_data` |

### Questions

All answered on 2026-10-05 by the human; spec review decisions are marked.

1. **Import from Claude Code transcripts in this ticket.** Answer: as recommended (AC-3).
2. **How imported tokens are attributed to a ticket.** Explicit key and window only, no guessing. Answer: as
   recommended.
3. **What goes into `decisions.jsonl` now.** Confirmations and closings, written by `ok` and `close`;
   `stapel decide` with `ask` in phase 1. Answer: as recommended.
4. **Cost.** Answer: tokens only.
5. **Old hand-written records of STP-1 and STP-2.** Answer: keep them as they are; no import into STP-1 or
   STP-2 for now.
6. **A corrupt journal line** (spec review S-4). A warning in `status`, an error in `stapel tokens`; no
   repair command now. Decided by the orchestrator from the reviewer's recommendation; the human may change it.
8. **Model calls after close** (S2-5). `tokens add` and `tokens import` accept closed tickets; the review and
   closing calls are imported after the close. Decided as in 6.
7. **Roles outside `stapel.toml`** (S-5). Accepted with a warning; this repository's `stapel.toml` gains
   `research` and `translator` with the GREEN commit of step 4. Decided as in 6.

### Out of scope

- `stapel decide`, returns of a section with a reason, "won't fix because" (phase 1 and STP-5).
- Cost in money; budgets per role.
- Test results inside the confirmed Proof section (found at the close of STP-2): STP-4 keeps results in
  `runs.jsonl` and shows them through `status`.
- Guessing the ticket of a transcript; importing transcripts automatically; forked sessions that copy lines
  with the same message ids into another file (counted again in the new file).
- Network or 9p file systems (for example `/mnt/c` under WSL), where append-mode writes are not atomic.
- A command that marks a corrupt journal line as void.

## Design

### What klc does, and what we take

From a study of `../klc` (token journal entry `author-research`):

| klc | Take / do differently | Why |
|---|---|---|
| No decision log: picks in a rewritten `phase_history`, decisions as free-text admonitions with colliding ids; acks without who or why | One append-only `decisions.jsonl`, written only by the commands that make the decision | Principle 5 |
| One writer per store; record ids assigned by the writer; deterministic ids for imports | Take, with the covered range in the record | Re-imports are idempotent; growing files stay importable (S-1) |
| 196 of 207 token records store 0 for "not measured" | Absent for not given, 0 only when given or measured | AC-2, AC-4 |
| Sources `provider/transcript/signal/estimated`; reports never mix sources | Take, as `measured` and `estimate`, and legacy shown apart | Principle 8 |
| Transcript import: sum per `message.id`; skip transcripts written in the last 10 minutes | Take; a live file is fine when the window ends a minute before its last write (S-2) | Importing one's own session |
| Ticket inferred from the first key-shaped word | Explicit key | KLC-172 false matches |
| Journal compaction rewrote the file and dropped corrupt lines | Never rewrite; report and keep corrupt lines | KLC-119, KLC-133 |
| No schema version; old shapes guessed | `v: 1` on every line; a legacy mapping table | AC-5, below |
| Fixtures leaked a real local path and ids (KLC-133) | Synthetic fixtures and a private-data test | AC-7 |

### External contract (facts)

| Claim | Source | Mark |
|---|---|---|
| A session keeps a transcript at `~/.claude/projects/<project>/<session>.jsonl`; each subagent one at `<session>/subagents/agent-<id>.jsonl`, whose lines are all `isSidechain: true` and carry `agentId`, with a `.meta.json` holding `agentType` and `description` | this session's files, 2026-10-05 | verified |
| An assistant line holds `timestamp` (RFC 3339 with milliseconds, `Z`), `message.id`, `message.model` and `message.usage` with `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`; one message appears on several lines; in the main transcript all its lines carry the same usage, in subagent files output tokens grow across them (2 → 4618) and the last line carries the full count; the lines of one message span up to 101 s | this session's files; spec reviews S-2 and S2-1, S2-3 | verified |
| A resumed session appends to the same file; versions 2.1.285 and 2.1.287 wrote this session's file | this session's main transcript | verified |
| An API error appears as an assistant line with `isApiErrorMessage: true`, model `<synthetic>`, zero usage and a UUID id | this session's main transcript | verified |
| Transcript model ids may carry a date (`claude-haiku-4-5-20251001`) unlike `stapel.toml` | this session's subagent files | verified |
| Transcripts carry no cost; the main transcript is 9 MB and the longest line 119 KB | this session's files | verified |
| A single `write(2)` in append mode on a local file system is not interleaved with other appends | POSIX `write(2)` for regular files | read |

### Decisions

- **Formats.** One JSON object per line, `v: 1`, numbers as JSON integers, keys as in AC-1 to AC-3.
- **Legacy mapping** for lines without `v`: `ts` → `at`; `kind` `measured`/`estimate` → `source`;
  `input_tokens`/`output_tokens` → `input`/`output`; `total_tokens` → a total without a split; `null` →
  absent. Legacy is an origin flag; such rows show in the legacy column.
- **Writing.** Build the whole line with its newline; refuse it over 64 KiB; open in append mode; when the file
  is not empty and its last byte is not a newline, prefix one; write with one `write` call and treat a short
  write as an error.
- **Reading.** Journals line by line with the 64 KiB bound (a longer line is one problem, reading resumes at the
  next newline); transcripts streamed line by line with a 1 MiB per-line bound, no size limit for the file; empty
  lines are skipped in both.
- **Model ids.** Records keep the model string as given or as in the transcript; the report groups on the exact
  string, so a dated transcript id and a configured id are two rows.
- **Ids.** `d-` or `t-` plus 12 hex digits from 48 random bits of the OS generator for written records;
  imported records use `t-` plus 12 hex of SHA-256 over the transcript identity (session id, agent id),
  the model and the covered range.
- **Where.** `stapel-core` gets `journal` (append, read, legacy, problems) and `tokens` (records, import,
  report); `ok` and `close` call `journal::append`; the drift check reads `state.json` through the STP-2 code.

### Risks

| Risk | Test |
|---|---|
| R-1 Two writers at once lose or merge a line | `journal::concurrent_appends_keep_every_line`, `journal::missing_newline_is_repaired_on_append` |
| R-2 An estimate shown as a measurement, or a missing value shown as 0 | `tokens::report_never_mixes_sources`, `tokens::report_matches_golden` |
| R-3 Importing twice doubles the numbers, or a grown transcript loses its new part | `tokens::import_is_idempotent`, `tokens::import_refuses_overlap`, `tokens::import_appends_a_later_range` |
| R-4 A corrupt line loses data or blocks the ticket | `journal::corrupt_line_is_reported_and_kept`, `journal::status_warns_without_failing` |
| R-5 A private term ships in a fixture | `fixtures::contain_no_private_data` |

## Test plan

| Category | Cases | Tests |
|---|---|---|
| Main path | ok and close write decisions; add measured and estimate; import main and subagent fixtures; report one and all tickets | `decisions::ok_and_close_append_a_decision`, `tokens::add_measured`, `tokens::add_estimate`, `tokens::import_sums_usage_per_message`, `tokens::report_matches_golden`, `tokens::report_all_matches_golden` |
| Negative | refused ok writes no decision; append failure; bad numbers, input alone, both sources, long note, closed or legacy ticket; overlapping import; live transcript without an early `--until` | `decisions::refusal_appends_nothing`, `decisions::append_failure_is_reported`, `tokens::add_refuses_bad_input`, `tokens::import_refuses_overlap`, `tokens::import_refuses_live_transcript_without_until` |
| Abuse | the journals are machine files for the write tools (STP-1); a forged line is a record, not a proof | STP-1 `hook::denies_machine_file_writes_even_with_build` |
| Robustness | corrupt middle line, truncated last line without newline, a 70 KiB line, a 1.5 MiB transcript line, error lines, numbers as strings, booleans, negatives; property test: the transcript line parser and the time parser never panic and stay fast on arbitrary bytes | `journal::corrupt_line_is_reported_and_kept`, `journal::missing_newline_is_repaired_on_append`, `tokens::import_skips_error_and_unreadable_lines`, `tokens::add_refuses_bad_input`, `tokens::parsers_never_panic` |
| Environment | ten processes appending at once; non-ASCII notes; real legacy lines of STP-1, STP-2, STP-3 copied verbatim | `journal::concurrent_appends_keep_every_line`, `journal::legacy_lines_are_read` |
| Repeated runs | the same import twice; a grown transcript imported again | `tokens::import_is_idempotent`, `tokens::import_appends_a_later_range` |
| Integration | the real binary; synthetic transcript fixtures with only the keys the importer reads | all CLI tests, `fixtures::contain_no_private_data`, `fixtures::checker_finds_private_data` (the checker on known inputs) |

## Proof

Results come from the test runs recorded in `runs.jsonl` (STP-4 shows them in `status`); this table names
the tests only.

| Criterion or risk | Test |
|---|---|
| AC-1 | `decisions::ok_and_close_append_a_decision`, `decisions::refusal_appends_nothing`, `decisions::append_failure_is_reported` |
| AC-2 | `tokens::add_measured`, `tokens::add_estimate`, `tokens::add_refuses_bad_input`, `tokens::add_warns_on_unconfigured_role` |
| AC-3, R-3 | `tokens::import_sums_usage_per_message`, `tokens::import_skips_error_and_unreadable_lines`, `tokens::import_is_idempotent`, `tokens::import_refuses_overlap`, `tokens::import_appends_a_later_range`, `tokens::import_refuses_live_transcript_without_until`, `tokens::import_respects_window`, `tokens::parsers_never_panic` |
| AC-4, R-2 | `tokens::report_matches_golden`, `tokens::report_all_matches_golden`, `tokens::report_never_mixes_sources` |
| AC-5, R-1, R-4 | `journal::corrupt_line_is_reported_and_kept`, `journal::missing_newline_is_repaired_on_append`, `journal::legacy_lines_are_read`, `journal::concurrent_appends_keep_every_line`, `journal::status_warns_without_failing` |
| AC-6 | `ticket_drift::open_tickets_are_plans` |
| AC-7, R-5 | `fixtures::contain_no_private_data` |

## Plan

Each step is a pair of commits, RED then GREEN, with `cargo fmt` before RED. Check command:
`cargo test --workspace`, which includes the `ticket_drift` check. Until step 1 is built, that check fails on this
very draft: it names `ticket_drift::open_tickets_are_plans`, a test in an existing module that does not exist yet,
and quotes output that does not exist yet; this is expected and is what AC-6 changes.

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 1 | The drift check treats open tickets before a fresh proof as plans | AC-6 | this draft is flagged |
| 2 | `journal`: append, read, problems, legacy | AC-5 | module missing |
| 3 | Decisions from `ok` and `close`; PHASES note on what `decisions.jsonl` holds now | AC-1 | no decisions file |
| 4 | `stapel tokens add`; roles `orchestrator`, `research` and `translator` in this repository's `stapel.toml` | AC-2 | subcommand missing |
| 5 | `stapel tokens import` with synthetic fixtures; private-data test | AC-3, AC-7 | subcommand missing |
| 6 | `stapel tokens` report and its golden files | AC-4 | subcommand missing |
| 7 | After the close: import this session's transcripts for STP-3 only — the main transcript with role `orchestrator` and the window from the STP-3 creation commit to the closing commit (times from `TZ=UTC git log --date=format-local:%Y-%m-%dT%H:%M:%SZ`, `--until` at least 5 minutes before the import), and each subagent file of STP-3 with its role by `description` (klc study `research`, spec and code reviewers `external`, `reviewer`, `drift`); record the output in `runs.jsonl` | manual | — |

## Review

Spec review before the build by the external reviewer (round 1: 18 findings, round 2: 14 findings, applied); after the build, the three
reviewers. Each finding gets `catchable_at`.

## Summary

Generated after merge.
