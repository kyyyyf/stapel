# STP-29 — attention numbers per ticket

## Description

Source: `docs/PHASES.md`, stage A item A3 (plan item 1.14); ledger decisions 2026-11-10 and 2026-11-11; the
investigation and the answers in the chat of 2026-11-11.

### Frame

**Problem.** The pilot's gate compares the human's minutes per ticket with a plain agent, and stapel counts
only tokens; the main transcript of Claude Code holds the times and the permission source of every tool
result. Also, 312 of the 545 token records of STP-27 are empty records for subagent transcripts that never
existed. The measurements are in Design, F-1.

**Plan item:** A3 (1.14).

**Risk tags:** `data` (a new record kind in `tokens.jsonl`), `interface` (a new command, new report lines).

**Non-goals.** The count of questions that concern the feature, not the ticket (counted by hand in the
pilot's summary; the human, 2026-11-11); a merge request time (`ship` is not in the plan: wall time ends at
the person's last event); dialogs and questions of subagents; a display in the editor.

**Promise (closed list).**

1. Attention events from a main transcript: a question, a dialog the person answered, a typed message;
   each with its wait.
2. Two times: attention (each wait up to 10 minutes) and waited (in full, with the count of longer waits);
   waits that overlap count once.
3. The hooks `stop` and `session-end` append one attention record per call to the ticket's `tokens.jsonl`;
   a repeated call adds nothing.
4. `stapel tokens KEY` shows the attention numbers, the Plan steps and the wall time under the token
   table, which is unchanged.
5. `stapel tokens measure FILE` prints tokens and attention numbers of a main transcript and its subagent
   files, with no ticket and no write.
6. The `subagent-stop` hook writes no record for a transcript file that does not exist.
7. An event made in a ticket's worktree goes to that ticket: one call's events are split by the worktree
   folder of each event's line (SR1-11, widened by the human on 2026-11-11).

Not promised: another transcript format than Claude Code 2.1 (unknown lines are skipped); attention outside
the chat; a wait that ends in another session; sessions that ended before this ticket; a session that
serves several tickets in one folder (its events go to the ticket current at the hook call).

**Size.** About 500 source lines and 800 test lines; 9 RED and GREEN pairs and 3 coverage commits (12 Plan
steps, the limit). Route: full (tags), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-11: Decision (the human's minutes, the human, recommended): both numbers side by side: attention,
  each wait counted up to 10 minutes, and waited, the raw sum with the count of longer waits. Rejected: only
  the capped number (the idle time is hidden); only the raw number (nights count as attention).
- 2026-11-11: Decision (the plain agent, the human, recommended): a command that measures any transcript
  without a ticket, so both ways of the pilot are measured by one code. Rejected: a one-off script outside
  the ticket (two counters).
- 2026-11-11: Decision (feature questions, the human, recommended): counted by hand in the pilot's summary
  from each ticket's Decisions log. Rejected: a tag in the Decisions log counted by stapel (a new skill rule,
  and the plain agent is counted by hand anyway).
- 2026-11-11: Assumption (the orchestrator): the wall time of a ticket runs from the first to the last
  event of the person in its attention records; the wait of a typed message runs from the latest earlier
  `assistant` line.
- 2026-11-11: Decision (events of several tickets in one session, the human; SR1-11): split in this ticket,
  not listed as "not promised". How (the orchestrator): by the worktree folder in the `cwd` of each event's
  line, because every ticket has its own worktree since STP-27 and no file stores when a ticket started.
  Rejected: a split by the tickets' close times (events before a ticket starts would go to the ticket
  closed last); a start time in `state.json` (a change of the state schema for one number).

## Spec

Terms:

- **Main transcript**: a Claude Code transcript file whose identity (`Transcript::identity()`, STP-26) has
  no agent part. A file with an agent part is a subagent transcript.
- **Call**: a `tool_use` item in the `message.content` of an `assistant` line; its time is the time of the
  first line that holds its `id`. **Result**: a `tool_result` item in a `user` line; its time is that
  line's time, and its **source** is that line's `permissionDecision.source`.
- **Event**: a question, a dialog or a typed message (AC-1 to AC-3). Its **time** is the time of the
  result (question, dialog) or of the line (typed message). Its **wait** in milliseconds ends at that time;
  a wait below zero is 0. Its **interval** runs from its time minus its wait to its time.
- **Cap**: 600 000 ms (10 minutes). A **long wait** is a wait over the cap.
- **Attention numbers**: `questions`, `dialogs`, `typed` (counts), `waited_ms` (the length of the union of
  the events' intervals), `attention_ms` (the same union with each interval cut to its last 600 000 ms) and
  `long_waits`. Intervals that overlap count once, so parallel calls and typed messages in a row do not add
  up (SR1-7, SR1-8).
- **Attention record**: one line of a ticket's `tokens.jsonl` with `v: 1`, `kind: "attention"`, an id that
  starts with `a-`, `at`, `ticket`, `hook` (`stop` or `session-end`), `transcript` (the identity), `from`
  and `to` (the times of the window's first and last event) and the six attention numbers. It has no
  `role`, `model` or `source`.
- **Attention cursor**: the latest `to` among the attention records of every ticket's `tokens.jsonl`
  whose `transcript` is the identity; a line that is not a JSON object is skipped; without such a record,
  no cursor.
- **Window**: the events whose time is later than the attention cursor (all without one). The token window
  of STP-26 does not bound it: a result line and a typed line are whole once they are in the file (SR1-4).
- **Target ticket**: as in STP-26, with the window's first event in place of the first message when the
  call writes no token record.
- **Worktree ticket** of an event: the ticket of the hook's repository whose key equals the path component
  that follows the components `.claude` and `worktrees` in the `cwd` of the event's line; none when the
  line has no such `cwd` or the repository has no ticket of that key.
- **Duration text**: whole minutes, rounded down: `<m> min` below 60 minutes, else `<h> h <m> min`.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN a main transcript holds the result of an `AskUserQuestion` call THE reader gives one question event that counts the items of the call's `questions` (1 when that is no array or is empty), with the wait from the call to the result. | `attention_events::question_counts_its_questions` |
| AC-2 | WHEN the result of a call of another tool has a source that starts with `user` THE reader gives one dialog event with the wait from the call to the result; a result with another source or with none is no event. | `attention_events::dialog_by_permission_source` |
| AC-3 | WHEN a `user` line is not meta, is no sidechain line and no compaction summary, holds no result, and its text starts neither with `<` nor with `[Request interrupted` THE reader gives one typed message whose wait runs from the latest earlier `assistant` line, 0 without one. | `attention_events::typed_message_and_its_wait` |
| AC-4 | WHEN events are summed THE waited time is the length of the union of their intervals, the attention time is that union with each interval cut to its last 600 000 ms, and the long waits are the events whose wait is over the cap. | `attention_events::cap_and_long_waits` |
| AC-5 | IF a line is not a JSON object, has no time that can be read, is longer than 16 MiB, or the call of a result is not on an earlier line of the file THEN the reader skips that line or result and counts the rest. | `attention_events::broken_lines_are_skipped` |
| AC-6 | WHEN the `session-end` hook, or the `stop` hook when it passes the 10-minute rule of STP-26, reads a main transcript THE hook appends one attention record for the window to the target ticket, in the same write as its token records when it has any; a window with no event gives no attention record. | `attention::stop_appends_one_record`, `attention::session_end_appends_the_rest` |
| AC-7 | WHEN a hook runs again with no new event THE `tokens.jsonl` files stay unchanged; WHEN the attention cursor comes from another ticket's record THE window starts after it. | `attention::repeated_call_adds_nothing`, `attention::cursor_across_tickets` |
| AC-8 | WHEN the `subagent-stop` hook runs THE hook appends no attention record. | `attention::subagent_stop_has_no_attention` |
| AC-15 | WHEN the events of one window have different worktree tickets THE hook appends one attention record per ticket: the events of a worktree ticket to that ticket, open or closed, and the events without one to the target ticket; each record holds the numbers, `from` and `to` of its own events. | `attention::events_split_by_worktree` |
| AC-9 | WHEN `stapel tokens`, with or without KEY, reads a ticket that has an attention record THE report prints after that ticket's table the lines `attention: <d> dialogs, <q> questions, <t> messages; <duration> attention, <duration> waited (<n> over 10 min)` and `steps: <n>; wall time: <duration>`, the numbers summed over the ticket's attention records; an attention record is no row and no problem, and a ticket without one gets neither line. | `attention::report_shows_the_attention_lines`, `attention::report_without_attention_is_unchanged` |
| AC-13 | WHEN the report prints the steps line THE steps are the distinct step labels of the ticket among the subjects of the commits reachable from `HEAD`, merge commits followed (`—` when git cannot be read), and the wall time runs from the earliest `from` to the latest `to` of the ticket's attention records. | `attention::report_counts_steps_after_a_merge` |
| AC-10 | WHEN `stapel tokens measure FILE` gets a main transcript THE command prints `transcript: <identity>`, a table with the columns source, model, input, output, cache read and cache write (one `main` row per model of the file, one `subagents` row per model of the `*.jsonl` files of the same session in `<sessionId>/subagents` beside it), the attention line of AC-9 and `wall time: <duration>` from the first to the last timed line. | `attention::measure_prints_tokens_and_attention` |
| AC-14 | WHEN `stapel tokens measure` runs THE command writes no file and exits 0, also in a folder that is no stapel repository and no git repository. | `attention::measure_outside_a_repository` |
| AC-11 | IF `FILE` does not exist, is no regular file, cannot be read, has no `sessionId` or is a subagent transcript THEN `stapel tokens measure` prints one line on standard error that names the path and the reason, prints nothing on standard output and exits 2; without `FILE` it exits 2 with the usage text. | `attention::measure_refuses_bad_files` |
| AC-12 | WHEN the `subagent-stop` hook gets a transcript path that does not exist THE hook writes nothing; the gap records of the other reasons, and of `stop` and `session-end`, are unchanged. | `attention::missing_subagent_transcript_writes_nothing`, `hook_tokens::unreadable_transcript_gives_a_gap_record`, `hook_tokens::bad_input_writes_nothing`, `hook_tokens::token_hooks_on_a_branch` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-15; the token rows of the report, `stapel tokens
  import`, `import-records`, `add` and the content of the token records of the hooks are unchanged. The
  reader streams the file: it holds one line (at most 16 MiB) and the ids of the calls (SR1-14).
- **Not promised:** the Frame's list; the tool's run time inside a dialog's wait (the transcript has no
  time of the answer, so the wait of a dialog is an upper bound, cut by the cap; SR1-7); a result on a line
  that holds several results with different decisions (the line's one source counts for each); a question
  the person rejected and a dialog the person denied are events like the others; a typed message sent while
  the agent works (its wait runs from the latest `assistant` line, which may be seconds); a session resumed
  under a new `sessionId` (a new identity); an event with the same millisecond as the cursor that is written
  later (lost); a `stop` call whose token records come up to 10 minutes later because an attention record
  alone is the newest record of the transcript (SR1-13); `import-records` of an attention record (refused as
  any record without a `source`); the gap records already in a journal; steps of a ticket whose commits are
  not reachable from `HEAD` (the count is then lower); a clock that runs back (waits of 0); a hook call
  that has no target ticket (several open tickets and no branch ticket, or none open and none closed in the
  window): it writes no attention record, also for events that have a worktree ticket, and the cursor does
  not move (D3d-1); a call that fails between the writes into two tickets' journals (the events of the
  second ticket before the first record's `to` are lost; D3d-3); a `sessionId` that holds a path separator
  (`measure` then looks for subagent files in the folder that path names; D5-1).

## Test plan

Unit tests of the reader in the new file `crates/stapel-core/tests/attention_events.rs`: each test writes a
transcript of a few lines to a temporary folder. CLI tests in the new file
`crates/stapel-cli/tests/attention.rs` with `common::repo_with_ticket`; a main transcript is built in the
test with a `sessionId`, `assistant` lines with calls, `user` lines with results and a
`permissionDecision`, and typed lines; hook inputs go to stdin as in `hook_tokens`. Exact report and
`measure` output lives in golden files under `crates/stapel-cli/tests/golden`. Every "nothing written"
case first shows that the same setup with a good input writes. Light tests run through `stapel cargo`:
`test -j 4 -p stapel-core --test attention_events`, `test -j 4 -p stapel-cli --test attention --
--test-threads=3` (and `--test hook_tokens`, `--test tokens`); before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Question | a call of `AskUserQuestion` with 3 questions and its result 90 s later: one event, count 3, wait 90 000; `questions` absent, a string, an empty array: count 1; a rejected question (`is_error`): still an event | `attention_events::question_counts_its_questions` |
| Dialog | results with source `user_temporary`, `user_permanent`: dialogs; `config`, `hook`, no `permissionDecision`, a source that is a number: no event; the call streamed on two lines: the wait runs from the first | `attention_events::dialog_by_permission_source` |
| Typed | a string content and a content of one `text` item: typed, wait from the latest `assistant` line; the first line of the file: wait 0; two typed lines in a row: two events from the same `assistant` line; `isMeta: true`, `isSidechain: true`, `isCompactSummary: true`, a text that starts with `<`, a text that starts with `[Request interrupted`, a line with a result: not typed | `attention_events::typed_message_and_its_wait` |
| Sums | separate waits of 0, 599 999, 600 000, 600 001 and 7 200 000 ms: attention, waited and 2 long waits; two parallel calls with results 60 s and 90 s after the same call time: waited 90 000, not 150 000; two typed lines in a row: their intervals count once; a result timed before its call: wait 0; no event: all numbers 0 | `attention_events::cap_and_long_waits` |
| Broken lines | a line `not json`, a JSON array, a line without `timestamp`, one with `timestamp: 5`, a line of 16 MiB + 1, a result whose call is absent, a result whose call is on a later line, an empty file: the other events counted, no error | `attention_events::broken_lines_are_skipped` |
| Stop | a main transcript with 2 dialogs, 1 question of 2 and 1 typed message, no record yet: `stop` appends the token records and one attention record with the numbers, `from` and `to` of the first and last event, `hook: "stop"`, the identity; a record of the transcript 9 minutes old: nothing; an event after the last complete message is counted; no event: token records only; no complete message and one event: an attention record alone | `attention::stop_appends_one_record` |
| Session end | a record 1 minute old and new events: `session-end` appends the rest with `hook: "session-end"`; events and no new message: an attention record alone | `attention::session_end_appends_the_rest` |
| Repeat | the same `session-end` input twice: the second run leaves every `tokens.jsonl` byte for byte | `attention::repeated_call_adds_nothing` |
| Cursor | an attention record of this transcript in a closed ticket's journal with `to` at event 2: the open ticket gets the later events only; a cursor between a call and its result: that event is counted once, by its result; a token record there is no attention cursor; a line `not json` there is skipped | `attention::cursor_across_tickets` |
| Subagent | a subagent transcript with a call and a result of source `user_temporary`: token records, no attention record | `attention::subagent_stop_has_no_attention` |
| Split | one `session-end` call over events with `cwd` in `.claude/worktrees/ABC-1` (a closed ticket), in `.claude/worktrees/ABC-2` (the open ticket), in the main checkout, in `.claude/worktrees/ZZZ-9` (no such ticket) and without `cwd`: one record in ABC-1 with its events, one in ABC-2 with the rest; a second call adds nothing; a `cwd` that is a number counts as none | `attention::events_split_by_worktree` |
| Report | a journal with token records and two attention records: the golden `tokens_attention.txt` holds the table as before and the two lines of AC-9 with the summed numbers; the report without KEY prints the lines in that ticket's block only; exit 0 and no `problem:` line | `attention::report_shows_the_attention_lines` |
| Steps and wall time | two step labels on a branch that a merge commit brings into `HEAD`: `steps: 2`; a ticket with no step commit: `steps: 0`; no git repository: `steps: —`; the wall time from the earliest `from` to the latest `to` of two attention records, 59 min and 61 min apart for both duration forms | `attention::report_counts_steps_after_a_merge` |
| Report unchanged | a journal without attention records: the output equals the output before this ticket, byte for byte | `attention::report_without_attention_is_unchanged` |
| Measure | a main transcript with two models, a folder `<session>/subagents` beside it with two files of that session, one file of another session and a file `notes.txt`: the golden `tokens_measure.txt` counts the first two; a last message without a final line: its output with `≥` as in the report; no subagent folder: main rows only; one timed line: `wall time: 0 min`; no file under the test folder changes | `attention::measure_prints_tokens_and_attention` |
| Outside | the same call in a folder that is no stapel repository and no git repository: the same output | `attention::measure_outside_a_repository` |
| Bad files | a missing path, a folder, a FIFO, a file without read permission (as `hook_tokens` tests it), a file without `sessionId`, a subagent transcript: exit 2, standard output empty, the path and the reason on standard error; no argument: exit 2 and the usage text | `attention::measure_refuses_bad_files` |
| Missing subagent file | `subagent-stop` with a missing path: no file changed, exit 0; the same input with a file writes | `attention::missing_subagent_transcript_writes_nothing` |
| Gap records | the STP-26 test without its missing-path case for `subagent-stop`: a folder, a file without `sessionId`, a file without read permission still give gap records; `stop` with a missing path still gives one | `hook_tokens::unreadable_transcript_gives_a_gap_record` |
| Old examples | two STP-26 tests used a missing subagent transcript as their example of a gap record: the control of the bad-input test and the gap record on a branch ticket now use a folder path, and each asserts that a missing path writes nothing | `hook_tokens::bad_input_writes_nothing`, `hook_tokens::token_hooks_on_a_branch` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `FILE` of `measure` | path | any | no argument: usage, exit 2 | missing, a folder, not readable, no `sessionId`, a subagent transcript: exit 2 | as given, links followed | `attention::measure_refuses_bad_files` |
| transcript line | JSON object | — / 16 MiB | blank: skipped | not an object, longer: skipped | — | `attention_events::broken_lines_are_skipped` |
| `timestamp` | RFC 3339 time | — | absent: line skipped | not a time: line skipped | milliseconds | `attention_events::broken_lines_are_skipped` |
| `permissionDecision.source` | string | any | absent: no dialog | not a string: no dialog | prefix `user` | `attention_events::dialog_by_permission_source` |
| `questions` of a call | array | 1 / any | empty or absent: 1 | not an array: 1 | item count | `attention_events::question_counts_its_questions` |
| wait | milliseconds | 0 / any | — | below zero: 0 | cap at 600 000 exactly | `attention_events::cap_and_long_waits` |
| `cwd` of a line | path string | any | absent: no worktree ticket | not a string, a key with no ticket: no worktree ticket | the key component, exact case | `attention::events_split_by_worktree` |
| attention record fields | JSON numbers | 0 / u64 | absent number: counts as 0 | not a number: counts as 0 | whole milliseconds | `attention::report_shows_the_attention_lines` |
| attention record `to` | RFC 3339 time | — | absent: no cursor | not a time: no cursor | milliseconds | `attention::cursor_across_tickets` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | transcripts: main and subagent files | events from the main file only; `measure` adds the tokens of `<session>/subagents/*.jsonl` | `attention::measure_prints_tokens_and_attention` |
| claude-code | transcripts: streamed lines of one message | a call's time is its first line | `attention_events::dialog_by_permission_source` |
| claude-code | transcripts: resumed sessions, compaction | a result without its call is skipped; a new `sessionId` is a new identity | `attention_events::broken_lines_are_skipped` |
| claude-code | permission mode; settings levels; allow rules | a call allowed by a mode, a rule or the classifier has source `config` and is no dialog | `attention_events::dialog_by_permission_source` |
| claude-code | environment of hook processes; hook input | as STP-26; no new field is read | `attention::stop_appends_one_record` |
| claude-code | tool call succeeded or failed | a failed or rejected call with a person's source is an event | `attention_events::question_counts_its_questions` |
| git | history shape, for the step count: merge commits | the distinct step labels among the subjects of every commit reachable from `HEAD` | `attention::report_counts_steps_after_a_merge` |
| git | no repository, unborn `HEAD` | `steps: —` | `attention::report_counts_steps_after_a_merge` |
| git | shallow clone, rebased or squashed history | the count is of the commits that are there; not promised beyond it | — |
| git | linked worktrees | an event's ticket is read from the `cwd` of its line, not from git; a ticket that exists only in another worktree gets no record (its events go to the target ticket) | `attention::events_split_by_worktree` |
| git | other dimensions | the report reads the history of the current worktree; not promised beyond it | — |
| cargo | every dimension | not run | — |
| os | file permissions; symlinks | `measure` refuses a file it cannot read; links are followed | `attention::measure_refuses_bad_files` |
| os | two hook processes at once | the tokens lock of STP-26 covers the attention record | `attention::repeated_call_adds_nothing` |
| os | clock skew, time zones | a wait below zero is 0; times are UTC | `attention_events::cap_and_long_waits` |

**Review Focus:** a main transcript of tens of MB read a second time for the events (time not tested); a
dialog whose result line has no `permissionDecision` in a later Claude Code version (the count falls to 0
without a sign; `measure` on a real transcript is the probe); an attention record read by an older `stapel`
binary (it prints a `problem:` line); a typed message that is a pasted command output starting with `<`
(not counted).

**Author self-check:** 2026-11-11, after spec review round 1: every promise item has a criterion and a test
(item 1: AC-1, AC-2, AC-3, AC-5; item 2: AC-4; item 3: AC-6, AC-7, AC-8; item 4: AC-9, AC-13; item 5: AC-10,
AC-11, AC-14; item 6: AC-12; item 7: AC-15); each Inputs and External states row has a test or "not promised"; the readers
of `tokens.jsonl` that see the new record are the report, the token cursor of the hooks and
`import-records`.

## Design

### Facts

- F-1: measured on two past sessions (STP-21, STP-22): 30 and 13 dialogs, 12 and 7 questions, 26 and 16
  hours of raw waiting, nights included. On those main transcripts (Claude Code 2.1.303) a `user` line with a
  `tool_result` has the top-level field `permissionDecision` with `decision` and `source`; the sources seen
  are `config` (with `reasonType` `classifier`, `mode`, `other`), `user_temporary` and `hook`. The result of
  `AskUserQuestion` has source `user_temporary`.
- F-2: typed messages are `user` lines with a string content, without `isMeta: true`; injected lines start
  with `<` (`<task-notification>`, `<command-name>`, `<local-command-stdout>`). Every line of a main
  transcript has `isSidechain: false`.
- F-3: `tokens::report` (`crates/stapel-cli/src/tokens.rs`) counts a `v: 1` record whose `source` is
  neither `measured` nor `estimate` as a problem and exits 1; the attention record must be taken out before
  that test.
- F-4: `tokens::hook_records` returns early when the token window is empty; the attention record needs its
  own window and its own target when no token record is written.
- F-5: `tokens::cursors` reads `at` of every record of the identity for the 10-minute rule and needs
  `model` and `to` for a token cursor; an attention record has no `model`, so it is no token cursor, and
  its `at` counts for the 10-minute rule.
- F-6: subagent transcripts of a session lie in `<folder of the main file>/<sessionId>/subagents/` as
  `agent-<id>.jsonl`.
- F-7: `tokens::read_for_hook` gives the reason `missing` for a path that does not exist; `gap_record`
  writes the record of STP-26 AC-9.

### Decisions (before the build)

- The reader is the new module `crates/stapel-core/src/attention.rs`: it streams the file with
  `read_bounded_line`, keeps the first time of each call id and the name and question count of each call,
  and returns the events in file order. The sums are a function of a slice of events.
- The reader makes one pass: a result whose call is not on an earlier line is skipped (SR1-5). The union
  of the intervals is computed over the events sorted by the start of their interval.
- The attention window is not tied to the token window: it holds every event after the attention cursor,
  and `to` is the time of its last event (SR1-2, SR1-4). `stop` keeps its 10-minute rule for the whole call;
  `session-end` has none (SR1-3).
- The events of a window are grouped by their worktree ticket, else the target ticket; each group gives
  one record in that ticket's journal, and the attention cursor is the latest `to` of them all (SR1-11).
- The attention record goes into the same `append_all` call as the token records of the hook call, under
  the tokens lock; its id is `a-` and the first 12 hex digits of SHA-256 over the ticket, the identity and
  `to`.
- The report sums the attention records of a ticket before the row logic sees them. The steps are the
  distinct labels that `parse_subject` reads from the subjects of `git log HEAD` without `--first-parent`,
  so a merged ticket branch counts (SR1-10); the wall time runs from the earliest `from` to the latest `to`
  of the ticket's attention records (SR1-6).
- `measure` is a new `TokensAction` that does not open the repository; it reuses `read_transcript`,
  `sum_window` and the reader. A subagent file is a regular `*.jsonl` file directly in the folder whose
  `sessionId` is that of the main file; other files are skipped (SR1-15). The table is rendered as the
  report's table.
- The skill text is not changed: rule 8 already sends the orchestrator to `stapel tokens KEY`, which now
  prints the attention lines.

### As built

- The reader has two entries: `read_events` gives the events, `read_events_with_cwd` gives each event with
  the `cwd` of its line. `Event` has no `cwd` field, because the step tests build it with struct literals
  and a GREEN commit may not change tests (step 3d).
- `hook_records` keeps the 10-minute rule of `stop` for the whole call. A `stop` with no complete message
  no longer returns early: its token window is empty and an attention record alone may be written (step 3).
- The attention cursor is the function `attention_cursor`; the worktree key comes from `worktree_key`,
  which takes the first pair of the components `.claude` and `worktrees` of a path (D3d-5).
- The records of worktree tickets are appended first, one journal at a time, then the target ticket's
  lines (D3d-3).
- The report sums the records in `AttentionSum`; `step_count` reads the subjects of `git log HEAD`; the
  helper `duration` prints the Duration text, `2 h 0 min` for a whole number of hours (D4-1, fixed in step
  4c).
- `measure` checks the file type before it opens the file, then reads it three times (tokens, events,
  first and last time); the row of a model whose last message has no final line shows `≥` and counts that
  message's streamed output; a refusal is the line `stapel: <path>: <reason>` with the reasons `missing`,
  `not a file`, `not readable`, `no sessionId` and `subagent transcript`. `render` became generic over the
  number of columns; the report's table is byte for byte as before (D5-4).
- The attention line is formatted in two places, the report and `measure`, with the same text (D5-3).
- Step 6 changed three old tests of `hook_tokens`, not one: the plan had missed two that used a missing
  subagent transcript as their example of a gap record; the GREEN builder stopped and the RED was amended.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `attention_events::question_counts_its_questions` |
| AC-2 | `attention_events::dialog_by_permission_source` |
| AC-3 | `attention_events::typed_message_and_its_wait` |
| AC-4 | `attention_events::cap_and_long_waits` |
| AC-5 | `attention_events::broken_lines_are_skipped` |
| AC-6 | `attention::stop_appends_one_record`, `attention::session_end_appends_the_rest` |
| AC-7 | `attention::repeated_call_adds_nothing`, `attention::cursor_across_tickets` |
| AC-8 | `attention::subagent_stop_has_no_attention` |
| AC-15 | `attention::events_split_by_worktree` |
| AC-9 | `attention::report_shows_the_attention_lines`, `attention::report_without_attention_is_unchanged` |
| AC-13 | `attention::report_counts_steps_after_a_merge` |
| AC-10 | `attention::measure_prints_tokens_and_attention` |
| AC-14 | `attention::measure_outside_a_repository` |
| AC-11 | `attention::measure_refuses_bad_files` |
| AC-12 | `attention::missing_subagent_transcript_writes_nothing`, `hook_tokens::unreadable_transcript_gives_a_gap_record`, `hook_tokens::bad_input_writes_nothing`, `hook_tokens::token_hooks_on_a_branch` |

## Plan

Route: full (tags `data`, `interface`), auto. Every numbered step is a RED and GREEN pair whose named tests
fail at the RED (a test file that does not compile yet counts as failing); a step marked "coverage" is one
commit of a test that passes at once (SR1-9).

1. The events reader: questions and dialogs (AC-1, AC-2; tests
   `attention_events::question_counts_its_questions`, `attention_events::dialog_by_permission_source`): the
   new module `attention.rs` in `stapel-core`. Must not change: `read_transcript` and the token sums.
1b. Typed messages and broken lines (AC-3, AC-5; tests `attention_events::typed_message_and_its_wait`,
   `attention_events::broken_lines_are_skipped`): the RED makes the reader's result carry typed messages, so
   the test file fails to compile there. Must not change: step 1.
2. The sums: union, cap and long waits (AC-4; test `attention_events::cap_and_long_waits`). Must not
   change: steps 1 and 1b.
3. The attention record of `stop` and `session-end` (AC-6; tests `attention::stop_appends_one_record`,
   `attention::session_end_appends_the_rest`), without the cursor: each call counts every event of the file.
   Must not change: the token records of the hooks (`hook_tokens` passes unchanged).
3b. Coverage, no GREEN (AC-8; test `attention::subagent_stop_has_no_attention`): the subagent hook writes
   no attention record after step 3, so the test passes at once.
3c. The attention cursor and the repeat (AC-7; tests `attention::repeated_call_adds_nothing`,
   `attention::cursor_across_tickets`). Must not change: the token cursor.
3d. The split by worktree (AC-15; test `attention::events_split_by_worktree`): the reader's events carry
   the `cwd` of their line. Must not change: a window whose events have no worktree ticket (steps 3, 3c).
4. The report lines (AC-9; test `attention::report_shows_the_attention_lines`). Must not change: the token
   table and the goldens of `tokens`.
4b. Coverage, no GREEN (AC-9; test `attention::report_without_attention_is_unchanged`).
4c. Steps and wall time (AC-13; test `attention::report_counts_steps_after_a_merge`): step 4 prints the
   steps line with `—` for the steps; this step counts them. Must not change: step 4's attention line.
5. `stapel tokens measure` (AC-10, AC-11, AC-14; tests `attention::measure_prints_tokens_and_attention`,
   `attention::measure_outside_a_repository`, `attention::measure_refuses_bad_files`). Must not change:
   the other `tokens` actions.
6. No record for a missing subagent transcript (AC-12; tests
   `attention::missing_subagent_transcript_writes_nothing`,
   `hook_tokens::unreadable_transcript_gives_a_gap_record`, `hook_tokens::bad_input_writes_nothing`,
   `hook_tokens::token_hooks_on_a_branch`): the RED changes the missing-path case of `subagent-stop` in the
   three old tests to expect no record, so all four tests fail there. Must not change: the gap records of
   the other reasons and of `stop` and `session-end`.
7. Fix after code review round 1 (AC-7; tests `attention::repeated_call_adds_nothing`,
   `attention::cursor_across_tickets`): the cursor across checkouts. The RED adds to both tests a second
   checkout (a linked worktree) where the hook runs after the first and must add nothing. The GREEN keeps
   the latest `to` of each transcript also in a file in the git common folder, under the tokens lock; the
   cursor is the later of the two. Must not change: the cursor inside one checkout, the token cursor.
7b. Coverage, no GREEN (AC-7; test `attention::bad_shared_cursor_file`): a shared cursor file that is no
   JSON, an array, or holds a value that is no time gives no value and the hook writes its records; the
   later of the shared and the journal value wins in both orders (D7-1, D7-2).

## Review

TODO

## Summary

TODO
