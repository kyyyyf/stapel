# STP-23 — Findings import and triage

## Description

Source: `docs/PHASES.md`, plan item 1.5, part 1 (split by the ledger decision of 2026-10-22); the ledger
decision of 2026-10-21 (approval dialogs, part 1); `docs/decisions/0002-build-and-review-from-bmad.md`, item 7 (label STP-8).

### Frame

**Problem.** Every findings import and ledger record is a permission dialog (17 of 27 in STP-21); a
free-text `fate` hides untriaged findings from `stapel next`.

**Plan item:** 1.5 (part 1).

**Risk tags:** `guard` (two record commands lose the dialog), `data` (new fields and records, old ones
kept), `interface` (a new command and two `next` actions).

**Non-goals.** Part 2 of item 1.5 (`review start`, the pack); items 1.6 to 1.8; editing a record;
tracking a `patch`; other dialogs.

**Promise (closed list).**

1. `findings import` and `ledger add --kind finding|catches` need no dialog; each record gets
   `imported_by` (`agent` or `person`, and `user.name`).
2. A `fate` is one of `open`, `patch`, `bad_plan`, `intent_gap`, `defer`, `no-change`; `verdict`,
   `evidence`, `root`, `note` are optional.
3. `stapel findings triage FILE [KEY]` appends triage records; the last record of an id wins.
4. `stapel next` gives `triage` for an open HIGH or MEDIUM inside finding and `ask-human` for a finding
   that waits for the person, before `confirm`; `brief` shows the current fate.
5. Old records (free-text fates) stay readable and count as triaged.
6. The skill names the new commands, the fates and the two actions.

Not promised: an importer who lies about being a person; deliberate circumvention.

**Size.** About 350 source lines and 800 test lines; split past 600. Route: full (tags), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-22: Decision (split, the human, recommended): item 1.5 is two tickets; this one takes the import
  without a dialog and the triage, part 2 takes `review start` and the pack (ledger). Rejected: one ticket
  of about 700 source lines; the pack first.
- 2026-10-22: Decision (external reviewer, the human, recommended; for part 2): the agent `stapel-reviewer`
  with the model of the role `external`. Rejected: a fifth agent file.
- 2026-10-22: Decision (fate after import, the human, recommended): triage records with the same `id`,
  the last one wins, appended by `findings triage`; the file stays append-only and `next` sees a finding
  as soon as it is imported. Rejected: import only after triage, as in STP-22.
- 2026-10-22: Decision (fates, the human, recommended): `open`, `patch`, `bad_plan`, `intent_gap`,
  `defer` and `no-change` (a false finding, or one accepted without a change), with a free `note`.
  Rejected: the four fates of decision 0002 only.
- 2026-10-22: Decision (importer, the human, recommended): `imported_by` holds `agent` or `person` (from
  the agent-shell variables) and `git config user.name`. Rejected: `agent` or `person` only.
- 2026-10-22: Decision (frame, the human): the Frame above is confirmed in the chat.
- 2026-10-22: Decision (record form, the human, recommended): a triage record carries `"kind": "triage"`;
  a record without `kind` is a finding, as today; old files do not change. Rejected: a separate
  `triage.jsonl`.
- 2026-10-22: Decision (verdict, the human, recommended): the last verdict `high`, `medium` or `low`
  replaces the severity for `next`; `false` takes the finding out of the blocking set; `maybe-false` keeps
  the severity; the finding record itself never changes. Rejected: a verdict that is only stored.
- 2026-10-22: Decision (what blocks, the human, recommended): an open HIGH or MEDIUM inside finding gives
  `triage`; an open outside finding of any severity gives `ask-human`, since widening the promise is the
  person's choice; an open LOW inside finding does not block; any stage counts. Rejected: inside only;
  every open finding. Frame item 4 holds this rule.
- 2026-10-22: Decision (order, the human, recommended): `triage` and `ask-human` come after `write-frame`,
  `write-section` and `spec-review` and before `confirm` and every later action, so one `stapel ok`
  follows a triaged round. Rejected: after `confirm`, `fix-history` and `fix-check`.
- 2026-10-22: Decision (triage errors, the human, recommended): an unknown id or an id twice in one file
  refuses the whole file, as `findings import` does. Rejected: skip the bad lines.
- 2026-10-22: Decision (closed ticket, the human, recommended): `findings triage` works on a closed
  ticket, as `findings import` does, so an `after-close` finding gets a fate. Rejected: refuse.
- 2026-10-22: Decision (process text, the human, recommended): one Plan step changes the skill text: import
  without a dialog, the triage command, the fates, the actions `triage` and `ask-human`. Rejected: a later
  ticket (the skill would contradict the code).
- 2026-10-22: Decision (rounds, the human, recommended): an open blocking finding whose `round` is a whole
  number of 2 or more and whose stage is `spec-review` or `code-review` gives `ask-human`; a text round
  (`step-3` of a drift review) is not counted. Rejected: `code-review` only.
- 2026-10-23: Decision (string round, SR1-17, the human, recommended): a `round` string of ASCII digits
  counts as that number for a person's finding; import does not change. Rejected: not promised.
- 2026-10-23: Decision (the person's words, SR1-18, the human, recommended): a triage line for a person's
  finding needs a `note` with the person's answer, else exit 2. Rejected: `note` stays optional.
- 2026-10-23: Decision (after spec review round 1, recommended): the 16 inside findings are patched
  (`.stapel/scratch/stp23/findings-sr1.jsonl`); no HIGH finding, so no second round. `findings import`
  loses `--grant` (SR1-14); a closed ticket stays `closed` in `next` (SR1-9); a Bash redirection into
  `findings.jsonl` is not promised and stays with items 1.9 and 1.10 (SR1-3).
- 2026-10-23: Decision (drift review of step 4, recommended): coverage step 4b (D4-2 to D4-4); a round
  `2.0` is the whole round 2 by the term, so it gives `ask-human`, and its line shows `2.0` as written;
  `findings import` refuses `2.0` (STP-18), so it comes only from a hand-written line (D4-3, D4B-1).
- 2026-10-23: Decision (an old quote, the human, recommended): STP-18 AC-12 quotes the refusal text as
  changed by STP-23 AC-2, with a note; the builder's comment line that kept the old text in `fail.rs` goes
  in step 2. Rejected: keep the comment; a lint rule for superseded quotes now.
- 2026-10-23: Decision (drift review of step 1c, recommended): two coverage tests in step 1d (D1C-1,
  D1C-3); the order of the `user.name` refusal before the record checks is accepted and noted as built
  (D1C-2).
- 2026-10-23: Decision (drift reviews of steps 2 and 3, recommended): coverage steps 3b and 3c; AC-6 and
  AC-7 name the refusals step 3 built (the `id` rule, `imported_by`, a damaged target line), as
  `findings import` has them (D3-1 to D3-3); `FATES`, `VERDICTS` and `check_triage_fields` stay public
  (D2-3).
- 2026-10-23: Decision (code review round 1, the orchestrator, the human away and asking for decisions
  to be taken and reported): fix step 7 and coverage step 7b; the Design and AC-2 texts corrected; STP-18
  AC-9 to AC-11 and AC-17 get a note, as AC-12 did (E-2); R-1 and E-3 wait for the human
  (`.stapel/scratch/stp23/report-for-human.md`).
- 2026-10-23: Decision (drift review of step 5, recommended): coverage step 5b (D5-1, D5-2); the other
  brief lines stay guarded by `tests/brief.rs` (D5-3).

## Spec

Terms:

- **Finding record**: a record of `findings.jsonl` without a `kind` field.
- **Triage record**: a record of `findings.jsonl` with `"kind": "triage"`.
- **Fate list**: `open`, `patch`, `bad_plan`, `intent_gap`, `defer`, `no-change`.
- **Verdict list**: `high`, `medium`, `low`, `false`, `maybe-false`.
- **Agent shell**: a process with `CLAUDECODE` or `CLAUDE_CODE_ENTRYPOINT` set (`identity::in_agent_shell`).
- **Importer**: the JSON object `{"by":"agent","user":"<name>"}` in an agent shell, else
  `{"by":"person","user":"<name>"}`, where `<name>` is `git config user.name` read in the repository.
- **The finding** of an id: the last finding record with that string `id`; records whose `id` is not a
  string, and lines that are not records, are skipped.
- **Current state** of a finding: its fate is the `fate` of the last record with its `id` in file
  order; its severity is the uppercase form of the last `verdict` among `high`, `medium`, `low` of those
  records, else its own `severity`; it is **false** when the last `verdict` of those records is `false`;
  a `fate`, `severity` or `verdict` that is not a string reads as absent.
- **Legacy finding**: a finding whose current fate is absent or not in the fate list.
- **Whole round**: a `round` that is a whole number, or a string of ASCII digits only, read as that
  number.
- **Blocking finding**: a finding record, not legacy, not false, whose current fate is `intent_gap`, or
  whose current fate is `open` and whose `promise` is `outside` or whose current severity is `HIGH` or
  `MEDIUM`.
- **Person's finding**: a blocking finding whose current fate is `intent_gap`, whose `promise` is
  `outside`, or whose whole round is 2 or more with stage `spec-review` or `code-review`.
- **Importer insert**: the text `,"imported_by":<importer>` put before the last `}` of a line, after
  trailing whitespace is removed; the rest of the line stays byte for byte.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel findings import FILE [KEY]` runs in an agent shell THE command needs no grant and has no `--grant` option, appends each input line with the importer insert, and prints `imported: <n> findings into <KEY>`; the guard allows the plain command without a dialog in every permission mode; IF a line with the insert is longer than 64 KiB THEN exit 2 with `stapel: <file>:<n>: longer than 64 KiB` and nothing appended. | `triage::import_in_an_agent_shell`, `triage::guard_allows_import_without_a_dialog`, `triage::guard_allow_output_is_empty` |
| AC-2 | WHEN `stapel ledger add` has exactly one `--kind` with the value `finding` or `catches` (as `--kind X` or `--kind=X`) THE guard allows it without a dialog in every permission mode and the command appends the record with `imported_by` as its last field in an agent shell without a grant; `--kind decision` and two `--kind` words keep the dialog, an unknown value is refused as in STP-18, a `decision` record gets no `imported_by` and reads no `user.name`, and in an agent shell without a grant it exits 1 with `stapel: only a person writes ledger decisions: this shell belongs to an agent (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); run the command as one plain command so that Claude Code shows the permission dialog, or, from a Zed task started from a Claude Code shell, run it with env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT`. | `triage::ledger_records_without_a_dialog`, `triage::ledger_decision_keeps_its_dialog`, `triage::ledger_kind_words` |
| AC-3 | IF `git config user.name` is unset or blank THEN `findings import` and `ledger add --kind finding|catches` print `stapel: git config user.name is not set; set it to the name confirmations are recorded under`, exit 1 and append nothing; IF a record of FILE has `imported_by` THEN exit 2 with `stapel: <file>:<n>: imported_by: written by stapel, not by the file`. | `triage::importer_recorded`, `triage::import_needs_a_user_name`, `triage::ledger_needs_a_user_name`, `triage::importer_escapes_the_user_name`, `triage::ledger_importer_for_catches` |
| AC-4 | IF a finding record of `findings import` has a `fate` outside the fate list, a `verdict` outside the verdict list, a `verdict` `false` or `maybe-false` without `evidence`, an `evidence` or `note` that is not a non-empty string, a `root` that breaks the `id` rule, or any `kind` THEN exit 2 with `stapel: <file>:<n>: <field>: <reason>` from the cases table and nothing appended. | `triage::import_checks_the_fate`, `triage::import_checks_the_triage_fields`, `triage::import_root_and_type_bounds` |
| AC-5 | WHEN `stapel findings triage FILE [KEY]` runs, each line of FILE an object with `id`, `ticket` and `fate`, and optionally `verdict`, `evidence`, `root`, `note`, THE command appends each as `{"kind":"triage",` followed by the line after its first `{`, with the importer insert, needs no grant or dialog, and prints `triaged: <n> findings of <KEY>`; the limits of `findings import` apply (100 records, 256 KiB, 64 KiB a line). | `triage::triage_appends_records`, `triage::triage_line_rules`, `triage::guard_allows_triage` |
| AC-6 | IF a triage line lacks `id`, `ticket` or `fate`, names an id that has no finding in the ticket's file, repeats an id of an earlier line, has an `id` that breaks the `id` rule of STP-18, has `imported_by` (the AC-3 text), has another field outside the seven (`<field>: not a triage field`), breaks a rule of AC-4, or has no `note` while its finding is a person's finding before this file THEN exit 2 with `stapel: <file>:<n>: <reason>` from the cases table and nothing appended; an unset `user.name` gives the text of AC-3, exit 1. | `triage::triage_refusals`, `triage::triage_note_reads_the_current_state`, `triage::triage_line_rules` |
| AC-7 | WHEN `findings triage` names a closed ticket THE records are appended; IF the ticket has a legacy (STP-1) state, or its `findings.jsonl` has a damaged line, THEN the refusal of `findings import`. | `triage::triage_on_a_closed_ticket`, `triage::triage_line_rules` |
| AC-8 | WHEN the ticket's `findings.jsonl` holds a blocking finding and none is a person's finding, and `next` would give `confirm` or a later action, THE action is `triage` with the target `.stapel/tickets/<KEY>/findings.jsonl`, after the line `blocked: finding <id> (<severity>, <promise>, <stage> round <round>): <fate>` for each blocking finding in file order, at most ten, then `blocked: and <n> more`; a closed ticket stays `closed`. | `triage::next_asks_for_triage`, `triage::next_triage_before_confirm`, `triage::next_triage_among_the_actions`, `triage::next_cap_of_ten`, `triage::next_spec_review_before_triage` |
| AC-9 | WHEN at least one blocking finding is a person's finding THE action is `ask-human` with the same target and lines. | `triage::next_asks_the_human`, `triage::next_round_forms` |
| AC-10 | WHEN `next` reads the current state THE last fate and the last verdict hold; a false, legacy or LOW inside open finding gives no line. | `triage::current_state_of_a_finding`, `triage::legacy_records_do_not_block`, `triage::only_triage_kind_counts` |
| AC-11 | WHEN `stapel brief` shows a finding a step names THE line is `- <id> (<current severity>): <claim of the finding> Fate: <current fate>`, followed by ` Verdict: <verdict>` when the last verdict is `false` or `maybe-false`. | `triage::brief_shows_the_current_fate`, `triage::brief_reads_old_and_later_records` |
| AC-12 | WHEN the skill is written THE body holds `stapel findings import and stapel ledger add --kind finding or catches run without a dialog; stapel ledger add --kind decision keeps its dialog.`, a line that starts ``- `triage`:`` and holds `stapel findings triage FILE KEY` and `patch, bad_plan, intent_gap, defer or no-change`, a line that starts ``- `ask-human`:`` and holds `ask the person in the chat` and `with the person's answer as the note`, and no longer holds `each import is a permission dialog`. | `init_process::skill_text_names_the_triage` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-12; records already in `findings.jsonl` and the ledger
  are never rewritten; `findings import` keeps every check of STP-18 except the dialog; `ledger add --kind
  decision` keeps its dialog and grant; `tokens import-records`, `ok` and `close` are unchanged.
- **Not promised:** the Frame's list; whether a `patch` was built; an agent that marks findings
  `no-change` (recorded with `imported_by` agent, not prevented); a person's terminal inside a Zed task
  started from a Claude Code shell (counted as `agent`); the order of fields in records already written;
  a Bash redirection into `findings.jsonl` (items 1.9 and 1.10); open findings of a closed ticket in
  `next`; old records whose fate is already in the fate list (they block as new ones; only closed tickets
  hold them, R-4); a step that names an id with only a triage record (a hand-written file).

## Test plan

CLI tests in the new file `crates/stapel-cli/tests/triage.rs` with `common::git_repo` and
`common::stapel`; the agent shell is `CLAUDECODE=1` in the command's environment, the guard is
`stapel hook pre-tool-use` with a Bash tool input, as in `tests/records.rs`. AC-12 adds one function to
`tests/init_process.rs`. Light tests: `cargo test -j 4 -p stapel-cli --test triage -- --test-threads=3`
(and `--test records`, `--test token_records`, `--test brief`, `--test init_process`); before each GREEN
also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Import in an agent shell | `CLAUDECODE=1`, no grant: exit 0, `imported: 2 findings into ABC-1`, each appended line equals its input line with the importer insert (odd spacing and key order kept); a line of 64 KiB less 10 bytes: exit 2, `longer than 64 KiB`, nothing appended; `--grant x`: a usage error, exit 2 | `triage::import_in_an_agent_shell` |
| Guard, import | the hook on `stapel findings import f.jsonl ABC-1` with permission mode `default`, `bypassPermissions` and no mode: allowed, no `ask`, no grant file | `triage::guard_allows_import_without_a_dialog` |
| Ledger without a dialog | `--kind finding`, `--kind=catches`, mode `bypassPermissions`: the hook allows, the command in an agent shell appends with `imported_by` | `triage::ledger_records_without_a_dialog` |
| Ledger decision | `--kind decision`, `--kind=decision`, `--kind finding --kind decision`: the hook asks; `--kind Decision`: the hook denies with `--kind must be one of finding, catches, decision`; `--kind decision` in an agent shell without a grant: the AC-2 text, exit 1; through the grant: no `imported_by`, appended with `user.name` unset | `triage::ledger_decision_keeps_its_dialog` |
| Importer | without `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT`: `"by":"person"`; `CLAUDE_CODE_ENTRYPOINT=` (empty): `agent`; a record with `imported_by`: exit 2 | `triage::importer_recorded` |
| Guard output | the hook on the import command in modes `default`, `bypassPermissions` and none: stdout empty, stderr empty, exit 0 (D1-3) | `triage::guard_allow_output_is_empty` |
| Escaped name | `user.name` `a "q" é\\`: the appended `imported_by` parses as JSON with that exact user (D1-4) | `triage::importer_escapes_the_user_name` |
| Kind words | `--kind Decision`: the hook denies with the reason `--kind must be one of finding, catches, decision`; `--kind finding --kind decision` in modes `default` and `auto`: the hook asks (D1C-1) | `triage::ledger_kind_words` |
| Catches importer | `ledger add --kind catches` in an agent shell with `user.name` `a "q" \\`: the record's `imported_by` parses as JSON with `by` `agent` and that exact user; `user.name` unset: exit 1, the AC-3 text (D1C-3) | `triage::ledger_importer_for_catches` |
| No user name, import | `user.name` unset, then blank, for `findings import`: exit 1, the text, files unchanged | `triage::import_needs_a_user_name` |
| No user name, ledger | `user.name` unset, then blank, for `ledger add --kind finding`: exit 1, the text, the ledger unchanged | `triage::ledger_needs_a_user_name` |
| Fate | `fixed`, `Open`, `""`, a number: `fate: must be one of open, patch, bad_plan, intent_gap, defer, no-change` (a missing fate keeps `fate: missing`) | `triage::import_checks_the_fate` |
| Triage fields at import | `verdict` `wrong`: `verdict: must be one of high, medium, low, false, maybe-false`; `verdict` `false` without evidence: `evidence: missing; the verdict false needs it`; `evidence` `""` or `note` `3`: `<field>: must be a non-empty string`; `root` `a b`: `root: must be 1 to 64 characters with no whitespace or control character`; `kind` `triage`: `kind: a finding has no kind; triage records go through stapel findings triage` | `triage::import_checks_the_triage_fields` |
| Triage | two lines for F-1 and F-2 with `verdict`, `root`, `fate`: `triaged: 2 findings of ABC-1`, the appended lines start with `{"kind":"triage",` | `triage::triage_appends_records` |
| Triage refusals | no `id`: `id: missing`; no `ticket`: `ticket: missing`; an open outside F-3 without `note`: `note: missing; a finding for the person needs the person's answer`; id `F-9` not in the file: `id F-9 is not a finding in .stapel/tickets/ABC-1/findings.jsonl`; F-1 twice: `id F-1 repeats line 1`; no fate: `fate: missing`; `severity`: `severity: not a triage field`; the id of a triage record only: not a finding; a `ticket` of another key: `ticket: must be ABC-1`; no `findings.jsonl`: every id is not a finding; `user.name` unset: exit 1, the text of AC-3 | `triage::triage_refusals` |
| Root and types | `root` of 64 characters: accepted; 65 characters, `a\tb`, `a\nb`: the `root` refusal; `verdict` `5` or `null`: the `verdict` refusal; `evidence` `5` with `verdict` `false`: `evidence: must be a non-empty string`; `kind` `null` or `""`: the `kind` refusal (D2-1, D2-2) | `triage::import_root_and_type_bounds` |
| Note by the current state | F-3 open outside, then a triage record `defer` with a note: a later line without `note` is accepted; F-4 HIGH inside moved to `intent_gap` by an earlier record: a line without `note` is refused; F-5 outside with an earlier `verdict` `false`: no `note` needed (D3-4) | `triage::triage_note_reads_the_current_state` |
| Triage line rules | `imported_by` in a line: `imported_by: written by stapel, not by the file`; an `id` of 65 characters or with a space: the STP-18 `id` refusal; a line of 64 KiB: `longer than 64 KiB`; a file over 256 KiB: the STP-18 size refusal; a damaged line in `findings.jsonl`: the refusal of `findings import` (D3-1, D3-2, D3-3, D3-5) | `triage::triage_line_rules` |
| Guard, triage | the hook on `stapel findings triage f.jsonl ABC-1` in modes `default`, `bypassPermissions` and none: exit 0, stdout and stderr empty (D3-6) | `triage::guard_allows_triage` |
| Closed | a closed ticket: appended; a legacy `state.json`: the refusal of `findings import` | `triage::triage_on_a_closed_ticket` |
| Next, triage | a ticket past spec review with a confirmed spec and an open HIGH inside F-1 of round 1: `blocked: finding F-1 (HIGH, inside, code-review round 1): open`, `next: triage — .stapel/tickets/ABC-1/findings.jsonl`; twelve open MEDIUM: ten lines and `blocked: and 2 more`; the same file on a closed ticket: `closed` | `triage::next_asks_for_triage` |
| Next, order | a ticket with spec-review findings and no confirmation: `triage`, not `confirm`; after a triage record with fate `patch`: `confirm` | `triage::next_triage_before_confirm` |
| Next among the actions | a blocking finding on a ticket without a Frame: `write-frame`; with an unwritten Plan: `write-section`; on a confirmed ticket with step 1 built as RED only: `triage`, not `green-step` (D4-2) | `triage::next_triage_among_the_actions` |
| Next, ten lines | ten blocking findings: ten lines and no `and` line; eleven: ten lines and `blocked: and 1 more`; eleven of which one is outside: `ask-human` with the same ten lines and `blocked: and 1 more` (D4-4) | `triage::next_cap_of_ten` |
| Next, round forms | an open HIGH of `code-review` round `"2a"`: `triage`; round `2.0`: `ask-human` (a whole round), the line shows `round 2.0`; an open HIGH of `spec-review` round 2: `ask-human` (D4-3) | `triage::next_round_forms` |
| Next, person | an open outside LOW; a fate `intent_gap`; an open HIGH of `code-review` round 2, and of round `"2"`: `ask-human`; an open HIGH of `step-drift` round `step-2`: `triage` | `triage::next_asks_the_human` |
| Current state | F-1 HIGH, then triage `verdict` `low`: no line; then `verdict` `medium`: `(MEDIUM, …)`; then `verdict` `false`: no line; then `fate` `open` with no verdict: still false, no line; written by hand: two finding records F-4 (LOW, then HIGH open): one line for the last; F-5 with `fate` `["x"]` and F-6 with `severity` `3` open inside: no line; a line that is not JSON: skipped | `triage::current_state_of_a_finding` |
| Legacy | an open HIGH with fate `fixed: done` (written as an old line), and a new open HIGH F-2: only F-2 named | `triage::legacy_records_do_not_block` |
| Brief | a step naming F-1, which has a triage record with `verdict` `low` and fate `defer`: `- F-1 (LOW): <claim> Fate: defer`; F-2 with a last verdict `false` and fate `no-change`: the line ends `Fate: no-change Verdict: false` | `triage::brief_shows_the_current_fate` |
| Brief, old and later records | F-1 with fate `fixed: done` and no triage record: `- F-1 (HIGH): <claim> Fate: fixed: done`; F-2 with `verdict` `false`, then a fate-only record `defer`: `Fate: defer Verdict: false`; two finding records F-3, LOW then HIGH with another claim: the HIGH line with the second claim (D5-1, D5-2) | `triage::brief_reads_old_and_later_records` |
| Only the triage kind | F-1 open HIGH, then a hand-written record `{"kind":"note","id":"F-1","fate":"defer"}`: F-1 still blocks; then a triage record `defer`: no line (code review, external LOW) | `triage::only_triage_kind_counts` |
| Spec review first | Frame, Spec, Design, Proof and Plan written, no confirmation, no `spec-review` finding, one open HIGH inside `code-review` finding: `next: spec-review` (E-1) | `triage::next_spec_review_before_triage` |
| Skill text | the sentences of AC-12 in `.claude/skills/stapel/SKILL.md` written by `init` | `init_process::skill_text_names_the_triage` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `FILE` of `findings triage` | path to a JSONL file | 1 record / 100 records, 256 KiB, 64 KiB a line | no records: `<file>: no records` | as `findings import` (STP-18) | exact | `triage::triage_refusals` |
| `KEY` of `findings triage` | ticket key | — | absent: the one open ticket, as `findings import` | unknown: the STP-18 refusal | exact | `triage::triage_on_a_closed_ticket` |
| `fate` | string | — | `""`: refused; missing: `fate: missing` | outside the fate list (case counts) | exact | `triage::import_checks_the_fate` |
| `verdict` | string, optional | — | absent: no verdict | outside the verdict list | exact | `triage::import_checks_the_triage_fields` |
| `evidence`, `note` | string, optional | 1 character / the line limit | `""`: refused | not a string | as given | `triage::import_checks_the_triage_fields` |
| `root` | string, optional | 1 / 64 characters | `""`: refused | whitespace or a control character | exact | `triage::import_checks_the_triage_fields` |
| `imported_by` in FILE | any | — | — | present: exit 2 | — | `triage::importer_recorded` |
| `kind` in FILE | any | — | — | present in an import, or in a triage line: exit 2 | — | `triage::import_checks_the_triage_fields`, `triage::triage_refusals` |
| `--kind` of `ledger add` | string | — | missing: the STP-18 refusal | not `finding`, `catches`, `decision`: STP-18 | exact; only one plain `finding` or `catches` skips the dialog | `triage::ledger_records_without_a_dialog`, `triage::ledger_decision_keeps_its_dialog`, `triage::ledger_kind_words` |
| `--grant` of `ledger add` | string | — | absent: needed only for `--kind decision` in an agent shell | from an agent: denied by the guard, as STP-18; `findings import` has no such option: a usage error | exact | `triage::ledger_decision_keeps_its_dialog`, `triage::import_in_an_agent_shell` |
| `round` | number or string | as STP-18 | — | as STP-18 | a whole round counts for a person's finding; another string never | `triage::next_asks_the_human`, `triage::next_round_forms` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | configuration: `user.name` | read for the importer; unset or blank: exit 1 | `triage::import_needs_a_user_name`, `triage::ledger_needs_a_user_name` |
| git | history, shallow, object format, worktrees, submodules, paths, messages, dirty tree | not read by the new code; `next` as STP-20 | — |
| claude-code | environment of hook processes versus the agent's Bash shell | `by` is `agent` when an agent-shell variable is set, empty included | `triage::importer_recorded` |
| claude-code | permission mode; whether the dialog is offered | the two record commands need no dialog in any mode; `ledger add --kind decision` as STP-18 | `triage::guard_allows_import_without_a_dialog`, `triage::ledger_decision_keeps_its_dialog` |
| claude-code | settings, allow rules | an allow rule changes nothing: the guard allows the two commands anyway | `triage::guard_allows_import_without_a_dialog` |
| claude-code | transcripts; tool call succeeded or failed | not read | — |
| cargo | every dimension | not run | — |
| os | file permissions | a write error refuses as STP-18 | — |
| os | case-insensitive file systems; symlinks; processes; time; WSL | not promised | — |

### Abuse table

Attempts of a careless agent against "every record says who imported it, and the person decides the
findings that are theirs".

| Attempt | Outcome |
|---|---|
| The agent writes `"imported_by":{"by":"person",…}` into its file | refused, exit 2, AC-3 |
| The agent runs `env -u CLAUDECODE stapel findings import …` to read as a person | the guard denies it (STP-14, clearing the Claude Code variables) |
| The agent appends to `findings.jsonl` with Edit or Write | the guard denies it (machine file, STP-2) |
| The agent appends to `findings.jsonl` with a Bash redirection | not denied today; items 1.9 and 1.10 (not promised) |
| The agent relays the person's answer and leaves out what the person said | refused without `note`, AC-6 |
| The agent writes `--kind=decision` or `--kind finding --kind decision` to skip the dialog | the guard asks, AC-2 |
| The agent marks an outside finding `no-change` to avoid asking the person | recorded with `by: agent`; not prevented (not promised) |
| The agent triages a finding id of another ticket | refused, `ticket: must be <KEY>` or not a finding, AC-6 |
| The agent imports a finding with `fate: "patch"` and no evidence to skip triage | accepted; the fate counts, `imported_by` names the agent (not promised to stop) |

**Review Focus:** the guard's reading of `--kind`
after `--` or in a quoted word (`triage::ledger_decision_keeps_its_dialog` covers `--kind=` only); a
triage record written while another `next` reads the file (append-only lines; not tested); ids that differ
only in case (exact match, not tested).

**Author self-check:** 2026-10-22, before spec review: every promise item has a criterion and a test
(item 1: AC-1 to AC-3; item 2: AC-4; item 3: AC-5 to AC-7; item 4: AC-8, AC-9, AC-11; item 5: AC-10;
item 6: AC-12); each input has its empty and invalid case; the rows were checked against `records.rs`,
`dialog.rs`, `guard.rs`, `next.rs`, `brief.rs` and the catalogue; no open question is left.
2026-10-23, after spec review round 1 (external): 18 findings, 0 HIGH, 7 MEDIUM, 11 LOW; 16 inside, 2
outside, both taken by the human. One change: the importer is inserted into the input line and the line
limit counts it (SR1-1, SR1-2, SR1-15); the dialog-free Allow comes before the mode check (SR1-6); ledger
decisions get no importer and a new refusal text (SR1-5, SR1-12); hand-written records have a defined
reading (SR1-7); a string round of digits counts and a person's finding needs a note (SR1-17, SR1-18);
the Abuse row on redirections is corrected (SR1-3); AC-12 pins the fates and the `ask-human` text
(SR1-4); smaller fixes SR1-8 to SR1-11, SR1-13, SR1-14, SR1-16.

## Design

### Facts

- F-0: prior art: `records.rs` (`check_findings`, `plan_ledger`), `guard.rs` and `dialog.rs` (the
  record dialogs of STP-18), `next.rs` (`finding_stages`); the triage files of STP-22
  (`.stapel/scratch/stp22/findings-*.jsonl`).
- F-1: `records::check_findings` appends the input lines as they are; `dialog::prepare_ask` builds the
  dialog of `findings import`, `ledger add` and `tokens import-records` through `prepare_records`;
  `guard::decide` calls `dialog::ask` for every `plain_invocation` (`crates/stapel-core/src`).
- F-2: `findings import` and `ledger add` refuse in an agent shell without a grant (`fail::AGENT_SHELL`,
  `crates/stapel-cli/src/findings.rs`, `ledger.rs`).
- F-3: `next` reads only the `stage` of each finding (`crates/stapel-cli/src/next.rs`, `finding_stages`);
  `brief` takes the last record with an id (`crates/stapel-core/src/brief.rs`), which would be a triage
  record.
- F-4: `stapel check` counts as step tests only the `#[test]` functions whose text a RED changes
  (`steps::step_tests`); a helper such as `records.rs::base` may change in a RED.
- F-5: `tests/records.rs` and `tests/token_records.rs` drive the findings-import dialog and the ledger
  dialog with `finding` and `catches` (`dialog_for_findings_import`, `grant_writes_once`,
  `grant_names_the_records` and others); `base()` has the fate `fixed`.

### Decisions (before the build)

- `dialog::ask` returns Allow for `findings import`, and for `ledger add` when its words hold exactly one
  `--kind` whose value is `finding` or `catches`, before its checks of the permission mode and of
  `CLAUDE_CODE_EXECPATH`; `findings import` loses its `--grant` option and `findings_facts`.
- The importer insert works on the text of the line, so the appended line keeps the input's bytes and no
  JSON is serialised again; the `preserve_order` feature of `serde_json` does not matter (SR1-15).
- `fail::AGENT_SHELL` gets the text of AC-2 and is printed only for `ledger add --kind decision`.
- `records.rs` gains the importer (from `identity`), the fate and verdict lists, `check_triage` and
  `current(records)`, the current state used by `findings triage` (the note rule, step 3), `next` and
  `brief`.
- The CLI `next` passes the parsed finding records to `stapel_core::next::Facts` instead of the stages.
- Old tests: in the RED of steps 1, 1b and 1c, each test of `tests/records.rs` and `tests/token_records.rs`
  whose assertions the step changes (F-5, and the raw-line tests `import_appends_all_in_order`,
  `import_accepts_the_limits`, `import_creates_and_repairs_target`, `closed_ticket_by_key_only`) is either
  changed to assert the new behaviour, so that it fails at the RED, or deleted when a `triage::` test
  covers it; when the GREEN breaks one more, the builder amends the RED before the GREEN is committed; in
  the RED of step 2 `base()` gets the fate `patch` and each test with an inline free-text fate gets a case
  of the AC-4 refusal.

### Decisions (as built)

- `findings import` and `ledger add --kind finding|catches` read `user.name` before the record checks, so
  an unset name is reported first (D1C-2).
- `crates/stapel-core/src/triage.rs` holds `plan_triage` and the current state (`state_of`, `whole_round`,
  `is_blocking`, `is_persons`); `findings triage` refuses a `kind` with `kind: not a triage field` and over
  100 records with `more than 100 records; split the file`, as `findings import` does.
- `triage::blocking_findings` gives the blocking findings with their lines; `next::decide` checks them after
  the `spec-review` branch and before `confirm`; the CLI `next` reads `findings.jsonl` with `journal::read` and passes the
  list in `Facts::blocking`.
- `brief` takes the severity, claim, fate and verdict of a finding from `triage::state_of`.
- `dialog::ask` allows `findings import` and `ledger add` with one `--kind finding|catches` before the mode
  check; `findings triage` never reaches it, because `guard::plain_invocation` does not read it as a dialog
  command (DR-1, R-2); `findings_facts`, the findings dialog and its 30-record cap (STP-18 AC-17) are
  removed, and STP-18 AC-9 to AC-11 and AC-17 carry a note (E-2).
- The importer insert is `records::with_importer`; the importer JSON is `records::importer_json`.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `triage::import_in_an_agent_shell`, `triage::guard_allows_import_without_a_dialog`, `triage::guard_allow_output_is_empty` |
| AC-2 | `triage::ledger_records_without_a_dialog`, `triage::ledger_decision_keeps_its_dialog`, `triage::ledger_kind_words` |
| AC-3 | `triage::importer_recorded`, `triage::import_needs_a_user_name`, `triage::ledger_needs_a_user_name`, `triage::importer_escapes_the_user_name`, `triage::ledger_importer_for_catches` |
| AC-4 | `triage::import_checks_the_fate`, `triage::import_checks_the_triage_fields`, `triage::import_root_and_type_bounds` |
| AC-5 | `triage::triage_appends_records`, `triage::triage_line_rules`, `triage::guard_allows_triage` |
| AC-6 | `triage::triage_refusals`, `triage::triage_note_reads_the_current_state`, `triage::triage_line_rules` |
| AC-7 | `triage::triage_on_a_closed_ticket`, `triage::triage_line_rules` |
| AC-8 | `triage::next_asks_for_triage`, `triage::next_triage_before_confirm`, `triage::next_triage_among_the_actions`, `triage::next_cap_of_ten`, `triage::next_spec_review_before_triage` |
| AC-9 | `triage::next_asks_the_human`, `triage::next_round_forms` |
| AC-10 | `triage::current_state_of_a_finding`, `triage::legacy_records_do_not_block`, `triage::only_triage_kind_counts` |
| AC-11 | `triage::brief_shows_the_current_fate`, `triage::brief_reads_old_and_later_records` |
| AC-12 | `init_process::skill_text_names_the_triage` |

## Plan

Route: full (tags `guard`, `data`, `interface`), auto.

1. `findings import` without a dialog (AC-1; tests `triage::import_in_an_agent_shell`,
   `triage::guard_allows_import_without_a_dialog`); the old findings-dialog tests as in Design. Must not
   change: the checks of STP-18 and the dialog of `ledger add`, `tokens import-records`, `ok`, `close`.
1b. The importer of `findings import` (AC-3; test `triage::importer_recorded`); the raw-line tests as in
   Design. Must not change: step 1.
1c. `ledger add --kind finding|catches` without a dialog, with the importer, and the new refusal text of
   `--kind decision` (AC-2, AC-3; tests `triage::ledger_records_without_a_dialog`,
   `triage::ledger_decision_keeps_its_dialog`, `triage::ledger_needs_a_user_name`); the old ledger-dialog
   tests as in Design. Must not change: the dialog and grant of `ledger add --kind decision`, steps 1
   and 1b.
1d. Coverage after step 1 and the drift review of step 1 (D1-3, D1-4; AC-1, AC-3): new `#[test]`
   functions `triage::import_needs_a_user_name` (it passed at the RED of step 1b),
   `triage::guard_allow_output_is_empty`, `triage::importer_escapes_the_user_name`. Tests only, each
   helper nested inside its function; a test that fails is an `intent_gap`.
1e. Coverage after the drift review of step 1c (D1C-1, D1C-3; AC-2, AC-3): new `#[test]` functions
   `triage::ledger_kind_words`, `triage::ledger_importer_for_catches`. Tests only, each helper nested
   inside its function; a test that fails is an `intent_gap`.
2. The fate list and the triage fields at import (AC-4; tests `triage::import_checks_the_fate`,
   `triage::import_checks_the_triage_fields`); `base()` as in Design; the GREEN also removes the comment line of
   `crates/stapel-cli/src/fail.rs` that quotes the old STP-18 text (a lint workaround of step 1c). Must not change: the other refusals
   and their order.
3. `stapel findings triage FILE [KEY]` (AC-5 to AC-7; tests `triage::triage_appends_records`,
   `triage::triage_refusals`, `triage::triage_on_a_closed_ticket`). Must not change: `findings import`.
3b. Coverage after the drift review of step 2 (D2-1, D2-2; AC-4): new `#[test]` function
   `triage::import_root_and_type_bounds`. Tests only, each helper nested inside its function; a test that
   fails is an `intent_gap`.
3c. Coverage after the drift review of step 3 (D3-1 to D3-6; AC-5 to AC-7): new `#[test]` functions
   `triage::triage_note_reads_the_current_state`, `triage::triage_line_rules`, `triage::guard_allows_triage`.
   Tests only, each helper nested inside its function; a test that fails is an `intent_gap`.
4. The current state and the actions `triage` and `ask-human` of `next` (AC-8 to AC-10; tests
   `triage::next_asks_for_triage`, `triage::next_triage_before_confirm`, `triage::next_asks_the_human`,
   `triage::current_state_of_a_finding`, `triage::legacy_records_do_not_block`). Must not change: the
   other actions of `next` and their order.
4b. Coverage after the drift review of step 4 (D4-2 to D4-4; AC-8, AC-9): new `#[test]` functions
   `triage::next_triage_among_the_actions`, `triage::next_cap_of_ten`, `triage::next_round_forms`. Tests
   only, each helper nested inside its function, no `cargo fmt`; a test that fails is an `intent_gap`.
5. `brief` shows the current state (AC-11; test `triage::brief_shows_the_current_fate`). Must not change:
   the other lines of the brief.
5b. Coverage after the drift review of step 5 (D5-1, D5-2; AC-11): new `#[test]` function
   `triage::brief_reads_old_and_later_records`. Tests only, each helper nested inside its function, no
   `cargo fmt`; a test that fails is an `intent_gap`.
6. The skill text (AC-12; test `init_process::skill_text_names_the_triage`): `crates/stapel-cli/assets/skill.md`;
   the GREEN runs the built `stapel init` here so that `init_process::this_repository_has_the_current_files`
   holds. Must not change: the other sentences that `init_process` tests.

7. Fixes after code review round 1 (code review, external LOW items; AC-10; test
   `triage::only_triage_kind_counts`): `triage::state_of` reads only records with `"kind": "triage"` as
   triage records; the unused `records::Row`, `Findings.rows` and `Findings.hash` and the comments of
   `dialog.rs` that still name `findings import` as a dialog command go. Must not change: any other test.
7b. Coverage after code review round 1 (E-1; AC-8): new `#[test]` function
   `triage::next_spec_review_before_triage`. Tests only, each helper nested inside its function, no
   `cargo fmt`; a test that fails is an `intent_gap`.

## Review

TODO

## Summary

TODO
