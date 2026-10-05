# STP-4 — RED to GREEN check on the final commit

## Description

Source: `docs/PHASES.md`, phase 0, fourth ticket; phase-0 criterion 4: "for a plan step with a test command,
the tool distinguishes: the test did not fail before the change; failed and now passes; does not pass. The
check runs on the final commit." No external tracker.

Every plan step here is a pair of commits, RED (tests) then GREEN (code). Today nobody checks that the RED
tests really failed before the GREEN code: a test that passes from the start proves nothing, and klc, which
checks only the order of commits, accepted such tests (KLC-110). This ticket adds `stapel check`: it finds the
ticket's RED and GREEN commits, runs each step's tests at its RED commit (they must fail), at its GREEN commit
and at `HEAD` (they must pass), runs the whole suite at `HEAD`, and appends the result to `runs.jsonl`.
`stapel status` shows the result, and `stapel close` refuses while the current code has no passing check.

The first step adds the mechanical drift checks agreed after STP-3 (CLAUDE.md items 5 and 9).

**Risk tags:** `guard` (the close gate), `data` (a new record format in `runs.jsonl`), `interface` (a new
command, a new `status` line, a new `stapel.toml` section).

## Spec

### Acceptance criteria

Criteria state behaviour; exact output lives in golden files under the CLI tests' `golden` folder
(CLAUDE.md item 9). Terms used below:

- **Risk tags.** The first line of the Description that starts with `**Risk tags:**`; the tags are its
  backticked words.
- **HEAD.** Resolved to one sha when the command starts; every run, the dirty check and the record use it,
  and a `HEAD` that moved by the end is reported as a notice.
- **Step commit.** A commit on the first-parent history of HEAD whose subject starts with `<KEY> ` and then
  has a *marker*, the first ` RED: ` or ` GREEN: ` in the rest of the subject; the *label* is the text between
  `<KEY> ` and the marker, trimmed; it must be non-empty and labels compare byte for byte. Labels are listed
  oldest first, by the first of their step commits in history.
- **Step test.** A top-level function with the attribute `#[test]` (also inside `proptest!`; other test
  attributes such as `#[tokio::test]` are not step tests) in `crates/<dir>/tests/<file>.rs`; its identity is
  `<dir>/<file>::<name>`, printed so in lists, reports and records. Its *text* is the function from its
  attributes to its closing brace, found by brace depth outside literals, as tokens: comments dropped,
  whitespace between tokens collapsed, string, char and raw literals kept byte for byte.
- **Protected content of a step.** Its step tests, and every other path under `crates/<dir>/tests/` that the
  RED commit adds or changes: `.rs` files by their text without their `#[test]` functions (same token rules),
  other files (golden files, fixtures) byte for byte.
- **Dirty.** `git status --porcelain --untracked-files=all` lists a path (both sides of a rename) other than
  the machine files (`state.json`, `decisions.jsonl`, `findings.jsonl`, `runs.jsonl`, `tokens.jsonl` of any
  ticket); ignored files do not count.

| № | Criterion | Test |
|---|---|---|
| AC-1 | `stapel check [KEY] --list` · works without `[check]`, takes no lock, ignores a dirty tree, runs no test, exits 2 only for an unknown key or a ticket without step commits, and prints per label its RED and GREEN commits and its step tests: those whose text the RED commit adds or changes (a file renamed with `git diff -M` keeps its tests, which count only when their text changes; a rename below git's similarity threshold is an added file); a subject without a marker, with an empty label or with another key is not a step commit; the layout is that of the golden file `check_list.txt`. | `check::list_pairs_red_and_green_by_label`, `check::list_ignores_other_keys_and_prose`, `check::list_finds_added_and_changed_tests`, `check::list_keeps_renamed_and_reformatted_tests`, `check::list_matches_golden` |
| AC-2 | `stapel check` · gives each step the first outcome that holds, in this order, and runs no test for a step whose outcome comes from history alone (the first five): `unpaired` (a RED without a GREEN, a GREEN without a RED, or a GREEN before its RED); `duplicate` (two RED or two GREEN commits with one label); `no-tests` (the RED commit adds or changes no step test, a delete-only commit and a commit that changes only golden files or helpers included); `red-changes-code` (the RED commit changes a `.rs` file outside `crates/*/tests/`, `build.rs` included, a path other than `crates/*/tests/**`, `Cargo.toml` files, `Cargo.lock`, `.stapel/**`, `docs/**` and `*.md`, or a `Cargo.toml` in anything but its `dev-dependencies` tables, with the first 20 paths named); `tests-changed` (a part of the step's protected content that still exists at HEAD was changed after the RED commit by a commit that is not a RED commit of the ticket); `unverified`, `no-red`, `not-green` and `removed` (AC-3); `pass`. A step test whose text a later RED commit of the ticket changed runs with its HEAD text, and the step is shown as `superseded by <label>`; it counts as passing. A step test deleted by a commit of the ticket whose message contains `spec change:` is shown as `retired by <label>` and is not run; a step whose tests are all retired is `retired`, which counts as passing and is listed in the report. | `check::outcome_order_and_static_outcomes`, `check::outcome_unpaired_and_duplicate`, `check::outcome_no_tests_for_delete_only_red`, `check::outcome_red_changes_code`, `check::outcome_tests_changed_after_red`, `check::superseded_tests_are_shown`, `check::retired_tests_need_a_spec_change` |
| AC-3 | `stapel check` · runs a step's tests at its RED commit, its GREEN commit and HEAD, each with `cargo test --locked -p <package> --test <file> -- --exact <names>`, the package name read from `crates/<dir>/Cargo.toml` at that commit (a test whose crate directory is gone at HEAD is `removed`); a run counts only when the output has exactly one `running <n> test` or `running <n> tests` line with `n` equal to the number of names, exactly one `test result:` line whose passed, failed and ignored counts equal those of the names' result lines, exactly one result line per name (`test <name> ... ok`, `... FAILED`, `... ignored` or `... ignored, <reason>`, each also with ` - should panic` after the name), and an exit code that agrees (0 with no failure, 101 with one); at RED a test *failed* when its line is `FAILED` (shown as `assert`) or when the output has `error: could not compile` for the step's test target and no `running` line (shown as `compile`); at GREEN and HEAD any `could not compile` means it did not pass; the step is `unverified` with the reason when a run does not count, on a timeout, a run that did not start, an out-of-date `Cargo.lock`, a `could not compile` at RED for another target or package, any other cargo error, or a step test `ignored` at RED or GREEN; then `no-red` when a step test passed at RED twice in a row, `not-green` when one did not pass at GREEN, `removed` when one is missing or `ignored` at HEAD, or did not pass there. | `check::outcome_pass_with_assert_and_compile_reds`, `check::outcome_no_red`, `check::outcome_not_green`, `check::outcome_removed_or_ignored_at_head`, `check::outcome_unverified_on_timeout_and_stale_lock`, `check::outcome_unverified_on_ignored_or_unmatched_test`, `check::cargo_error_is_not_a_red`, `check::child_output_cannot_fake_a_result`, `check::output_parser_never_panics` |
| AC-4 | `stapel check [KEY]` · runs nothing in the working tree: it holds an OS lock (`flock`) on `<git common dir>/stapel/check.lock`, which also holds its pid for the notice, removes a leftover worktree, uses one worktree at `<git common dir>/stapel/check-worktree`, created detached and removed at the end, also after a failure; every run gets `CARGO_TARGET_DIR=<git common dir>/stapel/check-target` and `CARGO_TERM_COLOR=never`, keeps the rest of the environment, and is killed with its whole process group after `timeout_secs`; after the steps it runs the whole suite at `HEAD` with `cargo test --workspace --locked`; it refuses with exit 2 and runs nothing when `[check]` is missing, the tree is dirty, the ticket has no step commit or another check holds the lock; it exits 1 when a step or the suite does not pass, and 0 otherwise. The layout of its report is that of the golden file `check_report.txt`. | `check::runs_in_a_worktree_and_leaves_the_tree_alone`, `check::refuses_a_dirty_tree`, `check::refuses_ticket_without_steps`, `check::refuses_a_second_check_and_takes_over_a_dead_lock`, `check::suite_failure_fails_the_check`, `check::report_matches_golden` |
| AC-5 | `stapel check` · appends one line to the ticket's `runs.jsonl` per run that was not refused, through the STP-3 journal (`v: 1`): `kind: "check"`, `id`, `at`, `ticket`, `head` (the full sha), `tool` (`stapel` and its version), `result` (`pass` or `fail`), per step (at most 200, then `steps_truncated: true`) its label (at most 200 bytes), RED and GREEN commits, outcome, reason, the count of its tests and at most 20 failing or changed names, and for the suite its outcome and the counts of its `test result:` lines; when the journal refuses the line, the check prints the reason and exits 1; lines without `kind: "check"` are kept and ignored. | `check::appends_a_run_record`, `check::refusal_appends_no_record`, `check::record_caps_long_lists`, `check::other_run_lines_are_ignored` |
| AC-6 | `stapel status` and `stapel close` · take the last well-formed `kind: "check"` line of the ticket's `runs.jsonl` in file order; it is current when its `head` is a commit of the repository, `git diff --name-only <head>` lists no path other than the machine files and the tree is not dirty; `status` prints one line: `check: pass at <short sha>`, `check: fail at <short sha>`, `check: stale` with the reason (the first five changed paths, or an unknown commit; `stale` wins over `fail`), `check: none`, or `check: not configured`; with `[check]` configured, `close` refuses with exit 1 unless the current record is `pass`, naming the reason and `stapel check`, and its permission dialog shows the check line; without `[check]`, `close` does not ask for a check and its dialog says `check: not configured`. | `check::status_shows_the_check_line`, `check::machine_files_keep_a_check_current`, `check::unknown_head_is_stale`, `check::close_needs_a_current_passing_check`, `check::close_dialog_shows_the_check` |
| AC-7 | `stapel.toml` · may have `[check]` with `runner = "cargo"` (the only runner) and `timeout_secs` from 10 to 86 400 (default 900); another runner, a value out of range or an unknown key is a configuration error of every command, as in STP-2; `stapel init` writes `[check]` with `runner = "cargo"` when the repository root has a `Cargo.toml`, and a commented example otherwise; `stapel.toml` stays denied to the write tools (STP-1). | `core::config::check_section_is_validated`, `init::writes_check_for_cargo_repositories`, `init::writes_commented_check_otherwise`, `check::refuses_without_configuration` |
| AC-8 | The `ticket_drift` check (CLAUDE.md items 5 and 9), for tickets not in its list of earlier tickets (STP-1, STP-2, STP-3), once every criterion is built (all tests it names exist, the STP-3 rule) or the ticket is closed · fails when a ticket has no risk tags line; when a ticket tagged `security` or `guard` has no heading that starts with `### Abuse`, or no line that starts with `**Author self-check` and carries a date `YYYY-MM-DD` and no `To be done`; when the tests named in its criteria and risks differ from those named in its Proof, or one of them is missing from its Test plan; and, for every ticket, when a long option of a `stapel` subcommand (from its `--help`) is named in no ticket's Inputs table, apart from the options listed in the check as older than this rule. | `ticket_drift::abuse_table_and_self_check_are_required`, `ticket_drift::test_lists_must_agree`, `ticket_drift::every_option_is_in_an_inputs_table` |

### Guarantees

- **Promised, against an agent that makes mistakes:** while `[check]` is configured, a ticket closes only
  when, for the code as it is, every step's tests failed at its RED commit and passed at its GREEN commit and
  at HEAD with the protected content they had at RED (or as rewritten by a later RED, which the report names), and the whole suite passes at `HEAD`; outcomes are computed from git
  history and real test runs, never from a stored claim; a later change of anything but machine files makes
  the check stale.
- **Not promised:** against an agent that appends a forged line to `runs.jsonl` or deletes `[check]` through
  Bash (the declared STP-1 hole; the write tools cannot write either file, and the close dialog shows
  `check: not configured`), or rewrites history; that a RED test fails for the right reason (a `compile` RED
  says only that the test needed new code); that the tests are good tests; flaky tests beyond one repeat of a
  passing RED; tests retired through `spec change:` (the report names them); tests nested in modules, test attributes other than `#[test]`, unit tests inside `crates/*/src`, targets with `harness = false`
  (their steps are `unverified`); steps merged from another branch (only the first-parent history is read, so
  the check runs on the ticket's branch before a merge); runners other than `cargo`.

### Questions

All answered on 2026-10-05 by the human ("давай попробуем": as recommended), except those marked.

1. **How RED is checked.** In a detached worktree at the RED commit; then at GREEN and at `HEAD`; the whole
   suite at `HEAD`. Answer: as recommended.
2. **Which tests belong to a step.** Those the RED commit adds or changes, found from its diff; no plan
   column. Answer: as recommended.
3. **A RED that only fails to compile.** Accepted and shown as `compile`. Answer: as recommended.
4. **Does the check block the close.** Yes, when `[check]` is configured. Answer: as recommended.
5. **Repeats and time.** A passing RED test runs once more; everything else once; one time limit per run in
   `stapel.toml`. Answer: as recommended (900 s rather than the 300 s first proposed: the whole suite of this
   repository with a cold build takes longer).
6. **Earlier tickets.** `stapel check STP-3` on this repository is the integration run; what it finds is data,
   the old tickets are not changed. Answer: as recommended.
7. **Fixed commands, no shell** (orchestrator, from klc KLC-174 F-001). The runner builds the `cargo`
   argument list itself; `stapel.toml` cannot hold a command line.
8. **Freshness by content, not time** (orchestrator, from klc KLC-174 F-002, F-003). A record names a commit;
   it is current while nothing but machine files differs from it, so committing `runs.jsonl` does not make it
   stale.
9. **No `[check]`, no gate** (orchestrator). Repositories without `cargo` and the CLI tests of other commands
   work as before; the close dialog says so.
10. **Spec review round 1** (20 findings, applied by the orchestrator): test text is part of a step test's
    identity (`tests-changed`), an explicit outcome order, a strict output parser, `retired` tests through
    `spec change:`, refusals exit 2. The human may change any of these.
11. **Spec review round 2** (15 findings, applied by the orchestrator): protected content includes golden
    files and helpers, `superseded` and `retired` are shown, an explicit table of `ignored` and `could not
    compile` by position, test identity with the crate, HEAD resolved once, an OS lock.

### Out of scope

- Runners other than `cargo` (the comparison with klc on a TypeScript project needs one; a later ticket).
- Tests nested in modules, test attributes other than `#[test]`, unit tests inside `crates/*/src`,
  `harness = false` targets.
- A RED that changes only a golden file or a helper is `no-tests`: such a RED also changes the test that reads
  the file (CLAUDE.md item 9 puts exact output in golden files, so this is common and stated here).
- Checking a single step during the build (`--step`); the per-step drift review of CLAUDE.md item 11 is done
  by the orchestrator by hand in phase 0.
- Signing or hashing records against forgery.
- Running steps in parallel; caching results between checks.
- Tickets without code steps (a documentation-only ticket cannot be closed while `[check]` is configured).
- Steps merged in through a merge commit or squashed; checking on `main` after such a merge.

## Design

### What klc does, and what we take

From a study of `../klc` (token journal entry `research`, 2026-10-05):

| klc | Take / do differently | Why |
|---|---|---|
| RED→GREEN checked as commit order by changed paths only; the test is never run at RED (KLC-039 chose this; its HIGH "true was-red verification" was left unfixed) | Run the step's tests at the RED commit | The phase-0 criterion: "did not fail before the change" is invisible otherwise (KLC-110: vacuous REDs) |
| Steps found by subject `<KEY> step-N`, with a guard so `step-1` does not match `step-10` | Labels matched as the whole text between key and marker | Same defect, and code review rounds are steps too |
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
| `cargo test --test <file> -- --exact <a> <b>` runs only tests named exactly `a` and `b` of that target; a top-level integration test's name is its bare function name, a nested one `mod::name`; a filter matching nothing prints `running 0 tests` and exits 0 | cargo 1.93.1 on this repository and a scratch crate, 2026-10-05 | verified |
| A run prints `running 1 test` or `running <n> tests`, one line per test: `test <name> ... ok`, `test <name> - should panic ... ok`, `test <name> ... FAILED`, `test <name> ... ignored, <reason>`; then `failures:` sections with captured output, then `test result: <ok\|FAILED>. <p> passed; <f> failed; <i> ignored; …`; with `-q` the format differs, so `-q` is not used | same | verified |
| Output of a child process started by a test is not captured and appears among the result lines (a child printing `test b ... ok` did) | same | verified |
| A failing test or a compile error exits 101; a compile error prints `error: could not compile` and no `running` line for that target | same | verified |
| `--locked` with an out-of-date `Cargo.lock` exits 101 with `cannot update the lock file … because --locked was passed` | same | verified |
| `git worktree add --detach <path> <commit>` creates a checkout sharing the object store; `git worktree remove --force` removes it; the main working tree is not touched | git, scratch repository, 2026-10-05 | verified |
| `cargo test --workspace --test <file>` selects the package that has the target; `-p` makes it exact | this repository, 2026-10-05 | verified |
| Cargo reuses compiled dependencies through a shared `CARGO_TARGET_DIR`; workspace crates rebuild when their path changes, so one fixed worktree path keeps builds incremental | cargo documentation (fingerprints include the path) | read |
| `git status --porcelain --untracked-files=all` lists tracked changes and untracked files that are not ignored | git documentation | read |
| `.stapel/stapel.toml` and the five machine files are denied to the write tools | `crates/stapel-core/src/guard.rs`, STP-1 | verified |

### Decisions (as built)

To be filled by the GREEN commits: the parser of test functions and their text, the worktree, lock and
process-group handling, the output parser of the `cargo` runner.

### Risks

| Risk | Test |
|---|---|
| R-1 A test that never failed, or was rewritten after RED, is accepted | `check::outcome_no_red`, `check::outcome_tests_changed_after_red` |
| R-2 The check runs in or changes the person's working tree | `check::runs_in_a_worktree_and_leaves_the_tree_alone` |
| R-3 Output that looks like a result line changes an outcome | `check::child_output_cannot_fake_a_result` |
| R-4 A check of other code is taken as current | `check::close_needs_a_current_passing_check`, `check::machine_files_keep_a_check_current`, `check::unknown_head_is_stale` |
| R-5 A hung test blocks the check for ever | `check::outcome_unverified_on_timeout_and_stale_lock` |
| R-6 A build or fetch error is taken as a failing RED | `check::cargo_error_is_not_a_red` |

## Test plan

CLI tests build a tiny `cargo` crate without dependencies in a temporary git repository and commit RED and
GREEN steps into it; each test sets its own `CARGO_TARGET_DIR`.

| Category | Cases | Tests |
|---|---|---|
| Main path | list and check a ticket with assert and compile REDs; report and list against golden files; status line; close after a passing check | `check::list_pairs_red_and_green_by_label`, `check::list_finds_added_and_changed_tests`, `check::list_matches_golden`, `check::outcome_pass_with_assert_and_compile_reds`, `check::report_matches_golden`, `check::appends_a_run_record`, `check::status_shows_the_check_line`, `check::close_needs_a_current_passing_check` |
| Negative | every failing outcome and their order; dirty tree; no steps; no configuration; suite failure; bad `[check]` values | `check::outcome_order_and_static_outcomes`, `check::outcome_no_red`, `check::outcome_not_green`, `check::outcome_unpaired_and_duplicate`, `check::outcome_no_tests_for_delete_only_red`, `check::outcome_red_changes_code`, `check::outcome_tests_changed_after_red`, `check::outcome_removed_or_ignored_at_head`, `check::superseded_tests_are_shown`, `check::retired_tests_need_a_spec_change`, `check::refuses_a_dirty_tree`, `check::refuses_ticket_without_steps`, `check::refuses_without_configuration`, `check::suite_failure_fails_the_check`, `check::refusal_appends_no_record`, `core::config::check_section_is_validated` |
| Abuse | see the table below | `check::list_ignores_other_keys_and_prose`, `check::child_output_cannot_fake_a_result`, `check::close_dialog_shows_the_check`, `check::cargo_error_is_not_a_red` |
| Robustness | a test that sleeps past `timeout_secs` (10) and starts a sleeping child; a stale `Cargo.lock`; an ignored or unmatched step test; a dead lock and a leftover worktree; a second check while one runs; a ticket with 250 steps and a RED with 30 code paths; property test: the output parser never panics and stays fast on arbitrary bytes | `check::outcome_unverified_on_timeout_and_stale_lock`, `check::outcome_unverified_on_ignored_or_unmatched_test`, `check::refuses_a_second_check_and_takes_over_a_dead_lock`, `check::record_caps_long_lists`, `check::output_parser_never_panics` |
| Environment | merge commits (first parent only); a renamed test file and a reformatted neighbour test; machine files changed after the check; an unknown `head`; other lines in `runs.jsonl` | `check::list_pairs_red_and_green_by_label`, `check::list_keeps_renamed_and_reformatted_tests`, `check::machine_files_keep_a_check_current`, `check::unknown_head_is_stale`, `check::other_run_lines_are_ignored` |
| Repeated runs | two checks append two records; the last is current | `check::appends_a_run_record` |
| Integration | `stapel init` in a cargo and a non-cargo repository; `stapel check STP-3` and `STP-4` on this repository (plan step 7) | `init::writes_check_for_cargo_repositories`, `init::writes_commented_check_otherwise`, `check::runs_in_a_worktree_and_leaves_the_tree_alone` |
| Process (drift) | abuse table and dated self-check; test lists; options in Inputs tables | `ticket_drift::abuse_table_and_self_check_are_required`, `ticket_drift::test_lists_must_agree`, `ticket_drift::every_option_is_in_an_inputs_table` |
| Step 3 drift review | a `;` in a signature; escaped char literals; raw strings, nested and doc comments, lifetimes, attribute runs, `proptest!`, nested and other test attributes; a dissimilar rename; a root commit; a dirty tree for `--list` | `rust_tests::signature_with_semicolon_keeps_the_body`, `rust_tests::escaped_char_literals_do_not_swallow_code`, `rust_tests::lexer_follows_the_terms`, `check::list_counts_a_dissimilar_rename_as_added`, `check::list_reads_a_root_commit_and_ignores_a_dirty_tree` |
| Step 4 drift review | retirement only by a commit of the ticket; a test deleted and added again weaker | `check::retirement_needs_a_ticket_commit`, `check::deleted_and_readded_tests_stay_protected`, `check::red_paths_allowed_and_capped` |
| Step 6 | at most 20 names in a record | `check::record_names_at_most_twenty` |
| Step 5 drift review | a detached grandchild that keeps the output open | `check::detached_grandchild_does_not_hang_the_check` |
| Step 6 drift review | many steps with long reasons fit one journal line | `check::record_fits_the_journal_line` |
| Step 1 drift review | the rules wait until built; every Inputs table and only its rows; `security` alone; undated self-check; `####` is not the heading | `ticket_drift::process_rules_wait_until_built`, `ticket_drift::inputs_tables_are_read_in_full`, `ticket_drift::security_tag_and_undated_self_check` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` of `check` | ticket key | an existing ticket | the only open ticket, as in STP-2 | unknown or legacy key: exit 2 | — | `check::refuses_ticket_without_steps` |
| `--list` | flag | — | — | — | — | `check::list_matches_golden` |
| `[check] runner` | string | `cargo` only | absent: `cargo` | anything else: configuration error | — | `core::config::check_section_is_validated` |
| `[check] timeout_secs` | integer | 10 / 86 400 | absent: 900 | 0, negative, over the bound, not an integer: configuration error | seconds | `core::config::check_section_is_validated` |
| Commit subjects | text | the whole first-parent history | no step commit: exit 2 | no marker, empty label, other key: not a step commit | byte-exact labels after trimming | `check::list_ignores_other_keys_and_prose` |
| Steps and paths in a record | counts | 200 steps, 20 names or paths per step, 200-byte labels | — | more: cut, with `steps_truncated` | — | `check::record_caps_long_lists` |
| Test output | bytes | read up to 16 MiB per run, the rest discarded | no `running` line: `compile` only with `error: could not compile`, else `unverified` | non-UTF-8: read lossily | — | `check::output_parser_never_panics` |
| `runs.jsonl` lines | JSON | STP-3 bounds | no record: `check: none` | unreadable line: STP-3 problem rules; unknown `head`: `stale` | — | `check::other_run_lines_are_ignored`, `check::unknown_head_is_stale` |

### Abuse table

| Attempt | Outcome |
|---|---|
| A RED commit that also adds the code | `red-changes-code` with the paths |
| A RED that only deletes a test | `no-tests` |
| A GREEN that weakens the RED test (`assert!(true)`), or rewrites the golden file or helper it reads | `tests-changed` |
| A later RED that rewrites an earlier step's test | that step is `superseded by <label>` in the report |
| A RED whose `Cargo.toml` points a library at a `.rs` file under the docs folder, or adds `build.rs` | `red-changes-code` |
| A later commit that marks a step test `#[ignore]` or deletes it without `spec change:` | `removed` |
| A test that prints `test x ... ok` from a child process | the run does not count: two result lines for one name; `unverified` (R-3) |
| A RED whose cargo run fails offline or on a missing target | `unverified`, not a RED (R-6) |
| A subject `STP-4: notes about step 1 RED: …`, `STP-4 RED: x` or another ticket's key | not a step commit |
| Editing code, tests or ticket after a passing check, then closing | `check: stale`; close refuses |
| A forged `pass` line appended to `runs.jsonl`, or `[check]` deleted, through Bash | not prevented (Guarantees); the write tools cannot write either file; the dialog shows the check line |
| A `[check]` with a command line | unknown key: configuration error; no command text is read from files |
| A test that never finishes, or leaves a child running | the process group is killed at `timeout_secs`; `unverified` |
| A test that leaves a child in its own session holding the output open | the readers stop 2 s after cargo exits; the child itself is not killed (step 5 drift review D5-2) |
| Two checks at once fighting over the worktree, or a pid reused after a crash | the OS lock decides; the second refuses with exit 2 |
| A commit landing in the main tree while a check runs | runs and record use the HEAD resolved at the start; a notice names the move |

**Author self-check (CLAUDE.md item 7).** Done on 2026-10-05 against this table, before code review: every row has
a test (`check::outcome_red_changes_code`, `check::outcome_no_tests_for_delete_only_red`,
`check::outcome_tests_changed_after_red`, `check::superseded_tests_are_shown`, `check::red_paths_allowed_and_capped`,
`check::outcome_removed_or_ignored_at_head`, `check::child_output_cannot_fake_a_result`, `check::cargo_error_is_not_a_red`,
`check::list_ignores_other_keys_and_prose`, `check::close_needs_a_current_passing_check`,
`core::config::check_section_is_validated`, `check::outcome_unverified_on_timeout_and_stale_lock`,
`check::detached_grandchild_does_not_hang_the_check`, `check::refuses_a_second_check_and_takes_over_a_dead_lock`)
except the forged line (not promised) and the HEAD-moved notice (untested, D5-9 accepted). The detached grandchild
row was added from D5-2; nothing else new.

## Proof

Results come from the test runs recorded in `runs.jsonl`; this table names the tests only.

| Criterion or risk | Test |
|---|---|
| AC-1 | `check::list_pairs_red_and_green_by_label`, `check::list_ignores_other_keys_and_prose`, `check::list_finds_added_and_changed_tests`, `check::list_keeps_renamed_and_reformatted_tests`, `check::list_matches_golden` |
| AC-2, R-1 | `check::outcome_order_and_static_outcomes`, `check::outcome_unpaired_and_duplicate`, `check::outcome_no_tests_for_delete_only_red`, `check::outcome_red_changes_code`, `check::outcome_tests_changed_after_red`, `check::superseded_tests_are_shown`, `check::retired_tests_need_a_spec_change` |
| AC-3, R-1, R-3, R-5, R-6 | `check::outcome_pass_with_assert_and_compile_reds`, `check::outcome_no_red`, `check::outcome_not_green`, `check::outcome_removed_or_ignored_at_head`, `check::outcome_unverified_on_timeout_and_stale_lock`, `check::outcome_unverified_on_ignored_or_unmatched_test`, `check::cargo_error_is_not_a_red`, `check::child_output_cannot_fake_a_result`, `check::output_parser_never_panics` |
| AC-4, R-2 | `check::runs_in_a_worktree_and_leaves_the_tree_alone`, `check::refuses_a_dirty_tree`, `check::refuses_ticket_without_steps`, `check::refuses_a_second_check_and_takes_over_a_dead_lock`, `check::suite_failure_fails_the_check`, `check::report_matches_golden` |
| AC-5 | `check::appends_a_run_record`, `check::refusal_appends_no_record`, `check::record_caps_long_lists`, `check::other_run_lines_are_ignored` |
| AC-6, R-4 | `check::status_shows_the_check_line`, `check::machine_files_keep_a_check_current`, `check::unknown_head_is_stale`, `check::close_needs_a_current_passing_check`, `check::close_dialog_shows_the_check` |
| AC-7 | `core::config::check_section_is_validated`, `init::writes_check_for_cargo_repositories`, `init::writes_commented_check_otherwise`, `check::refuses_without_configuration` |
| AC-8 | `ticket_drift::abuse_table_and_self_check_are_required`, `ticket_drift::test_lists_must_agree`, `ticket_drift::every_option_is_in_an_inputs_table` |

## Plan

Each step is a pair of commits, RED then GREEN, with `cargo fmt` before RED and `Cargo.lock` in the commit that
changes a manifest. Check command: `cargo test --workspace`, which includes the `ticket_drift` check. After each
GREEN commit the orchestrator runs a drift review of the step's diff against this ticket (CLAUDE.md item 11).

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 1 | Drift check: abuse table and dated self-check, test lists, options in Inputs tables | AC-8 | tests missing their checks |
| 2 | `[check]` in `stapel.toml` and in `init` | AC-7 | unknown section |
| 3 | `stapel check --list`: step commits, labels, step tests and their text | AC-1 | subcommand missing |
| 4 | Outcomes from history alone and their order | AC-2 | no outcomes |
| 5 | The worktree, lock and `cargo` runner with its strict output parser; run outcomes; report | AC-3, AC-4 | no runner |
| 6 | Run records in `runs.jsonl` | AC-5 | no record |
| 7 | `status` line, close gate and dialog line; `[check]` in this repository's `stapel.toml` (by the human) | AC-6 | no check line |
| 8 | After the build: `stapel check STP-3` and `stapel check STP-4` on this repository; findings recorded as data | manual | — |

## Review

Spec review before the build by the external reviewer (round 1: 20 findings, round 2: 15 findings, applied); after the build, the
three reviewers. Each finding gets `catchable_at`.

## Summary

Generated after merge. Compared with STP-3: tokens by role, and findings by stage and `catchable_at`.
