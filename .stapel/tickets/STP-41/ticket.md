# STP-41 — The guards folder

## Description

Source: `docs/PLAN.md`, phase 5, item 5.1 (the guards folder); the pilot repository's `tests/guards/`
folder (a coverage floor, a bundle ceiling, forbidden colours).

### Frame

**Problem.** A project's machine checks (a coverage floor, forbidden patterns) live in files the agent
edits as freely as the code, so a check loosened to pass leaves no trace. A check with old violations
fails on its first run, so it is left out, and new violations pass too.

**Plan item:** 5.1.

**Risk tags:** `guard` (writes into the guards folder ask the person), `interface` (`[guards]`, the check
lines), `data` (`stapel check` rewrites a baseline in the working tree).

**Non-goals.** The API snapshot and the dependency dialog (5.4); a guard format of stapel's own; a
command to accept a new violation; guards at level `steps`; a baseline per ticket; numeric thresholds.

**Promise (closed list).**

1. Declaration: `[guards] dir`, one folder; `[[guards.check]]` entries with `name`, shell `command` and
   optional `baseline` inside `dir`; bad entries fail loading `stapel.toml`.
2. Running: `stapel check` at `head` and `full` runs the guards after the project commands, under the
   time limit; a failing guard fails the check, so the close and the CI refuse it.
3. Exit-code guard (no `baseline`): passes on exit 0.
4. Ratchet guard: each standard output line is a violation; a line not in the baseline, or no baseline,
   fails; exit codes other than 0 and 1 fail as broken.
5. Tightening: a passing ratchet guard's fixed lines are removed from the baseline in the working tree;
   the passing check stays valid after that change alone is committed.
6. Reach: an agent's write tool call into `dir` asks the person (main agent, dialog mode) or is denied
   (subagent, other modes), as for `stapel.toml`; `always_writable` does not open `dir`.
7. Starter: a commented `[guards]` example in the starter `stapel.toml`; the skill names the folder.

Not promised: a shell command that writes into `dir` (the guard freeze, `docs/HISTORY.md` 2026-11-10);
non-UTF-8 output; repeated lines; a guard that leaves processes behind; a command that hides its own
failure (exit 0, no output); symlinks inside `dir`; a baseline changed between the check's read and write.

**Prior art.** STP-37 (`[check] project`); STP-36 3.7a (`toml_edit_ask`); `guard.rs::decide_write`;
ratchet baselines of `betterer`, `detekt` and `rubocop --auto-gen-config`.

**Size.** About 400 source lines and 600 test lines; 10 Plan steps. Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-23: Decision (declaration, the human, recommended): a folder `[guards] dir` and a list of
  commands `[[guards.check]]` with an optional baseline; the project's existing checks are named as they
  are. Rejected: only the folder (every executable file a guard); only the list, with no protected folder.
- 2026-11-23: Decision (reach, the human, recommended): an agent's write into the folder goes through the
  permission dialog, as an edit of `stapel.toml` does. Rejected: always denied; no write rule (only the CI
  at the boundary).
- 2026-11-23: Decision (ratchet, the human, recommended): each output line of a guard with a baseline is
  one violation and a line outside the baseline fails; a guard without a baseline is judged by its exit
  code. Rejected: one number per guard; exit codes only.
- 2026-11-23: Decision (tightening, the human, recommended): `stapel check` itself writes the baseline
  without the fixed lines into the working tree and the orchestrator commits it; only the person adds a
  line. Rejected: the check fails until the person removes the line; a separate `stapel guards tighten`.
- 2026-11-23: Decision (agent): guards run at levels `head` and `full` after the project commands, as the
  project commands of STP-37 do; at level `steps` they do not run.
- 2026-11-23: Assumption: the guard freeze of 2026-11-10 does not cover this ticket, because plan item 5.1
  names the reach of the agent as its subject; Bash writes stay outside the promise for the freeze.
- 2026-11-23: Assumption: STP-40 is open in parallel; `stapel depends` runs before the build.

## Spec

Terms:

- **Relative path**: a string that, split at `/`, has no empty, `.` or `..` part, does not start with
  `/`, and holds no backslash (one `\` character) and no NUL; a `dir` may end with one `/`, which is dropped.
- **Guards folder**: the folder `[guards] dir`, a relative path from the repository root whose first part
  is not `.stapel`, `.git` or `.claude` in any case.
- **Guard**: one entry of `[[guards.check]]`: a `name`, a `command` run with `sh -c`, and an optional
  `baseline`, a relative path, read as `<dir>/<baseline>`: the **baseline path**. The **baseline** is the
  file at the baseline path in the commit the check resolved as HEAD at its start, read with `git show`
  before the command runs, lossy as UTF-8. A guard with a baseline is a **ratchet guard**; one without is an **exit-code guard**.
- **Lines** of a text: the text split at `\n`, each line with trailing whitespace (`\r` included)
  removed, empty lines left out, compared as a set of exact strings.
- **Violation**: a line of the standard output alone (not standard error) of a ratchet guard's command;
  a **new** violation is one that is not a line of the baseline; a **fixed** line is a line of the
  baseline that is not a violation. Counts are of distinct lines.
- **Guards folder write**: a Write, Edit, MultiEdit or NotebookEdit whose target, resolved as the guard
  resolves every write target, lies inside `<root>/<dir>`, compared per component without case.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF `[guards] dir` is not a relative path (`""`, `/abs`, `.`, `a/../b`, `a//b`, `a/./b`, `a\b`), or its first part is `.stapel`, `.git` or `.claude` in any case (`.github/g` loads) THEN loading `stapel.toml` fails with `guards.dir must be a relative folder of the repository outside .stapel, .git and .claude: <dir>`; IF `[guards]` is present without `[check]` THEN it fails with `guards need [check]`. | `check_guards::refuses_a_bad_dir` |
| AC-2 | IF a guard's `name` or `command` is empty or whitespace THEN loading fails with `guards.check entries need a name and a command`; IF two guards share a name THEN with `guards.check name is used twice: <name>`; IF a `baseline` is not a relative path THEN with `guards.check baseline must be a relative path inside guards.dir: <baseline>`. | `check_guards::refuses_bad_entries` |
| AC-3 | WHEN the level is `head` or `full` and guards are declared THE check runs each in order with `sh -c` in the check copy, after setting it to HEAD with tracked changes discarded before each guard (untracked files stay), with the environment of the project commands, after them; an exit-code guard prints `guard: <name>: pass` on exit 0, or `guard: <name>: fail (exit <n>)` and the last 20 lines of its standard output and error; names and lines are printed with control characters as spaces; a fail makes the result `fail`, exit 1, and the close refuses it as any failing check. | `check_guards::exit_code_guards`, `check_guards::a_failing_guard_fails_the_check` |
| AC-4 | WHEN the level is `steps` and guards are declared THE check runs none and prints `guards: skipped: level steps`. | `check_guards::steps_level_skips` |
| AC-5 | IF the time limit runs out before or during a guard, or a guard cannot start or ends by a signal THEN its line says `guard: <name>: unverified: <reason>` as a project command's line does (after the time limit, every later guard says `unverified: time limit` too), the result is `fail`, and no baseline is written. | `check_guards::time_limit_stops_a_guard` |
| AC-6 | WHEN a ratchet guard's command exits 0 or 1 and every violation is a line of its baseline THE guard prints `guard: <name>: pass`; lines on standard error are no violations. | `check_guards::known_violations_pass` |
| AC-7 | IF a ratchet guard has new violations THEN it prints `guard: <name>: fail: <n> new` (`n` distinct lines), then the first 20 new violations in output order, each as `  <line>` with control characters printed as spaces; the result is `fail` and the baseline file is not written; a command that rewrites its baseline file in the copy changes nothing, because the baseline was read from git. | `check_guards::a_new_violation_fails` |
| AC-8 | IF the baseline path of a ratchet guard is no file in the HEAD commit, and AC-9 does not apply, THEN the guard prints `guard: <name>: fail: no baseline <dir>/<baseline>`, then the first 20 violations as in AC-7; the result is `fail` and nothing is written. | `check_guards::a_missing_baseline_fails` |
| AC-9 | IF a ratchet guard's command exits with a code other than 0 and 1 (checked before AC-8) THEN it prints `guard: <name>: fail (exit <n>)` and the last 20 lines of its output, the result is `fail` and the baseline file is not written. | `check_guards::a_broken_ratchet_guard_fails` |
| AC-10 | WHEN a ratchet guard passes and its baseline file has fixed lines THE check writes `<root>/<dir>/<baseline>` in the working tree it was started in as the baseline's lines that are still violations, in their order, the first of equal lines only, each ending with `\n` (a baseline with no fixed line is not written), and prints `guard: <name>: pass, <n> fixed removed from <dir>/<baseline>; commit it`; IF the write fails THEN it prints `guard: <name>: pass; baseline not written: <reason>` and the guard stays `pass`; after the change is committed, a fixed line that comes back is a new violation. | `check_guards::fixed_violations_leave_the_baseline`, `check_guards::tightening_edge_cases` |
| AC-11 | WHEN a check ran guards THE record holds `guards`: one `{name, outcome, new, removed}` per guard, with the outcome `pass`, `fail` or `unverified`, `new` and `removed` counts (0 for an exit-code, broken or unverified guard; `new` is every distinct violation when the baseline is missing), the name cut to 200 characters; past 20 guards the rest are left out with `guards_truncated: true`. | `check_guards::the_record_lists_the_guards`, `check_guards::known_violations_pass`, `check_guards::a_new_violation_fails`, `check_guards::fixed_violations_leave_the_baseline` |
| AC-12 | WHEN the main agent makes a guards folder write in a dialog permission mode THE guard answers `ask` with the reason `a write into the guards folder <dir>: the person confirms it in this dialog`, whatever `always_writable` and the build permit say. | `hook_guards::guards_folder_edit_asks` |
| AC-13 | IF a subagent makes a guards folder write THEN the guard denies it with `only the main agent writes into the guards folder <dir>, through the person's dialog`; IF the main agent makes one in a permission mode without the dialog THEN it denies with `a write into the guards folder <dir> needs the permission dialog, which Claude Code does not show in permission mode "<mode>"`; IF `stapel.toml` does not load THEN no guards folder exists and the write rules of today apply. | `hook_guards::guards_folder_refusals` |
| AC-14 | WHEN `stapel init` writes the starter `stapel.toml` and the skill THE starter holds a commented `[guards]` example with `dir` and one `[[guards.check]]` with a `baseline`, and the skill names the guards folder, the dialog of a write into it and the commit of a tightened baseline with a `history:` line. | `init_process::starter_names_guards` |
| AC-15 | WHEN the newest check record passed and every path that differs from its commit (committed or not) is a baseline path whose lines are a subset of its lines at that commit THE check stays current for `stapel status`, `stapel next` and `stapel close`; IF any other path inside the guards folder differs, or a baseline path is no regular file at that commit or is missing or no regular file now, THEN the check is stale, under `docs/` too. | `check_guards::a_tightening_commit_keeps_the_check` |
| AC-16 | IF a ratchet guard's standard output reached the output limit or was still open at the drain deadline THEN its line says `guard: <name>: unverified: output cut`, the result is `fail` and no baseline is written; an exit-code guard with a cut output is judged by its exit code. | `check_guards::a_cut_output_is_unverified`, `check_guards::a_held_output_is_unverified` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-1 and AC-2, item 2 by AC-3 to AC-5 and AC-11,
  item 3 by AC-3, item 4 by AC-6 to AC-9 and AC-16, item 5 by AC-10 and AC-15, item 6 by AC-12 and AC-13, item 7 by AC-14.
  Unchanged: a check without `[guards]` (its lines and its record); the project commands; the step cache
  (guards are never cached); the write rules for every path outside the guards folder; the dialog of
  `stapel.toml` edits.
- **Not promised:** the Frame's list; two checks of one working tree that write the same baseline at once;
  lines of a baseline other than violations (comments, headers), which leave it at a tightening; a
  baseline committed as a symlink, a submodule or an LFS pointer (read as its git text); under junit a
  baseline path that matches a test or input glob; a baseline path that is also a guard's script.

### Abuse table

| Attempt of a careless agent | Outcome | Test |
|---|---|---|
| edits a guard's script with Edit so that it passes | the person's dialog; a subagent is denied | `hook_guards::guards_folder_edit_asks`, `hook_guards::guards_folder_refusals` |
| adds its new violation to the baseline with Write | the person's dialog; a subagent is denied | `hook_guards::guards_folder_edit_asks`, `hook_guards::guards_folder_refusals` |
| writes through `./x/../<dir>/f`, an absolute path, a symlink into the folder, or another case of `<dir>` | resolved as every write target and compared without case: the same dialog or denial | `hook_guards::guards_folder_edit_asks` |
| writes into the folder in `bypassPermissions` or `dontAsk` mode, where no dialog shows | denied | `hook_guards::guards_folder_refusals` |
| adds the guards folder to `always_writable` | the `stapel.toml` edit goes through its dialog, and `always_writable` does not open the folder | `hook_guards::guards_folder_edit_asks` |
| removes a guard or changes `dir` in `stapel.toml` | the dialog of a `stapel.toml` edit (STP-36) | `hook::stapel_toml_edit_asks` |
| breaks a ratchet guard's command so that it prints nothing (exit 127) | the guard fails and the baseline stays | `check_guards::a_broken_ratchet_guard_fails` |
| deletes the baseline with `rm`, or edits a guard with `sed -i` | a shell write is not promised (the guard freeze); a missing baseline fails the check | `check_guards::a_missing_baseline_fails` |

**Author self-check (item 7).** Done on 2026-11-23: each row of the Abuse table maps to promise item 4 or
6 or to the Not promised list; a guards folder write gains no way past the person that an edit of
`stapel.toml` does not already have.

## Test plan

Integration tests in `crates/stapel-cli/tests/`: the new files `check_guards.rs` and `hook_guards.rs` and
the existing `init_process.rs`. `check_guards.rs` builds its fixture as `check_project.rs` does (the
fake cargo of `tests/common/`, a ticket with one passed step) and declares guards as small shell scripts
committed in the guards folder; a guard that writes a mark file shows the order and the folder it ran in.
The hook tests live in the new file `hook_guards.rs` (not `hook.rs`, which STP-40 also extends; DP1-1)
and feed hook input as `hook::stapel_toml_edit_asks` does. Light tests: `stapel cargo test -j 4 -p
stapel-cli --test check_guards -- --test-threads=4` (and `--test hook_guards`, `--test init_process`);
before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Bad dir | `""`, `/abs`, `.`, `a/../b`, `./g`, `a//b`, `a/./b`, `a\b`, `.stapel/g`, `.GIT`, `.Claude/x`; `[guards]` without `[check]`; `tests/guards` and `guards/` load | `check_guards::refuses_a_bad_dir` |
| Bad entries | an empty name, a whitespace command, two guards named `size`, a baseline `""`, `/b`, `../b`, `a/./b`, `a//b`, `b/`; no `[[guards.check]]` at all loads | `check_guards::refuses_bad_entries` |
| Exit-code guards | two passing guards in order after one project command (a log file shows the order and the copy's path); guards with no project command at `head` and at `full`; a project command that edits a tracked file: the guard sees the HEAD text; a name with a newline printed as a space; guard 1 edits a tracked file and guard 2 sees the HEAD text; a guard that exits 3 with 30 output lines: the last 20 printed, result `fail`, exit 1, `stapel close` refuses | `check_guards::exit_code_guards`, `check_guards::a_failing_guard_fails_the_check` |
| Level | `--level steps` with two guards; `--level full` runs them | `check_guards::steps_level_skips` |
| Limits | a guard that sleeps past the limit and starts a child: `unverified: time limit` and the next guard too; a guard killed by a signal; the record's `unverified` outcome | `check_guards::time_limit_stops_a_guard` |
| Ratchet pass | violations equal to the baseline; a subset whose missing lines are blank lines only; CRLF output and trailing spaces; exit 1 with known violations; a line on standard error that is not in the baseline; the record's `new` 0 and `removed` 0 | `check_guards::known_violations_pass` |
| New violations | one new line among known ones; 25 new lines: `25 new` and the first 20; one new line printed three times: `1 new`; a line with a control character; the baseline file unchanged byte for byte; a command that first appends its violations to its baseline file in the copy: still fails; the record's `new` | `check_guards::a_new_violation_fails` |
| Missing baseline | no baseline file at HEAD with three violations (record `new` 3); a baseline path that is a folder; a baseline file present in the working tree but not committed; exit 2 with no baseline: the line of AC-9 | `check_guards::a_missing_baseline_fails` |
| Broken command | exit 127 with no output; exit 2 with output: tail printed, baseline unchanged | `check_guards::a_broken_ratchet_guard_fails` |
| Tightening | three baseline lines, one fixed: the file holds the two others in order, the line names `1 fixed`; a baseline with a duplicated line; the next check refuses the uncommitted change; after a commit, the fixed line printed again fails as new; a check that fails in a step still tightens a passing guard; two ratchet guards, one fails and one tightens; a ratchet guard with fixed lines stopped by the time limit: baseline unchanged; a read-only baseline file: `baseline not written`, pass; the record's `removed` |
| Cut output | a ratchet guard with fixed lines whose standard output passes the output limit: `unverified: output cut`, baseline unchanged, record `new` 0 and `removed` 0; an exit-code guard with the same output and exit 0: pass | `check_guards::a_cut_output_is_unverified` | `check_guards::fixed_violations_leave_the_baseline` |
| Record | two guards, one passing and one failing exit-code guard (`fail`, `new` 0); 22 guards and a name of 300 `ж`: cut and `guards_truncated`; no `[guards]`: no `guards` key | `check_guards::the_record_lists_the_guards` |
| Hook ask | Write, Edit, MultiEdit and NotebookEdit into `<dir>/x` by the main agent in `default`, `acceptEdits`, `auto` and `plan`; an absolute path, `./a/../<dir>/x`, a symlink to the folder, another case of `<dir>`; a `dir` listed in `always_writable`; a ticket without a build permit | `hook_guards::guards_folder_edit_asks` |
| Hook refusals | a subagent's Write into `<dir>/x`; the main agent in `bypassPermissions` and `dontAsk`; a broken `stapel.toml`: the old rules (denied without a build permit); a path beside the folder (`<dir>x/f`) follows the old rules | `hook_guards::guards_folder_refusals` |
| Check after tightening | a passing check, the tightened baseline committed: `stapel status` shows the check current and `stapel close` accepts it; the same with the change uncommitted; a commit that adds a line to the baseline, one that edits a guard script, one that edits a guard script under a guards folder in `docs/`, one that deletes the baseline, one that adds a baseline absent at the record commit, and one that changes `[guards]` with a subset baseline: stale; a path beside the folder (docs/guardsx/f) and, without `[guards]`, a `docs/` change: current | `check_guards::a_tightening_commit_keeps_the_check` |
| Drift fixes | a ratchet guard with fixed lines whose detached child (`setsid`, outside the group) holds standard output open past the drain: `unverified: output cut`, baseline unchanged; a fixed line that returns after the tightening commit: `1 new`; a check whose step fails still tightens a passing guard; a ratchet guard with fixed lines stopped by the time limit: baseline unchanged | `check_guards::a_held_output_is_unverified`, `check_guards::tightening_edge_cases` |
| Process files | the starter `stapel.toml` and the skill | `init_process::starter_names_guards` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[guards] dir` | relative path string | one component / any | `""`: load error; absent: the TOML error of the parser (`missing field`) | absolute, `.`, `..`, under `.stapel`, `.git`, `.claude`: load error | per component, without case in the hook | `check_guards::refuses_a_bad_dir` |
| `[[guards.check]] name` | string | one character / any; 200 in the record | `""` or whitespace: load error | a duplicate: load error | exact | `check_guards::refuses_bad_entries`, `check_guards::the_record_lists_the_guards` |
| `[[guards.check]] command` | string | one character / any | `""` or whitespace: load error | — | run as given by `sh -c` | `check_guards::refuses_bad_entries`, `check_guards::exit_code_guards` |
| `[[guards.check]] baseline` | relative path string | one component / any | absent: exit-code guard; `""`: load error | absolute, `.`, `..`: load error | joined to `dir` | `check_guards::refuses_bad_entries` |
| Guards per config | list | 0 / any; 20 in the record | absent or none: no guard line | — | in order | `check_guards::the_record_lists_the_guards` |
| Guard output | standard output, read lossy as UTF-8 | nothing / the output limit | nothing: no violation | non-UTF-8: not promised; cut: unverified | lines as in Terms | `check_guards::known_violations_pass`, `check_guards::fixed_violations_leave_the_baseline` |
| Baseline file | file in the HEAD commit | empty / any | empty: every violation new | missing or a folder: fail | lines as in Terms, lossy UTF-8 | `check_guards::a_missing_baseline_fails` |
| Write target of the hook | path | — / — | no `file_path`: denied as today | — | resolved, compared per component without case | `hook_guards::guards_folder_edit_asks` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | dirty tree: the tightened baseline left uncommitted | the next check refuses it as any uncommitted change | `check_guards::fixed_violations_leave_the_baseline` |
| git | history shape, shallow clone, object format, worktrees, submodules, paths, messages, configuration | unchanged by this ticket; guards run at HEAD in the check copy | — |
| claude-code | permission mode: `default`, `acceptEdits`, `auto`, `plan` ask; `bypassPermissions`, `dontAsk` deny | as for `stapel.toml` edits | `hook_guards::guards_folder_edit_asks`, `hook_guards::guards_folder_refusals` |
| claude-code | main agent versus subagent (`agent_id`) | a subagent is denied | `hook_guards::guards_folder_refusals` |
| claude-code | allow rules that skip the dialog | the `ask` of the hook, as for `stapel.toml` edits; not tested again | — |
| claude-code | environment of hooks, transcripts, tool call failed | not read by this ticket | — |
| os | case-insensitive file systems: another case of `<dir>` | compared without case | `hook_guards::guards_folder_edit_asks` |
| os | symlinks: a link outside the folder that points into it | the resolved target counts | `hook_guards::guards_folder_edit_asks` |
| os | symlinks inside the folder; file permissions of a guard script | not promised | — |
| os | processes of a guard: groups, children that leave the group, signals | the kill of the check's time limit, as for project commands | `check_guards::time_limit_stops_a_guard` |
| cargo | every dimension | unchanged; guards are shell commands, not cargo runs | — |
| node | every dimension | unchanged; under junit the guards get the linked `node_modules` as project commands do | — |

**Review Focus:** the commit of a tightened baseline when the guards folder lies under a test folder that
is a build input (the skill gives it a `history:` line; not tested); a guard that writes files into the
check copy (tracked files are reset before each guard; untracked ones stay; not tested); a guard that
cannot start (`sh` always starts in a test; the `unverified` line of `run_program`'s error, as for project
commands; not tested); a guards folder named with a trailing `/`
(`guards/` loads and means `guards`, in `check_guards::refuses_a_bad_dir`); two ratchet guards that share
one baseline file (each judged alone; the second write wins; not promised); a guards folder write by the
`stapel` binary itself, such as the tightening (not a tool call, so the hook does not see it; by design).

## Design

### Facts

- F-1: `check.rs::Runner::project` (about line 1481) runs the project commands with `sh -c` in the check
  copy at HEAD, with the cargo environment or the linked `node_modules`, the time limit, the output tail
  `PROJECT_TAIL` and the `unverified` reasons; `check.rs` (about line 668) calls it at `head` and `full`
  and writes `project` into the record with `MAX_PROJECT` and `MAX_COMMAND`.
- F-2: `check.rs` (about line 375) refuses a working tree with uncommitted changes outside the machine
  files.
- F-3: `guard.rs::toml_edit_ask` gives `Decision::AskEdit` for an edit of `stapel.toml` by the main agent
  in a mode of `dialog::DIALOG_MODES` and denies a subagent and other modes; `decide` calls it before
  `decide_write`, which applies `always_writable` and the build permit.
- F-4: `config.rs::Config::parse` validates `[check]` (`check.project entries must not be empty`);
  `starter_toml` writes `PROJECT_COMMENT` under `[check]`.
- F-6: `runner.rs::run_child` returns one `output`, stdout then stderr, each kept up to `MAX_OUTPUT`
  and drained for 2 seconds after the group is killed.
- F-7: `checkstate.rs::current` keeps a check when the changed paths are machine files, `docs/` or the
  ticket's own folder with an unchanged Plan.
- F-5: the skill is `crates/stapel-cli/assets/skill.md`; `stapel init` installs it, and
  `init_process::this_repository_has_the_current_files` compares the installed copy.

### Shape

- `stapel-core::config`: `Guards { dir, check: Vec<GuardEntry> }`, `GuardEntry { name, command,
  baseline: Option<String> }` with `deny_unknown_fields`, `Config.guards: Option<Guards>`, validated in
  `parse`; a helper `relative_normal(path)` shared by `dir` and `baseline`.
- `stapel-core::guard`: `guards_write_ask(input, target)` beside `toml_edit_ask`, called before it in
  `decide`; it reads the config of the target's root and returns `None` when there is no guards folder.
- `stapel-core::runner`: `RunResult` gains `stdout` (standard output alone) and `stdout_cut` (it reached
  `MAX_OUTPUT` or was still open at the drain deadline); `output` stays stdout then stderr.
- `stapel-cli::check`: `Runner::project` becomes a shared run of one shell command that returns the
  outcome, exit code, standard output and output tail; the project loop and a new guards loop use it; the
  guards loop resets the copy's tracked files to HEAD before each guard, reads each baseline with `git
  show <head>:<path>` (the head resolved at the start) before its command,
  judges ratchet guards and writes a tightened baseline into `root`, never into the copy.
- `stapel-core::checkstate::current`: a changed path inside the guards folder keeps the check only when it
  is a baseline path whose lines are a subset of its lines at the record's commit; the `docs/` exception
  does not apply inside the guards folder. The guards folder and the baseline paths come from the
  working-tree config, which is safe because a change of `stapel.toml` is itself stale; paths are
  compared per component and exactly, as git paths are.

### Decisions (before the build)

- 2026-11-23, agent: a ratchet guard reads exit codes 0 and 1 as a list, because linters exit 1 when they
  find something; any other code is a broken command, so a guard that cannot run never empties its
  baseline.
- 2026-11-23, agent: a guard runs after the project commands, even when a step or a project command
  failed, so one check shows every failure; a ratchet guard tightens whenever it passes itself.
- 2026-11-23, agent (SR1-2): the baseline is read from the HEAD commit before the command runs and
  written into the working tree; the two are equal because the check refuses an uncommitted working tree
  (F-2).
- 2026-11-23, agent (SR1-5): a commit that only tightens baselines keeps the passing check, so a tightening
  does not cost a second check of 45 to 100 minutes; a subset of lines cannot hide a new violation.
- 2026-11-23, agent (SR1-9): the guards folder is compared without case on every file system, so a
  case-variant folder `Guards/` beside `guards/` also asks; this can only ask more, never less.
- 2026-11-23, agent (SR2-2): a tightening commit that removes a line which is still a violation keeps the
  check; the CI's check, which runs the guard, refuses it.
- 2026-11-23, agent (SR2-6): a failed baseline write leaves the guard `pass` with its own line, because
  the code passed; the untightened baseline only allows what HEAD already allowed.
- 2026-11-23, agent (DP1-1): the hook tests go to `tests/hook_guards.rs`, and the new texts and
  `guards_write_ask` sit beside `toml_edit_ask`, so the merge with STP-40 does not touch the same lines;
  main is merged into STP-41 after STP-40 closes and before the close of STP-41.
- 2026-11-23, agent: the hook's guards folder rule needs no build permit and ignores `always_writable`,
  as the `stapel.toml` dialog does, so the person can change a guard outside a build.

### Decisions (as built)

All dated 2026-11-23, mark `agent`; the person may overrule them at the close.

- Step 1 (5fb7160): `Config.guards: Option<Guards>` with `deny_unknown_fields`; the first part of `dir`
  is compared without case to `.stapel`, `.git` and `.claude`, so `.github/g` loads; the order of the
  load errors is `dir`, then `guards need [check]`, then the entries; a missing `dir` is the parser's
  `missing field` error.
- Step 2 (647b8b8): `Worktree::reset_tracked` (`git checkout --detach --force` without a clean) runs
  before each guard; `Runner::guards` runs after `Runner::project`.
- Steps 4 and 5 (09d61ef, 7b9f097): `RunResult` gained `stdout` and `stdout_cut`; the baseline is read
  with `git cat-file blob <head>:<dir>/<baseline>` with the head id resolved at the start, so a folder
  counts as missing. Step 4 already made AC-9 and AC-16 pass, so their tests are a coverage commit
  (c76a427, with a `history:` line) and step 5's RED holds only the AC-8 test.
- Step 6 (b1f4bb7): the tightened baseline is written into the working tree with `std::fs::write`; a
  failed write prints `baseline not written: <error>`.
- Step 8: step 7's GREEN (ee83773) already denies a subagent and the modes without the dialog, so step 8
  is one coverage commit (bc1e8ab, with a `history:` line).
- Step 9 (5c80eea): `GUARDS_COMMENT` after the other comments of `starter_toml`; one skill line before the
  close lines; the installed skill was written by `stapel init` of this branch.
- Step 10 (cf1da5c): `checkstate::current` takes the changed paths inside the guards folder out before the
  `changed.retain` filter, drops a baseline path whose lines (`line_set`) are a subset at the record
  commit and now (`baseline_subset`), and adds every other one back after the filter.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `check_guards::refuses_a_bad_dir` |
| AC-2 | `check_guards::refuses_bad_entries` |
| AC-3 | `check_guards::exit_code_guards`, `check_guards::a_failing_guard_fails_the_check` |
| AC-4 | `check_guards::steps_level_skips` |
| AC-5 | `check_guards::time_limit_stops_a_guard` |
| AC-6 | `check_guards::known_violations_pass` |
| AC-7 | `check_guards::a_new_violation_fails` |
| AC-8 | `check_guards::a_missing_baseline_fails` |
| AC-9 | `check_guards::a_broken_ratchet_guard_fails` |
| AC-10 | `check_guards::fixed_violations_leave_the_baseline`, `check_guards::tightening_edge_cases` |
| AC-11 | `check_guards::the_record_lists_the_guards`, `check_guards::known_violations_pass`, `check_guards::a_new_violation_fails`, `check_guards::fixed_violations_leave_the_baseline` |
| AC-12 | `hook_guards::guards_folder_edit_asks` |
| AC-13 | `hook_guards::guards_folder_refusals` |
| AC-14 | `init_process::starter_names_guards` |
| AC-15 | `check_guards::a_tightening_commit_keeps_the_check` |
| AC-16 | `check_guards::a_cut_output_is_unverified`, `check_guards::a_held_output_is_unverified` |

## Plan

Route: full (tags `guard`, `interface`, `data`), auto.

1. Config (AC-1, AC-2; tests `check_guards::refuses_a_bad_dir`, `check_guards::refuses_bad_entries`):
   the RED adds `tests/check_guards.rs` with its fixture helper; the GREEN adds `Guards` and `GuardEntry`
   to `Config` and validates them. Must not change: a config without `[guards]`.
2. Exit-code guards (AC-3, AC-4, AC-5; tests `check_guards::exit_code_guards`,
   `check_guards::a_failing_guard_fails_the_check`, `check_guards::steps_level_skips`,
   `check_guards::time_limit_stops_a_guard`): the GREEN shares the run of `Runner::project` and runs the
   guards after the project commands at `head` and `full`. Must not change: the lines of project commands.
3. Record (AC-11; test `check_guards::the_record_lists_the_guards`): the GREEN writes `guards` and
   `guards_truncated`. Must not change: the other record fields.
4. Ratchet (AC-6, AC-7; tests `check_guards::known_violations_pass`, `check_guards::a_new_violation_fails`):
   the GREEN adds `stdout` and `stdout_cut` to `RunResult`, reads the baseline from git, splits the
   standard output into lines, compares them and writes `new` into the record. Must not change: exit-code
   guards and the `output` of every other run.
5. Missing baseline (AC-8; test `check_guards::a_missing_baseline_fails`): the GREEN prints the baseline
   path and the violations and counts `new`. The tests of AC-9 and AC-16 passed before this step, because
   step 4 already made them pass; they are a coverage commit of their own before the RED (c76a427). Must
   not change: the ratchet pass.
6. Tightening (AC-10; test `check_guards::fixed_violations_leave_the_baseline`): the GREEN writes the
   baseline without the fixed lines into the working tree, prints the count and writes `removed`; a failed
   write prints its line. Must not change: a guard without fixed lines writes nothing.
7. Hook dialog (AC-12; test `hook_guards::guards_folder_edit_asks`): the RED adds `tests/hook_guards.rs`;
   the GREEN adds `guards_write_ask` beside `toml_edit_ask` in `guard.rs` and calls it in `decide`. Must not change: the write rules outside the guards folder and the
   `stapel.toml` dialog.
8. Hook refusals (AC-13): a coverage commit only (bc1e8ab), no RED and no GREEN, because step 7's
   `guards_write_ask` already denies a subagent and the modes without the dialog.
9. Starter and skill (AC-14; test `init_process::starter_names_guards`): the GREEN adds the comment to
   `starter_toml`, the guards lines to `assets/skill.md`, and runs `stapel init` so that
   `init_process::this_repository_has_the_current_files` passes. Must not change: the other starter lines.
10. Check after tightening (AC-15; test `check_guards::a_tightening_commit_keeps_the_check`): the GREEN
   lets `checkstate::current` keep a check across baseline subsets and makes other guards folder changes
   stale. Must not change: the other kept paths.
11. Held output (DR1-1, AC-16; test `check_guards::a_held_output_is_unverified`): the GREEN makes
   `runner.rs::run_child` set `stdout_cut` when standard output was still open at the drain deadline.
   Must not change: the `output` of every run.
12. Tightening cases (DR1-4, AC-10; test `check_guards::tightening_edge_cases`): a coverage commit of the
   three Tightening cases the step 6 test left out, with a `history:` line; no GREEN.

## Review

TODO

<!-- stapel:review:begin -->
Generated by stapel; edits between the markers are overwritten.

Findings:

| Stage | Round | Roles | HIGH | MEDIUM | LOW | False | Fates |
|---|---|---|---|---|---|---|---|
| spec-review | 1 | reviewer | 1 | 6 | 2 | 0 | defer 1, patch 8 |
| spec-review | 2 | reviewer | 0 | 3 | 7 | 0 | patch 10 |
| step-drift | 1 | drift | 0 | 1 | 3 | 0 | no-change 1, patch 3 |
| dependency | STP-40:00d8a34c:3a09984c | depends | 0 | 0 | 1 | 0 | open 1 |
| dependency | STP-40:4afbf11c:3a09984c | depends | 0 | 0 | 1 | 0 | open 1 |
| dependency | STP-40:4afbf11c:bd0eb2ad | depends | 0 | 1 | 0 | 0 | patch 1 |
| dependency | STP-40:796eb16c:0646a136 | depends | 0 | 0 | 1 | 0 | open 1 |
| dependency | STP-40:d1af2e56:3a09984c | depends | 0 | 0 | 1 | 0 | open 1 |

Check runs:

| Level | Result | Commit | At | Steps |
|---|---|---|---|---|
| head | pass | 65538da | 2026-11-23T20:48:48Z | pass 10 |
<!-- stapel:review:end -->

## Summary

TODO
