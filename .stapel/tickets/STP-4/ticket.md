# STP-4 — RED to GREEN check on the final commit

## Description

Source: `docs/PHASES.md`, phase 0, fourth ticket; phase-0 criterion 4: "for a plan step with a test command,
the tool distinguishes: the test did not fail before the change; failed and now passes; does not pass. The
check runs on the final commit." No external tracker.

Every plan step here is a pair of commits, RED (tests) then GREEN (code). Today nobody checks that the RED
tests really failed before the GREEN code: a test that passes from the start proves nothing, and klc, which
checks only the order of commits, accepted such tests (KLC-110). This ticket adds `stapel check`: it finds the
ticket's RED and GREEN commits, runs each step's tests at its RED commit (they must fail) and at its GREEN
commit (they must pass), runs the whole suite at the final commit, and appends the result to `runs.jsonl`.
`stapel status` shows the result, and `stapel close` refuses while the current code has no passing check.

The first step adds the mechanical drift checks agreed after STP-3 (CLAUDE.md items 5 and 9).

**Risk tags:** `guard` (the close gate), `data` (a new record format in `runs.jsonl`), `interface` (a new
command, a new `status` line, a new `stapel.toml` section).

## Spec

### Acceptance criteria

Criteria state behaviour; exact output lives in golden files under the CLI tests' `golden` folder
(CLAUDE.md item 9). A step is named by its *label*: the text between the key and `RED:` or `GREEN:` in a
commit subject (`step 3`, `code review round 1`).

| № | Criterion | Test |
|---|---|---|
| AC-1 | `stapel check [KEY] --list` · reads the first-parent history of `HEAD` (at most 10 000 commits) and prints, per label in the order of its RED commit, the RED and GREEN commits and the step's tests, without running anything; a commit belongs to the ticket when its subject starts with `<KEY> <label> RED: ` or `<KEY> <label> GREEN: ` (so `STP-3 ` never matches `STP-30 `, and `RED` elsewhere in a subject is not a marker); a step's tests are the test functions of `crates/*/tests/*.rs` that the RED commit adds or whose text it changes, named `module::name` as tickets name them; the layout is that of the golden file `check_list.txt`. | `check::list_pairs_red_and_green_by_label`, `check::list_ignores_other_keys_and_prose`, `check::list_finds_added_and_changed_tests`, `check::list_matches_golden` |
| AC-2 | `stapel check` · gives each step one outcome: `pass`; `unpaired` (a RED without a GREEN, a GREEN without a RED, or a GREEN before its RED); `duplicate` (two RED or two GREEN commits with one label); `no-tests` (the RED commit adds or changes no test, a delete-only commit included); `red-changes-code` (the RED commit changes a path other than `crates/*/tests/**`, `Cargo.toml` files, `Cargo.lock`, `.stapel/**`, `docs/**` and `*.md`, with the paths named); `no-red` (a step test passes at the RED commit twice in a row, with the names); `not-green` (a step test does not pass at the GREEN commit); `removed` (a step test no longer exists at `HEAD`); `unverified` with a reason (timeout, the run did not start, a step test is ignored or was not run, `Cargo.lock` out of date). A RED test that fails because its file does not compile counts as failing and is shown as `compile`, one that compiles and fails as `assert`. | `check::outcome_pass_with_assert_and_compile_reds`, `check::outcome_no_red`, `check::outcome_not_green`, `check::outcome_unpaired_and_duplicate`, `check::outcome_no_tests_for_delete_only_red`, `check::outcome_red_changes_code`, `check::outcome_removed`, `check::outcome_unverified_on_timeout_and_stale_lock` |
| AC-3 | `stapel check [KEY]` · runs nothing in the working tree: it uses one worktree at `<git common dir>/stapel/check-worktree`, created detached and removed at the end, also after a failure; it runs the tests of a step at a commit with `cargo test --locked -p <package> --test <file> -- --exact <names>` and, at `HEAD`, the whole suite with `cargo test --workspace --locked`, each with `CARGO_TARGET_DIR` at `<git common dir>/stapel/check-target` and a time limit of `timeout_secs`, killing the whole process group when it runs out; it refuses with exit 1 when files outside the machine files have uncommitted changes, when the ticket has no step, and when another `stapel check` of the repository runs; it exits 0 only when every step and the suite pass. The layout of its report is that of the golden file `check_report.txt`. | `check::runs_in_a_worktree_and_leaves_the_tree_alone`, `check::refuses_uncommitted_changes`, `check::refuses_ticket_without_steps`, `check::refuses_a_second_check`, `check::suite_failure_fails_the_check`, `check::report_matches_golden` |
| AC-4 | `stapel check` · appends one line to the ticket's `runs.jsonl` per run, through the STP-3 journal (`v: 1`, at most 64 KiB): `kind: "check"`, `id`, `at`, `ticket`, `head` (the full sha), `tool` (`stapel` and its version), `result` (`pass` or `fail`), per step its label, RED and GREEN commits, outcome, the counts of its tests and at most 20 failing names, and for the suite its outcome and counts; a refused check appends nothing; lines without `kind: "check"` are kept and ignored. | `check::appends_a_run_record`, `check::refusal_appends_no_record`, `check::other_run_lines_are_ignored` |
| AC-5 | `stapel status` and `stapel close` · treat the newest check record of the ticket as current when no tracked or untracked file outside the machine files (`state.json`, `decisions.jsonl`, `tokens.jsonl`, `runs.jsonl`, `findings.jsonl` of any ticket) differs between its `head` and the working tree; `status` prints one line: `check: pass at <short sha>`, `check: fail at <short sha>`, `check: stale` with the first changed paths, `check: none`, or `check: not configured`; `close` refuses with exit 1 unless the current record is `pass`, naming the reason and `stapel check`, and when `[check]` is configured the permission dialog of `close` shows the check line. | `check::status_shows_the_check_line`, `check::machine_files_keep_a_check_current`, `check::close_needs_a_current_passing_check`, `check::close_dialog_shows_the_check` |
| AC-6 | `stapel.toml` · may have `[check]` with `runner = "cargo"` (the only runner) and `timeout_secs` from 10 to 86 400 (default 900); another runner, a value out of range or an unknown key is a configuration error of every command, as in STP-2; without `[check]`, `stapel check` refuses with `check is not configured` and `close` does not ask for a check; `stapel init` writes `[check]` with `runner = "cargo"` when the repository root has a `Cargo.toml`, and a commented example otherwise. | `core::config::check_section_is_validated`, `init::writes_check_for_cargo_repositories`, `check::refuses_without_configuration` |
| AC-7 | The `ticket_drift` check (CLAUDE.md items 5 and 9), for tickets not in its list of earlier tickets (STP-1, STP-2, STP-3) · fails when, once every criterion is built or the ticket is closed, a ticket tagged `security` or `guard` has no abuse table (a heading that starts with `### Abuse`) or no line that starts with `**Author self-check`; when the tests named in its criteria and risks differ from those named in its Proof, or one of them is missing from its Test plan; and when a long option of a `stapel` subcommand (from its `--help`) is named in no ticket's Inputs table, apart from the options listed in the check as older than this rule. | `ticket_drift::abuse_table_and_self_check_are_required`, `ticket_drift::test_lists_must_agree`, `ticket_drift::every_option_is_in_an_inputs_table` |

### Guarantees

- **Promised, against an agent that makes mistakes:** a ticket closes only when, for the code as it is,
  every step's tests failed at its RED commit and passed at its GREEN commit, and the whole suite passes at
  `HEAD`; outcomes are computed from git history and real test runs, never from a stored claim; a later edit
  of code, tests or ticket text makes the check stale.
- **Not promised:** against an agent that appends a forged line to `runs.jsonl` through Bash (the declared
  STP-1 hole; the write tools cannot write it), rewrites history, or deletes the `[check]` section through
  Bash; that a RED test fails for the right reason (a `compile` RED says only that the test needed new code);
  that the tests are good tests; flaky tests beyond one repeat of a passing RED; unit tests inside
  `crates/*/src` (not named by tickets); runners other than `cargo`.

### Questions

All answered on 2026-10-05 by the human ("давай попробуем": as recommended), except those marked.

1. **How RED is checked.** In a detached worktree at the RED commit; then at GREEN; the whole suite at `HEAD`.
   Answer: as recommended.
2. **Which tests belong to a step.** Those the RED commit adds or changes, found from its diff; no plan
   column. Answer: as recommended.
3. **A RED that only fails to compile.** Accepted and shown as `compile`. Answer: as recommended.
4. **Does the check block the close.** Yes, when `[check]` is configured. Answer: as recommended.
5. **Repeats and time.** A passing RED test runs once more; everything else once; one time limit per run in
   `stapel.toml`. Answer: as recommended (limit 900 s rather than the 300 s first proposed: the whole suite of
   this repository with a cold build takes longer).
6. **Earlier tickets.** `stapel check STP-3` on this repository is the integration run; what it finds is data,
   the old tickets are not changed. Answer: as recommended.
7. **Fixed commands, no shell** (orchestrator, from klc KLC-174 F-001). The runner builds the `cargo`
   argument list itself; `stapel.toml` cannot hold a command line.
8. **Freshness by content, not time** (orchestrator, from klc KLC-174 F-002, F-003). A record names a commit;
   it is current while nothing but machine files differs from it, so committing `runs.jsonl` itself does not
   make it stale.
9. **No `[check]`, no gate** (orchestrator). Repositories without `cargo` and the CLI tests of other commands
   work as before; the close dialog says whether a check was required.

### Out of scope

- Runners other than `cargo` (the comparison with klc on a TypeScript project needs one; a later ticket).
- Unit tests inside `crates/*/src`.
- Checking a single step during the build (`--step`); the per-step drift review of CLAUDE.md item 11 is done
  by the orchestrator by hand in phase 0.
- Signing or hashing records against forgery.
- Running steps in parallel; caching results between checks.
- Tickets without code steps (a documentation-only ticket cannot be closed while `[check]` is configured).

## Design

### What klc does, and what we take

From a study of `../klc` (token journal entry `research`, 2026-10-05):

| klc | Take / do differently | Why |
|---|---|---|
| RED→GREEN checked as commit order by changed paths only; the test is never run at RED (KLC-039 chose this; its HIGH "true was-red verification" was left unfixed) | Run the step's tests at the RED commit | The phase-0 criterion: "did not fail before the change" is invisible otherwise (KLC-110: vacuous REDs) |
| Steps found by subject `<KEY> step-N`, with a guard so `step-1` does not match `step-10` | Labels matched as the whole text between key and `RED:`/`GREEN:` | Same defect, and code review rounds are steps too |
| Commit classes from `--first-parent -M -C --name-status` against the commit and its parent only | Take | KLC-109: verdicts tied to the working tree flipped later; renames counted as added tests |
| A delete-only test commit counts as RED (KLC-126, open) | `no-tests` | Fixes the open hole |
| A stored `exit_code` is trusted at ack (KLC-174 F-007) | Outcomes are recomputed by every check; the record is a log | Principle 2 |
| Commands from the plan through a shell-like split; `a#b; cmd` ran (KLC-174 F-001) | No command text from files; the runner builds argv | Removes the class |
| Verify bound to a 1-second timestamp, a future `ran_at` never stale (KLC-174 F-002, F-003) | Bound to a commit and to content | Questions 7 and 8 |
| A generic verify that matched zero tests passed; skipped meant pass only in pytest | Not run or ignored is `unverified` | `cargo test` exits 0 on zero matches (verified) |
| Timeout is "unverified", never "fail"; the process group is killed; output capped | Take | KLC-115: a shared timeout reported every node as failing |

### External contract (facts)

| Claim | Source | Mark |
|---|---|---|
| `cargo test --test <file> -- --exact <a> <b>` runs only tests named exactly `a` and `b` of that target; an integration test's name is its bare function name; a filter matching nothing prints `running 0 tests` and exits 0 | cargo 1.93.1 on this repository and a scratch crate, 2026-10-05 | verified |
| A failing test or a compile error exits 101; a compile error prints `error: could not compile` and no `running` line for that target; a run prints `running <n> tests`, one `test <name> ... ok\|FAILED\|ignored` line per test, then `failures:` sections with captured output, then `test result:` | same | verified |
| `--locked` with an out-of-date `Cargo.lock` exits 101 with `cannot update the lock file … because --locked was passed` | same | verified |
| `git worktree add --detach <path> <commit>` creates a checkout sharing the object store; `git worktree remove --force` removes it; the main working tree is not touched | git, scratch repository, 2026-10-05 | verified |
| `cargo test --workspace --test <file>` selects the package that has the target; `-p` makes it exact | this repository, 2026-10-05 | verified |
| Cargo reuses compiled dependencies through a shared `CARGO_TARGET_DIR`; workspace crates rebuild when their path changes, so one fixed worktree path keeps builds incremental | cargo documentation (fingerprints include the path) | read |
| `git status --porcelain --untracked-files=all` lists tracked changes and untracked files that are not ignored | git documentation | read |

### Decisions (as built)

To be filled by the GREEN commits: parser rules for test functions and their text, the label pattern, the
worktree and lock file handling, the output parser of the `cargo` runner.

### Risks

| Risk | Test |
|---|---|
| R-1 A test that never failed is accepted as RED | `check::outcome_no_red` |
| R-2 The check runs in or changes the person's working tree | `check::runs_in_a_worktree_and_leaves_the_tree_alone` |
| R-3 Captured test output that looks like a result line changes an outcome | `check::captured_output_cannot_fake_a_result` |
| R-4 A check of other code is taken as current | `check::close_needs_a_current_passing_check`, `check::machine_files_keep_a_check_current` |
| R-5 A hung test blocks the check for ever | `check::outcome_unverified_on_timeout_and_stale_lock` |

## Test plan

CLI tests build a tiny `cargo` crate without dependencies in a temporary git repository and commit RED and
GREEN steps into it; one shared target folder per test keeps them fast.

| Category | Cases | Tests |
|---|---|---|
| Main path | list and check a ticket with assert and compile REDs; report and list against golden files; status line; close after a passing check | `check::list_pairs_red_and_green_by_label`, `check::list_finds_added_and_changed_tests`, `check::list_matches_golden`, `check::outcome_pass_with_assert_and_compile_reds`, `check::report_matches_golden`, `check::appends_a_run_record`, `check::status_shows_the_check_line`, `check::close_needs_a_current_passing_check` |
| Negative | every failing outcome; uncommitted changes; no steps; no configuration; suite failure; bad `[check]` values | `check::outcome_no_red`, `check::outcome_not_green`, `check::outcome_unpaired_and_duplicate`, `check::outcome_no_tests_for_delete_only_red`, `check::outcome_red_changes_code`, `check::outcome_removed`, `check::refuses_uncommitted_changes`, `check::refuses_ticket_without_steps`, `check::refuses_without_configuration`, `check::suite_failure_fails_the_check`, `check::refusal_appends_no_record`, `core::config::check_section_is_validated` |
| Abuse | see the table below | `check::list_ignores_other_keys_and_prose`, `check::captured_output_cannot_fake_a_result`, `check::close_dialog_shows_the_check` |
| Robustness | a test that sleeps past `timeout_secs` (set to 10); a stale `Cargo.lock`; a leftover worktree from a killed check; a second check while one runs | `check::outcome_unverified_on_timeout_and_stale_lock`, `check::refuses_a_second_check`, `check::runs_in_a_worktree_and_leaves_the_tree_alone` |
| Environment | merge commits (first parent only); renamed test files; machine files changed after the check; other lines in `runs.jsonl` | `check::list_pairs_red_and_green_by_label`, `check::machine_files_keep_a_check_current`, `check::other_run_lines_are_ignored` |
| Repeated runs | two checks append two records; the newest is current | `check::appends_a_run_record` |
| Integration | `stapel init` in a cargo repository; `stapel check STP-3` on this repository (plan step 7) | `init::writes_check_for_cargo_repositories` |
| Process (drift) | abuse table and self-check; test lists; options in Inputs tables | `ticket_drift::abuse_table_and_self_check_are_required`, `ticket_drift::test_lists_must_agree`, `ticket_drift::every_option_is_in_an_inputs_table` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` of `check` | ticket key | an existing ticket | the only open ticket, as in STP-2 | unknown or legacy key: exit 1 | — | `check::refuses_ticket_without_steps` |
| `--list` | flag | — | — | — | — | `check::list_matches_golden` |
| `[check] runner` | string | `cargo` only | absent: `cargo` | anything else: configuration error | — | `core::config::check_section_is_validated` |
| `[check] timeout_secs` | integer | 10 / 86 400 | absent: 900 | 0, negative, over the bound, not an integer: configuration error | seconds | `core::config::check_section_is_validated` |
| Commit subjects | text | 10 000 commits read | no ticket commit: refused | a label with `RED:` twice is matched on the first `RED: ` after the key | — | `check::list_ignores_other_keys_and_prose` |
| Test output | bytes | captured up to 16 MiB per run, the rest discarded | no `running` line: `compile` or `unverified` | non-UTF-8: read lossily | — | `check::captured_output_cannot_fake_a_result` |
| `runs.jsonl` lines | JSON | STP-3 bounds | no record: `check: none` | unreadable line: STP-3 problem rules | — | `check::other_run_lines_are_ignored` |

### Abuse table

| Attempt | Outcome |
|---|---|
| A RED commit that also adds the code | `red-changes-code` with the paths |
| A RED that only deletes a test | `no-tests` |
| A test whose captured output prints `test x ... ok` | result lines are read only between `running <n> tests` and the first blank line or `failures:` (R-3) |
| A commit subject `STP-4: notes about step 1 RED: …` or another ticket's key | not a step (AC-1) |
| Editing code after a passing check, then closing | `check: stale`; close refuses |
| A forged `pass` line appended to `runs.jsonl` through Bash | not prevented (Guarantees); the write tools cannot write it (STP-1) |
| A `[check]` with a command line | unknown key: configuration error; no command text is read from files |
| A test that never finishes | killed at `timeout_secs`; `unverified` |
| Two checks at once fighting over the worktree | the second refuses |

**Author self-check (CLAUDE.md item 7).** To be done against this table before code review.

## Proof

Results come from the test runs recorded in `runs.jsonl`; this table names the tests only.

| Criterion or risk | Test |
|---|---|
| AC-1 | `check::list_pairs_red_and_green_by_label`, `check::list_ignores_other_keys_and_prose`, `check::list_finds_added_and_changed_tests`, `check::list_matches_golden` |
| AC-2, R-1, R-5 | `check::outcome_pass_with_assert_and_compile_reds`, `check::outcome_no_red`, `check::outcome_not_green`, `check::outcome_unpaired_and_duplicate`, `check::outcome_no_tests_for_delete_only_red`, `check::outcome_red_changes_code`, `check::outcome_removed`, `check::outcome_unverified_on_timeout_and_stale_lock` |
| AC-3, R-2, R-3 | `check::runs_in_a_worktree_and_leaves_the_tree_alone`, `check::refuses_uncommitted_changes`, `check::refuses_ticket_without_steps`, `check::refuses_a_second_check`, `check::suite_failure_fails_the_check`, `check::report_matches_golden`, `check::captured_output_cannot_fake_a_result` |
| AC-4 | `check::appends_a_run_record`, `check::refusal_appends_no_record`, `check::other_run_lines_are_ignored` |
| AC-5, R-4 | `check::status_shows_the_check_line`, `check::machine_files_keep_a_check_current`, `check::close_needs_a_current_passing_check`, `check::close_dialog_shows_the_check` |
| AC-6 | `core::config::check_section_is_validated`, `init::writes_check_for_cargo_repositories`, `check::refuses_without_configuration` |
| AC-7 | `ticket_drift::abuse_table_and_self_check_are_required`, `ticket_drift::test_lists_must_agree`, `ticket_drift::every_option_is_in_an_inputs_table` |

## Plan

Each step is a pair of commits, RED then GREEN, with `cargo fmt` before RED and `Cargo.lock` in the commit that
changes a manifest. Check command: `cargo test --workspace`, which includes the `ticket_drift` check. After each
GREEN commit the orchestrator runs a drift review of the step's diff against this ticket (CLAUDE.md item 11).

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 1 | Drift check: abuse table and self-check, test lists, options in Inputs tables | AC-7 | tests missing their checks |
| 2 | `[check]` in `stapel.toml` and in `init` | AC-6 | unknown section |
| 3 | `stapel check --list`: labels, pairing, step tests | AC-1 | subcommand missing |
| 4 | Runs in the worktree, the `cargo` runner, outcomes, report | AC-2, AC-3 | no runner |
| 5 | Run records in `runs.jsonl` | AC-4 | no record |
| 6 | `status` line, close gate and dialog line; `[check]` in this repository's `stapel.toml` (by the human) | AC-5 | no check line |
| 7 | After the build: `stapel check STP-3` and `stapel check STP-4` on this repository; findings recorded as data | manual | — |

## Review

Spec review before the build by the external reviewer; after the build, the three reviewers. Each finding gets
`catchable_at`.

## Summary

Generated after merge. Compared with STP-3: tokens by role, and findings by stage and `catchable_at`.
