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
- **HEAD.** Resolved to one sha when the command starts; every run and the record use it, the dirty check
  reads the working tree at that moment, and a `HEAD` that moved by the end is reported as a notice on stderr.
- **Step commit.** A commit on the first-parent history of HEAD whose subject starts with `<KEY> ` and then
  has a *marker*, the first ` RED: ` or ` GREEN: ` in the rest of the subject; the *label* is the text between
  `<KEY> ` and the marker, trimmed; it must be non-empty and labels compare byte for byte. Labels are listed
  oldest first, by the first of their step commits in history.
- **Step test.** A top-level function with the attribute `#[test]` (also inside a top-level `proptest! {` or
  `proptest::proptest! {`; doc comments count as comments; other test
  attributes such as `#[tokio::test]` are not step tests) in `crates/<dir>/tests/<file>.rs`; its identity is
  `<dir>/<file>::<name>`, printed so in lists, reports and records. Its *text* is the function from its
  attributes to its closing brace, found by brace depth outside literals, as tokens: comments dropped,
  whitespace between tokens collapsed, string, char and raw literals kept byte for byte.
- **Protected content of a step.** Its step tests; every file under `crates/<dir>/tests/` that is not a `.rs`
  file (golden files, fixtures), byte for byte as at the RED commit, for each `<dir>` that holds a step test;
  and every `.rs` file under `crates/<dir>/tests/` whose text without its `#[test]` functions (same token rules)
  the RED commit adds or changes.
- **Commit ids.** 40 hex digits (SHA-1) or 64 (SHA-256); the empty tree comes from `git hash-object -t tree`.
- **Dirty.** `git status --porcelain --untracked-files=all` lists a path (both sides of a rename) other than
  the machine files (`state.json`, `decisions.jsonl`, `findings.jsonl`, `runs.jsonl`, `tokens.jsonl` of any
  ticket); ignored files do not count.

| № | Criterion | Test |
|---|---|---|
| AC-1 | `stapel check [KEY] --list` · works without `[check]`, takes no lock, ignores a dirty tree, runs no test, exits 2 for a refusal (an unknown, legacy or unreadable ticket, a shallow clone, a git error, or a ticket without step commits), and prints per label its RED and GREEN commits and its step tests: those whose text the RED commit adds or changes (a file renamed with `git diff -M` keeps its tests, which count only when their text changes; a rename below git's similarity threshold is an added file); a subject without a marker, with an empty label or with another key is not a step commit; the layout is that of the golden file `check_list.txt`. | `check::list_pairs_red_and_green_by_label`, `check::list_ignores_other_keys_and_prose`, `check::list_finds_added_and_changed_tests`, `check::list_keeps_renamed_and_reformatted_tests`, `check::list_matches_golden`, `check::commit_bodies_cannot_inject_steps`, `check::sha256_repositories_work` |
| AC-2 | `stapel check` · gives each step the first outcome that holds, in this order, and runs no test for a step whose outcome comes from history alone (the first five): `unpaired` (a RED without a GREEN, a GREEN without a RED, or a GREEN before its RED); `duplicate` (two RED or two GREEN commits with one label); `no-tests` (the RED commit adds or changes no step test, a delete-only commit and a commit that changes only golden files or helpers included); `red-changes-code` (the RED commit changes a `.rs` file outside `crates/*/tests/`, `build.rs` included, a path other than `crates/*/tests/**`, `Cargo.toml` files, `Cargo.lock`, `.stapel/**`, `docs/**` and `*.md`, or a `Cargo.toml` in anything but its `dev-dependencies` tables, an added or deleted `Cargo.toml` included, with the first 20 paths named); `tests-changed` (a part of the step's protected content that still exists at HEAD was changed after the RED commit by a commit that is not a RED commit of the ticket); `unverified`, `no-red`, `not-green` and `removed` (AC-3); `pass`. A part of the protected content that a later RED commit of the ticket changed is shown as `superseded by <label>` (a step test then runs with its HEAD text); it counts as passing. A git error while reading history makes the check refuse with exit 2; paths are given to git literally. A step test deleted by a commit of the ticket (its subject starts with `<KEY> ` or `<KEY>:`) whose message contains `spec change:` is shown as `retired by <label>` and is not run; a step whose tests are all retired is `retired`, which counts as passing and is listed in the report. | `check::outcome_order_and_static_outcomes`, `check::outcome_unpaired_and_duplicate`, `check::outcome_no_tests_for_delete_only_red`, `check::outcome_red_changes_code`, `check::outcome_tests_changed_after_red`, `check::superseded_tests_are_shown`, `check::retired_tests_need_a_spec_change`, `check::untouched_golden_files_are_protected`, `check::glob_characters_in_paths_are_literal`, `check::superseded_golden_files_are_shown` |
| AC-3 | `stapel check` · runs a step's tests at its RED commit, its GREEN commit and HEAD, each with `cargo test --locked -p <package> --test <file> -- --exact <names>`, the package name read from `crates/<dir>/Cargo.toml` at that commit (a test whose file is gone at HEAD is `removed`; a manifest that is missing at a commit makes the step `unverified`); a run counts only when the output has exactly one `running <n> test` or `running <n> tests` line with `n` equal to the number of names, exactly one `test result:` line whose passed, failed and ignored counts equal those of the names' result lines, exactly one result line per name (`test <name> ... ok`, `... FAILED`, `... ignored` or `... ignored, <reason>`, each also with ` - should panic` after the name), and an exit code that agrees (0 with no failure, 101 with one); at RED a test *failed* when its line is `FAILED` (shown as `assert`) or when the output has `error: could not compile` for the step's test target and no `running` line (shown as `compile`); at GREEN and HEAD any `could not compile` means it did not pass; the step is `unverified` with the reason when a run does not count, on a timeout, a run that did not start, an out-of-date `Cargo.lock`, a `could not compile` at RED for another target or package, any other cargo error, or a step test `ignored` at RED or GREEN; then `no-red` when a step test passed at RED, `not-green` when one did not pass at GREEN, `removed` when one is missing or `ignored` at HEAD, or did not pass there. | `check::outcome_pass_with_assert_and_compile_reds`, `check::outcome_no_red`, `check::outcome_not_green`, `check::outcome_removed_or_ignored_at_head`, `check::outcome_unverified_on_timeout_and_stale_lock`, `check::outcome_unverified_on_ignored_or_unmatched_test`, `check::cargo_error_is_not_a_red`, `check::child_output_cannot_fake_a_result`, `check::output_parser_never_panics`, `check::a_single_pass_at_red_is_no_red` |
| AC-4 | `stapel check [KEY]` · runs nothing in the working tree: it holds an OS lock (`flock`) on `<git common dir>/stapel/check.lock`, which also holds its pid for the notice, replaces a leftover worktree of its own (no other worktree is pruned), uses one worktree at `<git common dir>/stapel/check-worktree`, created detached and removed at the end, also after a failure; every run gets `CARGO_TARGET_DIR=<git common dir>/stapel/check-target` and `CARGO_TERM_COLOR=never`, keeps the rest of the environment, and is killed with its whole process group after `timeout_secs` (per cargo run); its output is read for at most 2 s more after cargo ends, so a child in its own session that keeps the output open cannot hold the check; after the steps it runs the whole suite at `HEAD` with `cargo test --workspace --locked`; it refuses with exit 2 and runs nothing when `[check]` is missing, the clone is shallow, the tree is dirty, the ticket has no step commit or another check holds the lock (and on a git, worktree or lock error before the runs); it exits 1 when a step or the suite does not pass, and 0 otherwise. The layout of its report is that of the golden file `check_report.txt`. | `check::runs_in_a_worktree_and_leaves_the_tree_alone`, `check::refuses_a_dirty_tree`, `check::refuses_ticket_without_steps`, `check::refuses_a_second_check_and_takes_over_a_dead_lock`, `check::suite_failure_fails_the_check`, `check::report_matches_golden`, `check::other_worktrees_survive_a_check`, `check::shallow_clones_are_refused` |
| AC-5 | `stapel check` · appends one line to the ticket's `runs.jsonl` per run that was not refused, through the STP-3 journal (`v: 1`): `kind: "check"`, `id`, `at`, `ticket`, `head` (the full sha), `tool` (`stapel` and its version), `result` (`pass` or `fail`), per step (at most 200, then `steps_truncated: true`) its label (at most 200 bytes), RED and GREEN commits (full shas, or null), outcome, reason (at most 1024 bytes), the count of its step tests and, for `no-red`, `not-green`, `removed` and `tests-changed`, at most 20 of the names; a record over 60 KiB loses steps from its end and gets `steps_truncated: true`; and for the suite its outcome and the passed, failed and ignored counts of its `test result:` lines, summed with saturation; when the journal refuses the line, the check prints the reason and exits 1; lines without `kind: "check"` are kept and ignored. | `check::appends_a_run_record`, `check::refusal_appends_no_record`, `check::record_caps_long_lists`, `check::other_run_lines_are_ignored`, `check::record_names_at_most_twenty`, `check::record_fits_the_journal_line` |
| AC-6 | `stapel status` and `stapel close` · take the last well-formed `kind: "check"` line of the ticket's `runs.jsonl` in file order (well-formed: a `head` that is a commit id and a `result` of `pass` or `fail`); it is current when its `head` is a commit of the repository and an ancestor of HEAD (so a rewritten history is stale), `git diff --name-only --no-renames <head>` lists no path other than the machine files and the tree is not dirty; when git cannot read the tree, the line is `stale`, so the gate fails closed; `status` prints one line, for open and closed tickets alike (a closed ticket goes stale with later commits): `check: pass at <short sha>`, `check: fail at <short sha>`, `check: stale` with the reason (the first five changed paths, or an unknown commit; `stale` wins over `fail`), `check: none`, or `check: not configured`; with `[check]` configured, `close` refuses with exit 1 unless the current record is `pass`, naming the reason and `stapel check`, and its permission dialog shows the check line; without `[check]`, `close` does not ask for a check and its dialog says `check: not configured`. | `check::status_shows_the_check_line`, `check::machine_files_keep_a_check_current`, `check::unknown_head_is_stale`, `check::close_needs_a_current_passing_check`, `check::close_dialog_shows_the_check`, `check::a_record_needs_a_full_commit_id`, `check::close_refuses_a_failing_check`, `check::rewritten_history_is_stale`, `check::a_rename_after_the_check_is_stale` |
| AC-7 | `stapel.toml` · may have `[check]` with `runner = "cargo"` (the only runner) and `timeout_secs` from 10 to 86 400 (default 900); another runner, a value out of range or an unknown key is a configuration error of every command, as in STP-2; `stapel init` writes `[check]` with `runner = "cargo"` when the repository root has a `Cargo.toml`, and a commented example otherwise; `stapel.toml` stays denied to the write tools (STP-1). | `core::config::check_section_is_validated`, `init::writes_check_for_cargo_repositories`, `init::writes_commented_check_otherwise`, `check::refuses_without_configuration`, `hook::denies_machine_file_writes_even_with_build` |
| AC-8 | The `ticket_drift` check (CLAUDE.md items 5 and 9), for tickets not in its list of earlier tickets (STP-1, STP-2, STP-3), once every criterion is built (all tests it names exist, the STP-3 rule) or the ticket is closed · fails when a ticket has no risk tags line; when a ticket tagged `security` or `guard` has no heading that starts with `### Abuse`, or no line that starts with `**Author self-check` and carries a date `YYYY-MM-DD` and no `To be done`; when the tests named in its criteria and risks differ from those named in its Proof, or one of them is missing from its Test plan; and, for every ticket, when a long option of a `stapel` subcommand (from its `--help`) is named in no ticket's Inputs table, apart from the options listed in the check as older than this rule. | `ticket_drift::abuse_table_and_self_check_are_required`, `ticket_drift::test_lists_must_agree`, `ticket_drift::every_option_is_in_an_inputs_table` |

### Guarantees

- **Promised, against an agent that makes mistakes:** while `[check]` is configured, a ticket closes only
  when, for the code as it is, every step's tests failed at its RED commit and passed at its GREEN commit and
  at HEAD with the protected content they had at RED (or as rewritten by a later RED, which the report names), and the whole suite passes at `HEAD`; outcomes are computed from git
  history and real test runs, never from a stored claim; a later change of anything but machine files makes
  the check stale.
- **Not promised:** against an agent that hides code from the check on purpose: a GREEN that redirects a test
  target in a manifest (`[[test]]`, `harness = false`), `.cargo/config` runners or wrappers, `build.rs`, a
  `PATH` or cargo variables that pick another cargo or rustc, code placed under `crates/*/tests/`, or files of
  the allowed paths that code includes at compile time (code review E-1, E-4, E-5, E-7, E-13; a later ticket);
  helper code in `.rs` test files that the RED did not change (the drift check keeps its own code in a test
  file; moving it out is a later ticket, code review E-2, F-3); against an agent that appends a forged line to `runs.jsonl` or deletes `[check]` through
  Bash (the declared STP-1 hole; the write tools cannot write either file, and the close dialog shows
  `check: not configured`), or rewrites history; that a RED test fails for the right reason (a `compile` RED
  says only that the test file did not build, code review E-15); that the tests are good tests; flaky tests beyond one repeat of a
  passing RED; tests retired through `spec change:` (the report names them); tests nested in modules, test attributes other than `#[test]`, unit tests inside `crates/*/src`, targets with `harness = false`
  (their steps are `unverified`); steps merged from another branch (only the first-parent history is read, so
  the check runs on the ticket's branch before a merge); runners other than `cargo`; a check stopped by a signal
  (its cargo may keep running until it ends; the next check removes the worktree); a child that leaves the
  process group is not killed; the group kill uses the `kill` program, so without it only cargo is killed
  (code review F-7, E-10).

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

12. **Drift reviews after every GREEN step** (CLAUDE.md item 11): 54 findings over steps 1 to 7; nine of them
    were defects in the code or tests, fixed by the extra pairs in the Plan; the text changes above are a spec
    change (CLAUDE.md item 10), so spec, design and proof must be confirmed again by the human.

13. **Code review round 1** (38 findings): 11 fixed in code as agreed with the human on 2026-10-05; deliberate
    hiding of code from the check is not promised (Guarantees); helper protection stays as built because the
    drift check keeps its code in a test file; the rest are text changes. `Cargo.lock` needs no drift rule: every
    run uses `--locked`, so a manifest change without its lock file makes the step `unverified` (code review
    D-1; seen on STP-3 steps 6 and 6b).

### Out of scope

- Runners other than `cargo` (the comparison with klc on a TypeScript project needs one; a later ticket).
- Defences against an agent that hides code from the check on purpose (Guarantees); moving the drift check's
  code out of its test file.
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
| `git merge-base --is-ancestor A B` exits 0 when A is an ancestor of B; `git rev-parse --is-shallow-repository` prints `true` in a shallow clone; `git hash-object -t tree /dev/null` prints the empty tree of the repository's hash | git documentation | read |
| `git diff -M` detects renames at 50 % similarity by default; `--no-renames` lists both paths; a `:(literal)` pathspec matches the path as written | git documentation | read |
| `File::try_lock` takes an exclusive advisory lock (flock on Linux) that the OS releases when the process ends | Rust 1.89 std documentation | read |
| The group kill uses the `kill` program (`kill -KILL -- -<pgid>`), checked on a harmless `setsid sleep` | this machine, 2026-10-05 | verified |
| `.stapel/stapel.toml` and the five machine files are denied to the write tools | `crates/stapel-core/src/guard.rs`, STP-1 | verified |

### Decisions (as built)

- Step 1: process rules live in tests/ticket_drift.rs: `effective_lifecycle`, `process_problems`, `list_problems`, `stapel_options` (walks `--help` of every subcommand), `options_missing`; EARLIER_TICKETS and OLDER_OPTIONS lists. Applied in the main drift test only once the ticket is fully built or closed.
- Protected content: a `.rs` helper file is protected only when its text without `#[test]` functions changed in the RED (a RED that only adds tests to a file does not protect the file's other code). Needed because ticket_drift.rs holds both the drift code and its tests.
- Step 1 review: `full_problems` = STP-3 checks plus the AC-8 rules once built or closed. Inputs tables: every section whose heading starts with `### Inputs`, up to the next level-2 or level-3 heading; only table rows; the option is the first word of a backticked span (split at space or `=`). "To be done" is matched case-sensitively.
- Step 2: `starter_toml(prefix, cargo)` appends a two-line comment and `[check]` with `runner = "cargo"` and `timeout_secs = 900`, or the same three lines commented; `cargo` is `root/Cargo.toml` being a file (a symlink to a file counts, a directory does not). Every command reads stapel.toml through `Config::parse`, so a bad `[check]` is a configuration error everywhere. AC-7 should name the STP-1 guard test for "stapel.toml stays denied" (D2-2).
- Step 3: `stapel-core::rust_tests` (lexer: comments dropped, literals kept, raw strings, lifetimes vs chars; items scanned at top level and inside a top-level `proptest! {}`; a test is a function whose attribute run has exactly `#[test]`); `stapel-core::steps` (parse_subject, step_commits from `git log --first-parent --reverse` of HEAD, steps grouped by label, changes from `git diff --name-status -M -z <first parent or empty tree> <sha>`, step_tests: test functions of `crates/<dir>/tests/<file>.rs` whose token text is new or differs from the old path's). `--list` prints the tests of the first RED commit of a label. Without `--list` the command refuses (exit 2) until step 5.
- Step 3b: the fixture of `list_keeps_renamed_and_reformatted_tests` was too small for git's 50 % rename similarity (the code behaved as the ticket says: a rename below the threshold is an added file); fixed in a RED commit of its own, as STP-4 asks. Also amended before push: e-mail-like test identities replaced (the STP-3 private-data check found them).
- Step 3 review: the body of a test function is the first `{` outside parentheses and brackets (a `;` there means no body); an escaped char literal skips the quote, backslash and escaped character before looking for the closing quote. Any `#[test]` in the attribute run counts (`#[ignore]`, `#[should_panic]`, `#[cfg]` beside it do not matter); qualifiers pub/async/unsafe/const/extern are allowed; only the literal shape `proptest ! {` at top level is scanned (not `proptest::proptest!` or `proptest!( )`); doc comments are comments; adjacent punctuation is one token per character, so `&&` and `& &` are equal. `--list` takes the step tests of the first RED of a label; a missing commit prints `—`; refusals (unknown or legacy key, unreadable state, git failure, no step commits) exit 2.
- Step 4: `stapel-core::outcomes::analyse`: order unpaired (no RED, no GREEN, or first GREEN before first RED by first-parent position), duplicate, no-tests, red-changes-code (`build.rs` anywhere; `.rs` outside `crates/*/tests/`; `Cargo.toml` compared as TOML without `dev-dependencies` and `target.*.dev-dependencies`, an added or deleted manifest is code; allowed: `crates/*/tests/**`, `Cargo.lock`, `*.md`, `.stapel/**`, `docs/**`), tests-changed (units: step tests by token text, `.rs` files under tests whose helper text the RED changed, other files under tests byte for byte; commits after the RED that touch the path, first parent; a change by a ticket RED moves the baseline and marks a step test `superseded by <label>`; any other change of a unit that exists at HEAD is tests-changed; a test deleted by a commit whose message has `spec change:` and absent at HEAD is `retired by <label>` — the label of that commit, or its short sha); a step whose tests are all retired is `retired` (passing). Step 4b: a fixture fix (the code was right). The drift check now reads test functions with the same lexer; the lexer also scans `proptest::proptest! {`.
- Step 4 review: retirement needs a commit of the ticket (subject starts with `<KEY> ` or `<KEY>:`) whose message contains `spec change:` anywhere (case-sensitive); after any other deletion the walk goes on, so a re-added test is compared with its protected text. Any change after the RED by a commit that is not a RED of the ticket is `tests-changed`, even when a later commit restores the text (D4-3). A later RED moves the baseline of any protected part and adds a `superseded by` note (code review E-6); REDs of unpaired or duplicate steps also supersede (D4-4). Helpers and other files cannot be retired (D4-5). An added or deleted `Cargo.toml` is code (D4-9). Step 4 review 2: a target table that held only dev-dependencies is removed before the comparison.
- Step 5: `stapel-core::runner` (`run_cargo`: own process group, stdout and stderr read in threads up to 16 MiB each, `try_wait` every 50 ms, at the limit and after every run `kill -KILL -- -<pgid>`; `parse_run`: lock error first, then the `running` line, `could not compile` for `(test "<file>")` at RED as compile, any compile error at GREEN and HEAD as failing, exactly one running line and one summary, one result line per requested name, counts and exit code agree). `stapel-core::worktree` (`stapel_dir` = `<git common dir>/stapel`, `dirty_paths` from `git status --porcelain=v1 -z --untracked-files=all` without machine files, `Lock` = `File::try_lock` on `check.lock` holding the pid (the previous pid is read after the lock is held, code review F-8), `Worktree` at `check-worktree`: leftover removed, `git worktree add --force --detach` (no global prune, code review E-11), `checkout --force` and `clean -fdx` per commit, removed on drop). Refusal order: no `[check]`, dirty tree, no step commits, lock held. A step runs at RED, GREEN and HEAD (code review F-5: no second run at RED; any pass there is `no-red`); the first of unverified (any position; ignored at RED or GREEN), no-red, not-green, removed (missing at HEAD, ignored there or not passing) wins; pass reads `<n> test(s), RED <a> assert, <c> compile`. The suite reads `<p> passed, <f> failed, <i> ignored` summed over all summaries. A HEAD that moved is a notice on stderr. Step 5b: the CLI tests serialize full checks and give them `CARGO_BUILD_JOBS=2` (parallel nested builds crashed the machine); three fixture fixes.
- Step 5 review: output readers write into shared buffers; after cargo exits (or is killed) they get 2 s more, then what was read is used, so a detached grandchild (setsid) that keeps the pipe open cannot hang the check; that grandchild is not killed (Guarantees: not promised). Exit 2 also for an analysis, worktree or lock I/O error; a checkout failure mid-run makes that step or the suite unverified; the suite is unverified on a timeout, a lock error or a failed start, and fails on `error: could not compile`. A missing manifest at HEAD gives unverified (D5-1). The lock notice names the pid left in the file; the Drop truncation makes it a dead holder in practice (D5-4). A second RED run's result replaces the first (D5-7). `timeout_secs` is per cargo run (D5-8). The group kill uses the `kill` program (D5-10).
- Step 6: the record is built after the report and appended through `journal::append`; `id` is `c-` plus 12 hex digits; per step `label` (cut at 200 bytes on a char boundary), `red`, `green` (full shas or null), `outcome`, `reason` (cut at 1024 bytes), `tests` (the count of step tests), `names` (for no-red, not-green, removed and tests-changed: the names in the reason, at most 20); `steps_truncated: true` past 200 steps; `suite` with `outcome`, `passed`, `failed`, `ignored` (saturating sums). A journal refusal prints `stapel: the check record was not written: <reason>` and exits 1.
- Step 6 review: `reason` is cut at 1024 bytes; when the record is over 60 KiB, steps are dropped from the end and `steps_truncated` is set.
- Step 7: `stapel-core::checkstate::current`: the last well-formed `kind: "check"` line (with string `head` and `result`); `git cat-file -e <head>^{commit}` for an unknown commit; changed paths from `git diff --name-only -z <head>` plus the dirty paths, without machine files; stale prints `check: stale (changed: <first five>)` or `check: stale (unknown commit <sha>)`. `status` prints the line for open and closed tickets before the build line; `close` with `[check]` refuses unless pass: `close needs a current passing check (<line>); run \`stapel check <KEY>\` first`; the close dialog reason carries the line.
- Step 7 review: a record is well-formed with a 40-hex `head` and `result` pass or fail; a git error while reading the tree makes the line `check: stale (cannot read the tree: …)`, so the gate fails closed. Legacy and unreadable tickets show no check line; `close` refuses them before the gate (D7-5). Stale paths are sorted and deduplicated before the first five are named (D7-7).

- Code review round 1: `steps::step_commits` and `outcomes::touching` read `git log -z` with NUL between
  fields; `touching` returns git errors (the check refuses with exit 2) and passes `:(literal)<path>`; the
  protected content of a step adds every non-`.rs` file listed by `git ls-tree -r -z` under
  `crates/<dir>/tests/` at the RED for each crate of a step test; `checkstate::current` requires
  `git merge-base --is-ancestor <head> HEAD` (else `check: stale (history rewritten: …)`) and diffs with
  `--no-renames`; commit ids of 40 or 64 hex digits; the empty tree from `git hash-object -t tree /dev/null`;
  `git rev-parse --is-shallow-repository` refuses a shallow clone before anything else is read.

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
| Integration | the STP-1 guard on `stapel.toml` (`hook::denies_machine_file_writes_even_with_build`); `stapel init` in a cargo and a non-cargo repository; `stapel check STP-3` and `STP-4` on this repository (plan step 8, by hand; the CLI tests use a synthetic crate) | `init::writes_check_for_cargo_repositories`, `init::writes_commented_check_otherwise`, `check::runs_in_a_worktree_and_leaves_the_tree_alone` |
| Code review round 1 | rewritten history; a rename after the check; a golden file the RED did not change; git errors and literal paths; NUL separators; any pass at RED; a superseded golden file; no global prune; SHA-256; shallow clones | `check::rewritten_history_is_stale`, `check::a_rename_after_the_check_is_stale`, `check::untouched_golden_files_are_protected`, `check::glob_characters_in_paths_are_literal`, `check::commit_bodies_cannot_inject_steps`, `check::a_single_pass_at_red_is_no_red`, `check::superseded_golden_files_are_shown`, `check::other_worktrees_survive_a_check`, `check::sha256_repositories_work`, `check::shallow_clones_are_refused` |
| Not tested | the HEAD-moved notice (D5-9, D-6); symlinks, case-insensitive file systems and non-UTF-8 paths (read lossily); a missing `kill` program; the 16 MiB output cap | — |
| Process (drift) | abuse table and dated self-check; test lists; options in Inputs tables | `ticket_drift::abuse_table_and_self_check_are_required`, `ticket_drift::test_lists_must_agree`, `ticket_drift::every_option_is_in_an_inputs_table` |
| Step 3 drift review | a `;` in a signature; escaped char literals; raw strings, nested and doc comments, lifetimes, attribute runs, `proptest!`, nested and other test attributes; a dissimilar rename; a root commit; a dirty tree for `--list` | `rust_tests::signature_with_semicolon_keeps_the_body`, `rust_tests::escaped_char_literals_do_not_swallow_code`, `rust_tests::lexer_follows_the_terms`, `check::list_counts_a_dissimilar_rename_as_added`, `check::list_reads_a_root_commit_and_ignores_a_dirty_tree` |
| Step 4 drift review | retirement only by a commit of the ticket; a test deleted and added again weaker | `check::retirement_needs_a_ticket_commit`, `check::deleted_and_readded_tests_stay_protected`, `check::red_paths_allowed_and_capped` |
| Step 6 | at most 20 names in a record | `check::record_names_at_most_twenty` |
| Step 5 drift review | a detached grandchild that keeps the output open | `check::detached_grandchild_does_not_hang_the_check` |
| Step 6 drift review | many steps with long reasons fit one journal line | `check::record_fits_the_journal_line` |
| Step 7 drift review | a ref name instead of a commit id; close with a failing check | `check::a_record_needs_a_full_commit_id`, `check::close_refuses_a_failing_check` |
| Step 1 drift review | the rules wait until built; every Inputs table and only its rows; `security` alone; undated self-check; `####` is not the heading | `ticket_drift::process_rules_wait_until_built`, `ticket_drift::inputs_tables_are_read_in_full`, `ticket_drift::security_tag_and_undated_self_check` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` of `check` | ticket key | an existing ticket | the only open ticket, as in STP-2 | unknown or legacy key: exit 2 | — | `check::refuses_ticket_without_steps` |
| `--list` | flag | — | — | — | — | `check::list_matches_golden` |
| `[check] runner` | string | `cargo` only | absent: `cargo` | anything else: configuration error | — | `core::config::check_section_is_validated` |
| `[check] timeout_secs` | integer | 10 / 86 400 | absent: 900 | 0, negative, over the bound, not an integer: configuration error | seconds, per cargo run | `core::config::check_section_is_validated` |
| Commit subjects | text | the whole first-parent history | no step commit: exit 2 | no marker, empty label, other key: not a step commit | byte-exact labels after trimming | `check::list_ignores_other_keys_and_prose` |
| Steps and paths in a record | counts | 200 steps, 20 names or paths per step, 200-byte labels | — | more: cut, with `steps_truncated` | — | `check::record_caps_long_lists` |
| Test output | bytes | read up to 16 MiB per run, the rest discarded | no `running` line: `compile` only with `error: could not compile`, else `unverified` | non-UTF-8: read lossily | — | `check::output_parser_never_panics` |
| `runs.jsonl` lines | JSON | STP-3 bounds | no record: `check: none` | unreadable line: STP-3 problem rules; unknown `head`: `stale` | — | `check::other_run_lines_are_ignored`, `check::unknown_head_is_stale` |

### Abuse table

| Attempt | Outcome |
|---|---|
| A RED commit that also adds the code | `red-changes-code` with the paths |
| A RED that only deletes a test | `no-tests` |
| A GREEN that weakens the RED test (`assert!(true)`), or rewrites a golden file of the test's crate, or a helper the RED changed | `tests-changed` |
| Squashing or rebasing the steps after a passing check | the old head is no ancestor of HEAD: `check: stale` |
| A commit body with the bytes git log fields are split on | fields are split on NUL, which a message cannot hold |
| A later RED that rewrites an earlier step's test | that step is `superseded by <label>` in the report |
| A RED whose `Cargo.toml` points a library at a `.rs` file under the docs folder, or adds `build.rs` | `red-changes-code` |
| A later commit that edits a step test (adding `#[ignore]` included) | `tests-changed`, unless it is a RED of the ticket: then `superseded`, and an ignored test at HEAD is `removed` |
| A later commit that deletes a step test without `spec change:` | `removed` |
| A test that prints `test x ... ok` from a child process | the run does not count: two result lines for one name; `unverified` (R-3) |
| A RED whose cargo run fails offline or on a missing target | `unverified`, not a RED (R-6) |
| A subject `STP-4: notes about step 1 RED: …`, `STP-4 RED: x` or another ticket's key | not a step commit |
| Editing code, tests or ticket after a passing check, then closing (a rename included) | `check: stale`; close refuses |
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
| AC-1 | `check::list_pairs_red_and_green_by_label`, `check::list_ignores_other_keys_and_prose`, `check::list_finds_added_and_changed_tests`, `check::list_keeps_renamed_and_reformatted_tests`, `check::list_matches_golden`, `check::commit_bodies_cannot_inject_steps`, `check::sha256_repositories_work` |
| AC-2, R-1 | `check::outcome_order_and_static_outcomes`, `check::outcome_unpaired_and_duplicate`, `check::outcome_no_tests_for_delete_only_red`, `check::outcome_red_changes_code`, `check::outcome_tests_changed_after_red`, `check::superseded_tests_are_shown`, `check::retired_tests_need_a_spec_change`, `check::untouched_golden_files_are_protected`, `check::glob_characters_in_paths_are_literal`, `check::superseded_golden_files_are_shown` |
| AC-3, R-1, R-3, R-5, R-6 | `check::outcome_pass_with_assert_and_compile_reds`, `check::outcome_no_red`, `check::outcome_not_green`, `check::outcome_removed_or_ignored_at_head`, `check::outcome_unverified_on_timeout_and_stale_lock`, `check::outcome_unverified_on_ignored_or_unmatched_test`, `check::cargo_error_is_not_a_red`, `check::child_output_cannot_fake_a_result`, `check::output_parser_never_panics`, `check::a_single_pass_at_red_is_no_red` |
| AC-4, R-2 | `check::runs_in_a_worktree_and_leaves_the_tree_alone`, `check::refuses_a_dirty_tree`, `check::refuses_ticket_without_steps`, `check::refuses_a_second_check_and_takes_over_a_dead_lock`, `check::suite_failure_fails_the_check`, `check::report_matches_golden`, `check::other_worktrees_survive_a_check`, `check::shallow_clones_are_refused` |
| AC-5 | `check::appends_a_run_record`, `check::refusal_appends_no_record`, `check::record_caps_long_lists`, `check::other_run_lines_are_ignored`, `check::record_names_at_most_twenty`, `check::record_fits_the_journal_line` |
| AC-6, R-4 | `check::status_shows_the_check_line`, `check::machine_files_keep_a_check_current`, `check::unknown_head_is_stale`, `check::close_needs_a_current_passing_check`, `check::close_dialog_shows_the_check`, `check::a_record_needs_a_full_commit_id`, `check::close_refuses_a_failing_check`, `check::rewritten_history_is_stale`, `check::a_rename_after_the_check_is_stale` |
| AC-7 | `core::config::check_section_is_validated`, `init::writes_check_for_cargo_repositories`, `init::writes_commented_check_otherwise`, `check::refuses_without_configuration`, `hook::denies_machine_file_writes_even_with_build` |
| AC-8 | `ticket_drift::abuse_table_and_self_check_are_required`, `ticket_drift::test_lists_must_agree`, `ticket_drift::every_option_is_in_an_inputs_table` |

## Plan

Each step is a pair of commits, RED then GREEN (a fixture-only pair has an empty GREEN, and its fixes count as
`superseded`; tests that already passed went in commits of their own as coverage, not as REDs:
`ticket_drift::security_tag_and_undated_self_check`, `rust_tests::lexer_follows_the_terms`,
`check::list_counts_a_dissimilar_rename_as_added`, `check::list_reads_a_root_commit_and_ignores_a_dirty_tree`,
`check::refusal_appends_no_record`, `check::close_refuses_a_failing_check`), with `cargo fmt` before RED and `Cargo.lock` in the commit that
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
| 1 review | Drift review D1: the AC-8 rules wait until built; every Inputs table | AC-8 | rules applied to unbuilt tickets |
| 3b | Fixture: the renamed file is large enough for git's rename detection | AC-1 | fixture below the threshold |
| 3 review | Drift review D3: a `;` in a signature, an escaped quote char literal | AC-1 | body lost, code swallowed |
| 4b | Fixture: dev-dependencies placed after the build line | AC-2 | fixture moved the build line |
| 4 review | Drift review D4: retirement only by the ticket; a deleted and re-added test stays protected | AC-2 | foreign retirement, weakened re-add |
| 4 review 2 | Target tables holding only dev-dependencies | AC-2 | empty target table counted as code |
| 5b | Tests: full checks one at a time with two build jobs; three fixture fixes | AC-3, AC-4 | parallel builds exhausted memory |
| 5 review | Drift review D5: a detached grandchild cannot hang the check | AC-4 | check hangs |
| 6 review | Drift review D6: a record of many long steps fits one journal line | AC-5 | record lost |
| 7 review | Drift review D7: full commit ids only; the gate fails closed | AC-6 | ref names counted |
| R1 | Code review round 1: the 11 code fixes | AC-1 to AC-6 | see `findings.jsonl` |
| 8 | After the build: `stapel check STP-3` and `stapel check STP-4` on this repository; findings recorded as data | manual | — |

## Review

Spec review before the build by the external reviewer (round 1: 20 findings, round 2: 15 findings, applied); after the build, the
three reviewers. Each finding gets `catchable_at`.

## Summary

Generated after merge. Compared with STP-3: tokens by role, and findings by stage and `catchable_at`.
