# STP-33 — stapel check in the CI of the repository

## Description

Source: `docs/PHASES.md`, plan item 3.2; ledger decision 2026-11-17 (phases 3 to 5 start before the gate).

### Frame

**Problem.** `stapel check` runs only in the agent's shell, so the session that wrote the code also reports
its proof, and every new rule must go into the guard. The same check must run on the server where the
branch is pushed, so that its status is visible before the merge and comes from outside the agent's shell.

**Plan item:** 3.2.

**Risk tags:** `interface` (a new file that `stapel init` installs, new `init` output lines, build metadata
in the binary).

**Non-goals.** Reading the CI status back into `stapel next`, `stapel close` or the guard; branch protection
on GitHub (the person sets it); CI systems other than GitHub Actions; a cache of the build between CI runs;
a check of `main` after a merge; a setup step that makes a `junit` project run green without the person's
edit.

**Promise (closed list).**

1. `stapel init` in the shared mode writes `.github/workflows/stapel-check.yml` when the file is missing,
   rewrites it when it is the unchanged text of an earlier `stapel init`, and keeps a file edited by hand
   with a warning line, as it does for the skill and the agent files. In the local mode it writes no
   workflow. When the binary has no build commit or was built from a dirty tree, or the ticket key has no
   prefix, `init` writes no workflow and prints a warning line with the reason; the rest of `init` runs
   as before. When the workflow's path, `.github` or `.github/workflows` is not a place to write a file
   (a folder at the file path, a file or a symlink at a folder path), `init` exits 1 and writes nothing.
2. The workflow runs on a push of a branch whose name is a ticket key of the repository's prefix
   (`STP-*`), and on a pull request whose head branch is such a name. It checks out the head of the
   branch (not the merge commit) with the full history and a local branch for every remote branch, and
   gives the job read-only access to the repository contents and no secrets.
3. The workflow installs `stapel` with `cargo install --git https://github.com/kyyyyf/stapel --rev
   <commit> --locked stapel-cli`, where `<commit>` is the git commit the running binary was built from;
   `stapel --version` prints that commit. A new file `docs/install.md` gives the install command that
   records the commit and marks a tree with any change, untracked files included, as dirty.
4. The workflow runs `stapel check <branch>` and the job fails when the check exits other than 0, a
   refusal for a branch without step commits included. The job has a limit of 60 minutes.
5. Under `runner = "junit"` the workflow has a marked setup step for the person to fill, and `init` prints
   a `note:` line that names it.
6. This repository has the workflow installed by `stapel init`, and it runs green on the branch of this
   ticket.

Not promised: an agent that edits the branch's own files the check reads (SR1-6, deferred); a pin that
stays old until `init` runs again (SR1-5, deferred); the rest of the list in Spec, Guarantees.

**Size.** About 200 source lines (init, the build commit, the workflow template) and 400 test lines, 10
Plan steps; split past 400 source lines. Route: full (tag `interface`), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-18: Decision (trigger, the human, recommended): CI runs on a push of a ticket branch and on a
  pull request from it; the ticket key is the branch name. Rejected: a push to `main` checking the tickets
  closed in the pushed range (the status comes after the merge); both.
- 2026-11-18: Decision (where the file lives, the human): `stapel init` writes the workflow for any
  repository. Rejected (recommended): a hand-written file of this repository only.
- 2026-11-18: Decision (feedback, the human, recommended): stapel does not read the CI status; the status
  is visible on GitHub only. Rejected: `stapel close` refuses without a green CI status read through `gh`.
- 2026-11-18: Decision (binary in CI, the human, recommended): the workflow runs `cargo install --git
  https://github.com/kyyyyf/stapel --rev <commit> --locked` with the commit the binary was built from.
  Rejected: install from `main` (CI may run another version than the local one); a build from the checkout
  in this repository only (a second template).
- 2026-11-18: Decision (junit, the human, recommended): under `runner = "junit"` the same workflow with a
  marked setup step and a checklist line. Rejected: no workflow for junit.
- 2026-11-18: Decision (when not written, the human, recommended): no workflow in the local mode; an
  existing hand-edited file is kept. Rejected: also none without a `github.com` remote.
- 2026-11-18: Decision (no build commit, the human, recommended): when the binary has no build commit or
  was built from a dirty tree, `init` writes no workflow and prints the reason. Rejected: write the
  workflow with `--branch main` and a warning.
- 2026-11-18: Decision (branch without steps, the human, recommended): every exit other than 0 fails the
  job, the refusal for a branch without step commits included. Rejected: a green status with a notice.
- 2026-11-18: Decision (re-run of init, the human, recommended): `init` rewrites the workflow when it is
  the unchanged text of an earlier `init`, and keeps a hand-edited file with a warning. Rejected: the
  file is written once.
- 2026-11-18: Decision (time limit, the human, recommended): the job has `timeout-minutes: 60`.
  Rejected: 30 minutes; no limit (the GitHub default of 6 hours).
- 2026-11-18: Assumption (orchestrator): the build commit comes from `STAPEL_BUILD_COMMIT` at compile
  time (`option_env!`), set by the install command in `docs/install.md` as `$(git rev-parse HEAD)`
  followed by `-dirty` when `git status --porcelain` prints anything (untracked files count; the full id
  in a SHA-256 repository). No build script: a `build.rs` in the ticket's range is a build input, and
  `stapel check` then reports every step unverified (ledger finding of STP-31).
- 2026-11-18: Assumption (orchestrator): debug builds read `STAPEL_TEST_BUILD_COMMIT` at run time in
  place of the compiled value, so that the CLI tests can set it.
- 2026-11-18: Assumption (orchestrator): the workflow marker is its first line, because a second `---`
  in YAML starts a new document; the hash and version rules are those of the process files.
- 2026-11-18: Assumption (orchestrator): the push filter is `<PREFIX>-[0-9]+`; a `[tickets] key` that is
  not `<PREFIX>-{n}` gives no workflow and a warning.
- 2026-11-18: Assumption (orchestrator): under junit the `Setup` step fails with a message until the
  person edits it, so that a forgotten setup is red, not a silent pass; without `[check]` the cargo
  workflow is written and `stapel check` refuses in CI as it does locally.

## Spec

Terms:

- **Workflow**: the file `.github/workflows/stapel-check.yml`.
- **Build commit**: the value of `STAPEL_BUILD_COMMIT` when the binary was compiled, read with
  `option_env!`; in a debug build the variable `STAPEL_TEST_BUILD_COMMIT` at run time takes its place,
  so that tests can set it. It is **clean** when it is 40 or 64 lowercase hex digits, **dirty** when it is
  such digits followed by `-dirty`, and **missing** otherwise (unset, empty or any other text).
- **Prefix**: the text before `-{n}` of `[tickets] key`, when the key is `<PREFIX>-{n}` with a prefix of 2
  to 8 uppercase Latin letters (as `stapel init --prefix` takes it).
- **Workflow marker**: the first line `# stapel: <version> sha256:<hex>`, where `<hex>` is the SHA-256 of
  the file's bytes after that line. A workflow is **stapel's** when its marker is valid and its hash
  matches, as for the process files (STP-22), with the marker as the first line because YAML has no
  front matter.
- **Golden workflow**: the text of `tests/golden/stapel_check_cargo.yml` (runner `cargo` or no `[check]`)
  or `tests/golden/stapel_check_junit.yml` (runner `junit`); the test fills `<prefix>` and `<commit>` and
  puts the marker line, with `<version>` and `<hash>` computed, before it. The cargo golden file has no
  `Setup` step; the two golden files differ only by that step.
- **Order of outcomes** of `init` for the workflow, the first that applies: the local mode (nothing about
  the workflow, AC-6); a path that is not a writable place (AC-5: refusal, exit 1, nothing written); a key
  without a prefix (AC-8); a missing or dirty build commit (AC-7); otherwise AC-1, AC-3 or AC-4. Only one
  warning about the workflow is printed; an existing file is neither rewritten nor warned about as AC-4
  when an earlier outcome applies. The junit `note:` line is printed whenever the runner is `junit` and the
  outcome is AC-1 or AC-3, the case of an equal text included, so a re-run still names the step.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel init` runs in the shared mode with a clean build commit and a prefix, and the workflow is missing THE command writes the golden workflow for the cargo runner and prints `created: .github/workflows/stapel-check.yml`. | `init_workflow::writes_the_workflow` |
| AC-2 | WHEN the workflow is written THE job runs on a push of a branch matching `<PREFIX>-[0-9]+` and on a pull request whose head branch starts with `<PREFIX>-`; checks out the head commit of the branch with `fetch-depth: 0` and `persist-credentials: false`; makes a local branch for every branch that Checkout fetched, without the network, with `git fetch --no-tags --update-head-ok . '+refs/remotes/origin/*:refs/heads/*' '^refs/remotes/origin/HEAD'`; has `permissions: contents: read` and `timeout-minutes: 60`; installs with `cargo install --git https://github.com/kyyyyf/stapel --rev <commit> --locked stapel-cli`; and runs `stapel check "$BRANCH"` with the branch name, so any exit other than 0 fails the job. | `init_workflow::workflow_lines` |
| AC-3 | WHEN `stapel init` runs again and the workflow is stapel's of a version not greater than the binary's THE command writes the new text and prints `updated: .github/workflows/stapel-check.yml` when the text differs, and prints nothing for it when it is equal. | `init_workflow::rewrites_an_unchanged_workflow` |
| AC-4 | IF the workflow is not stapel's (edited by hand, no marker, a symlink) THEN `init` keeps it and prints `warning: .github/workflows/stapel-check.yml is not the file stapel wrote; kept. Delete it and run stapel init to take stapel's text`; IF it is stapel's of a newer version THEN it keeps it with the newer-version warning of STP-22. | `init_workflow::keeps_a_hand_edited_workflow`, `init_workflow::keeps_a_newer_workflow` |
| AC-5 | IF in the shared mode a directory stands at the workflow's path, or `.github` or `.github/workflows` is a file or a symlink THEN `init` exits 1 with `<path> is a directory; nothing was written` or `<path> is not a directory; nothing was written` and writes no file, whatever the build commit and the key. | `init_workflow::workflow_path_is_a_directory`, `init_workflow::github_folder_is_not_a_directory` |
| AC-6 | WHEN `stapel init` runs in the local mode THE command writes no workflow and prints nothing about it, whatever the build commit, the key, the runner and the path. | `init_workflow::local_mode_writes_no_workflow` |
| AC-7 | IF the build commit is missing THEN `init` writes no workflow, prints `warning: this stapel has no build commit, so .github/workflows/stapel-check.yml was not written; reinstall stapel from a clean checkout as docs/install.md says`, and does the rest of its work with exit 0; IF it is dirty THEN the same with `warning: this stapel was built from a dirty tree (<value>), so .github/workflows/stapel-check.yml was not written; commit and push the change, then reinstall stapel`. | `init_workflow::no_build_commit`, `init_workflow::dirty_build_commit` |
| AC-8 | IF `[tickets] key` has no prefix THEN `init` writes no workflow and prints `warning: the ticket key <key> is not <PREFIX>-{n}, so .github/workflows/stapel-check.yml was not written`. | `init_workflow::key_without_prefix` |
| AC-9 | WHEN `[check] runner = "junit"` THE workflow is the junit golden workflow: before the install step a step `Setup` whose run line fails with `fill in the step Setup of .github/workflows/stapel-check.yml`; and `init` prints `note: fill in the step Setup of .github/workflows/stapel-check.yml; the job fails until you do`. | `init_workflow::junit_setup_step` |
| AC-10 | WHEN `stapel --version` runs THE output is `stapel <version> (<build commit>)` with a clean or dirty build commit and `stapel <version>` with a missing one. | `init_workflow::version_names_the_commit` |
| AC-11 | WHEN the tests of this repository run THE file `.github/workflows/stapel-check.yml` is stapel's and equals the golden workflow for prefix `STP` and the commit in its install line; `docs/install.md` holds the install command that sets `STAPEL_BUILD_COMMIT` from `git rev-parse HEAD` and `git status --porcelain`. | `init_process::this_repository_has_the_workflow` |

### Guarantees

- **Promised:** the Frame's promise items 1 to 5 by AC-1 to AC-10 (AC-5 and AC-8 under item 1); item 6 by AC-11 and by one green run
  of the workflow on the pushed branch `STP-33`, linked in Proof (the person pushes; agents may not).
  Unchanged by the steps of this ticket: every other file and line `init` writes; the exit code of `init`
  except AC-5; `stapel check`. Changes that came into `base..head` by the merges of main or by ledger
  decisions, not by a step, are listed in Design "as built" (CR1-2).
- **Not promised:** the Frame's list; the check reads the branch's own `stapel.toml`, `state.json` and
  workflow, which an agent can edit (SR1-6); a pin that stays on an old commit after later tickets change
  `stapel check` (SR1-5); a repository not hosted on GitHub (the file is written and does nothing); pull
  requests from forks whose branch is a ticket key of another repository; a pull request branch that
  starts with `<PREFIX>-` but is no ticket key, or differs in case (the check refuses and the job fails);
  two runs for one commit pushed and opened as a pull request; a build commit that is not on
  `github.com/kyyyyf/stapel` (the install step fails); a binary installed by `cargo install --git` without `STAPEL_BUILD_COMMIT` (no
  workflow, AC-7); GitHub's behaviour itself (the tests read the file, they do not run Actions); a branch
  `STP-x` that matches the pull request condition but is no ticket (the check refuses and the job fails);
  a workflow file that cannot be read for reasons other than AC-5 (exit 1 with the file system's error,
  not tested).
- **Time (SR1-9):** estimate for this repository on a GitHub runner of 4 cores: `cargo install` about 4
  minutes; `stapel check` of 10 steps about 25 minutes (here about 10 minutes at `-j 4`, the runner taken
  as 2 to 3 times slower, cold). The green run in Proof records the measured time; a run over 45 minutes
  goes to the person as a finding at the close.

## Test plan

CLI tests in the new file `crates/stapel-cli/tests/init_workflow.rs` with `common::git_repo`,
`common::stapel` and `common::read`; they set `STAPEL_TEST_BUILD_COMMIT` on the command (the test binary is
a debug build). Golden files `crates/stapel-cli/tests/golden/stapel_check_cargo.yml` and
`stapel_check_junit.yml`. The marker check reuses the hash rule of `init_process::check_marker` on the first
line. The existing `init` tests run with a missing build commit; those that assert an exact stderr learn
the AC-7 warning line in the RED of the step that adds it. Light tests: `stapel cargo test -j 4 -p
stapel-cli --test init_workflow -- --test-threads=4` (and `--test init`, `--test init_process`); before
each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Write | shared mode, clean commit `0123…` (40 hex), prefix `ABC`, no workflow: the file equals the golden cargo workflow, `created:` line, valid marker | `init_workflow::writes_the_workflow` |
| Lines | the AC-2 lines read from the written file: `branches: ["ABC-[0-9]+"]`, `startsWith(github.head_ref, 'ABC-')`, `ref: ${{ github.event.pull_request.head.sha \|\| github.sha }}`, `fetch-depth: 0`, `persist-credentials: false`, `contents: read`, `timeout-minutes: 60`, the install line with the commit, `stapel check "$BRANCH"` with `BRANCH: ${{ github.head_ref \|\| github.ref_name }}`; no `secrets.` | `init_workflow::workflow_lines` |
| Re-run | init twice with the same commit: no workflow line the second time; then with another clean commit: `updated:` and the new commit; a 64-hex commit: written; an old stapel workflow with a dirty commit, and with a key without prefix: not rewritten, only the AC-7 or AC-8 warning | `init_workflow::rewrites_an_unchanged_workflow` |
| Hand edit | a line added; the marker removed; a symlink to a stapel workflow: kept, warning, bytes unchanged; a hand-edited file with a missing commit: kept, only the AC-7 warning | `init_workflow::keeps_a_hand_edited_workflow` |
| Newer | a stapel workflow with marker version `99.0.0` and a valid hash: kept, warning names `99.0.0` | `init_workflow::keeps_a_newer_workflow` |
| Directory | a folder at the path: exit 1, the AC-5 text, no process file written; the same with a missing build commit | `init_workflow::workflow_path_is_a_directory` |
| Folder not a directory | `.github` a file; `.github/workflows` a file; `.github` a symlink to a folder outside the repository: exit 1, the AC-5 text naming that path, nothing written there or in the repository | `init_workflow::github_folder_is_not_a_directory` |
| Local | `init --local` with a clean commit; with a missing commit; with a key without prefix; with runner `junit`; with a folder at the workflow path: no `.github` written, exit 0, no line about the workflow | `init_workflow::local_mode_writes_no_workflow` |
| Missing commit | variable unset; empty; `abc`; 39 hex; uppercase hex: no file, the AC-7 warning, exit 0, the skill written | `init_workflow::no_build_commit` |
| Dirty commit | `<40 hex>-dirty`: no file, the AC-7 dirty warning with the value | `init_workflow::dirty_build_commit` |
| Key | `key = "{n}"`, `key = "abc-{n}"`, `key = "ABC{n}"`, `key = "A-{n}"`, `key = "ABCDEFGHI-{n}"`: no file; `AB-{n}` and `ABCDEFGH-{n}`: written, the AC-8 warning; with a missing commit too: the AC-8 warning only | `init_workflow::key_without_prefix` |
| Junit | runner `junit`: the golden junit workflow, the `note:` line; a second run with the same commit: the `note:` line again, no `updated:` line; with a missing commit: no `note:` line | `init_workflow::junit_setup_step` |
| Version | clean, dirty, missing: the three AC-10 forms | `init_workflow::version_names_the_commit` |
| This repository | the workflow of this repository against the golden text; `docs/install.md` holds `STAPEL_BUILD_COMMIT=`, `git rev-parse HEAD` and `git status --porcelain` | `init_process::this_repository_has_the_workflow` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `STAPEL_BUILD_COMMIT` / `STAPEL_TEST_BUILD_COMMIT` | text | 40 or 64 hex, optional `-dirty` | unset or empty: missing (AC-7) | uppercase, 39 hex, other text: missing; `-dirty`: dirty | exact | `init_workflow::no_build_commit`, `init_workflow::dirty_build_commit` |
| `[tickets] key` | template | prefix of 2 / 8 letters | — | not `<PREFIX>-{n}`: AC-8 | exact | `init_workflow::key_without_prefix` |
| `[check] runner` | `cargo`, `junit`, absent | — | absent: the cargo workflow | other values: refused by the config parser as before | exact | `init_workflow::junit_setup_step` |
| `[repository] history` / `--local` | `shared`, `local` | — | absent: shared | — | exact | `init_workflow::local_mode_writes_no_workflow` |
| `--prefix` | 2 to 8 uppercase letters | as before | asked as before | refused as before | exact | `init_workflow::writes_the_workflow` |
| `--version` | flag | — | — | — | exact | `init_workflow::version_names_the_commit` |
| The workflow file | file | 0 bytes / any | missing: written | edited, symlink, newer: kept; directory: AC-5 | exact bytes | `init_workflow::keeps_a_hand_edited_workflow`, `init_workflow::workflow_path_is_a_directory` |
| `.github`, `.github/workflows` | folders | — | missing: created | a file or a symlink: AC-5 | — | `init_workflow::github_folder_is_not_a_directory` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | shallow clone in CI | `fetch-depth: 0` in the workflow | `init_workflow::workflow_lines` |
| git | detached HEAD in CI; the merge commit of a pull request | the checkout takes the head commit; `stapel check` gets the key from `$BRANCH` | `init_workflow::workflow_lines` |
| git | no local branches in CI (only `origin/*`); `stapel check` reads `refs/heads/<base_branch>` for a merge of the base branch (F-6) | the local fetch line makes the local branches from `refs/remotes/origin/*` | `init_workflow::workflow_lines` |
| git | history shape, object format, worktrees, submodules, paths, messages | as `stapel check` before; not changed | — |
| git | the install command of `docs/install.md`: untracked files, a SHA-256 repository, a linked worktree | `git status --porcelain` counts untracked files as dirty; `git rev-parse HEAD` gives the full id (64 hex) and works in a worktree; the command is text, not run by the tests | `init_process::this_repository_has_the_workflow` |
| git | dirty tree at build time | the dirty build commit (AC-7) | `init_workflow::dirty_build_commit` |
| git | configuration: hooks path, `GIT_*` | not read by the new code | — |
| github-actions | event: push of a ticket branch, pull request from it, other branches | the AC-2 triggers and condition | `init_workflow::workflow_lines` |
| github-actions | pull request from a fork; secrets; token rights | read-only contents, no secrets, no stored credentials; no step after Checkout uses the network, so a private repository works (CR1-1) | `init_workflow::workflow_lines` |
| github-actions | runner image and its Rust toolchain; job time | `ubuntu-latest` with its toolchain; 60 minutes | `init_workflow::workflow_lines` |
| cargo | network errors; the commit not on the remote; `Cargo.lock` out of date | the install step fails and the job fails; not promised | — |
| cargo | `STAPEL_BUILD_COMMIT` changed between builds | `option_env!` makes cargo rebuild; not tested | — |
| cargo | workspace, targets, `.cargo/config.toml`, build scripts | no build script is added | — |
| os | a directory at the workflow path | AC-5 | `init_workflow::workflow_path_is_a_directory` |
| os | symlink at the workflow path | kept as not stapel's | `init_workflow::keeps_a_hand_edited_workflow` |
| os | case-insensitive file systems; permissions | not promised | — |

**Review Focus:** a GitHub glob in `branches:` that GitHub reads differently from the intent `<PREFIX>-` and
digits (the file is checked as text only; the green run in Proof is the one real test); `${{ … }}` text in
the golden file that a template filler could treat as its own placeholder (`init_workflow::writes_the_workflow`
compares whole bytes); a debug-only test variable that leaks into a release build
(`init_workflow::version_names_the_commit` runs the debug binary only; not tested for release); CRLF in an
edited workflow (reads as not stapel's; not tested); an `init` run inside a ticket worktree (the file goes
into that branch; not promised).

## Design

### Facts

- F-1: `init.rs::plan_create_text` writes a process file when nothing is at its path, updates it when
  `unchanged_version` finds a valid marker of a version not greater than the binary's, and keeps it with
  a warning otherwise; the marker lives in a `---` front matter.
- F-2: `init.rs::plan_process_files` refuses a directory at a process file's path before anything is
  written (`<rel> is a directory; nothing was written`).
- F-3: `main.rs` declares `#[command(name = "stapel", version)]`; `stapel --version` prints `stapel 0.1.0`.
- F-4: `build_inputs.rs` treats `build.rs`, toolchain files and `Cargo.toml` keys as build inputs; a
  non-RED commit of the range that changes one makes `stapel check` report the steps unverified.
- F-5: `check.rs` refuses a shallow clone; `tickets::match_branch` lets a KEY pass when HEAD is on no
  branch, so `stapel check STP-33` works on a detached checkout.
- F-6: `build_inputs::base_merge_parent` reads `refs/heads/<base_branch>` to see that a merge brings the
  base branch; without that local ref the merge's build inputs block the steps after it.

### Decisions (before the build)

- A new module `crates/stapel-cli/src/build_commit.rs` gives `build_commit() -> BuildCommit` (`Clean(sha)`,
  `Dirty(value)`, `Missing`) from `option_env!("STAPEL_BUILD_COMMIT")`, or in a debug build from the run-time
  variable `STAPEL_TEST_BUILD_COMMIT` when it is set; and `version_text()` for clap's `version`.
- A new module holds the templates as `include_str!` of one asset with a marked junit block, fills
  `<prefix>` and `<commit>` by plain replacement of those two tokens only (the marker's version and hash
  are computed, not filled), adds the marker line, and gives `plan_workflow`. The marker check is a
  first-line form of `unchanged_version`, sharing its version and hash rules. (As built differs: see
  below.)
- `init::run` calls `plan_workflow` after `plan_process_files`; it follows the order of outcomes of the
  Terms; the path refusals of AC-5 (`symlink_metadata` of `.github`, `.github/workflows` and the file)
  join the other refusals before anything is written; the warnings and the `note:` line print with the
  other lines.
- `docs/install.md` (new) says why the commit is recorded and names the install command:
  `STAPEL_BUILD_COMMIT="$(git rev-parse HEAD)$(test -z "$(git status --porcelain)" || echo -dirty)" cargo
  install --path crates/stapel-cli --locked`.
- This repository's workflow is written by `stapel cargo run -p stapel-cli -- init` in the worktree with
  `STAPEL_BUILD_COMMIT` set to the RED commit of step 6 (clean, holding all the code), not by the installed
  binary.

### Decisions (as built)

- `crates/stapel-cli/src/build_commit.rs` holds `BuildCommit`, `parse`, `build_commit()` and
  `version_text()`; `main.rs` declares `version = build_commit::version_text()`.
- There is no `workflow.rs`: the workflow code lives in `crates/stapel-cli/src/init.rs` as
  `check_workflow_path` (the AC-5 refusals, shared mode only, before anything is written), `key_prefix`,
  `plan_workflow` (the warnings in the order of the Terms, then `plan_create_text`; it returns whether the
  junit `note:` line is due) and `unchanged_workflow_version` (the first-line marker).
  `plan_create_text` takes the marker reader as an argument; the process files pass `unchanged_version`.
- Two templates, not one with a block: `crates/stapel-cli/assets/stapel-check.yml` and
  `crates/stapel-cli/assets/stapel-check-junit.yml`, with the tokens `{PREFIX}` and `{COMMIT}`; the golden
  files use `<prefix>` and `<commit>`.
- Step 2 first wrote the workflow outside the plan; step 4 moved it into the plan, so a kept workflow
  stops the `already set up` line as a kept process file does.
- `workflow_lines` checks the shared mode only and nothing about `init`'s output lines; it also checks the
  `push:` and `pull_request:` triggers and the whole `if:` line (D1-3). The RED of step 3 makes
  `init_process::hash_computed_by_hand` and `hooks_install::no_warning_when_stapel_on_path` expect exactly
  the AC-7 missing-commit warning (D1-2), so both fail at that RED.
- The history was rewritten with the human's consent (2026-11-18): `stapel check` found steps 2a, 2b and
  3 `no-red`, because their REDs changed old tests that did not fail there. The `workflow_lines` changes
  moved into the RED of step 2, and the two step 3 tests became exact. The old head is the local branch
  `STP-33-backup`.
- The quoted output lines stand as literals in the code that prints them (the ticket lint looks for them);
  the missing-commit warning is one line under `#[rustfmt::skip]`. Step 4 also repeats the `created:`,
  `updated:` and kept lines in a doc comment of `plan_workflow`.
- This repository's workflow pins `f49448d2da1aabeea046a5e44d5b72bb56f319e7`, the RED commit of step 6;
  step 7 moves the pin to its own RED commit.
- The first CI run failed the suite on `export::export_output` (newer git refuses `git remote add my/fork`
  while `my` exists). A ticket STP-34 for it could not close, because `stapel check` verifies no fix that
  lies only in tests, and a fix commit inside this branch made every step unverified; that commit was
  dropped by a second history rewrite (ledger decisions of 2026-11-18). The fix came by a commit to main
  and a merge of main.
- Changes in `base..head` that no step makes (CR1-2, CR1-8): by the merges of main, `tests/export.rs`
  (`git config remote.my/fork.url`), `tests/check_history.rs` (`twenty_steps_within_five_seconds` allows
  15 s, which weakens STP-14 AC-10, about 850 git calls through the macOS xcrun shim), `tests/tokens.rs`
  (the fixture copy is read and written, so it gets the current modification time on macOS), and the
  skill text of rules 2 and 6 (`crates/stapel-cli/assets/skill.md`, `.claude/skills/stapel/SKILL.md`, from
  the STP-32 ledger decision of 2026-11-17); by a ledger decision of 2026-11-19, `check.timeout_secs` 1800
  in `.stapel/stapel.toml` (was 900). The local check ran with `TMPDIR` on a case-sensitive disk image.
- Step 7 (CR1-1): the `Local branches` step fetches from `.`, the local repository, not from `origin`;
  Checkout with `fetch-depth: 0` has already fetched every branch to `refs/remotes/origin/*`.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `init_workflow::writes_the_workflow` |
| AC-2 | `init_workflow::workflow_lines` |
| AC-3 | `init_workflow::rewrites_an_unchanged_workflow` |
| AC-4 | `init_workflow::keeps_a_hand_edited_workflow`, `init_workflow::keeps_a_newer_workflow` |
| AC-5 | `init_workflow::workflow_path_is_a_directory`, `init_workflow::github_folder_is_not_a_directory` |
| AC-6 | `init_workflow::local_mode_writes_no_workflow` |
| AC-7 | `init_workflow::no_build_commit`, `init_workflow::dirty_build_commit` |
| AC-8 | `init_workflow::key_without_prefix` |
| AC-9 | `init_workflow::junit_setup_step` |
| AC-10 | `init_workflow::version_names_the_commit` |
| AC-11 | `init_process::this_repository_has_the_workflow`; the green run of the workflow on branch `STP-33` at `4bad7c3`: https://github.com/kyyyyf/stapel/actions/runs/44380029933 (2026-11-18, 10 min 55 s for the whole job, under the estimate of SR1-9) |

## Plan

Route: full (tag `interface`), auto.

1. The build commit and the version (AC-10; test `init_workflow::version_names_the_commit`): the new file
   `tests/init_workflow.rs` comes with this RED. Must not change: any other output.
2. The workflow lines (AC-2; test `init_workflow::workflow_lines`, which fails at the RED because no
   file is written): the GREEN writes the template with the prefix and a clean build commit when nothing
   is at the path, in both modes (knowingly wrong until step 2b), with no marker and no output line, and
   never touches an existing path; with a missing or dirty commit or no prefix it writes nothing,
   silently. Must not change: the process files and the other `init` lines.
2a. The marker and the golden text (AC-1; test `init_workflow::writes_the_workflow`, which fails at the
   RED on the marker and the `created:` line): the golden cargo workflow comes with the RED. Must not
   change: step 2.
2b. The local mode (AC-6; test `init_workflow::local_mode_writes_no_workflow`, which fails at the RED
   because step 2 writes in both modes). Must not change: step 2a.
3. No build commit (AC-7; tests `init_workflow::no_build_commit`, `init_workflow::dirty_build_commit`,
   which fail at the RED on the warning lines): the RED also teaches the existing `init` tests that
   assert an exact stderr the AC-7 warning line; `docs/install.md` comes with the GREEN and is tested in step 6. Must not change:
   step 2b.
3a. No prefix (AC-8; test `init_workflow::key_without_prefix`, which fails at the RED on the warning
   line). Must not change: step 3.
4. Re-run and kept files (AC-3, AC-4; tests `init_workflow::rewrites_an_unchanged_workflow`,
   `init_workflow::keeps_a_hand_edited_workflow`, `init_workflow::keeps_a_newer_workflow`, which fail at
   the RED because step 2 never rewrites and prints no warning). Must not change: the process-file rules.
4a. The path refusals (AC-5; tests `init_workflow::workflow_path_is_a_directory`,
   `init_workflow::github_folder_is_not_a_directory`, which fail at the RED: a folder at the file path is
   skipped with exit 0, `.github` as a file fails at the write after the process files are written, and a
   symlinked `.github` is written through). Must not change: step 4.
5. Junit (AC-9; test `init_workflow::junit_setup_step`): the golden junit workflow comes with the RED.
   Must not change: the cargo workflow.
6. This repository (AC-11; test `init_process::this_repository_has_the_workflow`): the GREEN writes the
   workflow with `stapel cargo run -p stapel-cli -- init` and `STAPEL_BUILD_COMMIT` set to the RED commit
   of this step. Must not change: the other installed files. After the GREEN the person pushes the branch
   and the run's link goes into Proof.
7. The local fetch (AC-2; test `init_workflow::workflow_lines`, which fails at the RED on the new fetch
   line; the RED also changes that line in both golden files): the GREEN changes the `Local branches` line
   of both templates and rewrites this repository's workflow with `stapel cargo run -p stapel-cli -- init`
   and `STAPEL_BUILD_COMMIT` set to the RED commit of this step. Must not change: the other workflow lines.
   After the GREEN the person pushes the branch and the new run's link goes into Proof.

## Review

TODO

## Summary

TODO
