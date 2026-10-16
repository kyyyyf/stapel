# STP-18 — Machine records through stapel: findings import and ledger add

## Description

Source: `docs/PHASES.md`, phase 0, STP-18 (asked by the human at the close of STP-14); the next-session note
`.stapel/scratch/stp18/next-session.md`.

### Frame

**Problem.** Review findings and process-ledger records are machine records: JSON lines with history, read by
the measures after a close. The guard denies an agent's Edit and Write to a ticket's `findings.jsonl`, so in
STP-14 the human typed nine `cat … >> findings.jsonl` commands and one for the ledger. Nothing checked those
lines: the history holds `round` as a number and as a string, about 80 forms of `fate`, and records without
`stage` or `promise`. `.stapel/process-ledger.jsonl` is not protected at all: an agent's Edit or Write to it
is allowed today.

**Risk tags:** `guard` (new permission dialogs and grants; the ledger becomes a machine file), `data` (records
appended to files with history), `interface` (two new commands).

**Non-goals.** Importing token journal records from a file (split to its own ticket, Decisions); writes to
`runs.jsonl`, `state.json` and `decisions.jsonl`; a Bash command that writes these files directly (`cat >>`,
`tee`, a script), which the guard does not see today; checking or rewriting the records already in the files;
editing or removing a record.

**Promise (closed list).**

1. `stapel findings import FILE [KEY]` reads `FILE` as JSON lines and appends every record to the ticket's
   `findings.jsonl` (without `KEY`: the only open ticket), all or nothing.
2. Each record must have `id`, `ticket` (equal to the key), `stage`, `round` (a number or a string), `role`,
   `severity` (`HIGH`, `MEDIUM`, `LOW`), `promise` (`inside`, `outside`), `claim`, `catchable_at` (`spec`,
   `design`, `test-plan`, `code`) and `fate` (non-empty text); `stage` and `role` come from closed lists;
   `rule` is optional text; other fields are allowed and kept. A record that breaks this, a line that is not a
   JSON object, an empty file, an `id` repeated in the file or already in `findings.jsonl` is refused with its
   line number and nothing is written (exit 2).
3. `stapel ledger add --kind KIND --note TEXT [KEY]` appends one record `{"v": 1, "at": <today, UTC>, "kind",
   "ticket", "note"}` to `.stapel/process-ledger.jsonl`; `KIND` is `finding`, `catches` or `decision`; the
   note must not be empty.
4. When an agent runs either command, the guard runs the same checks, issues a one-time grant whose facts name
   the key, the action and a hash of the records, and asks Claude Code to show the permission dialog, as for
   `stapel ok`. The dialog shows the count, the totals by severity, and one line per record: `id`, `severity`,
   `promise`, and the start of `claim` up to 80 characters (for the ledger: kind, ticket and the start of the
   note up to 200 characters; the replaced command shows the whole note). A
   refusal of the checks is a denial with its reason, without a dialog.
5. The command run with the grant appends only when the records still hash to the granted value; in an agent's
   shell without a grant it refuses, as `stapel ok` does. A person in a terminal runs it without a dialog.
6. `.stapel/process-ledger.jsonl` becomes a machine file: the guard denies an agent's Edit and Write to it.
7. A change of `.stapel/process-ledger.jsonl` does not make the tree dirty or a check stale, as for the
   machine files of a ticket (`worktree::is_machine_file`; SR1-16, widened by the human).

Not promised: deliberate circumvention (no `security` tag); a `FILE` that changes between the dialog and the
run other than by refusal; two imports at the same moment; records with a `ticket` field in another case
than the folder.

**Prior art.** `stapel ok KEY` (STP-17): the guard prepares, issues a grant over the hashes and asks
(`crates/stapel-core/src/dialog.rs`, `grant.rs`); `stapel tokens import` appends to a journal with
`stapel_core::journal::append`. klc keeps findings in a file the reviewer writes directly.

**Size.** About 430 changed source lines (`crates/stapel-core`: a records module for the checks, `dialog.rs`,
`guard.rs`; `crates/stapel-cli`: two command modules, `main.rs`) and about 1 000 test lines. With the token
import it was about 580 and 1 400, so it was split. Route: full (tags `guard`, `data`, `interface`).

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-15: Decision (record check, recommended): the required fields strictly, closed lists for
  `severity`, `promise`, `catchable_at`, `role` and `stage`, `round` a number or a string, `fate` free text,
  other fields allowed. Rejected: an exact field set; presence only.
- 2026-10-15: Decision (dialog for many records, recommended): count, totals by severity, one line per record
  with id, severity, promise and the start of the claim. Rejected: totals only; the full JSON.
- 2026-10-15: Decision (ledger command, recommended): flags `--kind` and `--note`; `v` and `at` set by the
  command. Rejected: a file of records; both forms.
- 2026-10-15: Decision (token journal, not recommended): records of `tokens.jsonl` imported from a file with
  the same dialog; an imported record may be `measured` and gets the field `imported_from` with the file name;
  the report shows such records apart. Rejected: leave it out; only `estimate`.
- 2026-10-15: Decision (split, recommended): the token record import is its own ticket after STP-18, because
  the promise grew to about 580 source and 1 400 test lines. Rejected: one ticket.
- 2026-10-15: Decision (ledger file, recommended): `.stapel/process-ledger.jsonl` becomes a machine file.
  Rejected: only the new command.
- 2026-10-15: Decision (frame): the human confirmed the frame in the chat; `stapel ok` has no section for it.
- 2026-10-15: Decision (closed ticket, recommended): both commands accept a closed ticket only by an explicit
  `KEY`; without `KEY` they take the only open ticket; the dialog says that the ticket is closed. Rejected:
  open tickets only.
- 2026-10-15: Decision (target file, recommended): a line of the target file that is not a JSON object is a
  refusal with its line number; a missing final newline is added before the append; a missing file is
  created. Rejected: a refusal for every defect.
- 2026-10-15: Decision (limits, recommended): at most 100 records and 256 KB in `FILE`; more is a refusal that
  advises to split the file. Rejected: no limit.
- 2026-10-15: Decision (input file, recommended): `FILE` is a regular UTF-8 file anywhere; a machine file, a
  directory, `-` (stdin) and a file that is not UTF-8 are refused. Rejected: stdin, which the guard cannot show.
- 2026-10-15: Decision (SR1-16, recommended): the ledger also counts as a machine file for `stapel check`, so
  `ledger add` during a build leaves the tree clean (promise item 7). Rejected: not promised.
- 2026-10-15: Assumption: `claim` is required, because all 669 records in the history have it.
- 2026-10-15: Assumption: the `stage` list for new records is `spec-review`, `step-drift`, `code-review`,
  `after-close`; the round goes in `round`, not in the stage name (the history has `spec-review-2`).
- 2026-10-15: Assumption: the `role` list is `external`, `drift`, `reviewer`, `fresh-code`, `builder`,
  `orchestrator`, `human` (the first six are in the history).
- 2026-10-15: Assumption: `ledger add` without `KEY` takes the only open ticket, as `tokens import` does; a
  record without `ticket` (two in the history) is not promised.

## Spec

Terms:

- **Record command**: `stapel findings import FILE [KEY]` or `stapel ledger add --kind KIND --note TEXT [KEY]`.
- **Target file**: the ticket's `findings.jsonl` for `findings import`; `.stapel/process-ledger.jsonl` for
  `ledger add`.
- **Ticket choice**: an explicit `KEY` names any open or closed ticket (any case, as `tickets::resolve`);
  without `KEY`, the only open ticket; no open ticket or several give today's refusal of `tickets::resolve`;
  a legacy ticket gets `ticket <KEY> has a legacy (STP-1) state.json; its journal is kept by hand` and an
  unreadable one its reason, as `tokens::journal_ticket` gives (SR1-2).
- **Records hash**: for `findings import`, SHA-256 of the exact lines the command would append, in order; for
  `ledger add`, SHA-256 of `kind`, `ticket` and `note`, without `at` (SR1-6).
- **Refusal**: a record command's refusal is printed as `stapel: <reason>` on stderr, exit 2; a grant error
  exits 1, as for `stapel ok` (SR1-8).
- **Dialog text**: in a claim or a note shown in the dialog, every control character (newline included) is
  shown as one space (SR1-10).

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN a person runs `stapel findings import FILE [KEY]` in a terminal and every record passes AC-2 THE command appends all records, in file order and with their fields as given, to the target file in one write, and prints `imported: <N> findings into <KEY>` (exit 0). A missing target file is created; a target file without a final newline gets one first. | `records::import_appends_all_in_order`, `records::import_creates_and_repairs_target` |
| AC-2 | IF a record of `FILE` lacks a required field, has a wrong type, or has a value outside its list (Frame promise item 2; `id`: 1 to 64 characters with no whitespace or control character (D3-1); `claim` and `fate`: non-empty strings, SR1-7; `stage`: `spec-review`, `step-drift`, `code-review`, `after-close`; `role`: `external`, `drift`, `reviewer`, `fresh-code`, `builder`, `orchestrator`, `human`), its `ticket` differs from the folder key, `rule` is present and not a non-empty string, or `v` is present and not `1` (SR1-3), THEN the command exits 2 with `stapel: <FILE>:<line>: <field>: <reason>` for the first such record and writes nothing. Output: `golden/records_refusals.txt`. | `records::import_refuses_bad_fields`, `records::import_refuses_unsafe_id` |
| AC-3 | IF `FILE` has a line that is not a JSON object, a line longer than 64 KiB with its newline (as `journal::MAX_LINE`; SR1-3, D2-3), no record (blank lines are skipped), more than 100 records, more than 256 KiB, or an `id` that repeats an earlier line of `FILE` or a record of the target file, THEN the command exits 2 with the line number and the reason and writes nothing. | `records::import_refuses_bad_file`, `records::import_refuses_repeated_id`, `records::import_accepts_the_limits` |
| AC-4 | IF `FILE` is missing, `-`, not a regular file after following symlinks (a directory, a FIFO, a device; checked before it is read, SR1-4), not UTF-8, or a machine file (any `MACHINE_FILES` name of a ticket folder, or the ledger; compared on the resolved path without case, SR1-15), or the target file has a line that is not a JSON object, THEN the command exits 2 with the path and the reason and writes nothing. | `records::import_refuses_bad_input_path`, `records::import_refuses_broken_target` |
| AC-5 | WHEN a person runs `stapel ledger add --kind KIND --note TEXT [KEY]` THE command appends `{"v":1,"at":"<UTC date YYYY-MM-DD>","kind":KIND,"ticket":<folder key>,"note":TEXT}` to `.stapel/process-ledger.jsonl` and prints `recorded: <KIND> for <KEY>` (exit 0); a missing ledger is created and a missing final newline added. | `records::ledger_add_appends_one_record` |
| AC-6 | IF `KIND` is not `finding`, `catches` or `decision`, or `TEXT` is empty after trimming, or the ledger has a line that is not a JSON object, THEN `ledger add` exits 2 with the reason and writes nothing. | `records::ledger_add_refusals` |
| AC-7 | WHEN a record command names a closed ticket by `KEY` THE command writes as for an open ticket; without `KEY` it takes the only open ticket. | `records::closed_ticket_by_key_only` |
| AC-8 | IF an agent's Edit, Write, MultiEdit or NotebookEdit targets `.stapel/process-ledger.jsonl` (any case) THEN the guard denies with today's machine-file reason. | `records::guard_denies_ledger_writes` |
| AC-9 | WHEN an agent runs `findings import` as one plain command in a dialog permission mode and its checks pass THE guard answers `ask`, writes one grant file, replaces the command by `<program> findings import <FILE> <KEY> --grant <token>`, and gives the reason `stapel: import <N> findings into <KEY>[ (closed)]: <H> HIGH, <M> MEDIUM, <L> LOW \| <id> <severity> <promise> <claim, 80 characters, then …> \| …; runs: <program>. Answer Yes or No; do not choose "don't ask again".` Output: `golden/records_dialog.txt`. | `records::dialog_for_findings_import`, `records::dialog_cuts_at_the_limits` |
| AC-10 | IF the checks of AC-2 to AC-4 or AC-6 fail when the guard prepares the dialog, or the permission mode is not a dialog mode, or an allow rule would skip the dialog, THEN the guard denies with the same reason as the command or as `stapel ok`, and writes no grant. | `records::guard_denies_what_the_command_refuses` |
| AC-11 | WHEN a record command runs with the grant token, its checks pass again, and the ticket key, the action and the records hash are those of the dialog THE command writes as in a terminal and removes the grant; the same token then refuses again: `ledger add` with the mismatch reason of AC-14, `findings import` by AC-3 (exit 2), because its ids are now in the target. A check that fails at the run (an id added to the target meanwhile) refuses by AC-2 to AC-6 and leaves the grant unused (SR1-1, D3-2). | `records::grant_writes_once`, `records::ledger_grant_survives_a_failed_check` |
| AC-12 | IF a record command runs in an agent's shell without a grant THEN it exits 1 with `only a person writes review records and ledger records: this shell belongs to an agent (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); run the command as one plain command so that Claude Code shows the permission dialog, or, from a Zed task started from a Claude Code shell, run it with env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT` (SR1-14) and writes nothing. | `records::agent_shell_without_grant_refused` |
| AC-13 | WHEN an agent runs `ledger add` as one plain command in a dialog permission mode and its checks pass THE guard answers `ask`, writes one grant file, replaces the command by `<program> ledger add --kind <KIND> --note <TEXT> <KEY> --grant <token>`, and gives the reason `stapel: add a ledger record for <KEY>[ (closed)]: <KIND>: <note, 200 characters, then …>; runs: <program>. Answer Yes or No; do not choose "don't ask again".` Output: `golden/records_dialog.txt`. | `records::dialog_for_ledger_add` |
| AC-14 | IF the records hash, the key or the action differs from the dialog's (`FILE`, the note, the kind or the key changed; for `findings import` another key meets the `ticket` check of AC-2 first, exit 2, D3-2), or the grant was used, THEN the record command exits 1 with `the grant does not match: the records, the ticket or the stapel build changed since the dialog, or the grant was used already; ask the agent to run the command again` and writes nothing; an expired grant gives today's "the grant expired". A grant of one record command does not open the other, nor `stapel ok` (SR1-5). | `records::grant_names_the_records` |
| AC-15 | IF an agent adds its own `--grant` to a record command, or a word with `$` after the action, THEN the guard denies with `a --grant comes only from the permission dialog; stapel ok, stapel close, stapel findings import and stapel ledger add may not carry one of their own or words filled in at run time` (SR1-5). | `records::guard_denies_own_grant` |
| AC-16 | WHEN `.stapel/process-ledger.jsonl` is the only changed path THE `stapel check` command reads the tree as clean and keeps its check fresh, as for the machine files of a ticket. | `records::ledger_change_keeps_tree_clean`, `records::ledger_change_keeps_check_fresh` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-16; the existing tests of `ok`, `close` and `hook` keep
  their behaviour.
- **Not promised:** the Frame's list; a record command inside a compound command, a pipe or a redirection
  (the guard allows it as today and the command refuses in an agent's shell by AC-12); a `--note` longer than
  the 64 KiB journal line, which `journal::append` refuses as today; a run outside a stapel repository, which exits 1 as every command does (D1-2); a literal `$` in `--note` or in `FILE`, which the guard denies as a run-time word (AC-15, SR1-9); ` | ` inside a claim or a note, which the dialog shows as is; a flag form that the guard reads and clap refuses (a repeated `--kind`, `--note -x`), which gets a dialog and then a usage error with no write (D3-5); Unicode line separators and bidi marks in a claim or a note (D3-6).

## Design

### Facts (external contract and states)

- F-1: a PreToolUse `ask` with `updatedInput` shows the modified input in the permission prompt; a hook's
  `ask` forces the prompt in auto mode as well (the classifier may deny, never approve silently; Claude Code
  2.1.211 and later; this machine runs 2.1.300). Deny and ask rules of the settings still apply; several hooks
  combine as `deny` > `defer` > `ask` > `allow` (https://code.claude.com/docs/en/hooks, "PreToolUse decision
  control", read 2026-10-15).
- F-2: `guard::plain_invocation` reads only a command that is exactly one simple command, and today only the
  actions `ok` and `close`; a compound command returns `None` and the guard allows it (the `cd …; stapel ok`
  case seen at the frame).
- F-3: a grant file name is a hash of the token and `Facts` (key, action, detail, hash, dependencies, crate
  version); `grant::consume` removes the file whatever the outcome.
- F-4: `journal::append` writes one record per call and adds a missing final newline; a line is at most
  64 KiB. A multi-record append in one write needs a new function next to it.
- F-5: `guard::is_machine_file` compares lowercase path components; the ledger is a two-component path
  `.stapel/process-ledger.jsonl`, like `stapel.toml`.

### Decisions (before the build)

- A new module `stapel_core::records` holds the record checks, the lines to append and the records hash; the
  CLI and `dialog::prepare_ask` both call it, so the dialog refuses what the command refuses (as
  `confirm::prepare` for `ok`).
- Grant `Facts`: action `findings-import` or `ledger-add`, key the folder key, detail the ids joined by `,`
  (for the ledger the kind), hash the records hash, no dependencies. For the ledger the hash covers `kind`,
  `ticket` and `note`, not `at`, so a dialog before midnight UTC still matches after it.
- `plain_invocation` reads the actions `findings` (sub-action `import`) and `ledger` (sub-action `add`);
  `check_stapel` treats `--grant` and `$` words after them as for `ok` and `close`.
- The repeated-id check reads the target file as `journal::read` does; a legacy line counts by its `id`.
- At the run with a grant the command first runs every check of AC-1 to AC-7, then consumes the grant, then
  writes; so a check that fails leaves the grant to expire (AC-11).
- A refusal type in the CLI carries the exit code (2 for record checks, 1 for grant errors and the agent
  shell); `main.rs` maps it, and every other command keeps exit 1 (SR1-8).
- A new `journal::append_all` writes all lines in one `write` call, with the same final-newline repair as
  `append` (F-4).
- The mismatch text of AC-14 is new and used only by the record commands; `ok` and `close` keep theirs. The
  guard's grant reason (`GRANT_REASON`) becomes the one sentence of AC-15 for all four commands; no existing
  test quotes the old sentence.

## Test plan

CLI tests in a new file `crates/stapel-cli/tests/records.rs`, with `common::repo_with_ticket`,
`common::ask`, `common::hook_bash` and `common::token_of`. Golden files under
`crates/stapel-cli/tests/golden/`. Light tests: `cargo test -j 4 -p stapel-cli --test records --
--test-threads=3`; before GREEN also `--test fixtures` and `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Main path | three findings with extra fields `note`, `step`; order and bytes kept; output line | `records::import_appends_all_in_order` |
| State | target missing; target ending without a newline | `records::import_creates_and_repairs_target` |
| Negative fields | each required field missing; `id` as `""`, `7` and `null`; `claim` `""`; `round` as `true`, `""` and `-1`; `severity` `high`; `stage` `spec-review-2`; `role` `nobody`; `ticket` of another key; `rule` `""`; `fate` `""`; `v` `2` and `"1"` | `records::import_refuses_bad_fields` |
| Negative file | a line `[1]`, a broken line, a line of 64 KiB + 1 byte, only blank lines, 101 records, 256 KiB + 1 byte, an id twice in `FILE` | `records::import_refuses_bad_file` |
| Coverage (step 1c) | exactly 100 records, a line of exactly 64 KiB, a file of exactly 256 KiB are imported (D1-3) | `records::import_accepts_the_limits` |
| Repeated id | an id already in the target file, including a legacy line | `records::import_refuses_repeated_id` |
| Input path | missing, a directory, a FIFO, `-`, Latin-1 bytes, the ticket's own `findings.jsonl`, another ticket's `state.json`, the ledger, a symlink to another ticket's `state.json`, `.STAPEL/tickets/STP-1/FINDINGS.jsonl`; a symlink to a regular file is read | `records::import_refuses_bad_input_path` |
| Broken target | a non-JSON line in `findings.jsonl` | `records::import_refuses_broken_target` |
| Ledger | record bytes and output; missing ledger; missing final newline | `records::ledger_add_appends_one_record` |
| Ledger negative | kind `Finding`, `other`; note `"  "`; a broken ledger line | `records::ledger_add_refusals` |
| Closed ticket | closed by explicit key in upper and lower case; without key and one open ticket; without key and no open ticket; without key and two open tickets; a legacy ticket by key for both commands; each refusal starts with one `stapel: ` | `records::closed_ticket_by_key_only` |
| Guard | Write, Edit, MultiEdit, NotebookEdit on `.stapel/process-ledger.jsonl` and `.STAPEL/Process-Ledger.jsonl` | `records::guard_denies_ledger_writes` |
| Check | only the ledger changed: `stapel check` reads the tree clean; the ledger and a source file changed: dirty | `records::ledger_change_keeps_tree_clean` |
| Coverage (step 2c) | a passing check recorded at HEAD, then only the ledger changed: `stapel status` still shows the check as passing; a source file changed as well: stale (D2-1) | `records::ledger_change_keeps_check_fresh` |
| Dialog | reason, replaced command and one grant file for both commands, `stapel` and `cargo run … --` forms; a claim of 81 characters, in Cyrillic; a claim with a newline; a note of 201 characters; a closed ticket | `records::dialog_for_findings_import`, `records::dialog_for_ledger_add` |
| Dialog negative | a bad record; `bypassPermissions`; an allow rule `Bash(stapel findings:*)`; no grant file in each case | `records::guard_denies_what_the_command_refuses` |
| Grant | used once, then "does not match"; an id added to the target after the dialog refuses by AC-3 and the grant stays; `FILE` edited, another note, another kind, another key; expired; a `findings-import` grant on `ledger add`, and an `ok` grant on `findings import` | `records::grant_writes_once`, `records::grant_names_the_records` |
| Agent shell | both commands without grant; own `--grant` on both; `$(cat f)` as `FILE` | `records::agent_shell_without_grant_refused`, `records::guard_denies_own_grant` |
| Coverage (step 3c) | a ledger line broken after the `ledger add` dialog refuses and leaves the grant, which then writes once the line is gone (D3-3); a claim of exactly 80 and a note of exactly 200 characters are shown whole, without `…` (D3-7) | `records::ledger_grant_survives_a_failed_check`, `records::dialog_cuts_at_the_limits` |
| Step 4 | `id` of 65 characters, with a space, a tab, a newline: refused with the line number; `id` of 64 characters accepted (D3-1) | `records::import_refuses_unsafe_id` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `FILE` (positional of `findings import`) | path to a regular UTF-8 file, symlinks followed | 1 record; 100 records, 256 KiB, a line of 64 KiB | no record: refused | missing, directory, `-`, not UTF-8, machine file: refused | — | `records::import_refuses_bad_file`, `records::import_refuses_bad_input_path` |
| Finding record | JSON object, one per line | the required fields only; any extra fields | blank line: skipped | AC-2 and AC-3 refusals | exact strings, case-sensitive lists | `records::import_refuses_bad_fields` |
| `round` | non-negative integer or non-empty string | `0`; any string | `""`: refused | negative, float, bool, null: refused | exact | `records::import_refuses_bad_fields` |
| `KEY` (positional, both commands) | ticket folder name, any case | one ticket | absent: the only open ticket | no such folder, legacy, several open without key: today's refusal | ASCII case ignored | `records::closed_ticket_by_key_only` |
| `--kind` | one of `finding`, `catches`, `decision` | — | absent: usage error (exit 2) | other or other case: refused | exact | `records::ledger_add_refusals` |
| `--note` | text | one non-blank character; the 64 KiB line limit | absent: usage error; blank: refused | — | kept as given | `records::ledger_add_appends_one_record`, `records::ledger_add_refusals` |
| `--grant` | 32 hex characters, set by the guard | one grant per dialog | absent: terminal path or AC-12 | wrong, expired, reused, other action: AC-11 | exact | `records::grant_writes_once`, `records::grant_names_the_records` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | permission mode; whether the dialog is offered | dialog modes ask (auto included, F-1); others deny | `records::dialog_for_findings_import`, `records::guard_denies_what_the_command_refuses` |
| claude-code | environment of hook processes versus the agent's Bash shell | an agent shell without grant is refused | `records::agent_shell_without_grant_refused` |
| claude-code | allow rules that skip the dialog | a matching allow rule denies | `records::guard_denies_what_the_command_refuses` |
| claude-code | transcripts; other settings levels | not touched | — |
| os | case-insensitive file systems | the ledger is a machine file in any case | `records::guard_denies_ledger_writes` |
| os | symlinks | `FILE` that is a symlink to a regular file is read; a symlinked target file: not promised | — |
| os | file permissions | an unreadable `FILE` or an unwritable target: the OS error with the path, exit 2; not retested | — |
| os | time: clock skew, time zones | `at` is the UTC date of the run; skew not promised | `records::ledger_add_appends_one_record` |
| os | processes, WSL | not touched | — |
| git | every dimension | not touched: the commands read no history | — |
| cargo | every dimension | not touched | — |

### Abuse table

Attempts of a careless agent against "only a person approves records" and "a grant names its records".

| Attempt | Outcome |
|---|---|
| The agent runs a record command in its own shell without a grant | refused, AC-12 |
| The agent adds its own `--grant`, or builds `FILE` at run time | the guard denies, AC-15 |
| The agent edits `FILE` after the dialog | the records hash changes: "does not match", AC-14 |
| The agent imports into another ticket with the grant of one | `findings import`: the `ticket` field check refuses, AC-2; `ledger add`: "does not match", AC-14 |
| A grant of `ledger add` used for `findings import`, or an `ok` grant | the action differs: "does not match", AC-14 |
| The agent writes the ledger with Edit or Write | denied, AC-8 |
| The agent appends with `cat >>` or a script | not promised (Frame non-goals) |
| The agent runs the command inside `cd …;` or a pipe | the guard allows, the command refuses in an agent shell, AC-12 |
| The agent imports `findings.jsonl` of another ticket as `FILE` | refused as a machine file, AC-4 |

**Review Focus:** a legacy line in the target file with the same `id` (AC-3 test covers it); a closed ticket
without `KEY` (AC-7); a dialog shown before midnight UTC and run after it (not tested, it needs a clock; the
ledger's records hash takes no date by its Terms definition);
a claim with multi-byte characters cut at 80 characters, not bytes (dialog test, a claim in Cyrillic); a
`FILE` that holds the same record twice with different ids (allowed: ids differ).

**Author self-check:** 2026-10-15, before spec review: every promise item has a criterion and a test; the
Inputs and External states rows were checked against `guard.rs`, `dialog.rs`, `grant.rs` and `journal.rs`;
the Claude Code behaviour the dialog relies on was checked in the hooks docs (F-1); no open question is left.
2026-10-15, after spec review round 1: 0 HIGH, 7 MEDIUM, 9 LOW; every inside finding applied as one change
(SR1-1 to SR1-15); AC-9, AC-11 and AC-12 split into AC-9 and AC-13, AC-11 and AC-14, AC-12 and AC-15 (SR1-13);
SR1-16 (outside) accepted by the human as promise item 7 and AC-16; no round 2.
2026-10-16, after the drift review of step 1: D1-1 to D1-7 (1 MEDIUM, 6 LOW); one spec change: plan step 1c
(D1-3), the several-open case in step 2 (D1-1), a run outside a repository not promised (D1-2).
2026-10-16, after the drift review of step 2: D2-1 to D2-6 (2 MEDIUM, 4 LOW; D2-2 and D2-4 no defect); one
spec change: plan step 2c (D2-1), AC-3 counts the newline in the line limit (D2-3).
2026-10-16, after the drift review of step 3: D3-1 to D3-7 (1 MEDIUM, 6 LOW) and the builder's intent_gap on
AC-11; one spec change: AC-2 limits `id` and plan step 4 (D3-1), AC-11 and AC-14 say which check refuses
first for `findings import` (D3-2), plan step 3c (D3-3, D3-7), the Abuse table cites AC-14 and AC-15 (D3-4),
two items not promised (D3-5, D3-6).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `records::import_appends_all_in_order`, `records::import_creates_and_repairs_target` |
| AC-2 | `records::import_refuses_bad_fields`, `records::import_refuses_unsafe_id` |
| AC-3 | `records::import_refuses_bad_file`, `records::import_refuses_repeated_id`, `records::import_accepts_the_limits` |
| AC-4 | `records::import_refuses_bad_input_path`, `records::import_refuses_broken_target` |
| AC-5 | `records::ledger_add_appends_one_record` |
| AC-6 | `records::ledger_add_refusals` |
| AC-7 | `records::closed_ticket_by_key_only` |
| AC-8 | `records::guard_denies_ledger_writes` |
| AC-9 | `records::dialog_for_findings_import`, `records::dialog_cuts_at_the_limits` |
| AC-10 | `records::guard_denies_what_the_command_refuses` |
| AC-11 | `records::grant_writes_once`, `records::ledger_grant_survives_a_failed_check` |
| AC-12 | `records::agent_shell_without_grant_refused` |
| AC-13 | `records::dialog_for_ledger_add` |
| AC-14 | `records::grant_names_the_records` |
| AC-15 | `records::guard_denies_own_grant` |
| AC-16 | `records::ledger_change_keeps_tree_clean`, `records::ledger_change_keeps_check_fresh` |

## Plan

Route: full (tags `guard`, `data`, `interface`), pinned at the frame.

1. `findings import` in a terminal: `stapel_core::records`, the CLI command, the multi-record append (AC-1 to
   AC-4, AC-7 for findings). Must not change: `journal::append`, `ok`, `close`.
1c. Coverage after the drift review of step 1: the accepted limits (D1-3). Tests only.
2. `ledger add` in a terminal and the ledger as a machine file (AC-5, AC-6, AC-7 for the ledger, AC-8,
   AC-16). Must
   not change: the guard's other machine files.
2c. Coverage after the drift review of step 2: a check stays fresh after a ledger change (D2-1). Tests only:
   one new `#[test]` function, no helper.
3. The dialog and the grant for both commands, and the agent-shell refusal (AC-9 to AC-15). Must not change:
   the `ok` and `close` dialogs and their grant facts.
3c. Coverage after the drift review of step 3 (D3-3, D3-7). Tests only: two new `#[test]` functions, no
   helper.
4. `id` limited to 64 characters without whitespace or control characters (AC-2, D3-1). Must not change the
   other field checks.

## Review

TODO

## Summary

TODO
