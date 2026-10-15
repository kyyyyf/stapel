# STP-14 — stapel check --history and its commit hook

## Description

Source: `docs/PHASES.md`, phase 0, STP-14 (found during STP-6); the next-session note
`.stapel/scratch/stp14/next-session.md`.

### Frame

**Problem.** `stapel check` builds and runs every step's tests at RED, GREEN and HEAD and takes minutes. Six of
its outcomes come from git history alone (`unpaired`, `duplicate`, `no-tests`, `red-changes-code`,
`tests-changed`, and `build-input-changed`), and so does `unverified: interleaved`. A builder's mistake of this
kind is found today only by the full check at the end of the build, when later commits sit on top of it and the
fix is a rewrite of a chain. Found right after the commit, it is fixed by amending that commit.

**Risk tags:** `interface` (a new CLI option and a new hook entry point), `guard` (`stapel init` writes a second
hook into `.claude/settings.json`; the hook runs inside every agent's session).

**Non-goals.** A hook that blocks or undoes a commit (git `pre-commit` or `commit-msg`); a commit made from a
script, an alias or another program than a Bash tool call of Claude Code; any outcome that needs a test run;
`--json` output; `--history` counting toward `close`; detection of a `git commit` that the guard's shell parser
cannot read.

**Promise (closed list).**

1. `stapel check [KEY] --history` prints, for each step of the ticket, the outcome the full check would print
   when that outcome comes from history alone: `unpaired`, `duplicate`, `no-tests`, `red-changes-code`,
   `tests-changed`, `unverified: interleaved: <label>`, and `unverified: build-input-changed: <path> at <sha>`
   for the steps it blocks; with the same reason text and the same notes. A step whose outcome needs a run
   prints `<label>: needs-run`.
2. The output keeps the full check's form without the `cargo:` and `suite:` lines; the last line is
   `result: fail` when any step has a failing outcome, otherwise `result: no history faults`. Exit 0, 1 for
   `fail`, 2 for a refusal.
3. It refuses (exit 2) as the full check does for a legacy or unreadable ticket, a shallow clone, a bad base,
   a closed ticket without commits of its key, and a ticket without step commits; it does not refuse for uncommitted changes, a missing `[check]` table or
   a held check lock. It runs no cargo, creates no worktree and writes nothing: no record in `runs.jsonl`.
4. A closed ticket is read at the end of its range, as the full check does.
5. `--history` with `--list` is a usage error (exit 2).
6. `stapel hook post-tool-use` reads a Claude Code PostToolUse payload. When the tool is Bash and the command
   holds a `git … commit` by the guard's shell parser, it reads the repository of the payload's `cwd` (else of
   `CLAUDE_PROJECT_DIR`, else of its own working directory), takes the key from the subject of `HEAD`, and runs
   the history check of that open ticket. It exits 2 when a step fails, except a step whose only commit is a
   RED at `HEAD` (`unpaired`, the normal state between RED and GREEN); it prints every failing step with one
   advice line per step: `git commit --amend` when the step's last commit is `HEAD`, otherwise that the fix
   rewrites the history from that commit and goes to the human (Decision SR1-2). In every other case (no
   commit, no key in the subject, no such ticket, a closed ticket, no steps, a refusal, an unreadable payload,
   a panic) it prints nothing and exits 0.
7. `stapel init` adds the PostToolUse hook next to the PreToolUse hook, keeps every other key and hook, and adds
   it to an existing settings file once; when the binary is not on `PATH` the hook line exits 0 (advice, not a
   guard).

Not promised: deliberate circumvention (no `security` tag); outcomes that differ because the history changed
between the commit and the hook; a commit whose subject names a key of another repository; commits made by
`git cherry-pick`, `revert` or `merge` (Decision SR1-10); a hook that Claude Code stops at its 10 s timeout.

**Prior art.** klc's git `pre-commit` hook runs its consistency check and exits 0 when it is misconfigured
(advice that never blocks); klc's `tdd_order.verify_step` classifies commits of a step by changed paths. Here
the step classification already exists (`stapel_core::outcomes::analyse`, `build_inputs::analyse`); the hook
is a Claude Code PostToolUse hook, so it reaches the agent that made the commit.

**Size.** Medium: about 250 changed source lines (`crates/stapel-cli/src/check.rs`, `hook.rs`, `init.rs`,
`main.rs`; `crates/stapel-core/src/guard.rs` for the commit detection) and about 600 test lines (the history
mode against the full check on the same histories, the hook payloads, `init`). Without the hook it would be
about 120 and 350. Route: full (tags `interface`, `guard`).

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-15: Decision (tags, recommended): `interface` only for the CLI option; widened to `interface` and
  `guard` when the hook came into the ticket (below).
- 2026-10-15: Decision (output, recommended): the full check's form, without `cargo:` and `suite:`, exit
  0/1/2. Rejected: only failing steps; an added `--json`.
- 2026-10-15: Decision (undecided steps, recommended): `<label>: needs-run`, which does not make the result
  fail. Rejected: leave such steps out.
- 2026-10-15: Decision (more outcomes, recommended): `unverified: interleaved` and `unverified:
  build-input-changed` are shown as the full check shows them; `build-input-outside` (a configuration above the
  temporary worktree) is not history and is not promised. Rejected: the six outcomes of `docs/PHASES.md` only.
- 2026-10-15: Decision (refusals, recommended): only the history refusals stay; a dirty tree, a missing
  `[check]` and a held lock do not refuse. Rejected: every refusal of the full check.
- 2026-10-15: Decision (hook, not recommended): the hook comes in this ticket, not later. Rejected: a non-goal
  and a later ticket. Size re-stated: about 250 source and 600 test lines (about 120 and 350 without it).
- 2026-10-15: Decision (trigger, recommended): PostToolUse on Bash when the command holds `git … commit`
  (`--amend` too) by the guard's shell parser. Rejected: any Bash call after which `HEAD` moved (a state file
  and a git read on every call).
- 2026-10-15: Decision (hook result, recommended): silent when nothing fails; exit 2 with the failing lines and
  the `git commit --amend` advice otherwise. Rejected: a summary after every commit.
- 2026-10-15: Decision (hook ticket, recommended): the key from the subject of `HEAD`; every error is silent
  (exit 0). Rejected: the only open ticket.
- 2026-10-15: Decision (frame): the human confirmed the frame.
- 2026-10-15: Decision (time, not recommended): the hook timeout is 10 s, as for PreToolUse, and a test holds
  `--history` under 5 s on a ticket of 20 steps. Rejected: 30 s and a measurement in Proof only.
- 2026-10-15: Decision (git states, recommended): `--history` reads the first-parent history of `HEAD` as the
  full check does; the hook is silent while a rebase, merge, cherry-pick or revert is in progress; after a
  failed commit it checks the unchanged `HEAD`. Rejected: the hook also during a rebase.
- 2026-10-15: Decision (repository, recommended): the hook reads the repository of `CLAUDE_PROJECT_DIR`, or of
  its own working directory without it; a commit in another repository (`cd`, `git -C`) is not promised.
  Rejected: following `cd` and `-C` in the command. Changed by SR1-6 below.
- 2026-10-15, spec review round 1 (SR1-1 to SR1-12, two HIGH; round 2 follows; details in `findings.jsonl`),
  fixed in the text: SR1-1 (a lone RED at `HEAD` is not a hook fault), SR1-3 (F-1; each mode keeps its own
  refusal order), SR1-4 (`retired` is a history outcome), SR1-5 (heredoc and several `-m`), SR1-7 (one settings
  plan, two more install cases), SR1-8, SR1-11, SR1-12.
- 2026-10-15: Decision (SR1-2, not recommended): the hook prints every failing step with advice per step
  (amend when the step's last commit is `HEAD`, otherwise the human rewrites the chain). Rejected: only the
  faults `HEAD` adds (a comparison with `HEAD^`).
- 2026-10-15: Decision (SR1-6, recommended): the hook reads the payload's `cwd`, then `CLAUDE_PROJECT_DIR`,
  then its working directory, as the guard does; a builder's commit in a linked worktree is checked there.
- 2026-10-15: Decision (SR1-9 accepted, SR1-10 not promised, recommended): the hook is silent for a closed
  ticket; commits by cherry-pick, revert or merge do not run it.

## Spec

Terms:

- **History outcome** of a step: an outcome the full check gives without a cargo run, in the full check's order:
  the fixed outcome of `outcomes::analyse`, whatever it is (`unpaired`, `duplicate`, `no-tests`,
  `red-changes-code`, `tests-changed`, and the passing `retired`); else `unverified: build-input-changed: <path> at <sha>` when a build input changed; else
  `unverified: interleaved: <label>`. Other steps have none.
- **Failing**: an outcome for which `outcomes::passing` is false. `needs-run` is not failing.
- **History check**: `stapel check [KEY] --history`. **Full check**: `stapel check [KEY]`.
- **Commit command**: a Bash command in which the guard's shell parser finds a `git` call whose subcommand is
  `commit`.
- **Hook fault**: a step with a failing history outcome, except a step whose only commit is a RED at `HEAD`.
- **Hook repository**: the repository of the payload's `cwd`, else of `CLAUDE_PROJECT_DIR`, else of the hook's
  working directory.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the history check runs on a ticket with step commits THE `stapel check` command prints `ticket: <KEY> at <sha>` (`… (closed)` for a closed ticket), the `build inputs:` line when the full check prints it, then for each step in order its history outcome line and notes in the full check's form, or `<label>: needs-run`, and last `result: fail` or `result: no history faults`; it prints no `cargo:` and no `suite:` line. Output: `golden/check_history.txt`. | `check_history::prints_the_full_check_form` |
| AC-2 | WHEN the full check and the history check read the same history THE line and notes of each step with a history outcome are equal byte for byte, for each of `unpaired`, `duplicate`, `no-tests`, `red-changes-code`, `tests-changed`, `retired`, `interleaved` and `build-input-changed`, and with the notes `superseded by` and `retired by`. | `check_history::same_lines_as_the_full_check`, `check_history::same_lines_when_a_build_input_changed` |
| AC-3 | WHEN the history check ends THE command exits 1 when any step has a failing outcome, else 0; `needs-run` steps alone give 0. | `check_history::exit_code_follows_the_outcomes` |
| AC-4 | IF the ticket is legacy or unreadable, the clone is shallow, the base is not an ancestor of `HEAD`, the ticket is closed and has no commit of its key, or the ticket has no step commits THEN the history check exits 2 with the full check's refusal text. | `check_history::refuses_as_the_full_check` |
| AC-5 | WHEN the working tree has uncommitted changes, `[check]` is missing from `stapel.toml`, or another check holds the lock THE history check still runs and exits as AC-3. | `check_history::runs_without_the_run_preconditions` |
| AC-6 | WHEN the history check runs THE command starts no cargo (a `cargo` on `PATH` that records its start is never started), creates no worktree and changes no file under the repository, `runs.jsonl` included. | `check_history::writes_nothing_and_starts_no_cargo` |
| AC-7 | WHEN the ticket is closed THE history check reads the end of its range, as the full check does, and later commits of other tickets do not change its lines. | `check_history::closed_ticket_at_the_end_of_its_range` |
| AC-8 | IF `--history` and `--list` are given together THEN `stapel check` exits 2 with clap's usage error and prints no step. | `check_history::history_with_list_is_a_usage_error` |
| AC-9 | WHEN only the history check ran on a ticket THE `stapel close` command still refuses with "close needs a current passing check". | `check_history::does_not_count_for_close` |
| AC-10 | WHEN the history check runs on a ticket of 20 steps THE command ends in less than 5 s. | `check_history::twenty_steps_within_five_seconds` |
| AC-11 | WHEN `stapel hook post-tool-use` reads a payload with `tool_name` `Bash` and a commit command, the subject of `HEAD` in the hook repository starts with the key of an open ticket (`<KEY> ` or `<KEY>:`), and that ticket has a hook fault, THE hook exits 2 and prints to stderr each hook fault's step line with its notes, then per hook fault `stapel: <label>: its last commit is HEAD; fix it with git commit --amend` or `stapel: <label>: its last commit <sha> is below HEAD; the fix rewrites the history from there; ask the human`, then `details: stapel check <KEY> --history`. Output: `golden/hook_post_fail.txt`. | `hook_post::failing_history_exits_2_with_advice` |
| AC-12 | WHEN the open ticket has no hook fault THE hook prints nothing and exits 0; this holds when its last step's only commit is a RED at `HEAD` (`unpaired`). | `hook_post::silent_without_faults`, `hook_post::silent_after_a_lone_red` |
| AC-13 | IF the tool is not Bash, the command is no commit command, the payload is not valid JSON, the subject of `HEAD` names no ticket folder, the ticket is closed, the ticket has no step commits, the history check refuses, or `HEAD` is unborn THEN the hook prints nothing and exits 0. | `hook_post::silent_cases` |
| AC-14 | IF a rebase, merge, cherry-pick or revert is in progress in the repository THEN the hook prints nothing and exits 0, whatever the history. | `hook_post::silent_during_a_rebase` |
| AC-15 | WHEN the command is `git commit -m x`, `git commit -a -m x -m y`, `git commit -m "$(cat <<'EOF' … EOF)"` (Claude Code's usual form), `git commit --amend --no-edit`, `git -C . commit -m x`, `cd . && git commit -m x`, `bash -c 'git commit -m x'` or `git -c user.name=a commit -m x` THE hook runs the history check; WHEN it is `git log --grep commit`, `echo git commit`, `git commit-tree …` or `git status` THE hook does not. | `hook_post::commit_command_forms` |
| AC-16 | WHEN the payload has `cwd` THE hook reads the repository of that directory, whatever `CLAUDE_PROJECT_DIR` and its own working directory are; without `cwd`, the repository of `CLAUDE_PROJECT_DIR`; without both, of its working directory. A `cwd` in a linked worktree reads that worktree's `HEAD`. | `hook_post::reads_the_hook_repository` |
| AC-17 | WHEN `stapel init` runs THE `.claude/settings.json` gets one PostToolUse entry with matcher `Bash`, command `command -v stapel >/dev/null 2>&1 \|\| exit 0; stapel hook post-tool-use` and timeout 10, next to the PreToolUse entry, in one write; other keys and hooks stay; a second run adds nothing. A file with neither stapel entry prints `appended: .claude/settings.json (stapel hooks)`; a file with only today's PreToolUse entry prints `appended: .claude/settings.json (stapel commit hook)`; a file with the bare `stapel hook pre-tool-use` entry prints `updated: .claude/settings.json (stapel hook line)` and gets the PostToolUse entry too. | `hooks_install::installs_the_post_tool_use_hook`, `hooks_install::adds_the_post_hook_to_an_existing_install` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-17; the existing tests of `check.rs`, `hook.rs` and
  `hooks_install.rs` keep the full check, the guard and the PreToolUse install as they are.
- **Not promised:** the Frame's list.

## Design

### Facts (external contract and states)

- F-1: `outcomes::analyse` returns per step `fixed`, `interleaved` and `notes` from history; `build_inputs::analyse`
  returns `changed`; neither runs cargo or needs a worktree. The full check calls them after its refusals for a
  missing `[check]`, a dirty tree and no steps, and after it takes the lock (corrected, SR1-3).
- F-2: in the full check a fixed outcome comes first; then `Runner::step` gives `unverified: <blocked>` when a
  build input changed (or lies outside the worktree), then `unverified: interleaved: <label>`.
- F-3: `close` reads `checkstate::current`, which reads `runs.jsonl` only.
- F-4: Claude Code shows the stderr of a PostToolUse hook that exits 2 to the agent; the tool already ran.
  Other non-zero exit codes show stderr to the person only, so the hook prints nothing on them; at its timeout
  the hook is stopped. The payload has `tool_name`, `tool_input.command`, `cwd`.
- F-5: the guard parses shell text in `guard::scan`; a `git` call goes through `check_git` and
  `check_subcommand`; text it cannot read sets `too_deep`. Any `stapel hook …` from an agent is denied
  (`HOOK_SELF_REASON`).
- F-6: a rebase, merge, cherry-pick or revert in progress leaves `rebase-merge`, `rebase-apply`, `MERGE_HEAD`,
  `CHERRY_PICK_HEAD` or `REVERT_HEAD` under the git directory (`git rev-parse --git-path`).

### Decisions (before the build)

- One function `outcomes::history_outcome(analysis, blocked) -> Option<(String, String)>` gives F-2's order;
  the full check's loop and `Runner::step` use it, so both modes have one copy of the order. `blocked` is the
  `build-input-changed` text; `build-input-outside` stays in the full check only.
- Each refusal of the full check becomes a small function (ticket status, `HEAD`, shallow clone, base, closed
  range, no steps) that both modes call, each in its own order: the full check keeps today's order, `[check]`
  and the dirty tree included (SR1-3). `check::history_report(root, ticket, at, steps, bounds)` builds the
  report from `analyse`, `build_inputs::analyse` and `history_outcome`; `--history` prints all of it, and the
  hook prints its hook faults. Each step in the report keeps its last commit for the hook's advice.
- `guard::holds_git_commit(command) -> bool`: `scan` sets a new `Found::commit` when `check_subcommand` sees
  `commit`; unreadable text gives `false`.
- `hook::post_tool_use` catches panics and maps every error to exit 0 with no output (F-4: advice, not a
  guard). The key is the first word of the subject of `HEAD` without a trailing `:`, resolved by
  `tickets::resolve`; a closed ticket is silent. The hook repository is found with `guard::stapel_root` from
  `cwd`, `CLAUDE_PROJECT_DIR` or the working directory, in that order.
- `init::plan_hooks` reads the settings once, adds or upgrades both entries in one value and plans one `Step`
  (SR1-7); the PostToolUse line exits 0 when the binary is missing.

### Decisions (as built)

Notes are kept in `.stapel/scratch/stp14/as-built.md` during the build and moved here before code review.

## Test plan

CLI tests in two new files, `crates/stapel-cli/tests/check_history.rs` and
`crates/stapel-cli/tests/hook_post.rs`, and new functions in `hooks_install.rs`. The history fixtures copy the
helpers of `check.rs` (`git`, `write`, `commit`, `add_test`, `edit`); only `same_lines_as_the_full_check`
builds with cargo. Light runs: `cargo test -j 4 -p stapel-cli --test check_history -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | a ticket with one step of each history outcome and one clean pair; golden output | `check_history::prints_the_full_check_form` |
| Equality | the same history through the full check (cargo fixture) and `--history`: every history-outcome line, a fully retired step (`retired`) and the two notes equal; the clean pair is `pass` in the full check and `needs-run` here | `check_history::same_lines_as_the_full_check` |
| Equality | a step's RED changes `Cargo.toml`: every non-fixed step reads the same `unverified: build-input-changed` line in both modes; a fixed step keeps its fixed outcome | `check_history::same_lines_when_a_build_input_changed` |
| Exit | only `needs-run` (0); one `unpaired` (1); one `interleaved` (1) | `check_history::exit_code_follows_the_outcomes` |
| Negative | legacy ticket, shallow clone, base not an ancestor, a closed ticket without commits of its key, no step commits; each text equal to the full check's (with a clean tree and `[check]` set, so the full check reaches the same refusal) | `check_history::refuses_as_the_full_check` |
| Preconditions | dirty tree, no `[check]`, a lock file of a running process | `check_history::runs_without_the_run_preconditions` |
| No side effect | a fake `cargo` first on `PATH` writes a marker; tree snapshot, `git worktree list` and `runs.jsonl` before and after | `check_history::writes_nothing_and_starts_no_cargo` |
| Closed | a closed ticket followed by commits of another ticket | `check_history::closed_ticket_at_the_end_of_its_range` |
| Input | `--history --list` | `check_history::history_with_list_is_a_usage_error` |
| Close | `[check]` set and no earlier check record; `--history` with no faults, then `stapel close` refuses with "close needs a current passing check" | `check_history::does_not_count_for_close` |
| Time | 20 RED/GREEN pairs, one test each | `check_history::twenty_steps_within_five_seconds` |
| Hook | a ticket with a clean pair, an older `unpaired` step and a `red-changes-code` step whose RED is `HEAD`: exit 2, golden stderr with both advice forms and without the clean step, nothing on stdout | `hook_post::failing_history_exits_2_with_advice` |
| Hook | a clean ticket: exit 0, empty output | `hook_post::silent_without_faults` |
| Hook | clean pairs, then a correct RED commit at `HEAD`: exit 0, empty output; the same RED one commit below `HEAD` (an unrelated commit on top) gives exit 2 | `hook_post::silent_after_a_lone_red` |
| Hook | tool `Edit`; `git status`; payload `not json`; subject without a key; key of no folder; a closed failing ticket; ticket without steps; shallow clone; unborn `HEAD` | `hook_post::silent_cases` |
| Hook | a failing ticket with `rebase-merge`, `MERGE_HEAD`, `CHERRY_PICK_HEAD`, `REVERT_HEAD` present in turn | `hook_post::silent_during_a_rebase` |
| Hook | the twelve command forms of AC-15 against a failing ticket: exit 2 or 0 | `hook_post::commit_command_forms` |
| Hook | three places, one failing and two clean: `cwd` failing with the other two clean; `cwd` absent and `CLAUDE_PROJECT_DIR` failing; both absent and the working directory failing; `cwd` in a linked worktree whose `HEAD` fails while the main checkout is clean | `hook_post::reads_the_hook_repository` |
| Install | fresh `init`; second `init` changes nothing; settings with a foreign PostToolUse hook kept | `hooks_install::installs_the_post_tool_use_hook` |
| Install | settings with neither stapel entry; with only today's PreToolUse entry; with the bare `stapel hook pre-tool-use` entry: labels of AC-17, both entries present, other hooks kept | `hooks_install::adds_the_post_hook_to_an_existing_install` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `--history` | flag of `stapel check` | — | absent: the full check | with `--list`: usage error, exit 2 | — | `check_history::history_with_list_is_a_usage_error`, `check_history::prints_the_full_check_form` |
| `KEY` (positional) | ticket folder name, any case | one ticket; several open | absent: the only open ticket, as the full check | as the full check's refusals | as `tickets::resolve` | `check_history::refuses_as_the_full_check` |
| Step history | first-parent commits of the ticket | one step; 20 steps | no step commits: refusal | each history outcome | byte-equal lines | `check_history::same_lines_as_the_full_check`, `check_history::twenty_steps_within_five_seconds` |
| Hook payload | JSON on stdin | any size Claude Code sends | empty stdin: silent | not JSON, no `tool_name`, no `command`: silent | — | `hook_post::silent_cases` |
| `tool_input.command` | shell text | one word; nested `bash -c` | empty: silent | unreadable by the parser: silent | the guard's parser | `hook_post::commit_command_forms`, `hook_post::silent_cases` |
| Subject of `HEAD` | first line of the commit message | `<KEY>:`; `<KEY> <label> RED: …` | empty subject: silent | no folder of that key: silent | first word, trailing `:` removed | `hook_post::silent_cases` |
| `cwd` (payload), `CLAUDE_PROJECT_DIR` | paths | — | absent: the next in the order of AC-16 | not inside a repository with `.stapel/`: silent | — | `hook_post::reads_the_hook_repository`, `hook_post::silent_cases` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | history shape: merge commits, rebased or amended history | first-parent history of `HEAD`, as the full check; an amended commit is read as it is now | `check_history::same_lines_as_the_full_check` |
| git | shallow clone | refused (exit 2) by the history check; silent in the hook | `check_history::refuses_as_the_full_check`, `hook_post::silent_cases` |
| git | detached HEAD | read as `HEAD`, as the full check | `check_history::closed_ticket_at_the_end_of_its_range` |
| git | rebase, merge, cherry-pick or revert in progress | hook silent | `hook_post::silent_during_a_rebase` |
| git | dirty tree | not refused; only commits are read | `check_history::runs_without_the_run_preconditions` |
| git | linked worktrees | the hook reads the worktree of `cwd` | `hook_post::reads_the_hook_repository` |
| git | commits by cherry-pick, revert, merge | no hook run (SR1-10) | not promised |
| git | object format, submodules, path and message bytes, configuration | as the full check (same reads); not retested | — |
| cargo | every dimension | never started | `check_history::writes_nothing_and_starts_no_cargo` |
| claude-code | environment of hook processes: payload `cwd`, `CLAUDE_PROJECT_DIR`, working directory | AC-16 | `hook_post::reads_the_hook_repository` |
| claude-code | hook timeout (10 s) | the hook is stopped; not promised | not promised |
| claude-code | settings at project level | AC-17; user and local levels not touched | `hooks_install::installs_the_post_tool_use_hook` |
| claude-code | permission mode | the hook runs in every mode; output does not depend on it | not promised |
| claude-code | transcripts | not touched | — |
| os | time | AC-10 | `check_history::twenty_steps_within_five_seconds` |
| os | binary not on `PATH` | the hook line exits 0 | `hooks_install::installs_the_post_tool_use_hook` (the line's text) |
| os | case-insensitive file systems, symlinks, processes, WSL | as the full check; not retested | — |

### Abuse table

Attempts of a careless agent against "the history check never counts as a pass" and "the hook reaches the agent".

| Attempt | Outcome |
|---|---|
| The agent runs `--history`, sees `no history faults`, and runs `stapel close` | close refused, AC-9 |
| The agent runs `stapel hook post-tool-use` itself to see a clean answer | the guard denies any `stapel hook` (F-5); not retested |
| The agent commits with `git -C . commit` or inside `bash -c` | the hook still runs, AC-15 |
| The agent commits with a subject without the key | the hook is silent (AC-13); the full check finds the step missing at the end; not promised here |
| The agent removes the hook from `.claude/settings.json` | the guard's existing write rules apply; not promised |

**Review Focus** (inputs and failing code no planned test exercises): (1) a step with both a fixed outcome and
a changed build input: wrong code that tests `blocked` before `fixed` passes AC-1's golden unless the fixture
has such a step, so `same_lines_when_a_build_input_changed` holds one; (2) a `needs-run` that wrongly makes the
result fail passes every test with a failing step, so `exit_code_follows_the_outcomes` has a case with only
`needs-run`; (3) a hook that prints the whole report instead of failing lines passes an exit-code test, so
AC-11 uses a golden with a clean step in the fixture; (4) a hook that reads one of `cwd`, `CLAUDE_PROJECT_DIR` or
its working directory and ignores the others passes every test where they are equal, so AC-16 makes each
differ; (5) a hook that skips every `unpaired` step, not only a lone RED at `HEAD`, passes AC-12, so
`silent_after_a_lone_red` also puts that RED below `HEAD`. A history check that writes a `pass` record is
caught by AC-6 and AC-9.

**Author self-check:** 2026-10-15, before spec review: every promise item has a criterion and a test; the
Inputs and External states rows were checked against `check.rs`, `outcomes.rs`, `guard.rs` and `init.rs`; each
criterion has a negative case in Review Focus or in its test's cases; no open question is left.
2026-10-15, after spec review round 1: every inside finding applied as one change; SR1-2, SR1-6, SR1-9 and
SR1-10 decided by the human; round 2 for SR1-1 and SR1-2 (HIGH).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check_history::prints_the_full_check_form` |
| AC-2 | `check_history::same_lines_as_the_full_check`, `check_history::same_lines_when_a_build_input_changed` |
| AC-3 | `check_history::exit_code_follows_the_outcomes` |
| AC-4 | `check_history::refuses_as_the_full_check` |
| AC-5 | `check_history::runs_without_the_run_preconditions` |
| AC-6 | `check_history::writes_nothing_and_starts_no_cargo` |
| AC-7 | `check_history::closed_ticket_at_the_end_of_its_range` |
| AC-8 | `check_history::history_with_list_is_a_usage_error` |
| AC-9 | `check_history::does_not_count_for_close` |
| AC-10 | `check_history::twenty_steps_within_five_seconds` |
| AC-11 | `hook_post::failing_history_exits_2_with_advice` |
| AC-12 | `hook_post::silent_without_faults`, `hook_post::silent_after_a_lone_red` |
| AC-13 | `hook_post::silent_cases` |
| AC-14 | `hook_post::silent_during_a_rebase` |
| AC-15 | `hook_post::commit_command_forms` |
| AC-16 | `hook_post::reads_the_hook_repository` |
| AC-17 | `hooks_install::installs_the_post_tool_use_hook`, `hooks_install::adds_the_post_hook_to_an_existing_install` |

## Plan

Route: full (source: `auto`; tags `interface`, `guard`).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | `outcomes::history_outcome`; `check::history` and the `--history` flag in `check.rs` and `main.rs`; the full check calls both | AC-1 to AC-10 | the full check's output and records, `check.rs` tests, golden files of the full check |
| 2 | `guard::holds_git_commit`; `stapel hook post-tool-use` in `hook.rs` and `main.rs` | AC-11 to AC-16 | the PreToolUse decisions and their tests, the check output |
| 3 | The PostToolUse entry in `init::plan_hooks` | AC-17 | the PreToolUse entry, its upgrade path and its tests |

## Review

TODO

## Summary

TODO
