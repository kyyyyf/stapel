# STP-13 — Hardening of stapel check, part two

## Description

Source: `docs/PHASES.md`, phase 0, STP-13; the "Not promised" list of STP-6 (Decisions 13, 21, 30, 33, 34).

### Frame

**Problem.** STP-6 closed the build inputs of its promise and moved a second list here. Its manifest and
test-tree part is still open: a commit can redirect a dependency to code it controls (`path`, `git`,
`package`, `[patch]`, `[replace]`), change `[profile]`, `[features]`, `[lints]` or `edition`, add a
dev-dependency that resolves a RED that did not compile, or add a data file under `crates/*/tests/` that a test
reads at run time; a RED can add a `Cargo.toml` or a symbolic link under `crates/*/tests/` or change a build
script or target file that lies there. None of these makes a step fail today.

**Risk tags:** `security`, `guard`.

**Non-goals.** STP-6's non-goals; the include and run part of the list (split to STP-15, Decision 1): an
`include*!` with a non-literal argument or a path outside the repository, includes followed through another
included file, a later RED that rewrites a helper or included file an earlier step relies on, a config planted
inside the check's worktree between runs, a run-time read of a file outside `crates/*/tests/`; the code of a
published crate; `Cargo.lock` alone.

**Promise (closed list).** Against an agent that tries on purpose to make a step look verified, `stapel
check` gives STP-6's build-input outcome when a non-RED commit of the Range:

1. adds a dependency entry, or changes one so that its source changes, in any dependency table of any
   `Cargo.toml`, unless the new source is empty (crates.io or `workspace = true`) or, outside
   `dev-dependencies`, a path to a workspace member; or moves an entry from a `path` or `git` source to an
   empty one; or adds, changes or deletes a `[patch]` or `[replace]` table;
2. adds, changes or deletes `[profile]`, `[features]`, `[lints]`, `[workspace.lints]` or the `edition` of
   `[package]`, `[project]` or `[workspace.package]` of any `Cargo.toml`;
3. adds an entry to a `dev-dependencies` table;
4. adds a file under `crates/*/tests/` that is not a `.rs` file, or changes, deletes or renames one in a
   crate's tests folder that holds no step test (STP-4's `tests-changed` covers the others).

It gives `red-changes-code` when a RED of the ticket:

5. redirects a dev-dependency as in item 1;
6. adds, changes or deletes a `Cargo.toml`, a symbolic link, a `.cargo` path or a `rust-toolchain` file under
   `crates/*/tests/`;
7. adds, changes or deletes a file that a `build` key or the `path` key of `[lib]`, `[[bin]]`, `[[example]]`
   or `[[bench]]` names.

Not promised: a dependency's `features`; what a crates.io version or a workspace member contains (code); proc-macro
crates that read files; a RED that does not compile and that a crates.io entry added by a GREEN to
`[dependencies]` resolves (see Open question 6); STP-6's "Not promised" list.

**Size.** Medium: about eight criteria, 600–900 changed lines, most of them tests. Beyond ten criteria the
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
- Open question 3 (edge-failure): which other manifest tables count? (a) `[profile]` and `[features]` only;
  (b) **recommended** after spec review SR-2: `[profile]`, `[features]`, `[lints]`, `[workspace.lints]` and
  `edition` of `[package]` and `[workspace.package]` (a lint set to `allow` or an older edition turns a RED that
  did not compile into a pass); (c) `[profile]` only. Assumption: (b).
- Open question 4 (edge-failure): non-`.rs` files under `crates/*/tests/` after RED? (a) any add, change or
  delete by a non-RED commit gives the build-input outcome, golden files included; it collides with STP-4's
  per-step `tests-changed` (spec review SR2-1) and turns every step `unverified` for one file; (b)
  **recommended** after round 2: an added file anywhere, and a change, delete or rename in a crate's tests
  folder without a step test; STP-4's `tests-changed` keeps the rest. Assumption: (b).
- Open question 5 (interaction-ux): the outcome text? (a) **recommended**: STP-6's `build-input-changed:
  <path> at <short sha>` with the manifest's path, no new format and no new golden file; (b) name the table or
  entry too (`<path> [dependencies.foo]`). Assumption: (a).
- 2026-10-09, spec review round 1 (SR-1 to SR-7 and LOW items, `findings.jsonl`), fixed in the text under the
  Assumptions: SR-3 (`dev_dependencies`, `build_dependencies`, `[project]` aliases), SR-4 (a RED's symbolic
  link under `crates/*/tests/`, AC-6), SR-6 (run-time reads outside `crates/*/tests/`: non-goal, STP-15),
  SR-7 (a `path` or `git` entry moved to crates.io is a redirect), and the LOW items. SR-2 changed the
  recommendation of Open question 3. SR-1 is HIGH, so a round 2 runs.
- Open question 6 (edge-failure, SR-1): a RED whose test does not compile counts as RED (STP-4); a GREEN that
  only adds the missing crates.io dependency makes it pass with no behaviour under test. (a) **recommended**: a
  dev-dependency entry added by a non-RED commit gives the build-input outcome (dev-dependencies belong to the
  RED, which may already add them); the same through `[dependencies]` is not promised; (b) both tables; (c)
  not promised at all. Assumption: (a).
- Open question 7 (constraints-tradeoffs, SR-5): AC-4 and AC-6 together mean no commit may add a `Cargo.toml`
  under `crates/*/tests/`. (a) **recommended**: record it as a rule: fixture projects are built at run time
  (as `check.rs` does today); (b) narrow AC-6 to a manifest that a dependency table, `[patch]` or `[replace]`
  names. Assumption: (a).
- 2026-10-09, spec review round 2 (SR2-1 to SR2-11, no HIGH; no round 3), fixed in the text under the
  Assumptions: SR2-1 (AC-5 narrowed, Open question 4 changed), SR2-2 (`package` is a source key), SR2-3 (a
  dev-dependency moved to a member path is a redirect), SR2-5 to SR2-10 (wording, tables, AC-7 widened to
  `.cargo` and toolchain files). SR2-4 (any dependency added after a RED that did not compile) is an option of
  Open question 6: (d) a dependency entry added between such a RED and its GREEN gives the build-input
  outcome; it needs the RED's run result before the build-input rules, so it is not recommended here.

## Spec

Terms of STP-6 (Range, non-RED commit, build-input outcome) hold. New terms:

- **Dependency table**: `dependencies`, `dev-dependencies` (alias `dev_dependencies`) or `build-dependencies`
  (alias `build_dependencies`) of a manifest, at the top level, under `[target.<cfg>]` (quoted or bare key) or,
  for `dependencies`, under `[workspace]`; an entry is a key of such a table, in inline, dotted or
  `[dependencies.<name>]` form, compared by table and key.
- **Source** of an entry: its values of `path`, `git`, `branch`, `tag`, `rev`, `package`, `registry`,
  `registry-index`; a string entry, a table without these keys and `workspace = true` have an empty source.
- **Member folder**: the folder of the root package or of a `members` entry of the root manifest, globs
  expanded over the commit's tree and `exclude` removed.
- **Redirect**: an entry present at a commit whose source differs from the same entry at the parent (an
  absent entry has an empty source), unless the new source is empty, or the entry is not in a
  `dev-dependencies` table and its new source is a `path` (with or without `package`) that names a member
  folder at the commit; an entry whose source at the parent has `path` or `git` and is empty at the commit is
  a redirect; an entry that is not a string or a table is a redirect. A deleted entry is no redirect.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF a non-RED commit makes a redirect in any dependency table of any `Cargo.toml` THEN the check gives the build-input outcome naming the manifest. | `check::dependency_redirect_is_unverified` |
| AC-2 | WHEN a non-RED commit adds or changes an entry of `dependencies` or `build-dependencies` whose new source is empty or names a member folder, or changes a `dev-dependencies` entry without changing its source, THE steps keep their outcomes. | `check::crates_io_and_member_dependencies_are_allowed` |
| AC-3 | IF a non-RED commit adds, changes or deletes `[patch]`, `[replace]`, `[profile]`, `[features]`, `[lints]`, `[workspace.lints]`, or `edition` of `[package]`, `[project]` or `[workspace.package]`, of any `Cargo.toml` THEN the check gives the build-input outcome naming the manifest. | `check::patch_profile_features_lints_and_edition_are_unverified` |
| AC-4 | IF a non-RED commit adds an entry to a `dev-dependencies` table THEN the check gives the build-input outcome naming the manifest. | `check::green_adding_a_dev_dependency_is_unverified` |
| AC-5 | IF a non-RED commit adds a file under `crates/*/tests/` that is not a `.rs` file (a submodule entry included), or changes, deletes or renames such a file in a crate's tests folder that holds no step test of the ticket, THEN the check gives the build-input outcome naming the file. | `check::test_data_change_is_unverified` |
| AC-6 | IF a RED makes a redirect in a `dev-dependencies` table THEN the step is `red-changes-code` naming the manifest. | `check::red_redirecting_a_dev_dependency_changes_code` |
| AC-7 | IF a RED adds, changes or deletes a `Cargo.toml`, a symbolic link, a path with a `.cargo` component, `rust-toolchain` or `rust-toolchain.toml` under `crates/*/tests/` THEN the step is `red-changes-code` naming it. | `check::red_adding_a_test_manifest_or_link_changes_code` |
| AC-8 | IF a RED adds, changes or deletes a file that, at the RED or its parent, a manifest's `build` key of `[package]` or `[project]` or the `path` key of `[lib]`, `[[bin]]`, `[[example]]` or `[[bench]]` names THEN the step is `red-changes-code` naming the file. | `check::red_changing_a_build_or_target_file_changes_code` |

### Guarantees

- **Promised, against an agent that tries on purpose to make a step look verified** (tag `security`): the
  Frame's promise, by AC-1 to AC-8.
- **Not promised:** the Frame's list; a manifest that does not parse (STP-6's fail-closed rule keeps it a
  build input: AC-1 of STP-6); a crate folder renamed by a non-RED commit makes every entry of its manifest
  new (fails closed, a false positive accepted); fixture projects committed under `crates/*/tests/` (they are
  built at run time, Open question 7).

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
| Cargo still reads the aliases `dev_dependencies`, `build_dependencies` and `[project]` (deprecated; refused in edition 2024) | Cargo book, "The Manifest Format"; `outcomes::manifest_without_dev` strips both aliases | documented, verified |
| `steps::changes` reads `--name-status`, without modes, so a RED's symbolic link under `crates/*/tests/` is not seen as a link today | `crates/stapel-core/src/steps.rs` | verified |
| A RED whose step test does not compile counts as a RED (`TestState::Compile`) | `crates/stapel-cli/src/check.rs` | verified |

### Decisions (as built)

Written as notes during the build and moved here before code review (`CLAUDE.md`, item 6).

## Test plan

CLI tests in `crates/stapel-cli/tests/check.rs`, as in STP-6: a tiny cargo crate in a temporary git
repository with RED and GREEN steps. Heavy runs: `cargo test -j 4 … -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | crates.io, `workspace = true` and member dependencies added by a GREEN (member by glob, root package) | `check::crates_io_and_member_dependencies_are_allowed` |
| Negative | each source key in each table form and alias; a `path` entry moved to crates.io; `[patch]`, `[replace]`, `[profile]`, `[features]`, `[lints]`, `edition`; a dev-dependency added by a GREEN; a data file added, changed, deleted, renamed; RED-side items | `check::dependency_redirect_is_unverified`, `check::patch_profile_features_lints_and_edition_are_unverified`, `check::green_adding_a_dev_dependency_is_unverified`, `check::test_data_change_is_unverified`, `check::red_redirecting_a_dev_dependency_changes_code`, `check::red_adding_a_test_manifest_or_link_changes_code`, `check::red_changing_a_build_or_target_file_changes_code` |
| Abuse | see the Abuse table | the Negative tests |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| Dependency entry | TOML value | a string version to an inline or dotted table with every source key | absent at parent: an empty source | not a string or table: a redirect | source keys compared as parsed TOML values | `check::dependency_redirect_is_unverified` |
| Member path | string | relative to the manifest's folder, `..` allowed | — | leaves the repository or names no member folder at the commit: a redirect | components normalised as STP-6 `build_file`; members globbed over `git ls-tree` of the commit; no link resolution (links are STP-6 AC-2) | `check::crates_io_and_member_dependencies_are_allowed` |
| `[patch]`, `[replace]`, `[profile]`, `[features]`, `[lints]`, `[workspace.lints]`, `edition` | TOML values | absent to many keys | absent on both sides: no change; absent on one side: a change | manifest does not parse: build input (STP-6) | compared as parsed values | `check::patch_profile_features_lints_and_edition_are_unverified` |
| Path under `crates/*/tests/` | git path and mode | any depth; mode 100644, 100755, 120000 (link: AC-7 in a RED), 160000 (submodule: a file for AC-5) | — | non-UTF-8: read lossily, as STP-6 | `.rs` by suffix, byte-exact | `check::test_data_change_is_unverified`, `check::red_adding_a_test_manifest_or_link_changes_code` |
| `build` and target `path` keys | string | relative to the manifest | absent: no named file (`build.rs` stays STP-6 AC-2) | not a string: ignored; absolute: taken as relative, cargo fails first | as STP-6 `build_file` | `check::red_changing_a_build_or_target_file_changes_code` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| cargo | workspace with several packages; package name differs from folder name | member folders from the root manifest at each commit; `package` with a member path allowed | `check::crates_io_and_member_dependencies_are_allowed` |
| cargo | custom targets | STP-6 AC-1; a target `path` changed by a RED: AC-8 | `check::red_changing_a_build_or_target_file_changes_code` |
| cargo | `.cargo/config.toml` at any level (`[patch]`, `[source]`, `paths`) | STP-6 AC-2, AC-7 | — |
| cargo | environment | STP-6 AC-8 | — |
| cargo | `Cargo.lock` out of date; offline; network errors | unchanged (`--locked`); a lock-only source change: not promised | — |
| cargo | build scripts and `include!` | STP-6; a `build` key under `tests/`: AC-8 | `check::red_changing_a_build_or_target_file_changes_code` |
| cargo | libtest output; a RED that does not compile | unchanged; a GREEN dev-dependency that resolves it: AC-4 | `check::green_adding_a_dev_dependency_is_unverified` |
| git | renames and similarity | a rename names the old path, as STP-6; a renamed crate folder makes its entries new (fails closed) | `check::test_data_change_is_unverified` |
| git | paths with spaces or glob characters, non-UTF-8 | literal paths, as STP-6 | `check::test_data_change_is_unverified` |
| git | submodules under `crates/*/tests/` | a file for AC-5 | `check::test_data_change_is_unverified` |
| git | history shape, shallow clone, object format, detached HEAD, worktrees, dirty tree, configuration, message bytes | unchanged from STP-6 | — |
| os | symlinks under `crates/*/tests/` | non-RED: STP-6 AC-2; RED: AC-7 | `check::red_adding_a_test_manifest_or_link_changes_code` |
| os | case-insensitive file systems; permissions; processes; time; WSL | unchanged from STP-6 | — |

**Review Focus:** a dependency entry that is not a table nor a string; a `[target]` table whose cfg key is
quoted; a workspace whose `members` is a glob with `exclude`; `[patch]` in a member manifest (ignored by
cargo but still counted, by AC-3); the underscore aliases. Each has a case in the named tests.

### Abuse table

| Attempt | Outcome |
|---|---|
| A GREEN that points `foo = { path = "tests/fake" }` at a crate it wrote | build-input outcome, AC-1 (and STP-6 AC-1: an added `Cargo.toml`) |
| A GREEN that sets `git` on an existing dependency, or under `[build_dependencies]`, or renames a crates.io dependency by `package` | AC-1 |
| A GREEN that points an existing dev-dependency at a workspace member by `path` | AC-1 |
| A GREEN that swaps a member `path` dependency for a crates.io crate of the same name | AC-1 |
| A GREEN that adds `[patch.crates-io]` or `[replace]` | AC-3 |
| A GREEN that sets `[profile.test] debug-assertions = false` | AC-3 |
| A GREEN that drops a feature a test's `cfg` needs | AC-3 |
| A GREEN that allows a deny-by-default lint, or sets an older `edition`, so a RED that did not compile passes | AC-3 |
| A GREEN that adds the crates.io dev-dependency a RED that did not compile needs | AC-4 |
| A GREEN that adds the crates.io `[dependencies]` entry a RED that did not compile needs | not promised (Open question 6) |
| A GREEN that adds `tests/data/expected.json` a test reads | AC-5 |
| A GREEN that rewrites a golden file the step's RED saw | STP-4 `tests-changed` |
| A RED that adds a dev-dependency with `path` outside the members, filled by a later GREEN | AC-6 |
| A RED that adds `crates/x/tests/fake/Cargo.toml`, a link `crates/x/tests/data -> /tmp/d`, or `crates/x/tests/.cargo/config.toml` | AC-7 |
| A RED that changes a `build = "tests/gen.rs"` script | AC-8 |

**Author self-check:** 2026-10-09, after spec review round 1: every promise item has a criterion and a test;
the Inputs and External states rows come from the catalogue; round 2 fixes applied; open questions 1 to 7
wait for the human.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check::dependency_redirect_is_unverified` |
| AC-2 | `check::crates_io_and_member_dependencies_are_allowed` |
| AC-3 | `check::patch_profile_features_lints_and_edition_are_unverified` |
| AC-4 | `check::green_adding_a_dev_dependency_is_unverified` |
| AC-5 | `check::test_data_change_is_unverified` |
| AC-6 | `check::red_redirecting_a_dev_dependency_changes_code` |
| AC-7 | `check::red_adding_a_test_manifest_or_link_changes_code` |
| AC-8 | `check::red_changing_a_build_or_target_file_changes_code` |

## Plan

Route: full (source: `auto`; tags `security`, `guard`).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | Dependency sources and redirects in `build_inputs.rs` | AC-1, AC-2 | STP-6 outcomes and golden files |
| 2 | `[patch]`, `[replace]`, `[profile]`, `[features]`, `[lints]`, `[workspace.lints]`, `edition`; dev-dependencies added by a non-RED commit | AC-3, AC-4 | the manifest keys STP-6 AC-1 compares |
| 3 | Non-`.rs` files under `crates/*/tests/` | AC-5 | `.rs` helper and step-file rules of STP-6; STP-4 `tests-changed` and its test `check::outcome_tests_changed_after_red` |
| 4 | RED rules in `outcomes::is_code`, with modes for links | AC-6, AC-7, AC-8 | STP-4 RED rule for other paths |
| 5 | `docs/PHASES.md`: STP-15 | — | other tickets |
| 6 | By hand: `stapel check` on STP-2 to STP-6; new hits recorded as escaped defects | — | closed tickets' text |

## Review

TODO

## Summary

TODO
