# STP-20 — stapel next: the next action from the facts

## Description

Source: `docs/PHASES.md`, plan item 1.1 (`docs/decisions/0004-product-direction-and-interface.md`); the next-session note
`.stapel/scratch/next/next-session.md`.

### Frame

**Problem.** The orchestrator finds the next action of a ticket by re-reading `CLAUDE.md`, the 50 KB ticket,
`stapel status`, `stapel check --history` and the journals on every turn; the ledger of 2026-10-08 names the
orchestrator's context as the main cost. Every fact needed is already on disk or in git. `stapel next [KEY]`
computes the action from them, so the skill of item 1.3 can be one rule: run `stapel next`, do what it says,
run it again.

**Plan item:** 1.1.

**Risk tags:** `interface` (a new command; its `--json` output is what the skill reads).

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
   the `stage` of the records in `findings.jsonl`, `.stapel/stapel.toml`, and the working tree through
   `git status` (without optional locks) for the paths outside the machine files.
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
- **Done step**: a Plan step whose label has a GREEN commit in history, or, when it has no RED, a commit on the first-parent
  history of `HEAD` whose subject starts with `<KEY> <label> ` followed by a word other than `RED:` or
  `GREEN:` that is at least one character and then `:` (`STP-19 step 3c coverage: …`; a bare `:` is no
  word).
- **Open RED**: a Plan step whose only step commit is a RED that is the newest step commit of the ticket,
  as the commit hook reads it (`check::hook_faults`); history gives it `unpaired`, which is not a fault here.
- **Fact line**: `blocked: <fact>`. **Action line**: `next: <action>` followed by ` <subject>` when the
  action has one and by ` — <target>` when it has a command or a path.
- **Ticket path**: `.stapel/tickets/<KEY>/ticket.md`, with the folder key.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the next command runs on an open ticket THE command prints `ticket: <KEY> — <title>`, then one fact line per blocking fact of AC-7 and AC-9, and of AC-11 to AC-13 and AC-20 when the history is read (AC-10), then one action line: the first that applies of AC-5 to AC-15, AC-20, AC-16 to AC-19, in that order; exit 0; it writes no file, no git object and not the git index. Output: `golden/next_text.txt`. | `next::prints_facts_then_one_action`, `next::writes_nothing`, `next::index_untouched` |
| AC-2 | WHEN the next command runs with `--json` THE command prints one line, a JSON object with the keys `v` (1), `ticket` (the key or `null`), `action`, `subject`, `command`, `path` (each a string or `null`) and `facts` (an array of strings: the fact texts without `blocked: `), in this order, and nothing else; exit as AC-1. Output: `golden/next_json.txt`. | `next::json_has_the_same_action`, `next::json_after_the_build` |
| AC-3 | WHEN the next command runs without `KEY` and no ticket is open THE command prints `next: new-ticket — stapel new "<title>"` (exit 0); with `--json`, `ticket` is `null` and `command` is `stapel new "<title>"`. | `next::no_open_ticket_names_new` |
| AC-4 | IF `KEY` names no ticket folder or several folders that differ only in case, or no `KEY` is given and several tickets are open, THEN the command prints `stapel: <reason of the ticket choice>` on stderr and exits 2; IF the ticket is legacy, its `state.json` or `ticket.md` cannot be read, or there is no repository or `.stapel/stapel.toml` cannot be read, THEN it prints `stapel: <reason>` and exits 1; stdout is empty in both cases. | `next::refuses_a_wrong_call`, `next::refuses_an_unreadable_ticket` |
| AC-5 | WHEN the ticket is closed THE action line is `next: closed` and there is no fact line. | `next::closed_ticket` |
| AC-6 | WHEN the Description section is not written or has no line `### Frame` THE action line is `next: write-frame — <ticket path>`. | `next::frame_first` |
| AC-7 | WHEN a confirmed section is not written, is missing or is found twice THE action line is `next: write-section <title> — <ticket path>` for the first such section in the configured order; a found-twice section adds the fact `<title> is duplicated`. A written Plan with no Plan step adds the fact `the Plan has no steps`, one with a label twice adds `the Plan names <label> twice`, and either counts as the Plan not written. | `next::sections_in_order`, `next::plan_structure`, `next::plan_labels_with_letters`, `next::generated_sections_wait` |
| AC-8 | WHEN every confirmed section is written, the route is not `light`, `state.json` holds no confirmation of any section, and no record of `findings.jsonl` has `stage` `spec-review` THE action line is `next: spec-review — <ticket path>`; a review with no finding is followed by `stapel ok <KEY>`, whose confirmation ends this rule; on a `light` route the rule does not apply. | `next::spec_review_until_a_fact`, `next::light_route_skips_spec_review`, `next::route_word_edges`, `next::findings_without_version` |
| AC-9 | WHEN a confirmed section has no confirmation or a stale one and no rule of AC-6 to AC-8 applies THE command adds the fact `<id> not confirmed`, or `<id> confirmation is stale: <reason>` (`Reason::describe`) once per stale reason, in the configured order, and the action line is `next: confirm — stapel ok <KEY>`. | `next::confirm_names_each_section`, `next::stale_reasons_each_a_fact`, `next::folder_key_in_outputs`, `next::facts_wait_for_confirm` |
| AC-10 | IF no rule of AC-5 to AC-9 applies and the history cannot be read (a shallow clone, a `base` off the first-parent history of `HEAD`, a git error) THEN the command prints `stapel: <the refusal of stapel check>` and exits 1; while one of those rules applies, the command does not read the history, so neither stops it. | `next::history_read_only_when_needed` |
| AC-11 | WHEN a step commit of the ticket has a label that no Plan step has THE command adds the fact `history has <label>, the Plan does not` for each such label in history order, and the action line is `next: write-section Plan — <ticket path>`. | `next::step_outside_the_plan`, `next::history_order_of_facts` |
| AC-12 | WHEN a step's history outcome is not passing, or is `unverified` for a reason other than `interleaved` (a build input changed), or an open RED gets `no-tests` or `red-changes-code` from `outcomes::open_red`, THE command adds the fact `<label>: <outcome> at <short sha> — <reason>` (the fault commit) per step in history order and the action line `next: fix-history <label> — stapel check --history <KEY>` for the first; an open RED's own `unpaired` and `interleaved` are no fault. | `next::history_faults_block_the_build`, `next::open_red_is_judged_alone`, `next::history_order_of_facts`, `next::unpaired_red_behind_a_green`, `next::build_input_change_is_a_fault`, `next::build_input_names_the_first_step` |
| AC-13 | WHEN `[check]` is set and the newest check record is a current `fail` (`check: fail at <sha>` in `stapel status`) THE command adds the fact `check fails at <short sha>` and the action line is `next: fix-check — .stapel/tickets/<KEY>/runs.jsonl`. | `next::failing_check_blocks_the_build`, `next::rule_order_after_history`, `next::check_records_after_the_steps` |
| AC-14 | WHEN history has an open RED without a fault THE action line is `next: green-step <its label> — <ticket path>`, wherever the step stands in the Plan; else, for the first Plan step in Plan order that is not done, `next: build-step <label> — <ticket path>`. | `next::first_open_step`, `next::open_red_out_of_plan_order` |
| AC-15 | WHEN a Plan step has no RED and no GREEN but a commit `<KEY> <label> <word>: …` with another word (coverage, cleanup) THE step is done and AC-14 moves to the next Plan step. | `next::coverage_and_cleanup_steps_are_done`, `next::coverage_word_after_a_red` |
| AC-16 | WHEN every Plan step is done, `[check]` is set and the check line of `stapel status` is not `check: pass at <sha>` THE action line is `next: run-check — stapel check <KEY>`; without `[check]` this rule does not apply. | `next::run_check_after_the_steps`, `next::check_records_after_the_steps` |
| AC-17 | WHEN the rules above do not apply and the section of id `review` is configured and not written THE action line is `next: code-review — <ticket path>`. | `next::code_review_then_summary`, `next::review_and_summary_edges` |
| AC-18 | WHEN the rules above do not apply and the section of id `summary` is configured and not written THE action line is `next: write-summary — <ticket path>`. | `next::code_review_then_summary`, `next::review_and_summary_edges` |
| AC-19 | WHEN no rule of AC-5 to AC-18 and AC-20 applies THE action line is `next: close — stapel close <KEY>`. | `next::close_at_the_end` |
| AC-20 | WHEN every Plan step is done and the working tree has changes outside the machine files (`worktree::dirty_paths`, which `stapel check` refuses) THE command adds the fact `uncommitted changes: <the first five paths of worktree::dirty_paths, joined by a comma and a space>` and the action line is `next: commit — git status`; this rule comes before AC-16. | `next::uncommitted_changes_before_the_check`, `next::uncommitted_paths_order` |
| AC-21 | WHEN `stapel.toml` configures no section of id `plan` THE command reads no Plan step; AC-11, AC-15 and the `build-step` part of AC-14 do not apply, so after AC-12, AC-13 and an open RED's `green-step` the rules AC-20 and AC-16 to AC-19 follow. | `next::no_plan_section_configured`, `next::open_red_without_a_plan_section` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-21; a fault fact of AC-12 goes when the history is
  rewritten (amend or rebase), or, for `unpaired` without a GREEN, when that GREEN is committed; `stapel status`, `stapel check` and `stapel check
  --history` keep their output and their tests.
- **Not promised:** the Frame's list; without `KEY`, a ticket whose `state.json` cannot be read counts as
  not open, as `tickets::resolve` reads it, and so does a legacy one (D1-1); a Description found twice gives
  `write-frame` with no fact; a `### Frame` line inside a code fence counts (D1-7); a subject `<KEY> step 2 coverage RED: …` is a step commit with
  the label `step 2 coverage` (`parse_subject`), so AC-11 names it; a Plan step written inside a list item or a code block (any line
  that starts at the first column counts); a label in history that differs from a Plan label only in case
  (they differ); an unborn `HEAD` (no commit yet) reads as an empty history, not as a refusal of AC-10, and
  a git error of `outcomes::open_red` exits 1 untested (D2-3); `findings.jsonl` lines that are not JSON (skipped, as `status` skips them); the time a long
  history takes (item 1.11); a ticket that changes while the command runs; a failing check hidden by a dirty tree, which makes it stale
  as in `stapel status` (E-5); `run-check` named again when `stapel check` refuses without a record (no
  step commits, a held lock) (E-7); a ticket without `[check]` and with an `interleaved` step goes to
  `close` (DR-7); a fault with no commit at all, which history cannot give (R-3).

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

### Decisions (as built)

- Step 1 (302cdcd, 77b9d28): `stapel_core::next::decide(&Facts) -> Decision` is pure; `plan_steps`, `is_light`,
  `ticket_path` and `Action::line` sit next to it; the command is `crates/stapel-cli/src/next.rs`, routed in
  `main.rs` through `fail::finish`. The facts of AC-7 and AC-9 are always gathered and printed whatever the
  action (a closed ticket has none), AC-7 facts before AC-9 facts, one AC-9 fact per stale reason. An empty or
  `TODO` Plan gives `write-section Plan` without the "has no steps" fact. The Route word is the first token
  after `Route:` with trailing `, . ; :` trimmed, compared to `light` in any case. `ticket.md` is read before
  the closed check, so an unreadable one exits 1 for a closed ticket too. Without `KEY`, `next` counts open
  tickets itself (`tickets::list`) for `new-ticket`; `resolve` is unchanged. A legacy ticket exits 1 with
  "ticket <KEY> is in the STP-1 format; it has no stage to compute". `findings.jsonl` stages count from V1
  and Legacy lines; non-JSON lines are skipped; an unreadable file exits 1. Until step 2, the placeholder
  after AC-9 is `build-step <first Plan label>`; until step 3, `--json` is parsed and ignored. Since step 2
  the first pass of `decide` still returns that `build-step` as a value the CLI replaces after the history is
  read (DR-6).
- Step 1c (3097086): five coverage tests, all passing at once.
- Step 2 (b1cfc37, ff25d51): `stapel_core::next` gains `History`, `Fault` and `closes_step` (the AC-15 subject
  rule); `Facts.history` is `Option<&History>` and `Decision.needs_history` tells the CLI to gather it. The CLI
  runs `decide` without history first; only on `needs_history` it reads `HEAD`, `shallow_refusal`, `base_of`
  (now `pub(crate)` in `check.rs`), `step_commits`, `history_report`, `open_red`, the first-parent subjects and
  `checkstate::current`, then runs `decide` again. An unborn `HEAD` is an empty history, not an AC-10
  refusal; a folder that is not a git repository exits 1 with the git error. A fault without a fault commit
  takes the step's RED sha, else its GREEN. Labels outside the Plan come from RED and GREEN commits only. An
  `open_red` git error exits 1. A check stays current after an empty commit (`checkstate::current`), so the
  stale case of the test changes a file. Until step 3, `run-check` is the placeholder after the steps.
- Step 2c (8780e13): three coverage tests, all passing at once.
- Step 3 (bfe6d56, a5332a2): `History` gains `check_configured`, `check_pass` and `dirty`, filled in `gather`
  from `checkstate::current` and `worktree::dirty_paths` only when the history is needed; `after_the_steps`
  applies `commit`, `run-check`, `code-review`, `write-summary`, `close` in that order. The uncommitted fact
  joins the first five paths of `dirty_paths` (`git status --porcelain=v1`: tracked changes, then untracked
  files) with `, `. `run-check` fires with `[check]` when the
  newest record is none or stale (a current fail stops at `fix-check` before). `code-review` and
  `write-summary` use the section ids `review` and `summary` and are skipped when those are not configured.
  `--json` is `serde_json::json!` under `preserve_order`, one line, also for `new-ticket` and `closed`. The
  tests after the build share their helpers through one top-level `macro_rules! after_the_build`.
  `next::writes_nothing` passed before the code and moved to the coverage step 3c.
- Step 3c (cc6d366): five coverage tests, all passing at once. In `review_and_summary_edges` the
  `review` and `summary` sections are removed from `stapel.toml` after the confirmations, which stay fresh.

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
| Coverage (step 2c) | history has `step 6` RED then `step 5` RED, the Plan has `1.` to `4.`: the facts in that order; step 2 faults before step 1 in history: the exact fact list in history order, the action names step 2 (D2-1, D2-8) | `next::history_order_of_facts` |
| Coverage (step 2c) | step 1 RED, step 2 RED, step 1 GREEN, no step 2 GREEN: the exact fact `step 2: unpaired at <step 1 GREEN> — …` and `next: fix-history step 2 — stapel check --history ABC-1` (D2-2) | `next::unpaired_red_behind_a_green` |
| Coverage (step 2c) | a current failing check and a history fault: the fault fact, then the check fact, action `fix-history`; a current failing check and step 2 not built: `fix-check`; no `[check]` and a `fail` record: no check fact, `build-step` (D2-5) | `next::rule_order_after_history` |
| Uncommitted | all steps done, `ticket.md` edited and not committed: the fact and `next: commit — git status`; only `runs.jsonl` changed: not `commit` | `next::uncommitted_changes_before_the_check` |
| Run check | all steps done, no check record: `run-check`; a passing check at `HEAD`: not `run-check`; no `[check]`: not `run-check` | `next::run_check_after_the_steps` |
| Review and summary | Review `TODO`: `code-review`; Review written, Summary `TODO`: `write-summary` | `next::code_review_then_summary` |
| Close | everything done: `next: close — stapel close ABC-1` | `next::close_at_the_end` |
| Coverage (step 3c) | the Read-only row: `next::writes_nothing` passed before the code of step 3 | `next::writes_nothing` |
| Coverage (step 3c) | no `review` and no `summary` section configured, all else done: `close`; Review `TODO` with Summary written: `code-review`; Summary with an empty body: `write-summary` (D3-1) | `next::review_and_summary_edges` |
| Coverage (step 3c) | steps done, a `fail` record at `HEAD`: the fact `check fails at <sha>` and `fix-check`; a record without `head`: `run-check`; a record with `result` `unknown`: `run-check` (D3-2) | `next::check_records_after_the_steps` |
| Coverage (step 3c) | a tracked file changed and untracked files created in reverse name order, one in a subfolder: the fact lists the paths in `git status --porcelain` order, first five (D3-4) | `next::uncommitted_paths_order` |
| Coverage (step 3c) | `--json` at `commit`: `command` `git status`, `path` `null`, the uncommitted fact; at `close`: `command` `stapel close ABC-1`, `facts` `[]` (D3-7) | `next::json_after_the_build` |
| Index | a tracked file touched (same content, new mtime) and every step done: the bytes of `.git/index` before and after `next` are equal (R-1, DR-3) | `next::index_untouched` |
| Facts wait | a ticket with Description `TODO`: only the action line, no `not confirmed` fact; Spec `TODO`: the same; at `confirm`: the facts (E-9) | `next::facts_wait_for_confirm` |
| Build input | step 1 RED and GREEN, then a commit `ABC-1 deps: …` that adds a dependency to a manifest, then step 2 RED and GREEN: the fact `step 2: unverified at <the dependency commit> — build-input-changed: …` and `fix-history` (E-4) | `next::build_input_change_is_a_fault` |
| Open RED order | Plan `1.` and `2.`, history only a clean step 2 RED: `green-step step 2` (E-1) | `next::open_red_out_of_plan_order` |
| Word after RED | `ABC-1 step 2 RED: …` then `ABC-1 step 2 wip: …`: step 2 is not done, `green-step step 2`; `ABC-1 step 1c RED` then `ABC-1 step 1c coverage:` the same (E-3, E-10) | `next::coverage_word_after_a_red` |
| No Plan section | a `stapel.toml` without the `plan` section, Review and Summary written, a passing check: `close`; with a step commit in history: no `history has` fact (R-2, E-2) | `next::no_plan_section_configured` |
| Coverage (step 4c) | a `findings.jsonl` line without `v` and with `stage` `spec-review` (the shape of every record of STP-19): `confirm`, not `spec-review` (R-4) | `next::findings_without_version` |
| Coverage (step 4d) | no `plan` section and a clean open RED: `green-step <its label>` (D4-1) | `next::open_red_without_a_plan_section` |
| Coverage (step 4d) | the Build input case: the exact facts in history order and the exact action `next: fix-history step 1 — stapel check --history ABC-1` (D4-3) | `next::build_input_names_the_first_step` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` (positional of `next`) | ticket folder name, any case | one ticket | absent: the only open ticket, or `new-ticket` when none is open | no such folder, several open without key: exit 2; legacy or unreadable: exit 1 | ASCII case ignored | `next::refuses_a_wrong_call`, `next::closed_ticket` |
| `--json` | flag | — | absent: text | an unknown option such as `--jsonx`: exit 2 (clap) | — | `next::json_has_the_same_action`, `next::refuses_a_wrong_call` |
| `state.json` | JSON, read by `load` of `state.rs` | — | no confirmations: AC-8 or AC-9 | not JSON, legacy: exit 1 | — | `next::refuses_an_unreadable_ticket`, `next::spec_review_until_a_fact` |
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
2026-10-19, after the drift review of step 2: D2-1 to D2-8 (3 MEDIUM, 5 LOW); one spec change: the Done step
term needs a word before `:` (D2-4), Not promised names the unborn `HEAD` and an untested `open_red` git error
(D2-3), plan step 2c with three coverage tests (D2-1, D2-2, D2-5, D2-8); D2-6 (a branch no ticket reaches)
and D2-7 (the open-RED test repeats the commit hook's) go to Design as built.
2026-10-19, after step 3 and its drift review: `next::writes_nothing` passed before the code of step 3 (the
plan should have made it a coverage step); D3-1 to D3-9 (4 MEDIUM, 5 LOW). One spec change: AC-20 names the
path order of `git status --porcelain` (D3-4), plan step 3c with `writes_nothing` and four coverage tests
(D3-1, D3-2, D3-4, D3-7). No change for D3-3 (`stapel-cli` enables `preserve_order`), D3-5 (`dirty_paths`
runs on every history read), D3-6 (a doc comment, for the code review cleanup), D3-8, D3-9.
2026-10-20, after step 3c: the drift test checks the built ticket in full and found a missing path, a
risk tags line that did not start its line and a quoted name read as a test; all three corrected. Design as
built moved into the ticket; Design is complete before code review.
2026-10-20, after code review round 1 (reviewer, external, drift): 0 HIGH; 11 MEDIUM over 8 root causes,
all inside but E-2 (a foreign `stapel.toml`). With the human: an `unverified` build-input outcome is a fault
(E-4) and the confirmation facts wait until no earlier action applies (E-9). One spec change: Frame item 2
names `stapel.toml` and the working tree (DR-2); AC-1 also keeps the git index (R-1, DR-3); AC-7, AC-9,
AC-12, AC-14, AC-15, AC-20 and the new AC-21 (no `plan` section; R-2, DR-4, E-2); Not promised gains E-5,
E-7, DR-7, R-3; plan steps 4 and 4c. No change for E-6 and E-11 (documented in AC-8), E-10 (fixed by AC-15),
the R-5 code quality notes. `stapel check` passed at 134973b (549 tests) before this change.
2026-10-20, after the drift review of step 4: D4-1 to D4-9 (1 HIGH, 3 MEDIUM, 5 LOW). D4-1 (an open RED
gives `green-step` without a `plan` section, which AC-21 excluded) is settled in the text: the open RED
keeps its `green-step`, as a `run-check` on an unfinished step would loop; D4-6 the Done step term names
the RED; plan step 4d with two coverage tests (D4-1, D4-3). No change for D4-2 (the build-input sha is
taken only when no fixed outcome exists, so only for `unverified`), D4-4 and D4-5 (a defensive branch that
AC-12 and AC-14 make unreachable), D4-7 (the interleaved case is tested), D4-8, D4-9. The builder of step 4
appended its tests with `cat >>`, which the guard allowed (item 1.9).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `next::prints_facts_then_one_action`, `next::writes_nothing`, `next::index_untouched` |
| AC-2 | `next::json_has_the_same_action`, `next::json_after_the_build` |
| AC-3 | `next::no_open_ticket_names_new` |
| AC-4 | `next::refuses_a_wrong_call`, `next::refuses_an_unreadable_ticket` |
| AC-5 | `next::closed_ticket` |
| AC-6 | `next::frame_first` |
| AC-7 | `next::sections_in_order`, `next::plan_structure`, `next::plan_labels_with_letters`, `next::generated_sections_wait` |
| AC-8 | `next::spec_review_until_a_fact`, `next::light_route_skips_spec_review`, `next::route_word_edges`, `next::findings_without_version` |
| AC-9 | `next::confirm_names_each_section`, `next::stale_reasons_each_a_fact`, `next::folder_key_in_outputs`, `next::facts_wait_for_confirm` |
| AC-10 | `next::history_read_only_when_needed` |
| AC-11 | `next::step_outside_the_plan`, `next::history_order_of_facts` |
| AC-12 | `next::history_faults_block_the_build`, `next::open_red_is_judged_alone`, `next::history_order_of_facts`, `next::unpaired_red_behind_a_green`, `next::build_input_change_is_a_fault`, `next::build_input_names_the_first_step` |
| AC-13 | `next::failing_check_blocks_the_build`, `next::rule_order_after_history`, `next::check_records_after_the_steps` |
| AC-14 | `next::first_open_step`, `next::open_red_out_of_plan_order` |
| AC-15 | `next::coverage_and_cleanup_steps_are_done`, `next::coverage_word_after_a_red` |
| AC-16 | `next::run_check_after_the_steps`, `next::check_records_after_the_steps` |
| AC-17 | `next::code_review_then_summary`, `next::review_and_summary_edges` |
| AC-18 | `next::code_review_then_summary`, `next::review_and_summary_edges` |
| AC-19 | `next::close_at_the_end` |
| AC-20 | `next::uncommitted_changes_before_the_check`, `next::uncommitted_paths_order` |
| AC-21 | `next::no_plan_section_configured`, `next::open_red_without_a_plan_section` |

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
2c. Coverage after the drift review of step 2 (D2-1, D2-2, D2-5, D2-8). Tests only: three new `#[test]`
   functions, each helper nested inside its function.
3. The rules after the build and the JSON form: uncommitted changes, run check, code review, summary,
   close, `--json` (AC-2, AC-16 to AC-20). Must not change: `checkstate.rs`.
3c. Coverage after step 3 and its drift review: `next::writes_nothing` of AC-1 over the `confirm`,
   `build-step` and `run-check` stages, which passed before the code of step 3, and D3-1, D3-2, D3-4,
   D3-7. Tests only: five new `#[test]` functions, each helper nested inside its function.
4. Code review round 1: the index untouched (`--no-optional-locks` for `git status` in
   `worktree::dirty_paths`), AC-9 facts wait, `unverified` build-input outcomes as faults, the open RED
   wherever it stands, coverage words only for a step without a RED, no `plan` section (AC-1, AC-9, AC-12,
   AC-14, AC-15, AC-21). The RED may change assertions of earlier tests that AC-9 now changes, and the
   header comment of `tests/next.rs` (DR-5). The GREEN also corrects the doc comment of
   `History::check_pass` (D3-6). Must not change: `checkstate.rs`, `outcomes.rs`, the output and exit codes
   of `stapel check`.
4c. Coverage after code review round 1 (R-4): one new `#[test]` function, its helpers nested inside it.
4d. Coverage after the drift review of step 4 (D4-1, D4-3): two new `#[test]` functions, each helper nested
   inside its function.

## Review

TODO

## Summary

TODO
