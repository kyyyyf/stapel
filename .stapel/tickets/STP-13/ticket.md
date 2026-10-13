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
3. adds an entry to a `dev-dependencies` table, or changes one (Open question 8);
4. adds a file under `crates/*/tests/` that is not a regular `.rs` file, or changes, deletes or renames one
   in a crate's tests folder that holds no step test of a RED before the commit; in a tests folder that does,
   deletes or renames one or changes its mode, link target or submodule commit (STP-4's `tests-changed`
   covers a change of its content; code review round 1).

It gives `red-changes-code` when a RED of the ticket:

5. redirects a dev-dependency as in item 1;
6. adds, changes or deletes a `Cargo.toml`, a symbolic link, a `.cargo` path or a `rust-toolchain` file under
   `crates/*/tests/`;
7. adds, changes or deletes a file that a `build` key or the `path` key of `[lib]`, `[[bin]]`, `[[example]]`
   or `[[bench]]` names.

Not promised: the `features` of an entry outside `dev-dependencies`; what a crates.io version or a workspace member contains (code); proc-macro
crates that read files; a RED that does not compile and that a crates.io entry added by a GREEN to
`[dependencies]` resolves (see Open question 6); STP-6's "Not promised" list.

**Size.** Medium: about eight criteria, 600–900 changed lines, most of them tests. Beyond ten criteria the
ticket is split again. As built after step 4b (2026-10-12): eight criteria, about 540 changed source lines and
1600 test lines; the estimate counted too few test lines, no split needed.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question. The human is away from 2026-10-09
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
- 2026-10-12: Decision: the human answered Open questions 1 to 7 "as recommended" (1a, 2a, 3b, 4b, 5a, 6a,
  7a); every Assumption above is now a Decision. STP-15 takes the include and run part (plan step 5).
- 2026-10-12, during the build: spec change after the drift reviews of steps 1 to 4 (orchestrator, the human
  away; needs a new confirmation): D1-1 (an entry under both spellings of a table is a redirect), D1-2
  (`workspace = true` takes the root's `[workspace.dependencies]` source), D1-3 (an absolute or backslash
  `path` names no member folder), D3-2 (AC-5 counts a link or submodule entry whatever its name). Plan step 4b.
- 2026-10-12, code review round 1 (R, E, DR; no HIGH), orchestrator, the human away; needs a new
  confirmation. Inside the promise, fixed in plan step 7 (spec change of promise item 4 and AC-5 to what
  item 4 already claimed): R-1, E-2, DR-1, DR-2 (a delete, rename, mode, link or submodule change in a step
  crate), R-2, DR-3 (a crate is a step crate for a commit only after the RED of a step with a test in it),
  R-3 (fail closed), DR-7, DR-8, E-3 (text). DR-5 is rejected: cargo cannot build a crate whose manifest
  does not parse, so its `build` and `path` keys name nothing that is built. B4b-1 and D4b-2 are covered by the same fix.
- Open question 8 (code review E-1, outside the promise): a non-RED commit changes a dev entry's version or
  `features` without changing its source, and a RED that did not compile then passes. (a) **recommended**:
  widen promise item 3 and AC-4 to "adds or changes an entry of a `dev-dependencies` table" (AC-2 keeps
  `dependencies` and `build-dependencies`); (b) leave it in Not promised. Assumption: (b) until the human
  answers; the build does not start on it.
- 2026-10-13: Decision: the human answered Open question 8 (a). Plan step 8.

## Spec

Terms of STP-6 (Range, non-RED commit, build-input outcome) hold. New terms:

- **Dependency table**: `dependencies`, `dev-dependencies` (alias `dev_dependencies`) or `build-dependencies`
  (alias `build_dependencies`) of a manifest, at the top level, under `[target.<cfg>]` (quoted or bare key) or,
  for `dependencies`, under `[workspace]`; an entry is a key of such a table, in inline, dotted or
  `[dependencies.<name>]` form, compared by table and key; an entry present under both spellings of one
  table at a commit is a redirect (spec change, D1-1).
- **Source** of an entry: its values of `path`, `git`, `branch`, `tag`, `rev`, `package`, `registry`,
  `registry-index`; a string entry and a table without these keys have an empty source; an entry with
  `workspace = true` has the source of the same entry of the root manifest's `[workspace.dependencies]` at
  that commit (spec change, D1-2).
- **Member folder**: the folder of the root package or of a `members` entry of the root manifest, globs
  expanded over the commit's tree and `exclude` removed.
- **Redirect**: an entry present at a commit whose source differs from the same entry at the parent (an
  absent entry has an empty source), unless the new source is empty, or the entry is not in a
  `dev-dependencies` table and its new source is a `path` (with or without `package`) that names a member
  folder at the commit (an absolute `path` or one holding a backslash names no member folder, D1-3); an
  entry whose source at the parent has `path` or `git` and is empty at the commit is
  a redirect; an entry that is not a string or a table is a redirect. A deleted entry is no redirect.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF a non-RED commit makes a redirect in any dependency table of any `Cargo.toml` THEN the check gives the build-input outcome naming the manifest. | `check::dependency_redirect_is_unverified`, `check::redirect_under_both_spellings_is_unverified`, `check::inherited_member_path_in_dev_table_is_unverified`, `check::absolute_path_names_no_member` |
| AC-2 | WHEN a non-RED commit adds or changes an entry of `dependencies` or `build-dependencies` whose new source is empty or names a member folder, THE steps keep their outcomes. | `check::crates_io_and_member_dependencies_are_allowed` |
| AC-3 | IF a non-RED commit adds, changes or deletes `[patch]`, `[replace]`, `[profile]`, `[features]`, `[lints]`, `[workspace.lints]`, or `edition` of `[package]`, `[project]` or `[workspace.package]`, of any `Cargo.toml` THEN the check gives the build-input outcome naming the manifest. | `check::patch_profile_features_lints_and_edition_are_unverified` |
| AC-4 | IF a non-RED commit adds an entry to a `dev-dependencies` table or changes one (version, `features` or any other key; spec change, Open question 8) THEN the check gives the build-input outcome naming the manifest. | `check::green_adding_a_dev_dependency_is_unverified`, `check::root_change_of_an_inherited_dev_entry_is_unverified` |
| AC-5 | IF a non-RED commit adds an entry under `crates/*/tests/` that is not a regular `.rs` file (a symbolic link or submodule entry counts whatever its name, D3-2), or changes, deletes or renames such an entry in a crate's tests folder that holds no step test of a RED before the commit, or deletes or renames such an entry or changes its mode, link target or submodule commit in a tests folder that does, THEN the check gives the build-input outcome naming it. | `check::test_data_change_is_unverified`, `check::delete_rename_or_chmod_in_a_step_crate_is_unverified`, `check::step_crate_counts_only_after_its_red`, `check::link_retarget_and_rs_to_data_rename_in_a_step_crate_are_unverified` |
| AC-6 | IF a RED makes a redirect in a `dev-dependencies` table THEN the step is `red-changes-code` naming the manifest. | `check::red_redirecting_a_dev_dependency_changes_code`, `check::red_inheriting_a_path_into_a_dev_table_changes_code` |
| AC-7 | IF a RED adds, changes or deletes a `Cargo.toml`, a symbolic link, a path with a `.cargo` component, `rust-toolchain` or `rust-toolchain.toml` under `crates/*/tests/` THEN the step is `red-changes-code` naming it. | `check::red_adding_a_test_manifest_or_link_changes_code` |
| AC-8 | IF a RED adds, changes or deletes a file that, at the RED or its parent, a manifest's `build` key of `[package]` or `[project]` or the `path` key of `[lib]`, `[[bin]]`, `[[example]]` or `[[bench]]` names THEN the step is `red-changes-code` naming the file. | `check::red_changing_a_build_or_target_file_changes_code`, `red_fails_closed::unreadable_tree_counts_as_code` |

### Guarantees

- **Promised, against an agent that tries on purpose to make a step look verified** (tag `security`): the
  Frame's promise, by AC-1 to AC-8.
- **Not promised:** the Frame's list; a manifest that does not parse (STP-6's fail-closed rule keeps it a
  build input: AC-1 of STP-6; in a RED it names no files for AC-8, code review DR-5); a crate folder renamed by a non-RED commit makes every entry of its manifest
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
| `steps::changes` read `--name-status`, without modes, before STP-13 step 4, so a RED's symbolic link under `crates/*/tests/` was not seen as a link; it now reads `--raw -z` with modes | `crates/stapel-core/src/steps.rs` | verified |
| A RED whose step test does not compile counts as a RED (`TestState::Compile`) | `crates/stapel-cli/src/check.rs` | verified |

### Decisions (as built)

Written as notes during the build and moved here before code review (`CLAUDE.md`, item 6).

#### Step 1
- `manifest_hit` (status M/T) parses both texts after STP-6's `targets()` comparison and returns the manifest path when `dependency_redirect(before, after, manifest, is_member)` holds.
- `dependency_entries` maps (scope, kind, name) to the TOML value; scope is top level, `target <cfg>` or `workspace`; `dev_dependencies`/`build_dependencies` fold into the hyphenated kinds.
- Source = the eight source keys; a changed entry is a redirect when it is not a string or table, when its new source is non-empty unless outside `dev-dependencies` and a `path` (optionally `package`) naming a member folder, or when it moves from `path`/`git` to empty. An unchanged invalid entry is not flagged (cargo rejects it).
- `member_folders(root, commit)`: root package folder, `members` globs over `git ls-tree` manifest folders (links skipped), `exclude` removed as folder or prefix; in-code `glob_path` (`*`, `?`, `[..]`), no new crate; lazy per commit; unreadable root manifest: no members (fails closed). `resolve()` extracted from `build_file`.
- The AC-2 test passed at RED and moved to a coverage commit after the GREEN (a local rewrite by the human; old ids 99d5f62, 3fd2856).
#### Step 2
- `build_tables(&toml::Table)` collects `patch`, `replace`, `profile`, `features`, `lints`, `package.edition`, `project.edition`, `workspace.lints`, `workspace.package.edition`; a member's `[patch]` counts.
- `manifest_hit`: STP-6 `targets()` first; then `build_tables` differ (absent vs present counts) or `dependency_redirect` fires; `edition.workspace = true` is a parsed value, so a change.
- `dependency_redirect` is true for an entry absent at the parent whose kind is `dev`, whatever its source (AC-4); a dependency moved from `[dependencies]` to `[dev-dependencies]` counts as added; a move between the two dev spellings does not.
#### Step 3
- "Holds a step test" follows STP-4: `analyse()` collects `step_crates`, the `t.dir` of the step tests of every RED of the range (same loop as `step_names`); in such a crate a change, delete or rename of a data file stays STP-4's `tests-changed`.
- `data_hit` runs after the manifest rule and before the `.rs` helper rule; a data path counts as added when the entry has no old path or the old path is in another crate; a gitlink is an ordinary entry; a rename names the old path first.
- The RED moved the example data file of the STP-6 test `unchanged_build_inputs_are_named` from `crates/<tiny>/tests/golden/g.txt` to `notes/golden/g.txt` (AC-5 now counts the old place).
#### Step 4
- `steps::changes` reads `git diff --raw -M -z`; `Change` gains `old_mode`/`new_mode` (`120000` is a link; a type change T counts through either side).
- `outcomes::is_code` is STP-4's rule (renamed `is_code_stp4`, unchanged) plus the STP-13 rules over a `Change`: under `crates/*/tests/` a link, `Cargo.toml`, `rust-toolchain(.toml)` or `.cargo` component on the old or new path (AC-7); a path named by a manifest at the RED or its parent (AC-8); a manifest whose `dev-dependencies` hold a pure redirect (AC-6).
- `build_inputs::entry_redirect` is the pure redirect of one entry; `dependency_redirect` (AC-1/AC-4) adds "an added dev entry is a hit" on top, so AC-4 is unchanged. `dev_redirect(before, after, manifest)` runs `entry_redirect` over `dev` entries only, with no member folders (a dev `path` is never a member exemption); a crates.io dev entry added is no redirect.
- `target_files(manifest, text)`: `build` of `[package]`/`[project]`, `path` of `[lib]`, `[[bin]]`, `[[example]]`, `[[bench]]`, resolved as `resolve`; `Files::named_files(commit)` lists every `Cargo.toml` of the commit over `git ls-tree` once and caches the set.
#### Step 4b
- `dependency_entries(t, workspace_deps)` returns `Dep { value, inherited }`; `workspace = true` takes the root's `[workspace.dependencies]` entry (absent: empty string, empty source) and its `path` resolves against `Cargo.toml`; an entry under both spellings of one table becomes an array of both values (neither string nor table), so a change is a redirect.
- `manifest_hit` reads the root manifest at the parent and at the commit; when the root is the changed manifest and its `[workspace.dependencies]` differ, `inheritors_redirect` re-evaluates every manifest of the commit with old and new root deps (a failed `ls-tree` counts as a redirect). `dev_redirect` takes the root texts at the RED's parent and at the RED.
- `entry_redirect`: an absolute `path` or one with a backslash names no member folder (`resolve`, used by AC-8, is unchanged: absolute is taken as relative there).
- `data_hit`: a mode 120000 or 160000 entry is data whatever its name; an entry that became a link or submodule counts as added. Already caught before by the STP-6 link rule and the `.rs` helper rule, so its test is a coverage test.
- `Files::named_files` returns `Option`; a failed `ls-tree` or an unreadable manifest is `None` and `is_code` returns true. `member_folders` already returned `None` (not a member, a redirect) on a git error or an unreadable root manifest.
#### Step 7
- `data_hit` takes `step_crates: HashMap<crate, first RED index>` and the commit's range index `at`; a crate is a step crate only when its index is below `at` (R-2, DR-3).
- In a step crate a delete, a rename, a mode change or a submodule commit change of a non-`.rs` entry (or a link/submodule-named `.rs`) names the old path; a content change stays STP-4 `tests-changed` (R-1, E-2, DR-1, DR-2).
- `steps::changes` calls the new `parse_changes`, which returns `Err` on a record without `:`, a short record or truncated paths (R-3, DR-4).
- DR-5 is dropped: cargo cannot build a crate whose manifest does not parse, and STP-6 flags a change of such a manifest; `target_files` stays unchanged.
- Coverage: a member whose package name differs from its folder, and glob characters in a data path (DR-8).

#### Step 7b
- `data_hit`: a link entry (mode 120000) in a step crate that changes counts as moved, so its path is named (D7-1). The suite line already named it; the step line stays `tests-changed`.
- `data_hit`: a rename or copy from a regular `.rs` file to a non-`.rs` path counts the new path as added, in any crate (D7-2).
#### Step 8
- `dependency_redirect`: a `dev` entry (either spelling, `[target.<cfg>]` included) that differs from its old value is a hit, added or changed in any key; a move between the two spellings with an equal value is equal in `dependency_entries`, so no change.
- Unchanged: `dependencies` and `build-dependencies` (AC-2), `entry_redirect` and `dev_redirect` (a RED that changes a crates.io dev entry stays valid).
- Test: the target-table cases run in a second repository, because the check names only the first unverified commit.
#### Step 8b
- `inheritors_redirect`: a `dev` entry whose inherited root entry differs (any key, version or `features` included) is a hit, naming the root manifest as for D1-2 (D8-1). `dependencies` and `build-dependencies` keep the source-only rule (AC-2).

## Test plan

CLI tests in `crates/stapel-cli/tests/check.rs`, as in STP-6: a tiny cargo crate in a temporary git
repository with RED and GREEN steps. Heavy runs: `cargo test -j 4 … -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | crates.io, `workspace = true` and member dependencies added by a GREEN (member by glob, root package) | `check::crates_io_and_member_dependencies_are_allowed` |
| Main path, RED | a RED adds a crates.io dev-dependency and a data file under `crates/*/tests/` | `check::red_adding_a_crates_io_dev_dependency_stays_valid` |
| Negative | each source key in each table form and alias; a `path` entry moved to crates.io; `[patch]`, `[replace]`, `[profile]`, `[features]`, `[lints]`, `edition`; a dev-dependency added by a GREEN; a data file added, changed, deleted, renamed; RED-side items | `check::dependency_redirect_is_unverified`, `check::patch_profile_features_lints_and_edition_are_unverified`, `check::green_adding_a_dev_dependency_is_unverified`, `check::test_data_change_is_unverified`, `check::red_redirecting_a_dev_dependency_changes_code`, `check::red_adding_a_test_manifest_or_link_changes_code`, `check::red_changing_a_build_or_target_file_changes_code`, `check::redirect_under_both_spellings_is_unverified`, `check::inherited_member_path_in_dev_table_is_unverified`, `check::absolute_path_names_no_member`, `check::red_inheriting_a_path_into_a_dev_table_changes_code`, `red_fails_closed::unreadable_tree_counts_as_code`, `check::delete_rename_or_chmod_in_a_step_crate_is_unverified`, `check::step_crate_counts_only_after_its_red`, `steps_changes::malformed_or_short_records_fail_closed`, `check::link_retarget_and_rs_to_data_rename_in_a_step_crate_are_unverified`, `check::root_change_of_an_inherited_dev_entry_is_unverified` |
| Coverage (steps 4b, 7) | drift findings D1-L, D2-2, D2-L, D3-2, D3-L, D4-2, D4-L; code review DR-8 | `check::manifest_with_ac3_tables_added_deleted_renamed_is_unverified`, `check::link_named_rs_under_tests_is_data`, `check::data_moves_and_renames_are_unverified`, `check::version_description_and_dev_spelling_changes_are_allowed`, `check::member_globs_and_exclusions_are_read`, `check::red_changing_a_file_named_through_dots_or_a_subfolder_changes_code`, `check::red_edge_cases_of_named_files_and_dev_entries_change_code`, `check::member_package_name_may_differ_from_its_folder`, `check::data_path_with_glob_characters_is_unverified` |
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
| A RED that adds `crates/<x>/tests/fake/Cargo.toml`, a link `crates/x/tests/data -> /tmp/d`, or `crates/<x>/tests/.cargo/config.toml` | AC-7 |
| A RED that changes a `build = "tests/gen.rs"` script | AC-8 |

**Author self-check:** 2026-10-09, after spec review round 1: every promise item has a criterion and a test;
the Inputs and External states rows come from the catalogue; round 2 fixes applied; open questions 1 to 7
wait for the human.
2026-10-12, after code review round 1: promise item 4 and AC-5 rewritten to the behaviour item 4 claimed;
every promise item still has a criterion and a test; Open question 8 waits for the human.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check::dependency_redirect_is_unverified`, `check::redirect_under_both_spellings_is_unverified`, `check::inherited_member_path_in_dev_table_is_unverified`, `check::absolute_path_names_no_member` |
| AC-2 | `check::crates_io_and_member_dependencies_are_allowed` |
| AC-3 | `check::patch_profile_features_lints_and_edition_are_unverified` |
| AC-4 | `check::green_adding_a_dev_dependency_is_unverified`, `check::root_change_of_an_inherited_dev_entry_is_unverified` |
| AC-5 | `check::test_data_change_is_unverified`, `check::delete_rename_or_chmod_in_a_step_crate_is_unverified`, `check::step_crate_counts_only_after_its_red`, `check::link_retarget_and_rs_to_data_rename_in_a_step_crate_are_unverified` |
| AC-6 | `check::red_redirecting_a_dev_dependency_changes_code`, `check::red_inheriting_a_path_into_a_dev_table_changes_code` |
| AC-7 | `check::red_adding_a_test_manifest_or_link_changes_code` |
| AC-8 | `check::red_changing_a_build_or_target_file_changes_code`, `red_fails_closed::unreadable_tree_counts_as_code` |

## Plan

Route: full (source: `auto`; tags `security`, `guard`).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | Dependency sources and redirects in `build_inputs.rs` | AC-1, AC-2 | STP-6 outcomes and golden files |
| 2 | `[patch]`, `[replace]`, `[profile]`, `[features]`, `[lints]`, `[workspace.lints]`, `edition`; dev-dependencies added by a non-RED commit | AC-3, AC-4 | the manifest keys STP-6 AC-1 compares |
| 3 | Non-`.rs` files under `crates/*/tests/` | AC-5 | `.rs` helper and step-file rules of STP-6; STP-4 `tests-changed` and its test `check::outcome_tests_changed_after_red` |
| 4 | RED rules in `outcomes::is_code`, with modes for links | AC-6, AC-7, AC-8 | STP-4 RED rule for other paths |
| 4b | Drift fixes: D1-1, D1-2, D1-3, D3-2 (spec change, AC-1, AC-2, AC-5), D4-1 (fail closed); tests for D2-2, D4-2 and the LOW gaps | AC-1, AC-2, AC-5, AC-8 | the outcomes of steps 1 to 4 |
| 5 | `docs/PHASES.md`: STP-15 | — | other tickets |
| 6 | By hand: `stapel check` on STP-2 to STP-6; new hits recorded as escaped defects | — | closed tickets' text |
| 7 | Code review round 1 fixes: R-1, R-2, R-3, DR-2 in code; tests for DR-8 rows | AC-5, AC-8 | STP-4 `tests-changed`; outcomes of steps 1 to 4b |
| 7b | Drift fixes of step 7: D7-1 (link retarget in a step crate), D7-2 (`.rs` renamed to a data name) | AC-5 | STP-4 `tests-changed` |
| 8 | A non-RED commit that changes a dev entry gives the build-input outcome (Open question 8) | AC-2, AC-4 | AC-2 for `dependencies` and `build-dependencies`; a RED adding a crates.io dev entry stays valid |
| 8b | Drift fix D8-1: a root `[workspace.dependencies]` change of an entry a dev entry inherits | AC-4 | AC-2 |

## Review

Round 1 (2026-10-12), on a read-only copy at `2f61cd1` with the as-built notes: fresh reviewer (Sonnet, R-1 to
R-8), external reviewer (Fable, E-1 to E-4), drift reviewer (Sonnet, DR-1 to DR-9). No HIGH, so no round 2.
Eight MEDIUM, seven of them inside the promise, all of one class: promise item 4 said STP-4's `tests-changed`
covers changes in a step crate, but it misses a delete, a rename, a mode, link or submodule change, and the
step crates were taken without the order of the REDs. Fixed in plan steps 7 and 7b (with drift D7-1, D7-2);
DR-5 rejected; E-1 (outside the promise) became Open question 8, answered (a) on 2026-10-13 and built in plan
step 8, with drift fix D8-1 in step 8b (a root change of an inherited dev entry). A non-dev root change with
an equal source has no test of its own (AC-2 path, unchanged).

## Summary

**Measures after the close (2026-10-13), compared with STP-6.** Nine RED/GREEN steps (1, 2, 3, 4, 4b, 7, 7b,
8, 8b) and two steps without code (5, 6); the last `stapel check` passes at `d9ad20c` with 377 tests. Tokens
are measured: the STP-13 part of the main transcripts `14b5ca6e` (after the STP-6 close) and `e930def0`, and
all 22 subagent files of `e930def0`, each mapped to its role from its first prompt; no file was left
unmapped. Subagent output counts are partial in the transcripts (`≥`), so cache reads are compared. STP-6 has
no measured records by role, only the session totals of `14b5ca6e` by model.

| Measure | STP-6 | STP-13 |
|---|---|---|
| Escaped defects found so far | 0 in STP-6; older ones in STP-2, STP-3, STP-4 | 0 in STP-13; plan step 6: no new hits of the STP-13 rules on STP-2 to STP-6; after the close X-1 (`stapel status … \| head` panics on a closed pipe, from STP-2 step 7, LOW, a later ticket) |
| Findings before code (spec review, per-step drift) | 23 + 33 | 13 + 22 (HIGH and MEDIUM: 11 + 13) |
| Code review findings, all / HIGH and MEDIUM inside the promise | 32 over two rounds / 10 (4 HIGH) | 12 rows in one round / 7 (0 HIGH) |
| Code review HIGH and MEDIUM by `catchable_at` | spec 7, design 5, code 4, test-plan 3 | design 5, spec 2, code 1 |
| `ticket.md` | 46 KB | 39 KB |
| Changed lines (frame estimate) | about 3 600 (400–500) | about 2 170: 650 source, 1 520 tests (600–900) |
| Steps with RED/GREEN, tests at the close | 15, 349 | 9, 377 |
| Subagent estimates by role (end notices) | builder 1.40M, external 0.53M, drift 0.39M, reviewer 0.15M | builder 0.80M, drift 0.46M, external 0.27M, reviewer 0.12M |
| Measured cache read (cache write) by role | — | orchestrator 42.5M (0.68M), builder 15.4M (2.32M), external 1.95M (0.42M), drift 1.88M (0.37M), reviewer 0.50M (0.11M) |
| Measured cache read by model (session totals for STP-6) | Opus 72.3M, Sonnet 33.0M, Fable 4.0M | Opus 42.5M, Sonnet 17.6M, Fable 1.95M, Haiku 0.17M |
| Orchestrator output | 199k (session total) | 172k |
| Local history rewrites | 3 | 3 (steps 1, 3, 4b), run by the human |
| Wall time | 2 days, about 12 active hours | 4 calendar days (2026-10-09 to 2026-10-13), about 14 active hours |

Tokens, measured against estimated: an end notice gives the context size of the last call, not a sum, so it
understates a subagent by 4 times (reviewer) to 19 times (builder). Builders read 15.4M cached tokens over 9
steps (about 1.7M a step); the orchestrator read 42.5M, still the largest cost, though 41 % less than STP-6.

What improved: the split held (eight criteria, one round of code review, no HIGH); the spec review and the
per-step drift reviews caught the dependency and data-file bypasses before code review; the orchestrator only
coordinated, every code step came from a builder brief; measured tokens fell for every model against STP-6
(Opus 41 %, Sonnet 47 %, Fable 52 % less); no new escaped defect of STP-13. What did not: the frame's line
estimate counted too few test lines; the promise leaned on an older rule (STP-4 `tests-changed`) without
checking what it covers, and code review found that gap; the outside finding E-1 became two more steps (8,
8b) late in the build; builders still caused three history rewrites, each now a rule in the builder
briefs, and STP-14 (`stapel check --history`) would have shown all three at once; active time grew a little
(about 14 hours against 12) for a smaller change; subagent output is still not measured (partial lines).

**Process ledger (CLAUDE.md item 9), code review HIGH and MEDIUM by the rule that should have caught them:**
item 3 (tables and promise against an older rule) 5, item 4 1, item 2 1, item 8 1. Item 9 over STP-5, STP-6
and STP-13: no item went three tickets in a row without a catch or a miss; items 1 and 5 had none in STP-13
only, so nothing is removed.
