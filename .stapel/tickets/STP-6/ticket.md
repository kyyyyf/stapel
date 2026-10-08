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
(`crates/stapel-core/src/runner.rs:243` sets only `CARGO_TARGET_DIR` and `CARGO_TERM_COLOR`).

**Risk tags:** `security`, `guard`.

**Non-goals.** Runners other than `cargo`; a sandbox for the test process (network, file system); a forged
line in `runs.jsonl` or a deleted `[check]` (the STP-1 hole); rewritten history; whether the tests are
good tests; unit tests inside `crates/*/src`.

**Promise (closed list).** Against an agent that tries on purpose to make a step look verified,
`stapel check` gives `unverified` or a failing outcome, never `pass`, when a commit of the ticket's range
(first-parent commits from the ticket's first commit to HEAD) that is not a RED of the ticket:

1. changes a target table or target key of any `Cargo.toml`, or adds or deletes a `Cargo.toml`;
2. adds, changes or deletes a cargo configuration file, a `.cargo` path, a `rust-toolchain` file, a build
   script or any symbolic link;
3. changes a `.rs` file under `crates/*/tests/` other than a top-level test target, or a file that a step
   test or such a helper includes by a literal path.

And: a RED that changes a file a source file includes by a literal path is `red-changes-code`; the check's
worktree lies outside the repository, and a cargo configuration or toolchain file in a folder above it
makes every step `unverified`; every cargo run starts from an allow-listed environment and one `cargo`
program, resolved before the first run and named in the report.

Not promised: cargo configuration in `CARGO_HOME`; `rustup` overrides and default toolchains; a compromised
toolchain; a `rustc`, linker or other tool found through `PATH`; dependency redirection (`path`, `git`,
`package`, `[patch]`, `[replace]`) and `include*!` with a non-literal argument or a path outside the
repository (split to STP-13, Decision 13); proc-macro crates that read files.

**Size.** Medium: about eleven criteria, 400–500 changed lines, after the split of Decision 13.

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
## Spec

Criteria are short (`CLAUDE.md`, the spec process, item 3); exact output is in golden files. Terms:

- **Range**: the first-parent commits from the ticket's first commit (the oldest whose subject starts with
  `<KEY> ` or `<KEY>:`) to HEAD. A **non-RED commit** is a commit of the range that is not a RED of the
  ticket (STP-4's rule).
- **Helper**: a `.rs` file under `crates/*/tests/<dir>/`, any depth, that is not a top-level `.rs` target.
- **Included file**: a path named by `include!`, `include_str!` or `include_bytes!` with a string literal
  (raw strings included), relative to the including file, found by a text search (comments count) at the
  range's base, at every RED and GREEN, and at HEAD.
- **Build-input outcome**: every step of the ticket reads `unverified: build-input-changed: <path> at
  <short sha>`, naming the first such commit and path (golden file `check_report_build_inputs.txt`).

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF a non-RED commit changes, in any `Cargo.toml`, a `[lib]`, `[[bin]]`, `[[test]]`, `[[example]]`, `[[bench]]` or `[workspace]` table, or the `build`, `autolib`, `autobins`, `autotests`, `autoexamples` or `autobenches` key of `[package]`, or adds or deletes a `Cargo.toml` THEN the check gives the build-input outcome. | `check::manifest_target_change_is_unverified` |
| AC-2 | IF a non-RED commit adds, changes, deletes or renames a path whose last component is `.cargo`, `rust-toolchain` or `rust-toolchain.toml`, a file under a `.cargo` folder, a `build.rs`, the file a `[package] build` key names, or any symbolic link THEN the check gives the build-input outcome. | `check::config_toolchain_build_script_and_links_are_unverified` |
| AC-3 | IF a non-RED commit changes a helper THEN the check gives the build-input outcome. | `check::helper_change_is_unverified` |
| AC-4 | IF a non-RED commit changes a file that a step test's file or a helper includes THEN the check gives the build-input outcome. | `check::included_test_data_change_is_unverified` |
| AC-5 | IF a RED changes a file that a `.rs` file outside `crates/*/tests/` includes, at the RED or its parent THEN the step is `red-changes-code` and the path is named. | `check::red_changing_an_included_file_changes_code` |
| AC-6 | WHEN files of AC-2 exist at HEAD and no non-RED commit changes them THE report prints `build inputs: <path>, …` before the step lines, and the steps keep their outcomes. | `check::unchanged_build_inputs_are_named` |
| AC-7 | WHEN the check runs THE worktree lies outside the repository folder; IF a folder above the worktree, other than `CARGO_HOME`, holds `.cargo/config`, `.cargo/config.toml`, `rust-toolchain` or `rust-toolchain.toml` THEN every step reads `unverified: build-input-outside: <path>`. | `check::worktree_is_outside_the_repository`, `check::config_above_the_worktree_is_unverified` |
| AC-8 | WHEN the check starts a cargo run THE run gets an empty environment plus `HOME`, `USER`, `PATH`, `LANG`, `TMPDIR`, `CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN` when set in the caller, and the variables the check sets; no other variable of the caller reaches it. | `check::cargo_runs_with_the_allow_list_only` |
| AC-9 | WHEN the check starts THE cargo program is `[check] cargo` if set, else the first `cargo` in an absolute `PATH` entry, resolved once; every run uses it; the report prints `cargo: <path> (<version>)`, the first line of `cargo --version` cut to 200 bytes, and the run record carries `cargo` with the same text. | `check::cargo_path_and_version_are_reported` |
| AC-10 | IF `[check] cargo` is not an absolute path to an executable regular file, or lies under the repository folder after links are resolved THEN loading the configuration fails, naming the key. | `core::config::check_cargo_key_is_validated` |
| AC-11 | IF no `cargo` is found, or `cargo --version`, run in the worktree with the AC-8 environment, fails or passes `timeout_secs` THEN `stapel check` exits 2 and appends no record. | `check::refuses_without_a_working_cargo` |

### Guarantees

- **Promised, against an agent that tries on purpose to make a step look verified** (tag `security`): the
  Frame's promise, by AC-1 to AC-11.
- **Not promised:** the Frame's list; a symbolic link or file outside the repository changed without a
  commit; artifacts planted in the check's target folder (`.git/stapel/`, which the guard protects); the
  outcome of a closed ticket's history before this ticket (Decision 4: a step that turns `unverified` is an
  escaped defect, recorded, not hidden).

## Design

TODO

## Test plan

CLI tests build a tiny `cargo` crate in a temporary git repository, as in STP-4, and commit RED and GREEN
steps into it; each sets its own `CARGO_TARGET_DIR`. Heavy runs: `cargo test -j 4 … -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | unchanged build inputs are named; the cargo path and version; the worktree's place | `check::unchanged_build_inputs_are_named`, `check::cargo_path_and_version_are_reported`, `check::worktree_is_outside_the_repository` |
| Negative | each build input changed by a non-RED commit, before and after the first RED; a RED that changes an included source file; bad `[check] cargo`; no working cargo; a config above the worktree | `check::manifest_target_change_is_unverified`, `check::config_toolchain_build_script_and_links_are_unverified`, `check::helper_change_is_unverified`, `check::included_test_data_change_is_unverified`, `check::red_changing_an_included_file_changes_code`, `core::config::check_cargo_key_is_validated`, `check::refuses_without_a_working_cargo`, `check::config_above_the_worktree_is_unverified` |
| Abuse | see the Abuse table | `check::cargo_runs_with_the_allow_list_only` and the Negative tests |
| Integration | `stapel check STP-1` to `STP-5` on this repository (plan step 6, by hand; Decision 4); its output goes into Proof | — |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[check] cargo` | string | an absolute path to an executable regular file outside the repository | absent: `cargo` on `PATH` | relative, missing, a folder, not executable, under the repository: configuration error naming the key | byte-exact path, no `~` expansion; links resolved for the repository test | `core::config::check_cargo_key_is_validated` |
| Caller environment | variables | any | no `cargo` found: exit 2 | relative `PATH` entries: skipped | names compared byte-exactly | `check::cargo_runs_with_the_allow_list_only`, `check::refuses_without_a_working_cargo` |
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

**Author self-check (CLAUDE.md item 7).** To be done before code review.

**Review Focus.** Unchecked: an honest GREEN that adds a `[[test]]` (build-input outcome for every step;
accepted by Decision 3); a link added by a RED (RED may add test files; a link in a RED is `red-changes-code`
only if STP-4's rule says so — to confirm in Design); `RUSTUP_TOOLCHAIN` choosing another toolchain (not
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

## Plan

Route: full (source: `auto`; tags `security`, `guard`; about 400–500 lines).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | Environment allow list; `[check] cargo`; the cargo path and version; refusal without a working cargo | AC-8, AC-9, AC-10, AC-11 | outcomes of STP-4 tests |
| 2 | Worktree outside the repository; configuration above it | AC-7 | the lock and target folder under `.git/stapel/` |
| 3 | Build inputs in manifests, configuration, toolchain, build scripts, links; the `build inputs` line | AC-1, AC-2, AC-6 | STP-4 report lines and golden files |
| 4 | Helpers and included files | AC-3, AC-4, AC-5 | `tests-changed` for helpers the RED changed |
| 5 | `docs/PHASES.md`: STP-13 in the list of first tickets | — | other phases |
| 6 | By hand: `stapel check STP-1` to `STP-5` on this repository; escaped defects recorded | — | closed tickets' text |

## Review

TODO

## Summary

TODO
