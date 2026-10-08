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
`stapel check` gives `unverified` or `fail`, never `pass`, when a step's GREEN or a later commit up to HEAD:

1. changes a `[[test]]`, `[[bin]]`, `[lib]` or `[workspace]` target table, or `harness`, in any manifest;
2. adds or changes a cargo configuration file (`.cargo/config`, `.cargo/config.toml`, at any level of the
   repository), a `rust-toolchain` file or a `build.rs`;
3. adds or changes a non-test file under `crates/*/tests/`, or a file that a test or source file includes
   at compile time by a literal path.

And every cargo run of the check starts from a fixed environment: the variables that pick a tool or change
the build (`RUSTC*`, `CARGO_*` apart from those the check sets, `RUSTFLAGS`, `RUSTDOCFLAGS`) are removed,
and `cargo` is resolved once, before the first run, the path shown in the report.

Not promised: a cargo configuration outside the repository (`~/.cargo/config.toml`, a parent directory);
a compromised toolchain; include paths built by macros other than the literal-path `include*!` family;
proc-macro crates that read files.

**Size.** Medium: three to five criteria for the promise, one for the environment; about 300–500 changed
lines. If the include check (point 3, second half) grows past that, it is split into its own ticket.

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

## Spec

Criteria are short (`CLAUDE.md`, the spec process, item 3); exact output is in golden files. "After RED"
means a commit of the first-parent history after the step's RED up to HEAD that is not a RED of the
ticket. A step marked by these rules reads `unverified: build-input-changed: <path> at <short sha>` in the
report (golden file `check_report_build_inputs.txt`); the first such path and commit are named.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF after RED a commit changes, in any `Cargo.toml`, a `[lib]`, `[[bin]]`, `[[test]]`, `[[example]]`, `[[bench]]` or `[workspace]` table, or the `build`, `autotests`, `autobins`, `autoexamples` or `autobenches` key of `[package]` THEN the step is `unverified` with `build-input-changed`. | `check::target_table_change_after_red_is_unverified` |
| AC-2 | IF after RED a commit adds, changes, deletes or renames a `.cargo/config` or `.cargo/config.toml`, a `rust-toolchain` or `rust-toolchain.toml`, at any depth of the repository, or a `build.rs` THEN the step is `unverified` with `build-input-changed`. | `check::cargo_config_toolchain_and_build_script_after_red_are_unverified` |
| AC-3 | WHEN files of AC-2 exist at HEAD and no commit after a step's RED changes them THE report prints `build inputs: <path>, …` before the step lines, and the steps keep their outcomes. | `check::unchanged_build_inputs_are_named` |
| AC-4 | IF after RED a commit changes a helper that a step test's file reaches through `mod` declarations or `#[path]` attributes, directly or through other helpers THEN the step is `unverified` with `build-input-changed`. A helper is a file under `crates/*/tests/` other than a top-level `.rs` file and a file under `golden/`. | `check::helper_change_after_red_is_unverified` |
| AC-5 | IF a RED changes a file that a `.rs` file outside `crates/*/tests/` names in `include!`, `include_str!` or `include_bytes!` with a string literal THEN the step is `red-changes-code` and the path is named. | `check::red_changing_an_included_file_changes_code` |
| AC-6 | IF after RED a commit changes a file that a step test's file or one of its helpers names in `include!`, `include_str!` or `include_bytes!` with a string literal THEN the step is `unverified` with `build-input-changed`. | `check::included_file_change_after_red_is_unverified` |
| AC-7 | WHEN the check starts a cargo run THE run gets an empty environment plus `HOME`, `USER`, `PATH`, `LANG`, `TMPDIR`, `CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN` when set in the caller, and the variables the check sets; `RUSTFLAGS`, `RUSTC_WRAPPER` or any other variable of the caller does not reach it. | `check::cargo_runs_with_the_allow_list_only` |
| AC-8 | WHEN the check starts THE cargo program is `[check] cargo` if set, else the first `cargo` on `PATH`, resolved once to an absolute path; every run uses that path; the report prints `cargo: <path> (<first line of cargo --version>)` and the run record carries `cargo` with the same text. | `check::cargo_path_and_version_are_reported` |
| AC-9 | IF `[check] cargo` is not an absolute path to an executable regular file THEN loading the configuration fails, naming the key; IF the key is absent and no `cargo` is on `PATH`, or `cargo --version` fails THEN `stapel check` exits 2 and appends no record. | `core::config::check_cargo_key_is_validated`, `check::refuses_without_a_cargo` |

### Guarantees

- **Promised, against an agent that tries on purpose to make a step look verified** (tag `security`): the
  three cases of the Frame's promise give `unverified` or a failing outcome, never `pass`; every cargo run
  has the environment of AC-7 and the program of AC-8.
- **Not promised:** the Frame's list; a `rustc`, linker or other tool that cargo finds through `PATH` or
  `RUSTUP_TOOLCHAIN`; include paths built by `concat!`, `env!` or other macros; a submodule's files; a
  `.cargo` directory that is a symlink to a place outside the repository changed without a commit; the
  outcome of a closed ticket's history before this ticket (Decision 4: a step that turns `unverified` is
  an escaped defect, recorded, not hidden).

## Design

TODO

## Test plan

CLI tests build a tiny `cargo` crate in a temporary git repository, as in STP-4, and commit RED and GREEN
steps into it; each sets its own `CARGO_TARGET_DIR`. Heavy runs: `cargo test -j 4 … -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | unchanged build inputs are named; the cargo path and version in report and record | `check::unchanged_build_inputs_are_named`, `check::cargo_path_and_version_are_reported` |
| Negative | each changed build input after RED; a RED that changes an included file; bad `[check] cargo`; no cargo | `check::target_table_change_after_red_is_unverified`, `check::cargo_config_toolchain_and_build_script_after_red_are_unverified`, `check::helper_change_after_red_is_unverified`, `check::included_file_change_after_red_is_unverified`, `check::red_changing_an_included_file_changes_code`, `core::config::check_cargo_key_is_validated`, `check::refuses_without_a_cargo` |
| Abuse | see the Abuse table | `check::cargo_runs_with_the_allow_list_only` and the Negative tests |
| Integration | `stapel check STP-1` to `STP-5` on this repository (plan step 5, by hand; Decision 4) | — |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[check] cargo` | string | an absolute path to an executable regular file | absent: `cargo` on `PATH` | relative, missing, a directory, not executable: configuration error naming the key | byte-exact path, no `~` expansion | `core::config::check_cargo_key_is_validated` |
| Caller environment | variables | any | empty `PATH` with no key: exit 2 | — | names compared byte-exactly | `check::cargo_runs_with_the_allow_list_only`, `check::refuses_without_a_cargo` |
| `Cargo.toml` changes | TOML diff | every manifest of the tree | — | unparsable at either side: `unverified` naming the path | tables compared as parsed values, not text | `check::target_table_change_after_red_is_unverified` |
| `include*!` literals | Rust tokens | every `.rs` file at HEAD | none: no included files | non-UTF-8 file: read lossily; a path that leaves the repository: ignored | literal paths, relative to the including file | `check::included_file_change_after_red_is_unverified`, `check::red_changing_an_included_file_changes_code` |
| `mod` and `#[path]` in tests | Rust tokens | nested helpers, any depth; cycles stop | no helper: nothing | a `mod` without a file: ignored | `mod x;` gives `x.rs` or `x/mod.rs` | `check::helper_change_after_red_is_unverified` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| cargo | workspace with several packages | manifests read at every depth | `check::target_table_change_after_red_is_unverified` |
| cargo | custom targets (`[[test]]`, `harness = false`, `autotests = false`) | changed after RED: AC-1 | `check::target_table_change_after_red_is_unverified` |
| cargo | `.cargo/config.toml` at any level, wrappers, runners | in the repository: AC-2 and AC-3; outside it: not promised | `check::cargo_config_toolchain_and_build_script_after_red_are_unverified` |
| cargo | environment `CARGO_*`, `RUSTC*`, `RUSTFLAGS`, `PATH` | AC-7 and AC-8; tools found through `PATH`: not promised | `check::cargo_runs_with_the_allow_list_only` |
| cargo | `Cargo.lock` out of date; offline; network errors | unchanged from STP-4 (`--locked`) | — |
| cargo | build scripts and `include!` of files outside `src` | AC-2, AC-5, AC-6 | `check::included_file_change_after_red_is_unverified` |
| cargo | libtest output format | unchanged from STP-4 | — |
| git | renames and similarity | a rename of a build input counts as a change of both paths | `check::cargo_config_toolchain_and_build_script_after_red_are_unverified` |
| git | paths with spaces or glob characters | literal paths, as STP-4 | `check::included_file_change_after_red_is_unverified` |
| git | submodules | not promised | — |
| git | history shape, shallow clone, object format, detached HEAD, dirty tree, configuration | unchanged from STP-4 | — |
| os | symlinks | a symlink is a file; its target's content is not followed: not promised | — |
| os | case-insensitive file systems | not promised (names compared byte-exactly) | — |

### Abuse table

| Attempt | Outcome |
|---|---|
| A GREEN that points `[[test]] path` at a file that always passes, or sets `harness = false` | `unverified`, AC-1 |
| A GREEN that adds `.cargo/config.toml` with a `runner` or `rustc-wrapper` | `unverified`, AC-2 |
| A GREEN that adds `build.rs` or `rust-toolchain.toml` | `unverified`, AC-2 |
| A GREEN that moves the code into `tests/common/mod.rs` used by the step test | `unverified`, AC-4 |
| A GREEN that edits a file a test includes with `include_str!` | `unverified`, AC-6 |
| A RED that adds the code as a file the library includes with `include!` | `red-changes-code`, AC-5 |
| A caller that sets `RUSTC_WRAPPER`, `RUSTFLAGS` or `CARGO_BUILD_RUSTC` | removed, AC-7 |
| A `PATH` with a fake `cargo` first | it runs, but the report and the record name its path and version (AC-8); not prevented |
| A `[check] cargo` that points at a script | allowed: `stapel.toml` is a machine file the guard protects; the report names the path |

**Author self-check (CLAUDE.md item 7).** To be done before code review.

**Review Focus.** Unchecked: a GREEN that adds a new `[[test]]` honestly (also `unverified`; accepted by
Decision 3); a manifest whose `[package] build` names a script that is not `build.rs` (covered by AC-1
through the `build` key); a helper reached only through `include!` (AC-6); `RUSTUP_TOOLCHAIN` in the allow
list choosing another toolchain (not promised).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check::target_table_change_after_red_is_unverified` |
| AC-2 | `check::cargo_config_toolchain_and_build_script_after_red_are_unverified` |
| AC-3 | `check::unchanged_build_inputs_are_named` |
| AC-4 | `check::helper_change_after_red_is_unverified` |
| AC-5 | `check::red_changing_an_included_file_changes_code` |
| AC-6 | `check::included_file_change_after_red_is_unverified` |
| AC-7 | `check::cargo_runs_with_the_allow_list_only` |
| AC-8 | `check::cargo_path_and_version_are_reported` |
| AC-9 | `core::config::check_cargo_key_is_validated`, `check::refuses_without_a_cargo` |

## Plan

Route: full (source: `auto`; tags `security`, `guard`; about 300–500 lines).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | Environment allow list; `[check] cargo`; the cargo path and version in report and record | AC-7, AC-8, AC-9 | outcomes of STP-4 tests |
| 2 | Manifest target tables and `[package]` keys after RED | AC-1 | the RED rule of STP-4 |
| 3 | Cargo configuration, toolchain and build script files after RED; the `build inputs` line | AC-2, AC-3 | report lines of STP-4 golden files |
| 4 | Helpers through `mod` and `#[path]`; `include*!` literals in RED and after RED | AC-4, AC-5, AC-6 | `tests-changed` for helpers the RED changed |
| 5 | By hand: `stapel check STP-1` to `STP-5` on this repository; record escaped defects | — | closed tickets' text |

## Review

TODO

## Summary

TODO
