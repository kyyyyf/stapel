# STP-20 — stapel next: the next action from the facts

## Description

Source: `docs/PHASES.md`, plan item 1.1 (`docs/decisions/0004`); the next-session note
`.stapel/scratch/next/next-session.md`.

### Frame

**Problem.** The orchestrator finds the next action of a ticket by re-reading `CLAUDE.md`, the 50 KB ticket,
`stapel status`, `stapel check --history` and the journals on every turn; the ledger of 2026-10-08 names the
orchestrator's context as the main cost. Every fact needed is already on disk or in git. `stapel next [KEY]`
computes the action from them, so the skill of item 1.3 can be one rule: run `stapel next`, do what it says,
run it again.

**Plan item:** 1.1. **Risk tags:** `interface` (a new command; its `--json` output is what the skill reads).

**Non-goals.** Writing any file or record (the review record is item 1.5); the builder brief or its path (item
1.2); the ticket lint (item 1.6); findings fates and triage (item 1.5: today no command sets a fate after the
import, so `next` reads no fate); the drift review after a GREEN step; choosing reviewers by tags; reading
`docs/PHASES.md`; running cargo.

**Promise (closed list).**

1. `stapel next [KEY] [--json]` resolves the ticket as `stapel status` does; with no `KEY` and no open
   ticket it names the action `new-ticket` with `stapel new "<title>"`. It writes nothing.
2. It reads only: `state.json` (closed, confirmations), the sections of `ticket.md`, the `Route:` line and
   the numbered step items of the Plan, the first-parent history of `HEAD`, `base` bounding the build-input range as in `check` (step commits and the
   history analysis of `stapel check --history`, without cargo), the newest check record of `runs.jsonl`,
   and the `stage` of the records in `findings.jsonl`.
3. It prints the blocking facts first (each confirmation missing or stale, a Plan with no step or a step
   named twice, a step in history that the Plan does not name, each history fault, a failing current
   check), then exactly one action, the first that applies of: `closed`; `write-frame` (Description
   is empty, `TODO` or has no `### Frame`); `write-section` (the first configured, not `generated` section
   that is empty or `TODO`); `spec-review` (route not `light`, no confirmation ever, no `spec-review`
   finding); `confirm` (`stapel ok KEY`); `fix-history` (the step, its outcome and commit); `fix-check`;
   `green-step` or `build-step` (the first Plan step not done, in Plan order); `commit` (the tree has
   changes outside the machine files); `run-check` (`stapel check
   KEY`, when `[check]` is set and the check is not a pass at HEAD); `code-review` (Review is `TODO`);
   `write-summary`; `close` (`stapel close KEY`, by the person).
4. A Plan step `N.` or `Nx.` (a letter suffix) has the label `step N` or `step Nx`. It is done when
   history has its GREEN, or a commit `<KEY> step <label> <word>: …` where the word is not `RED`
   (coverage, cleanup); with a RED and no GREEN the action is `green-step`.
5. `--json` prints one JSON object with the same action, its command or path and the facts.

Not promised: a Plan written in another form; review rounds
after the first (no fact tells them apart until item 1.5); a review that found nothing and left the Review
section `TODO`; deliberate circumvention.

**Prior art.** `stapel status` (`crates/stapel-cli/src/status.rs`: `waiting_for`, `freshness`,
`checkstate::current`); `stapel check --history` (`crates/stapel-cli/src/check.rs`, `outcomes::analyse`,
`history_outcome`); `steps::parse_subject`; `tickets::resolve`. klc has no next-action command.

**Size.** About 450 changed source lines (`crates/stapel-core`: a `next` module with the rules and the Plan
parse; `crates/stapel-cli`: the command, `main.rs`) and about 1 500 test lines, from STP-19's actual 524 and
2 000 against its estimate of 350. Past about 650 source lines the ticket is split before the spec review.
Route: full (tag `interface`), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-19: Decision (review facts, not recommended): a record "review round N done" is wanted, but it
  comes with item 1.5 (`stapel review start`); in this ticket `next` reads indirect facts: `spec-review`
  findings or an earlier confirmation for the spec review, the Review section for the code review. Rejected:
  widening item 1.1 by a ledger decision; moving item 1.5 forward.
- 2026-10-19: Decision (route and drift, recommended): `next` reads the Plan's `Route:` line and names no
  spec review on `light`; the drift review after a step stays the orchestrator's rule, since a clean drift
  review leaves no fact.
- 2026-10-19: Decision (no ticket, recommended): with no open ticket `next` names `stapel new "<title>"` and
  does not read `docs/PHASES.md`, which a foreign repository does not have.
- 2026-10-19: Decision (history, recommended): `next` runs the history analysis of `check --history`
  itself: git only, no cargo, no record in `runs.jsonl`.
- 2026-10-19: Decision (output, recommended): text lines `ticket:`, `blocked:` per fact, one `next:`; exit
  0 whenever an action is named, blocked or not; exit 1 when the ticket or the history cannot be read; exit
  2 for a wrong call. Rejected: exit 3 when blocked.
- 2026-10-19: Decision (unreadable history, recommended): the actions before the build (frame, sections,
  spec review, confirm) need no git; only when no such action applies does `next` read the history, and
  then a shallow clone or a `base` off the first-parent history exits 1 with the refusal of `check`.
  Rejected: refuse at once, as `check` does.
- 2026-10-19: Decision (Plan structure, recommended): a Plan with no step or with a step named twice is a
  blocking fact, and the action is `write-section` for the Plan. Rejected: leave it to the lint (item 1.6).
- 2026-10-19: Decision (step outside the Plan, recommended): a step commit whose label the Plan does not
  name is a blocking fact, and the action is `write-section` for the Plan; this moves the case from the
  Frame's "not promised" list into the promise. Rejected: ignore it.
- 2026-10-19: Assumption: findings fates stay out (no command sets a fate after the import, so a HIGH
  finding without a fate could block `next` forever); item 1.5 adds them to `next`.

## Spec

Terms:

- **Next command**: `stapel next [KEY] [--json]`.
- **Ticket choice**: `tickets::resolve`, as `stapel status`: an explicit `KEY` names any open or closed
  ticket in any case; without `KEY`, the only open ticket.
- **Written**: a `## <title>` section of `ticket.md` found once (`ticket::find`) whose body, trimmed, is
  neither empty nor `TODO`.
- **Confirmed section**: a section of `stapel.toml` whose owner is not `generated`, in the configured order.
- **Plan step**: a line of the Plan section that starts at its first column with one or more digits, at
  most one lowercase ASCII letter, `.` and a space; its label is `step ` followed by the digits and the
  letter (`1c.` gives `step 1c`).
- **Route**: `light` when the first Plan line that starts with `Route:` has, as its first word after the colon
  (split at whitespace, trailing `,`, `.`, `;` and `:` removed), `light` in any case; else `full`, also with
  no such line (`light-ish` and `light!` are `full`).
- **History**: the step commits of the ticket on the first-parent history of `HEAD` and their history
  outcomes, as `stapel check --history` reads them (`steps::step_commits`, `check::history_report`); a
  closed ticket is read up to its range end, as `check` does.
- **Done step**: a Plan step whose label has a GREEN commit in history, or a commit on the first-parent
  history of `HEAD` whose subject starts with `<KEY> <label> ` followed by a word other than `RED:` or
  `GREEN:` that ends with `:` (`STP-19 step 3c coverage: …`).
- **Open RED**: a Plan step whose only step commit is a RED that is the newest step commit of the ticket,
  as the commit hook reads it (`check::hook_faults`); history gives it `unpaired`, which is not a fault here.
- **Fact line**: `blocked: <fact>`. **Action line**: `next: <action>` followed by ` <subject>` when the
  action has one and by ` — <target>` when it has a command or a path.
- **Ticket path**: `.stapel/tickets/<KEY>/ticket.md`, with the folder key.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the next command runs on an open ticket THE command prints `ticket: <KEY> — <title>`, then one fact line per blocking fact of AC-7 and AC-9, and of AC-11 to AC-13 and AC-20 when the history is read (AC-10), then one action line: the first that applies of AC-5 to AC-15, AC-20, AC-16 to AC-19, in that order; exit 0; it writes no file and no git object. Output: `golden/next_text.txt`. | `next::prints_facts_then_one_action`, `next::writes_nothing` |
| AC-2 | WHEN the next command runs with `--json` THE command prints one line, a JSON object with the keys `v` (1), `ticket` (the key or `null`), `action`, `subject`, `command`, `path` (each a string or `null`) and `facts` (an array of strings: the fact texts without `blocked: `), in this order, and nothing else; exit as AC-1. Output: `golden/next_json.txt`. | `next::json_has_the_same_action` |
| AC-3 | WHEN the next command runs without `KEY` and no ticket is open THE command prints `next: new-ticket — stapel new "<title>"` (exit 0); with `--json`, `ticket` is `null` and `command` is `stapel new "<title>"`. | `next::no_open_ticket_names_new` |
| AC-4 | IF `KEY` names no ticket folder or several folders that differ only in case, or no `KEY` is given and several tickets are open, THEN the command prints `stapel: <reason of the ticket choice>` on stderr and exits 2; IF the ticket is legacy, its `state.json` or `ticket.md` cannot be read, or there is no repository or `.stapel/stapel.toml` cannot be read, THEN it prints `stapel: <reason>` and exits 1; stdout is empty in both cases. | `next::refuses_a_wrong_call`, `next::refuses_an_unreadable_ticket` |
| AC-5 | WHEN the ticket is closed THE action line is `next: closed` and there is no fact line. | `next::closed_ticket` |
| AC-6 | WHEN the Description section is not written or has no line `### Frame` THE action line is `next: write-frame — <ticket path>`. | `next::frame_first` |
| AC-7 | WHEN a confirmed section is not written, is missing or is found twice THE action line is `next: write-section <title> — <ticket path>` for the first such section in the configured order; a found-twice section adds the fact `<title> is duplicated`. A Plan with no Plan step adds the fact `the Plan has no steps`, one with a label twice adds `the Plan names <label> twice`, and either counts as the Plan not written. | `next::sections_in_order`, `next::plan_structure`, `next::plan_labels_with_letters`, `next::generated_sections_wait` |
| AC-8 | WHEN every confirmed section is written, the route is not `light`, `state.json` holds no confirmation of any section, and no record of `findings.jsonl` has `stage` `spec-review` THE action line is `next: spec-review — <ticket path>`; a review with no finding is followed by `stapel ok <KEY>`, whose confirmation ends this rule; on a `light` route the rule does not apply. | `next::spec_review_until_a_fact`, `next::light_route_skips_spec_review`, `next::route_word_edges` |
| AC-9 | WHEN a confirmed section has no confirmation or a stale one THE command adds the fact `<id> not confirmed`, or `<id> confirmation is stale: <reason>` (`Reason::describe`) once per stale reason, in the configured order, and the action line is `next: confirm — stapel ok <KEY>`. | `next::confirm_names_each_section`, `next::stale_reasons_each_a_fact`, `next::folder_key_in_outputs` |
| AC-10 | IF no rule of AC-5 to AC-9 applies and the history cannot be read (a shallow clone, a `base` off the first-parent history of `HEAD`, a git error) THEN the command prints `stapel: <the refusal of stapel check>` and exits 1; while one of those rules applies, the command does not read the history, so neither stops it. | `next::history_read_only_when_needed` |
| AC-11 | WHEN a step commit of the ticket has a label that no Plan step has THE command adds the fact `history has <label>, the Plan does not` for each such label in history order, and the action line is `next: write-section Plan — <ticket path>`. | `next::step_outside_the_plan` |
| AC-12 | WHEN a step has a history outcome that is not passing and not `unverified`, or is an open RED that `outcomes::open_red` gives `no-tests` or `red-changes-code` (fault commit: the RED), THE command adds the fact `<label>: <outcome> at <short sha> — <reason>` per step in history order and the action line `next: fix-history <label> — stapel check --history <KEY>` for the first; an open RED's own `unpaired` is no fault, and `unverified` is left to AC-16. | `next::history_faults_block_the_build`, `next::open_red_is_judged_alone` |
| AC-13 | WHEN `[check]` is set and the newest check record is a current `fail` (`check: fail at <sha>` in `stapel status`) THE command adds the fact `check fails at <short sha>` and the action line is `next: fix-check — .stapel/tickets/<KEY>/runs.jsonl`. | `next::failing_check_blocks_the_build` |
| AC-14 | WHEN the first Plan step in Plan order that is not done is an open RED THE action line is `next: green-step <label> — <ticket path>`; with no step commit, `next: build-step <label> — <ticket path>`. | `next::first_open_step` |
| AC-15 | WHEN a Plan step has no GREEN but a commit `<KEY> <label> <word>: …` with another word (coverage, cleanup) THE step is done and AC-14 moves to the next Plan step. | `next::coverage_and_cleanup_steps_are_done` |
| AC-16 | WHEN every Plan step is done, `[check]` is set and the check line of `stapel status` is not `check: pass at <sha>` THE action line is `next: run-check — stapel check <KEY>`; without `[check]` this rule does not apply. | `next::run_check_after_the_steps` |
| AC-17 | WHEN the rules above do not apply and the section of id `review` is configured and not written THE action line is `next: code-review — <ticket path>`. | `next::code_review_then_summary` |
| AC-18 | WHEN the rules above do not apply and the section of id `summary` is configured and not written THE action line is `next: write-summary — <ticket path>`. | `next::code_review_then_summary` |
| AC-19 | WHEN no rule of AC-5 to AC-18 and AC-20 applies THE action line is `next: close — stapel close <KEY>`. | `next::close_at_the_end` |
| AC-20 | WHEN every Plan step is done and the working tree has changes outside the machine files (`worktree::dirty_paths`, which `stapel check` refuses) THE command adds the fact `uncommitted changes: <at most five paths>` and the action line is `next: commit — git status`; this rule comes before AC-16. | `next::uncommitted_changes_before_the_check` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-20; a fault fact of AC-12 goes when the history is
  rewritten (amend or rebase), or, for `unpaired` without a GREEN, when that GREEN is committed; `stapel status`, `stapel check` and `stapel check
  --history` keep their output and their tests.
- **Not promised:** the Frame's list; without `KEY`, a ticket whose `state.json` cannot be read counts as
  not open, as `tickets::resolve` reads it, and so does a legacy one (D1-1); a Description found twice gives
  `write-frame` with no fact; a `### Frame` line inside a code fence counts (D1-7); a subject `<KEY> step 2 coverage RED: …` is a step commit with
  the label `step 2 coverage` (`parse_subject`), so AC-11 names it; a Plan step written inside a list item or a code block (any line
  that starts at the first column counts); a label in history that differs from a Plan label only in case
  (they differ); `findings.jsonl` lines that are not JSON (skipped, as `status` skips them); the time a long
  history takes (item 1.11); a ticket that changes while the command runs.

## Design

### Facts (external contract and states)

- F-1: `tickets::resolve` gives the texts `no ticket <KEY> in .stapel/tickets`, `several open tickets, name
  one: …` and `no open ticket; name one, e.g. stapel status <KEY>`; `stapel status` maps its errors to exit 1
  through `main`, `stapel check` to exit 2 through `check::refuse`.
- F-2: `stage::waiting_for`, `stage::latest` and `stage::freshness` give the confirmed sections, their latest
  confirmation and its stale reasons; `Reason::describe` gives the texts `stapel status` prints after
  `stale: <id> — `.
- F-3: `check::run` refuses, in order: ticket choice, legacy or unreadable state, no `HEAD`,
  `shallow_refusal`, `base_of` (`build_inputs::check_base`), `range_end` for a closed ticket. `history_report`
  gives per step an `Analysis` and an outcome (`None` for `needs-run`); `outcomes::passing` is true for
  `pass` and `retired`.
- F-4: `steps::step_commits` walks the whole first-parent history of `HEAD` and keeps the subjects
  `<KEY> <label> RED: …` and `<KEY> <label> GREEN: …` (`parse_subject`); a coverage or cleanup commit is not
  a step commit today (`STP-19 step 3c coverage: …`, `STP-19 step 4 cleanup: …`, read 2026-10-19).
- F-5: `checkstate::current` gives `CheckLine::{NotConfigured, None, Pass, Fail, Stale}` from the newest
  well-formed check record and the working tree; it runs git.
- F-6: `findings.jsonl` records carry `stage` (`spec-review`, `step-drift`, `code-review` in STP-19, read
  2026-10-19); `journal::read` gives `V1` entries for well-formed lines.
- F-7: the Plan of STP-19 has the steps `1.`, `1c.`, `2.`, `2c.`, `3.`, `3c.`, `3d.`, `4.`, `4c.` and the line
  `Route: full (tags …), pinned at the frame.`

### Decisions (before the build)

- A module `crates/stapel-cli/src/next.rs` holds the command; the rules are a pure function in
  `stapel_core::next` (`decide(&Facts) -> Decision`) over facts gathered by the CLI, so most cases are tested
  without git through the binary on small fixtures. The history part reuses `check::history_report` and the
  refusals of `check.rs` (F-3), which become `pub(crate)` where needed.
- History is gathered lazily: the CLI first evaluates AC-5 to AC-9 from `state.json`, `ticket.md`,
  `stapel.toml` and `findings.jsonl`; only when none applies it reads `HEAD`, the history and the check line.
- The commits for AC-15 come from one `git log --first-parent --format=%s` over `HEAD` (or the range end of
  a closed ticket, which AC-5 already stops).
- After the steps, each commit of `ticket.md` (Review, Summary) makes the check record stale, so
  `run-check` comes again before `close`; accepted (SR2-6): `stapel close` needs a current check anyway.
- The JSON line is built with `serde_json::json!` under `preserve_order` (as `tokens add` does), so the key
  order of AC-2 holds.

## Test plan

CLI tests in a new file `crates/stapel-cli/tests/next.rs`, with `common::repo_with_ticket`,
`common::set_section` and git commits made by the test. Golden files under `crates/stapel-cli/tests/golden/`.
Light tests: `cargo test -j 4 -p stapel-cli --test next -- --test-threads=3`; before GREEN also `--test
status`, `--test check_history` and `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Main path | a ticket with Spec stale and Plan not confirmed: the `ticket:` line, two fact lines in configured order, `next: confirm — stapel ok ABC-1`, by the golden file | `next::prints_facts_then_one_action` |
| Read-only | the tree, `.stapel/` and `git count-objects`/refs before and after a run at each of `confirm`, `build-step` and `run-check` are equal | `next::writes_nothing` |
| JSON | the same ticket with `--json`: one line, key order, `facts` without `blocked: `, `subject` `null`; a `build-step` case with `subject` `step 1` and `path`; by the golden file | `next::json_has_the_same_action` |
| No ticket | no ticket folder; only closed tickets; text and JSON | `next::no_open_ticket_names_new` |
| Wrong call | `KEY` `NOPE-1`; two folders `ABC-1` and `abc-1` with `KEY` `abc-1`; `--jsonx`; two open tickets without `KEY`; exit 2, stderr text, empty stdout | `next::refuses_a_wrong_call` |
| Unreadable | a legacy `state.json`; a `state.json` that is not JSON; a `ticket.md` that is a directory; outside a repository; a broken `stapel.toml`; exit 1, empty stdout | `next::refuses_an_unreadable_ticket` |
| Closed | a closed ticket by `KEY` in lower case: `next: closed`, no fact line | `next::closed_ticket` |
| Frame | Description `TODO`; empty; written without `### Frame`; `#### Frame` does not count | `next::frame_first` |
| Sections | Spec `TODO` with Design written gives Spec; Spec written, Design missing gives Design; Proof found twice gives the duplicated fact; Review and Summary `TODO` do not trigger this rule | `next::sections_in_order` |
| Plan structure | a Plan with only prose; a Plan with `2.` twice; `  1. ` indented and `1.x` do not count; `1C.` does not count; `12b.` gives `step 12b` | `next::plan_structure` |
| Spec review | all written, no confirmation, no findings: `spec-review`; a `step-drift` finding only: still `spec-review`; a `spec-review` finding: `confirm`; one earlier confirmation of `plan` only: `confirm` | `next::spec_review_until_a_fact` |
| Light route | `Route: light, pinned`, `Route: LIGHT`, `Route: lightweight` (full), no Route line (full) | `next::light_route_skips_spec_review` |
| Confirm | `spec` not confirmed and `design` stale by a change: two facts with the `status` texts, `next: confirm — stapel ok ABC-1` | `next::confirm_names_each_section` |
| Coverage (step 1c) | a Plan with `12b.` twice gives the fact `the Plan names step 12b twice` (D1-3) | `next::plan_labels_with_letters` |
| Coverage (step 1c) | all four sections written and confirmed, Review and Summary `TODO`, no commit: `next: build-step step 1 — .stapel/tickets/ABC-1/ticket.md` (D1-7) | `next::generated_sections_wait` |
| Coverage (step 1c) | `Route: light.` and `Route: light;` are light; `Route: light!` and `Route: light-ish` are full (D1-2) | `next::route_word_edges` |
| Coverage (step 1c) | `design` confirmed, then `design` and `spec` changed: two `design` facts, `changed since …` and `depends on spec, which changed` (D1-6) | `next::stale_reasons_each_a_fact` |
| Coverage (step 1c) | `next abc-1` on an open ticket at `write-frame` and at `confirm`: the path and `stapel ok ABC-1` use the folder key (D1-5) | `next::folder_key_in_outputs` |
| Lazy history | a shallow clone at the `confirm` stage: the action is printed (exit 0); the same clone with every section confirmed: exit 1 with the shallow refusal; a `base` off history at the `confirm` stage: the action is printed; with every section confirmed: exit 1 with the `check` text | `next::history_read_only_when_needed` |
| Step outside | history has `step 5` RED, the Plan has `1.` to `4.`: the fact and `write-section Plan` | `next::step_outside_the_plan` |
| History faults | a RED that changes code with its GREEN after it (`red-changes-code`); a GREEN without a RED (`unpaired`); two faults: two facts in history order with their commits, the action names the first; step 1 RED, step 2 RED, step 1 GREEN, step 2 GREEN (step 1 `unverified: interleaved`): no fact | `next::history_faults_block_the_build` |
| Failing check | a `runs.jsonl` check record `fail` at `HEAD` with a clean tree: the fact and `fix-check`; the same record after a new commit is stale and does not block | `next::failing_check_blocks_the_build` |
| Open RED | step 1 RED only, which changes code: the fact `step 1: red-changes-code at <the RED>` and `fix-history step 1`; step 1 RED only with no test: `fix-history step 1`; step 2 RED after step 1 RED with no GREEN: step 1 `unpaired` is a fault, step 2 is the open RED; step 2 RED with no test while step 1 has no commit: `fix-history step 2`, not `build-step step 1` | `next::open_red_is_judged_alone` |
| Steps | no commit: `build-step step 1`; step 1 RED only: `green-step step 1`; step 1 RED and GREEN: `build-step step 2`; step 2 done before step 1: `build-step step 1` | `next::first_open_step` |
| Coverage steps | `ABC-1 step 1c coverage: …` marks `step 1c` done; `ABC-1 step 4 cleanup: …` marks `step 4` done; `ABC-1 step 2 notes` (no colon) does not; `ABC-1 step 2: …` does not | `next::coverage_and_cleanup_steps_are_done` |
| Uncommitted | all steps done, `ticket.md` edited and not committed: the fact and `next: commit — git status`; only `runs.jsonl` changed: not `commit` | `next::uncommitted_changes_before_the_check` |
| Run check | all steps done, no check record: `run-check`; a passing check at `HEAD`: not `run-check`; no `[check]`: not `run-check` | `next::run_check_after_the_steps` |
| Review and summary | Review `TODO`: `code-review`; Review written, Summary `TODO`: `write-summary` | `next::code_review_then_summary` |
| Close | everything done: `next: close — stapel close ABC-1` | `next::close_at_the_end` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` (positional of `next`) | ticket folder name, any case | one ticket | absent: the only open ticket, or `new-ticket` when none is open | no such folder, several open without key: exit 2; legacy or unreadable: exit 1 | ASCII case ignored | `next::refuses_a_wrong_call`, `next::closed_ticket` |
| `--json` | flag | — | absent: text | an unknown option such as `--jsonx`: exit 2 (clap) | — | `next::json_has_the_same_action`, `next::refuses_a_wrong_call` |
| `state.json` | JSON, `state::load` | — | no confirmations: AC-8 or AC-9 | not JSON, legacy: exit 1 | — | `next::refuses_an_unreadable_ticket`, `next::spec_review_until_a_fact` |
| `runs.jsonl` check record | JSON line, newest well-formed (`checkstate::current`: full `head`, `result` `pass` or `fail`) | — | no file or no record: `run-check` | a record without `head` or with another `result`: skipped | short sha of 7 | `next::failing_check_blocks_the_build`, `next::run_check_after_the_steps` |
| `[check]` of `stapel.toml` | table | — | absent: AC-13 and AC-16 do not apply | a broken `stapel.toml`: exit 1 | — | `next::run_check_after_the_steps`, `next::refuses_an_unreadable_ticket` |
| `ticket.md` sections | Markdown, `## ` headings | — | missing, empty or `TODO`: not written | found twice: a fact | title case ignored (`ticket::find`) | `next::sections_in_order` |
| Plan step lines | `<digits>[a-z]. ` at the first column | `1.`; any number of digits | none: a fact | indented, uppercase letter, two letters: not a step | exact label | `next::plan_structure` |
| `Route:` line | text | — | absent: `full` | other word: `full` | `light` in any case, whole word | `next::light_route_skips_spec_review` |
| `findings.jsonl` `stage` | string | — | no file: no finding | a line that is not JSON: skipped | exact `spec-review` | `next::spec_review_until_a_fact` |
| commit subjects | text | — | no step commit: `build-step` | — | `<KEY> <label> <word>:`, key and label exact | `next::coverage_and_cleanup_steps_are_done` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | history shape: merge commits, rebased or squashed history, amended commits | first-parent only, as `check`; a `base` off it: exit 1 | `next::history_read_only_when_needed` |
| git | shallow clone | exit 1 only when the history is needed | `next::history_read_only_when_needed` |
| git | object format | not touched beyond `check`'s handling | — |
| git | detached HEAD; linked worktrees | `HEAD` is read as is; not promised beyond `check` | — |
| git | submodules | not read | — |
| git | paths with spaces and odd bytes | not read: only subjects | — |
| git | commit messages with control bytes | subjects are compared as text; not promised | — |
| git | dirty tree | after the steps, changes outside the machine files give `commit` (AC-20); machine files do not count | `next::uncommitted_changes_before_the_check` |
| git | configuration and `GIT_*` environment | not promised, as `check --history` | — |
| cargo | every dimension | not run | — |
| claude-code | every dimension | not read: the guard allows `stapel next` like `stapel status` | — |
| os | case-insensitive file systems; symlinks; permissions | as `stapel status` reads the ticket; an unreadable file exits 1 | `next::refuses_an_unreadable_ticket` |
| os | processes, time, WSL | not touched | — |

**Review Focus:** a closed pipe on stdout (`out.rs` drops it for every command, STP-16; not tested here);
two Plan lines `1.` and `01.` (labels `step 1` and `step 01` differ; not promised); a Route word with a
hyphen (`light-ish`, full by the letters-only rule; plan structure test); a fix-history fault in a step the
Plan does not name (AC-11 comes first; step outside test); `findings.jsonl` with a `spec-review` record of
another ticket's key (counted: the file is the ticket's own; not tested).

**Author self-check:** 2026-10-19, before spec review: every promise item has a criterion and a test; each
input has its empty and invalid case; the Inputs and External states rows were checked against `status.rs`,
`check.rs`, `stage.rs`, `steps.rs`, `checkstate.rs` and the catalogue; no open question is left.
2026-10-19, after spec review round 1 (external): 1 HIGH (SR1-1: an open RED reads `unpaired`, so
`green-step` was never named), 4 MEDIUM, 6 LOW; 10 inside, 1 outside (SR1-10, recorded as not promised).
All applied as one change: the term Open RED and test `next::open_red_is_judged_alone` (SR1-1); AC-12 names
the fault commit, says how the fact goes and leaves `unverified` to the full check (SR1-2); AC-20 `commit`
before `run-check` and test `next::uncommitted_changes_before_the_check` (SR1-3); Inputs rows for
`state.json`, `runs.jsonl` and `[check]` (SR1-4); AC-8 names the exit after a clean review (SR1-5); AC-1,
Frame item 2, AC-4, AC-9, AC-2 and Not promised (SR1-6 to SR1-11). Round 2 runs, as SR1-1 is HIGH.
2026-10-19, after spec review round 2 (external): SR1-1 and SR1-3 to SR1-11 fixed, SR1-2 partly; 0 HIGH,
2 MEDIUM, 4 LOW, all inside. One change: the open RED is judged in AC-12 wherever it stands, and AC-14
only names `green-step` or `build-step` (SR2-1, SR2-5); the interleaved test case gets both GREENs
(SR2-2); Guarantees name AC-20 and when a fault fact goes (SR2-3, SR2-4); Design accepts a second
`run-check` after the Review and Summary commits (SR2-6). No third round: no HIGH.
2026-10-19, after the drift review of step 1: D1-1 to D1-7 (3 MEDIUM, 4 LOW); one spec change: the Route
term states the word rule as built (D1-2), AC-9 gives one fact per stale reason (D1-6), Not promised names a
legacy ticket without `KEY`, a Description found twice and a `### Frame` in a fence (D1-1, D1-7), plan step
1c with five coverage tests (D1-2, D1-3, D1-5 to D1-7); D1-4 (the placeholder) is replaced by step 2.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `next::prints_facts_then_one_action`, `next::writes_nothing` |
| AC-2 | `next::json_has_the_same_action` |
| AC-3 | `next::no_open_ticket_names_new` |
| AC-4 | `next::refuses_a_wrong_call`, `next::refuses_an_unreadable_ticket` |
| AC-5 | `next::closed_ticket` |
| AC-6 | `next::frame_first` |
| AC-7 | `next::sections_in_order`, `next::plan_structure`, `next::plan_labels_with_letters`, `next::generated_sections_wait` |
| AC-8 | `next::spec_review_until_a_fact`, `next::light_route_skips_spec_review`, `next::route_word_edges` |
| AC-9 | `next::confirm_names_each_section`, `next::stale_reasons_each_a_fact`, `next::folder_key_in_outputs` |
| AC-10 | `next::history_read_only_when_needed` |
| AC-11 | `next::step_outside_the_plan` |
| AC-12 | `next::history_faults_block_the_build`, `next::open_red_is_judged_alone` |
| AC-13 | `next::failing_check_blocks_the_build` |
| AC-14 | `next::first_open_step` |
| AC-15 | `next::coverage_and_cleanup_steps_are_done` |
| AC-16 | `next::run_check_after_the_steps` |
| AC-17 | `next::code_review_then_summary` |
| AC-18 | `next::code_review_then_summary` |
| AC-19 | `next::close_at_the_end` |
| AC-20 | `next::uncommitted_changes_before_the_check` |

## Plan

Route: full (tag `interface`), auto.

1. The command, the ticket choice and the rules before the build: text output, no ticket, refusals, closed,
   frame, sections and Plan structure, spec review, confirm (AC-1 without `next::writes_nothing`, AC-3 to
   AC-9). Must not change: `stapel status`, `tickets::resolve`, `stage.rs`.
1c. Coverage after the drift review of step 1 (D1-2, D1-3, D1-5 to D1-7). Tests only: five new `#[test]`
   functions, each helper nested inside its function.
2. The rules that read history: lazy history and its refusals, steps outside the Plan, history faults,
   failing check, open RED, open steps, coverage and cleanup steps (AC-10 to AC-15). Must not change: the output and the exit codes of `stapel check` and `stapel check
   --history`, `outcomes.rs`, `steps::parse_subject`.
3. The rules after the build and the JSON form: uncommitted changes, run check, code review, summary,
   close, `--json` (AC-2, AC-16 to AC-20, and `next::writes_nothing` of AC-1 over the `confirm`, `build-step` and `run-check` stages). Must not change: `checkstate.rs`.

## Review

TODO

## Summary

TODO
