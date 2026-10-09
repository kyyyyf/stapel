# STP-6 — Hardening of stapel check against deliberate circumvention

## Description

Source: `docs/PHASES.md`, phase 0, STP-6; the Guarantees of STP-4 ("Not promised", code review E-1, E-4,
E-5, E-7, E-13).

### Frame

**Problem.** `stapel check` runs `cargo test` and trusts what that run reports. STP-4 promised this only
against a careless agent. A GREEN commit can still make a test pass without the code under test: it can
redirect a test target in a manifest (`[[test]] path`, `harness = false`), add a `.cargo/config.toml`
runner, wrapper or `rustflags`, add a `build.rs`, place code under `crates/*/tests/` for a later step, or
include an allowed path at compile time (`include!`, `include_str!`). The run also inherits the caller's
environment: `PATH`, `RUSTC`, `RUSTC_WRAPPER`, `CARGO_*` and `RUSTFLAGS` decide which tools run
(`crates/stapel-core/src/runner.rs` set only `CARGO_TARGET_DIR` and `CARGO_TERM_COLOR`).

**Risk tags:** `security`, `guard`.

**Non-goals.** Runners other than `cargo`; a sandbox for the test process (network, file system); a forged
line in `runs.jsonl` or a deleted `[check]` (the STP-1 hole); rewritten history; whether the tests are
good tests; unit tests inside `crates/*/src`.

**Promise (closed list).** Against an agent that tries on purpose to make a step look verified,
`stapel check` gives `unverified` or a failing outcome, never `pass`, when a commit of the ticket's range
(Decisions 18 and 19) that is not a RED of the ticket:

1. changes a target table or target key of any `Cargo.toml`, or adds or deletes a `Cargo.toml`;
2. adds, changes or deletes a cargo configuration file, a `.cargo` path, a `rust-toolchain` file, a build
   script or any symbolic link;
3. changes a `.rs` file under `crates/*/tests/` that holds no step test, or a file that a step test or such
   a helper includes by a literal path.

A RED of the ticket that lies between another step's RED and GREEN makes that step `unverified`.

And: a RED that changes a file a source file includes by a literal path is `red-changes-code`; the check's
worktree lies outside the repository, and a cargo configuration or toolchain file in a folder above it
makes every step `unverified`; every cargo run starts from an allow-listed environment and one `cargo`
program, resolved before the first run and named in the report.

Not promised: cargo configuration in `CARGO_HOME`; `rustup` overrides and default toolchains; a compromised
toolchain; a `rustc`, linker or other tool found through `PATH`; dependency redirection (`path`, `git`,
`package`, `[patch]`, `[replace]`) and `include*!` with a non-literal argument or a path outside the
repository, a `Cargo.toml` added by a RED under `crates/*/tests/`, includes followed through another included
file (split to STP-13, Decisions 13, 21 and 30);
proxy and certificate variables of the caller; proc-macro crates that read files.

**Size.** Medium: about fourteen criteria, 400–500 changed lines, after the split of Decision 13.

### Decisions

- 2026-10-08: the frame is confirmed by the human.
- 2026-10-08, questions, group 1: (1) Decision: every cargo run starts from a cleared environment plus an
  allow list (`HOME`, `USER`, `PATH`, `LANG`, `TMPDIR`, `CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN`) and
  the variables the check sets; as recommended. (2) Decision: `cargo` is taken from a `[check] cargo` key in
  `stapel.toml`, else from `PATH`, resolved once before the first run; the report and `runs.jsonl` show its
  path and `cargo --version`; the human chose the key over the recommended `PATH` only. (3) Decision: a
  change named by the promise gives `unverified`, naming the file and commit; as recommended. (4) Decision:
  the rules apply to every ticket, STP-1 to STP-5 included; a step of a closed ticket that becomes
  `unverified` is an escaped defect; as recommended.
- 2026-10-08, questions, group 2, all as recommended: (5) Decision: a cargo configuration or `rust-toolchain`
  file present at the ticket's base and unchanged by the step is used, and the report names its path; only
  a change after RED gives `unverified`. (6) Decision: literal-path `include!`, `include_str!` and
  `include_bytes!` are found by a text search over every `.rs` file at HEAD; an included file counts as
  code (the RED rule forbids it in RED), and a change to it after RED on a step test's path gives
  `unverified`; the include check stays in STP-6. (7) Decision: a helper is any file under
  `crates/*/tests/` other than a top-level `.rs` target and a golden file; a change after RED to a helper
  that a step test uses gives `unverified`; other test targets may change.
- Open question: how "a helper that a step test uses" is found (`mod` declarations and `#[path]` from the
  test file); to settle in Design.
- 2026-10-08, spec review round 1 (10 findings, `findings.jsonl`): every inside-promise finding is fixed in
  the text (SR-1, SR-2, SR-4, SR-5, SR-7, SR-9 and the LOW items); the open question above is closed by
  SR-1. Decisions, all as recommended: (8) SR-1: every `.rs` file under `crates/*/tests/<dir>/` other than a
  top-level target, changed after RED, gives `unverified`; this replaces Decision 7's reachability. (9) SR-2:
  the check's worktree moves outside the repository (`TMPDIR`); a `.cargo/` or `rust-toolchain*` in any
  ancestor of it other than `CARGO_HOME` makes every step `unverified`. (10) SR-3, promise widened: a
  dependency entry that gains or changes `path`, `git` or `package`, any `[patch]` or `[replace]` table, or a
  `Cargo.toml` under `crates/*/tests/` gives `unverified` after RED and `red-changes-code` in a RED;
  crates.io dependencies and paths to workspace members stay allowed. (11) SR-6, promise widened: a build
  input first added or changed by a commit of the ticket gives `unverified` even before the first RED;
  Decision 5 holds only for inputs older than the ticket. (12) SR-8, promise widened: an `include*!` with a
  path outside the repository or a non-literal argument, in a step test, its helpers or a source file,
  gives `unverified`.
- 2026-10-08, after spec review round 1: (13) Decision, the human chose "the more efficient" and the
  orchestrator chose a split: SR-3 (dependency redirection) and SR-8 (non-literal and outside includes) go
  to STP-13, so the widened promise stays within one review. Decisions 10 and 12 move with them. (14)
  Decision: since a build input changed by any non-RED commit of the range is seen by the HEAD run of every
  step, the build-input outcome marks every step, not only the one whose GREEN made the change (refines
  Decisions 3 and 11). (15) Decision: a step test's `.rs` helpers are protected whole, without `mod`
  parsing (Decision 8); included files are found at base, REDs, GREENs and HEAD (SR-7).
- 2026-10-08, spec review round 2 (13 findings): the human answered "all as recommended" and started the
  build. Inside-promise findings SR2-3, SR2-4, SR2-5, SR2-8, SR2-9, SR2-10 are fixed in the text.
  Decisions: (16) SR2-1, promise widened: a RED of the ticket between another step's RED and GREEN makes
  that step `unverified`. (17) SR2-2: a Helper is any `.rs` file under `crates/*/tests/` that holds no step
  test of the ticket; the top-level exception is dropped. (18) SR2-6: for a closed ticket the range ends at
  the last commit whose subject carries its key. (19) SR2-7: `stapel new` records `base`, the HEAD commit,
  in `state.json`; the range starts after it; a ticket without `base` keeps the subject rule. (20) SR2-11:
  of `[workspace]` only `members`, `exclude` and `default-members` count. (21) SR2-12 stays with STP-13;
  SR2-13: proxy and certificate variables are not in the allow list (not promised).
- 2026-10-08, during step 1: (22) Decision, as recommended: `CARGO_BUILD_JOBS` joins the allow list of AC-8;
  it changes only parallelism, and without it nested test builds take every core (the STP-4 crash).
  (23) Decision: STP-13 is taken right after STP-6; plan step 6 writes it into `docs/PHASES.md`.
- 2026-10-08, step 2 drift review (D2-1 to D2-6), all as recommended: (24) only `config` and `config.toml`
  directly in `CARGO_HOME` are skipped; `rust-toolchain*` never; a `CARGO_HOME` equal to the worktree's folder
  or above it counts as found (D2-1). (25) Spec change AC-7: `build-input-outside` follows the build-input
  term's order, STP-4's history outcomes first (D2-2). (26) A stale `stapel-check-*` worktree registration
  whose folder is gone is pruned before a check; other worktrees are not touched (D2-4). (27) An ancestor
  that cannot be read counts as found (D2-6).
- 2026-10-08, step 3: (28) Decision, proposed by the builder and agreed by the human: a RED of the ticket
  that changes code (`red-changes-code`) counts as a non-RED commit for the build-input rules of the other
  steps, since their HEAD runs use that input. The golden fix of step 3 was moved into its RED by the human
  (a local rewrite of two unpushed commits; old ids 32d43b1 and 2cfbac9).
- 2026-10-08, after step 3: (29) Decision: STP-14, `stapel check --history`, follows STP-13 (`docs/PHASES.md`).
  Builder briefs from step 4 on: a GREEN changes nothing under `crates/*/tests/`; a coverage commit only adds
  `#[test]` functions with their helpers nested inside; `stapel check` runs after every builder commit.
- 2026-10-08, step 4 drift review (D4-1 to D4-6), all as recommended: (30) spec change of the term Included
  file: every escape is decoded and an undecodable literal counts as a change (D4-1); the relation is read at
  every commit of the range, not only at REDs, GREENs and HEAD (D4-3); a chain of includes is not promised and
  goes to STP-13 (D4-4). D4-2, D4-5 and D4-6 get coverage tests.
- 2026-10-09, step 5 drift review (D5-1 to D5-5), all as recommended: (31) spec change: the Range is the
  range of the build-input rules only (D5-1); "carries its key" reads "starts with `<KEY> ` or `<KEY>:`"
  (D5-2); `stapel new` fails on a git error other than a repository without commits (D5-4). D5-3 and D5-5
  get coverage tests.
- 2026-10-09, after plan step 7: (32) Decision, as recommended: AC-16 is added. Plan step 7 showed that later
  tickets that change shared test files (fixtures, golden files, `check.rs`) make every older closed ticket
  `tests-changed`, which hides the real escaped defects (STP-3 step 3 `no-red`; STP-2 step 6 changed a helper).
  A closed ticket is checked as it was at its last commit.
## Spec

Criteria are short (`CLAUDE.md`, the spec process, item 3); exact output is in golden files. Terms:

- **Range** (of the build-input rules; STP-4's history outcomes keep their own walk from each RED): the
  first-parent commits after `base` of `state.json` (else from the oldest commit whose
  subject starts with `<KEY> ` or `<KEY>:`) up to HEAD, or, for a closed ticket, up to the last commit
  whose subject starts with `<KEY> ` or `<KEY>:`. A **non-RED commit** is a commit of the range that is not a RED of the
  ticket (STP-4's rule).
- **Helper**: a `.rs` file under `crates/*/tests/`, any depth, that holds no step test of the ticket.
- **Included file**: a path named by `include!`, `include_str!` or `include_bytes!` with a string literal
  (raw strings included, every Rust escape decoded; a literal that cannot be decoded counts as a change of its
  includer), relative to the including file, found by a text search (comments count) at the range's base and
  at every commit of the range.
- **Build-input outcome**: every step of the ticket reads `unverified: build-input-changed: <path> at
  <short sha>`, naming the first such commit and path (golden file `check_report_build_inputs.txt`). A step
  that STP-4's history rules already fail (`unpaired`, `duplicate`, `no-tests`, `red-changes-code`,
  `tests-changed`) keeps that outcome; the rest get this one. With it, no cargo runs and the report reads
  `suite: unverified: build-input-changed`.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF a non-RED commit changes, in any `Cargo.toml`, a `[lib]`, `[[bin]]`, `[[test]]`, `[[example]]`, or `[[bench]]` table, `members`, `exclude` or `default-members` of `[workspace]`, or the `build`, `autolib`, `autobins`, `autotests`, `autoexamples` or `autobenches` key of `[package]`, or adds or deletes a `Cargo.toml` THEN the check gives the build-input outcome. | `check::manifest_target_change_is_unverified` |
| AC-2 | IF a non-RED commit adds, changes, deletes or renames a path whose last component is `.cargo`, `rust-toolchain` or `rust-toolchain.toml`, a file under a `.cargo` folder, a `build.rs`, the file a `[package] build` key names, or any symbolic link THEN the check gives the build-input outcome. | `check::config_toolchain_build_script_and_links_are_unverified` |
| AC-3 | IF a non-RED commit changes a helper THEN the check gives the build-input outcome. | `check::helper_change_is_unverified` |
| AC-4 | IF a non-RED commit changes a file that a step test's file or a helper includes THEN the check gives the build-input outcome. | `check::included_test_data_change_is_unverified` |
| AC-5 | IF a RED changes a file that a `.rs` file outside `crates/*/tests/` includes, at the RED, its parent, any later GREEN or HEAD THEN the step is `red-changes-code` and the path is named. | `check::red_changing_an_included_file_changes_code` |
| AC-6 | WHEN files of AC-2 exist at HEAD and no non-RED commit changes them THE report prints `build inputs: <path>, …` before the step lines, and the steps keep their outcomes. | `check::unchanged_build_inputs_are_named` |
| AC-7 | WHEN the check runs THE worktree is a new folder with a unique name under `TMPDIR`, outside the repository folder; IF a folder above the worktree, other than `CARGO_HOME`, holds `.cargo/config`, `.cargo/config.toml`, `rust-toolchain` or `rust-toolchain.toml` THEN every step that STP-4's history rules do not fail reads `unverified: build-input-outside: <path>`, found before `cargo --version` runs; only `config` and `config.toml` directly in `CARGO_HOME` are skipped, and a `CARGO_HOME` that is the worktree's folder or a folder above it counts as found. | `check::worktree_is_outside_the_repository`, `check::config_above_the_worktree_is_unverified` |
| AC-8 | WHEN the check starts a cargo run THE run gets an empty environment plus `HOME`, `USER`, `PATH`, `LANG`, `TMPDIR`, `CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN`, `CARGO_BUILD_JOBS` when set in the caller, and the variables the check sets; no other variable of the caller reaches it. | `check::cargo_runs_with_the_allow_list_only` |
| AC-9 | WHEN the check starts THE cargo program is `[check] cargo` if set, else the first `cargo` in an absolute `PATH` entry, resolved once; every run uses it; the report prints `cargo: <path> (<version>)`, the first line of `cargo --version` cut to 200 bytes, and the run record carries `cargo` with the same text. | `check::cargo_path_and_version_are_reported` |
| AC-10 | IF `[check] cargo` is not an absolute path to an executable regular file, or lies under the repository folder after links are resolved THEN loading the configuration fails, naming the key. | `core::config::check_cargo_key_is_validated` |
| AC-11 | IF no `cargo` is found, or `cargo --version`, run in the worktree with the AC-8 environment, fails or passes `timeout_secs` THEN `stapel check` exits 2 and appends no record. | `check::refuses_without_a_working_cargo` |
| AC-12 | IF a RED of the ticket lies between another step's RED and its GREEN THEN that step reads `unverified: interleaved: <label of the RED>`. | `check::interleaved_red_is_unverified` |
| AC-13 | WHEN `stapel new` creates a ticket THE `state.json` holds `base` with the full id of HEAD (absent only in a repository without commits; any other git error fails `new`), and the range starts after it. | `new::records_the_base_commit`, `check::range_starts_after_the_base` |
| AC-14 | IF `TMPDIR`, after links are resolved, lies under the repository folder, or the worktree folder cannot be created THEN `stapel check` exits 2 and appends no record. | `check::refuses_a_tmpdir_inside_the_repository` |
| AC-15 | WHEN the ticket is closed THE range ends at the last commit whose subject starts with `<KEY> ` or `<KEY>:`, so later commits do not change its build-input outcome. | `check::closed_ticket_range_ends_at_its_last_commit` |
| AC-16 | WHEN the ticket is closed THE check reads STP-4's history outcomes only up to the end of its Range and runs its steps' tests and the suite at that end commit instead of HEAD; the report's first line reads `ticket: <KEY> at <short sha of the end> (closed)` and the run record carries `end` with its full id, `head` staying HEAD. | `check::closed_ticket_is_checked_at_its_last_commit` |

### Guarantees

- **Promised, against an agent that tries on purpose to make a step look verified** (tag `security`): the
  Frame's promise, by AC-1 to AC-16.
- **Not promised:** the Frame's list; a symbolic link or file outside the repository changed without a
  commit; artifacts planted in the check's target folder (`.git/stapel/`, which the guard protects); the
  outcome of a closed ticket's history before this ticket (Decision 4: a step that turns `unverified` is an
  escaped defect, recorded, not hidden).

## Design

### Facts (external contract and states)

| Claim | Source | Mark |
|---|---|---|
| Cargo reads `.cargo/config.toml` (and `.cargo/config`) from the current folder and every folder above it, then from `CARGO_HOME` | Cargo book, "Configuration", hierarchical structure | documented |
| `rustup` picks a toolchain from `RUSTUP_TOOLCHAIN`, then a directory override, then `rust-toolchain.toml` or `rust-toolchain` in the current folder or above, then the default | rustup book, "Overrides" | documented |
| The check's worktree today is `.git/stapel/check-worktree` with a fixed name, guarded by the lock in `.git/stapel/` | `crates/stapel-core/src/worktree.rs` | verified |
| Cargo runs inherit the caller's environment; only `CARGO_TARGET_DIR` and `CARGO_TERM_COLOR` are set; the program is `cargo` from `PATH` | `crates/stapel-core/src/runner.rs` | verified |
| STP-4's RED rule (`is_code`): `build.rs` and any `.rs` outside `crates/*/tests/` are code; a manifest change outside `dev-dependencies` is code; other paths outside tests, docs, `.stapel/` and `*.md` are code, so a RED that adds `.cargo/config.toml` or `rust-toolchain.toml` is already `red-changes-code` | `crates/stapel-core/src/outcomes.rs` | verified |
| A RED may add any file under `crates/*/tests/`, a symbolic link included; such a link pointing at source code is not promised | same | verified |

### Decisions (as built)

Written as notes during the build and moved here before code review (`CLAUDE.md`, item 6). Commits are found
by their subjects (`STP-6 step <n> …`); two local rewrites changed their ids (Decision 28, plan step notes).

#### Step 1
- `Config::parse` stays lenient and does not validate `[check] cargo`; only `Config::parse_at(text, root)` does, so the guard keeps working with a stale path.
- The 200-byte cut is made after lossy decoding, at a char boundary, trailing whitespace trimmed.
- `cargo --version` goes through `run_cargo`: same environment, process group and `timeout_secs`; failure exits 2 with `<path>: …`.
- `CARGO_BUILD_JOBS` no longer reaches cargo (AC-8); the tests' `CARGO_BUILD_JOBS=2` is a no-op now. Open: see the human's answer.
- Drift fixes: `validate_cargo` is public and runs again before the `--version` run (a link retargeted after load is refused, exit 2); `resolve_cargo` takes only executable regular files from `PATH`.
#### Step 2
- `resolve_tmpdir` (worktree.rs): unset or empty TMPDIR is `/tmp`; relative is resolved against the current folder; links resolved; missing, not a folder, or under the canonical repository folder: exit 2 `TMPDIR …`, before the worktree is created, so no record.
- Worktree folder `stapel-check-<pid>-<nanos>-<n>` made with `create_dir`; removed only by its own Drop; a crashed run's leftover is left alone; no `git worktree prune`.
- `create` still removes the legacy `.git/stapel/check-worktree`; lock and `check-target` stay under `.git/stapel/`.
- `build_input_outside` walks the worktree's ancestors; skips CARGO_HOME (caller's, else `$HOME/.cargo`, links resolved). When found: no `cargo --version`, the `cargo:` line shows the path only, every step and the suite read `unverified: build-input-outside: <path>`, exit 1, record written.
- Step 2 drift fixes: history outcomes already came first (`Analysis.fixed` is read before `Runner::step`); `build_input_outside` returns CARGO_HOME when an ancestor resolves to it, skips only `.cargo/config*` when `<folder>/.cargo` is CARGO_HOME, and counts any stat error but NotFound as found; `prune_stale_checks` removes `<common>/worktrees/<id>` only for a gone `stapel-check-*` folder; `run_cargo` passes `TMPDIR` as an absolute path (a relative one broke the child's build).
#### Step 3
- New module `stapel-core/src/build_inputs.rs`; `range()` is one function (step 5 changes start and end); `analyse()` reads `git diff-tree -r -z -M --raw` of the non-RED commits.
- A RED that changes code counts as non-RED for the build-input rules (Decision 28); new `outcomes::red_changes_code`, `named` made public.
- With `build-input-changed` no cargo runs, `cargo --version` included; the `cargo:` line shows the path only; `build-input-changed` comes before `build-input-outside`.
- A rename names the old path first; the `build` key is read at parent and commit; the `build inputs:` line is capped at 20 paths by `named()`.
- Step 3 drift fixes: reading fails closed as "counts as a change": a manifest that does not parse or whose `git show` fails hides its `build` key, so any entry in that manifest's folder (base or commit) counts and is named; a malformed `diff-tree` record or short output counts as `diff of <sha>`; only a malformed `ls-tree` record is an error (exit 2).
#### Step 4
- New `stapel-core/src/includes.rs`: text search for `include!`, `include_str!`, `include_bytes!` with plain or raw literals, comments count; paths relative to the including file; absolute paths and paths leaving the repository skipped (STP-13); cached per blob.
- A Helper is any `.rs` under `crates/*/tests/` that is no step-test file of a RED in the range; AC-3/AC-4 hits come after the manifest check.
- AC-4 includers: every `.rs` under `tests/`; relation at base, every RED and GREEN, HEAD. AC-5 in `outcomes::analyse`, includers outside `tests/` read at RED, parent, later GREENs, HEAD.
- AC-12: `Analysis.interleaved`; after the build-input outcome, before running; STP-4 history outcomes (and `retired`) first.
- The step-4 RED changed the STP-4 test `suite_failure_fails_the_check`: its keyed non-RED commit adding `tests/other.rs` is now a Helper change; the setup commit became unkeyed and a case for a keyed new test file was added. To verify with `stapel check STP-4` in plan step 7.
- Step 4 drift fixes: `literals()` returns `Option<String>` per literal, `None` for an undecodable one, whose includer then joins the included set; `seen` is HEAD, the parent of the first range commit and every range commit, cached per blob. A RED that adds a test to `tests/common/mod.rs` does not make it a step file: a later change gives the build-input outcome.
#### Step 5
- `State.base: Option<String>`; old `state.json` files load; `stapel new` takes `git rev-parse --verify HEAD`, absent without commits.
- `build_inputs::range` takes `Bounds { base, closed }`: after `base` on the first-parent line, else the subject rule; a closed ticket (`closed` in `state.json`, written by `stapel close` with its fact) ends at its last keyed commit, empty if none.
- `build_inputs::check_base`: 40 or 64 lowercase hex and on `git rev-list --first-parent HEAD`, checked before `step_commits`; else exit 2 `state.json has base <x>, which is not a full commit id on the first-parent history of HEAD`.
- Step 5 drift fixes: `read_base` in `new.rs` runs before the folder is made; `git rev-parse --verify -q HEAD`: success records `base`, exit 1 with empty stderr records none (an unborn HEAD; git also reports a ref with a garbage hash so), anything else fails `new` with `git rev-parse --verify HEAD: <stderr>`.
#### Step 8
- Only `check.rs` changed: for a closed ticket `build_inputs::range(...).last()` is `end`; it replaces HEAD in `outcomes::analyse`, `Worktree::create` and `Runner.head` (step runs, suite, `removed`); `build_inputs::analyse` keeps the real HEAD with `Bounds { closed }`. First line `ticket: K at <short end> (closed)`; the record gets `end`, `head` stays HEAD; open tickets unchanged, no `end`. A closed ticket with an empty range: exit 2 naming it (untested).

## Test plan

CLI tests build a tiny `cargo` crate in a temporary git repository, as in STP-4, and commit RED and GREEN
steps into it; each sets its own `CARGO_TARGET_DIR`. Heavy runs: `cargo test -j 4 … -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | unchanged build inputs are named; the cargo path and version; the worktree's place | `check::unchanged_build_inputs_are_named`, `check::cargo_path_and_version_are_reported`, `check::worktree_is_outside_the_repository` |
| Negative | each build input changed by a non-RED commit, before and after the first RED; a RED that changes an included source file; bad `[check] cargo`; no working cargo; a config above the worktree | `check::manifest_target_change_is_unverified`, `check::config_toolchain_build_script_and_links_are_unverified`, `check::helper_change_is_unverified`, `check::included_test_data_change_is_unverified`, `check::red_changing_an_included_file_changes_code`, `core::config::check_cargo_key_is_validated`, `check::refuses_without_a_working_cargo`, `check::config_above_the_worktree_is_unverified`, `check::interleaved_red_is_unverified`, `check::refuses_a_tmpdir_inside_the_repository` |
| Ranges | the base commit; a closed ticket's range; a closed ticket checked at its end | `new::records_the_base_commit`, `check::range_starts_after_the_base`, `check::closed_ticket_range_ends_at_its_last_commit`, `check::closed_ticket_is_checked_at_its_last_commit` |
| Abuse | see the Abuse table | `check::cargo_runs_with_the_allow_list_only` and the Negative tests |
| Step 1 drift review | every allowed variable and one cargo; the cargo key through the CLI; the cargo key checked again before the run (a link retargeted after load); a `cargo` on `PATH` that is not executable | `check::cargo_runs_see_every_allowed_variable_and_one_cargo`, `check::cargo_key_is_validated_through_the_cli`, `check::cargo_key_is_checked_again_before_the_run`, `check::path_cargo_must_be_executable` |
| Step 2 drift review | `CARGO_HOME` at or above the worktree; history outcomes first; relative and unwritable `TMPDIR`; stale check worktrees after a crash; an unreadable ancestor | `check::cargo_home_cannot_hide_a_config_above`, `check::history_outcomes_come_before_build_input_outside`, `check::relative_tmpdir_is_resolved`, `check::stale_check_worktrees_are_pruned`, `check::unwritable_tmpdir_is_refused`, `check::unreadable_ancestor_counts_as_found` |
| Step 3 drift review | a RED with only a build input; an unkeyed commit in the range; dotted and inline TOML targets; other `[workspace]` keys; reading that fails closed | `check::red_with_only_a_build_input_changes_code`, `check::unkeyed_commit_in_the_range_is_seen`, `check::dotted_and_inline_toml_targets_are_seen`, `check::other_workspace_keys_are_not_build_inputs`, `check::build_input_reading_fails_closed` |
| Step 4 drift review | a helper turned into a step file; include reading that fails closed; three interleaved steps and a duplicate label; escapes in include literals; an include seen only in a middle commit | `check::helper_turned_step_file_stays_protected`, `check::include_reading_fails_closed`, `check::interleaving_with_three_steps`, `check::include_escapes_are_decoded`, `check::include_seen_in_a_middle_commit` |
| Step 5 drift review | prefix keys, `base` at HEAD, merges, commits after a close; exact refusals of a bad `base`; a git error in `stapel new` | `check::range_edges`, `check::bad_base_is_refused_exactly`, `new::base_read_errors_fail_new` |
| Integration | `stapel check STP-1` to `STP-5` on this repository (plan step 7, by hand; Decision 4); its output goes into Proof | — |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[check] cargo` | string | an absolute path to an executable regular file outside the repository | absent: `cargo` on `PATH` | relative, missing, a folder, not executable, under the repository: configuration error naming the key | byte-exact path, no `~` expansion; links resolved for the repository test | `core::config::check_cargo_key_is_validated` |
| Caller environment | variables | any | no `cargo` found: exit 2 | relative `PATH` entries: skipped | names compared byte-exactly | `check::cargo_runs_with_the_allow_list_only`, `check::refuses_without_a_working_cargo` |
| `TMPDIR` | path | any absolute folder outside the repository | unset: `/tmp` | relative: resolved against the current folder; under the repository or not creatable: exit 2 | links resolved | `check::refuses_a_tmpdir_inside_the_repository` |
| `cargo --version` output | bytes | first line, cut to 200 bytes | empty: the version reads `?` | non-UTF-8: read lossily | — | `check::cargo_path_and_version_are_reported` |
| `Cargo.toml` changes | TOML diff | every manifest of the tree | added or deleted: a change | unparsable at either side: the build-input outcome naming it | tables and keys compared as parsed values | `check::manifest_target_change_is_unverified` |
| `include*!` literals | Rust text | every `.rs` file at base, REDs, GREENs, HEAD | none: no included files | non-UTF-8 file: read lossily; non-literal argument or path outside the repository: not promised (STP-13) | plain and raw strings; matches in comments count | `check::included_test_data_change_is_unverified`, `check::red_changing_an_included_file_changes_code` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| cargo | workspace with several packages | manifests read at every depth; an added member's manifest is AC-1 | `check::manifest_target_change_is_unverified` |
| cargo | custom targets (`[[test]]`, `harness = false`, `autotests = false`) | AC-1 | `check::manifest_target_change_is_unverified` |
| cargo | `.cargo/config.toml` at any level, wrappers, runners | in the repository: AC-2, AC-6; above the worktree: AC-7; in `CARGO_HOME`: not promised | `check::config_toolchain_build_script_and_links_are_unverified`, `check::config_above_the_worktree_is_unverified` |
| cargo | environment `CARGO_*`, `RUSTC*`, `RUSTFLAGS`, `PATH` | AC-8, AC-9; tools found through `PATH`: not promised | `check::cargo_runs_with_the_allow_list_only` |
| cargo | `Cargo.lock` out of date; offline; network errors | unchanged from STP-4 (`--locked`) | — |
| cargo | build scripts and `include!` of files outside `src` | AC-2, AC-4, AC-5 | `check::included_test_data_change_is_unverified` |
| cargo | libtest output format | unchanged from STP-4 | — |
| git | renames and similarity | a rename changes both paths | `check::config_toolchain_build_script_and_links_are_unverified` |
| git | paths with spaces or glob characters | literal paths, as STP-4 | `check::included_test_data_change_is_unverified` |
| git | submodules | not promised | — |
| git | history shape, shallow clone, object format, detached HEAD, dirty tree, configuration | unchanged from STP-4 | — |
| os | symlinks | any link changed by a non-RED commit: AC-2; a link's target outside the repository: not promised | `check::config_toolchain_build_script_and_links_are_unverified` |
| os | case-insensitive file systems | not promised (names compared byte-exactly) | — |
| os | file permissions | the executable bit of `[check] cargo`: AC-10; other modes: unchanged from STP-4 | `core::config::check_cargo_key_is_validated` |
| os | processes: signals, groups, children | unchanged from STP-4; `cargo --version` runs under `timeout_secs` | `check::refuses_without_a_working_cargo` |
| os | time | not used by this ticket | — |
| os | WSL: Windows binaries on `PATH` | a `cargo.exe` is not `cargo`: not found; not promised beyond that | — |

### Abuse table

| Attempt | Outcome |
|---|---|
| A GREEN that points `[[test]] path` at a file that always passes, or sets `harness = false` | build-input outcome, AC-1 |
| A GREEN that adds a crate with `members = ["crates/*"]` | build-input outcome, AC-1 (added manifest) |
| A GREEN that adds `.cargo/config.toml` with a `runner` or `rustc-wrapper`, or a `.cargo` link | build-input outcome, AC-2 |
| A GREEN that adds `build.rs` or `rust-toolchain.toml` | build-input outcome, AC-2 |
| A ticket commit before the first RED that adds a build input | build-input outcome (Decision 11) |
| A GREEN that moves code into `tests/common/mod.rs`, or into a helper reached by `#[path]` | build-input outcome, AC-3 |
| A GREEN that edits a file a test includes with `include_str!` | build-input outcome, AC-4 |
| A RED that adds code as a file the library includes with `include!` | `red-changes-code`, AC-5 |
| An ignored `.cargo/config.toml` in the main tree, or one in a folder above the repository | not seen by the worktree (AC-7); above the worktree: `build-input-outside` |
| A caller that sets `RUSTC_WRAPPER`, `RUSTFLAGS` or `CARGO_BUILD_RUSTC` | removed, AC-8 |
| A `PATH` with a fake `cargo` first | it runs; the report and the record name its path and version (AC-9); not prevented |
| A `[check] cargo` that points at a script in the repository | configuration error, AC-10 |
| A later step's RED, between step 1's RED and GREEN, that rewrites step 1's helper | `interleaved`, AC-12 |
| A GREEN that edits a top-level `tests/common.rs` that a step test reaches by `mod common;` | build-input outcome, AC-3 |
| A non-keyed commit before the ticket's first keyed commit that adds a build input | inside the range after `base`, AC-13 |
| A `.cargo/config.toml` in a shared `/tmp` from another user | `build-input-outside`, AC-7 (a denial, not a pass) |

**Author self-check (CLAUDE.md item 7).** Done on 2026-10-08 against the Abuse table, after step 5 and before
code review: every row has a test — target tables and the members glob (`check::manifest_target_change_is_unverified`,
`check::dotted_and_inline_toml_targets_are_seen`); `.cargo`, links, `build.rs`, toolchain
(`check::config_toolchain_build_script_and_links_are_unverified`, `check::red_with_only_a_build_input_changes_code`);
a commit before the first RED (`check::unkeyed_commit_in_the_range_is_seen`, `check::range_starts_after_the_base`);
helpers, a top-level `tests/helper.rs` included (`check::helper_change_is_unverified`,
`check::helper_turned_step_file_stays_protected`); included test data (`check::included_test_data_change_is_unverified`,
`check::include_escapes_are_decoded`, `check::include_seen_in_a_middle_commit`); an included source file in a RED
(`check::red_changing_an_included_file_changes_code`); configuration in or above the main tree
(`check::worktree_is_outside_the_repository`, `check::config_above_the_worktree_is_unverified`,
`check::cargo_home_cannot_hide_a_config_above`); the caller's variables (`check::cargo_runs_with_the_allow_list_only`,
`check::cargo_runs_see_every_allowed_variable_and_one_cargo`); a fake `cargo` on `PATH`, named, not prevented
(`check::cargo_path_and_version_are_reported`); a `[check] cargo` in the repository
(`core::config::check_cargo_key_is_validated`, `check::cargo_key_is_validated_through_the_cli`); interleaved REDs
(`check::interleaved_red_is_unverified`, `check::interleaving_with_three_steps`). New since the spec: none.

**Review Focus.** Unchecked: an honest GREEN that adds a `[[test]]` (build-input outcome for every step;
accepted by Decision 3); a link added by a RED under `crates/*/tests/` (allowed by STP-4's rule; see Design
facts); `RUSTUP_TOOLCHAIN` choosing another toolchain (not
promised).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check::manifest_target_change_is_unverified` |
| AC-2 | `check::config_toolchain_build_script_and_links_are_unverified` |
| AC-3 | `check::helper_change_is_unverified` |
| AC-4 | `check::included_test_data_change_is_unverified` |
| AC-5 | `check::red_changing_an_included_file_changes_code` |
| AC-6 | `check::unchanged_build_inputs_are_named` |
| AC-7 | `check::worktree_is_outside_the_repository`, `check::config_above_the_worktree_is_unverified` |
| AC-8 | `check::cargo_runs_with_the_allow_list_only` |
| AC-9 | `check::cargo_path_and_version_are_reported` |
| AC-10 | `core::config::check_cargo_key_is_validated` |
| AC-11 | `check::refuses_without_a_working_cargo` |
| AC-12 | `check::interleaved_red_is_unverified` |
| AC-13 | `new::records_the_base_commit`, `check::range_starts_after_the_base` |
| AC-14 | `check::refuses_a_tmpdir_inside_the_repository` |
| AC-15 | `check::closed_ticket_range_ends_at_its_last_commit` |
| AC-16 | `check::closed_ticket_is_checked_at_its_last_commit` |
| Manual run | plan step 7, 2026-10-09; run 1 at `dfc3fb6` (before AC-16): every closed ticket `fail`, mostly `tests-changed` from later tickets' changes of shared test files. Run 2 after step 8 (closed tickets checked at their last commit): STP-1 cannot be checked (legacy `state.json`); STP-5 `pass`; STP-4 `fail`, steps 3, 4, 5, 6, 7 `interleaved` (review pairs placed between a step's RED and GREEN); STP-3 `fail`, step 3 `no-red: stapel-cli/decisions::refusal_appends_nothing`, step 5 and code review round 1 `tests-changed` by STP-3's own later commits, steps 6 and 6b `Cargo.lock is out of date`; STP-2 `fail`, `tests-changed` by its own later commits and `build-input-changed: crates/stapel-cli/tests/common/mod.rs at 7a4c754` (STP-2 step 6 GREEN). These are escaped defects by Decision 4; STP-2 and STP-3 predate the RED to GREEN check. |

## Plan

Route: full (source: `auto`; tags `security`, `guard`; about 500 lines).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | Environment allow list; `[check] cargo`; the cargo path and version; refusal without a working cargo | AC-8, AC-9, AC-10, AC-11 | outcomes of STP-4 tests |
| 2 | Worktree in a unique folder under `TMPDIR`; configuration above it; bad `TMPDIR` | AC-7, AC-14 | the lock and target folder under `.git/stapel/` |
| 3 | Build inputs in manifests, configuration, toolchain, build scripts, links; the `build inputs` line | AC-1, AC-2, AC-6 | STP-4 report lines and golden files |
| 4 | Helpers and included files; interleaved REDs | AC-3, AC-4, AC-5, AC-12 | `tests-changed` for helpers the RED changed |
| 5 | `base` in `state.json`; range start and end | AC-13, AC-15 | `state.json` of earlier tickets |
| 6 | `docs/PHASES.md`: STP-13 in the list of first tickets | — | other phases |
| 8 | A closed ticket checked at its last commit; plan step 7 repeated | AC-16 | open tickets' check at HEAD |
| 7 | By hand: `stapel check STP-1` to `STP-5` on this repository; escaped defects recorded | — | closed tickets' text |

## Review

TODO

## Summary

TODO
