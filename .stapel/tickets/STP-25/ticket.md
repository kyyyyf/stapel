# STP-25 — One dialog per ticket

## Description

Source: `docs/PHASES.md`, plan item 1.8; `docs/decisions/0004-product-direction-and-interface.md`, §4 item 3
and §5 item 4; the plan order of the ledger decision of 2026-10-21.

### Frame

**Problem.** The dialog of `stapel ok KEY` shows one line per section, so the person approves text they
do not see; the Frame and the Decisions are not confirmed at all. `stapel close` has a dialog (STP-2), but
the skill sends the person to their own terminal.

**Plan item:** 1.8.

**Risk tags:** `guard` (the text and the grant of the guard's dialog), `interface` (a new section, a
changed `next` action).

**Non-goals.** The one-section form of `stapel ok`; the person's language and autonomy levels (phase 3);
`stapel ship` (1.7); closed tickets; `stapel status` output.

**Promise (closed list).**

1. `description` (title `Description`, owner `product`) is a confirmable section in this repository's
   `stapel.toml` and in the one `stapel init` writes, and is in `build.requires`; no section depends on
   it. With a `stapel.toml` without it, the page holds only the criteria.
2. The guard's dialog for `stapel ok KEY` shows a page of at most 22 lines: the Description outside
   `### Decisions` (the Frame), the criteria rows with a count of other changed Spec lines, and the
   Decisions; all lines when never confirmed with stored text, else the changed ones with `+` and `-`;
   one line counts the lines over the limit.
3. Every other waiting section appears as one line, `<id>: N lines changed (+a -b)` or
   `<id>: new, N lines` (`1 line` when N is 1); section lines past the tenth fold into one line.
4. The terminal form of `stapel ok KEY` prints the whole page, without the limit, before its `confirmed:`
   lines.
5. The grant binds the hash of `description`; an edit of it after the dialog voids the grant.
6. `stapel next` gives `close` with the command `stapel close KEY --reason "<text>"`; the skill text says
   the orchestrator runs it with a real reason and the person answers the dialog.

Not promised: the whole page in the dialog when it is longer than the limit; a Spec without criteria rows
is shown as its logical lines, without the count of other lines.

**Size.** About 350 source lines and 800 test lines; split past 600. Route: full (tags), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-29: Decision (page place, the human, recommended): the human page is in the text of the
  permission dialog, so the "yes" covers what the person saw. Rejected: the page in the chat and a summary
  in the dialog.
- 2026-10-29: Decision (Frame binding, the human, recommended): `description` becomes a confirmable section
  in `build.requires`. Rejected: the Frame shown but not bound by the grant.
- 2026-10-29: Decision (no dependency, the human, recommended): no section depends on `description`, so a
  new Decisions line makes only `description` stale and one `stapel ok KEY` renews it; code writes wait
  for it as for any required section. Rejected: `spec` depends on it (each Decisions line would also
  re-confirm Spec); `description` outside `build.requires`.
- 2026-10-29: Decision (old configuration, the human, recommended): with a `stapel.toml` without the
  `description` section the dialog shows only the criteria, which the grant binds; `stapel init` does not
  change an existing `stapel.toml`. Rejected: a refusal.
- 2026-10-29: Decision (diff form, the human, recommended): the full text on the first confirmation, the
  changed lines after it; Design, Proof and Plan as one line each, in full with `--full`. Replaces the
  STP-17 decision of 2026-10-14 (question 4) for the key form. Rejected: the full text every time.
  Narrowed by the decision "short page": no `--full`.
- 2026-10-29: Decision (close, the human, recommended): the orchestrator runs `stapel close` and the person
  answers its dialog; `next` names the command. Rejected: close stays in the person's terminal.
- 2026-10-29: Decision (short page, the human): Claude Code shows about 25 lines of a dialog text and
  cuts the rest (probe with a real dialog of 80 lines: lines 1 to 25 shown); the dialog holds a short page
  of at most 22 lines and counts the hidden lines. Replaces the decision "page place" for long pages and
  the `--full` option of the plan item. Rejected: the page in the chat with a code in the dialog; the page
  in a file with its path in the dialog.
- 2026-10-29: Decision (page layout, the human, recommended): each dialog line is cut at 100 characters
  with `…`; the header line and one line per waiting section are always shown, the rest of the 22 lines go
  to the Frame, then the criteria, then the Decisions; the last line reads `… N more lines (frame a,
  criteria b, decisions c); all of them: stapel status KEY`. The terminal form prints the whole page, then
  the change blocks of the other sections as today. Rejected: counting wrapped lines; the criteria first;
  a count without the command; one line per section in the terminal. Narrowed by spec review round 2
  (SR2-3): the overflow line is the line before the answer line, which stays last.
- 2026-10-29: Decision (spec questions, the human, recommended): (1) the template gets `description` and
  the test helpers and the exact lists of waiting sections change with it (step 5); (2) the section line
  reads `<id>: N lines changed (+a -b)`, the `new, N lines` and `unchanged` forms stay; (3) the terminal
  form keeps the change block of Spec; (4) the overflow line names `stapel status KEY`, which shows no
  text for a new section (not promised). Rejected: the section only in this repository's `stapel.toml`;
  `N lines changed` without the counts; no Spec change block; naming `ticket.md` for new sections.
  Narrowed by spec review round 2 (SR2-1, SR2-2): the reserved title is lifted in step 2a; in step 5 the
  test fixture drops the `description` entry, so the old exact lists of waiting sections stay.
- 2026-10-29: Decision (spec review round 1, the human, recommended): the Frame part is every Description
  line outside `### Decisions`, the Source paragraph and other headings included, so nothing the grant
  binds stays unseen (SR1-2); a new or changed AC row is shown whole, its Test cell included, and the
  other changed Spec lines are counted in one line (SR1-3); section lines past the tenth fold into one
  count line, so the 22-line bound holds (SR1-5). Rejected: a count line for other Description text; the
  full Spec diff; not promised past ten sections.
- 2026-10-29: Decision (spec review round 2, the human): SR2-9 (a padded Frame of 22 or more new lines
  pushes every criterion out of the dialog, which the overflow line counts) is accepted as stated, and no
  third spec review round runs. Rejected: a cap on the Frame lines; a third round.

## Spec

Terms:

- **Waiting section**: a confirmable section without a fresh confirmation (`confirm::prepare_waiting`).
- **Logical line**: a paragraph or list item joined by spaces; `- `, `* ` or `<digits>. ` starts an item,
  a blank line or an AC row ends a paragraph, a heading (as `ticket::parse`: not in a fence or HTML
  comment) is a logical line of its own; a row in a fence or comment is plain text; counts are
  logical.
- **Parts**, in order: **Frame** (Description's logical lines outside the heading `### Decisions`, written exactly so, which runs to the
  next heading of level 1 to 3), **Criteria** (each `| AC-<n> |` row of Spec, trimmed; without rows, the
  Spec's logical lines), **Decisions** (the logical lines under `### Decisions`). A part appears only when
  its section (`description` or `spec`) is configured and waiting.
- **Shown lines**: without a confirmation with stored text, each line with `  ` before it; else the lines
  a line diff inserts or deletes, with `+ ` or `- `, in diff order. With AC rows, Criteria ends with
  `spec, other lines: +a -b` (the other Spec lines inserted and deleted; a new Spec counts all as a) when
  a + b > 0.
- **Count**: `1 line` for one, `<N> lines` otherwise; `section(s)` alike.
- **Section line**: `<id>: new, <count>` (no confirmation with stored text), `<id>: <count> changed (+a
  -b)` (N = a + b) or `<id>: unchanged`, then `; <reason>` per stale reason other than the text change
  (Design F-7). Sections past the tenth fold into `… <m> more waiting section(s): <id>, <id>`.
- **Dialog page**: `stapel: confirm <n> waiting section(s) of <KEY>; runs: <program>` (`<program>` as
  printed today), the section lines in `stapel.toml` order, each part with shown lines under its label
  (`Frame:`, `Criteria:`, `Decisions:`), the overflow line when needed, then `Answer Yes or No; do not
  choose "don't ask again".`
- **Overflow line**: `… <N> more line(s) (frame a, criteria b, decisions c); all of them: stapel status
  <KEY>`; a, b, c count each part's hidden shown lines, N their sum; a left-out label is not counted.
- **Clean**: each control character becomes one space (as `dialog::shown`), in dialog and terminal.
- **Cut**: a clean dialog line over 100 characters (Unicode scalar values) becomes its first 99 and `…`.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel init` writes a new `stapel.toml` THE first `[[sections]]` entry is `id = "description"`, `title = "Description"`, `owner = "product"` without `depends_on`, no section depends on it, and `[build] requires` is `["description", "spec", "design", "proof"]`; this repository's `.stapel/stapel.toml` has the same entry and `requires`; `Config::validate` accepts the title `Description` for the id `description` only. | `init::template_has_the_description_section`, `init_process::this_repository_confirms_the_description`, `core::config::rejects_bad_sections` |
| AC-2 | WHEN the guard asks about `stapel ok KEY` THE dialog text is the dialog page of the Terms, and each waiting section has its section line (`<id>: new, 1 line`, `<id>: 2 lines changed (+1 -1)`, `<id>: unchanged`, each followed by `; depends on <id>, which changed` and the other suffixes of the Terms) in place of the STP-17 line with the short hash. | `ok_page::section_lines`, `ok_all::dialog_lists_every_waiting_section`, `ok_all::new_with_reasons_in_terminal_and_dialog`, `ok_all::new_never_shows_the_text_change`, `ok_all::dialog_counts_added_and_removed_apart`, `ok_all::dialog_shows_every_stale_reason` |
| AC-3 | WHEN `spec` waits THE dialog page holds the Criteria part: every AC row whole, its Test cell included, for a Spec never confirmed with stored text, else only the rows added or removed since, then `spec, other lines: +a -b` when other Spec lines changed; IF the Spec has no AC row THEN the part holds the Spec's logical lines the same way. | `ok_page::criteria_new_and_changed`, `ok_page::criteria_without_rows` |
| AC-4 | WHEN `description` is configured and waits THE dialog page holds the Frame part, every Description line outside `### Decisions` with the Source paragraph and other headings, and the Decisions part; IF the Description has no `### Decisions` THEN all of it is the Frame part; IF `stapel.toml` has no `description` section THEN neither part appears. | `ok_page::frame_and_decisions_new_and_changed`, `ok_page::description_without_headings`, `ok_page::config_without_description`, `ok_page::decisions_boundaries`, `ok_page::frame_and_decisions_diffs` |
| AC-5 | WHEN the dialog page would hold more than 22 lines THE shown lines fill the space in the order Frame, Criteria, Decisions, a label only with at least one of its lines after it (so a page may hold 21 lines), and the overflow line stands before the answer line, so that the text holds 22 lines; more than ten waiting sections fold as the Terms say; every line is clean and cut at 100 characters, as the golden file `ok_page_dialog.txt`. | `ok_page::dialog_limit_and_cut`, `ok_page::dialog_exactly_at_the_limit`, `ok_page::many_waiting_sections`, `ok_page::cut_and_limit_boundaries` |
| AC-6 | WHEN `stapel ok KEY` runs in a terminal or with a grant THE output is the `to confirm:` line, the label and clean shown lines of each part without the limit and the cut, the change blocks of the waiting sections other than `description` as before, then the `confirmed:` lines, as the golden files `ok_page_terminal.txt`, `ok_key_form.txt` and `ok_change_blocks.txt`. | `ok_page::terminal_prints_the_whole_page`, `ok_all::key_form_confirms_every_waiting_section`, `ok_all::change_block_for_each_kind` |
| AC-7 | IF the Frame or the Decisions change after the dialog of `stapel ok KEY` THEN `stapel ok KEY --grant <token>` exits 1 with the text that the grant does not match and records nothing. | `ok_page::grant_binds_the_description`, `ok_page::grant_passes_when_unchanged` |
| AC-8 | WHEN `stapel next` gives `close` THE target is the command `stapel close <KEY> --reason "<text>"`. | `next::close_at_the_end`, `next::review_and_summary_edges`, `next::json_after_the_build`, `next::no_plan_section_configured` |
| AC-9 | WHEN `stapel init` writes the skill THE skill holds `They type no stapel command.`, ``- `close`: run the command that stapel next prints, with `<text>` replaced by one sentence of why the ticket is done; the person answers its permission dialog.`` and `stapel ok and stapel close run only as one plain command whose permission dialog the person answers.`, and no longer holds `the one exception is stapel close` or `stapel close is run by the person`. | `init_process::skill_text_runs_close_through_the_dialog`, `init_process::skill_text_is_consistent`, `init_process::texts_agree_on_close_lint_and_green_step` |
| AC-10 | IF `description` is configured and the ticket has no `## Description` or two THEN the guard denies `stapel ok KEY` and the command exits 1, both with `section description (## Description) is missing in ticket.md` or `section description (## Description) appears more than once`, as for any section. | `ok_page::description_missing_or_twice` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-10; the grant facts of the key form are those of STP-17
  and take `description` like any other section; the one-section form, its dialog and its output are
  unchanged; the other actions of `next` and their order are unchanged.
- **Not promised:** the Frame's list; how Claude Code wraps a line of up to 100 characters in a narrow
  terminal; `stapel status` shows a diff only for a section with stored text, so for a new section the
  overflow line's command shows no text (the ticket file holds it); a `stapel.toml` whose `description`
  section has another title or owner; a part whose subsection heading appears twice (the first one counts).

## Test plan

CLI tests in the new file `crates/stapel-cli/tests/ok_page.rs` with `common::repo_with_ticket`,
`common::set_section` and `common::ask`; a test that needs the `description` section inserts the
template's entry as the first `[[sections]]` entry of `.stapel/stapel.toml` (from step 5 on,
`common::stapel_repo` drops it, so every test adds it the same way). Golden files
`crates/stapel-cli/tests/golden/ok_page_dialog.txt` and `ok_page_terminal.txt`. Old tests of
`tests/ok_all.rs`, `tests/next.rs`, `tests/init_process.rs` and their golden files change in the RED of
their step. Light tests: `cargo test -j 4 -p stapel-cli --test ok_page -- --test-threads=3` (and `--test
ok_all`, `--test next`, `--test init`, `--test init_process`); before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Section lines | `spec: 2 lines changed (+1 -1)`; `design: unchanged; depends on spec, which changed`; `proof: new, 1 line`; no stored text: `new, 3 lines`; a Spec paragraph re-wrapped (`keep one` / `keep two` as `keep one keep` / `two`): `spec: 4 lines changed (+2 -2)`, no `Criteria:`; header first, answer line last | `ok_page::section_lines` |
| Old dialog tests | the STP-17 tests expect section lines, not hash lines (`3 lines changed (+2 -1)`) | `ok_all::dialog_lists_every_waiting_section`, `ok_all::new_with_reasons_in_terminal_and_dialog`, `ok_all::new_never_shows_the_text_change`, `ok_all::dialog_counts_added_and_removed_apart`, `ok_all::dialog_shows_every_stale_reason` |
| Criteria | a Spec with AC-1 and AC-2 never confirmed: both rows whole with `  ` and `spec, other lines: +a -0`; confirmed, then the Test cell of AC-2 changed, AC-3 added and a Terms line changed: `- ` and `+ ` of AC-2, `+ ` of AC-3, `spec, other lines: +1 -1`; spec not waiting: no `Criteria:` label | `ok_page::criteria_new_and_changed` |
| No rows | a Spec of two paragraphs and no AC row: two logical lines, no count line; a wrapped paragraph is one line | `ok_page::criteria_without_rows` |
| Frame and Decisions | a Description with a Source paragraph, `### Frame` (a paragraph, a list of two items) and `### Decisions` (two items): new, the Source line, the `### Frame` line and every Frame line under `Frame:`, the two items under `Decisions:`; after a confirmation, one Decisions item added: only `+ ` that item and no `Frame:` label | `ok_page::frame_and_decisions_new_and_changed` |
| Without headings | a Description without `### Frame`: its lines outside `### Decisions` under `Frame:`; without `### Decisions`: all its lines under `Frame:`, no `Decisions:` label | `ok_page::description_without_headings` |
| Old configuration | `stapel.toml` without `description`: no `Frame:` and no `Decisions:` label, the Criteria part as before | `ok_page::config_without_description` |
| No Description | `description` configured, the `## Description` heading removed; then doubled: the guard denies with the AC-10 text; the terminal form exits 1 with it; nothing recorded | `ok_page::description_missing_or_twice` |
| Limit and cut | 12 Frame lines, 10 criteria, 3 decisions; a criterion of 300 characters with `é` at the cut; one with `\x1b[31m` and a tab: 22 lines, equal to `ok_page_dialog.txt` | `ok_page::dialog_limit_and_cut` |
| At the limit | exactly 22 lines: no overflow line; one criteria line more: `… 1 more line (frame 0, criteria 1, decisions 0); all of them: stapel status ABC-1`, then the answer line; a `Decisions:` label with no room for a line: left out, its two lines give `decisions 2` | `ok_page::dialog_exactly_at_the_limit` |
| Many sections | 12 waiting sections: ten lines, then `… 2 more waiting sections: s11, s12`; 11: `… 1 more waiting section: s11`; at most 22 lines | `ok_page::many_waiting_sections` |
| Terminal | the same ticket in a terminal: the whole page without the cut, the control characters as spaces, then the change blocks of spec, design, proof and plan, then the `confirmed:` lines, equal to `ok_page_terminal.txt` | `ok_page::terminal_prints_the_whole_page` |
| Old terminal goldens | `ok_change_blocks.txt` with the Criteria part of the fixture Spec; `ok_key_form.txt` unchanged, its Spec does not wait | `ok_all::key_form_confirms_every_waiting_section`, `ok_all::change_block_for_each_kind` |
| Grant | the dialog, then a Frame edit; again, then a new Decisions line: exit 1, the mismatch text, `state.json` and `decisions.jsonl` unchanged | `ok_page::grant_binds_the_description` |
| Decisions bounds | `## Next` after Decisions goes back to Frame; `#### sub` stays in Decisions; a fenced `### Decisions` is no heading; an empty `### Decisions` gives no label | `ok_page::decisions_boundaries` |
| Frame and Decisions diffs | after a confirmation, a Frame line changed and a Decisions line removed: `+ `/`- `; a confirmation without stored text: `  ` | `ok_page::frame_and_decisions_diffs` |
| Cut and limit bounds | a line of 100 characters stays whole, one of 101 is cut; a part ending with one free line gives a page of 21 lines | `ok_page::cut_and_limit_boundaries` |
| Grant unchanged | the dialog, no edit: the grant confirms every waiting section | `ok_page::grant_passes_when_unchanged` |
| Template | `stapel init` in an empty repository: the `description` entry first, no `depends_on` naming it, the `requires` of AC-1; the file parses | `init::template_has_the_description_section` |
| Reserved title | the title `Description` for the id `design`: still refused; for the id `description`: accepted | `core::config::rejects_bad_sections` |
| This repository | `.stapel/stapel.toml` here: the same entry and `requires` | `init_process::this_repository_confirms_the_description` |
| Next close | the four `next` tests expect `stapel close ABC-1 --reason "<text>"`, also as the JSON `command` | `next::close_at_the_end`, `next::review_and_summary_edges`, `next::json_after_the_build`, `next::no_plan_section_configured` |
| Skill text | the AC-9 sentences, with the one about `<text>`, in the skill written by `init`; the old close sentences absent | `init_process::skill_text_runs_close_through_the_dialog`, `init_process::skill_text_is_consistent`, `init_process::texts_agree_on_close_lint_and_green_step` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` of `stapel ok` | ticket key | — | as STP-17 | as STP-17 | as `resolve` | `ok_all::dialog_lists_every_waiting_section` |
| `description` entry of `[[sections]]` | `stapel.toml` table | — | absent: no Frame or Decisions part | as `Config::parse_at` | exact id | `ok_page::config_without_description` |
| Description text | Markdown | — | `TODO`: one logical line | no `### Decisions`: all of it is the Frame part; no `## Description` or two: AC-10 | logical lines | `ok_page::description_without_headings`, `ok_page::description_missing_or_twice` |
| Spec text | Markdown | — | no AC row: logical lines | an `AC-` row without ` \|` after the number is not a row | as `row_number` in `brief.rs` | `ok_page::criteria_without_rows` |
| A page line | text | 0 / any characters | an empty logical line is never made | control characters: one space each | 100 Unicode scalar values | `ok_page::dialog_limit_and_cut` |
| Page length | lines | 3 / 22 in the dialog | no part: header, section lines, answer line | — | exact | `ok_page::dialog_exactly_at_the_limit` |
| Waiting sections | count | 1 / any; past ten, one line | none: no dialog (STP-17) | — | exact | `ok_page::many_waiting_sections` |
| Stored text of a confirmation | `text` field | — | absent: shown as new | — | normal form | `ok_page::section_lines` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | permission mode; whether the dialog is offered | as STP-17: modes without a dialog deny and write no grant | `ok_all::dialog_lists_every_waiting_section` |
| claude-code | how much of the dialog text is shown | about 25 lines (probe of 2026-10-29); the page holds 22 | `ok_page::dialog_limit_and_cut` |
| claude-code | allow rules that skip the dialog | as STP-17: denied | `ok_all::dialog_lists_every_waiting_section` |
| claude-code | environment of hook processes; transcripts; tool results | as STP-17; transcripts not read | — |
| git | every dimension | the key form reads no git state | — |
| cargo | every dimension | not run | — |
| os | terminal width; file permissions; WSL | not promised | — |

### Abuse table

Attempts of a careless agent against "the person sees what they confirm, and the yes covers it".

| Attempt | Outcome |
|---|---|
| The agent edits the Frame or a Decisions line after the dialog appears | the grant does not match, exit 1, nothing recorded (AC-7) |
| The agent edits a criterion after the dialog appears | the grant does not match (STP-17) |
| The agent writes text above the Frame or under another heading of Description | shown in the Frame part (AC-4) |
| The agent changes a Test cell, a Term or a table line of Spec | the row is shown whole, other lines in the count line (AC-3) |
| The agent writes an escape sequence into a Frame line | shown as spaces (AC-5, AC-6) |
| The agent re-wraps a paragraph or adds a blank line, every logical line equal | only the section line shows the change, `<count> changed (+a -b)`, with no shown lines (AC-2; promised) |
| A long Frame pushes a changed criterion out of the 22 lines | the overflow line counts the hidden criteria lines (AC-5) |
| A criterion longer than 100 characters hides its end | the cut shows `…` (AC-5); the rest is not promised |
| The agent shows a shorter page in the chat | the dialog text comes from the guard, not from the agent |
| The agent removes the `description` section from `stapel.toml` | the Frame is no longer shown or bound; not promised (`.stapel/` is writable by agents) |
| The agent runs `stapel close` without `--reason` or before a passing check | refused by the command as before (STP-2, STP-4) |

**Review Focus:** a Frame line that is a table row or a code block (read as a paragraph; not tested); a
diff of logical lines when an item is moved (shown as one `-` and one `+`; not tested); a section line of
a section confirmed under an older normal form (`unchanged; confirmed under normal form 0`, as STP-17; not
tested again); a ticket lint failure on the STP-17 criteria that quote the old dialog line (handled in the
step that changes it).

**Author self-check:** 2026-10-29, before spec review: every promise item has a criterion and a test (item
1: AC-1; item 2: AC-3, AC-4, AC-5; item 3: AC-2; item 4: AC-6; item 5: AC-7; item 6: AC-8, AC-9); each
input has its empty and invalid case; the rows were checked against `confirm::prepare_waiting`,
`confirm::change_block`, `dialog::waiting_line`, `dialog::ok_key_facts`, `review::criteria`,
`row_number` of `brief.rs`, `next.rs` (the close action), `crates/stapel-core/src/stapel.toml` and
`config::validate`.
2026-10-29, after spec review round 1 (stapel-spec-reviewer): 11 findings, 1 HIGH, 6 MEDIUM; 10 inside,
1 outside, not taken by the human (SR1-11). One change: the reserved title `Description` is lifted for
the id `description` (SR1-1); the Frame part is the whole Description outside Decisions (SR1-2); AC rows
whole and a count of other Spec lines (SR1-3); tests that pass at a RED moved to the coverage step
(SR1-4); more than ten section lines fold (SR1-5); one count form and the exact stale suffixes (SR1-6);
control characters as spaces (SR1-7); the close reason in the skill (SR1-8); a missing or doubled
Description refused (SR1-9, AC-10); step 4b marked as coverage and step 5 split (SR1-10).
2026-10-29, after spec review round 2 (stapel-spec-reviewer): 9 findings, 1 HIGH, 1 MEDIUM; all inside;
SR2-9 accepted by the human, no third round. One change: the reserved title is lifted in the new step 2a
before any step configures `description` (SR2-1); step 5 keeps the old tests on the old configuration
through `common::stapel_repo` and lists the files that depend on it (SR2-2); the overflow line before the
answer line (SR2-3); a left-out label not counted (SR2-4); the test entry inserted first (SR2-5); the
header's `runs:` as printed today (SR2-6); a re-wrap shows only the section line (SR2-7); stale step
numbers, `--full` and headings in fences or comments (SR2-8). Every step's tests were checked against
the step order: each fails at its RED for its own behaviour and passes at its GREEN.

## Design

### Facts

- F-0: `dialog::prepare_ask` builds the key-form reason as one line from `waiting_line` (id, short hash,
  `+a -b lines since last confirmation`, stale reasons); `finish` issues the grant from
  `dialog::ok_key_facts`, which covers every waiting section, so a configured `description` is bound
  without a change.
- F-1: `confirm::change_block` prints the terminal blocks; `ok.rs::run_key` prints `to confirm:`, the
  blocks, then the `confirmed:` lines.
- F-2: `review::criteria` gives `AC-<n>: <text>` from the Spec rows; `brief::frame_of` reads the
  `### Frame` subsection (it drops `**Size.**`; the page keeps every paragraph).
- F-3: the template `crates/stapel-core/src/stapel.toml` has `[build] requires = ["spec", "design",
  "proof"]`; `config::default_requires` serves a `stapel.toml` without `[build]`, as this repository's,
  and stays, so an old file without `description` still parses.
- F-4: twenty test files build repositories with `stapel init`; once the template has `description`,
  `description` waits in every fixture ticket: `tests/hook.rs::confirmed_repo`, `tests/next.rs::confirm_all`
  and the STP-17 tests that list the waiting sections exactly expect it.
- F-6: `Config::validate` refuses any section titled `Description` with `sections: the title Description is
  reserved for the ticket description` (`crates/stapel-core/src/config.rs`); `core::config::rejects_bad_sections`
  (`crates/stapel-core/tests/config.rs`) tests it for the id `design`.
- F-7: the stale reasons of `stage::Reason::describe` other than the text change: `depends on <id>, which
  changed`, `depends on <id>, which is no longer configured`, `depends on <id>, which is missing`,
  `depends on <id>, which is duplicated`, `section missing`, `section duplicated`, `confirmed under normal
  form <n>`.
- F-5: `next::after_the_steps` gives `close` with `stapel close <KEY>`; the guard already asks for
  `stapel close` with a grant (STP-2).

### Decisions (before the build)

- The new module `crates/stapel-core/src/page.rs` holds the pure parts: logical lines, the parts of a
  ticket, the shown lines, the section line, the dialog page with the limit and the cut, and the terminal
  page. `dialog.rs` and `ok.rs` call it; `waiting_line` goes.
- The line diff of the shown lines is `similar::TextDiff::from_lines` over the logical lines joined by
  newlines; the stored text is the confirmation's `text` of the part's section.
- Step 2a lifts the rule of F-6 for the id `description` only, before steps 2b to 4b configure that
  section in their tests; its RED adds the accepted case to `core::config::rejects_bad_sections`, which fails
  there.
- The template change of step 5 would make `description` wait in every ticket that `common::stapel_repo`
  builds. These test files and golden files depend on the old section list: `tests/ok.rs`,
  `tests/ok_all.rs`, `tests/status.rs`, `tests/hook.rs` (`confirmed_repo`), `tests/next.rs`
  (`confirm_all`), `tests/closed_pipe.rs`, `tests/decisions.rs`, `tests/flow.rs`, `tests/cli_config.rs`,
  `golden/next_text.txt`, `golden/next_json.txt`, `golden/ok_key_form.txt` and `golden/ok_change_blocks.txt`.
  The RED of step 5 makes `common::stapel_repo` remove the template's `description` entry and its
  `requires` item from the `stapel.toml` that `init` wrote (nothing to remove before the GREEN), so none of
  them changes; the `description` behaviour is tested by `tests/ok_page.rs`, which inserts the entry.
- Step 5 also adds the entry and `[build]` to this repository's `.stapel/stapel.toml`; from then on STP-25
  waits for `description`, and the orchestrator runs `stapel ok STP-25` before step 6.
- The STP-17 criteria that quote the old dialog line: if the ticket lint flags them, the GREEN of step 1
  marks them as replaced by STP-25 AC-2 in `.stapel/tickets/STP-17/ticket.md`.

### Decisions (as built)

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `init::template_has_the_description_section`, `init_process::this_repository_confirms_the_description`, `core::config::rejects_bad_sections` |
| AC-2 | `ok_page::section_lines`, `ok_all::dialog_lists_every_waiting_section`, `ok_all::new_with_reasons_in_terminal_and_dialog`, `ok_all::new_never_shows_the_text_change`, `ok_all::dialog_counts_added_and_removed_apart`, `ok_all::dialog_shows_every_stale_reason` |
| AC-3 | `ok_page::criteria_new_and_changed`, `ok_page::criteria_without_rows` |
| AC-4 | `ok_page::frame_and_decisions_new_and_changed`, `ok_page::description_without_headings`, `ok_page::config_without_description`, `ok_page::decisions_boundaries`, `ok_page::frame_and_decisions_diffs` |
| AC-5 | `ok_page::dialog_limit_and_cut`, `ok_page::dialog_exactly_at_the_limit`, `ok_page::many_waiting_sections`, `ok_page::cut_and_limit_boundaries` |
| AC-6 | `ok_page::terminal_prints_the_whole_page`, `ok_all::key_form_confirms_every_waiting_section`, `ok_all::change_block_for_each_kind` |
| AC-7 | `ok_page::grant_binds_the_description`, `ok_page::grant_passes_when_unchanged` |
| AC-8 | `next::close_at_the_end`, `next::review_and_summary_edges`, `next::json_after_the_build`, `next::no_plan_section_configured` |
| AC-9 | `init_process::skill_text_runs_close_through_the_dialog`, `init_process::skill_text_is_consistent`, `init_process::texts_agree_on_close_lint_and_green_step` |
| AC-10 | `ok_page::description_missing_or_twice` |

## Plan

Route: full (tags `guard`, `interface`), auto.

1. Dialog page and section lines (AC-2; tests `ok_page::section_lines`,
   `ok_all::dialog_lists_every_waiting_section`, `ok_all::new_with_reasons_in_terminal_and_dialog`,
   `ok_all::new_never_shows_the_text_change`, `ok_all::dialog_counts_added_and_removed_apart`,
   `ok_all::dialog_shows_every_stale_reason`); the old five fail at the RED. Must not change: grant facts.
2. The Criteria part in the dialog (AC-3; tests `ok_page::criteria_new_and_changed`,
   `ok_page::criteria_without_rows`). Must not change: step 1.
2a. The reserved title lifted for `description` (AC-1; test `core::config::rejects_bad_sections`): the RED adds
   the accepted case for the id `description`, so that the test fails there; the GREEN changes
   `crates/stapel-core/src/config.rs`. Must not change: the refusal for any other id.
2c. Fix of step 2 after its drift review (AC-3; tests `ok_page::criteria_new_and_changed`,
   `ok_page::criteria_without_rows`): the RED adds failing cases: a blank line or a re-wrap among the
   other Spec lines gives no count line (logical lines); `+0 -0` none; a removed row; no stored text; a
   4-space `# x`, a heading in a fence or comment; list items; the diff of a Spec without rows. The GREEN
   reads lines, headings and rows by the rules of `ticket::parse`, with no second copy. Must not change: 1.
2b. Frame and Decisions in the dialog (AC-4; tests `ok_page::frame_and_decisions_new_and_changed`,
   `ok_page::description_without_headings`). Must not change: 1, 2.
3. The limit and the cut (AC-5; tests `ok_page::dialog_limit_and_cut`,
   `ok_page::dialog_exactly_at_the_limit`); the golden file comes with the RED. Must not change: steps 1
   to 2b below the limit.
3b. Coverage, no GREEN (AC-5; test `ok_page::many_waiting_sections`): the fold past ten sections came
   with step 1, so the test passes at once.
3c. Fix (AC-3, AC-4; tests `ok_page::criteria_new_and_changed`,
   `ok_page::frame_and_decisions_new_and_changed`): RED: a Spec whose rows all went shows them as `- `;
   `### decisions` is no heading. GREEN: `page.rs`.
3d. Coverage, no GREEN (AC-4, AC-5, AC-7; tests `ok_page::decisions_boundaries`,
   `ok_page::frame_and_decisions_diffs`, `ok_page::cut_and_limit_boundaries`,
   `ok_page::grant_passes_when_unchanged`): new functions only, as the Test plan rows say.
4. The terminal page (AC-6; tests `ok_page::terminal_prints_the_whole_page`,
   `ok_all::change_block_for_each_kind`): the RED adds `ok_page_terminal.txt` and changes
   `ok_change_blocks.txt` so that the tests fail there. Must not change: the one-section output;
   `ok_key_form.txt` (its Spec does not wait).
4b. Coverage, no GREEN (AC-4, AC-7, AC-10; tests `ok_page::grant_binds_the_description`,
   `ok_page::config_without_description`, `ok_page::description_missing_or_twice`). Tests only, each
   helper nested inside its function, no `cargo fmt`; a test that fails is an `intent_gap`.
5. The `description` section in the template (AC-1; tests `init::template_has_the_description_section`,
   `init_process::this_repository_confirms_the_description`): the RED adds the tests, which fail there,
   and makes `common::stapel_repo` drop the entry as Design says; the GREEN changes
   `crates/stapel-core/src/stapel.toml` and `.stapel/stapel.toml`. Must not change: `default_requires`;
   the old tests and golden files Design lists.
5b. Repair of step 5, tests only (AC-1; tests `core::config::build_requires_defaults`,
   `core::config::default_config_has_roles_and_sections`, `core::config::rejects_bad_sections`): these
   old tests pin the template and fail since the GREEN of step 5, which its RED missed; they now expect
   the `description` entry and `requires`. Must not change: any other test.
6. Close through the dialog (AC-8, AC-9; tests `next::close_at_the_end`,
   `next::review_and_summary_edges`, `next::json_after_the_build`, `next::no_plan_section_configured`,
   `init_process::skill_text_runs_close_through_the_dialog`, `init_process::skill_text_is_consistent`,
   `init_process::texts_agree_on_close_lint_and_green_step`): the RED changes the old tests so that they
   fail there; the GREEN changes `next.rs`, `crates/stapel-cli/assets/skill.md` and runs the built
   `stapel init` here so that `init_process::this_repository_has_the_current_files` holds. Must not
   change: the other actions of `next`; the builder's ban on `stapel close`.

## Review

TODO

## Summary

TODO
