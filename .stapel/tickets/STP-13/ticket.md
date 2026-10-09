# STP-13 — Hardening of stapel check, part two

## Description

Source: `docs/PHASES.md`, phase 0, STP-13; the "Not promised" list of STP-6 (Decisions 13, 21, 30, 33, 34).

### Frame

**Problem.** STP-6 closed the build inputs of its promise and moved a second list here. Its manifest and
test-tree part is still open: a commit can redirect a dependency to code it controls (`path`, `git`,
`package`, `[patch]`, `[replace]`), change `[profile]` or `[features]`, or add a data file under
`crates/*/tests/` that a test reads at run time; a RED can add a `Cargo.toml` under `crates/*/tests/` or
change a build script or target file that lies there. None of these makes a step fail today.

**Risk tags:** `security`, `guard`.

**Non-goals.** STP-6's non-goals; the include and run part of the list (split to STP-15, Decision 1): an
`include*!` with a non-literal argument or a path outside the repository, includes followed through another
included file, a later RED that rewrites a helper or included file an earlier step relies on, a config planted
inside the check's worktree between runs; the code of a published crate; `Cargo.lock` alone.

**Promise (closed list).** Against an agent that tries on purpose to make a step look verified, `stapel
check` gives STP-6's build-input outcome when a non-RED commit of the Range:

1. adds a dependency entry, or changes one so that its source changes, in any dependency table of any
   `Cargo.toml`, unless the new source is crates.io or a path to a workspace member; or adds, changes or
   deletes a `[patch]` or `[replace]` table;
2. changes `[profile]` or `[features]` of any `Cargo.toml`;
3. adds, changes or deletes a file under `crates/*/tests/` that is not a `.rs` file.

It gives `red-changes-code` when a RED of the ticket:

4. redirects a dev-dependency as in item 1;
5. adds, changes or deletes a `Cargo.toml` under `crates/*/tests/`;
6. changes a file that a `build` key or a target's `path` key of a manifest names.

Not promised: `[lints]` (a lint cannot make a test pass); a dependency's `features`; what a crates.io version
or a workspace member contains (code); proc-macro crates that read files; STP-6's "Not promised" list.

**Size.** Medium: about seven criteria, 600–900 changed lines, most of them tests. Beyond ten criteria the
ticket is split again.

### Decisions

Typed lines (`docs/decisions/0003`): Decision, Assumption, Open question. The human is away from 2026-10-09
for some days and asked the orchestrator to go as far as it can; every Assumption below is the orchestrator's
recommendation, waiting for the human's answer, and the build does not start before it.

- Inherited from STP-6: Decision 10 (dependency redirection gives `unverified` after RED and
  `red-changes-code` in a RED; crates.io dependencies and paths to workspace members stay allowed), Decision 4
  (the rules apply to every ticket; a closed ticket's step that turns `unverified` is an escaped defect).
- Open question 1 (functional-scope): split STP-13? (a) **recommended**: STP-13 is the manifest and test-tree
  part (this frame); STP-15, after STP-14, takes includes (non-literal, outside, chains), the later RED that
  rewrites a helper, and the config planted in the worktree; (b) one ticket for all nine items (about fifteen
  criteria; STP-6 outgrew a smaller frame seven times); (c) three tickets (manifest, test tree, includes and
  runs). Assumption: (a).
- Open question 2 (domain-data-model): what is a dependency's source? (a) **recommended**: the keys `path`,
  `git`, `branch`, `tag`, `rev`, `package`, `registry` and `registry-index` of the entry; an entry with none of
  them, or with `workspace = true`, is crates.io or inherited; a `path` that resolves to the folder of a
  workspace member at that commit is allowed; (b) every change of a dependency entry other than `version`,
  `features`, `optional` and `default-features`; (c) every change of any dependency table (a GREEN cannot add
  a dependency without `unverified`). Assumption: (a).
- Open question 3 (edge-failure): which other manifest tables count? (a) **recommended**: `[profile]` (it turns
  off `debug-assertions` and `overflow-checks`, which tests rely on) and `[features]` (it turns off code and
  test parts by `cfg`); `[lints]` not promised; (b) all three as STP-6 listed them; (c) `[profile]` only.
  Assumption: (a).
- Open question 4 (edge-failure): non-`.rs` files under `crates/*/tests/` after RED? (a) **recommended**: any
  add, change or delete by a non-RED commit gives the build-input outcome, golden files included (the builder
  rule "a GREEN changes nothing under `tests/`" becomes a check); (b) only an added file (the hole STP-6 code
  review named; STP-4's `tests-changed` already covers files the step's RED saw). Assumption: (a).
- Open question 5 (interaction-ux): the outcome text? (a) **recommended**: STP-6's `build-input-changed:
  <path> at <short sha>` with the manifest's path, no new format and no new golden file; (b) name the table or
  entry too (`<path> [dependencies.foo]`). Assumption: (a).

## Spec

Terms of STP-6 (Range, non-RED commit, build-input outcome) hold. New terms:

- **Dependency table**: `dependencies`, `dev-dependencies` or `build-dependencies` of a manifest, at the top
  level, under `[target.<cfg>]` or (only `dependencies`) under `[workspace]`; an entry is a key of such a
  table, in inline, dotted or `[dependencies.<name>]` form.
- **Source** of an entry: its values of `path`, `git`, `branch`, `tag`, `rev`, `package`, `registry`,
  `registry-index`; a string entry has none. A **redirect** is an entry at a commit whose source differs from
  the same entry at the commit's parent (an absent entry has none) and is neither empty, `workspace = true`
  alone, nor a lone `path` that resolves to the folder of a manifest listed as a workspace member at the
  commit.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF a non-RED commit makes a redirect in any dependency table of any `Cargo.toml` THEN the check gives the build-input outcome naming the manifest. | `check::dependency_redirect_is_unverified` |
| AC-2 | WHEN a non-RED commit adds or changes a dependency entry whose source is empty, `workspace = true` or a path to a workspace member THE steps keep their outcomes. | `check::crates_io_and_member_dependencies_are_allowed` |
| AC-3 | IF a non-RED commit adds, changes or deletes a `[patch]` or `[replace]` table, or changes `[profile]` or `[features]`, of any `Cargo.toml` THEN the check gives the build-input outcome naming the manifest. | `check::patch_profile_and_features_are_unverified` |
| AC-4 | IF a non-RED commit adds, changes, deletes or renames a file under `crates/*/tests/` that is not a `.rs` file THEN the check gives the build-input outcome naming the file. | `check::test_data_change_is_unverified` |
| AC-5 | IF a RED makes a redirect in a `dev-dependencies` table THEN the step is `red-changes-code` naming the manifest. | `check::red_redirecting_a_dev_dependency_changes_code` |
| AC-6 | IF a RED adds, changes or deletes a `Cargo.toml` under `crates/*/tests/` THEN the step is `red-changes-code` naming it. | `check::red_adding_a_test_manifest_changes_code` |
| AC-7 | IF a RED changes a file that, at the RED or its parent, a manifest's `[package] build` key or the `path` key of `[lib]`, `[[bin]]`, `[[example]]` or `[[bench]]` names THEN the step is `red-changes-code` naming the file. | `check::red_changing_a_build_or_target_file_changes_code` |

### Guarantees

- **Promised, against an agent that tries on purpose to make a step look verified** (tag `security`): the
  Frame's promise, by AC-1 to AC-7.
- **Not promised:** the Frame's list; a manifest that does not parse (STP-6's fail-closed rule keeps it a
  build input: AC-1 of STP-6).

## Design

### Facts (external contract and states)

| Claim | Source | Mark |
|---|---|---|
| Dependency tables are `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, also under `[target.'<cfg>'.…]`; `[workspace.dependencies]` holds entries that members inherit by `workspace = true` | Cargo book, "Specifying Dependencies", "Workspaces" | documented |
| A source is crates.io unless the entry has `path`, `git` (with `branch`, `tag` or `rev`) or `registry`/`registry-index`; `package` renames the crate | same | documented |
| `[patch.<source>]` and `[replace]` take effect only in the workspace root manifest; cargo warns and ignores them elsewhere | same, "Overriding Dependencies" | documented |
| `[profile.*]` takes effect only in the workspace root manifest | Cargo book, "Profiles" | documented |
| STP-6 `targets()` compares `[lib]`, `[[bin]]`, `[[test]]`, `[[example]]`, `[[bench]]`, three `[workspace]` keys and six `[package]` keys; a manifest that does not parse counts as changed | `crates/stapel-core/src/build_inputs.rs` | verified |
| STP-4 `is_code`: a RED's `.rs` or other file under `crates/*/tests/` is not code, a `Cargo.toml` there included; a manifest change only in `dev-dependencies` is not code | `crates/stapel-core/src/outcomes.rs` | verified |
| STP-4 `tests-changed` protects every file under the tests folder of a crate with a step test as it was at the RED, not files added later | same | verified |

### Decisions (as built)

Written as notes during the build and moved here before code review (`CLAUDE.md`, item 6).

## Test plan

CLI tests in `crates/stapel-cli/tests/check.rs`, as in STP-6: a tiny cargo crate in a temporary git
repository with RED and GREEN steps. Heavy runs: `cargo test -j 4 … -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | crates.io and member dependencies added by a GREEN | `check::crates_io_and_member_dependencies_are_allowed` |
| Negative | each redirect key in each table form; `[patch]`, `[replace]`, `[profile]`, `[features]`; a data file added, changed, deleted, renamed; RED-side items | `check::dependency_redirect_is_unverified`, `check::patch_profile_and_features_are_unverified`, `check::test_data_change_is_unverified`, `check::red_redirecting_a_dev_dependency_changes_code`, `check::red_adding_a_test_manifest_changes_code`, `check::red_changing_a_build_or_target_file_changes_code` |
| Abuse | see the Abuse table | the Negative tests |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| Dependency entry | TOML value | a string version to an inline or dotted table with every source key | absent at parent: compared with an empty source | not a string or table: counts as a redirect | source keys compared as parsed TOML values | `check::dependency_redirect_is_unverified` |
| Member path | string | relative to the manifest's folder, `..` allowed | — | leaves the repository or names no member at the commit: a redirect | components normalised as STP-6 `build_file`; no link resolution (links are STP-6 AC-2) | `check::crates_io_and_member_dependencies_are_allowed` |
| `[patch]`, `[replace]`, `[profile]`, `[features]` | TOML tables | absent to many keys | absent on both sides: no change | manifest does not parse: build input (STP-6) | compared as parsed values | `check::patch_profile_and_features_are_unverified` |
| Path under `crates/*/tests/` | git path | any depth | — | non-UTF-8: read lossily, as STP-6 | `.rs` by suffix, byte-exact | `check::test_data_change_is_unverified` |
| `build` and target `path` keys | string | relative to the manifest | absent: no named file (`build.rs` stays STP-6 AC-2) | not a string: ignored | as STP-6 `build_file` | `check::red_changing_a_build_or_target_file_changes_code` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| cargo | workspace with several packages; package name differs from folder name | members read from the root manifest at each commit; `package` is a source key | `check::crates_io_and_member_dependencies_are_allowed` |
| cargo | custom targets | STP-6 AC-1; target `path` in a RED: AC-7 | `check::red_changing_a_build_or_target_file_changes_code` |
| cargo | `.cargo/config.toml` `[patch]` | STP-6 AC-2 | — |
| cargo | environment | STP-6 AC-8 | — |
| cargo | `Cargo.lock` out of date; offline; network errors | unchanged (`--locked`); a lock-only source change: not promised | — |
| cargo | build scripts and `include!` | STP-6; a `build` key under `tests/`: AC-7 | `check::red_changing_a_build_or_target_file_changes_code` |
| cargo | libtest output | unchanged | — |
| git | renames and similarity | a rename names the old path, as STP-6 | `check::test_data_change_is_unverified` |
| git | paths with spaces or glob characters, non-UTF-8 | literal paths, as STP-6 | `check::test_data_change_is_unverified` |
| git | history shape, shallow clone, object format, detached HEAD, worktrees, submodules, dirty tree, configuration, message bytes | unchanged from STP-6 | — |
| os | symlinks under `tests/` | a link is STP-6 AC-2 | — |
| os | case-insensitive file systems; permissions; processes; time; WSL | unchanged from STP-6 | — |

**Review Focus:** a dependency entry that is not a table nor a string; a `[target]` table whose cfg key is
quoted; a workspace whose `members` is a glob; `[patch]` in a member manifest (ignored by cargo but still
counted, by AC-3). Each has a case in the named tests.

### Abuse table

| Attempt | Outcome |
|---|---|
| A GREEN that points `foo = { path = "tests/fake" }` at a crate it wrote | build-input outcome, AC-1 (and STP-6 AC-1: an added `Cargo.toml`) |
| A GREEN that sets `git` or `package` on an existing dependency | AC-1 |
| A GREEN that adds `[patch.crates-io]` or `[replace]` | AC-3 |
| A GREEN that sets `[profile.test] debug-assertions = false` | AC-3 |
| A GREEN that drops a feature a test's `cfg` needs | AC-3 |
| A GREEN that adds `tests/data/expected.json` a test reads | AC-4 |
| A GREEN that rewrites a golden file | AC-4 |
| A RED that adds a dev-dependency with `path` to a crate under `tests/`, filled by a later GREEN | AC-5, and the GREEN's `.rs` files there are STP-6 helpers |
| A RED that adds `crates/x/tests/fake/Cargo.toml` | AC-6 |
| A RED that changes a `build = "tests/gen.rs"` script | AC-7 |

**Author self-check:** To be done before the spec review.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check::dependency_redirect_is_unverified` |
| AC-2 | `check::crates_io_and_member_dependencies_are_allowed` |
| AC-3 | `check::patch_profile_and_features_are_unverified` |
| AC-4 | `check::test_data_change_is_unverified` |
| AC-5 | `check::red_redirecting_a_dev_dependency_changes_code` |
| AC-6 | `check::red_adding_a_test_manifest_changes_code` |
| AC-7 | `check::red_changing_a_build_or_target_file_changes_code` |

## Plan

Route: full (source: `auto`; tags `security`, `guard`).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | Dependency sources, `[patch]`, `[replace]`, `[profile]`, `[features]` in `build_inputs.rs` | AC-1, AC-2, AC-3 | STP-6 outcomes and golden files |
| 2 | Non-`.rs` files under `crates/*/tests/` | AC-4 | `.rs` helper and step-file rules of STP-6 |
| 3 | RED rules in `outcomes::is_code` | AC-5, AC-6, AC-7 | STP-4 RED rule for other paths |
| 4 | `docs/PHASES.md`: STP-15 | — | other tickets |
| 5 | By hand: `stapel check` on STP-2 to STP-6; new hits recorded as escaped defects | — | closed tickets' text |

## Review

TODO

## Summary

TODO
