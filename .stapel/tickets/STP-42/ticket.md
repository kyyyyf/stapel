## Description

Source: `docs/PLAN.md`, phase 3b, ticket 3b-5 (ledger decision of 2026-11-24); the deferred findings
STP-39 XR2-7, STP-40 XR1-13 and XR2-9 (pack base); the notes of STP-40 and STP-41 (one cargo run at a time,
the external reviewer, failing tests unnamed, the fixtures privacy test).

### Frame

**Problem.** Parallel tickets wait for each other on one machine-wide cargo lock, though the machine can
run more than one build. The external reviewer is set to a model the person cannot use and is recorded
under the role `reviewer`. After a merge of main, the review pack diffs from the ticket base, so it holds
every ticket merged since. A failing suite prints only counts, so the orchestrator reruns cargo to find the
test. The builder does not run the fixtures privacy test, so a private term reaches a commit.

**Plan item:** 3b-5.

**Risk tags:** `interface` (`[cargo] slots`, the agent type `stapel-external-reviewer`, the `failing:` line
of `stapel check`, the waiting line of `stapel cargo`).

**Non-goals.** Measuring the memory that N builds need (the person sets N); splitting the code between
reviewers; a `history:` line for test helpers; a guard against shell edits; a lock across machines; the
merge round pack (it diffs from the merge base already); a change of the check base of `stapel check`.

**Promise (closed list).**

1. Slots: `[cargo] slots`, a whole number from 1 to 16, default 3; a bad value fails loading
   `stapel.toml`. `stapel cargo` and `stapel check` take one of N lock files under `~/.cache/stapel/`
   (`cargo.lock`, `cargo-2.lock` to `cargo-N.lock`); at most N runs of one repository setting run at once.
2. One run per worktree: a second cargo run from the same worktree waits for the first, before it takes
   a slot, and the waiting line names the holder.
3. All slots held: the run waits with no limit; the waiting line names every holder.
4. The held mark: a child of a holder takes no slot, as now.
5. External reviewer: `stapel init` installs the agent `stapel-external-reviewer` with the model of the
   role `external` in its header; `[roles]` maps it to `external`; the starter `stapel.toml` binds
   `external` to `claude-opus-5-5`; `stapel review start` names this agent for the external reviewer.
6. Pack base: `stapel review start --round N` diffs from the merge base of HEAD and the base branch of
   `state.json`; pack.md names that base and the ticket base. Without a base branch, the ticket base, as
   now.
7. Failing tests: a failing suite run prints `failing:` with up to 20 test names and `and N more`, and
   writes the list to `runs.jsonl`, for the cargo runner and the junit runner.
8. Builder: the builder agent runs the fixtures privacy test named by the Test plan with the ticket lint
   before each GREEN; CLAUDE.md of this repository says N cargo runs at most.

Not promised: a lock file on a file system without OS locks; slots of a repository with another N used at
the same time (each takes the first N of its own); test names a runner does not print; a run killed with
its children left holding a slot; the merge base when the base branch was rewritten.

**Prior art.** STP-27 (the cargo lock and `stapel cargo`); STP-28 (junit reports); STP-39 and STP-40
(merge rounds); the jobserver of GNU make (slots as tokens).

**Size.** About 300 source lines and 400 test lines; 9 Plan steps. Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-24: Decision (slots, the human, recommended): N is `[cargo] slots` in `stapel.toml`, default 3;
  the slots are N lock files. Rejected: a variable `STAPEL_CARGO_SLOTS`; a
  number in a file under `~/.cache/stapel`.
- 2026-11-24: Decision (external reviewer, the human, recommended): a new agent type
  `stapel-external-reviewer` with the model in its header, mapped to the role `external`. Rejected: the
  same agent with a mark in the prompt.
- 2026-11-24: Decision (pack base, the human, recommended): the merge base of HEAD and the base branch.
  Rejected: the first parent of the last merge of main.
- 2026-11-24: Decision (failing tests, the human, recommended): up to 20 names on the `failing:` line and
  the same list in `runs.jsonl`, for cargo and junit. Rejected: every name, in the output only.
- 2026-11-24: Decision (agent): slot 1 keeps the name `cargo.lock`, the others are `cargo-2.lock` to
  `cargo-N.lock`, so a stapel binary built before this ticket still shares slot 1 with a new one.
- 2026-11-24: Assumption: the per-worktree lock is a lock file under `~/.cache/stapel/` named by a hash of
  the worktree root, taken before a slot.

## Spec

Terms:

- **Slot**: one of the lock files `~/.cache/stapel/cargo.lock` (slot 1) and `cargo-<i>.lock` for i from 2
  to N, held by an OS lock as the cargo lock of STP-27 is.
- **N**: the value of `[cargo] slots` in the `stapel.toml` of the repository of the current folder, or 3
  when the key or the table is absent or the folder is in no stapel repository; a `stapel.toml` that
  fails loading is refused (AC-11).
- **Worktree lock**: the lock file `~/.cache/stapel/worktree-<h>.lock`, where `<h>` is the first 16 hex
  digits of the SHA-256 of the canonical root of the git working tree of the current folder (of the
  current folder itself outside git).
- **Holder line**: the text the holder writes into each lock file it holds, as STP-27 writes it.
- **Waiting line**: `waiting for the cargo lock: held by <holders>`, where `<holders>` is the holder lines
  of the locks waited for, joined by `; `, and an empty one is `an unknown holder (…)` as now.
- **Pack base**: the output of `git merge-base refs/heads/<base branch> HEAD` when `state.json` names a
  base branch and git finds one; otherwise the ticket base `state.json` `base`.
- **Failing name**: under the cargo runner, `<target> <name>` for each line `test <name> ... FAILED` of
  the suite output, where `<target>` is the path of the last `Running` line before it (`unittests ` and
  the ` (<binary>)` part removed), or `doc <crate>` after a `Doc-tests <crate>` line; under junit, `<suite> <case>` for each failed case of the report, the case name with outer
  whitespace removed (jest-junit puts a space before top-level names).

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF `[cargo] slots` is not a whole number from 1 to 16 (`0`, `17`, `-1`, `2.5`, `"3"`) THEN loading `stapel.toml` fails with `cargo.slots must be a whole number from 1 to 16`; WHEN the key or the table is absent THE N is 3; an unknown key under `[cargo]` fails as in other tables. | `cargo_lock::slots_setting`, `cargo_lock::default_slots_are_three` |
| AC-2 | WHEN fewer than N slots are held THE `stapel cargo` and `stapel check` take the first free slot, write the holder line into it and run at once; the held mark and the inherited descriptor cover it, as STP-27 does for one lock. | `cargo_lock::runs_in_parallel_up_to_slots` |
| AC-3 | WHEN every slot of 1 to N is held THE run prints the waiting line with the N holder lines once, waits with no time limit and takes the first slot that is freed; with N = 1 the behaviour is that of STP-27. A run prints at most two waiting lines: one for its worktree lock (AC-4), then one for the slots. | `cargo_lock::all_slots_held_waits`, `cargo_lock::two_waiting_lines` |
| AC-4 | WHEN `stapel cargo` or `stapel check` runs THE it takes the worktree lock before a slot and writes its holder line into it, and the held mark and the inherited descriptors cover both locks; WHEN the worktree lock is held THE a second run from that worktree prints the waiting line with that holder, waits, and holds no slot while it waits, even when slots are free. | `cargo_lock::one_run_per_worktree`, `cargo_lock::worktree_lock_outlives_its_holder` |
| AC-5 | WHEN `stapel init` runs THE it writes `.claude/agents/stapel-external-reviewer.md` with the body of the reviewer agent, the read-only tools and the model of `[models] external`; the starter `stapel.toml` binds `external` to `claude-opus-5-5` and maps `stapel-external-reviewer = "external"` under `[roles]`; with `[roles]` absent the agent's records get the role `external`. | `init_process::external_reviewer_agent` |
| AC-6 | WHEN `stapel review start` prints the reviewers of a full ticket with a risk tag THE the external line is `reviewer: stapel-external-reviewer, model <m> (role external)`; the other lines stay. | `review::reviewers_by_route_and_tags` |
| AC-7 | WHEN `stapel review start --round N` runs and the pack base differs from the ticket base THE `diff.patch` and the stat of `pack.md` are of `<pack base>..HEAD`, `pack.md` names `- base: <pack base>` and the line `- ticket base: <ticket base>`; WHEN the two are equal THE `pack.md` has the one base line, as now; IF `state.json` has no base branch, the branch is missing or git finds no merge base THEN the ticket base, as now, with no `ticket base` line. | `review::pack_base_after_a_main_merge`, `review::pack_base_edges` |
| AC-8 | WHEN the suite of `stapel check` fails under the cargo runner THE the line after the suite line is `failing: <names>` with the first 20 distinct failing names in output order joined by `, `, and ` and <n> more` when there are more distinct names; the record's `suite` gets `failing` (the same list) and `failing_more` (n); a passing suite or one with no failing name prints no such line and writes no such key. | `check_failing::cargo_names_failing_tests` |
| AC-9 | WHEN the suite fails under the junit runner THE the same line and record keys are made from the distinct failing names of the failed cases of the report, in report order. | `check_failing::junit_names_failing_tests` |
| AC-10 | WHEN `stapel init` installs the builder agent and the skill THE the builder text says to run the private-data test of the project with the ticket lint before each GREEN, and the skill's rule 6 says the Test plan names both. | `init_process::builder_runs_the_privacy_test` |
| AC-11 | IF the `stapel.toml` of the current repository fails loading (a bad `[cargo] slots` among other causes) THEN `stapel cargo` prints `stapel: ` and the load error, exits 1 and runs no cargo, as `stapel check` does; WHEN the current folder is in no stapel repository THE `stapel cargo` takes N = 3. | `cargo_lock::a_bad_config_refuses` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-1 to AC-3 and AC-11, item 2 by AC-4, item 3 by AC-3, item 4
  by AC-2, item 5 by AC-5 and AC-6, item 6 by AC-7, item 7 by AC-8 and AC-9, item 8 by AC-10.
  Unchanged: the held mark (`cargo_lock::held_mark_takes_no_lock`); a killed holder frees its locks
  (`cargo_lock::killed_holder_frees_the_lock`); the exit codes of `stapel cargo` when its `stapel.toml`
  loads; the merge round pack; the check base of `stapel check`; the counts of the suite line.
  Changed on purpose: a `stapel.toml` that fails loading now stops `stapel cargo` (AC-11), as it stops
  `stapel check`; with `external` among the agent roles, an `[models] external` binding that cannot go
  into an agent file makes `stapel init` refuse, and an absent binding gives the header no model, as for
  the other agent roles.
- **Not promised:** the Frame's list; test names under a cargo `--format` other than the default; a slot
  freed between the read of its holder line and the print of the waiting line (the line may name an old
  holder); two worktrees whose roots hash to the same 16 digits.

**Author self-check (item 7).** Done on 2026-11-24: each promise item maps to a criterion; the waiting
line keeps the text of STP-27, so the STP-27 tests hold under N = 1.

## Test plan

Integration tests in `crates/stapel-cli/tests/`: the existing `cargo_lock.rs`, `init_process.rs` and
`review.rs` and the new file `check_failing.rs`. `cargo_lock.rs` keeps its stand-in cargo and temporary
`HOME`; its fixture `check_repo` gains `[cargo] slots = 1`, so the STP-27 tests keep one slot (the
fixture change fails at RED, because the key is unknown before the code); the new tests use two
repositories or two worktrees and set N. `check_failing.rs` builds its fixture as `check_levels.rs` does
(the fake cargo of `tests/common/`, a ticket with one passed step) with a `[check] cargo` stand-in that
prints libtest output, and `common::junit_repo` for junit. Light tests: `stapel cargo test -j 4 -p
stapel-cli --test cargo_lock -- --test-threads=4` (and `--test check_failing`, `--test review`, `--test
init_process`); before each GREEN also `--test ticket_drift` and `--test fixtures`.

| Category | Cases | Tests |
|---|---|---|
| Slots value | `0`, `17`, `-1`, `2.5`, `"3"`, an unknown key `[cargo] x = 1`: load error; `1` and `16` load; absent table: 3 | `cargo_lock::slots_setting` |
| Parallel | every run of the tests of AC-2 and AC-3 starts from a worktree of its own; N = 2, two repositories: both stand-in cargos hold at once and slots 1 and 2 hold their holder lines; `stapel check` while one `stapel cargo` holds: runs on slot 2 | `cargo_lock::runs_in_parallel_up_to_slots` |
| All held | N = 2, both slots held by foreign processes with and without holder lines: the waiting line names both, once; slot 2 freed: the run starts on slot 2; a third run from a third worktree waits while two runs hold | `cargo_lock::all_slots_held_waits` |
| Same worktree | N = 3, one run holds in a worktree: its worktree lock holds its holder line; a second run from a subfolder of that worktree waits with the holder's line and holds no slot; the first ends: the second runs; a run from another worktree of the same repository runs at once; a run waiting first for its worktree and then for the slots prints two lines | `cargo_lock::one_run_per_worktree` |
| Bad config | `slots = 0` and a broken `stapel.toml`: `stapel cargo` exits 1 with the load error and the stand-in cargo did not run; a folder outside any stapel repository: runs | `cargo_lock::a_bad_config_refuses` |
| Agent file | the installed file's header (name, tools, model of `external`), its body equals the reviewer's; the starter `[models]` and `[roles]`; a token record of the agent type without `[roles]` | `init_process::external_reviewer_agent` |
| Reviewers | a full tagged ticket: the three lines; light and untagged tickets unchanged | `review::reviewers_by_route_and_tags` |
| Drift cases | no `[cargo]` table: three runs from three worktrees hold at once and a fourth waits; a run that waits for its worktree lock and then for the slots prints two waiting lines; the holder `stapel cargo` killed while its cargo child runs: the worktree lock stays held until the child ends; a run with the held mark takes no worktree lock | `cargo_lock::default_slots_are_three`, `cargo_lock::two_waiting_lines`, `cargo_lock::worktree_lock_outlives_its_holder` |
| Pack base edges | a base branch whose merge base is the ticket base: one base line; a base branch with no common history: the ticket base, one base line | `review::pack_base_edges` |
| Pack base | a ticket branch with a merge of a later main commit: diff and stat hold only the ticket's files, both base lines; no merge: one base line; no base branch in `state.json`; a deleted base branch | `review::pack_base_after_a_main_merge` |
| Cargo failing | 3 failing tests in two targets; a failing doc test after a `Doc-tests tiny` line: `doc tiny <name>`; 25 failing: 20 and `and 5 more`; one name printed twice; a compile failure; a passing suite: no line, no key | `check_failing::cargo_names_failing_tests` |
| Junit failing | two failed cases and one passed; one failed case named twice; a passing report | `check_failing::junit_names_failing_tests` |
| Texts | the builder asset and the skill sentence; the installed copies match | `init_process::builder_runs_the_privacy_test` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[cargo] slots` | TOML integer | 1 / 16 | absent: 3 | 0, 17, negative, a float, a string: load error; `stapel cargo` refuses | whole | `cargo_lock::slots_setting`, `cargo_lock::a_bad_config_refuses` |
| `stapel review start --round` | whole number | 1 / any | — | unchanged (STP-36) | whole | `review::pack_base_after_a_main_merge` |
| Suite output | text | nothing / the output limit | nothing: no names | lines not of libtest form: ignored | exact names | `check_failing::cargo_names_failing_tests` |
| Junit report | XML | no case / any | no failed case: no line | unreadable: unchanged (STP-28) | exact names | `check_failing::junit_names_failing_tests` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| os | lock files: held by a foreign process with or without a holder line | the waiting line names it or an unknown holder | `cargo_lock::all_slots_held_waits` |
| os | `HOME` unset or read-only | refused as in STP-27 | `cargo_lock::wrapper_runs_cargo` |
| os | file systems without OS locks; a holder killed with children left | not promised | — |
| git | worktrees: the current folder in a linked worktree or a subfolder | the worktree lock of its working tree root | `cargo_lock::one_run_per_worktree` |
| git | the base branch missing, or no merge base | the ticket base | `review::pack_base_after_a_main_merge` |
| git | a rewritten base branch | not promised | — |
| cargo | libtest output of failing tests and doc tests, a compile failure | names or no line | `check_failing::cargo_names_failing_tests` |
| cargo | `--format json` or a custom harness | not promised | — |
| node | junit report of failed cases | names | `check_failing::junit_names_failing_tests` |
| claude-code | the agent type of a subagent mapped to a token role | `stapel-external-reviewer` to `external` | `init_process::external_reviewer_agent` |

**Review Focus:** a run of `stapel check` in a temporary worktree copy (it takes its locks before the copy,
in the ticket's worktree; not tested again); two repositories with different N at once (not promised);
the CLAUDE.md line on cargo runs (text only, not tested).

## Design

### Facts

- F-1: `take` in `crates/stapel-cli/src/cargo.rs` opens `~/.cache/stapel/cargo.lock`, tries the OS lock,
  prints the waiting line with the holder line, waits, writes the holder line and clears close-on-exec;
  `check.rs` (about line 467) calls it as `take("stapel check")` and sets the held mark.
- F-2: `init.rs::AGENT_FILES` lists the agent files with name, role, tools and body; the model of the
  header comes from `[models] <role>`; `AGENT_ROLES` lists the roles checked for an agent file.
- F-3: `stapel-core::review::reviewers` returns `stapel-reviewer` with role `external` for the external
  reviewer; `review.rs` prints the reviewer lines.
- F-4: `review.rs::write_pack` diffs `state.base..HEAD`; `write_merge_pack` already takes the merge base
  of the base branch.
- F-5: `check.rs` sums the libtest summary lines of the suite into `Suite { outcome, detail, counts }`;
  `suite_junit` counts the cases of the report.
- F-6: the starter `stapel.toml` is `crates/stapel-core/src/stapel.toml`, with `[models]` and `[roles]`.

### Shape

- `stapel-core::config`: `Cargo { slots: Option<toml::Value> }` with `deny_unknown_fields`, `Config.cargo`,
  validated in `parse` by hand (an integer from 1 to 16, else the AC-1 message); `Config::cargo_slots()`
  returns N.
- `stapel-cli::cargo::take`: reads N from the config of the current repository (3 outside one; a load
  error is returned as the refusal of AC-11), takes
  the worktree lock, then tries the slots in order and, when all are held, waits on any slot by polling
  `try_lock` each 100 ms; the returned `CargoLock` holds both files.
- `stapel-core::review::reviewers`: the external entry gets the agent `stapel-external-reviewer`.
- `init.rs`: a sixth `AgentFile` `external-reviewer`, role `external`, body of `reviewer.md`; `external`
  joins `AGENT_ROLES`. `hook.rs::DEFAULT_ROLES` gains the row `stapel-external-reviewer` to `external`,
  so the records of the agent get the role without `[roles]`.
- `review.rs::write_pack`: computes the pack base and passes it with the ticket base to `pack_text`.
- `check.rs`: `Suite` gains `failing: Vec<String>` and `failing_more: u64`, filled by the cargo and junit
  paths; the run prints the `failing:` line and writes the keys.

### Decisions (before the build)

- 2026-11-24, agent: the worktree lock is taken before a slot, so a worktree that waits for itself holds
  no slot another worktree could use.
- 2026-11-24, agent: when every slot is held the run polls the slots instead of blocking on one, so it
  takes whichever is freed first.
- 2026-11-24, agent (SR1-2): a `stapel.toml` that fails loading stops `stapel cargo`, so a bad `slots`
  never runs with a default the person did not set.
- 2026-11-24, agent (SR1-5): a failing doc test is named with the target `doc <crate>`.
- 2026-11-24, agent (SR1-4): old tests whose expected output changes (the agent list, the frontmatter
  golden, the roles rows, the reviewer lines) are changed in the RED of their step, where they fail.
- 2026-11-24, agent: this repository's `.stapel/stapel.toml` gets the `[roles]` row and `[cargo] slots =
  3` from the orchestrator through its dialog, because a builder may not edit it.

### Decisions (as built)

- Step 1 (67a8c02, 105eb08): `Config.cargo: Option<CargoSettings>` keeps `slots` as a raw `toml::Value`;
  `DEFAULT_CARGO_SLOTS` is 3 and `Config::cargo_slots()` returns N; `cargo.rs::run` loads the config with
  `repo::load` and refuses on a load error. Before the code an unknown `[cargo]` table loaded, so the
  fixture change of `check_repo` alone did not fail at RED; the two new tests did.
- Step 2 (06ed3b9, a400b0e): `take` opens N slot files, takes the first free one, prints one waiting line
  with every holder joined by `; ` and polls each 100 ms.
- 2026-11-24, agent: `cargo_lock::junit_check_takes_the_lock` (STP-28) gets `[cargo] slots = 1` inside
  the test, amended into the step 2 RED, as the Test plan does for `check_repo`; with 3 slots it took
  slot 2 and never waited.
- Step 3 (4a8819e, ca5da16): the worktree lock `worktree-<h>.lock` hashes the canonical root that
  `git rev-parse --show-toplevel` prints; each wait prints its own line.
- 5b7c038: this repository's `.stapel/stapel.toml` gets `stapel-external-reviewer = "external"` and
  `[cargo] slots = 3`.
- Step 4 (997fb72, f5dd71b): the sixth `AgentFile` `external-reviewer`; `AGENT_ROLES` holds `external`;
  `hook.rs::DEFAULT_ROLES` has the row; the starter binds `claude-opus-5-5`; `stapel init` installed
  `.claude/agents/stapel-external-reviewer.md`.
- Step 5 (8d76561, 952142c): `reviewers()` names `stapel-external-reviewer`; the RED also changed the
  external line of `review.rs`'s test without a model and the expected model from fable to opus.
- Step 6 (92a357b, ebaf187): `Pack.ticket_base: Option<&str>`; `write_pack` takes the merge base of
  `refs/heads/<base branch>` and HEAD and falls back to the ticket base; the case with no merge is the
  existing goldens of rounds one and two.
- Step 7 (a245f3f, d227970): `Suite.failing`, `Suite.failing_more`, `cargo_failing_names` and
  `MAX_FAILING` = 20; the keys are written only when a name exists.
- Step 8 (66302cb, b73e607): `suite_junit` collects `<suite> <case>` with the case name trimmed, because
  jest-junit puts a leading space on top-level names.
- Step 9 (2a6ce01, 9605360): the builder asset, the skill's rule 6 and the CLAUDE.md rule on cargo runs;
  `stapel init` installed the copies.
- 2026-11-24, agent: the tests of `check_junit` with an absolute script path fail without a case-sensitive
  TMPDIR on this machine and pass with it; they are outside this ticket.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `cargo_lock::slots_setting`, `cargo_lock::default_slots_are_three` |
| AC-2 | `cargo_lock::runs_in_parallel_up_to_slots` |
| AC-3 | `cargo_lock::all_slots_held_waits`, `cargo_lock::two_waiting_lines` |
| AC-4 | `cargo_lock::one_run_per_worktree`, `cargo_lock::worktree_lock_outlives_its_holder` |
| AC-5 | `init_process::external_reviewer_agent` |
| AC-6 | `review::reviewers_by_route_and_tags` |
| AC-7 | `review::pack_base_after_a_main_merge`, `review::pack_base_edges` |
| AC-8 | `check_failing::cargo_names_failing_tests` |
| AC-9 | `check_failing::junit_names_failing_tests` |
| AC-10 | `init_process::builder_runs_the_privacy_test` |
| AC-11 | `cargo_lock::a_bad_config_refuses` |

## Plan

Route: full (tag `interface`), auto.

1. Slots setting (AC-1, AC-11; tests `cargo_lock::slots_setting`, `cargo_lock::a_bad_config_refuses`):
   the RED adds the tests and `[cargo] slots = 1` to the fixture `check_repo`; the GREEN adds `[cargo]` to
   `Config` and makes `stapel cargo` load the config. Must not change: a config without `[cargo]`.
2. Slots (AC-2, AC-3; tests `cargo_lock::runs_in_parallel_up_to_slots`, `cargo_lock::all_slots_held_waits`):
   the RED adds both tests, each run from a worktree of its own; the GREEN makes `take` try N slots and
   poll them when all are held. Must not change: the STP-27 tests.
3. Worktree lock (AC-4; test `cargo_lock::one_run_per_worktree`): the RED adds the test; the GREEN takes
   the worktree lock before a slot. Must not change: the held mark and the tests of step 2.
4. External agent (AC-5; test `init_process::external_reviewer_agent`): the RED also changes the old
   expectations the new agent breaks (`tests/golden/init_agent_frontmatter.txt`, the agent list and the
   `[roles]` rows of `init_process.rs`, `DEFAULT_ROLES` of `hook_tokens.rs`, the starter model of
   `external` in `crates/stapel-core/tests/config.rs`); the GREEN adds the agent
   file, the role, the `hook.rs` default row and the starter lines, and runs `stapel init`; the
   orchestrator adds the `[roles]` row of this repository before it. Must not change: the other agent
   files.
5. Reviewer lines (AC-6; test `review::reviewers_by_route_and_tags`): the RED changes the expected
   external line there and in the other test of `review.rs` that expects it; the GREEN names the agent in `reviewers`. Must not change: the other lines.
6. Pack base (AC-7; test `review::pack_base_after_a_main_merge`): the RED adds the test; the GREEN
   computes the pack base in `write_pack`. Must not change: the pack without a main merge and the merge
   round pack.
7. Cargo failing names (AC-8; test `check_failing::cargo_names_failing_tests`): the RED adds
   `tests/check_failing.rs` with the cargo test only; the GREEN parses the names and prints and records
   them. Must not change: the suite line.
8. Junit failing names (AC-9; test `check_failing::junit_names_failing_tests`): the RED adds the junit
   test to `tests/check_failing.rs`; the GREEN fills the names from the report. Must not change: the
   junit counts.
9. Texts (AC-10; test `init_process::builder_runs_the_privacy_test`): the RED adds the test; the GREEN changes the builder asset
   and the skill, runs `stapel init`, and changes the CLAUDE.md line on cargo runs. Must not change: the
   other rules.
10. Drift coverage of the slots (DR1-1, DR1-2; AC-1, AC-3, AC-4; tests
   `cargo_lock::default_slots_are_three`, `cargo_lock::two_waiting_lines`,
   `cargo_lock::worktree_lock_outlives_its_holder`): a coverage commit only, no RED and no GREEN, because
   steps 1 to 3 already do this.
11. Drift coverage of the pack base (DR1-3; AC-7; test `review::pack_base_edges`): a coverage commit only.

## Review

TODO

<!-- stapel:review:begin -->
Generated by stapel; edits between the markers are overwritten.

Findings:

| Stage | Round | Roles | HIGH | MEDIUM | LOW | False | Fates |
|---|---|---|---|---|---|---|---|
| spec-review | 1 | external | 1 | 5 | 3 | 0 | defer 1, no-change 1, patch 7 |
| spec-review | 2 | external | 0 | 4 | 3 | 0 | patch 7 |
| step-drift | 1 | drift | 0 | 1 | 4 | 0 | no-change 1, patch 4 |

Check runs: none
<!-- stapel:review:end -->

## Summary

TODO
