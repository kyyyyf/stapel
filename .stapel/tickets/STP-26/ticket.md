# STP-26 — Tokens from hooks

## Description

Source: `docs/PHASES.md`, plan item 1.4; `docs/decisions/0004-product-direction-and-interface.md` (no
transcript import by a person); the parser and import of STP-5 and STP-19.

### Frame

**Problem.** Token records come only from `stapel tokens import`, run by hand after the close, with a
5-minute quiet rule and a `--since` window kept in the head; builder and reviewer runs that nobody imports
are lost, and the orchestrator re-reads transcripts to do it.

**Plan item:** 1.4.

**Risk tags:** `data` (appends to `tokens.jsonl` from a hook), `interface` (new hook commands, a `[roles]`
table in `stapel.toml`, hooks written by `stapel init`).

**Non-goals.** Complete output counts of subagents (Claude Code writes the final streamed line of most
subagent messages late or never; such messages stay `partial`); records for past sessions; splitting one
session between two open tickets; cost in money; a change of `tokens import` or `import-records`.

**Promise (closed list).**

1. `stapel hook subagent-stop` reads `agent_type` and `agent_transcript_path` from the hook input and
   appends one `measured` record per model for the part of that transcript not yet recorded.
2. The role comes from the `[roles]` table of `stapel.toml` (agent type to role); `stapel init` writes
   the four `stapel-*` agents into it; a type without a row gets the role `research` and its type in `note`.
3. `stapel hook stop` appends the main transcript's unrecorded part with the role `orchestrator` when the
   last record of that transcript is 10 minutes old or more; `stapel hook session-end` appends it always.
4. The ticket is the one open ticket; with none, the ticket closed in that window; else nothing is written.
5. A transcript the parser cannot read gives an `estimate` record of 0 with the reason in `note`.
6. Every hook exits 0 and prints nothing on stdout; a repeated hook call adds no record twice.
7. `stapel init` installs the three hooks; the skill text no longer asks for a token import.

Not promised: output of a subagent message without its final line; a transcript edited between calls.

**Size.** About 400 source lines and 700 test lines; split past 600. Route: full (tags), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-30: Decision (main session, the human, recommended): `Stop` records the window since the last
  record of the main transcript only when that record is 10 minutes old or more; `SessionEnd` records the
  rest. Rejected: a record at every `Stop` (100 to 300 lines per ticket); `SessionEnd` only (a crashed
  session loses its tail).
- 2026-10-30: Decision (roles, the human, recommended): a `[roles]` table in `stapel.toml` maps an agent
  type to a role; a type without a row is recorded as `research` with the type in `note`. Rejected: an
  unmapped type skipped; the type itself as the role.
- 2026-10-30: Decision (no ticket, the human, recommended): with no open ticket or several, the hook
  writes nothing and exits 0; the tail after `stapel close` goes to the ticket closed in that window.
  Rejected: a session file for records without a ticket.
- 2026-10-30: Decision (unreadable transcript, the human, recommended): an `estimate` record with the count
  0 and the reason in `note`, so the gap shows in `stapel tokens`. Rejected: no record, an error on stderr.
- 2026-10-30: Decision (old configuration, the human, recommended): a `stapel.toml` without `[roles]` uses
  the four rows `stapel init` writes; with the table, only its rows count. `stapel init` does not change an
  existing `stapel.toml`; this repository's gets the table in this ticket. Rejected: every type `research`.
- 2026-10-30: Decision (cursor, the human, recommended): the recorded part of a transcript is read from
  the records of every ticket of the repository, so a session that spans `close` and `new` counts no
  message twice. Rejected: the records of the one ticket.
- 2026-10-30: Decision (no sign of a skip, the human, recommended; spec review SR1-13): a hook that finds
  no target ticket leaves no trace; a skip journal or count may come as a later plan item. Rejected: a
  skip journal and a count in `stapel tokens` in this ticket.
- 2026-10-30: Assumption (SessionEnd, docs of Claude Code hooks): it fires on `/clear`, `/resume` and exit,
  not surely on a crash; its default timeout is 1.5 s, so the installed entry sets its own `timeout`.
- 2026-10-30: Assumption (hook input, docs of Claude Code hooks): `SubagentStop` carries `agent_type` (the
  subagent type, e.g. `stapel-builder`) and `agent_transcript_path`; `Stop` and `SessionEnd` carry
  `transcript_path`; `CLAUDE_PROJECT_DIR` is set for hooks; a transcript may lag its last message, so a window ends at the last complete message
  and the next call takes the rest. Checked on one real run before the build.

## Spec

Terms:

- **Token hook**: `stapel hook subagent-stop`, `stapel hook stop` or `stapel hook session-end`; each reads
  one hook input (JSON) on stdin.
- **Hook root**: the stapel root of the first present source among `CLAUDE_PROJECT_DIR`, the input's
  `cwd` and the working directory; an empty value is absent. `CLAUDE_PROJECT_DIR` comes first because the
  input's `cwd` follows the agent's `cd` (SR1-5).
- **Identity**: `Transcript::identity()` of the transcript (`<session>` or `<session>/<agent>`).
- **Message time**: the time of a message's first line (STP-5).
- **Cursor** of a model: the latest `to` among the records of every ticket's `tokens.jsonl` whose
  `transcript` is the identity and whose `model` is that model; a line that is not a JSON object is
  skipped; without such a record, no cursor.
- **Window**: per model, the messages whose time is later than that model's cursor (all without one).
  For `stop`, one end for every model: the last message of the transcript with a final line, moved back
  while a message after it has the same time; later messages wait for the next call. `subagent-stop` and `session-end` take every
  message; one without a final line is `partial`.
- **Role map**: the `[roles]` table of `stapel.toml`, agent type to role; without the table, the default
  rows `stapel-builder = "builder"`, `stapel-drift = "drift"`, `stapel-reviewer = "reviewer"`,
  `stapel-spec-reviewer = "reviewer"`.
- **Target ticket**: the one open ticket; with none open, the closed ticket with the latest `closed.at`
  not earlier than the earliest first message of the call's windows over all models; else none. Every
  record of one call goes to this one ticket (SR2-3).
- **Hook record**: the record `stapel tokens import` writes for one model over the window (STP-5: the
  counts, `transcript`, `from`, `to`, `messages`, `partial`, `importer: 2`, the same id rule), with
  `hook` set to the hook's name (`subagent-stop`, `stop`, `session-end`).
- **Gap record**: `source: "estimate"`, `estimate: 0`, `model: "unknown"`, the hook's role, `hook`,
  `transcript` set to the path as given, and `note: "transcript not read: <reason>"`, where the reason is
  one of `missing`, `not a file`, `no sessionId`, `not readable`; its id is taken from the ticket, the
  path and the reason.
- **Shown type**: the agent type with each control character as one space, cut to 64 characters.
- The records of one hook call are appended in one write (`journal::append`), which starts a new line when
  the file ends without one.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel hook subagent-stop` gets an input with `agent_type` and `agent_transcript_path` THE hook appends one hook record per model of the window to the target ticket's `tokens.jsonl`, with the role of the role map, prints nothing on stdout and exits 0. | `hook_tokens::subagent_stop_records_each_model`, `hook_tokens::partial_messages_as_import` |
| AC-2 | WHEN the agent type has no row in the role map THE role is `research` and the record has `note: "agent type <shown type>"`; WHEN `stapel.toml` has a `[roles]` table THE default rows do not apply. | `hook_tokens::roles_from_the_table`, `hook_tokens::roles_without_a_table` |
| AC-3 | IF a `[roles]` key or value is empty, longer than 64 bytes or holds a control character THEN `Config::parse` refuses with `roles: <key> must be 1 to 64 bytes without control characters`. | `core::config::rejects_bad_roles` |
| AC-4 | WHEN `stapel hook stop` runs THE hook appends the window of `transcript_path` with the role `orchestrator` only when no record of any ticket has the transcript's identity as `transcript`, or the newest such record's `at` is 10 minutes or more before now. | `hook_tokens::stop_waits_ten_minutes` |
| AC-5 | WHEN `stapel hook session-end` runs THE hook appends the window of `transcript_path` with the role `orchestrator`, whatever the age of the last record, its messages without a final line counted as `partial`. | `hook_tokens::session_end_records_the_rest` |
| AC-6 | WHEN the last messages of a main transcript have no final line, or share the time of the last message that has one THE window of `stop` ends before them, and the next call records them. | `hook_tokens::window_ends_at_the_last_complete_message` |
| AC-7 | WHEN a token hook runs again with no new message THE `tokens.jsonl` files stay unchanged; WHEN the cursor comes from another ticket's records THE window starts after it. | `hook_tokens::repeated_call_adds_nothing`, `hook_tokens::cursor_across_tickets` |
| AC-8 | WHEN no ticket is open THE records go to the ticket closed latest at or after the window's first message; IF several tickets are open, or none is open and none closed in the window THEN nothing is written. | `hook_tokens::tail_after_close`, `hook_tokens::no_target_ticket` |
| AC-9 | IF the transcript cannot be read THEN the hook appends a gap record to the one open ticket (never to a closed one), once for the same path and reason, and `stapel tokens` shows it in its role's `estimate` cell. | `hook_tokens::unreadable_transcript_gives_a_gap_record` |
| AC-10 | IF the input is not JSON or lacks a field the hook needs, the hook root is not a stapel repository or has no git common folder, or `stapel.toml` or the target ticket's `tokens.jsonl` cannot be opened THEN nothing is written, stdout is empty and the exit code is 0. | `hook_tokens::bad_input_writes_nothing` |
| AC-14 | WHEN a `tokens.jsonl` line is not a JSON object, has a `to` that is not a time, or the file ends without a newline, or another ticket's `tokens.jsonl` cannot be opened THE hook skips that line or file for the cursor and appends its records on lines of their own. | `hook_tokens::broken_line_does_not_stop_the_hooks` |
| AC-15 | WHEN another process holds the tokens lock THE token hook writes nothing until it is released, then writes its records once. | `hook_tokens::two_hooks_at_once` |
| AC-11 | WHEN `stapel init` writes `.claude/settings.json` THE events `SubagentStop`, `Stop` and `SessionEnd` each get one entry without a matcher, with the line `command -v stapel >/dev/null 2>&1 \|\| exit 0; stapel hook <name> 2>/dev/null \|\| exit 0` and `timeout` 30; a second run adds none. | `hooks_install::token_hooks_installed` |
| AC-12 | WHEN `stapel init` writes a new `stapel.toml` THE `[roles]` table holds the four default rows; this repository's `.stapel/stapel.toml` holds the same table. | `init::template_has_the_roles_table`, `init_process::this_repository_has_the_roles_table` |
| AC-13 | WHEN `stapel init` writes the skill THE rule 8 begins `**Measures after the close.** Read the measured tokens with stapel tokens KEY; the hooks record them.` and the skill no longer holds `Import the measured tokens`. | `init_process::skill_text_names_the_token_hooks` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-15; `stapel tokens import`, `import-records`, `add` and
  the report are unchanged; hook records count in `stapel tokens` like imported ones.
- **Not promised:** the Frame's list; a resumed session whose new file repeats old messages under a new
  `sessionId` (counted again); hooks turned off in the person's settings; a crash that skips `SessionEnd`
  (the last `Stop` record stands); a clock that runs back, or a record `at` in the future (`stop` waits);
  two tickets closed at the same time in one window (either one); a session working in a second worktree
  (its records go to the tickets of `CLAUDE_PROJECT_DIR`); a sign that a hook found no target ticket
  (SR1-13, outside the promise).

## Test plan

CLI tests in the new file `crates/stapel-cli/tests/hook_tokens.rs` with `common::repo_with_ticket`; the
hook input is written to stdin; transcripts are built in the test from lines of the STP-5 fixtures
(`tests/fixtures`), with a `sessionId`, message ids, models and times chosen per case; "now" is the clock,
so the `at` of planted records is set relative to it. Every hook test asserts exit 0, and every "nothing
written" case first shows that the same setup with a good input writes (SR1-12). Light tests: `cargo test -j 4 -p stapel-cli --test
hook_tokens -- --test-threads=3` (and `--test hooks_install`, `--test init`, `--test init_process`),
`cargo test -j 4 -p stapel-core --test config`; before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Subagent records | a subagent transcript with two models (3 and 1 messages), type `stapel-builder`: two records, role `builder`, `hook: "subagent-stop"`, counts as `tokens import` gives, stdout empty, exit 0 | `hook_tokens::subagent_stop_records_each_model` |
| Partial | a message without a final line: `partial: 1`, its output left out, as `tokens import` | `hook_tokens::partial_messages_as_import` |
| Role table | `[roles]` with `Explore = "research2"` and no `stapel-builder` row: `Explore` gives `research2`; `stapel-builder` gives `research` with `note: "agent type stapel-builder"` | `hook_tokens::roles_from_the_table` |
| No table | a `stapel.toml` without `[roles]`: `stapel-spec-reviewer` gives `reviewer`; `general-purpose` gives `research` and its note | `hook_tokens::roles_without_a_table` |
| Bad roles | `"" = "x"`; `a = ""`; a key of 65 bytes; a value with `\t`: refused with the AC-3 text; 64 bytes accepted | `core::config::rejects_bad_roles` |
| Stop timing | no record: written; a record of the transcript's identity with `at` 9 minutes ago: nothing; 10 minutes ago: written from after its `to`; a `tokens import` record counts the same; a record of another session 20 minutes ago does not count; an input `session_id` that differs from the transcript's is ignored | `hook_tokens::stop_waits_ten_minutes` |
| Session end | a record 1 minute ago: the rest written with `hook: "session-end"`, role `orchestrator`; a last message without a final line: `partial: 1` | `hook_tokens::session_end_records_the_rest` |
| Lag | the last two messages without a final line: the `stop` record ends at the message before them; a message with the same millisecond as the last complete one: the window ends before both; then `session-end` records them | `hook_tokens::window_ends_at_the_last_complete_message` |
| Repeat | the same input twice: the second run leaves every `tokens.jsonl` byte for byte | `hook_tokens::repeated_call_adds_nothing` |
| Cursor | a record of this transcript in a closed ticket's `tokens.jsonl` with `to` at message 3: the open ticket gets messages 4 and later only; a record of model A only: model B's earlier messages are still recorded | `hook_tokens::cursor_across_tickets` |
| Broken line | a `tokens.jsonl` of another ticket ending in `{"v":1,"id"` without a newline, a line `not json`, a record of this transcript with `"to": 5`, and a third ticket whose `tokens.jsonl` is a folder: the hook writes its records, each on a line of its own; the cursor ignores those lines and that file | `hook_tokens::broken_line_does_not_stop_the_hooks` |
| Lock | the test holds the lock on `tokens.lock` in the git common folder and starts `subagent-stop`: after 1 s nothing is written; after the release the records are written once; a second run adds none | `hook_tokens::two_hooks_at_once` |
| After close | no open ticket, a ticket closed after the window's first message: the records go there; two closed in the window: the later; model A's window starts after the close and model B's before it: both records go to the closed ticket | `hook_tokens::tail_after_close` |
| No target | two open tickets; no open ticket and the only closed one closed before the window: nothing written, exit 0 | `hook_tokens::no_target_ticket` |
| Gap | a missing transcript path; a folder; a file without `sessionId`; a file without read permission: one gap record each with its reason and `transcript`; the same input again: no second record; `stapel tokens` shows `estimate` 0 in the role's row; no open ticket: nothing written | `hook_tokens::unreadable_transcript_gives_a_gap_record` |
| Bad input | stdin `not json`; `{}`; no `agent_type`; no `agent_transcript_path`; no `CLAUDE_PROJECT_DIR` and a `cwd` outside any stapel repository; a stapel repository that is not a git repository; a broken `stapel.toml`; the open ticket's `tokens.jsonl` a folder: no file changed, stdout empty, exit 0 | `hook_tokens::bad_input_writes_nothing` |
| Root order | `CLAUDE_PROJECT_DIR` at the repository and `cwd` at `/tmp`: written to the repository | `hook_tokens::subagent_stop_records_each_model` |
| Shown type | an unmapped type with a tab and 80 characters: the note holds a space and the first 64 | `hook_tokens::roles_from_the_table` |
| Install | `stapel init` twice in an empty repository: one entry per event, the line and the timeout of AC-11, no matcher; the other events unchanged | `hooks_install::token_hooks_installed` |
| Template | `stapel init`: the `[roles]` table with the four rows; the file parses | `init::template_has_the_roles_table` |
| This repository | `.stapel/stapel.toml` here holds the four rows | `init_process::this_repository_has_the_roles_table` |
| Skill text | the AC-13 sentence present, `Import the measured tokens` absent, in the skill written by `init` | `init_process::skill_text_names_the_token_hooks` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| hook input | JSON on stdin | — / any size | nothing written | not JSON: nothing written | — | `hook_tokens::bad_input_writes_nothing` |
| `agent_type` | string | any; the note shows 64 characters | absent: nothing written; `""`: no row, `research` | control characters: spaces in the note | exact match with a `[roles]` key | `hook_tokens::roles_from_the_table` |
| `agent_transcript_path`, `transcript_path` | path | any | absent: nothing written | missing, a folder, no `sessionId`, not readable: gap record | as given | `hook_tokens::unreadable_transcript_gives_a_gap_record` |
| `session_id` | string | — | not read | not read | — | `hook_tokens::stop_waits_ten_minutes` |
| `CLAUDE_PROJECT_DIR`, `cwd` | path | any | empty: absent | the first present one outside a stapel repository: nothing written | first present source | `hook_tokens::bad_input_writes_nothing` |
| `[roles]` row | TOML string to string | 1 / 64 bytes each | no table: the default rows | AC-3 text | exact | `core::config::rejects_bad_roles` |
| record `at`, `to` | RFC 3339 time | — | absent `to`: the record is no cursor | unreadable time: no cursor | milliseconds | `hook_tokens::cursor_across_tickets` |
| `tokens.jsonl` line | JSON object | 64 KiB | blank: skipped | not an object, cut short: skipped for the cursor | — | `hook_tokens::broken_line_does_not_stop_the_hooks` |
| Stop interval | minutes | 10 | — | — | whole seconds | `hook_tokens::stop_waits_ten_minutes` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | hook input of `SubagentStop`, `Stop`, `SessionEnd` | the fields of the Inputs table; other fields ignored | `hook_tokens::subagent_stop_records_each_model` |
| claude-code | transcripts: streamed lines of one message | as STP-5: the fullest line counts, a message without a final line is `partial` | `hook_tokens::partial_messages_as_import` |
| claude-code | transcripts: a file behind its last message | the window ends at the last complete message (`stop`, `session-end`) | `hook_tokens::window_ends_at_the_last_complete_message` |
| claude-code | transcripts: resumed sessions, compaction | a new `sessionId` is a new identity; repeated old messages not promised | — |
| claude-code | environment of hook processes | `CLAUDECODE` is set there; the token hooks do not test it; `CLAUDE_PROJECT_DIR` names the root | `hook_tokens::subagent_stop_records_each_model` |
| claude-code | a hook killed at its timeout | a cut last line is skipped and the next append starts a new line | `hook_tokens::broken_line_does_not_stop_the_hooks` |
| claude-code | `SessionEnd` timeout of 1.5 s | the entry sets `timeout` 30 | `hooks_install::token_hooks_installed` |
| claude-code | permission mode; settings levels; allow rules | hooks run in every mode; hooks turned off: not promised | — |
| git | the git common folder (for the lock) | as `stapel check`'s lock | `hook_tokens::two_hooks_at_once` |
| git | linked worktrees; other dimensions | the root is `CLAUDE_PROJECT_DIR`'s; not promised | — |
| cargo | every dimension | not run | — |
| os | two hook processes at once | a lock orders them; a hook waits at most 20 s for it, then writes nothing (not tested) | `hook_tokens::two_hooks_at_once` |
| os | clock skew, time zones | not promised; times are UTC | — |

**Review Focus:** a main transcript of tens of MB read at every `stop` after the interval (time not
tested; the record check reads only the transcript's first lines); a message written later with the same
millisecond as the cursor (lost, one per collision); a panic inside a hook (caught, exit 0,
not tested: no input causes one); a record written by `tokens import` for the same transcript (a cursor
like a hook record, AC-4 tests it).

**Author self-check:** 2026-10-30, before spec review: every promise item has a criterion and a test (item
1: AC-1, AC-7; item 2: AC-2, AC-3, AC-12; item 3: AC-4, AC-5, AC-6; item 4: AC-8; item 5: AC-9; item 6:
AC-1, AC-7, AC-10; item 7: AC-11, AC-13); each Inputs and External states row has a test or "not
promised"; the readers of the changed configuration (`Config` with `[roles]`) are `config.rs`, the token
hooks and `init`.

## Design

### Facts

- F-1: `tokens::import` (`crates/stapel-cli/src/tokens.rs`) sums a window per model and writes the STP-5
  record; it refuses a transcript written in the last 5 minutes. The hooks reuse the sum and the record,
  not the quiet rule.
- F-2: `stapel_core::tokens::read_transcript` keys lines by `message.id`, keeps the fullest usage, and
  marks `complete` at a `stop_reason` line.
- F-3: `Config` (`crates/stapel-core/src/config.rs`) has no `deny_unknown_fields` on the top level; a new
  `roles: Option<BTreeMap<String, String>>` field is read when present.
- F-4: `init::plan_hooks` writes four events with `has_command` against duplicates; the advisory line form
  is that of `SESSION_HOOK_LINE`.
- F-5: the guard denies an agent running `stapel hook` (`HOOK_SELF_REASON`), so an agent cannot write hook
  records through the hook entry point.
- F-6: `tickets::resolve` gives the one open ticket; `State.closed.at` holds the close time.

### Decisions (before the build)

- A new module `crates/stapel-cli/src/token_hook.rs` holds the three hooks; the sum and the record of
  `tokens::import` move into a function both call, so `tokens import` output stays the same.
- Each hook takes an exclusive lock (`File::try_lock` in a loop, at most 20 s, below the 30 s timeout) on
  `tokens.lock` in the git common folder, next to `check.lock`, from reading the cursor to the append;
  there it is outside the tree, so no `.gitignore` line is needed (SR1-11, SR2-2). Rust 1.93 here.
- The hooks become live in this repository with step 6, the last step; from then on this session's records
  land in STP-26's `tokens.jsonl`, so the close does not import those windows by hand (SR2-L5).
- `stop` reads the transcript's lines until the first `sessionId` for the identity, then the records, and
  leaves before reading the rest when the newest record is younger than 10 minutes.
- Every hook body runs in `catch_unwind` with a silent panic hook, as `session-start`; a panic writes
  nothing, because the one write comes last (SR1-7).
- This repository's `.stapel/stapel.toml` gets the `[roles]` table in the GREEN of its step; `stapel init`
  here installs the hooks and the skill, as `init_process::this_repository_has_the_current_files` requires.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `hook_tokens::subagent_stop_records_each_model`, `hook_tokens::partial_messages_as_import` |
| AC-2 | `hook_tokens::roles_from_the_table`, `hook_tokens::roles_without_a_table` |
| AC-3 | `core::config::rejects_bad_roles` |
| AC-4 | `hook_tokens::stop_waits_ten_minutes` |
| AC-5 | `hook_tokens::session_end_records_the_rest` |
| AC-6 | `hook_tokens::window_ends_at_the_last_complete_message` |
| AC-7 | `hook_tokens::repeated_call_adds_nothing`, `hook_tokens::cursor_across_tickets` |
| AC-8 | `hook_tokens::tail_after_close`, `hook_tokens::no_target_ticket` |
| AC-9 | `hook_tokens::unreadable_transcript_gives_a_gap_record` |
| AC-10 | `hook_tokens::bad_input_writes_nothing` |
| AC-11 | `hooks_install::token_hooks_installed` |
| AC-12 | `init::template_has_the_roles_table`, `init_process::this_repository_has_the_roles_table` |
| AC-13 | `init_process::skill_text_names_the_token_hooks` |
| AC-14 | `hook_tokens::broken_line_does_not_stop_the_hooks` |
| AC-15 | `hook_tokens::two_hooks_at_once` |

## Plan

Route: full (tags `data`, `interface`), auto.

1. The `[roles]` table (AC-3, AC-12; tests `core::config::rejects_bad_roles`,
   `init::template_has_the_roles_table`, `init_process::this_repository_has_the_roles_table`): the GREEN
   changes `config.rs`, the template and this repository's `.stapel/stapel.toml`. Must not change: a
   `stapel.toml` without `[roles]` parses as before.
2. `subagent-stop` records (AC-1; tests `hook_tokens::subagent_stop_records_each_model`,
   `hook_tokens::partial_messages_as_import`): the shared sum of `tokens::import`, roles from the default
   rows. Must not change: `tokens import` output and records.
2b. Roles of `subagent-stop` (AC-2; tests `hook_tokens::roles_from_the_table`,
   `hook_tokens::roles_without_a_table`). Must not change: step 2.
3. Cursor, repeat, target ticket and broken lines (AC-7, AC-8, AC-14; tests
   `hook_tokens::repeated_call_adds_nothing`, `hook_tokens::cursor_across_tickets`,
   `hook_tokens::tail_after_close`, `hook_tokens::no_target_ticket`,
   `hook_tokens::broken_line_does_not_stop_the_hooks`). Must not change: step 2.
3b. The lock (AC-15; test `hook_tokens::two_hooks_at_once`). Must not change: steps 2 and 3.
4. `stop` and `session-end` (AC-4, AC-5, AC-6; tests `hook_tokens::stop_waits_ten_minutes`,
   `hook_tokens::session_end_records_the_rest`, `hook_tokens::window_ends_at_the_last_complete_message`).
   Must not change: `subagent-stop` windows.
5. Gap records and bad input (AC-9, AC-10; tests `hook_tokens::unreadable_transcript_gives_a_gap_record`,
   `hook_tokens::bad_input_writes_nothing`). Must not change: steps 2 to 4.
6. Install and skill text (AC-11, AC-13; tests `hooks_install::token_hooks_installed`,
   `init_process::skill_text_names_the_token_hooks`): the GREEN changes `init.rs`, `assets/skill.md`, and
   runs `stapel init` here. Must not change: the four existing hook entries.

## Review

TODO

## Summary

TODO
