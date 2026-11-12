# STP-31 — stapel init on a foreign repository

## Description

Source: `docs/PHASES.md`, stage A item A2 (plan item 2.1, widened); ledger decisions 2026-11-10 (A2 widened)
and 2026-11-12 (A2 runs in parallel with A1c, STP-30).

### Frame

**Problem.** On a foreign repository `stapel init` writes into the shared `.gitignore` and `.gitattributes`,
the process commits `.stapel/` and `.claude/`, and the pre-push hook may land in a tracked hooks folder (the
pilot keeps `.husky/`). Every merge request would carry process files. Without `Cargo.toml` the starter
`[check]` is a cargo one, and the person gets no list of the manual steps.

**Plan item:** A2 (item 2.1, widened; A1 is closed as STP-28).

**Risk tags:** `interface` (an `init` option, a command, a config key, the checklist); `data` (the command
writes a branch of rewritten commits).

**Non-goals.** `init` without `--local`, apart from the starter template; the PreToolUse guard (frozen); the
push and the merge request (the person's); hooks folders outside the work tree (current warning); deliberate
circumvention of the pre-push check.

**Promise (closed list).**

1. `stapel init --local` records the local mode in `stapel.toml`, and later runs keep it. In local mode the
   ignore lines go to `.git/info/exclude` and the merge lines to `.git/info/attributes`, never to
   `.gitignore` or `.gitattributes`.
2. In local mode a stapel pre-push hook that `init` creates or finds inside the work tree (a `core.hooksPath` folder such as
   `.husky/`) is listed in `.git/info/exclude`; a pre-push that is not stapel's stays, with the current
   warning.
3. Without `Cargo.toml` the starter `stapel.toml` holds a commented `junit` `[check]` template (`command`,
   `report`, `tests`) in place of the cargo one, in both modes.
4. In local mode the pre-push hook also refuses a push whose new commits (not yet on a remote) have
   `.stapel/` in a tree, naming `stapel export`; `stapel next` names a missing stapel pre-push hook first.
5. `stapel export [KEY] [--onto <ref>]` writes the branch `export/<KEY>`: the commits of `<ref>..<KEY>`
   with the process paths as they are in `<ref>`, metadata kept, emptied commits dropped. Without `--onto`:
   the upstream of the base branch.
6. In local mode `init` ends with the person's checklist of manual steps.
7. A smoke test runs the whole path on a fixture repository like the pilot's.

Not promised: signed commits; submodules and LFS; tags; that Claude Code really calls the hooks (the
checklist); husky 9 (`.husky/_`).

**Size.** About 400 source lines and 800 test lines, 12 Plan steps. Route: full (tags `interface`, `data`),
auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-12: Decision (storage, the human, recommended): the process commits locally as it does now, and a
  new command makes a clean branch for the push; the pre-push hook refuses a push that carries `.stapel/`.
  Rejected: no process file tracked at all (state in the git common folder; over 12 steps, a split); only the
  files of `init` untracked (tickets would still reach the merge request).
- 2026-11-12: Decision (smoke ticket, the human, recommended): an integration test on a fixture repository
  plus a manual checklist item for the hooks in Claude Code. Rejected: `stapel init --smoke` with a real
  ticket in the foreign repository.
- 2026-11-12: Decision (silent hook loss, the human, recommended; SR1-4): in local mode `stapel next` names a
  missing stapel pre-push hook at the current hooks path first, so the session start shows it after `npm
  install` moves `core.hooksPath`. Rejected: the checklist alone.
- 2026-11-12: Decision (existing `export/<KEY>`, the human, recommended; SR1-8): export replaces it; the
  branch is derived from the ticket branch and the reflog keeps the old tip. Rejected: a refusal without
  `--force`.
- 2026-11-12: Decision (orchestrator, confirmed with the spec; SR1-6, SR2-7): promise item 2 also covers a
  stapel hook that `init` finds inside the work tree, and AC-9 may rewrite a stapel hook in shared mode when
  its text is not the mode's text. Rejected: a warning for ignored process files (SR1-7); the checklist's
  `git add -f` with exact paths covers it.
- 2026-11-12: Assumption (orchestrator): the local mode is chosen by the option `--local`, not detected,
  because the existing `init` tests run on fresh repositories without `.stapel/` and detection would change
  their meaning.
- 2026-11-12: Assumption (orchestrator): `export` takes the process paths from `<ref>`, so a tracked
  `.claude/settings.json` of the foreign repository keeps its own content in the merge request.

## Spec

Terms:

- **Local mode**: `stapel.toml` holds `[repository]` with `history = "local"`. Without that table, or with
  `history = "shared"`, the mode is **shared** and `init` works as before this ticket.
- **Exclude file** and **attributes file**: the paths that `git rev-parse --git-path info/exclude` and
  `--git-path info/attributes` print, so every worktree of the repository shares them.
- **Pre-push path**: the path that `git rev-parse --git-path hooks/pre-push` prints (`core.hooksPath`
  included), as `init` reads it today.
- **Process paths**: the folders `.stapel/` and `.claude/skills/stapel/`, every file directly in
  `.claude/agents/` whose name starts with `stapel-` and ends with `.md`, and the file
  `.claude/settings.json`.
- **Branch state**: the file `.stapel/tickets/<KEY>/state.json` as it is in the commit `refs/heads/<KEY>`
  (`git show`), so export reads the same ticket from any checkout, merged or not.
- **Onto ref**: the value of `--onto`; without it, `<base_branch>@{upstream}` of the Branch state. Its commit
  comes from `rev-parse --verify --end-of-options` (git 2.30 or later), its full name, when it has one, from
  `rev-parse --symbolic-full-name`; an ambiguous name resolves as git resolves it.
- **Export range**: the commits reachable from `refs/heads/<KEY>` and not from the Onto ref, parents before
  children.
- **Clean tree** of a commit: its tree with every Process path removed and every Process path that the Onto
  ref's tree has put back with that tree's mode and object.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel init --local` runs and `.stapel/stapel.toml` does not exist THE starter `stapel.toml` holds `[repository]` with `history = "local"` and the run is in Local mode. | `init_local::local_mode_is_recorded` |
| AC-2 | WHEN `stapel init` runs with or without `--local` and `stapel.toml` already has `history = "local"` THE run is in Local mode and changes nothing that a first run wrote. | `init_local::second_run_keeps_local_mode` |
| AC-3 | IF `stapel init --local` runs and `stapel.toml` exists in shared mode THEN init exits 1 with `.stapel/stapel.toml is in shared mode; --local applies only to the first stapel init. To switch, add [repository] with history = "local" to it, remove stapel's lines from .gitignore and .gitattributes, and run stapel init` and writes nothing. | `init_local::local_flag_refused_on_a_shared_config` |
| AC-4 | IF `[repository]` holds `history` with a value other than `local` or `shared`, or another key THEN loading `stapel.toml` fails with a message that names the key. | `core::config::repository_history_key` |
| AC-5 | WHEN init runs in Local mode THE ignore lines (`/.stapel/index/`, `/.claude/worktrees/`) go to the Exclude file and the merge lines (`… merge=union`) to the attributes file, each only when the file lacks it, the file and its folder made when missing; `.gitignore` and `.gitattributes` are neither made nor changed; each line label names the file relative to the root, or by its absolute path when it lies outside the root. | `init_local::lines_go_to_the_git_folder` |
| AC-6 | IF in Local mode the Exclude file or the attributes file is not UTF-8 THEN init exits 1 with a message that names the file and writes nothing. | `init_local::refuses_a_non_utf8_exclude_file` |
| AC-7 | WHEN init in Local mode creates, updates or finds a pre-push hook with stapel's marker and the Pre-push path lies inside the work tree THE path relative to the root, with a leading `/`, goes to the Exclude file when it lacks it; a Pre-push path inside the git folder adds no line. | `init_local::pre_push_in_a_tracked_hooks_folder` |
| AC-8 | WHEN the pre-push hook of Local mode runs and the agent check of shared mode lets it go on THE hook reads each input line `<local ref> <local sha> <remote ref> <remote sha>`, skips a line of another shape and an all-zero local sha, and IF `git rev-list -n 1 --full-history <local sha> --not --remotes -- .stapel` names a commit (a new commit that adds, changes or removes `.stapel/`) THEN it prints `stapel: <local ref> carries .stapel/; push the branch that stapel export writes` and exits 1; else it exits 0. | `init_local::pre_push_refuses_process_files` |
| AC-9 | WHEN init finds a pre-push hook with stapel's marker whose text differs from the text of the run's mode THE hook gets that text and stays executable, with the line `updated: <path> (pre-push)`; a hook without the marker is kept with the current warning. | `init_local::pre_push_text_follows_the_mode` |
| AC-10 | WHEN init writes a new `stapel.toml` in a repository without `Cargo.toml` THE commented `[check]` template names `runner = "junit"`, a `command` with `{file}`, `report` and `tests`, and the text with the `# ` taken off those lines loads; a cargo repository gets the cargo `[check]` as before. | `init::writes_commented_check_otherwise`, `init::writes_check_for_cargo_repositories` |
| AC-11 | WHEN init ends in Local mode THE checklist of the golden file `init_local_checklist.txt` follows the file lines, on every run; shared mode prints no checklist. | `init_local::prints_the_checklist` |
| AC-12 | WHEN `stapel export [KEY]` runs in any checkout of the repository THE ticket is KEY, without KEY the name of the current branch, the source is `refs/heads/<KEY>` and the ticket's data is its Branch state; IF KEY does not match the key pattern of `stapel.toml`, is no valid branch name (`git check-ref-format --branch`), the branch is missing, there is no current branch, or the Branch state cannot be read THEN export exits 1 with a message and changes no ref. | `export::export_key_and_refusals`, `export::key_must_be_a_branch_name` |
| AC-13 | WHEN `--onto` is absent THE Onto ref is the full name that `git rev-parse --symbolic-full-name <base_branch>@{upstream}` prints for the Branch state's `base_branch`; IF the Branch state has no `base_branch`, that exact branch has no upstream, or the git call fails THEN export exits 1 with a message that names `--onto`. | `export::onto_defaults_to_the_upstream`, `export::default_upstream_is_exact` |
| AC-14 | IF the Onto ref names no commit, shares no commit with the branch, has `.stapel` in its tree, the Export range is empty, holds a commit without parents, or every commit of it is dropped, or `export/<KEY>` is the checked-out branch of a worktree THEN export exits 1 with a message that names the cause and changes no ref. | `export::export_refusals` |
| AC-15 | WHEN export runs THE each commit of the Export range gets a new commit with its Clean tree, its parents mapped (a parent inside the range to that parent's new commit, any other parent as it is), and the rest of its raw commit object kept byte for byte (author and committer lines, an `encoding` header, the message), only `gpgsig`, `gpgsig-sha256` and `mergetag` headers dropped, written with `git hash-object -t commit --literally`, so a commit git stores with a malformed ident is copied too and a second run on the same commits gives the same tip. | `export::rewrites_the_range`, `export::export_is_repeatable`, `export::keeps_identity_and_encoding`, `export::copies_odd_commits_and_base_names` |
| AC-16 | WHEN the Clean tree of a commit with one parent equals the Clean tree of that parent (inside the range its new commit's tree, outside it the parent's own Clean tree) THE commit is dropped and its children take that parent in its place; WHEN the mapped parents of a merge are one commit THE merge is dropped if its Clean tree equals that commit's, else written with that one parent; WHEN a merge has two different mapped parents, one of them is an ancestor of the other, and the merge's Clean tree equals the Clean tree of the later one (a `git pull` into local `main`) THE merge is dropped and its children take that later parent; any other merge of two different parents is kept. | `export::drops_empty_commits`, `export::drops_against_clean_parent_trees`, `export::drops_pull_merges` |
| AC-17 | WHEN export ends THE ref `refs/heads/export/<KEY>` points at the new tip and the output is that of the golden file `export_output.txt`: one line with the tip, the commit count, the dropped count and the Onto ref, and, when the Onto ref lies under `refs/remotes/`, a line `push: git push <remote> export/<KEY>:<KEY>`, where `<remote>` is the configured remote with the longest name that, followed by `/`, starts the rest of the ref; the output goes through the `out` module, and child processes are piped with plain `.stdout(…)` and `.stderr(…)` calls. | `export::export_output`, `closed_pipe::no_print_macros_outside_out`, `closed_pipe::no_stream_aliases` |
| AC-18 | WHEN the person's path runs on a fixture repository without `Cargo.toml`, with hooks in a tracked folder and a script that writes a JUnit report THE `init --local`, `new`, `check`, the session and tool hooks, the pre-push and `export` succeed or refuse as AC-1 to AC-17 say, `export/<KEY>` holds no Process path, and no tracked file of the main checkout changes. | `init_local::foreign_repository_end_to_end`, `init_local::foreign_repository_real_pushes` |
| AC-19 | WHEN export ends or refuses THE ref `export/<KEY>` is written only as the last git call (the remote list is read before it), the export's temporary folder is gone, and the working tree, the index and `HEAD` are unchanged. | `export::export_leaves_the_checkout` |
| AC-20 | WHEN `stapel next` runs in Local mode, in the main checkout or a linked worktree, and the Pre-push path as the main worktree resolves it is missing, not executable, or a hook with stapel's marker whose text is not the Local-mode text THE fact `the stapel pre-push hook is missing or stale at <path>; run stapel init` is the first fact (the first `blocked:` line before any other in text, the first item of `facts` in `--json`) and the action is `init — stapel init`, in every case of `next`, no open ticket and a refusal of `next` included, and the session-start hook prints that fact and action also when `next` refuses; a hook without stapel's marker, and a Pre-push path outside the main worktree (init only warns there), give no fact. | `next::local_mode_names_a_missing_pre_push`, `next::hook_fact_in_worktrees_and_foreign_hooks`, `hook_session::session_start_keeps_the_hook_fact` |
| AC-21 | WHEN `stapel export` runs from a subfolder of the work tree or from a linked worktree THE Process paths and the `.stapel` check are read from the top of the tree, and `export/<KEY>` is the same as from the root. | `export::export_from_a_subfolder` |
| AC-22 | WHEN `stapel init` runs in Local mode from a linked worktree THE pre-push hook is planned at the Pre-push path as the main worktree resolves it, so the AC-20 fact clears there, and no hook file is written in the linked worktree. | `init_local::init_from_a_worktree_fixes_the_main_hook` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-22; `init` without
  `--local` on a repository without a Local-mode `stapel.toml` writes what it wrote before, apart from the
  starter `[check]` template (AC-10) and the pre-push update of AC-9.
- **Not promised:** the Frame's list; a Process path that the foreign repository tracks other than
  `.claude/settings.json` (export puts back the Onto ref's version, so a tracked `.claude/agents/stapel-x.md`
  of the team keeps its content); a change of `core.hooksPath` after init (the checklist says to run init
  again); a push that bypasses hooks (`--no-verify`); a pre-push input line of a deleted ref (all-zero local
  sha: allowed); a commit rewritten by export whose message or author was changed after the person read it;
  a ticket that itself changes `.claude/settings.json` (export drops that change); the speed of export on a
  range of thousands of commits; the time of the pre-push on a first push to a remote with no
  remote-tracking refs (one `rev-list` over the history); a hooks path with spaces, glob characters, a `#`
  at the start or a symlink to the root (the Exclude line may not match); a stapel hook committed earlier
  under a tracked folder (an exclude line does not untrack it); a pushed branch that carries only other
  Process paths (the pre-push checks `.stapel/` only); an export killed between its git calls (no ref is written, a temporary index may stay in the temporary
  folder of the system); `GIT_DIR` or `GIT_INDEX_FILE` set by a caller; a later ticket's export that holds the commits of
  an earlier ticket not yet merged upstream (the commits come out the same, so the later branch stacks on the
  earlier one); a push from a linked worktree (git resolves a relative hooks path in each worktree, where the hook file is absent, so it runs no stapel hook; the person pushes from the main checkout); a bare main repository with linked worktrees only (the hook fact cannot clear there); a sparse checkout; a Windows file system.

## Test plan

CLI tests in the new files `crates/stapel-cli/tests/init_local.rs` (the smoke test too: a coverage test in
a new file reads as a build input) and `export.rs`; the starter
template test in `init.rs`; the key test in `crates/stapel-core/tests/config.rs`. Each test makes a temporary
repository with `common::git_repo`; a test with a remote makes a bare repository with `git init --bare` and
pushes `main` to it, so `main@{upstream}` exists. The pre-push hook runs as `hooks_install.rs` runs it, with
`CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT` removed and the input lines on stdin. Each export test sets its own `TMPDIR`. Exact output lives in the
golden files `init_local_checklist.txt` and `export_output.txt`. Light tests: `stapel cargo test -j 4 -p
stapel-cli --test <file> -- --test-threads=3`, `-p stapel-core --test config`; before each GREEN also
`--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Mode recorded | `init --local --prefix ABC` in a fresh repository: `[repository] history = "local"` in `stapel.toml`; `Config` reads Local mode | `init_local::local_mode_is_recorded` |
| Mode kept | a second `init` without `--local`, a third with it: `already set up`, no file changed, `stapel.toml` still in Local mode | `init_local::second_run_keeps_local_mode` |
| Flag on shared | `init --prefix ABC`, then `init --local`: exit 1, the AC-3 message, the tree snapshot unchanged | `init_local::local_flag_refused_on_a_shared_config` |
| Key | `history = "local"`, `"shared"`: read; `"Local"`, `""`, `1`, an extra key `mode`: refused naming the key; no `[repository]`: shared | `core::config::repository_history_key` |
| Git folder | Local mode in a repository with a tracked `.gitignore` (`node_modules/`) and no `.gitattributes`: both unchanged and `git status` clean of them; the Exclude file without a final newline gets a newline first; a missing `info` folder is made; a second run adds no line twice; a linked worktree's `--git-path` gives the shared file | `init_local::lines_go_to_the_git_folder` |
| Not UTF-8 | an Exclude file with byte `0xff`: exit 1 naming it, the tree snapshot unchanged; the same for the attributes file | `init_local::refuses_a_non_utf8_exclude_file` |
| Hooks folder | `core.hooksPath = .husky` with a tracked `.husky/pre-commit`: `.husky/pre-push` made executable and `/.husky/pre-push` in the Exclude file, `git status` shows nothing new; `core.hooksPath` absolute inside the work tree: the same line; the default `.git/hooks`: no line; a hooks folder outside the work tree: the current warning, no line; a stapel hook already in `.husky/` from a shared install, then the config set to Local mode and `init`: the line added | `init_local::pre_push_in_a_tracked_hooks_folder` |
| Push check | input of a commit with `.stapel/` in its tree: exit 1, the AC-8 message with its ref; a commit without it: exit 0; a tip that removes `.stapel/` on top of a commit that has it, neither on a remote: exit 1; the same history already on the remote's ref: exit 0; two lines, the second carrying `.stapel/`: exit 1; a line of three words: skipped; an all-zero local sha: exit 0; empty input: exit 0; `CLAUDECODE` set: the agent refusal first | `init_local::pre_push_refuses_process_files` |
| Mode text | a shared install, then the config edited to Local mode and `init`: the hook updated with its line; back to shared: updated again; a foreign hook: kept with the warning | `init_local::pre_push_text_follows_the_mode` |
| Template | no `Cargo.toml`: the `# [check]` lines name `junit`, `{file}`, `report`, `tests`; with `# ` removed the text loads as a junit `[check]`; with `Cargo.toml`: `runner = "cargo"` | `init::writes_commented_check_otherwise`, `init::writes_check_for_cargo_repositories` |
| Checklist | first and second Local run: the golden text after the file lines; a shared run: no checklist line | `init_local::prints_the_checklist` |
| Next | Local mode, the hook in place: no new fact; the hook deleted; not executable; with the shared text; `core.hooksPath` set to a folder without it: the AC-20 fact first and the action `init`, with an open ticket, with none, and in `--json`; a foreign hook there: no fact; shared mode without a hook: no fact | `next::local_mode_names_a_missing_pre_push` |
| Next elsewhere | `next` in the ticket's linked worktree with `core.hooksPath = .husky` and the hook in the main checkout: no fact; the main checkout's hook deleted: the fact names `.husky/pre-push`; the default hooks folder deleted, seen from the worktree: the fact; a branch ticket with unreadable state, and a refusal of `next` (two open tickets on `main`): the fact still first | `next::hook_fact_in_worktrees_and_foreign_hooks` |
| Subfolder | `export` run from `src/` and from the ticket's linked worktree: `export/ABC-1` holds no Process path and has the same tip as from the root; `--onto main` with `.stapel/` refused from `src/` too | `export::export_from_a_subfolder` |
| Identity | commits by `John Smith Jr.`, by `"Q" <q@x>`, with an `encoding ISO-8859-1` header and Latin-1 message bytes, and a signed commit: the export keeps the author and committer lines and the encoding and message bytes exactly, drops `gpgsig` | `export::keeps_identity_and_encoding` |
| Clean parents | the range's first commit only changes `.claude/settings.json`, which the Onto ref changed since the merge base: dropped; a merge of local `main` whose parents collapse and whose tree also changes a code file: written with one parent, the change kept | `export::drops_against_clean_parent_trees` |
| Exact upstream | `base_branch = "feature"` with no upstream and a branch `feature/x` with one: exit 1 naming `--onto`; `base_branch = ""`: the same | `export::default_upstream_is_exact` |
| Branch name | `export a..b`, `export 'a b'`, `export x.lock` in a repository without `stapel.toml`: exit 1 naming the key, no ref | `export::key_must_be_a_branch_name` |
| Print | the `closed_pipe` scan finds no print macro in `export.rs` | `closed_pipe::no_print_macros_outside_out` |
| Stream aliases | a source line with `::stdout::<` or `::stderr::<` in `crates/*/src`: flagged; the scan of `stdout(` and `stderr(` matches only `io::stdout(` and `io::stderr(`, so `.stdout(Stdio::piped())` passes | `closed_pipe::no_stream_aliases` |
| Odd commits | a range commit stored with `hash-object --literally` whose author has an empty name and no space before `<`; a commit with `mergetag` and one with `gpgsig-sha256`; a Branch state whose `base_branch` is `-x`: the odd commits copied byte for byte minus the dropped headers; the `-x` base refused naming `--onto` | `export::copies_odd_commits_and_base_names` |
| Pull merges | local `main` = the install commit, then `git pull` brings a new origin commit as a merge; the ticket branch starts after it: the merge is dropped, the export's first commit sits on the new origin commit; a merge of two code lines is still kept | `export::drops_pull_merges` |
| Session start | Local mode, the hook deleted, two open tickets on `main` (`next` refuses): `stapel hook session-start` prints the AC-20 fact and `next: init — stapel init` | `hook_session::session_start_keeps_the_hook_fact` |
| Init from worktree | `core.hooksPath = .husky`, the main checkout's hook deleted, `stapel init` run in the ticket's linked worktree: the hook is written in the main checkout, no `.husky/pre-push` in the worktree, `next` in the worktree gives no fact; the default hooks folder: the same | `init_local::init_from_a_worktree_fixes_the_main_hook` |
| Key | in the worktree on branch `ABC-1` without KEY: `ABC-1`; `export ABC-1` from the main checkout before the merge of `ABC-1`: the ticket read from its branch; `export ABC-9` with no branch: exit 1; on branch `export/ABC-1` without KEY, and `export -x`: exit 1 (no key); detached `HEAD` without KEY: exit 1; a branch `ABC-2` without `.stapel/tickets/ABC-2/state.json`: exit 1; no `export/` ref made in each refusal | `export::export_key_and_refusals` |
| Default onto | `main` pushed to a bare remote: the Onto ref is `origin/main`; no upstream: exit 1 naming `--onto`; a Branch state without `base_branch`: the same | `export::onto_defaults_to_the_upstream` |
| Refusals | `--onto nosuch`; `--onto -x`; `--onto` an unrelated root commit; `--onto main` whose tree has `.stapel/`; `--onto ABC-1` (empty range); an orphan branch merged into the ticket branch; a branch whose only commits touch `.stapel/`; `export/ABC-1` checked out in a worktree: exit 1 naming each cause, `export/ABC-1` unchanged | `export::export_refusals` |
| Rewrite | a range of a RED, a GREEN and a close commit that each also change `.stapel/`, `.claude/agents/stapel-builder.md` and `.claude/skills/stapel/SKILL.md`, a merge of `main` in between, and a tracked `.claude/settings.json` of the Onto ref changed in the range: the export holds the same code files at each commit, no `.stapel/` and no stapel agent or skill in any tree, the Onto ref's `settings.json`, the same author, committer, dates and messages, the merge with both parents mapped; an executable file keeps its mode | `export::rewrites_the_range` |
| Repeat | export twice: the same tip; export, one more commit on the branch, export again: the old tip is the new tip's parent | `export::export_is_repeatable` |
| Empty | the close commit that changes only `.stapel/`: dropped, its child's parent is the GREEN; a merge of local `main` that brings only `.stapel/` changes: dropped; a merge of two code lines: kept | `export::drops_empty_commits` |
| Output | the golden two lines with a remote Onto ref; remotes `my` and `my/fork` and `--onto my/fork/release/1`: the remote `my/fork`; with `--onto <sha>`: one line | `export::export_output` |
| Checkout | a dirty tree with a staged and an unstaged change: after export and after a refusal, `git status --porcelain`, the index file and `HEAD` the same; the test's own `TMPDIR` holds no `stapel-export-*` folder; `GIT_TRACE` shows `update-ref` as the last git call, also with a remote Onto ref | `export::export_leaves_the_checkout` |
| Smoke | the fixture of promise item 7: `init --local`, the `[check]` command filled, the commit of the installed files, `new`, RED and GREEN in the worktree, `check` passes, `stapel hook session-start` prints `stapel next`, `stapel hook pre-tool-use` denies a `git push`, the pre-push run by hand refuses `main` and lets `export/ABC-1` go, `git status` of the main checkout shows no change of a tracked file | `init_local::foreign_repository_end_to_end` |
| Real pushes | the same fixture: the checklist's two `git add` lines commit the installed files, a real `git push` of `main` to a bare remote is refused by the pre-push naming `stapel export`, `git push origin export/ABC-1:ABC-1` goes through and the remote's `ABC-1` is the export tip | `init_local::foreign_repository_real_pushes` |

### Inputs

| Input | Type | Bounds | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `--local` of `init` | flag | — | absent: the mode of `stapel.toml`, else shared | on a shared `stapel.toml`: exit 1 | — | `init_local::local_flag_refused_on_a_shared_config` |
| `--prefix` of `init` | string | as before | as before | as before | — | `init::rejects_bad_prefix` |
| `[repository] history` | string | `local`, `shared` | table absent: shared | other value or key: load fails | exact case | `core::config::repository_history_key` |
| Exclude and attributes files | UTF-8 text | any | missing: made | not UTF-8: exit 1 | lines compared with trailing spaces trimmed, as `.gitignore` | `init_local::refuses_a_non_utf8_exclude_file` |
| pre-push input line | four words | any count | no line: exit 0 | a line of other shape: skipped | full sha | `init_local::pre_push_refuses_process_files` |
| `export KEY` | branch name | any | absent: the current branch | no branch, no Branch state, detached `HEAD`: exit 1 | exact | `export::export_key_and_refusals` |
| working folder of `export` | path | the root, a subfolder, a linked worktree | — | — | git calls run at the top of the work tree | `export::export_from_a_subfolder` |
| `--onto` of `export` | git revision | any | absent: the upstream of `base_branch` | no commit, starts with `-`, unrelated, holds `.stapel`, empty range: exit 1 | resolved once at the start | `export::export_refusals` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | configuration: hooks and their path setting | `core.hooksPath` relative or absolute inside the work tree: excluded; outside: the current warning; the default folder: no line | `init_local::pre_push_in_a_tracked_hooks_folder` |
| git | linked worktrees | the Exclude and attributes files come from `--git-path`, shared by every worktree; export reads the Branch state and refs, not a worktree, so it runs the same from the main checkout; a checked-out `export/<KEY>` is refused | `init_local::lines_go_to_the_git_folder`, `export::export_key_and_refusals`, `export::export_refusals` |
| git | linked worktrees with a relative hooks path | `next` checks the hook as the main worktree resolves it; a push from a linked worktree is not promised | `next::hook_fact_in_worktrees_and_foreign_hooks` |
| git | dirty tree: ignored files | the checklist commits `.stapel` with `git add` and the `.claude` paths with `git add -f` and their exact names; the smoke test runs both lines | `init_local::prints_the_checklist`, `init_local::foreign_repository_real_pushes` |
| git | version | 2.30 or later (`--end-of-options`); older: not promised | — |
| git | configuration: a hooks path changed after init | `stapel next` names the missing hook | `next::local_mode_names_a_missing_pre_push` |
| git | history shape: merge commits | parents mapped, empty merges dropped | `export::rewrites_the_range`, `export::drops_empty_commits` |
| git | history shape: rebased or amended branch | export runs on what the branch holds; the old `export/<KEY>` is replaced | `export::export_is_repeatable` |
| git | dirty tree | export reads commits only and leaves the tree; init's local writes are untracked or in the git folder | `export::export_output`, `init_local::foreign_repository_end_to_end` |
| git | detached HEAD | export without KEY refuses (no branch ticket) | `export::export_key_and_refusals` |
| git | paths with spaces or non-UTF-8 bytes | export copies tree entries by object id, never by text; a test file with a space in its path | `export::rewrites_the_range` |
| git | object format SHA-256; submodules; shallow clone | not promised | — |
| git | commit messages with control bytes | kept as bytes in the raw object | `export::rewrites_the_range` |
| git | `GIT_*` environment, global config | export clears no variable; signing config (`commit.gpgsign`) is ignored, signatures are dropped | — |
| claude-code | settings at project and local level | init writes `.claude/settings.json` as before; export puts back the Onto ref's version | `export::rewrites_the_range` |
| claude-code | whether the hooks really run | not promised; the checklist asks the person to see `stapel next` at session start | — |
| claude-code | environment of hook processes versus the agent's Bash shell | the pre-push agent check is unchanged; the smoke test runs the pre-push without the variables | `init_local::foreign_repository_end_to_end` |
| os | file permissions | the pre-push made or updated executable | `init_local::pre_push_text_follows_the_mode` |
| os | symlinks; case-insensitive file systems | not promised | — |
| cargo | every dimension | not run by this ticket's code | — |

**Review Focus:** a pre-push input line whose local ref is a tag or a non-branch ref (tested only with
branches; the check reads the commit's tree whatever the ref); a tree entry of mode `160000` (a submodule
link) inside a Process path (not promised, export copies it as an entry); an Exclude file that git reads but
another tool rewrites (not promised); a branch `export/<KEY>` the person edited by hand (replaced, the
reflog keeps it; `export::export_is_repeatable`); the time of export on a range with thousands of commits
(not promised).

**Author self-check:** 2026-11-12, before spec review: every promise item has a criterion and a test (item
1: AC-1, AC-2, AC-3, AC-4, AC-5, AC-6; item 2: AC-7, AC-9; item 3: AC-10; item 4: AC-8, AC-20; item 5:
AC-12 to AC-17, AC-19; item 6: AC-11; item 7: AC-18); again after spec review rounds 1 and 2; each Inputs and External states row
has a test or "not promised"; the readers of `stapel.toml` that see the new table are `Config::parse_at` and
`init`.

## Design

### Facts

- F-1: `init::run` (`crates/stapel-cli/src/init.rs`) reads and checks everything first and collects the
  writes in a plan, so a refusal writes nothing. `.gitignore` and `.gitattributes` go through
  `plan_append_lines`; the pre-push hook through `plan_pre_push`, which takes the path from `git rev-parse
  --git-path hooks/pre-push`, warns for a folder outside the work tree, keeps a hook without the marker
  `# stapel: pre-push guard`, and for a hook with the marker only repairs the executable bit.
- F-2: `starter_toml(prefix, cargo)` in `crates/stapel-core/src/config.rs` writes the commented cargo
  `[check]`. The top-level `Config` has no `deny_unknown_fields`; `Check` has it.
- F-3: a refusal of `init` is an `Err(String)`, which `main` turns into exit 1.
- F-4: `stapel new` writes `base` and `base_branch` into `state.json`.
- F-5: `common::junit_repo` appends an uncommented `[check]` to the starter `stapel.toml`; commented
  template lines do not clash with it.
- F-6: the pilot repository (measured 2026-11-12): husky 8 with a tracked `.husky/pre-commit` and no
  `pre-push`; `core.hooksPath` is not set until `npm install` runs `husky install`; `.claude/` is untracked
  there; jest with `jest-junit` is in its `package.json`.

### Decisions (before the build)

- The mode is the new table `Repository { history }` in `stapel-core` config, with `deny_unknown_fields`
  and an enum `History { Local, Shared }`; `Config.repository` is `Option<Repository>`, absent means shared.
- `init` takes the mode from an existing `stapel.toml`, else from `--local`. In Local mode the ignore and
  merge lines go through `plan_append_lines` with the paths of `--git-path info/exclude` and
  `info/attributes`; the line labels print those paths relative to the root (`.git/info/exclude`), or
  absolute when outside it (AC-5).
- Two pre-push texts share the agent check; the local one adds a `while read` loop that skips a line of
  another shape and an all-zero sha and runs `git rev-list -n 1 "$sha" --not --remotes -- .stapel` (AC-8).
  A hook with the marker whose text is not the mode's text is replaced (AC-9).
- `export` refuses a checked-out `export/<KEY>` by reading `git worktree list --porcelain` (AC-14).
- `export` is the new module `crates/stapel-cli/src/export.rs`, built on git plumbing: `rev-list
  --topo-order --reverse --parents <onto>..refs/heads/<KEY>`; per commit a temporary index
  (`GIT_INDEX_FILE` in a temporary folder): `read-tree <commit>`, `rm --cached -r -q --ignore-unmatch` of
  the Process paths, `ls-tree -r -z <onto> -- <paths>` into `update-index -z --index-info`, `write-tree`;
  `commit-tree` with the author and committer variables read from the commit and the raw message on stdin;
  last `update-ref -m "stapel export" refs/heads/export/<KEY> <tip>`. No working tree or `HEAD` is touched.
- The push line takes the remote from the full name of the Onto ref (`refs/remotes/<remote>/…`), matched
  against `git remote` by the longest name.
- The temporary index lives in a `stapel-export-*` folder of the system's temporary folder, removed on every
  return path; `update-ref` is the last git call.
- The checklist (AC-11) has five numbered lines under `checklist:`: commit the installed files on the base
  branch in two lines, `git add .stapel` (so the excluded `.stapel/index/` stays out) and `git add -f` with
  the exact installed `.claude` paths (`.claude/skills/stapel`, each `.claude/agents/stapel-*.md`,
  `.claude/settings.json`), so an ignored `.claude/` is still committed and no other `.claude/` file is; fill `[check] command` in `.stapel/stapel.toml`; restart
  Claude Code and see `stapel next` at session start; run `stapel init` again after anything changes
  `core.hooksPath` (husky on `npm install`); push only the `export/<KEY>` branches that `stapel export`
  writes.
- `export` reads the Branch state with `git show refs/heads/<KEY>:.stapel/tickets/<KEY>/state.json`.
- AC-20 is a check in `crates/stapel-cli/src/next.rs` before every early return of `next::lines` and the
  JSON form; it reads the mode from `stapel.toml`, the Pre-push path as `init` does, and compares the file
  with the Local-mode text, which `stapel-cli` keeps in one place for `init` and `next`.

### As built

- The starter junit template's command is `npx jest {file} --ci --reporters=jest-junit`; `{file}` stands
  right after `jest` because the config refuses `{file}` after a word that starts with `-`.
- `init.rs` keeps the Local-mode pre-push text and `stale_local_pre_push`, which `next` calls; it resolves
  the Pre-push path in the main worktree (`main_worktree`, the first entry of `git worktree list
  --porcelain`) and returns a path relative to that worktree; a hook is stale when missing, not executable,
  or stapel-marked with other text. `next::lines_parts` returns the lines and a refusal, so the fact and
  the action `init` come first also before a refusal.
- `export.rs` runs every git call at `rev-parse --show-toplevel`; reads the Branch state as any JSON object
  and takes only `base_branch`; checks KEY with `check-ref-format --branch` (the printed name must equal
  KEY) and against the key pattern when a `stapel.toml` is found. A new commit is the old raw object from
  `cat-file commit` with `tree` and `parent` lines replaced and `gpgsig`, `gpgsig-sha256` and `mergetag`
  dropped, written with `hash-object -t commit -w --stdin`. The drop test compares Clean trees
  (`clean_tree`) of a commit and its mapped parent. `git rm --cached` runs with `-f`. `rev-parse
  --symbolic-full-name --end-of-options` prints the `--end-of-options` word back, so that line is dropped.
  The remote list is read before `update-ref`. The output line, printed with `say!`, is
  `export/<KEY> <tip>: commits <n>, dropped <m>, onto <ref>`. `capture()` pipes a child's streams through
  `Command::stdout::<Stdio>` so the `closed_pipe` scan does not match (step 21 replaces it).
- The export test fixture keeps `main` free of `.stapel/` (the files of `init` are excluded), as the
  pilot's `origin/main` is; ticket data lives on the ticket branches.
- The smoke test lives in `init_local.rs`: a coverage test in a new file under `tests/` holds no step test,
  so the check reads it as a helper, a build input, and marks every step `unverified`.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `init_local::local_mode_is_recorded` |
| AC-2 | `init_local::second_run_keeps_local_mode` |
| AC-3 | `init_local::local_flag_refused_on_a_shared_config` |
| AC-4 | `core::config::repository_history_key` |
| AC-5 | `init_local::lines_go_to_the_git_folder` |
| AC-6 | `init_local::refuses_a_non_utf8_exclude_file` |
| AC-7 | `init_local::pre_push_in_a_tracked_hooks_folder` |
| AC-8 | `init_local::pre_push_refuses_process_files` |
| AC-9 | `init_local::pre_push_text_follows_the_mode` |
| AC-10 | `init::writes_commented_check_otherwise`, `init::writes_check_for_cargo_repositories` |
| AC-11 | `init_local::prints_the_checklist` |
| AC-12 | `export::export_key_and_refusals`, `export::key_must_be_a_branch_name` |
| AC-13 | `export::onto_defaults_to_the_upstream`, `export::default_upstream_is_exact` |
| AC-14 | `export::export_refusals` |
| AC-15 | `export::rewrites_the_range`, `export::export_is_repeatable`, `export::keeps_identity_and_encoding`, `export::copies_odd_commits_and_base_names` |
| AC-16 | `export::drops_empty_commits`, `export::drops_against_clean_parent_trees`, `export::drops_pull_merges` |
| AC-17 | `export::export_output`, `closed_pipe::no_print_macros_outside_out`, `closed_pipe::no_stream_aliases` |
| AC-18 | `init_local::foreign_repository_end_to_end`, `init_local::foreign_repository_real_pushes` |
| AC-19 | `export::export_leaves_the_checkout` |
| AC-20 | `next::local_mode_names_a_missing_pre_push`, `next::hook_fact_in_worktrees_and_foreign_hooks`, `hook_session::session_start_keeps_the_hook_fact` |
| AC-21 | `export::export_from_a_subfolder` |
| AC-22 | `init_local::init_from_a_worktree_fixes_the_main_hook` |

## Plan

Route: full (tags `interface`, `data`), auto. Every numbered step is a RED and GREEN pair whose named tests
fail at the RED (a test file that does not compile yet counts as failing); a step marked "coverage" is one
commit of a test that passes at once.

1. The starter junit template (AC-10; test `init::writes_commented_check_otherwise`): the RED changes the
   old test to expect the junit template, so it fails there. Must not change: the cargo `[check]`
   (`init::writes_check_for_cargo_repositories` passes unchanged).
2. The mode key (AC-4; test `core::config::repository_history_key`). Must not change: a `stapel.toml`
   without `[repository]`.
3. `init --local` (AC-1, AC-2, AC-3; tests `init_local::local_mode_is_recorded`,
   `init_local::second_run_keeps_local_mode`, `init_local::local_flag_refused_on_a_shared_config`): the
   option, the recorded mode and the refusal; the lines still go where shared mode puts them. Must not
   change: `init` without `--local`.
4. The lines in the git folder (AC-5, AC-6; tests `init_local::lines_go_to_the_git_folder`,
   `init_local::refuses_a_non_utf8_exclude_file`). Must not change: shared mode's `.gitignore` and
   `.gitattributes` lines.
5. The local pre-push (AC-7, AC-8; tests `init_local::pre_push_in_a_tracked_hooks_folder`,
   `init_local::pre_push_refuses_process_files`). Must not change: the shared pre-push text.
6. The pre-push text follows the mode (AC-9; test `init_local::pre_push_text_follows_the_mode`). Must not
   change: a foreign hook and the executable-bit repair.
7. The checklist (AC-11; test `init_local::prints_the_checklist`). Must not change: shared mode's output.
7b. The missing hook in `next` (AC-20; test `next::local_mode_names_a_missing_pre_push`). Do not run
   rustfmt on `tests/next.rs`. Must not change: `next` in shared mode.
8. `stapel export` with KEY and `--onto` given (AC-15, AC-16; tests `export::rewrites_the_range`,
   `export::export_is_repeatable`, `export::drops_empty_commits`): the rewrite and the ref.
9. `export` KEY and the default Onto ref (AC-12, AC-13; tests `export::export_key_and_refusals`,
   `export::onto_defaults_to_the_upstream`). Must not change: the rewrite of step 8.
10. `export` refusals and output (AC-14, AC-17; tests `export::export_refusals`, `export::export_output`).
    Must not change: steps 8 and 9.
11. Coverage, no GREEN (AC-18, AC-19; tests `init_local::foreign_repository_end_to_end`,
    `export::export_leaves_the_checkout`): the whole path on the fixture repository and the untouched
    checkout pass after step 10.

Fix steps after code review round 1 (they do not count toward the limit of 12):

12. Export from a subfolder (AC-21; test `export::export_from_a_subfolder`; ER1-1): every git call of
    export runs at `rev-parse --show-toplevel`. Must not change: the export from the root.
13. Raw commit objects (AC-15; test `export::keeps_identity_and_encoding`; R1-2, ER1-4): the new commit is
    the old object from `cat-file commit` with its `tree` and `parent` lines replaced and `gpgsig` and
    `mergetag` headers removed, written with `hash-object -t commit -w --stdin`. Must not change: the tips
    of `export::export_is_repeatable`.
14. Drops against Clean trees (AC-16; test `export::drops_against_clean_parent_trees`; R1-1, ER1-8,
    ER1-9). Must not change: `export::drops_empty_commits`.
15. Export key and upstream (AC-12, AC-13; tests `export::key_must_be_a_branch_name`,
    `export::default_upstream_is_exact`; R1-3, drift-2): `check-ref-format --branch`; the exact upstream.
15b. Export output (AC-17, AC-19; tests `closed_pipe::no_print_macros_outside_out`,
    `export::export_leaves_the_checkout`; CR1-0, ER1-5): output through `out`; the remote list read before
    `update-ref`. The RED adds a remote Onto ref to `export_leaves_the_checkout`, so it fails there.
16. The hook fact (AC-20; tests `next::hook_fact_in_worktrees_and_foreign_hooks`,
    `next::local_mode_names_a_missing_pre_push`; R1-5, R1-6, ER1-7): `--git-path` runs in the main worktree
    (first entry of `git worktree list --porcelain`). The RED makes the old test's foreign-hook case expect
    no fact. No rustfmt on `tests/next.rs`.
17. Pre-push history and the checklist (AC-8, AC-11; tests `init_local::pre_push_refuses_process_files`,
    `init_local::prints_the_checklist`; ER1-11, ER1-12): `--full-history`; the two `git add` lines; the
    unused `LOCAL_MARKER` removed. The RED adds a discarded side line to the push test and changes the
    golden checklist.
18. Coverage, no GREEN (AC-18; test `init_local::foreign_repository_real_pushes`; ER1-10): a new test
    runs the checklist's `git add` lines and real pushes to a bare remote; `foreign_repository_end_to_end`
    stays as it is, since a coverage commit only adds `#[test]` functions.

Fix steps after code review round 2 (ledger decision of 2026-11-12: round 3 instead of a split):

19. Session start keeps the hook fact (AC-20; test `hook_session::session_start_keeps_the_hook_fact`; ER2-2,
    CR2-1): `next::text_at` uses `lines_parts`, so the fact and the action are printed before a refusal.
20. Init from a worktree (AC-22; test `init_local::init_from_a_worktree_fixes_the_main_hook`; ER2-1):
    `plan_pre_push` resolves the path in the main worktree, as `stale_local_pre_push` does.
21. Stream aliases (AC-17; test `closed_pipe::no_stream_aliases`; CR2-4, ER2-3): the RED narrows the
    `stdout(`/`stderr(` patterns of the scan to `io::stdout(`/`io::stderr(` and adds the alias test; the
    GREEN replaces `capture()` with plain `.stdout(Stdio::piped())` calls.
22. Odd commits and base names (AC-15, AC-13; test `export::copies_odd_commits_and_base_names`; CR2-2,
    CR2-3, ER2-6): `hash-object --literally`; `--end-of-options` in `upstream_of`.
23. Pull merges (AC-16; test `export::drops_pull_merges`; ER2-4). Must not change: `export::drops_empty_commits`
    and `export::drops_against_clean_parent_trees`.

## Review

- Spec review, round 1 (2026-11-12): `stapel-spec-reviewer`; 11 findings, 1 HIGH, 6 MEDIUM, 4 LOW (one LOW
  outside the promise, SR1-8, kept out by the human); the HIGH (SR1-1) and the rest fixed in the text.
- Spec review, round 2 (2026-11-12): `stapel-spec-reviewer`; 13 findings, 0 HIGH, 5 MEDIUM, 8 LOW (one LOW
  outside, SR2-7, put to the human with this confirmation); all fixed in the text.
- Checks: after step 7b pass (923 tests); after step 11 the steps pass and the suite fails on
  `closed_pipe::no_print_macros_outside_out` (ledger decision of 2026-11-12: two checks, not one per step).
- Code review, round 1 (2026-11-12): `stapel-reviewer` (role reviewer), `stapel-reviewer` (role external),
  `stapel-drift` on the whole ticket; 28 findings with the orchestrator's CR1-0: 1 HIGH (ER1-1, export from a
  subfolder kept process files), 8 MEDIUM, 19 LOW; 25 patched by fix steps 12 to 18, 3 no-change (R1-4,
  R1-8, drift-6).
- Check after step 18: pass (939 tests).
- Code review, round 2 (2026-11-12): the same three reviewers; every round-1 patch confirmed except two
  halves; 14 findings, 0 HIGH, 2 MEDIUM (ER2-1, ER2-2, both on the hook fact), 12 LOW (two outside: ER2-4
  taken into AC-16 by the human, ER2-7 not promised); 12 patched by fix steps 19 to 23, 2 no-change (CR2-5,
  ER2-7).

## Summary

TODO
