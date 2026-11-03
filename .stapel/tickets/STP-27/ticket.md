# STP-27 — Parallel builds of independent tickets

## Description

Source: `docs/PHASES.md`, plan item 1.13; ledger decision 2026-11-02; the research in the chat of
2026-10-30 and 2026-11-02.

### Frame

**Problem.** One ticket builds at a time: two open tickets break commands without KEY, the build permit
and the keys; parallel cargo runs crash the machine (STP-4); independence is left to the person.

**Plan item:** 1.13.

**Risk tags:** `guard` (permit per ticket, cargo and git rules), `data` (keys, stops, merges),
`interface` (commands, `next` actions, hooks, process text).

**Non-goals.** One orchestrator driving two tickets; push and MR (1.7); the index (phase 4); a list of
shared files; deliberate bypass of the cargo lock.

**Promise (closed list).**

1. `stapel new` takes a key unused in any worktree or branch and makes a branch and a worktree for it.
2. Without KEY, commands and hooks take the current branch's ticket; a foreign KEY is refused.
3. The build permit covers only that ticket.
4. One machine-wide cargo lock that waits with no limit; agents run cargo only through `stapel cargo`.
5. Subagents run only allowed `git` commands, inside their worktree.
6. `stapel init` writes `merge=union` for the ledger and the session journal.
7. A trial merge after each commit; a conflict stops the later ticket (spec confirmed later).
8. A dependency agent compares both tickets before the build; a confirmed dependency (kind `spec` or
   `merge`) stops the later ticket; triage can refute it.
9. A stopped ticket waits; when the base branch holds another ticket's close, it merges the base branch
   (the orchestrator resolves conflicts), re-runs `stapel check` and gets a combined review before close.
10. After close the orchestrator merges the branch into main locally and removes the worktree.

Not promised: three or more builds at once; a dependency no conflict, agent, test or review shows.

**Size.** About 1100 source lines and 1700 test lines; split past 1500 (the human, 2026-11-03). Route:
full (tags), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-02: Decision (scope, the human): two builds at once, one ticket per session (option b of the
  research of 2026-10-30). Rejected: specs ahead with serial builds; one orchestrator driving two tickets.
- 2026-11-02: Decision (one ticket, the human): the whole item 1.13 is one ticket, taken before 1.6
  (ledger 2026-11-02). Rejected: a foundation ticket and an independence ticket.
- 2026-11-02: Decision (independence, the human): stapel decides it, not the person, at three levels:
  conflicts by a trial merge, meaning by a dependency agent, and a re-run of `stapel check` after the
  earlier ticket merges; the index blast radius joins in phase 4. Rejected: a `Runs beside` line in the
  Frame; the person's judgement.
- 2026-11-02: Decision (conflicts, the human, option 4): after each commit of either ticket, `git
  merge-tree` of both branches; a conflict stops the later ticket. Rejected: a list of shared files in
  `stapel.toml` (it goes stale as the project grows); a stop on any file both tickets change (16 of 19
  tickets changed `crates/stapel-cli/src/main.rs`, so almost every pair would stop).
- 2026-11-02: Decision (order, the human, recommended): the later ticket is the one whose spec was
  confirmed later; it stops on a dependency or a conflict. Rejected: whoever closes first.
- 2026-11-02: Decision (false dependency, the human, recommended): the agent's dependency finding goes to
  `findings.jsonl`; the orchestrator triages it; `false` with a refutation lifts the stop. Rejected: only
  the person lifts it.
- 2026-11-02: Decision (dependency agent timing, the human, recommended): it runs at the later ticket's
  `stapel ok` and again at each new spec confirmation of either ticket. Rejected: once only; after each
  commit.
- 2026-11-02: Decision (stopped ticket, the human, option 3): `stapel next` prints a wait line with the
  earlier ticket and the reason; when the earlier ticket merges into main, its session prints which
  waiting tickets can go on; the person says so in that window. Rejected: a silent wait; a background
  wait that goes on alone.
- 2026-11-02: Decision (cargo, the human, recommended): one machine-wide cargo lock; builder runs and
  `stapel check` wait with no limit and print the holder's ticket, pid and start time; the lock is an OS
  file lock, freed when its holder dies; the guard denies cargo outside the wrapper. Rejected: two slots
  with `-j 2`; a 30-minute limit.
- 2026-11-02: Decision (worktree, the human, recommended): `stapel new` creates the branch `STP-<n>` from
  local main and the worktree `.claude/worktrees/STP-<n>`; the orchestrator enters it with `EnterWorktree`;
  the person does not leave the agent or change folders. Rejected: a sibling folder; `EnterWorktree` alone
  (it bases the branch on `origin/main`).
- 2026-11-02: Decision (always a branch, the human, recommended): every ticket has its branch and
  worktree, also when it is the only one. Rejected: the only ticket on main; a choice in `stapel.toml`.
- 2026-11-02: Decision (merge, the human, recommended): after `stapel close` the orchestrator merges the
  branch into main locally with `git merge --no-ff`, with no push; then it leaves the worktree, removes it
  and deletes the branch with `git branch -d`. Rejected: the person merges; `stapel close` merges.
- 2026-11-02: Decision (prior art, the human): four ideas of klc join the promise: a combined review
  after the merge, a git guard for subagents, a check that KEY is the branch's ticket, and dependency
  kinds. Source: klc's retrospectives of KLC-078, KLC-079, wave 1, KLC-129, KLC-140, `git_guard.py`.
  Rejected: dependencies declared by a person (klc `blocked_by`); a shared checkout for parallel builds.
- 2026-11-03: Decision (size, the human, recommended): one ticket of about 1100 source lines; split past
  1500. Rejected: split past 1200; two tickets now.
- 2026-11-03: Decision (spec review SR1-18, the human, recommended): subagents get an allow list of git
  words instead of a deny list. Rejected: the deny list of seven words.
- 2026-11-03: Decision (spec review SR1-19, the human, recommended): keys also count remote-tracking
  branches. Rejected: local branches only.
- 2026-11-03: Decision (spec review SR1-20, the human, recommended): no trial merge before the later
  ticket's spec is confirmed; before it no code is written. Rejected: a trial merge at the first `ok`.
- 2026-11-02: Assumption (bootstrap): STP-27 itself is built on main, as before; the hooks run the
  installed `stapel` binary, which is not reinstalled during the build, so builders run bare cargo and the
  new rules start with the next ticket, after `cargo install` at the close (SR1-9).
- 2026-11-02: Assumption (git): `git merge-tree --write-tree` needs git 2.38; this machine has 2.43.

## Spec

Terms:

- **Ticket branch**: a local branch whose name is a ticket key of `stapel.toml` (`STP-<n>`), compared
  without case.
- **Base branch**: the branch checked out where `stapel new` ran, stored as `base_branch` in the ticket's
  `state.json`; it is not a ticket branch. The **main checkout** is the first entry of `git worktree list`.
- **Branch ticket**: the ticket of the current checkout, open or closed, whose key the current branch
  names; none on a base branch, a detached HEAD or an unborn HEAD.
- **Ticket worktree**: the linked worktree `.claude/worktrees/<KEY>` under the main checkout, of the
  ticket branch `<KEY>`.
- **Hook root**: for every stapel hook, the stapel root of the input's `cwd`, else of
  `CLAUDE_PROJECT_DIR`, else of the working directory. A path is **inside the worktree** of a hook when it
  is under the hook root and not under the hook root's `.claude/worktrees/`.
- **Named files** of a ticket: the words in backticks in its Plan steps that hold a `/` or a file extension.
- **Parallel ticket**: the branch ticket of another worktree that `git worktree list --porcelain` lists, not
  prunable, read from its folder on disk, while it is open or closed but not yet **closed on the base
  branch**: the base branch's `.stapel/tickets/<KEY>/state.json` holds `closed`.
- **Spec time**: the `at` of the first confirmation of the section `spec` in a ticket's `state.json`.
- **Order**: of two tickets with a spec time, the one with the later spec time is the **later** ticket; at
  the same second, the one with the higher key number; a ticket without a spec time takes no part.
- **Stop**: the file `<git common dir>/stapel/stops/<LATER>/<EARLIER>-<reason>.json`, one per pair and
  reason (`conflict` or `dependency`), with `kind` (`merge` or `spec`; a conflict is `merge`; a dependency takes its record's
  `waits`), `files` and `at`; it is written to a temporary file and renamed; a file that cannot be read as
  a stop counts as a `merge` stop of the pair its name gives (`unreadable stop file`). Each stop blocks on
  its own; AC-16, AC-18 and AC-22 remove them.
- **Cargo lock**: an OS file lock on `$HOME/.cache/stapel/cargo.lock`; its file holds the **holder line**
  `<branch ticket or ->, pid <pid>, since <RFC 3339 time>, <command>`, written to a temporary file and
  renamed after the lock is taken; before that a waiter shows `an unknown holder`.
- **Held mark**: the variable `STAPEL_CARGO_LOCK_HELD=<pid>` that the lock holder sets for its children.
- **Spec hash**: the hash of the section `spec` in a ticket's last confirmation of it.
- **Dependency round**: `<EARLIER>:<first 8 hex of the earlier spec hash>:<first 8 hex of the later spec
  hash>`; it is **done** when `findings.jsonl` holds a record of stage `dependency` with that round.
- **Close on the base branch the ticket lacks**: a ticket closed on the base branch whose `state.json` on
  HEAD is absent or has no `closed`.
- **Merge round**: `merge:<first 8 hex of the merge commit>` for the latest commit of HEAD that merged the
  base branch and brought a close; its **merge diff** is `git diff <merge>^1 <merge>`.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel init` runs THE `.gitignore` holds `/.claude/worktrees/` and `.gitattributes` holds `.stapel/process-ledger.jsonl merge=union` and `.stapel/sessions.jsonl merge=union`; a second run adds none. | `init::worktrees_ignored_and_journals_union` |
| AC-2 | WHEN `stapel new` takes a key THE number is one above the largest key number in the local tickets folder, in the tickets folder of each listed worktree that is not prunable, in `.stapel/tickets/` of each local and remote-tracking branch, and in each local or remote-tracking ticket branch name. | `new_branch::key_above_every_worktree_and_branch` |
| AC-3 | WHEN `stapel new` runs on a base branch with a commit and a clean tree THE command creates the branch `<KEY>` at HEAD and the worktree `.claude/worktrees/<KEY>` under the main checkout, writes the ticket there with `base` set to HEAD and `base_branch` to the current branch, and prints `created: <KEY>`, `branch: <KEY> from <base branch> at <short hash>` and `worktree: .claude/worktrees/<KEY>`. | `new_branch::new_creates_branch_and_worktree` |
| AC-4 | WHEN two `stapel new` start at once in the main checkout THE keys differ and both branches and worktrees exist. | `new_branch::two_new_at_once` |
| AC-5 | IF HEAD has no commit THEN `stapel new` writes the ticket in the current checkout as before and prints `note: no commit yet; the ticket stays in this checkout`; IF the current branch is a ticket branch THEN it refuses with `stapel new runs on the base branch, not on the ticket branch <branch>` and exit 2; IF the tree has changes outside machine files THEN it refuses with `commit the changes of <paths> first; the new worktree starts at HEAD` and exit 2. | `new_branch::unborn_head_and_ticket_branch` |
| AC-6 | WHEN a command that takes an optional KEY (`status`, `check`, `close`, `next`, `ok`, `findings`, `ledger add`, `tokens import-records`, `review start`, `depends`) or a token hook runs without KEY on a ticket branch THE ticket is the branch ticket; elsewhere the rule of one open ticket stays. | `new_branch::branch_names_the_ticket` |
| AC-7 | IF `check`, `ok` or `close` gets a KEY other than the branch ticket while a branch ticket exists THEN it refuses with `<KEY> is not the ticket of branch <branch>; run it in .claude/worktrees/<KEY>` and exit 2. | `new_branch::key_must_match_the_branch` |
| AC-8 | WHEN any stapel hook (the guard, post-tool-use, session-start, the token hooks) gets an input whose `cwd` is inside a stapel repository THE hook root is that `cwd`'s root, before `CLAUDE_PROJECT_DIR`. | `hook_session::root_from_input_cwd`, `hook_tokens::root_from_input_cwd` |
| AC-9 | WHEN the guard decides a code write in a checkout on a ticket branch THE build permit comes only from the branch ticket; on any other branch the rule of STP-18 stays. | `hook::permit_only_for_the_branch_ticket` |
| AC-10 | WHEN `stapel cargo <args>` runs without the held mark THE command takes the cargo lock, waiting with no time limit, writes its holder line, runs `cargo <args>` with the same stdio and the held mark, and exits with cargo's code, or 128 plus the signal number when cargo dies by a signal; IF `HOME` is empty or unset THEN it refuses with `no folder for the cargo lock: HOME is not set` and exit 1. | `cargo_lock::wrapper_runs_cargo` |
| AC-11 | WHILE another process holds the cargo lock THE waiting `stapel cargo` or `stapel check` prints once on stderr `waiting for the cargo lock: held by <holder line or an unknown holder>` and goes on when the lock is free; IF the holder is killed THEN the lock is free once its cargo child ends, for `stapel cargo` and for `stapel check`. | `cargo_lock::second_waits_and_names_the_holder`, `cargo_lock::killed_holder_frees_the_lock` |
| AC-12 | WHEN `stapel check` runs without the held mark THE check takes the cargo lock before its first cargo run and passes the held mark to cargo; with the held mark, `stapel check` and `stapel cargo` take no lock. | `cargo_lock::check_takes_the_lock`, `cargo_lock::held_mark_takes_no_lock` |
| AC-13 | IF an agent's Bash command runs `cargo` as a program, also behind a wrapper, `sh -c` or `eval`, or sets `STAPEL_CARGO_LOCK_HELD` THEN the guard denies it with ``cargo runs only through `stapel cargo`, which waits for the machine-wide cargo lock``; `stapel cargo …` passes. | `hook::bare_cargo_denied` |
| AC-14 | IF a subagent's Bash command (input with `agent_id`) runs `git` with a word outside `status`, `diff`, `log`, `show`, `add`, `commit`, `rev-parse`, `ls-files`, `grep`, `blame`, `merge-base`, `cat-file`, or with `-c`, `--git-dir`, `--work-tree`, `--output`, or `-C` to a path not inside its worktree, or sets a `GIT_*` variable, or runs `cd` to a path not inside its worktree THEN the guard denies it with `subagents run only these git commands in their own worktree: <list>`; the main session is not denied. | `hook::subagent_git_guard` |
| AC-15 | WHEN a commit command runs on a ticket branch whose ticket and a parallel ticket both have a spec time THE post-tool-use hook runs `git merge-tree --write-tree` of both branches; IF it reports a conflict THEN it writes the stop of the later ticket and prints `stapel: <LATER> conflicts with <EARLIER> in <files>; <LATER> waits for <EARLIER>`. | `parallel::conflict_stops_the_later` |
| AC-16 | WHEN `stapel next` runs for a ticket with a parallel ticket THE same trial merge runs, so a commit made outside the hook also gives the stop; a clean trial merge writes no stop and removes the pair's conflict stop; IF git exits other than 0 or 1 THEN `next` prints the fact `trial merge not run: <git's first stderr line>` and changes no stop. | `parallel::next_runs_the_trial_merge` |
| AC-17 | WHEN a ticket has a stop of kind `merge` and its earlier ticket is not closed on the base branch, or a stop of kind `spec` and the earlier ticket's spec confirmation is stale THE `stapel next` prints `blocked: <KEY> waits for <EARLIER> (<reason>)` and `next: wait — <EARLIER>`, and the guard denies its code writes; `always_writable` paths stay open; a `spec` stop with a fresh earlier confirmation does not block. | `parallel::stopped_ticket_waits`, `parallel::spec_kind_waits_for_the_spec` |
| AC-18 | WHEN the base branch holds a close the ticket lacks THE `stapel next` prints `next: merge-main — git merge <base branch>` before code review and close; IF so THEN `close` refuses with `close needs <base branch> merged: run git merge <base branch> and stapel check <KEY> first`; other base-branch commits ask for nothing. A stop whose earlier ticket's close HEAD holds is removed. | `parallel::merge_main_before_close` |
| AC-19 | WHEN HEAD has a merge round, the check passes and code review rounds are done as before THE `stapel next` names `code-review — stapel review start <KEY> --round merge` until a finding import of that merge round exists; a code review done before the merge stays valid; its pack holds the ticket's diff against the base branch and the merge diff; `close` refuses until then; a later merge round asks again. The order is merge-main, run-check, code review, merge review, close. | `depends::merge_review_after_merge_main` |
| AC-20 | WHILE the later ticket's current dependency round is not done THE `stapel next` names `dependency-review — stapel depends <KEY>` and the guard denies its code writes; `stapel depends <KEY>` writes a pack with the Frame, criteria, Plan and named files of both tickets and the earlier ticket's diff against the base branch, and prints its path and the round; IF no earlier parallel ticket exists THEN it refuses with `<KEY> has no earlier parallel ticket` and exit 2. | `depends::dependency_review_named`, `depends::pack_holds_both_tickets` |
| AC-21 | WHEN `findings import` gets the stage `dependency` THE record needs the role `depends`, the round of the current dependency round and, for HIGH or MEDIUM, `waits` of `spec` or `merge`; a review that finds no dependency returns one LOW record with the claim `no dependency found`; IF `waits` is on another stage or has another value THEN the import refuses with exit 2. | `records::dependency_stage` |
| AC-22 | WHEN a dependency round is done THE later ticket's dependency stop of that pair holds the HIGH or MEDIUM records of the round whose verdict is not `false`, with the kind of their `waits` (`merge` when both kinds are present); with none, the dependency stop is removed; a new spec confirmation of either ticket makes a new round. | `depends::confirmed_dependency_stops`, `depends::false_verdict_lifts_the_stop` |
| AC-23 | WHEN the branch ticket is closed and HEAD holds the close THE `stapel next` prints `next: merge-branch — git merge --no-ff <KEY> (in <main checkout path>)` until the base branch holds it; while the close is not committed it prints `next: commit-close — git commit .stapel/tickets/<KEY>`; IF the main checkout is not on the base branch or has changes outside machine files THEN the fact `main checkout not ready: <reason>` comes first. | `parallel::merge_branch_after_close` |
| AC-24 | WHEN `stapel next` runs in a checkout without a branch ticket and a listed worktree's ticket has a stop whose earlier ticket is closed on this branch THE first lines are `can go on: <LATER> (<EARLIER> is merged into <branch>)`. | `parallel::can_go_on_notice` |
| AC-25 | WHEN `stapel init` installs the process THE agent `.claude/agents/stapel-depends.md` (read-only tools) and the `[roles]` row `stapel-depends = "depends"` are written, and the skill names worktrees, `stapel cargo`, the waits and the merge into main. | `init_process::depends_agent_installed`, `init_process::skill_text_names_parallel_builds` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-25; outside ticket branches, commands without KEY
  behave as before; `stapel new` on a base branch with a commit makes a branch and a worktree (AC-3);
  agents run cargo through `stapel cargo`; a ticket branch whose ticket folder is missing has no branch
  ticket and keeps the old rules.
- **Not promised:** the Frame's list; a dependency that neither a conflict, the agent, the re-run check nor
  the merge review shows; Windows `cargo.exe` on the WSL `PATH`; cargo run by a script the guard does not
  read; a worktree folder deleted by hand (git lists it as prunable and stapel skips it); a base branch
  rewritten after a ticket branched from it; three or more parallel tickets (pairs are checked and the
  order holds, but it is not tested); a trial merge before the later ticket's spec is confirmed (SR1-20);
  a fair order among waiters for the cargo lock (any waiter may take it next).

## Test plan

CLI tests. New files: `crates/stapel-cli/tests/new_branch.rs`, `parallel.rs`, `depends.rs` and
`cargo_lock.rs`; a new helper `common::two_ticket_worktrees` makes one repository with a commit, two
ticket worktrees and their spec confirmations, added in the RED of its first step. Cargo-lock tests point
`HOME` at a temporary folder and run a stand-in `cargo` script first on `PATH`, so no real
build runs; the holder is a child process the test starts and kills. Light tests: `stapel cargo test -j 4
-p stapel-cli --test <file> -- --test-threads=3`; before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Init files | empty repository: both files written; a second run: no change; an existing `.gitattributes` with other lines: lines appended once | `init::worktrees_ignored_and_journals_union` |
| Keys | local `STP-1`; a worktree with `STP-4` on disk only; a branch whose tree has `STP-6`; a branch `STP-9` with no folder; a remote-tracking branch `origin/STP-11`; a prunable worktree whose folder held `STP-14` and whose branch `STP-12` still exists: next key `STP-13` | `new_branch::key_above_every_worktree_and_branch` |
| New | on `main` with a commit: branch, worktree, files only in the worktree, `base` and `base_branch` in `state.json`, the three lines; on a base branch named `master`: the same with `master` | `new_branch::new_creates_branch_and_worktree` |
| Race | two `stapel new` started together in the main checkout: two keys, two branches, two worktrees | `new_branch::two_new_at_once` |
| Unborn, ticket branch | no commit: old behaviour and the note; run inside a ticket worktree: the refusal, exit 2, no branch made; an uncommitted change of `docs/a.md`: the dirty refusal, exit 2; an uncommitted `state.json` only: allowed | `new_branch::unborn_head_and_ticket_branch` |
| Branch ticket | two open tickets in one checkout, branch `STP-2`: `status`, `next`, `ok` and `review start` without KEY take `STP-2`; branch `stp-2`: the same; `STP-2` closed: `next` still takes it; branch `STP-7` with no folder: the old rule; on `main`: refused as before | `new_branch::branch_names_the_ticket` |
| Wrong KEY | in the `STP-2` worktree, `check STP-1`, `ok STP-1`, `close STP-1`: the refusal, exit 2; on `main`: no refusal | `new_branch::key_must_match_the_branch` |
| Hook root | `CLAUDE_PROJECT_DIR` at main and input `cwd` at a ticket worktree: the session-start lines and the token records are the worktree's | `hook_session::root_from_input_cwd`, `hook_tokens::root_from_input_cwd` |
| Permit | a worktree on branch `STP-2` whose ticket is not confirmed, while `STP-1` in the same checkout is: a write in `crates/` denied; on `main`: allowed by `STP-1` | `hook::permit_only_for_the_branch_ticket` |
| Wrapper | stand-in `cargo` that prints its arguments and exits 3: same output, exit 3, holder line with the branch ticket written while it runs, held mark in its environment; stand-in killed by SIGTERM: exit 143; `HOME` empty: the AC-10 refusal, exit 1, cargo not run; `HOME` a read-only folder: refused, exit 1 | `cargo_lock::wrapper_runs_cargo` |
| Lock wait | a held lock: the second `stapel cargo` prints the wait line once, does not run cargo until the release, then runs once; the same for `stapel check`; a lock held with an empty lock file: `held by an unknown holder` | `cargo_lock::second_waits_and_names_the_holder` |
| Killed holder | a `stapel cargo` holder and a `stapel check` holder are each killed with SIGKILL while their cargo child sleeps: the waiter goes on only after the child ends | `cargo_lock::killed_holder_frees_the_lock` |
| Check lock | `stapel check` on a fixture: the holder line names the ticket and `stapel check` while its stand-in cargo runs, and the stand-in sees the held mark | `cargo_lock::check_takes_the_lock` |
| Held mark | the lock held and the held mark set: `stapel cargo` and `stapel check` run at once | `cargo_lock::held_mark_takes_no_lock` |
| Bare cargo | `cargo test`; `env FOO=1 cargo test`; `sh -c "cargo build"`; `eval cargo fmt`; `STAPEL_CARGO_LOCK_HELD=1 stapel cargo test`: denied with the AC-13 text; `stapel cargo test -p x`: allowed; `echo cargo`: allowed | `hook::bare_cargo_denied` |
| Git guard | input with `agent_id`: `git checkout main`, `git switch -c x`, `git stash`, `git restore .`, `git branch -D x`, `git worktree prune`, `git merge main`, `git -c core.worktree=/x status`, `git --git-dir=/x log`, `git -C /other status`, `git -C ../.. status`, `git diff --output=/tmp/x`, a `GIT_DIR` assignment before `git commit`, `cd ../.. && git commit -am x`, `cd .claude/worktrees/STP-1 && git status` from the main root: denied; `git -C crates status`, `cd crates && git status`, `git status`, `git diff`, `git add -A`, `git commit -m x`: allowed; the same denied commands without `agent_id`: allowed | `hook::subagent_git_guard` |
| Conflict | both tickets change the same line of one file and commit: the stop of the later one and the line; a change in two places of `main.rs`: no stop; both append a line to `.stapel/process-ledger.jsonl` (union): no stop; both change one line of `Cargo.lock`: stop; `GIT_DIR` and `GIT_INDEX_FILE` set in the hook's environment: the same results; a ticket without spec time: no trial merge | `parallel::conflict_stops_the_later` |
| Next merge | the conflicting commit made with `git` outside the hook: `stapel next` of either ticket writes the stop; a clean pair: no stop file; the conflict undone by a new commit: the conflict stop removed; a stand-in `git` that exits 128 on `merge-tree`: the fact `trial merge not run: …`, no stop changed | `parallel::next_runs_the_trial_merge` |
| Order | spec times 10:00 and 11:00: the 11:00 ticket is later; the same second: the higher key | `parallel::conflict_stops_the_later` |
| Wait | a `merge` stop: the fact, the action, a denied write in `crates/`, an allowed write in `.stapel/`; a stop file `{` (not JSON) and one of 5 KiB: the same wait with `unreadable stop file` and the earlier key from the file name; a conflict stop and a `spec` dependency stop of one pair with the earlier spec fresh: still waits on the conflict; a `spec` stop written by the test, with the earlier spec fresh: no wait; stale: wait | `parallel::stopped_ticket_waits`, `parallel::spec_kind_waits_for_the_spec` |
| Merge main | main gains the earlier ticket's close: `merge-main`; `close` refused with the AC-18 text; after `git merge main`: `run-check`, the stop file gone; main gains only a `docs/` commit: no `merge-main` | `parallel::merge_main_before_close` |
| Merge review | after the merge of main, a passing check and code review rounds done: `code-review` with `--round merge`; the pack holds the ticket diff and the merge diff; after an import of that merge round: the close is allowed; a second merge of main with a third ticket's close: `code-review` again | `depends::merge_review_after_merge_main` |
| Dependency review | the later ticket with no record of the round: the action and a denied write in `crates/`; `stapel depends` without KEY in the later ticket's worktree: the branch ticket; the pack holds both Frames, criteria, Plans, the named files of both Plans and the earlier diff; the round printed; a ticket with no earlier parallel ticket: the AC-20 refusal, exit 2 | `depends::dependency_review_named`, `depends::pack_holds_both_tickets` |
| Dependency stage | stage `dependency`, role `depends`, HIGH with `waits: merge`: accepted; LOW `no dependency found` without `waits`: accepted, the round done; HIGH without `waits`: refused; `waits: later`: refused; `waits` on `code-review`: refused; another round: refused | `records::dependency_stage` |
| Dependency stop | a MEDIUM record with `waits: spec`, verdict `medium`: a stop of kind `spec`; with `waits: merge`: kind `merge`; both: kind `merge`; triaged `false` with a refutation: the stop removed; a new round with only `no dependency found`: the stop removed; the earlier spec confirmed again: a new round, `dependency-review` again | `depends::confirmed_dependency_stops`, `depends::false_verdict_lifts_the_stop` |
| Merge branch | a closed ticket in its worktree with the close uncommitted: `commit-close`; committed, branch not in main: `merge-branch` with the main checkout path; the main checkout on another branch or with a changed `src/a.rs`: the fact first | `parallel::merge_branch_after_close` |
| Notice | on `main` after the earlier ticket's branch merged, a worktree ticket with a stop: the notice line first, then `new-ticket` | `parallel::can_go_on_notice` |
| Install | `stapel init`: the agent file with read-only tools, the `[roles]` row, the skill sentences; this repository's files the same | `init_process::depends_agent_installed`, `init_process::skill_text_names_parallel_builds` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| current branch | git ref name | any | detached or unborn: no branch ticket | a name that is no key: base branch | key compared without case | `new_branch::branch_names_the_ticket` |
| KEY of `check`, `ok`, `close` | ticket key | as STP-2 | absent: the branch ticket | another ticket: AC-7 refusal | exact folder name | `new_branch::key_must_match_the_branch` |
| `stapel cargo` arguments | words | none / any | none: `cargo` alone | — | passed as given | `cargo_lock::wrapper_runs_cargo` |
| `stapel depends` KEY | ticket key | as STP-2 | absent: the branch ticket | no earlier parallel ticket: refused, exit 2 | exact | `depends::dependency_review_named` |
| `review start --round` | number or `merge` | 1 / 2 or `merge` | as before | other words: refused as before | exact | `depends::merge_review_after_merge_main` |
| `waits` | string | `spec` or `merge` | HIGH or MEDIUM without it: refused | other value: refused | exact | `records::dependency_stage` |
| `HOME` | path | any | empty or unset: AC-10 refusal, exit 1 | unwritable folder: refused with the OS error, exit 1 | as given | `cargo_lock::wrapper_runs_cargo` |
| `STAPEL_CARGO_LOCK_HELD` | pid | any | empty: absent | any value: no lock taken | — | `cargo_lock::held_mark_takes_no_lock` |
| hook input `agent_id`, `cwd` | string, path | any | absent `agent_id`: main session; absent `cwd`: old root order | `cwd` outside a stapel repository: the next source | as given | `hook::subagent_git_guard`, `hook_session::root_from_input_cwd` |
| stop file | JSON | 4 KiB | — | not JSON or too large: a `merge` stop with `unreadable stop file` | — | `parallel::stopped_ticket_waits` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | linked worktrees, a folder that is gone | prunable entries skipped; their branch names still count for keys | `new_branch::key_above_every_worktree_and_branch` |
| git | detached HEAD, unborn HEAD | no branch ticket; `new` keeps the old behaviour on an unborn HEAD | `new_branch::unborn_head_and_ticket_branch` |
| git | history shape: merge commits | merging main keeps the RED and GREEN runs; a merge commit is no step | `parallel::merge_main_before_close` |
| git | history shape: rebase, squash, amend | not promised; a rebase drops the runs and `stapel check` runs again | — |
| git | `merge-tree --write-tree` (git 2.38 or later) | exit 1 is a conflict; another exit or an older git: no stop changed, a fact `trial merge not run: <reason>` | `parallel::next_runs_the_trial_merge` |
| git | base branch name | any name, read from the checkout where `new` ran (`main`, `master`) | `new_branch::new_creates_branch_and_worktree` |
| git | dirty tree in main from the nested worktree | `/.claude/worktrees/` ignored by AC-1 | `init::worktrees_ignored_and_journals_union` |
| git | dirty tree where `stapel new` runs; dirty main checkout at the merge | `new` refuses (AC-5); `next` names the main checkout not ready (AC-23) | `new_branch::unborn_head_and_ticket_branch`, `parallel::merge_branch_after_close` |
| git | configuration: `GIT_*` environment, global config | the hook's git calls run with `GIT_DIR`, `GIT_WORK_TREE` and `GIT_INDEX_FILE` removed | `parallel::conflict_stops_the_later` |
| git | attributes: `merge=union` in the trial merge | the union files give no conflict | `parallel::conflict_stops_the_later` |
| git | remote-tracking branches | their ticket names count for keys | `new_branch::key_above_every_worktree_and_branch` |
| git | shallow clone, submodules, SHA-256, odd path bytes | not promised | — |
| cargo | the real cargo behind `stapel cargo` | run as a child with inherited stdio and the environment plus the held mark | `cargo_lock::wrapper_runs_cargo` |
| cargo | other dimensions | as `stapel check` before | — |
| claude-code | `CLAUDE_PROJECT_DIR` after `EnterWorktree` | stays the launch folder (docs); the input `cwd` comes first | `hook_session::root_from_input_cwd` |
| claude-code | `agent_id` in hook input | present inside a subagent (docs); absent: the main session | `hook::subagent_git_guard` |
| claude-code | Bash call past its timeout | moved to the background (docs); the waiter keeps the lock queue | — |
| claude-code | SessionStart after entering a worktree | does not fire (docs); the skill says to run `stapel next` after `EnterWorktree` | `init_process::skill_text_names_parallel_builds` |
| claude-code | settings read from the launch folder or the worktree | both hold the same tracked `.claude/settings.json`; not promised beyond that | — |
| os | file locks | `File::lock` on a local file system; a `HOME` on `/mnt/c` not promised | `cargo_lock::second_waits_and_names_the_holder` |
| os | two processes writing one stop file | temporary file and rename; the last rename wins, both write the same pair | `parallel::conflict_stops_the_later` |
| os | a killed holder, pid reuse | `stapel cargo` and `stapel check` pass the lock fd to cargo, so the lock lives until the child ends; the holder line is read only while the lock is held | `cargo_lock::killed_holder_frees_the_lock` |
| os | WSL: Windows binaries on `PATH` | not promised | — |
| os | clock skew | spec times from one clock; not promised across machines | — |

### Abuse

| Attempt of a careless agent | Outcome | Test |
|---|---|---|
| runs `cargo test` directly, behind `env` or in `sh -c` | denied (AC-13) | `hook::bare_cargo_denied` |
| sets `STAPEL_CARGO_LOCK_HELD` to skip the queue | denied (AC-13) | `hook::bare_cargo_denied` |
| a builder runs `git checkout main`, `git stash` or `git restore .` in its worktree | denied for subagents (AC-14) | `hook::subagent_git_guard` |
| a builder runs `git -C ../.. status` or `git -c core.worktree=…` | denied (AC-14) | `hook::subagent_git_guard` |
| builds before the dependency review | code writes denied (AC-20) | `depends::dependency_review_named` |
| writes code in a stopped ticket | denied by the guard (AC-17) | `parallel::stopped_ticket_waits` |
| runs `stapel check STP-1` from the `STP-2` worktree | refused (AC-7) | `new_branch::key_must_match_the_branch` |
| closes a ticket without merging main | refused (AC-18) | `parallel::merge_main_before_close` |
| skips the merge review | `close` refuses (AC-19) | `depends::merge_review_after_merge_main` |
| deletes the stop file | the next trial merge or dependency round writes it again; deliberate removal of a dependency record is not promised | `parallel::next_runs_the_trial_merge` |
| writes a half stop file | counts as a stop (fail closed) | `parallel::stopped_ticket_waits` |

**Review Focus:** the nested `stapel check` in this repository's own tests under the held mark (a
deadlock if the mark is lost through `env_clear`); `stapel new` fixtures that commit before `new` and now
get a worktree; a merge of main that brings `Cargo.lock` and how `stapel check` treats it as a build input;
a stop written by the earlier ticket's hook into the shared git folder while the later session reads it.

**Author self-check:** 2026-11-03, before spec review, again after rounds 1 and 2: promise items 1 to 10
map to AC-2 to AC-5 (1), AC-6 to AC-8 (2), AC-9 (3), AC-10 to AC-13 (4), AC-14 (5), AC-1 (6), AC-15 and
AC-16 (7), AC-20 to AC-22 and AC-17 (8), AC-17 to AC-19 (9), AC-23 to AC-25 (10); each Inputs and External
states row has a test or "not promised"; readers of the changed configuration (`[roles]` with
`stapel-depends`, the finding stage `dependency`) are `config.rs`, `records.rs`, `triage.rs`, `next.rs`, the
token hooks and `init`.

## Design

### Facts

- F-1: `tickets::next_key` (`crates/stapel-core/src/tickets.rs:24`) takes the largest local folder number
  plus one; `new::run` (`crates/stapel-cli/src/new.rs:7`) writes in the current checkout; no lock.
- F-2: `tickets::resolve` (`tickets.rs:86`) takes the one open ticket without KEY; its callers are
  `status`, `check`, `close`, `next`, `confirm`, `dialog` and `records`; the token hooks keep their own
  rule (`crates/stapel-cli/src/tokens.rs` `hook_records`, `gap_record`).
- F-3: each worktree checks out its own `.stapel/tickets/`; machine files (`state.json`, findings) may be
  uncommitted (`worktree.rs` `is_machine_file`), so a parallel ticket is read from its folder on disk.
- F-4: `stage::build_permit` (`crates/stapel-core/src/stage.rs:170`) returns the first open ticket with
  fresh `build.requires`; the guard calls it with the root of the written path (`guard.rs:186`), which is
  the worktree's root.
- F-5: `check.lock` and `check-target` live in `<git common dir>/stapel/` and are shared by worktrees;
  `run_cargo` (`crates/stapel-core/src/runner.rs:308`) clears the environment and passes `ENV_ALLOW`.
- F-6: the guard reads Bash programs behind `WRAPPERS`, `sh -c` and `eval` (`guard.rs:571`
  `check_program`).
- F-7: the post-tool-use hook runs for a commit command (`hook.rs` `post_tool_use`) and returns
  `additionalContext`.
- F-8: finding records have the stages of `records.rs:16` and the roles of `records.rs:17`; a `false`
  verdict does not block (`triage.rs`).
- F-9: `token_root` and the session-start hook prefer `CLAUDE_PROJECT_DIR` (`hook.rs:52`, `hook.rs:147`);
  Claude Code keeps it at the launch folder after `EnterWorktree` and gives the worktree as `cwd`.
- F-10: 48 calls of `stapel new` in 19 test files expect the ticket in the current checkout; most fixtures
  have no commit, so AC-5's unborn rule keeps them unchanged.

### Decisions (before the build)

- A new module `crates/stapel-core/src/parallel.rs` holds the worktree list, the current branch, the
  branch ticket, parallel tickets, the order, the trial merge and the stops; `cargo_lock.rs` holds the
  lock; `crates/stapel-cli/src/cargo.rs` and `depends.rs` hold the two commands.
- `stapel new` holds a lock on `<git common dir>/stapel/new.lock` from the key scan to `git worktree add
  -b`; `git worktree add -b` also refuses a branch that exists.
- The cargo lock is polled with `try_lock` once a second so that the wait line prints once; `stapel cargo`
  passes the lock fd to cargo without close-on-exec, so a killed wrapper leaves the lock with cargo.
- The held mark passes through `run_cargo` by name; nested runs of this repository's tests take no lock and
  stay serial under the outer one.
- The cargo-lock path is not a `stapel.toml` key; it uses `HOME` only, because `XDG_RUNTIME_DIR` is set in
  some processes and not in others, which would split the lock (SR1-17); tests move it with `HOME`.
- `stapel check` also passes the lock fd to its cargo children (SR1-13). The wait is a poll, not a queue.
- Stop files are written to `<name>.tmp` and renamed; an unreadable one is a stop (SR1-12).
- The trial merge reads conflicts from `git merge-tree --write-tree --name-only --no-messages`, which
  honours the attributes of the merge result; the test pins the union case (SR1-11).
- The combined review reuses `stapel review start` with the round `merge` and the `stapel-reviewer` agent;
  the import records the merge round; the pack adds the merge diff. The orchestrator merges with
  `--no-ff`, so a merge of the base branch is a merge commit (SR1-16).
- The dependency agent `stapel-depends` has Read, Grep and Glob only, so `stapel depends` puts the earlier
  ticket's diff into its pack; it returns findings in the STP-23 format with `waits`.
- STP-27 is built on main by the old rules with the installed binary; builders run bare cargo through
  the whole build; `stapel init` with the new skill and builder text runs in step 17; the binary is
  reinstalled after the close (SR1-9).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `init::worktrees_ignored_and_journals_union` |
| AC-2 | `new_branch::key_above_every_worktree_and_branch` |
| AC-3 | `new_branch::new_creates_branch_and_worktree` |
| AC-4 | `new_branch::two_new_at_once` |
| AC-5 | `new_branch::unborn_head_and_ticket_branch` |
| AC-6 | `new_branch::branch_names_the_ticket` |
| AC-7 | `new_branch::key_must_match_the_branch` |
| AC-8 | `hook_session::root_from_input_cwd`, `hook_tokens::root_from_input_cwd` |
| AC-9 | `hook::permit_only_for_the_branch_ticket` |
| AC-10 | `cargo_lock::wrapper_runs_cargo` |
| AC-11 | `cargo_lock::second_waits_and_names_the_holder`, `cargo_lock::killed_holder_frees_the_lock` |
| AC-12 | `cargo_lock::check_takes_the_lock`, `cargo_lock::held_mark_takes_no_lock` |
| AC-13 | `hook::bare_cargo_denied` |
| AC-14 | `hook::subagent_git_guard` |
| AC-15 | `parallel::conflict_stops_the_later` |
| AC-16 | `parallel::next_runs_the_trial_merge` |
| AC-17 | `parallel::stopped_ticket_waits`, `parallel::spec_kind_waits_for_the_spec` |
| AC-18 | `parallel::merge_main_before_close` |
| AC-19 | `depends::merge_review_after_merge_main` |
| AC-20 | `depends::dependency_review_named`, `depends::pack_holds_both_tickets` |
| AC-21 | `records::dependency_stage` |
| AC-22 | `depends::confirmed_dependency_stops`, `depends::false_verdict_lifts_the_stop` |
| AC-23 | `parallel::merge_branch_after_close` |
| AC-24 | `parallel::can_go_on_notice` |
| AC-25 | `init_process::depends_agent_installed`, `init_process::skill_text_names_parallel_builds` |

## Plan

Route: full (tags `guard`, `data`, `interface`), auto.

1. Init files (AC-1; test `init::worktrees_ignored_and_journals_union`): `init.rs` writes the ignore line
   and `.gitattributes`. Must not change: the other init steps.
2. Keys across worktrees and branches (AC-2; test `new_branch::key_above_every_worktree_and_branch`):
   `parallel.rs` with the worktree list and branch helpers, `tickets::next_key`. Must not change: keys in a
   repository without worktrees.
3. `stapel new` with branch and worktree (AC-3, AC-4, AC-5; tests
   `new_branch::new_creates_branch_and_worktree`, `new_branch::two_new_at_once`,
   `new_branch::unborn_head_and_ticket_branch`). Must not change: `new` on an unborn HEAD.
4. The branch ticket (AC-6, AC-7; tests `new_branch::branch_names_the_ticket`,
   `new_branch::key_must_match_the_branch`): `tickets::resolve`, the token-hook rule, `check`, `ok`,
   `close`. Must not change: the one-open-ticket rule elsewhere.
5. Hook roots from the input `cwd` (AC-8; tests `hook_session::root_from_input_cwd`,
   `hook_tokens::root_from_input_cwd`). Must not change: the root order without `cwd`.
6. The build permit per branch (AC-9; test `hook::permit_only_for_the_branch_ticket`). Must not change:
   the permit on a base branch.
7. The cargo lock and `stapel cargo` (AC-10, AC-11; tests `cargo_lock::wrapper_runs_cargo`,
   `cargo_lock::second_waits_and_names_the_holder`, `cargo_lock::killed_holder_frees_the_lock`):
   `cargo_lock.rs`, `crates/stapel-cli/src/cargo.rs`. Must not change: `stapel check`.
8. The cargo lock in `stapel check` and the held mark (AC-11, AC-12; tests
   `cargo_lock::check_takes_the_lock`, `cargo_lock::held_mark_takes_no_lock`,
   `cargo_lock::killed_holder_frees_the_lock`, `cargo_lock::second_waits_and_names_the_holder`): `runner.rs`, `check.rs`;
   the RED adds the `stapel check` cases to `cargo_lock::killed_holder_frees_the_lock` and
   `cargo_lock::second_waits_and_names_the_holder`. Must not change: check outcomes, step 7.
9. The guard denies bare cargo (AC-13; test `hook::bare_cargo_denied`). Must not change: other Bash
   decisions.
10. The git guard for subagents (AC-14; test `hook::subagent_git_guard`). Must not change: the main
    session's git commands.
11. Order, trial merge and stops (AC-15, AC-16; tests `parallel::conflict_stops_the_later`,
    `parallel::next_runs_the_trial_merge`): `parallel.rs`, the post-tool-use hook, `next`. Must not change:
    `hook_faults`.
12. Wait and merge main (AC-17, AC-18; tests `parallel::stopped_ticket_waits`,
    `parallel::spec_kind_waits_for_the_spec`, `parallel::merge_main_before_close`): `next`, `close`, the
    guard; the `spec` stop is written by the test, because step 14 makes it. Must not change: the order of the existing `next` actions.
13. The dependency stage (AC-21; test `records::dependency_stage`): `records.rs`. Must not change: other
    stages.
14. Dependency review and stops (AC-20, AC-22; tests `depends::dependency_review_named`,
    `depends::pack_holds_both_tickets`, `depends::confirmed_dependency_stops`,
    `depends::false_verdict_lifts_the_stop`): `depends.rs`, `next`. Must not change: step 12.
15. The merge review (AC-19; test `depends::merge_review_after_merge_main`): `review.rs`, `next`, `close`.
    Must not change: review rounds 1 and 2.
16. Merge branch and the notice (AC-23, AC-24; tests `parallel::merge_branch_after_close`,
    `parallel::can_go_on_notice`). Must not change: `next` on a ticket that is open.
17. Install and skill text (AC-25; tests `init_process::depends_agent_installed`,
    `init_process::skill_text_names_parallel_builds`): `assets/agents/depends.md`, `assets/skill.md`,
    `init.rs`, this repository's `stapel init`, `CLAUDE.md` "The plan" and the builder text on `stapel cargo`.
    Must not change: the four agent files except the builder's cargo line.

## Review

TODO

## Summary

TODO
