# STP-38 — The records and the measures

## Description

Source: `docs/PLAN.md`, phase 3b, ticket 3b-4 (items 3.9, 3.10); ledger 2026-11-20 (the ten items of the
first cut grouped into four tickets).

### Frame

**Problem.** The orchestrator copies counts, rounds and check runs from `findings.jsonl` and
`runs.jsonl` into the Review section by hand, and writes the merge request text by hand. `stapel tokens
measure` prints no count of returns, and no command counts what each rule caught, so rule 9 is never
applied.

**Plan item:** 3b-4 (3.9, 3.10).

**Risk tags:** `data` (the binary writes `ticket.md`, the person's file), `interface` (the generated block
and its markers, `merge-request.md`, `stapel ledger rules`, the lines of `stapel tokens measure`).

**Non-goals.** Generating the Summary; posting the merge request to a forge; the `catches` records of the
ledger (STP-37); removing a rule (rule 9 stays a decision with the person); a record of stand runs (hand
lines outside the block); the Review of closed tickets; the removal of the empty crates.

**Promise (closed list).**

1. Review block: `stapel review start`, `stapel findings import`, `stapel findings triage` and `stapel
   check` rewrite one generated block in the Review section of `ticket.md`, between two marker lines; it
   lists per stage and round the roles of the findings and the findings by severity after triage and by
   fate, and the newest check run per level with its result, commit and step outcomes. Text outside the
   block is kept byte for byte; a Review section without the markers gets the block at its end; a
   `ticket.md` without a Review section gets no block; `stapel next` ignores the block.
2. Clean tree: an uncommitted change of `ticket.md` that lies only inside the generated block does not
   count as an uncommitted change for the stapel commands, and does not make a confirmation stale.
3. Merge request text: `stapel export` writes `.stapel/tickets/<KEY>/merge-request.md` with the title, the
   Summary section and one line per deferred finding not marked false; it is rewritten on every export.
4. Rule counts: `stapel ledger rules` reads `findings.jsonl` of every ticket and prints, per `rule` value
   and per ticket, the HIGH and MEDIUM findings inside the promise (severity after triage, not marked
   false, newest fate not `no-change`; the findings STP-37 counts); a finding
   without `rule` counts as `none`; a rule that caught nothing in the newest three tickets with findings
   is marked. It writes nothing.
5. Returns: `stapel tokens measure` and the ticket report of `stapel tokens` print the returns to the
   person (permission dialogs, question dialogs and typed messages) with their parts, the interaction
   minutes, and the capped waits, each on its own line.
6. The skill and `stapel init` name items 1, 3 and 4: the block is not edited by hand, `merge-request.md`
   is named at the close, `stapel ledger rules` runs at the measures.

Not promised: a hand edit inside the markers (overwritten); a `ticket.md` with two Review sections or
broken markers (refused, exit 2); a `rule` spelled two ways (two rules).

**Prior art.** STP-24, STP-29, STP-30, STP-35; the hand-written Review sections of STP-35 and STP-36.

**Size.** About 450 source and 600 test lines; 9 Plan steps. Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-22: Decision (Review, the human, recommended): a generated block between markers inside the
  Review section, rewritten by `review start`, `findings import`, `findings triage` and `check`; hand
  lines outside it stay. Rejected: the whole section generated; a separate `review.md`.
- 2026-11-22: Decision (merge request, the human, recommended): `stapel export` writes
  `.stapel/tickets/<KEY>/merge-request.md`, rewritten on each export. Rejected: stdout only; the message
  of the export branch's last commit.
- 2026-11-22: Decision (rule counts, the human, recommended): `stapel ledger rules` reads the `rule` field
  of every ticket's `findings.jsonl`; no dependency on the `catches` format of STP-37. Rejected: the
  `catches` records; both sources.
- 2026-11-22: Decision (returns, the human, recommended): returns are permission dialogs, question dialogs
  (`AskUserQuestion` calls) and typed messages, as in the measure of `docs/PLAN.md`. Rejected: dialogs
  and questions only.
- 2026-11-22: Decision (dependency on STP-37, the human, recommended): steps 1 and 5 to 8 build now;
  steps 2 to 4 and 9 build after main holds STP-37 and is merged into STP-38 (DP-2, DP-3). Rejected:
  waiting for the close of STP-37; finishing STP-37 first.
- 2026-11-22: Decision (agent, step 5): an export from a checkout without the ticket's `state.json`
  writes no `merge-request.md` and prints nothing (the existing export tests run from the base
  checkout). Rejected: a refusal.
- 2026-11-22: Decision (agent, step 5): the Summary body in `merge-request.md` has its leading and
  trailing empty lines trimmed; AC-7 says "as it is" of the text between. Rejected: the raw section.
- 2026-11-22: Decision (agent, step 7): `ledger_rules::bad_lines_and_last` passed before the code of
  step 7, because step 6 already held `--last`, the notice and `rules: no findings`; it is a coverage
  commit (`788453b`).
- 2026-11-22: Decision (agent, steps 7 and 8): two REDs changed earlier tests to the new behaviour:
  the step 6 golden `ledger_rules.txt` (`quiet` on four rows) and
  `attention::report_counts_steps_after_a_merge` (the old `attention:` line).
- 2026-11-23: Decision (agent): the first suite run at `c13ee4b` failed six old tests. Three in
  `depends.rs` and `parallel.rs` showed a defect: `stapel next` took the generated block for a written
  review and skipped `code-review`; AC-14 and Plan step 10 fix it. Three compare `ticket.md` or `git
  status` before and after a block writer (`check::runs_in_a_worktree_and_leaves_the_tree_alone`,
  `closed_pipe::closed_output_keeps_every_write`, `review::copy_keeps_the_index`); they leave the block
  out of the comparison in a test-only commit on main, merged into the branch (no step of STP-38
  touches those files). Rejected: a block that `stapel check` does not write.
- 2026-11-23: Decision (agent, steps 2 and 3): untested and left to the review: the case of no
  `ticket.md` (AC-5), and `stapel check` itself after a block-only change (AC-6 is tested through
  `dirty_paths` and `stapel next`).

## Spec

Terms:

- **Generated block**: the lines of the Review section from the line `<!-- stapel:review:begin -->` to the
  line `<!-- stapel:review:end -->`, both included, each marker alone on its line (trailing spaces and a
  carriage return ignored), and the one empty line right before the begin marker when there is one.
- **Current state** of a finding: `triage::state_of` (newest fate, severity after triage, newest
  verdict). A **false finding** has the newest verdict `false`.
- **Block writers**: `stapel findings import`, `stapel findings triage`, `stapel review start` and a
  `stapel check` run that appends a record to `runs.jsonl`.
- **Counted finding** (rule counts): a finding not false, whose newest fate is not `no-change`, with the
  promise `inside` and the severity after triage HIGH or MEDIUM (the same rule as the `catches` record of
  STP-37).

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN a block writer runs THE block has a table with one row per stage and round of the findings (stages in the order of `records::STAGES`, rounds ascending) giving the roles of its findings, the counts of HIGH, MEDIUM and LOW after triage without false findings, the count of false findings and the newest fates with their counts; the text matches the golden file `review_block.md`. | `review_block::import_and_triage_write_the_block` |
| AC-2 | WHEN `findings.jsonl` holds no finding THE block says `Findings: none`; WHEN `runs.jsonl` holds no check record THE block says `Check runs: none`. | `review_block::import_and_triage_write_the_block` |
| AC-3 | WHEN `stapel check` appends a record THE block has one row per check level (`steps`, `head`, `full`) with the newest record of that level: the result, the first 7 characters of the commit field `head`, `at`, and the step outcomes with their counts sorted by outcome (`cached 7, pass 3`); a record without `level` is of level `head`, one without `steps` has `—`; `--list` and `--history` write no block. | `review_block::check_writes_the_block` |
| AC-4 | WHEN the Review section has a generated block THE writer replaces only its lines; WHEN it has none THE block is inserted after the last non-empty line of the section, after one empty line, and the section's trailing empty lines stay after the block, so removing the block gives the text back; the block's lines take the line end of the begin marker, or else of the first line of `ticket.md`; every other byte of `ticket.md` stays. | `review_block::keeps_text_outside` |
| AC-5 | IF `ticket.md` has two Review sections, a marker without its pair, or two blocks THEN the block writer exits 2 with `stapel: <path>: <reason>` before it writes anything (`review start` before its pack, `stapel check` before any test); IF it has no Review section, or there is no `ticket.md`, THEN the command goes on as today without a block and prints `notice: <path> has no Review section; no generated block`; `stapel check --list` and `--history` do not read the block. | `review_block::broken_markers_refuse` |
| AC-6 | WHEN the only difference of `ticket.md` from HEAD lies in the generated block (the file with its block removed equals the HEAD text with its block removed, both with CRLF read as LF) THE `worktree::dirty_paths` leaves it out, so `stapel check`, `stapel review start`, `stapel next`, the check freshness of `checkstate` and the tree check of `parallel` do not count it; a change outside the block, or a `ticket.md` that `git show HEAD:<path>` cannot read, counts as today. | `review_block::block_only_change_is_clean` |
| AC-7 | WHEN `stapel export` succeeds THE file `.stapel/tickets/<KEY>/merge-request.md` holds `# <KEY> — <title>`, the Summary section body as it is, and `## Deferred findings` with one line `- <id> <severity> <stage> round <round>: <claim>` per finding not false whose newest fate is `defer`, or `none`; it is rewritten on every export and the command prints `merge request: <path>`; `merge-request.md` is a machine file (not an uncommitted change, the guard denies an agent write); the orchestrator commits it with the close commit (AC-12); from a checkout without the ticket's `state.json` the export writes no file and prints no line. | `export::writes_the_merge_request` |
| AC-8 | WHEN `stapel ledger rules` runs THE output is a table: a row per `rule` value of any finding, counted or not (trimmed; absent or empty is `none`), sorted by name, a column per ticket with at least one finding (the newest five, ordered by the number of the key, then by its prefix; or `--last <N>`) and a column `total` over every ticket, each cell the number of counted findings; the text matches the golden file `ledger_rules.txt`. It writes nothing. | `ledger_rules::counts_per_rule_and_ticket` |
| AC-9 | WHEN a rule other than `none` has no counted finding in any of the newest three ticket columns THE row ends with `quiet`; with fewer than three columns (`--last 1` or `2` included) no row is marked. | `ledger_rules::marks_quiet_rules` |
| AC-10 | IF a line of a `findings.jsonl` does not parse THEN it is skipped and one line `notice: <n> lines of findings.jsonl skipped` goes to stderr; IF `--last` is not a whole number of 1 or more THEN the command exits 2; WHEN no ticket has a finding THE output is `rules: no findings`. | `ledger_rules::bad_lines_and_last` |
| AC-11 | WHEN `stapel tokens measure` reads a transcript, or `stapel tokens KEY` reports a ticket with attention records, THE line `attention:` is replaced by three lines: `returns: <n> (<d> dialogs, <q> question dialogs, <t> messages)`, `interaction: <capped attention>` and `waits: <waited> in all, <k> over 10 min`, where a question dialog is one `AskUserQuestion` call; attention records get the field `question_dialogs`, and an older record without it counts its `questions`; the text matches the golden files `tokens_measure.txt` and `tokens_attention.txt`. | `attention::measure_prints_tokens_and_attention`, `attention::report_shows_the_attention_lines` |
| AC-12 | WHEN `stapel init` writes the skill THE skill holds the phrases `stapel writes the generated block of the Review section`, `name the path of merge-request.md at the close and commit it` and `run stapel ledger rules`. | `init_process::names_the_records` |
| AC-13 | WHEN a block writer writes the block THE new `ticket.md` is written to a temporary file in its folder and renamed over it, after the writer's own records; `review start`, which has no record of its own, writes the block from the records there are. | `review_block::review_start_writes_the_block` |
| AC-14 | WHEN `stapel next` judges whether the Review section is written THE text of the section without the generated block is judged, so a section of `TODO` and a block names `code-review` as before the block. | `review_block::next_ignores_the_block` |

### Guarantees

- **Promised:** item 1 by AC-1 to AC-5, AC-13 and AC-14, item 2 by AC-6 (the Review section is `generated` and
  is never confirmed, so no confirmation can go stale), item 3 by AC-7, item 4 by AC-8 to AC-10, item 5
  by AC-11, item 6 by AC-12. Unchanged: the records each writer appends; the export branch; `stapel
  ledger add`.
- **Not promised:** the Frame's list; two block writers at the same time (each writes the block from every
  record, the last rename wins); a Summary still `TODO` (written as it is); tickets that live only in
  other worktrees (`ledger rules` reads the tickets of the current tree); a symlinked `ticket.md` (the
  rename replaces the link with a file); a temporary file left by a crash.

## Test plan

Integration tests in `crates/stapel-cli/tests/`: the new files `review_block.rs` and `ledger_rules.rs`,
and `export.rs`, `attention.rs`, `init_process.rs`; golden files under `crates/stapel-cli/tests/golden/`.
Light tests: `stapel cargo test -j 4 -p stapel-cli --test <file> -- --test-threads=4`; before each GREEN
also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Block from findings | import of findings of two stages and two rounds (a LOW, a HIGH regraded `low`, a finding with the verdict `false`, three roles), then a triage: the golden block after each; no finding and no run: both `none` lines | `review_block::import_and_triage_write_the_block` |
| Next and the block | built steps and a passing check; a Review section of `TODO` and a generated block: `stapel next` names `code-review`; the same with one hand line outside the block: `write-summary` | `review_block::next_ignores_the_block` |
| Block from review start | a review round 1 on a ticket with spec-review findings: the block is written; no temporary file is left in the ticket folder | `review_block::review_start_writes_the_block` |
| Block from check | a fake-cargo check at level `steps`, then `head`: two rows, the newest per level; an older record without `level` and without `steps`; `--list`, `--history`: `ticket.md` unchanged | `review_block::check_writes_the_block` |
| Outside text | hand lines before and after an existing block; a Review section without a block followed by `## Summary`; a CRLF file with an existing block: replaced, CRLF kept | `review_block::keeps_text_outside` |
| Broken markers | two Review sections; a begin marker alone; an end marker alone; two blocks; for each, exit 2 and `findings.jsonl` unchanged; `review start` writes no pack; `stapel check` refuses before a run (no new record); no Review section, and no `ticket.md`: the import goes on, the notice, `ticket.md` unchanged | `review_block::broken_markers_refuse` |
| Clean tree | HEAD without a block, its Review section ending in two empty lines before `## Summary`: the round trip; a CRLF `ticket.md`; after an import `stapel check` runs and `stapel next` names no uncommitted change with `ticket.md` uncommitted; a hand change of the Spec makes it refuse; an untracked `merge-request.md` does not | `review_block::block_only_change_is_clean` |
| Merge request | an export with a Summary and two deferred findings (one false: left out, one regraded); with none: `none`; a second export rewrites; the guard denies a Write of `merge-request.md` | `export::writes_the_merge_request` |
| Rule counts | three tickets: rules `none` (absent, empty and a number), `rule 3`, ` rule 3 ` (trimmed, one rule); LOW, outside, false and `no-change` findings not counted; a HIGH regraded `low` not counted; a LOW regraded `high` counted; seven tickets: five columns and `total` | `ledger_rules::counts_per_rule_and_ticket` |
| Quiet rules | a rule counted only in the oldest of four tickets: `quiet`; a rule with only LOW findings: a row of zeros, `quiet`; a rule counted in one of the newest three: no mark; `none` with no count: no mark; two tickets: no mark; `--last 2`: no mark | `ledger_rules::marks_quiet_rules` |
| Bad input | a broken line in one `findings.jsonl`; `--last 0`, `--last x`; no ticket with findings | `ledger_rules::bad_lines_and_last` |
| Returns | the golden transcript: two dialogs, one `AskUserQuestion` call with two questions, one message: `returns: 4 (2 dialogs, 1 question dialogs, 1 messages)` | `attention::measure_prints_tokens_and_attention` |
| Returns in the report | the existing report test and its golden `tokens_attention.txt`, with one attention record with `question_dialogs` and one older record without it: the three lines with the summed counts | `attention::report_shows_the_attention_lines` |
| Init | the skill that `stapel init` writes | `init_process::names_the_records` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| Review section of `ticket.md` | Markdown text | no block / one block | no block: added; no section: notice, no block | two sections, unpaired or two blocks: exit 2 | markers matched as whole lines, trailing spaces and a carriage return ignored | `review_block::broken_markers_refuse`, `review_block::keeps_text_outside` |
| `findings.jsonl` | JSON lines | none / the journal limit | none: `Findings: none` | a broken line: the writers refuse as `records` does today; `ledger rules` skips it with a notice | severity and fate after triage | `review_block::import_and_triage_write_the_block`, `ledger_rules::bad_lines_and_last` |
| `runs.jsonl` | JSON lines | none / the journal limit | none: `Check runs: none` | a line without `level`: counted as `head`, the default level of older records | `head` cut to 7 characters | `review_block::check_writes_the_block` |
| `--last` | whole number | 1 / u32 | absent: 5 | 0, negative, not a number: exit 2 | exact | `ledger_rules::bad_lines_and_last` |
| `rule` field | text | absent / any length | absent or empty: `none` | not text: `none` | trimmed, case kept | `ledger_rules::counts_per_rule_and_ticket` |
| `question_dialogs` of an attention record | whole number | 0 / u64 | absent: the record's `questions` | not a number: as absent | exact | `attention::report_shows_the_attention_lines` |
| Summary section | Markdown text | any | `TODO` or empty: written as it is | — | byte for byte | `export::writes_the_merge_request` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | `ticket.md` not in HEAD (a new ticket never committed) | the clean-tree rule of AC-6 does not apply; the file is an uncommitted change as today | `review_block::block_only_change_is_clean` |
| git | other changes beside the block | refused as today | `review_block::block_only_change_is_clean` |
| git | `git show HEAD:<path>` fails (no commit, unborn branch) | the file counts as changed, as today | `review_block::block_only_change_is_clean` |
| git | history shape, worktrees, submodules, object format, shallow clones | unchanged by this ticket; `git show` reads the blob of HEAD in any of them | — |
| os | rename of a file in the same folder | the block is written to a temporary file beside `ticket.md` and renamed | `review_block::review_start_writes_the_block` |
| os | a symlinked `ticket.md`; a temporary file left by a crash | not promised | — |
| os | two writers at once | not promised | — |
| claude-code | transcript shape | read as in STP-29; question dialogs are the `Question` events | `attention::measure_prints_tokens_and_attention` |
| cargo | every dimension | not read by this ticket beyond `stapel check` as today | — |

**Review Focus:** a `ticket.md` whose Review section holds a fenced block with a marker line (the parser
skips fences; not tested); the block in a ticket of the base branch merged into a ticket branch (the
markers merge as text; not tested); a `rule` with a `|` (written `\|` in the table; not tested); very
many rules (no limit); `git show` on a path with spaces (passed as one argument; not tested).

**Author self-check (item 7).** Done on 2026-11-22: each writer of `ticket.md` refuses before its own
write when the block cannot be placed, so no command half-writes; the clean-tree rule compares with the
HEAD text, so a hand edit inside the block plus a Spec edit is still refused.

## Design

### Facts

- F-1: `findings.rs:23` `import` and `:39` `triage` append to `findings.jsonl` with
  `journal::append_all`; `triage::state_of` (core `triage.rs:37`) gives the newest fate, the severity
  after triage and the newest verdict.
- F-2: `review.rs:39` `start` refuses a dirty tree (`:66`) and writes the pack under
  `.stapel/scratch/review/<KEY>-r<N>/`; it knows the reviewer roles and models and stores neither.
- F-3: `check.rs` builds the run record at `:592` and appends it at `:636`; the dirty refusal is at
  `:350` through `worktree::dirty_paths` (core `worktree.rs:34`), which drops the machine files of
  `MACHINE_FILES` (`:9`) by path only.
- F-4: `ticket::parse` (core `ticket.rs:79`) splits at `## ` outside fences and HTML comments; sections
  with the owner `generated` (Review, Summary) are never confirmed (`confirm.rs:35`, `stage.rs`).
- F-5: only `new.rs:96` writes `ticket.md`, with `std::fs::write`; `state.rs:145` writes through a
  temporary file and a rename.
- F-6: `export.rs:21` `run` ends with `Ok(())` at `:64`; `is_process_path` (`:197`) leaves `.stapel/` out
  of the export branch.
- F-7: `ledger.rs` has only `add`; `main.rs:256` `LedgerAction`; `tokens.rs:858` `ticket_folders`.
- F-8: `tokens.rs:842` prints the `attention:` line from `attention::Sums`; one `Question(n)` event is one
  `AskUserQuestion` call; the guard's `MACHINE_FILES` (`guard.rs:148`) is a second list.

### Decisions (before the build)

- A new core module `review_block.rs`: `render(findings, runs) -> String` and `place(ticket_text, block)
  -> Result<String, String>` (AC-4, AC-5), and `strip(text)` for AC-6. The cli calls one function
  `review_block::write(ticket)` after the append of each writer, and `review_block::can_place` before it.
- `worktree::dirty_paths` takes the AC-6 rule: a modified `ticket.md` whose stripped text equals the
  stripped `git show HEAD:<path>` is dropped. Its five callers get it: `check.rs:350`, `review.rs:67`,
  `next.rs:156`, `checkstate.rs:92`, `parallel.rs:418`.
- The aggregate report (`tokens.rs:1006`) and `measure` print the three lines from one function; the
  attention record (`tokens.rs:1366`) gets `question_dialogs`.
- 2026-11-22, agent (SR1-4): a `ticket.md` without a Review section gets no block and a notice, so old
  tickets and the fixtures of existing tests keep working. Rejected: adding the section; a refusal.
- 2026-11-22, agent (SR1-6): the rule counts take inside findings only, as rule 7 and rule 9 count.
- 2026-11-22, agent (SR1-10): `merge-request.md` is committed with the ticket folder, as `runs.jsonl`.
- `merge-request.md` joins both machine-file lists (`worktree.rs`, `guard.rs`).
- `stapel ledger rules [--last N]` in `ledger.rs`, reading `ticket_folders` (moved to a shared place) and
  `triage::state_of`.
- `Sums` gets `question_dialogs`.

### Decisions (as built)

- Build order 1, 8, 5, 6, 7, then, after main with STP-37 was merged in at `ffaf444` (no conflicts), 2,
  3, 4, 9. `stapel check` does not enforce the Plan order. STP-37 changed four times during the build;
  each change needed a new `stapel depends` round (four rounds, no dependency for the steps built).
- Step 1: core `review_block::{render, place, can_place, write}`; `write` goes through a temporary file
  and a rename and returns `Ok(false)` with no Review section or no `ticket.md`; `records::STAGES` is
  public. The block begins `Generated by stapel; edits between the markers are overwritten.`, then
  `Findings:` and the table `| Stage | Round | Roles | HIGH | MEDIUM | LOW | False | Fates |`; roles and
  fates sorted, fates as `<fate> <n>`. `Findings: none` is tested by a direct `render(&[], &[])` call,
  since no command imports zero findings.
- Step 2: `review_block::guard(dir)` (writes nothing) and cli `findings::guard_block`; import and
  triage call it before the append; `review start` after the round refusals of STP-37, before the pack;
  `stapel check` after the `--list` and `--history` returns, before the `[check]` load, the dirty tree,
  the lock and the Node pin of STP-37.
- Step 3: `review_block::without_block` (CRLF read as LF; the block and the empty line before it
  removed; `None` on broken markers); `dirty_paths` drops a block-only `ticket.md` against
  `git show HEAD:<path>`; `review start` writes the block after the pack.
- Step 4: the check table `| Level | Result | Commit | At | Steps |`, a row only for a level that has a
  record, in the order `steps`, `head`, `full`; `stapel check` writes the block after the append to
  `runs.jsonl`.
- A block write after the writer's own records fails only with a warning (import, triage, review start,
  check); the placement is refused before any write by `guard`.
- Step 5: `export.rs::write_merge_request`, temporary file and rename; the title comes from the
  worktree's `state.json`; `merge-request.md` is in both machine-file lists.
- Step 6: `ledger.rs::{rules, key_order}`, `LedgerAction::Rules { last }`; `tokens::ticket_folders` is
  `pub(crate)`. Rows sort by byte order (`Rule 3` before `none`); the rule column is left-aligned, the
  counts right-aligned, two spaces apart.
- Step 7: `quiet` is an unpadded last cell; the RED added it to four rows of the step 6 golden file.
- Step 8: `Sums::question_dialogs`; `tokens.rs::returns_lines` serves `measure` and the ticket report;
  the attention record gets `question_dialogs`.
- Step 9: three phrases in `assets/skill.md`, installed by this branch's `stapel init`.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `review_block::import_and_triage_write_the_block` |
| AC-2 | `review_block::import_and_triage_write_the_block` |
| AC-3 | `review_block::check_writes_the_block` |
| AC-4 | `review_block::keeps_text_outside` |
| AC-5 | `review_block::broken_markers_refuse` |
| AC-6 | `review_block::block_only_change_is_clean` |
| AC-7 | `export::writes_the_merge_request` |
| AC-8 | `ledger_rules::counts_per_rule_and_ticket` |
| AC-9 | `ledger_rules::marks_quiet_rules` |
| AC-10 | `ledger_rules::bad_lines_and_last` |
| AC-11 | `attention::measure_prints_tokens_and_attention`, `attention::report_shows_the_attention_lines` |
| AC-12 | `init_process::names_the_records` |
| AC-13 | `review_block::review_start_writes_the_block` |
| AC-14 | `review_block::next_ignores_the_block` |

## Plan

Route: full, auto (risk tags `data`, `interface`).

1. The block from findings (AC-1, AC-2, AC-4; tests `review_block::import_and_triage_write_the_block`,
   `review_block::keeps_text_outside`): core `review_block.rs` render and place; `findings import` and
   `findings triage` write it; golden `review_block.md`.
2. Broken markers (AC-5; test `review_block::broken_markers_refuse`): `can_place` before the append of
   import and triage, before `review start` writes its pack, and before `stapel check` runs; the notice.
3. Review start and the clean tree (AC-13, AC-6; tests `review_block::review_start_writes_the_block`,
   `review_block::block_only_change_is_clean`): `review start` writes the block; `dirty_paths` drops a
   block-only change of `ticket.md`.
4. Check runs in the block (AC-3; test `review_block::check_writes_the_block`): rows per level after the
   append of `runs.jsonl`.
5. The merge request text (AC-7; test `export::writes_the_merge_request`): `export` writes
   `merge-request.md`; both machine-file lists name it.
6. Rule counts (AC-8; test `ledger_rules::counts_per_rule_and_ticket`): `stapel ledger rules` and its
   golden file `ledger_rules.txt`.
7. Quiet rules and bad input (AC-9, AC-10; tests `ledger_rules::marks_quiet_rules`,
   `ledger_rules::bad_lines_and_last`): the `quiet` mark, `--last`, the notice.
8. Returns (AC-11; tests `attention::measure_prints_tokens_and_attention`,
   `attention::report_shows_the_attention_lines`): `Sums::question_dialogs`, the record field and the three lines
   in both outputs; both golden files change in the RED.
9. The skill (AC-12; test `init_process::names_the_records`): `assets/skill.md`, then `stapel init`.
10. Next without the block (AC-14; test `review_block::next_ignores_the_block`): `next` judges whether
    the Review section is written on its text without the generated block.
11. Lone carriage return (XR1-1; test `review_block::lone_carriage_return_refuses`): a `ticket.md` with a
    carriage return not followed by a line feed refuses with exit 2 before any write.
12. Fenced markers (XR1-2, R1-2; test `review_block::fenced_markers_are_text`): marker lines inside a
    fence or an HTML comment are text; locating the block uses the fence state of `ticket::scan`.
13. `stapel new` (XR1-3; test `new_branch::block_and_merge_request_are_not_changes`): its dirty check
    leaves out `merge-request.md` and a block-only `ticket.md`, through `worktree.rs`.
14. Coverage (XR1-4, XR1-5, R1-5, R1-6; tests `review_block::check_after_a_block_only_change`,
    `review_block::no_ticket_md`, `export::without_state_json_no_merge_request`,
    `ledger_rules::orders_keys_by_number`): tests of built behaviour, each in a coverage commit.

| Fix step | Tests |
|---|---|
| 11 | `review_block::lone_carriage_return_refuses` |
| 12 | `review_block::fenced_markers_are_text` |
| 13 | `new_branch::block_and_merge_request_are_not_changes` |
| 14 | `review_block::check_after_a_block_only_change`, `review_block::no_ticket_md`, `export::without_state_json_no_merge_request`, `ledger_rules::orders_keys_by_number` |
| 15 | `review_block::unclosed_fence_or_comment_refuses` |
| 16 | `hook::denies_merge_request_writes`, `review_block::block_of_another_ticket_keeps_the_check` |

15. Unclosed fence or comment (R2-2, XR2-1; test `review_block::unclosed_fence_or_comment_refuses`): a
    Review section that ends inside an unclosed fence or HTML comment refuses with exit 2 before any write.
16. Coverage (R2-3, XR2-2; tests `hook::denies_merge_request_writes`,
    `review_block::block_of_another_ticket_keeps_the_check`): tests of built behaviour in a coverage commit.

## Review

TODO

## Summary

TODO
