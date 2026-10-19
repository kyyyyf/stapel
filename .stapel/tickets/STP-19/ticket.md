# STP-19 — Token journal records imported from a file

## Description

Source: `docs/PHASES.md`, phase 0, STP-19 (split from STP-18 at its frame, Decisions of STP-18 of 2026-10-15);
the next-session note `.stapel/scratch/stp19/next-session.md`.

### Frame

**Problem.** A ticket's `tokens.jsonl` takes records from two commands: `stapel tokens import` sums a Claude
Code transcript, and `stapel tokens add` writes one record from its flags. Usage that is in neither form (a
tool other than Claude Code, a file of several model calls) needs one `tokens add` per call, typed by hand.
The guard denies an agent's Edit and Write to `tokens.jsonl`, so such records have no checked path. STP-18
built that path for findings (`stapel findings import FILE [KEY]`, a permission dialog and a one-time grant);
this ticket gives the token journal the same path, and keeps records from a file apart from the measured
records of a transcript.

**Risk tags:** `guard` (a new permission dialog and grant), `data` (records appended to a journal with
history), `interface` (a new command and a changed report).

**Non-goals.** A dialog for `stapel tokens add` or `stapel tokens import` (they stay as they are, also with
measured counts in an agent's shell); records in the legacy (STP-1) shape; checking or rewriting the records
already in `tokens.jsonl`; editing or removing a record; a Bash command that writes `tokens.jsonl` directly
(STP-20).

**Promise (closed list).**

1. `stapel tokens import-records FILE [KEY]` reads `FILE` as JSON lines and appends every record to the
   ticket's `tokens.jsonl` (without `KEY`: the only open ticket; a closed ticket only by an explicit `KEY`),
   all or nothing.
2. A record of `FILE` has `id` (1 to 64 characters, no whitespace or control character), `role`, `model` and
   `source` (`measured` or `estimate`); a `measured` record has `input` and `output` and may have
   `cache_read` and `cache_write`, an `estimate` record has `estimate` and no measured count; counts are whole
   numbers from 0 to 10^15; `step` and `note` are optional, with the rules of `tokens add`; `ticket` is
   optional and equal to the key; `v` is optional and `1`. Any other field (`at`, `transcript`, `importer`,
   `partial`, `from`, `to`, `messages`, `imported_from`, …) is refused. A record that breaks this, a line
   that is not a JSON object, an empty file, more than 100 records or 256 KiB, an `id` repeated in the file or
   already in `tokens.jsonl` is refused with its line number and nothing is written (exit 2). `FILE` as
   `findings import` takes it: a regular UTF-8 file, not `-`, not a machine file.
3. Each appended record is `{"v":1,"id","at":<now, UTC>,"ticket":<folder key>,"role","model","source",
   <counts>,["step"],["note"],"imported_from":<base name of FILE>}`.
4. When an agent runs the command, the guard runs the same checks, issues a one-time grant whose facts name
   the key, the action and a hash of the records, and asks Claude Code to show the permission dialog, as for
   `findings import`. The dialog shows the count, the file's base name, the sums of the measured counts and of
   the estimates, and one entry per record: `id`, `role`, `model`, `source`, its counts, `step` and the start
   of `note` up to 40 characters; at most 30 records go through the dialog. A refusal of the checks is a
   denial with its reason, without a dialog.
5. The command run with the grant appends only when the records still hash to the granted value; in an
   agent's shell without a grant it refuses, as `findings import` does. A person in a terminal runs it
   without a dialog.
6. `stapel tokens [KEY]` shows records with `imported_from` in their own row per role and model, with the
   role cell `<role> (file)`; the other rows and the columns do not change.

Not promised: deliberate circumvention (no `security` tag); a `FILE` that changes between the dialog and the
run other than by refusal; two imports at the same moment; that the counts in `FILE` are true (the dialog
shows them, the person decides).

**Prior art.** `stapel findings import` (STP-18): `records::plan_findings`, `read_input`, `check_findings`,
the dialog in `crates/stapel-core/src/dialog.rs`, the grant in `grant.rs`, the command in
`crates/stapel-cli/src/findings.rs`. `stapel tokens add` (STP-3): the field rules in
`crates/stapel-cli/src/tokens.rs`. The report: `tokens::report`. The three record shapes in the history:
v1 `measured` (with or without `importer`, `partial`), v1 `estimate`, legacy (`ts`, `kind`,
`total_tokens` or `input_tokens` and `output_tokens`).

**Size.** About 350 changed source lines (`crates/stapel-core`: the token record checks in `records.rs`,
`dialog.rs`, `guard.rs`; `crates/stapel-cli`: the command module, `main.rs`, the report row in `tokens.rs`)
and about 1 000 test lines. STP-18's estimate was low by a factor of two; if the spec grows past about 500
source lines, the ticket is split before the spec review. Route: full (tags `guard`, `data`, `interface`).

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-15: Decision (STP-18 frame, not recommended there): records of `tokens.jsonl` imported from a file
  with the dialog of `findings import`; an imported record may be `measured` and gets `imported_from`; the
  report shows such records apart.
- 2026-10-19: Decision (record shape, recommended): `FILE` holds v1 records without the fields the command
  sets: `v`, `at`, `ticket` and `imported_from` are set by the command. Rejected: full v1 records; v1 and
  legacy.
- 2026-10-19: Decision (repeats, recommended): every record carries `id` with the rule of `findings import`;
  an `id` repeated in `FILE` or already in `tokens.jsonl` is a refusal. Rejected: ids made by the command
  (a second import doubles the records); a hash of the file kept in the records.
- 2026-10-19: Decision (fields, recommended): a closed set of fields; the fields of a transcript import and
  `imported_from` in `FILE` are refused. Rejected: other fields allowed and kept.
- 2026-10-19: Decision (`imported_from`, recommended): the base name of `FILE`, because a full path can hold a
  private term (a user name). Rejected: the path as typed; the path from the repository root.
- 2026-10-19: Decision (report, recommended): own row per role and model with the role cell `<role> (file)`.
  Rejected: a new `origin` column; a separate table.
- 2026-10-19: Decision (dialog, recommended): count, file name, sums, and one entry per record with id, role,
  model, source, counts, step and the start of the note; at most 30 records. Rejected: totals only.
- 2026-10-19: Decision (`tokens add`, recommended): a non-goal; `tokens add` and `tokens import` keep running
  without a dialog. Rejected: the same dialog for `tokens add` with measured counts.
- 2026-10-19: Decision (limits and ticket choice, recommended): as STP-18: 100 records and 256 KiB in `FILE`,
  30 in the dialog; a closed ticket only by an explicit `KEY`; the input file rules of `findings import`.
- 2026-10-19: Decision (estimate records, recommended): every record from a file gets `imported_from` and its
  own report row, `estimate` as well as `measured`. Rejected: only `measured`.
- 2026-10-19: Decision (`model`, recommended): required in `FILE`; no default from `stapel.toml`. Rejected:
  the default of `tokens add`.
- 2026-10-19: Decision (frame): the human confirmed the frame in the chat; `stapel ok` has no section for it.
- 2026-10-19: Decision (SR1-15, recommended): an allow rule that covers `stapel tokens` (such as
  `Bash(stapel tokens:*)`) denies the dialog, as for `findings import`; the person narrows it to
  `Bash(stapel tokens add:*)` and `Bash(stapel tokens import:*)`. No such rule exists in the settings today.
- 2026-10-19: Decision (SR1-13, recommended): the dialog cuts `role`, `model` and `step` to 40 characters as
  it cuts `note`, so 30 records keep the dialog readable.
- 2026-10-19: Assumption: `ticket` may be given in `FILE` and must then equal the folder key, as in
  `findings import`; the command writes the folder key in every case.
- 2026-10-19: Assumption: `at` of every appended record is the time of the run, the same for all records of
  one import.

## Spec

Terms:

- **Import command**: `stapel tokens import-records FILE [KEY]`.
- **Token record**: one JSON object of `FILE`, on one line.
- **Imported record**: a record of `tokens.jsonl` that has the field `imported_from`.
- **Base name**: the last component of `FILE` as typed (`Path::file_name`), without its directories.
- **Ticket choice**: as STP-18 (`records::journal_ticket`): an explicit `KEY` names any open or closed ticket
  in any case; without `KEY`, the only open ticket; a legacy ticket gets `ticket <KEY> has a legacy (STP-1)
  state.json; its journal is kept by hand`.
- **Records hash**: SHA-256 of the base name followed by `\n`, then of each record line of `FILE` as read (a
  trailing `\r` removed, blank lines skipped) followed by `\n`; lowercase hex. It does not cover `at`, so a
  dialog and a later run agree.
- **Refusal**: printed as `stapel: <reason>` on stderr, exit 2; a grant error, the agent-shell refusal, no
  repository and an unreadable `.stapel/stapel.toml` exit 1, as for `findings import` (SR1-16).
- **Short text**: a JSON string that is not blank after trimming, at most 256 bytes, with no control
  character (the rule of `tokens add` for `--role`, `--model` and `--step`).
- **Count**: a JSON number that is a whole number from 0 to 10^15 (`u64`); `1.0`, `1e3`, `-1`, a string and
  `null` are not counts.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN a person runs the import command in a terminal and every record passes AC-2 and AC-3 THE command appends one line per record, in file order and in one write, to the ticket's `tokens.jsonl`: `{"v":1,"id":<id>,"at":<now, UTC, as tokens add writes it, the same for all records>,"ticket":<folder key>,"role":<role>,"model":<model>,"source":<source>,` then the counts given, in the order `input`, `output`, `cache_read`, `cache_write`, `estimate`, then `step` and `note` when given, then `"imported_from":<base name>}`; and prints `imported: <N> token records into <KEY>` (exit 0). A missing `tokens.jsonl` is created; one without a final newline gets one first. | `token_records::import_appends_all_in_order`, `token_records::import_creates_and_repairs_target` |
| AC-2 | IF a token record breaks a field rule THEN the command exits 2 with `stapel: <FILE>:<line>: <field>: <reason>` for the first rule broken in this order, and writes nothing; a missing required field gets `<field>: missing` (SR1-12): `id` missing, not a non-empty string, or not 1 to 64 characters with no whitespace or control character (the texts of `findings import`); `ticket` present and not the folder key (`ticket: must be <KEY>`); `role`, then `model` missing or not a short text (`must be a non-blank string of at most 256 bytes with no control character`); `source` missing or not `measured` or `estimate` (`must be one of measured, estimate`); for `measured`: `input`, then `output` missing, `estimate` present (`estimate: not allowed in a measured record`); for `estimate`: `estimate` missing, or the first of `input`, `output`, `cache_read`, `cache_write` present (`<field>: not allowed in an estimate record`); a count field present and not a count (`must be a whole number from 0 to 10^15`), checked in the order `input`, `output`, `cache_read`, `cache_write`, `estimate`; `step` present and not a short text; `note` present and not a string of at most 4096 bytes with no control character but newline (`must be a string of at most 4096 bytes with no control character but newline`); `v` present and not `1` (`v: must be 1`); then the first other field of the line (`<field>: not allowed in a token record`). Output: `golden/token_records_refusals.txt`. | `token_records::import_refuses_bad_fields`, `token_records::field_limits_count_bytes_and_characters`, `token_records::field_check_order_edges` |
| AC-3 | IF `FILE` has a line that is not a JSON object, a line longer than 64 KiB with its newline (measured after a trailing `\r` is removed, so a CRLF line may have one byte more on disk, as in `findings import`; D1-4), no record (blank lines are skipped), more than 100 records (the 101st record line gets `more than 100 records; split the file` unless it is not a JSON object, which is checked first; a 101st line with a bad field gets the limit reason, D1-3), more than 256 KiB, or an `id` that repeats an earlier line of `FILE` (`id <id> repeats line <n>`) or a record of `tokens.jsonl` (`id <id> is already in .stapel/tickets/<KEY>/tokens.jsonl`, legacy lines included when they have an `id`), THEN the command exits 2 with the reason, prefixed by `<FILE>:<line>: ` when it is about one line, and writes nothing; the texts are those of `findings import`. | `token_records::import_refuses_bad_file`, `token_records::import_refuses_repeated_id`, `token_records::import_accepts_the_limits`, `token_records::file_limit_edges` |
| AC-4 | IF `FILE` is missing, `-`, not a regular file after following symlinks, not UTF-8, or a machine file (as `findings import`: any machine file of a ticket folder, the ledger, compared on the resolved path without case), or `tokens.jsonl` has a line that is not a JSON object (reported after the records pass, `.stapel/tickets/<KEY>/tokens.jsonl:<line>: <reason>`; a line with an unknown `v` is skipped), THEN the command exits 2 with the path and the reason and writes nothing; a missing `FILE` gives `<FILE>: ` and the OS error text (D1-6). | `token_records::import_refuses_bad_input_path`, `token_records::import_refuses_broken_target`, `token_records::missing_file_reason` |
| AC-5 | WHEN the import command names a closed ticket by `KEY` THE command writes as for an open ticket; without `KEY` it takes the only open ticket; no open ticket, several, or a legacy ticket by `KEY` give the refusals of the Ticket choice term (exit 2). The ticket is chosen before `FILE` is read, so a bad `KEY` with a missing `FILE` gets the `KEY` refusal (D1-7). | `token_records::closed_ticket_by_key_only`, `token_records::ticket_chosen_before_file` |
| AC-6 | WHEN an agent runs the import command as one plain command in a dialog permission mode and its checks pass THE guard answers `ask`, writes one grant file, replaces the command by `<program> tokens import-records <FILE> <KEY> --grant <token>`, and gives the reason `stapel: import <N> token records from <base name> into <KEY>[ (closed)]: measured input <sum>, output <sum>, cache read <sum>, cache write <sum>; estimate <sum> \| <id> <role> <model> <source> <counts> step <step or —> note <note or —> \| …; runs: <program>. Answer Yes or No; do not choose "don't ask again".` where `<counts>` is `input <n> output <n> cache read <n or —> cache write <n or —>` for `measured` and `estimate <n>` for `estimate`; a sum of no record is `0`; `role`, `model`, `step` and `note` are cut to 40 characters with `…` added when longer (SR1-13); in the base name and the note every control character is shown as one space (SR1-1). Output: `golden/token_records_dialog.txt`. | `token_records::dialog_for_import`, `token_records::dialog_cuts_the_note` |
| AC-7 | IF the checks of AC-2 to AC-5 fail when the guard prepares the dialog, or `.stapel/stapel.toml` cannot be read as the command reads it, or the permission mode is not a dialog mode, or an allow rule would skip the dialog, or the command has an option (`unexpected option <opt> for stapel tokens import-records`) or not one or two positional words (`usage: stapel tokens import-records FILE [KEY]`) (SR1-6), or `FILE` has more than 30 records (`the permission dialog shows at most 30 token records; split the file into parts of at most 30 records`), THEN the guard denies with that reason and writes no grant; a relative `FILE` is read from the hook's `cwd`; a person in a terminal imports up to 100. | `token_records::guard_denies_what_the_command_refuses`, `token_records::dialog_limits_to_30_records`, `token_records::dialog_usage_denials` |
| AC-8 | WHEN the import command runs with the grant token, its checks pass again, and the key, the action and the records hash are those of the dialog THE command writes as in a terminal and removes the grant; the same token then refuses by AC-3 (exit 2), as its ids are now in `tokens.jsonl`. A check that fails at the run refuses by AC-2 to AC-5 and leaves the grant unused. | `token_records::grant_writes_once`, `token_records::grant_survives_a_failed_check` |
| AC-9 | IF the records hash, the key or the action differs from the dialog's (`FILE` edited, renamed to another base name, another key with records that carry no `ticket` (records that carry `ticket` meet the AC-2 check first, exit 2; SR1-7); a grant of `findings import`, `ledger add` or `ok`), or the grant was used, THEN the command exits 1 with the mismatch text of `findings import` (`the grant does not match: the records, the ticket or the stapel build changed since the dialog, or the grant was used already; ask the agent to run the command again`) and writes nothing; an expired grant gives today's `the grant expired; ask the agent to run the command again`. A grant of the import command opens no other command. | `token_records::grant_names_the_records` |
| AC-10 | IF the import command runs in an agent's shell without a grant THEN it exits 1 with `only a person imports token records: this shell belongs to an agent (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); run the command as one plain command so that Claude Code shows the permission dialog, or, from a Zed task started from a Claude Code shell, run it with env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT` and writes nothing. | `token_records::agent_shell_without_grant_refused` |
| AC-11 | IF an agent adds its own `--grant`, or a word with `$`, after `tokens import-records` (sub-action in any case) THEN the guard denies with `a --grant comes only from the permission dialog; stapel ok, stapel close, stapel findings import, stapel ledger add and stapel tokens import-records may not carry one of their own or words filled in at run time`; this one sentence replaces today's for all five commands. `stapel tokens add …`, `stapel tokens import …`, `stapel tokens` and `stapel tokens <KEY>` stay allowed by the guard without a dialog (SR1-3). | `token_records::guard_denies_own_grant`, `token_records::other_tokens_commands_stay_allowed` |
| AC-12 | WHEN `stapel tokens [KEY]` reads a `source` `measured` or `estimate` record that has the field `imported_from` THE report adds it to its own row per role and model, with the role cell `<role> (file)`, sorted after the rows of records without `imported_from` and the legacy rows of the same role and model (SR1-14); it checks and sums it as other v1 records; other rows and the columns do not change; the sessions table of `stapel tokens` without `KEY` does not count it (SR1-2). Output: `golden/tokens_report_file.txt`. | `token_records::report_shows_imported_rows`, `token_records::report_imported_row_edges`, `token_records::sessions_ignore_imported_without_transcript` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-12; `tokens add`, `tokens import`, `tokens session`,
  `findings import`, `ledger add`, `ok` and `close` keep their behaviour and their tests, except the one
  sentence of AC-11, which the `findings import` tests also quote.
- **Not promised:** the Frame's list; an import command inside a compound command, a pipe or a redirection
  (the guard allows it and the command refuses in an agent's shell by AC-10; STP-21); a literal `$` in `FILE`,
  which the guard denies as a run-time word; ` | ` inside a field shown in the dialog; a `FILE` that starts
  with a UTF-8 byte order mark (refused as not JSON); Unicode format characters in an `id` or a text field;
  duplicate JSON keys in a record (the checks and the written line take the last value); a warning for a
  role that is not in `stapel.toml`; the number of digits of a count in the dialog (shown as is); a record
  in `tokens.jsonl` that has `imported_from` but was written by hand.

## Design

### Facts (external contract and states)

- F-1: the permission dialog and its modes as STP-18 F-1 (PreToolUse `ask` with `updatedInput`; a hook's
  `ask` forces the prompt in auto mode; https://code.claude.com/docs/en/hooks, read 2026-10-15).
- F-2: `guard::plain_invocation` reads one simple command with the actions `ok`, `close`, `findings import`
  and `ledger add`; `check_stapel` denies `--grant` and `$` words after them. `tokens` is not an action of
  either today.
- F-3: `tokens.jsonl` is in `guard::MACHINE_FILES` and `worktree::MACHINE_FILES`: an agent's Edit and Write
  are denied, and a change of it keeps the tree clean for `stapel check`.
- F-4: `records::read_input` gives the input file rules of AC-4; `journal::append_all` writes all lines in
  one write with the final-newline repair; `journal::read` gives `V1`, `Legacy` and `Problem` entries;
  `journal::is_unknown_version` marks a line with an unknown `v`.
- F-5: the three record shapes in the history (`.stapel/tickets/*/tokens.jsonl`, 208 lines, read
  2026-10-19; 228 lines, SR1-14): v1 `measured` (with or without `importer`, `partial`), v1 `estimate` (with or without `step`
  and `note`), legacy (`ts`, `kind`, `total_tokens` or `input_tokens` and `output_tokens`, STP-1 to STP-3).
  No line has `imported_from`, so AC-12 changes no report of the history.
- F-6: `tokens add` writes its record with the key order of insertion, because `stapel-cli` enables the
  `preserve_order` feature of `serde_json`; `stapel-core` does not (SR1-9). `at` comes from
  `time::now_rfc3339()`, `id` from `journal::new_id("t")`.
- F-7: the grant directory is `git rev-parse --git-path stapel/grants` and the grant file name hashes
  `CARGO_PKG_VERSION` (`grant.rs`), so a rebuilt stapel between the dialog and the run is a mismatch.

### Decisions (before the build)

- `stapel_core::records` gets `plan_tokens(root, base, file, key) -> TokensPlan` and `check_tokens`, next to
  `plan_findings`; the CLI (`crates/stapel-cli/src/token_records.rs`) and `dialog::prepare_records` both call
  it, so the dialog refuses what the command refuses. `TokensPlan` holds the key, `closed`, the target, the
  base name, the parsed fields of each record (not lines), the ids, the records hash and the dialog rows.
- The lines to append are built from the parsed fields with `format!` in the key order of AC-1, as
  `check_ledger` builds the ledger line, not with a `serde_json::Map` (F-6, SR1-9); `at` is taken once per
  run.
- The base name is taken after `read_input` succeeds, so `.`, `..` and `/` are refused as not a regular file
  before `Path::file_name` is read (SR1-10).
- Grant `Facts`: action `tokens-import-records`, key the folder key, detail the ids joined by `,`, hash the
  records hash, no dependencies.
- `plain_invocation` reads the action `tokens` with the sub-action `import-records` (any case) as `tokens
  import-records`; `check_stapel` treats `--grant` and `$` words after it as for `findings import`. `tokens
  import`, `tokens add` and `tokens` alone stay outside both.
- The short-text and count rules move from `crates/stapel-cli/src/tokens.rs` into core only if the builder
  needs them there; `tokens add` keeps its texts (`--role must be …`).
- The agent-shell text of AC-10 is a new constant next to `fail::AGENT_SHELL`; `GRANT_REASON` in `guard.rs`
  becomes the sentence of AC-11, and the constant `GRANT_DENIED` in `crates/stapel-cli/tests/records.rs`
  changes with it in the RED commit of that step.
- The report keys a row by role, model and origin: 0 for v1 records without `imported_from`, 1 for legacy, 2
  for imported records; the role cell of origin 2 is `<role> (file)`.

### Decisions (as built)

None yet.

## Test plan

CLI tests in a new file `crates/stapel-cli/tests/token_records.rs`, with `common::repo_with_ticket`,
`common::ask`, `common::hook_bash` and `common::token_of`. Golden files under
`crates/stapel-cli/tests/golden/`. Light tests: `cargo test -j 4 -p stapel-cli --test token_records --
--test-threads=3`; before GREEN also `--test records`, `--test tokens`, `--test fixtures` and `--test
ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Main path | a measured record with all four counts, step and note; a measured record with only `input` and `output`; an estimate record; a record with `ticket` and `v`; the bytes of each line, the key order, one `at` for all, `imported_from` the base name of `sub/in.jsonl`; output line | `token_records::import_appends_all_in_order` |
| State | `tokens.jsonl` missing; ending without a newline | `token_records::import_creates_and_repairs_target` |
| Negative fields | each AC-2 reason once, in the golden order: `id` missing, `""`, `7`; `ticket` of another key; `role` missing, `""`, `"  "`, `7`, with a tab; `model` missing; `source` missing, `Measured`; measured without `input`, without `output`, with `estimate`; estimate without `estimate`, with `input`, with `cache_write`; counts `-1`, `1.0`, `1e3`, `"5"`, `null`; `step` `""`; `note` `7`, with a tab; `v` `2`, `"1"`; fields `at`, `transcript`, `importer`, `partial`, `imported_from`, `kind`; the order of AC-2 by three records that break two rules each: measured with `estimate` and no `input` (`input: missing`), estimate with `input` and `note` `7` (`input: not allowed …`), `at` present and `cache_read` `-1` (`cache_read: must be …`) (SR1-4) | `token_records::import_refuses_bad_fields` |
| Boundaries of fields | `id` of 64 and 65 characters, 64 and 65 Cyrillic characters; a count of 10^15 accepted, 10^15 + 1 refused, `0` accepted; `role`, `model`, `step` of 256 bytes accepted, 257 refused, 128 Cyrillic characters (256 bytes) accepted, 129 refused; `note` of 4096 bytes accepted, 4097 refused, with a newline accepted; `note` `""` accepted | `token_records::field_limits_count_bytes_and_characters` |
| Negative file | a line `[1]`, a broken line, a line of 64 KiB + 1 byte with its newline, only blank lines, 101 records, 256 KiB + 1 byte, an id twice in `FILE`; a bad 101st line gets its own reason | `token_records::import_refuses_bad_file` |
| Boundaries of file | exactly 100 records; a line of exactly 64 KiB with its newline (padded with spaces); a file of exactly 256 KiB; CRLF lines | `token_records::import_accepts_the_limits` |
| Repeated id | an id already in `tokens.jsonl` from `tokens add`; a legacy line with that `id`; a line with `v: 2` and that id is skipped | `token_records::import_refuses_repeated_id` |
| Input path | missing, a directory, `sub/..`, a FIFO, `-`, Latin-1 bytes, the ticket's own `tokens.jsonl`, another ticket's `findings.jsonl`, the ledger, a symlink to `tokens.jsonl`, `.STAPEL/tickets/ABC-1/TOKENS.jsonl`; a symlink to a regular file is read | `token_records::import_refuses_bad_input_path` |
| Broken target | a non-JSON line in `tokens.jsonl` with good records; the same with a bad record: the record's reason comes first | `token_records::import_refuses_broken_target` |
| Closed ticket | closed by key in upper and lower case; without key and one open ticket; no open; two open; a legacy ticket by key; a key with no folder (SR1-8) | `token_records::closed_ticket_by_key_only` |
| Dialog | reason, replaced command and one grant file; `stapel` and `cargo run … --` forms; two measured and one estimate record, sums; no estimate record gives `estimate 0`; only estimate records give `measured input 0, output 0, cache read 0, cache write 0` (SR1-5); a measured record without cache counts shows `—`; no step and no note show `—`; a closed ticket | `token_records::dialog_for_import` |
| Dialog boundaries | a note of 40 characters whole, 41 cut with `…`, 41 Cyrillic characters cut at 40; a role, a model and a step of 40 characters whole and 41 cut (SR1-13); a note with a newline shown with a space; a base name with a tab (SR1-1) | `token_records::dialog_cuts_the_note` |
| Dialog negative | a bad record; `bypassPermissions`; an allow rule `Bash(stapel tokens:*)`; a broken `stapel.toml`; a relative `FILE` from a subdirectory as `cwd`; no grant file in each denial | `token_records::guard_denies_what_the_command_refuses` |
| Dialog limit | 30 records ask; 31 deny with no grant; 31 in a terminal import | `token_records::dialog_limits_to_30_records` |
| Dialog usage | `--role x`, `--note=y`, three positional words, no positional word | `token_records::dialog_usage_denials` |
| Grant | used once, then refused by AC-3; a record added to `tokens.jsonl` by `tokens add` with an id of `FILE` after the dialog refuses by AC-3 and the grant then writes once that line is gone | `token_records::grant_writes_once`, `token_records::grant_survives_a_failed_check` |
| Grant mismatch | `FILE` edited in a count; `FILE` copied to another base name; another key, records without `ticket` (SR1-7); a `findings-import` grant and an `ok` grant on the import command; an import grant on `findings import`; used twice; expired | `token_records::grant_names_the_records` |
| Agent shell | without grant | `token_records::agent_shell_without_grant_refused` |
| Own grant | `--grant x`, `--grant=x`, `$(cat f)` as `FILE`, `TOKENS IMPORT-RECORDS … --grant x`; `stapel tokens import … $x` is not denied by this rule; `findings import … --grant x` gets the new sentence | `token_records::guard_denies_own_grant` |
| Coverage (step 1c) | two other fields `zzz` then `at` give `zzz: not allowed in a token record` (D1-1); `output` `-1` with `cache_read` `-1` gives `output`; `role` and `model` both missing gives `role`; `id` `"  "`; `model` with DEL; `note` with `\r`; `step` `null` (D1-2, D1-6) | `token_records::field_check_order_edges` |
| Coverage (step 1c) | a 101st line with `role` missing gets the limit reason (D1-3); a CRLF line of 64 KiB with its LF plus a CR is accepted, one byte more is refused (D1-4) | `token_records::file_limit_edges` |
| Coverage (step 1c) | a missing `FILE` gives `stapel: nope.jsonl: No such file or directory (os error 2)` (D1-6) | `token_records::missing_file_reason` |
| Coverage (step 1c) | `KEY` `NOPE-1` with a missing `FILE` gives the `KEY` refusal (D1-7) | `token_records::ticket_chosen_before_file` |
| Coverage (step 2c) | a plain `builder` record of `claude-sonnet-5-5` sorts after the imported `builder (file)` row of `claude-opus-5-5` (D2-1); a hand-written imported measured record with `partial` `0` and one with `source` `other` (AC-2 refuses both at the import) each give a `problem:` line and are left out of the sums (D2-2); two imported measured and two imported estimate records of one role and model are summed in one row (D2-3) | `token_records::report_imported_row_edges` |
| Coverage (step 2c) | an imported measured record without `transcript` of the same model next to a matching session leaves the session row as it is without it (D2-4) | `token_records::sessions_ignore_imported_without_transcript` |
| Other tokens commands | in mode `auto` the hook allows (exit 0, no JSON) `stapel tokens add --role r --estimate 1`, `stapel tokens import t.jsonl --role r`, `stapel tokens` and `stapel tokens ABC-1` (SR1-3) | `token_records::other_tokens_commands_stay_allowed` |
| Report | one ticket with a transcript record, a `tokens add` record, a legacy line and two imported records of the same role and model (one measured, one estimate) and one imported record of another role: rows and order by the golden file; a ticket of the history shape without `imported_from` keeps `golden/tokens_report.txt`; with a `.stapel/sessions.jsonl` whose session matches the transcript record, `stapel tokens` without `KEY` counts only the transcript record's output in the sessions table (SR1-2) | `token_records::report_shows_imported_rows` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `FILE` (positional of `tokens import-records`) | path to a regular UTF-8 file, symlinks followed | 1 record; 100 records (30 in the dialog), 256 KiB, a line of 64 KiB with its newline | no record: refused | missing, directory, FIFO, `-`, not UTF-8, machine file: refused | base name kept as typed | `token_records::import_refuses_bad_file`, `token_records::import_refuses_bad_input_path`, `token_records::import_accepts_the_limits` |
| Token record | JSON object, one per line, closed field set | `id`, `role`, `model`, `source` and the counts its source needs | blank line: skipped | AC-2 and AC-3 refusals | exact strings, case-sensitive `source` | `token_records::import_refuses_bad_fields` |
| `id` | string | 1 character; 64 characters (not bytes) | `""`: refused | whitespace, control character, not a string, repeated: refused | exact | `token_records::field_limits_count_bytes_and_characters`, `token_records::import_refuses_repeated_id` |
| `role`, `model`, `step` | short text | 1 non-blank character; 256 bytes (not characters) | blank: refused | control character, not a string: refused | kept as given | `token_records::field_limits_count_bytes_and_characters` |
| `note` | string | `""`; 4096 bytes | `""`: accepted | control character other than newline, not a string: refused | kept as given | `token_records::field_limits_count_bytes_and_characters` |
| `input`, `output`, `cache_read`, `cache_write`, `estimate` | count | 0; 10^15 | absent: by `source` (AC-2) | negative, fraction, exponent form, string, null, above 10^15: refused | whole tokens | `token_records::import_refuses_bad_fields`, `token_records::field_limits_count_bytes_and_characters` |
| `source` | string | `measured` or `estimate` | absent: refused | other text, other case, not a string: refused | exact | `token_records::import_refuses_bad_fields` |
| `ticket` (in a record) | string | the folder key | absent: the folder key is written | another key, not a string: refused | exact | `token_records::import_refuses_bad_fields` |
| `v` (in a record) | number | `1` | absent: `1` is written | `2`, `"1"`: refused | exact | `token_records::import_refuses_bad_fields` |
| `KEY` (positional) | ticket folder name, any case | one ticket | absent: the only open ticket | no such folder, legacy, none or several open without key: refused | ASCII case ignored | `token_records::closed_ticket_by_key_only` |
| `--grant` | 32 hex characters, set by the guard | one grant per dialog | absent: terminal path or AC-10 | wrong, expired, reused, other action: AC-9 | exact | `token_records::grant_writes_once`, `token_records::grant_names_the_records` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | permission mode; whether the dialog is offered | dialog modes ask (auto included, F-1); others deny | `token_records::dialog_for_import`, `token_records::guard_denies_what_the_command_refuses` |
| claude-code | environment of hook processes versus the agent's Bash shell | an agent shell without grant is refused | `token_records::agent_shell_without_grant_refused` |
| claude-code | settings at user, project and local level; allow rules that skip the dialog | a matching allow rule denies, as `findings import` reads it | `token_records::guard_denies_what_the_command_refuses` |
| claude-code | transcripts: main and subagent files, resumed sessions, compaction | not read: `FILE` is not a transcript | — |
| claude-code | tool call succeeded or failed | not read | — |
| os | case-insensitive file systems | a machine file as `FILE` is refused in any case | `token_records::import_refuses_bad_input_path` |
| os | symlinks | a symlink to a regular file is read; to a machine file refused; the base name is that of `FILE` as typed, not of the link target | `token_records::import_refuses_bad_input_path` |
| os | file permissions | an unreadable `FILE` or an unwritable `tokens.jsonl`: refused with the OS error and the path, exit 2; not promised (SR1-11) | — |
| os | time: clock skew, time zones | `at` is the UTC time of the run; the records hash has no time; skew not promised | `token_records::import_appends_all_in_order` |
| os | processes, WSL | not touched | — |
| git | linked worktrees; configuration | the grant lives in the git directory (F-7) of each worktree, as STP-18; not promised (SR1-11) | — |
| git | other dimensions | not touched: the command reads no history | — |
| cargo | stapel rebuilt between the dialog and the run | the grant does not match, AC-9 (F-7) | `token_records::grant_names_the_records` |
| cargo | other dimensions | not touched | — |

### Abuse table

Attempts of a careless agent against "only a person approves token records" and "a grant names its records".

| Attempt | Outcome |
|---|---|
| The agent runs the import command in its own shell without a grant | refused, AC-10 |
| The agent adds its own `--grant`, or builds `FILE` at run time | the guard denies, AC-11 |
| The agent edits `FILE` or renames it after the dialog | the records hash changes: "does not match", AC-9 |
| The agent imports into another ticket with the grant of one | the key differs: "does not match", AC-9, or the `ticket` check, AC-2 |
| A grant of `findings import`, `ledger add` or `ok` used for the import, or the reverse | the action differs: "does not match", AC-9 |
| The agent writes a record that looks like a transcript import (`transcript`, `importer`, `partial`) | refused as a field not allowed, AC-2 |
| The agent sets `imported_from` itself to hide the origin | refused, AC-2; the command sets it |
| The agent imports `tokens.jsonl` of another ticket as `FILE` | refused as a machine file, AC-4 |
| The agent writes `tokens.jsonl` with Edit or Write | denied today (F-3) |
| The agent appends with `cat >>` or a script | not promised (Frame non-goals; STP-20) |
| The agent writes false counts | not promised; the dialog shows every record and the sums |

**Review Focus:** a record whose `source` is `estimate` with `cache_read` (AC-2 test); the report order when
an imported row and a transcript row share role and model (report test); a base name that is a directory
path ending in `/` (refused as not a regular file, AC-4 input path test with a directory); a `FILE` path with
`..` (base name `..` is a directory, refused); a dialog shown before midnight UTC and run after it (the
records hash has no time by its Terms definition; not tested).

**Author self-check:** 2026-10-19, before spec review: every promise item has a criterion and a test; each
limit has its accepted and refused boundary in the test plan, bytes and characters named; the Inputs and
External states rows were checked against `records.rs`, `dialog.rs`, `guard.rs`, `grant.rs`, `journal.rs`,
`tokens.rs` and the catalogue; the record shapes were checked against every `tokens.jsonl` (F-5); no open
question is left.
2026-10-19, after spec review round 1 (external): 0 HIGH, 4 MEDIUM, 12 LOW, all inside; every finding
applied as one change (SR1-1 to SR1-16); new test `token_records::other_tokens_commands_stay_allowed` for
AC-11; Facts F-6 corrected and F-7 added; no round 2.
2026-10-19, after the drift review of step 1: D1-1 to D1-8 (1 MEDIUM, 7 LOW); one spec change: plan step 1c
with four coverage tests, AC-3 says how a CRLF line is measured, AC-5 says the ticket is chosen first; D1-5
(a write error exits 2, as `findings import` and External states say) and D1-8 (the step-1 grant stub, tested
in step 3; duplicate keys not promised) need no change.
2026-10-19, after the drift review of step 2: D2-1 to D2-5 (2 MEDIUM, 3 LOW); one spec change: plan step 2c
with two coverage tests (D2-1 to D2-4); D2-5 (a hand-written `imported_from`) is not promised by
Guarantees and needs no change.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `token_records::import_appends_all_in_order`, `token_records::import_creates_and_repairs_target` |
| AC-2 | `token_records::import_refuses_bad_fields`, `token_records::field_limits_count_bytes_and_characters`, `token_records::field_check_order_edges` |
| AC-3 | `token_records::import_refuses_bad_file`, `token_records::import_refuses_repeated_id`, `token_records::import_accepts_the_limits`, `token_records::file_limit_edges` |
| AC-4 | `token_records::import_refuses_bad_input_path`, `token_records::import_refuses_broken_target`, `token_records::missing_file_reason` |
| AC-5 | `token_records::closed_ticket_by_key_only`, `token_records::ticket_chosen_before_file` |
| AC-6 | `token_records::dialog_for_import`, `token_records::dialog_cuts_the_note` |
| AC-7 | `token_records::guard_denies_what_the_command_refuses`, `token_records::dialog_limits_to_30_records`, `token_records::dialog_usage_denials` |
| AC-8 | `token_records::grant_writes_once`, `token_records::grant_survives_a_failed_check` |
| AC-9 | `token_records::grant_names_the_records` |
| AC-10 | `token_records::agent_shell_without_grant_refused` |
| AC-11 | `token_records::guard_denies_own_grant`, `token_records::other_tokens_commands_stay_allowed` |
| AC-12 | `token_records::report_shows_imported_rows`, `token_records::report_imported_row_edges`, `token_records::sessions_ignore_imported_without_transcript` |

## Plan

Route: full (tags `guard`, `data`, `interface`), pinned at the frame.

1. The import command in a terminal: `records::plan_tokens` and its checks, the CLI command, the append
   (AC-1 to AC-5). Must not change: `findings import`, `ledger add`, `tokens add`, `tokens import`,
   `journal::append_all`.
1c. Coverage after the drift review of step 1 (D1-1 to D1-4, D1-6, D1-7). Tests only: four new `#[test]`
   functions, each helper nested inside its function.
2. The report row of imported records (AC-12). Must not change: the other rows, the columns, the sessions
   table, `golden/tokens_report.txt` and `golden/tokens_report_all.txt`.
2c. Coverage after the drift review of step 2 (D2-1 to D2-4). Tests only: two new `#[test]` functions, each
   helper nested inside its function, expected texts inline.
3. The dialog, the grant, the agent-shell refusal and the own-grant denial (AC-6 to AC-11); the RED commit
   also changes `GRANT_DENIED` in `crates/stapel-cli/tests/records.rs` to the AC-11 sentence. Must not
   change: the `ok`, `close`, `findings import` and `ledger add` dialogs and their grant facts. The tests
   `token_records::agent_shell_without_grant_refused` (built in step 1) and
   `token_records::other_tokens_commands_stay_allowed` (today's guard) pass before the code, so they are not
   in the RED.
3c. Coverage after the GREEN of step 3: the two tests above, and any other test of step 3 that passed at its
   RED. Tests only: new `#[test]` functions, each helper nested inside its function.

## Review

TODO

## Summary

TODO
