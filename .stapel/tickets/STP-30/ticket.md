# STP-30 — STP-28 follow-ups: base merge, script path, report reader

## Description

Source: `docs/PHASES.md`, stage A item A1c; the deferred findings of STP-28 (ledger 2026-11-12).

### Frame

**Problem.** STP-28 closed with ten deferred findings. A merge of the base branch that brings a fixture
into a step test folder gives `tests-changed`, so a ticket cannot close after it merges main. An absolute
script path in `command` runs the working tree's script, not the one at the step's commit. The JUnit
reader accepts some malformed reports and refuses some valid ones. A broken `stapel.toml` silently makes a
junit ticket a cargo ticket. Some paths of the base merge and the cargo fixtures have no test.

**Plan item:** A1c (follow-up of STP-28, before A2).

**Risk tags:** `guard` (the check is the RED/GREEN boundary; each defect lets a wrong outcome pass or blocks
a correct close).

**Non-goals.** New runners or report formats; `stapel init` on a foreign repository (A2); a change of the
`base..HEAD` walk beyond the base merge; deliberate circumvention of the check.

**Promise (closed list).**

1. A file that only a merge of the base branch brings into a step test folder (its content and mode at
   the merge equal those at the merge's second parent) gives no `tests-changed`, under both runners; a
   mode change with an equal blob is no longer taken as equal (CR2-4, d20-2).
2. An absolute first word of `command` under the checkout root runs as it is at the step's commit, as a
   relative one does (CR2-5).
3. The JUnit reader refuses an unknown root, a second root and CDATA outside the root, each with its
   cause, and skips the content of unknown elements without decoding it (CR2-7, CR2-11).
4. When `stapel.toml` is a file that fails to load, `stapel check --history`, `stapel lint` and `stapel
   next` fail with the load fault and the commit hook prints the fault and lets the commit pass; none falls
   back to cargo; the root is found as before (CR2-8).
5. A report path that is a link, folder, FIFO, socket or device is not opened and is named by its kind;
   the 16 MiB bound of a regular report holds as before (d21-1, d21-2).
6. Tests for the base merge: build inputs of the cargo runner (`Cargo.toml`, `build.rs`, a
   toolchain file, `.cargo/config`), a mode-only or symlink change with an equal blob, a ticket whose base
   branch merges another branch, a deleted base branch ref, an octopus merge, a rename with mixed paths, a
   merge that brings one equal and one resolved input; a fixture added, deleted or changed beside a cargo
   step test (d20-2, d20-3, d22-1).

Not promised: other findings of STP-28; reports from tools other than jest-junit; a base branch that is not
a local ref.

**Size.** About 300 source lines and 800 test lines, 11 Plan steps. Route: full (tag `guard`), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-12: Decision (script path, the human, recommended): an absolute first word of `command` inside
  the repository is mapped into the check copy and runs as it is at the step's commit, as a relative path
  does. Rejected: refusing such a config with a message to use a relative path.
- 2026-11-12: Decision (broken config, the human, recommended): `stapel check --history` fails with the
  load fault; the commit hook prints the fault and lets the commit pass, so a fix of `stapel.toml` can be
  committed. Rejected: both refuse.
- 2026-11-12: Decision (base merge, the human, recommended): only a file whose content at the merge equals
  its content at the merge's second parent gives no `tests-changed`, the rule STP-28 uses for build inputs.
  Rejected: every file the merge brings (a test edit could hide in a merge).
- 2026-11-12: Decision (base edit, the human, recommended, SR1-14): a step test or fixture edited on the
  base branch by mistake and brought by a Base merge is not promised, as the STP-28 side-branch case.
  Rejected: freeing a unit only when the second parent did not change it since the ticket's RED.
- 2026-11-12: Assumption (missing config, SR1-1): an absent `stapel.toml` keeps today's behaviour; only a
  file that exists and fails to parse or validate is a load fault.
- 2026-11-12: Decision (Cargo.lock, the human, recommended, step 4): `Cargo.lock` is dropped from AC-14 and
  promise item 6; it is no build input in the code, also outside a merge, and `--locked` fails a stale lock
  file. Rejected: making it a build input (widens the promise; a dependency update would block a step).
- 2026-11-12: Decision (root lookup, the human, recommended, d8-3, d9-1): the root is found as before, by a
  regular `.stapel/stapel.toml`; a folder or a dangling link in its place gives today's refusal and leaves
  the hooks silent. Rejected: any entry marks the root for every command and hook (a broken nested entry
  would hide a valid outer root).
- 2026-11-12: Decision (check cadence, the human, ledger): `stapel check` runs once at the end of the
  build, before code review.
- 2026-11-12: Assumption (script root, SR1-9): "under the root" is a lexical prefix test against the root
  of the checkout where the check runs, for a path without `.`, `..` or empty parts.
- 2026-11-12: Assumption (config fault, SR2-1, SR2-4): `stapel next` fails with the load fault as `check
  --history` does; in the hook the fault takes the place of every step line.

## Spec

Terms (the others as in STP-28):

- **Base merge**: a merge commit of the Walk with exactly two parents whose second parent is an ancestor
  of `refs/heads/<base_branch>` (`base_branch` of `state.json`).
- **Protected unit**: what STP-4, STP-13 and STP-28 AC-27 and AC-42 guard with `tests-changed`: a step
  test, the rest of a test file, a data file or fixture beside a step test.
- **Equal at the merge**: a path (a build input, a data file, a fixture) has the same blob and the same
  mode at the Base merge as at its second parent, or is absent at both; a step test or the rest of a test
  file has the same text at both (its file's mode is not part of that text, as before).

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN a Base merge changes a Protected unit that is Equal at the merge THE change gives no `tests-changed`, under cargo and junit. | `check_junit::base_merge_fixture_is_no_tests_change`, `check_history::base_merge_data_file_is_no_tests_change`, `check_history::base_merge_step_test_unit`, `check_junit::base_merge_step_test_unit`, `check_history::base_merge_glob_names`, `check_junit::base_merge_glob_names`, `check_junit::base_merge_deletes_fixture` |
| AC-2 | IF a Base merge resolves a Protected unit that exists at both its parents to a value other than at its second parent THEN the merge gives `tests-changed` with the merge as fault commit, as before. | `check_junit::base_merge_fixture_is_no_tests_change`, `check_history::base_merge_data_file_is_no_tests_change`, `check_history::base_merge_step_test_unit`, `check_junit::base_merge_step_test_unit`, `check_history::base_merge_glob_names`, `check_junit::base_merge_glob_names` |
| AC-16 | WHEN a commit after an Equal Base merge changes the Protected unit THE commit is compared with the unit's value at the merge and gives `tests-changed`. | `check_junit::base_merge_then_edit_is_tests_change` |
| AC-3 | IF a Base merge brings a path whose blob equals that at the second parent but whose mode differs (an executable bit, a link) THEN the path is not Equal at the merge and gives `build-input-changed` at the merge when it is a build input or, under cargo, a data file or fixture beside a step test (STP-13); under junit the mode of a fixture is not part of its value, as before. | `check_history::base_merge_mode_change_blocks`, `check_history::base_merge_mode_outcomes`, `check_junit::base_merge_mode_change_blocks`, `check_junit::base_merge_fixture_mode_only` |
| AC-4 | IF the ticket has no `base_branch`, its `base_branch` ref is gone, the merge's second parent is not an ancestor of that ref, or the merge has more than two parents THEN the merge is no Base merge and its changes block as before. | `check_history::base_merge_other_parents` |
| AC-5 | WHEN a rename entry of a Base merge has one path Equal at the merge and the other not, or the merge brings one Equal and one resolved build input THE entry, or the resolved input, gives `build-input-changed` at the merge, as a change outside a Base merge does. | `check_history::base_merge_mixed_entries` |
| AC-14 | WHEN a Base merge under cargo brings `Cargo.toml`, `build.rs`, `rust-toolchain`, `rust-toolchain.toml`, `.cargo/config` or `.cargo/config.toml` Equal at the merge THE step is not blocked; resolved to other content, it is `build-input-changed` at the merge. | `check_history::base_merge_brings_cargo_inputs` |
| AC-15 | WHEN under cargo a commit after GREEN adds or deletes a data file beside a step test THE step is `build-input-changed` (STP-13); a content change of that file gives `tests-changed`; no junit rule of STP-28 AC-42 applies. | `check_history::cargo_fixture_beside_step_test` |
| AC-6 | WHEN the first word of `command` is an absolute path under the root of the checkout where the check runs, with no `.`, `..` or empty part, THE Per-file runs and the suite run the script at that path relative to the check copy, as it is at the run's commit; any other absolute path runs as given. | `check_junit::absolute_script_runs_at_step_commit`, `check_junit::absolute_script_edges`, `check_junit::absolute_script_dot_path_is_input` |
| AC-18 | IF the script that AC-6 maps is absent at the run's commit THEN the run cannot start and the step is `unverified` with that reason (STP-28 AC-20). | `check_junit::absolute_script_missing_at_commit`, `check_junit::absolute_script_edges` |
| AC-7 | IF the first top-level element of a report is not `testsuites` or `testsuite` THEN the reader refuses the report with a reason that names that element as an unknown root. | `junit_report::refuses_unknown_roots`, `junit_report::root_reason_before_attributes` |
| AC-17 | IF a top-level element follows the root, whatever its name, THEN the reader refuses the report as one with more than one root element. | `junit_report::refuses_second_roots`, `junit_report::root_reason_before_attributes` |
| AC-8 | IF a report holds a CDATA section outside its root element THEN the reader refuses it as text outside the root. | `junit_report::refuses_cdata_outside_root` |
| AC-9 | WHEN the reader is inside an element it does not know THE text and the attributes there are skipped without decoding, so an unknown entity or a bad character reference there is no refusal; tags there must still balance. | `junit_report::skips_content_of_unknown_elements`, `junit_report::known_elements_still_decode` |
| AC-10 | IF `.stapel/stapel.toml` is a file that fails to read, parse or validate THEN `stapel check --history` and `stapel lint` exit 2 and `stapel next` fails, each with the load fault and printing no step line; a folder or a dangling link in its place, or no entry, gives today's refusal that no `.stapel/stapel.toml` is here or above, with no fallback to cargo. | `check_junit::broken_config_is_refused`, `check_junit::broken_config_kinds` |
| AC-11 | IF `.stapel/stapel.toml` is a file that fails to load THEN the commit hook, after a commit whose HEAD names an open ticket, also during a merge, prints the load fault in place of every step line, the lines of parallel tickets following as before, and exits 0; with a folder or a dangling link in its place the hook stays silent as before. | `hook_post::broken_config_is_reported`, `hook_post::broken_config_kinds`, `hook_post::config_fault_with_parallel_conflict` |
| AC-12 | IF the report path is a link, a folder, a FIFO, a socket, a block device or a character device THEN the report is not opened and the run is `unverified` with a reason that names that kind once. | `check_junit::report_kinds_are_named` |
| AC-13 | WHEN the report is a regular file THE size bound holds as before: a report of exactly 16 MiB is read and a longer one is `unverified` as over 16 MiB (a coverage criterion). | `check_junit::report_size_bound`, `junit_report::read_is_bounded` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-18; every other outcome of STP-4, STP-13 and STP-28 is
  unchanged.
- **Not promised:** the Frame's list; a Base merge that deletes a Protected unit that the ticket still
  needs (the step test is then `removed` as before); a merge with the base branch as first parent; a step
  test or fixture edited on the base branch by mistake and then brought by a Base merge (SR1-14, the human);
  an absolute path of another checkout of the repository, which runs as given; a report file that grows
  or is swapped between the kind check and the read (the read is bounded, Design, not tested); the silent
  config fallback of the PreToolUse guard, which keeps today's behaviour; under cargo, which of
  `tests-changed` and `build-input-changed` a data file gives that both parents of a Base merge add and the
  merge resolves to a third text (it blocks either way); block and character devices as the report path,
  named by AC-12 but not tested (a test cannot make one without root).

## Test plan

CLI tests in `crates/stapel-cli/tests/check_junit.rs`, `check_history.rs` and `hook_post.rs`; reader tests in
`crates/stapel-core/tests/junit_report.rs`. The junit tests use `common::junit_repo`; a Base merge is made
by a commit on `main` and `git merge` on the ticket branch, with `base_branch = "main"` in `state.json`.
The absolute script test uses a script inside the repository whose working-tree text differs from its
text at the step's commit. The FIFO, socket and size cases are made by the stand-in command. Light tests:
`stapel cargo test -j 4 -p stapel-cli --test <file> -- --test-threads=3`, `-p stapel-core --test
junit_report`; before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Base merge, junit | a fixture `a b.json` brought by a merge of main into a step test folder, equal: pass; resolved to other text: `tests-changed` at the merge | `check_junit::base_merge_fixture_is_no_tests_change` |
| Base merge, later edit | an equal merge, then a GREEN edit of the fixture: `tests-changed` at the edit | `check_junit::base_merge_then_edit_is_tests_change` |
| Base merge, cargo | a data file in the tests folder of a crate brought equal: pass; resolved: `tests-changed` | `check_history::base_merge_data_file_is_no_tests_change` |
| Base merge, mode | `chmod +x` with an equal blob on a build input and on a fixture; a regular file turned into a link with the same blob | `check_history::base_merge_mode_change_blocks` |
| Base merge, mode outcomes | the outcome per path: `build-input-changed` for `build.rs` with `chmod +x`, for `build.rs` turned into a link and for a cargo fixture with `chmod +x`; an equal link at both parents passes | `check_history::base_merge_mode_outcomes` |
| Base merge, junit mode | a junit input (`package.json`) with `chmod +x` and an equal blob at a merge: `build-input-changed`; a fixture with only `chmod +x`: pass | `check_junit::base_merge_mode_change_blocks` |
| Base merge, step test unit | a step test resolved equal while its file differs elsewhere: pass; a step-test file with only a mode change: pass; a step test resolved to the first parent's text (the ticket's own protected text): no change; resolved to a third text: `tests-changed` at the merge; a step test deleted by a merge and re-added later: the same text passes, another text gives `tests-changed` at the re-add; under junit an edit elsewhere in the file changes the rest unit | `check_history::base_merge_step_test_unit`, `check_junit::base_merge_step_test_unit` |
| Base merge, cargo inputs | the seven named cargo inputs: equal and resolved | `check_history::base_merge_brings_cargo_inputs` |
| Base merge, parents | no `base_branch`; the ref deleted; a merge of another branch; an octopus merge of main and another branch | `check_history::base_merge_other_parents` |
| Base merge, entries | a rename with one equal and one changed path; one equal and one resolved input | `check_history::base_merge_mixed_entries` |
| Cargo fixture | a data file added, one deleted and one changed beside a cargo step test after GREEN | `check_history::cargo_fixture_beside_step_test` |
| Script path | an absolute script under the root changed in the working tree only: the committed text runs; `/<root>/a/../run.sh`, `/<root>//run.sh` and a path outside the root run as given | `check_junit::absolute_script_runs_at_step_commit` |
| Script missing | an absolute script under the root that is untracked | `check_junit::absolute_script_missing_at_commit` |
| Reader roots | `<foo/>`; `<foo/><testsuites/>`; `<a/><b/>` | `junit_report::refuses_unknown_roots` |
| Reader second root | `<testsuites/><foo/>`; `<testsuite/><testsuite/>` | `junit_report::refuses_second_roots` |
| Reader CDATA | `<![CDATA[x]]>` before and after the root | `junit_report::refuses_cdata_outside_root` |
| Reader skips | `&nbsp;` and `&#0;` inside `<system-out>` and in an attribute of `<properties>`; an unbalanced tag inside `<system-out>` still refused | `junit_report::skips_content_of_unknown_elements` |
| Broken config | `stapel.toml` with a syntax error and with an unknown `runner`: `check --history` and `lint` exit 2 and `next` fails with the fault; a folder in its place or no `stapel.toml`: today's refusal | `check_junit::broken_config_is_refused` |
| Broken config, hook | the hook after a commit of an open ticket with a failing step and during a merge: the fault, no step line; a folder in place of `stapel.toml`: silent; a commit of no ticket: nothing |
| Broken config kinds | an unreadable `stapel.toml` (mode 000) and a validation error: the fault for `check --history`, `lint` and `next`; a dangling link: today's refusal; no file: `next` as before; no step line at all | `check_junit::broken_config_kinds` |
| Broken config kinds, hook | the hook on an unreadable file and a validation error: the fault; a dangling link: silent; a closed ticket: silent | `hook_post::broken_config_kinds` |
| Config fault beside a parallel ticket | a config fault and a conflict with a parallel ticket: the fault line, then the conflict line | `hook_post::config_fault_with_parallel_conflict` |
| Script edges | a script present at HEAD but absent at RED; the script missing for the suite run; `/<root>2/run.sh` beside the root runs as given | `check_junit::absolute_script_edges` |
| Reader root before attributes | `<foo a="&bar;"/>`: unknown root; `<testsuites/><foo a="&bar;"/>`: second root; a comment before a CDATA outside the root: text outside the root | `junit_report::root_reason_before_attributes` |
| Reader known elements | `<testcase name="&bogus;">` in a known place is still refused for the entity | `junit_report::known_elements_still_decode` |
| Bounded read | a report file opened, then grown past 16 MiB through a second handle: refused as over 16 MiB | `junit_report::read_is_bounded` |
| Base merge, glob names | a data file and a build input named `d[1].txt` and `x[1].json`, resolved by a Base merge to a third text: `tests-changed` or `build-input-changed` at the merge; brought equal: pass | `check_history::base_merge_glob_names`, `check_junit::base_merge_glob_names` |
| Base merge, junit fixture mode | a fixture of step 1 with only a mode change at a Base merge: step 1 passes; a fixture equal in content to the second parent but not in mode: step 1 passes | `check_junit::base_merge_fixture_mode_only` |
| Base merge deletes a fixture | a fixture present at RED, deleted on main, brought by a Base merge (absent at both): no `tests-changed` | `check_junit::base_merge_deletes_fixture` |
| Script with dot parts | `/<root>/./run.sh` changed between RED and GREEN: `build-input-changed`, as STP-28 | `check_junit::absolute_script_dot_path_is_input` | `hook_post::broken_config_is_reported` |
| Report kinds | a link, a folder, a FIFO, a socket left as the report | `check_junit::report_kinds_are_named` |
| Report size | a report of exactly 16 MiB and of 16 MiB plus one byte | `check_junit::report_size_bound` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `command` first word | path or program name | one character / any | refused as before | absolute under the root without `.`, `..` or empty parts: mapped into the check copy; other absolute: as given | as written, no `~` or variables | `check_junit::absolute_script_runs_at_step_commit` |
| `base_branch` | branch name | one character / any | absent: no Base merge | ref gone: no Base merge | exact ref `refs/heads/<name>` | `check_history::base_merge_other_parents` |
| report file | XML | — / 16 MiB | refused as before | unknown root, second root, CDATA outside the root: refused; unknown content: skipped | exact names | `junit_report::refuses_unknown_roots` |
| report path kind | file system entry | — | missing: refused as before | link, folder, FIFO, socket, device: `unverified` with the kind (devices not tested) | not opened | `check_junit::report_kinds_are_named` |
| `.stapel/stapel.toml` | TOML | — | absent, a folder or a dangling link: today's refusal | unreadable, a parse or validation error: AC-10, AC-11 | the load fault as given | `check_junit::broken_config_is_refused` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | history shape: merge of the base branch, of another branch, octopus, with resolved conflicts | AC-1 to AC-5 | `check_history::base_merge_other_parents` |
| git | file mode and link entries in a merge | compared with the blob (AC-3) | `check_history::base_merge_mode_change_blocks` |
| git | renames in a merge | AC-5 | `check_history::base_merge_mixed_entries` |
| git | paths with spaces, glob characters, newlines | read NUL-separated with literal paths (`:(literal)`); a space and `[ ]` tested | `check_junit::base_merge_fixture_is_no_tests_change`, `check_junit::base_merge_glob_names` |
| git | non-UTF-8 paths, shallow clone, SHA-256, submodules, rebase, squash | as STP-28 | — |
| git | the base branch ref deleted or moved | a deleted ref: no Base merge; a moved ref: the ancestor test at check time | `check_history::base_merge_other_parents` |
| git | linked worktrees | the checkout root is the worktree's root (AC-6); the check copy as before | `check_junit::absolute_script_runs_at_step_commit` |
| git | dirty tree | `stapel.toml` and the script's working-tree text are not used for a committed script (AC-6) | `check_junit::absolute_script_runs_at_step_commit` |
| git | configuration `core.fileMode`, `core.symlinks` | modes are read from commits, so the working tree's setting does not apply | — |
| junit | report with unknown roots, CDATA, entities in unknown elements | AC-7 to AC-9, AC-17 | `junit_report::skips_content_of_unknown_elements` |
| os | report path is a link, folder, FIFO, socket or device | AC-12 | `check_junit::report_kinds_are_named` |
| os | a report that grows after the kind check | the read is bounded (Design); not tested | — |
| cargo | the runner's build inputs through a Base merge | AC-14 | `check_history::base_merge_brings_cargo_inputs` |
| claude-code | the hook on a broken config | AC-11 | `hook_post::broken_config_is_reported` |

### Abuse

| Attempt of a careless agent | Outcome | Test |
|---|---|---|
| merges main and in the same merge edits a step fixture | `tests-changed` at the merge | `check_junit::base_merge_fixture_is_no_tests_change` |
| merges main, then edits the fixture the merge brought | `tests-changed` at the edit | `check_junit::base_merge_then_edit_is_tests_change` |
| merges main and sets the executable bit on a merged input | `build-input-changed` at the merge | `check_history::base_merge_mode_change_blocks` |
| merges its own side branch, or main with another branch at once | its changes block as before | `check_history::base_merge_other_parents` |
| edits the absolute run script in the working tree after GREEN | the check runs the committed script | `check_junit::absolute_script_runs_at_step_commit` |
| breaks `stapel.toml` so the junit ticket is judged as cargo | `check --history` and `lint` exit 2, `next` fails; the hook prints the fault | `check_junit::broken_config_is_refused` |
| makes the command leave a FIFO as the report | `unverified` with the kind; the check does not block | `check_junit::report_kinds_are_named` |

**Review Focus:** a Base merge whose second parent is the base branch tip moved after the merge (the
ancestor test still holds); an unknown root that is empty and self-closed before a valid root; the hook's
lines when the config fault and a conflict line of a parallel ticket exist together (not tested); a
fixture deleted on main and brought by a Base merge (absent at both, so Equal); a script path through a
link into the root (compared as written).

**Author self-check:** 2026-11-12, before spec review and again after rounds 1 and 2: promise item 1 maps to
AC-1, AC-2, AC-16; item 2 to AC-6, AC-18; item 3 to AC-7 to AC-9, AC-17; item 4 to AC-10, AC-11; item 5
to AC-12, AC-13; item 6 to AC-3 to AC-5, AC-14, AC-15.

## Design

### Facts

- F-1: `build_inputs.rs` `base_merge_parent` gives the second parent of a Base merge; `same_as_parent`
  compares `rev-parse <commit>:<path>`, the blob only, so a mode change with an equal blob counts as equal.
- F-2: `outcomes.rs` `analyse_in` walks each Protected unit with `touching` (first-parent `git log` from
  RED to HEAD); a commit whose value differs stops the walk with `tests-changed`. It knows no Base merge.
- F-3: `check.rs` `junit_inputs` maps an absolute first word inside the repository to a repository path
  for the input rule, but `suite_junit` and the Per-file run pass the word to `run_program` as given.
- F-4: `junit.rs` `Reader::open` counts only `testsuites` and `testsuite` as roots; any other top-level
  element raises `unknown`; `markup` skips CDATA without looking at the stack; `run` decodes every text
  chunk, `open_tag` every attribute, also inside unknown elements.
- F-5: `check.rs` `junit_globs_at` and the junit config read in `history_report` turn a load fault into
  `None`, which is the cargo mode; `hook_faults` turns any fault into no line.
- F-6: `check.rs` `read_report_file` reads `symlink_metadata`, then `std::fs::read` of the whole path.

### Approach

- `same_as_parent` compares the `ls-tree -z` entry (mode and blob) of a literal path. It becomes public in
  `build_inputs.rs` with `base_merge_parent`, which also requires exactly two parents. `analyse_in` asks
  them for each commit `touching` returns: for a Base merge where the unit is Equal at the merge, the walk
  sets `current` to the unit's value at the merge and goes on.
- One function maps the first word of `command`: absolute, under the checkout root by a lexical prefix and
  without `.` or `..` parts gives the path in the check copy; both runs and `junit_inputs` use it.
- The reader counts every element with no parent as a root: the first one, if unknown, is refused by name;
  a second one is a second root. CDATA with an empty stack is text outside the root. Text and attributes
  are decoded only when `unknown` is 0.
- `junit_globs_at` returns `Result`, with an absent file as `None`; `history_report` passes the load fault
  up; `check --history` and `lint` refuse with exit 2 and `next` fails; `hook_faults` returns the fault in
  place of its step lines, and the hook adds the trial-merge lines as before.

### As built (steps 1 to 11)

- Step 1: `same_as_parent` compares the `git ls-tree` entry (mode and blob).
- Step 2: `analyse_in` takes `base_branch`; in the `touching` loop a Base merge whose path is Equal sets
  `current` and goes on. Step 13 compares the unit's value instead of the whole path (d2-1).
- Step 3: `base_merge_parent` returns no parent for a merge with a `^3` parent.
- Step 5: `script_under(root, word)` maps an absolute path under the root without `.`, `..` or empty parts;
  `Runner::script_path` uses it for the Per-file run and the suite; `junit_inputs` uses it, so such a
  script stays a fixed input (STP-28 AC-25, AC-39) and a path with dot parts no longer is.
- Steps 6 and 7: every element without a parent is a root; an unknown first root is refused as "unknown
  root <name>: the report has no <testsuites> element" (the old wording kept for STP-28 tests); CDATA with
  an empty stack is text outside the root; `open_tag` returns raw values and `open` decodes them only for
  known elements.
- Step 8: `repo::open` takes any entry at `.stapel/stapel.toml` as the root; step 16 restores the lookup by
  a regular file (the human, d8-3). `junit_globs_at` and `history_report` still read the config with
  `.ok()`; they run only after `repo::open` succeeded; step 17 makes `junit_globs_at` return `Result`.
- Step 9: `hook_faults` resolves the ticket of HEAD before the merge check and returns `stapel: <load
  fault>` for an open ticket (every hook line starts with `stapel: `); `config_root` in `hook.rs` goes back
  to `stapel_root` in step 16.
- Step 10: `read_report_file` names a socket, a block and a character device, else "of an unknown kind";
  the reason is "the report X of Y is <kind>"; the socket test makes a socket with `python3`. The one-open
  read is step 19 (d10-1).
- History: the commits of steps 7 and 8 interleaved (two builders at once); they were replayed in step
  order with the human's leave (tag `stp30-before-reorder`).
- The RED commits of steps 2 and 9 were amended before their GREEN and not rerun on their own trees; the
  final `stapel check` shows them.
- Step 12, 14, 17 and 18: coverage commits; their tests passed at once.
- Step 13: the Base-merge shortcut runs only for a unit present at the merge; a file unit is Equal by
  `same_as_parent` (content and mode), a test, helper or rest unit by its value; a unit absent at the
  merge takes the deletion path.
- Step 15: coverage commit; the doc comment of `open_tag` is a plain `STP-30:` commit (no behaviour).
- Step 16: `repo::open` and `config_root` find the root by `stapel_root` again.
- Step 17: `junit_globs_at` returns `Result`; `history_report`, `next` and `lint` pass the fault up, the
  hook path ignores it after `hook_faults` has reported it; a plain `STP-30:` commit, no test sees it.
- Step 19: `junit::read_open_report` reads an open file through `take(MAX_REPORT + 1)`;
  `read_report_file` keeps the kind check by `symlink_metadata`, opens with `O_NOFOLLOW | O_NONBLOCK`,
  checks the open file is regular and reads through it. Its RED failed by compile error (the function did
  not exist).
- `read_report_file` names the kind from `symlink_metadata` and opens only a regular file, through a
  stapel-core function that reads an open file with the bound, with
  `O_NOFOLLOW | O_NONBLOCK`; it checks the open file is regular and reads through `take(MAX_REPORT + 1)`
  (d21-1: Design intent, not a criterion; a race cannot be made in a test).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check_junit::base_merge_fixture_is_no_tests_change`, `check_history::base_merge_data_file_is_no_tests_change`, `check_history::base_merge_step_test_unit`, `check_junit::base_merge_step_test_unit`, `check_history::base_merge_glob_names`, `check_junit::base_merge_glob_names`, `check_junit::base_merge_deletes_fixture` |
| AC-2 | `check_junit::base_merge_fixture_is_no_tests_change`, `check_history::base_merge_data_file_is_no_tests_change`, `check_history::base_merge_step_test_unit`, `check_junit::base_merge_step_test_unit`, `check_history::base_merge_glob_names`, `check_junit::base_merge_glob_names` |
| AC-3 | `check_history::base_merge_mode_change_blocks`, `check_history::base_merge_mode_outcomes`, `check_junit::base_merge_mode_change_blocks`, `check_junit::base_merge_fixture_mode_only` |
| AC-4 | `check_history::base_merge_other_parents` |
| AC-5 | `check_history::base_merge_mixed_entries` |
| AC-6 | `check_junit::absolute_script_runs_at_step_commit`, `check_junit::absolute_script_edges`, `check_junit::absolute_script_dot_path_is_input` |
| AC-7 | `junit_report::refuses_unknown_roots`, `junit_report::root_reason_before_attributes` |
| AC-8 | `junit_report::refuses_cdata_outside_root` |
| AC-9 | `junit_report::skips_content_of_unknown_elements`, `junit_report::known_elements_still_decode` |
| AC-10 | `check_junit::broken_config_is_refused`, `check_junit::broken_config_kinds` |
| AC-11 | `hook_post::broken_config_is_reported`, `hook_post::broken_config_kinds`, `hook_post::config_fault_with_parallel_conflict` |
| AC-12 | `check_junit::report_kinds_are_named` |
| AC-13 | `check_junit::report_size_bound`, `junit_report::read_is_bounded` |
| AC-14 | `check_history::base_merge_brings_cargo_inputs` |
| AC-15 | `check_history::cargo_fixture_beside_step_test` |
| AC-16 | `check_junit::base_merge_then_edit_is_tests_change` |
| AC-17 | `junit_report::refuses_second_roots`, `junit_report::root_reason_before_attributes` |
| AC-18 | `check_junit::absolute_script_missing_at_commit`, `check_junit::absolute_script_edges` |

## Plan

Route: full (tag `guard`), auto.

1. Mode in Equal at the merge (AC-3; test `check_history::base_merge_mode_change_blocks`):
   `build_inputs.rs`. Must not change: a merge with an equal blob and an equal mode blocks nothing.
2. Base merge in `tests-changed` (AC-1, AC-2, AC-16; tests
   `check_junit::base_merge_fixture_is_no_tests_change`,
   `check_history::base_merge_data_file_is_no_tests_change`,
   `check_junit::base_merge_then_edit_is_tests_change`): `outcomes.rs`, `build_inputs.rs`. RED fails on the
   equal merge; the edit test guards that the walk goes on. Must not change: `tests-changed` of a commit
   that is no Base merge.
3. Two parents only (AC-4; test `check_history::base_merge_other_parents`): `build_inputs.rs`. RED fails
   on the octopus merge.
4. Coverage of mixed entries, cargo inputs and the cargo fixture (AC-5, AC-14, AC-15; tests
   `check_history::base_merge_mixed_entries`, `check_history::base_merge_brings_cargo_inputs`,
   `check_history::cargo_fixture_beside_step_test`): all three are expected to pass at once; a test that
   fails becomes a RED with its GREEN in this step.
5. Absolute script path (AC-6, AC-18; tests `check_junit::absolute_script_runs_at_step_commit`,
   `check_junit::absolute_script_missing_at_commit`): `check.rs`. Must not change: a relative script or a
   program from `PATH`.
6. Reader roots and CDATA (AC-7, AC-17, AC-8; tests `junit_report::refuses_unknown_roots`,
   `junit_report::refuses_second_roots`, `junit_report::refuses_cdata_outside_root`): `junit.rs`.
7. Unknown content skipped (AC-9; test `junit_report::skips_content_of_unknown_elements`): `junit.rs`.
   Must not change: the reading of the real reports.
8. Broken config in `check --history`, `lint` and `next` (AC-10; test
   `check_junit::broken_config_is_refused`): `check.rs`, `lint.rs`, `next.rs`.
9. Broken config in the hook (AC-11; test `hook_post::broken_config_is_reported`): `check.rs`, `hook.rs`.
10. Report kinds (AC-12; test `check_junit::report_kinds_are_named`): `check.rs`, the one-open read of
    Design. RED fails on the socket.
11. Report size coverage (AC-13; test `check_junit::report_size_bound`): a coverage commit.

Fix steps after the drift reviews of the build (they do not count toward the limit of 12, ledger
2026-11-12):

12. Mode outcomes per path (d1-1, d1-2, d1-3; tests `check_history::base_merge_mode_outcomes`,
    `check_junit::base_merge_mode_change_blocks`): a coverage step; a test that fails becomes a RED with its
    GREEN in this step.
13. Unit value at a Base merge (d2-1, d2-2, d2-3; tests `check_history::base_merge_step_test_unit`,
    `check_junit::base_merge_step_test_unit`): `outcomes.rs`, `build_inputs.rs`. Equal compares the unit's
    value at the merge with that at the second parent, and the mode for a file unit; a unit absent at the
    merge takes the deletion path as before.
14. Script edges (d5-2; test `check_junit::absolute_script_edges`): a coverage step.
15. Reader order and known elements (d6-1, d6-3, d7-1, d7-2; tests
    `junit_report::root_reason_before_attributes`, `junit_report::known_elements_still_decode`): `junit.rs`;
    the root is judged before its attribute values are decoded; the doc comment of `open_tag` names raw
    values.
16. Root lookup as before (d8-3, d9-1; tests `check_junit::broken_config_is_refused`,
    `hook_post::broken_config_is_reported`): the RED changes the folder cases of both tests to today's
    refusal and a silent hook; the GREEN restores `repo::open` and `config_root` to `stapel_root`.
17. Config fault kinds (d8-1, d8-2, d8-4, d9-3; tests `check_junit::broken_config_kinds`,
    `hook_post::broken_config_kinds`): `check.rs`; `junit_globs_at` returns `Result`; a coverage step for
    the cases that already hold.
18. Config fault beside a parallel ticket (d9-2; test `hook_post::config_fault_with_parallel_conflict`): a
    coverage step.
19. Bounded report read (d10-1, d10-2; test `junit_report::read_is_bounded`): a stapel-core function reads
    an open regular file through `take(MAX_REPORT + 1)`; `read_report_file` opens with `O_NOFOLLOW |
    O_NONBLOCK` and checks the open file is regular.

Fix steps after code review round 1 (not counted toward the limit of 12):

20. Literal paths at a Base merge (AC-1, AC-2, CR1-E1; tests `check_history::base_merge_glob_names`,
    `check_junit::base_merge_glob_names`): `build_inputs.rs` `same_as_parent` reads `:(literal)<path>`.
21. Junit fixture mode (AC-3, CR1-E2, CR1-D4; test `check_junit::base_merge_fixture_mode_only`): `outcomes.rs`; a
    junit file unit is Equal by content only.
22. A fixture absent at both (AC-1, CR1-E3; test `check_junit::base_merge_deletes_fixture`): `outcomes.rs`; a file
    unit absent at the merge and at the second parent is Equal; a test, helper or rest unit keeps the
    deletion path.
23. Dot parts stay an input (AC-6, CR1-E4; test `check_junit::absolute_script_dot_path_is_input`): `check.rs`;
    `junit_inputs` takes every absolute script path under the root as a fixed input, as STP-28 did; only
    the mapping into the check copy needs a path without dot or empty parts.

After step 23 one plain `STP-30:` commit with no behaviour change: remove `config_root` (CR1-D1, CR1-R1),
reflow the long lines (CR1-D2, CR1-R2), a typed over-size error in place of matching `MiB` (CR1-D3,
CR1-R3).

## Review

TODO

## Summary

TODO
