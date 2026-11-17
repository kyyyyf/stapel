# STP-32 — The human page as a file

## Description

Source: `docs/PHASES.md`, plan item 3.1; ledger decision 2026-11-17 (phases 3 to 5 start before the gate).

### Frame

**Problem.** The dialog of `stapel ok KEY` holds the page of STP-25: at most 22 lines, cut at 100
characters, the rest counted and hidden. A real Frame and its criteria do not fit, so the person approves
text they did not see in full, and the chat is not a good place to read it either. The person reads files
in the editor; the page should be a file there, and the dialog should only bind it.

**Plan item:** 3.1.

**Risk tags:** `guard` (the text and the grant of the guard's dialog for `stapel ok KEY`); `interface` (a
new command, a new file, a changed `next` action, the skill text).

**Non-goals.** An editor extension or an LSP (plan item 3.1 says so); the one-section form `stapel ok KEY
<section>`; the dialogs of `stapel close`, `stapel ledger add` and `stapel tokens import-records`;
`stapel status` output; the person's language; deliberate edits of the page meant to pass the check.

**Promise (closed list).**

1. `stapel page [KEY]` writes `.stapel/tickets/KEY/page.md` for a ticket with waiting sections: the title, the Frame
   (the Description outside `### Decisions`), the criteria, the Decisions lines, and one line each for
   Design, Proof, Plan and every other waiting section.
2. Each part of the page is the full text in a `diff` fence: a space before an unchanged line, `+` before a
   line added and `-` before a line removed since the last confirmation of its section, in place.
   `stapel init` adds `.stapel/tickets/*/page.md` to the ignore lines of its mode; `stapel page` refuses
   when git does not ignore the page.
3. The guard's dialog for `stapel ok KEY` shows a short summary: the title, the criteria count, one line
   per waiting section with its change, the path and the hash of the page.
4. The guard refuses the dialog, naming `stapel page KEY`, when the page is missing or differs from
   the page the current ticket gives (an edit of the ticket or of the page after it was written).
5. The grant binds the hash of the page file; the command refuses a grant whose page hash differs.
6. `stapel next` names `stapel page KEY` before `stapel ok KEY` when the page is missing or stale; the
   skill text says the orchestrator runs `stapel page KEY` and names the page's path in the chat before
   `stapel ok KEY`, also after a review round, and the person reads the page while the dialog waits.

Not promised: the terminal form of `stapel ok KEY` without an agent shell (it prints the page as today);
a page for a closed ticket; the page of a ticket whose `stapel.toml` has no `description` section beyond
the criteria and the section lines.

**Size.** About 300 source lines and 600 test lines, 6 to 8 Plan steps; split past 600 source lines.
Route: full (tags `guard`, `interface`), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-17: Decision (page writer, the human, recommended): a new command `stapel page KEY` writes the
  page; the orchestrator runs it before `stapel ok KEY` and names the path in the chat; `stapel next` names
  it when the page is missing or stale. Rejected: the guard writes the page at the dialog (the person has
  not read it before the "yes").
- 2026-11-17: Decision (tracking, the human, recommended): `page.md` is not tracked; `stapel init` adds
  `.stapel/tickets/*/page.md` to the ignore lines. Rejected: tracked like `ticket.md` (a second changed
  file on every edit, and an uncommitted page would block the build).
- 2026-11-17: Decision (change mark, the human, recommended): each part is the full text in a `diff`
  fence with a space, `+` or `-` before each line, removed lines in place; the base is the last
  confirmation with stored text of the section. Rejected: only the changed lines (no context); rendered
  Markdown with marks (removed lines away from their place).
- 2026-11-17: Assumption (orchestrator): the page is stale when its bytes differ from the page that
  `stapel page` would write now; the hash is the sha256 of those bytes.

## Spec

Terms:

- **Waiting section**, **logical line**, **section line**, **clean** and **cut**: as STP-25
  (`crates/stapel-core/src/page.rs`).
- **Page**: the file `.stapel/tickets/<KEY>/page.md` that `stapel page` writes, in this order: `# <KEY> —
  <title>`; `## Sections` with one item `- <section line>` per waiting section in `stapel.toml` order, no
  fold; then `## Frame`, `## Criteria` and `## Decisions`, each only when its part (as STP-25) has a line,
  holding the part in one diff fence. A blank line stands between blocks; the file ends with one newline.
- **Mark**: inside a fence each clean logical line has one character directly before it, with no space
  after the mark: a space when its section has no confirmation with stored text or the line is unchanged
  since it, `+` when the line diff of STP-25 inserts it, `-` when it deletes it; deleted lines stand where
  the diff puts them. The Criteria count line `spec, other lines: +a -b` stands after the Criteria fence
  as a plain line, without a mark. The title and the section lines are clean too.
- **Diff fence**: a line of backticks and `diff`, the marked lines, a line of the same backticks; the
  backticks are one more than the longest run of backticks in the part's lines, and at least three.
- **Fresh page**: `page.md` whose bytes equal the page that `stapel page` would write now; **page hash**:
  `sha256:` and the hex sha256 of those bytes; the **short page hash** keeps 12 hex digits.
- **Summary**: the dialog text of `stapel ok KEY`: `stapel: confirm <n> waiting section(s) of <KEY>;
  runs: <program>` (as STP-25), `title: <title>`, `criteria: <n>` (the AC rows of the ticket's current
  Spec, as STP-25's `spec_lines`, whether Spec waits or not), the section lines (folded past the tenth, as STP-25), `page: .stapel/tickets/<KEY>/page.md
  (<short page hash>)`, then `Answer Yes or No; do not choose "don't ask again".`; every line clean and cut.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel page [KEY]` runs for a ticket with waiting sections THE command writes the page and prints `page: <path> (<short page hash>)`; IF no section waits THEN it prints `nothing to confirm: <KEY>` and writes nothing; a foreign KEY or a closed ticket is refused as by `stapel ok KEY`; IF git does not ignore the page's path THEN it exits 1 with `.stapel/tickets/<KEY>/page.md is not ignored by git; add /.stapel/tickets/*/page.md to the ignore lines or run stapel init` and writes nothing. | `page::writes_the_page`, `page::nothing_waits`, `page::refusals_as_ok`, `page::refuses_when_not_ignored` |
| AC-2 | WHEN a ticket was never confirmed THE page holds every part with a space before each line, as the golden file `page_new.md`; WHEN it was confirmed and then edited THE page marks the changed lines with `+` and `-` in place, as `page_changed.md`. | `page::layout_of_a_new_ticket`, `page::marks_since_the_last_confirmation` |
| AC-3 | WHEN parts are built THE rules of STP-25 hold in the page: the Frame and the Decisions split at `### Decisions`, every AC row whole, the count line of the Terms when other Spec lines changed, a Spec without rows as its logical lines, no part for a section that is not configured or not waiting. | `ok_page::criteria_new_and_changed`, `ok_page::criteria_without_rows`, `ok_page::frame_and_decisions_new_and_changed`, `ok_page::description_without_headings`, `ok_page::config_without_description`, `ok_page::decisions_boundaries`, `ok_page::frame_and_decisions_diffs` |
| AC-4 | WHEN a part's line holds backticks or control characters THE fence is longer than any backtick run in the part and each control character is one space; the page has no line limit and no cut. | `page::fence_longer_than_any_backtick_run`, `ok_page::dialog_exactly_at_the_limit` |
| AC-5 | WHEN `stapel init` writes its ignore lines, in shared and in local mode THE lines hold `/.stapel/tickets/*/page.md`; this repository's `.gitignore` holds it too. | `init::page_is_ignored_in_both_modes` |
| AC-6 | WHEN the guard asks about `stapel ok KEY` with a fresh page THE dialog text is the summary, as the golden file `ok_page_dialog.txt`; the Frame, criteria and Decisions lines are not in it. | `ok_page::section_lines`, `ok_page::many_waiting_sections`, `ok_page::dialog_limit_and_cut`, `ok_page::cut_and_limit_boundaries` |
| AC-7 | IF the page is missing THEN the guard denies `stapel ok KEY` with `the page of <KEY> is missing; run stapel page <KEY> and read <path>`; IF it is not fresh THEN with `the page of <KEY> is stale; run stapel page <KEY> and read <path> again`. | `page::guard_refuses_without_the_page`, `page::guard_refuses_a_stale_page` |
| AC-8 | IF the page file changes after the dialog, rewritten or edited by hand, or is removed THEN `stapel ok KEY` with the dialog's grant exits 1 with the grant mismatch text of STP-17 and records nothing; the command does not check freshness itself, the grant facts hold the page hash or `missing`. | `page::grant_binds_the_page` |
| AC-9 | WHEN sections wait and nothing before `confirm` blocks, and the page is missing or not fresh THE `stapel next` action is `write-page` with the command `stapel page <KEY>` and the facts lines of `confirm`; with a fresh page it is `confirm` as before. | `next::page_before_confirm` |
| AC-10 | WHEN `stapel init` writes the skill THE skill holds ``- `write-page`: run `stapel page KEY` and name the path it prints in the chat; the person reads the page in the editor while the dialog of `stapel ok KEY` waits.`` and rule 5 holds ``Right after the list run `stapel page KEY`, name the path of the page in the chat, then run `stapel ok KEY` as one plain command``. | `init_process::skill_text_names_the_page` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-10. Unchanged: the terminal form of `stapel ok KEY` (it
  prints the STP-25 terminal page and needs no `page.md`); the one-section form; the grant facts other
  than the page hash; the other actions of `next` and their order. Unchanged and kept passing: the STP-25
  tests `ok_page::description_missing_or_twice` (a missing or doubled section is denied with its text
  before the page is read), `ok_page::grant_binds_the_description` and `ok_page::grant_passes_when_unchanged`.
- **Not promised:** the Frame's list; how an editor renders the fence; a page left behind after the
  close or after the sections are confirmed (it is ignored by git and stale); a `page.md` that is a
  folder or cannot be written (exit 1 with the file system's error, not tested); a symlinked `page.md`;
  case-insensitive file systems; bidi and other format characters (only control characters, Unicode
  category Cc, become spaces); a KEY of 49 characters or more, whose `page:` summary line is cut; a person
  who read an older page before the agent rewrote it (SR1-7, deferred); the Spec lines that are not AC
  rows, shown only as the count line (SR1-8, deferred).

## Test plan

CLI tests in the new file `crates/stapel-cli/tests/page.rs` with `common::repo_with_ticket`,
`common::set_section`, `common::ask` and `common::hook_bash`; golden files
`crates/stapel-cli/tests/golden/page_new.md` and `page_changed.md`; `ok_page_dialog.txt` is rewritten.
The STP-25 tests in `tests/ok_page.rs` named in AC-3, AC-4 and AC-6 keep their names, which STP-25
names for the lint, and change their bodies: they read the page in place of the dialog text (a coverage
commit, step 2), or the summary in place of the STP-25 dialog page (the RED of step 3a);
`ok_page::dialog_exactly_at_the_limit` now proves that the page has no limit. `common::ask`
runs `stapel page <arg>` first when the command is `stapel ok <one arg>`, ignoring its result, so that the
old dialog tests keep a fresh page. The `next` tests and golden files that expect `confirm` without a page
expect `write-page` from step 5. Light tests: `stapel cargo test -j 4 -p stapel-cli --test page --
--test-threads=4` (and `--test ok_page`, `--test ok_all`, `--test next`, `--test init`, `--test
init_process`); before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Write | waiting spec and plan: `page.md` written, the line `page: .stapel/tickets/ABC-1/page.md (sha256:` + 12 hex + `)`, the hash equal to the sha256 of the file; run again with no edit: the same bytes | `page::writes_the_page` |
| Nothing waits | every section confirmed: `nothing to confirm: ABC-1`, no `page.md` | `page::nothing_waits` |
| Refusals | an unknown KEY, a closed ticket, a foreign KEY in a ticket worktree: exit 1 with the same text as `stapel ok KEY`; no file | `page::refusals_as_ok` |
| Not ignored | the ignore line removed from `.gitignore`: exit 1 with the AC-1 text, no file; the line in `.git/info/exclude` only: written | `page::refuses_when_not_ignored` |
| New ticket | Description with Source, Frame and Decisions; Spec with two AC rows; Design, Proof and Plan waiting: equal to `page_new.md` | `page::layout_of_a_new_ticket` |
| Changed ticket | the same, confirmed, then one Frame line changed, AC-2 changed, AC-3 added, a Terms line changed, a Decisions line removed: equal to `page_changed.md`; unchanged lines keep a space; `spec, other lines: +1 -1` after the Criteria fence | `page::marks_since_the_last_confirmation` |
| Parts (STP-25) | the cases of the STP-25 rows for these tests, read from `page.md` with the marks of the Terms | `ok_page::criteria_new_and_changed`, `ok_page::criteria_without_rows`, `ok_page::frame_and_decisions_new_and_changed`, `ok_page::description_without_headings`, `ok_page::config_without_description`, `ok_page::decisions_boundaries`, `ok_page::frame_and_decisions_diffs` |
| Fence | a criterion with three backticks and one with five: the Criteria fence has six; a Frame line with `\x1b[31m` and a tab: spaces; a title with a tab: a space | `page::fence_longer_than_any_backtick_run` |
| No limit | 40 Frame lines and a criterion of 300 characters: all in the page, whole, no `more line` text | `ok_page::dialog_exactly_at_the_limit` |
| Ignore lines | `stapel init` and `stapel init --local`: the line in `.gitignore` or `.git/info/exclude`; a written page is not in `git status --porcelain`; this repository's `.gitignore` holds it | `init::page_is_ignored_in_both_modes` |
| Summary | the STP-25 fixture of `section_lines`: the summary with `title: ABC-1 title`, `criteria: 0`, the section lines and the `page:` line; no `Criteria:` label; a confirmed Spec with two AC rows and a waiting Plan: `criteria: 2` | `ok_page::section_lines` |
| Fold | 12 waiting sections: ten lines, then `… 2 more waiting sections: s11, s12`; the page lists all twelve | `ok_page::many_waiting_sections` |
| Cut | a title of 300 characters with `é` at the cut and `\x1b[31m`: the summary equal to `ok_page_dialog.txt`; a title of 100 characters whole, 101 cut | `ok_page::dialog_limit_and_cut`, `ok_page::cut_and_limit_boundaries` |
| Missing page | no `page.md` (`hook_bash` without `stapel page`): deny with the AC-7 text, no grant written | `page::guard_refuses_without_the_page` |
| Stale page | the page, then a Spec edit; again, then a hand edit of `page.md`; again, then `stapel ok ABC-1 spec` in the terminal: each denied with the stale text | `page::guard_refuses_a_stale_page` |
| Grant | the dialog, then `page.md` edited by hand (ticket unchanged); again, then removed; again, then rewritten after a Decisions edit: exit 1 with the grant mismatch text, `state.json` and `decisions.jsonl` unchanged | `page::grant_binds_the_page` |
| Next | waiting sections, no page: `next: write-page — stapel page ABC-1` with the `blocked:` lines that `confirm` prints; after `stapel page`: `next: confirm — stapel ok ABC-1`; after a Spec edit: `write-page` again; a blocking finding comes first; the JSON `command` field | `next::page_before_confirm` |
| Old next tests | the `confirm` expectations of `tests/next.rs`, `tests/triage.rs`, `golden/next_text.txt` and `golden/next_json.txt` become `write-page`, or write the page first where the test means `confirm` | the tests of those files |
| Skill text | the two AC-10 texts in the skill written by `init` and in this repository's skill; the old `Right after the list run \`stapel ok KEY\`` absent | `init_process::skill_text_names_the_page` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` of `stapel page` | ticket key, optional | — | absent: the ticket of the current branch | unknown, closed or foreign: refused as `stapel ok KEY` | as `tickets::resolve` | `page::refusals_as_ok` |
| Ignore state of the page's path | `git check-ignore` | — | no ignore line: refused (AC-1) | — | as git | `page::refuses_when_not_ignored` |
| `page.md` | file | 0 bytes / any | absent: AC-7 missing | edited, older, a byte added: AC-7 stale | exact bytes | `page::guard_refuses_a_stale_page` |
| A part line | text | 0 / any characters | an empty logical line is never made | control characters: one space each; backticks: a longer fence | Unicode scalar values; no cut | `page::fence_longer_than_any_backtick_run` |
| Waiting sections | count | 1 / any; the summary folds past ten, the page does not | none: AC-1 `nothing to confirm` | — | exact | `ok_page::many_waiting_sections` |
| Title | text | 0 / any characters | empty: `title: ` | control characters: spaces | cut at 100 in the summary | `ok_page::dialog_limit_and_cut` |
| The grant of `stapel ok KEY` | token | — | — | page hash differs: exit 1 | exact facts | `page::grant_binds_the_page` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | permission mode; whether the dialog is offered; allow rules | as STP-17: modes without a dialog deny and write no grant | `ok_all::dialog_lists_every_waiting_section` |
| claude-code | how much of the dialog text is shown | the summary holds at most 16 lines | `ok_page::many_waiting_sections` |
| git | ignore lines; the page untracked | `init` writes the line in both modes; a page is not in `git status` | `init::page_is_ignored_in_both_modes` |
| git | worktree of another ticket | a foreign KEY refused as by `stapel ok KEY` | `page::refusals_as_ok` |
| git | dirty tree: a worktree or repository without the ignore line | `stapel page` refuses, so no untracked page makes the tree dirty | `page::refuses_when_not_ignored` |
| os | the page cannot be written (a folder, permissions) | exit 1 with the error; not tested | — |
| os | symlinks; case-insensitive file systems | not promised | — |
| os | editor holds the file open; line endings changed by an editor | a changed byte makes the page stale (AC-7) | `page::guard_refuses_a_stale_page` |
| cargo | every dimension | not run | — |

### Abuse table

Attempts of a careless agent against "the person reads the page, and the yes covers it".

| Attempt | Outcome |
|---|---|
| The agent edits `page.md` to hide or soften a line | the page is stale; the guard denies (AC-7) |
| The agent edits `page.md` after the dialog appears | the page hash differs; the grant fails with the mismatch text, nothing recorded (AC-8) |
| The agent edits the ticket and rewrites the page after the person read it, before the dialog | the summary's section lines show the change; the rest is not promised (SR1-7, deferred) |
| The agent edits the ticket after writing the page | the page is stale (AC-7); after the dialog, the grant fails (AC-8, STP-17) |
| The agent removes `page.md` after the dialog | the grant fails (AC-8) |
| The agent names another path in the chat | the summary shows the real path and its hash (AC-6) |
| The agent writes a fence into a Frame line to close the diff block early | the fence is longer than any backtick run (AC-4) |
| The agent writes a control character (Unicode Cc, an escape sequence) into a line | shown as a space in the page and the summary (AC-4, AC-6); bidi characters are not promised |
| The agent runs `stapel ok KEY` without writing the page | denied, naming `stapel page KEY` (AC-7) |
| The agent commits `page.md` with `git add -f` | not promised |

**Review Focus:** a test that takes a snapshot of the repository before `common::ask` sees the page
`common::ask` writes (each such test changes in the RED of step 3a; not tested on its own); a logical line
that starts with `+` or `-` reads as a mark (the mark is the first character only; not tested); CRLF in
`ticket.md` (the page uses the parsed lines; not tested); `stapel page` run while the guard compares (no
lock; not promised).

**Author self-check:** 2026-11-17, before spec review: every promise item has a criterion and a test
(item 1: AC-1, AC-3; item 2: AC-2, AC-4, AC-5; item 3: AC-6; item 4: AC-7; item 5: AC-8; item 6: AC-9,
AC-10); each input has its empty and invalid case; the rows were checked against `page.rs` (`parts`,
`section_line`, `dialog_page`, `terminal_page`), `dialog.rs` (`ok_key_facts`, `ask`), `ok.rs`
(`run_key`), `next.rs` (the `confirm` action), `init.rs` (`IGNORE_LINES`) and `common::ask`.
2026-11-17, after spec review round 1 (stapel-spec-reviewer): 13 findings, 0 HIGH, 8 MEDIUM; 11 inside,
2 outside and deferred (SR1-7, SR1-8). One change: step 2 is a coverage commit and the fence test has its
own step 2a (SR1-1); step 4 names only the tests that fail at its RED (SR1-2); the command compares the
grant facts with the page hash and prints the mismatch text, no own freshness check (SR1-3); `criteria:
<n>` counts the current Spec's rows (SR1-4); rule 5 of the skill runs `stapel page` before `stapel ok`
(SR1-5); `stapel page` refuses an unignored page (SR1-6); `write-page` keeps the facts of `confirm`
(SR1-9); the count line after the fence and the bare mark (SR1-10); cleaning of the title, Cc only
(SR1-12); kept test names, symlinks and case not promised (SR1-13). SR1-11 no change.

## Design

### Facts

- F-1: `page::dialog_page` builds the STP-25 dialog page from `parts`, `section_line` and `clean_cut`;
  `page::terminal_page` prints the parts without the limit; `ok.rs::run_key` prints it in the terminal form.
- F-2: `dialog::ok_key_facts` gives the grant facts of the key form (ids, hashes, dependencies); the
  command recomputes them and `grant::consume` compares.
- F-3: `next::decide` gives `confirm` with `stapel ok <KEY>` when a section is unconfirmed and nothing
  before it blocks; its facts come from the caller, which reads the disk.
- F-4: `init.rs` has `IGNORE_LINES` (`/.stapel/index/`, `/.claude/worktrees/`), written to `.gitignore`
  or, in local mode, to `.git/info/exclude`.
- F-5: about 70 tests ask the guard about `stapel ok` through `common::ask` (`ok_page.rs`, `ok_all.rs`,
  `hook.rs`, `ok.rs`, `init_process.rs`, `flow.rs`, others).

### Decisions (before the build)

- `page.rs` gets `page_text(config, key, title, waiting) -> String` (the file), `summary(...)` (the
  dialog) and `page_state(dir, text) -> Fresh(hash) | Missing | Stale`; `dialog_page` and its overflow
  line go once the summary replaces it (step 3a). The parts and the marks share `parts` and `diff_lines`
  with a third form that keeps equal lines and writes the mark with no space after it (STP-25's form
  writes `+ ` and `- `).
- `stapel page` asks `git check-ignore -q` for the page's path before it writes.
- The new command lives in `crates/stapel-cli/src/page.rs`; it is not a confirmation, so it runs in an
  agent shell.
- The grant facts of the key form add the item `page=<page hash>` (or `page=missing`) to
  `dependencies`; the guard computes it from a fresh page, the command from the bytes of `page.md` as
  they are, and `grant::consume` compares; the command has no freshness check of its own.
- `next` takes a new fact `page_fresh: bool` from its caller; the caller renders the page only when
  sections wait.
- This repository's skill and `.gitignore` are written by `stapel cargo run -p stapel-cli -- init` from
  the worktree, because the installed binary is not reinstalled until PIL-3 closes (ledger 2026-11-17).
- If the ticket lint flags a quoted line of STP-25 that the summary removes, the GREEN of step 3a marks
  that criterion as replaced by STP-32 AC-6 in `.stapel/tickets/STP-25/ticket.md`.

### Decisions (as built)

TODO

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `page::writes_the_page`, `page::nothing_waits`, `page::refusals_as_ok`, `page::refuses_when_not_ignored` |
| AC-2 | `page::layout_of_a_new_ticket`, `page::marks_since_the_last_confirmation` |
| AC-3 | `ok_page::criteria_new_and_changed`, `ok_page::criteria_without_rows`, `ok_page::frame_and_decisions_new_and_changed`, `ok_page::description_without_headings`, `ok_page::config_without_description`, `ok_page::decisions_boundaries`, `ok_page::frame_and_decisions_diffs` |
| AC-4 | `page::fence_longer_than_any_backtick_run`, `ok_page::dialog_exactly_at_the_limit` |
| AC-5 | `init::page_is_ignored_in_both_modes` |
| AC-6 | `ok_page::section_lines`, `ok_page::many_waiting_sections`, `ok_page::dialog_limit_and_cut`, `ok_page::cut_and_limit_boundaries` |
| AC-7 | `page::guard_refuses_without_the_page`, `page::guard_refuses_a_stale_page` |
| AC-8 | `page::grant_binds_the_page` |
| AC-9 | `next::page_before_confirm` |
| AC-10 | `init_process::skill_text_names_the_page` |

## Plan

Route: full (tags `guard`, `interface`), auto.

1. The command `stapel page` (AC-1; tests `page::writes_the_page`, `page::nothing_waits`,
   `page::refusals_as_ok`, `page::refuses_when_not_ignored`): the page holds the title and the Sections
   list only; the parts come in step 1b. The tests write the ignore line themselves where they need it.
   Must not change: the dialog and the terminal form.
1b. The parts and the marks (AC-2; tests `page::layout_of_a_new_ticket`,
   `page::marks_since_the_last_confirmation`); the golden files come with the RED. Must not change: step 1.
2. Coverage, no GREEN (AC-3, AC-4; tests `ok_page::criteria_new_and_changed`,
   `ok_page::criteria_without_rows`, `ok_page::frame_and_decisions_new_and_changed`,
   `ok_page::description_without_headings`, `ok_page::config_without_description`,
   `ok_page::decisions_boundaries`, `ok_page::frame_and_decisions_diffs`,
   `ok_page::dialog_exactly_at_the_limit`): the old bodies read `page.md` in place of the dialog text and
   pass at once, because step 1 writes the parts.
2a. The fence and the cleaning (AC-4; test `page::fence_longer_than_any_backtick_run`). Must not change:
   steps 1 and 2.
3. The ignore line (AC-5; test `init::page_is_ignored_in_both_modes`); the GREEN also runs `init` here.
   Must not change: the other ignore lines.
3a. The summary (AC-6; tests `ok_page::section_lines`, `ok_page::many_waiting_sections`,
   `ok_page::dialog_limit_and_cut`, `ok_page::cut_and_limit_boundaries`): the RED changes `common::ask`
   and the golden file `ok_page_dialog.txt`. Must not change: the terminal form, the grant facts.
4. The guard's refusal and the grant (AC-7, AC-8; tests `page::guard_refuses_without_the_page`,
   `page::guard_refuses_a_stale_page`, `page::grant_binds_the_page`). Must not change: 3a and the STP-25
   tests `ok_page::description_missing_or_twice`, `ok_page::grant_binds_the_description`,
   `ok_page::grant_passes_when_unchanged`.
5. `write-page` in `next` (AC-9; test `next::page_before_confirm`); the RED also changes the old `confirm`
   expectations. Must not change: the order of the other actions.
6. The skill text (AC-10; test `init_process::skill_text_names_the_page`); the GREEN changes
   `crates/stapel-cli/assets/` and runs `init` here. Must not change: the skill sentences other than the
   `write-page` bullet and the rule 5 sentence.

## Review

TODO

## Summary

TODO
