# STP-1 — cargo workspace, `stapel init`, `stapel.toml`, hooks

## Description

Source: `docs/PHASES.md`, phase 0, the first ticket. There is no external tracker.

A cargo workspace of seven packages is needed, and the first command `stapel init`, which prepares someone
else's repository for work: it creates `.stapel/` with a starting `stapel.toml` and installs Claude Code hooks
that forbid agents from running `git push` and from writing to code before the build permit. Closes
criterion 1 of phase 0 and part of criterion 7.

## Spec

### Acceptance criteria

Each criterion is written in the form "who · what · with what · under what condition" and has a test. Test
names are provisional; the exact paths will appear at the RED step.

| № | Criterion | Test |
|---|---|---|
| AC-1 | Developer · builds the `stapel` binary · with the single command `cargo build --release` from the root · on a clean copy of the repository; the workspace has exactly seven packages from `CLAUDE.md`, five of them empty library stubs. | `cli::workspace_has_seven_members`, `cli::version_prints_package_version`, `cli::stub_packages_are_empty` |
| AC-2 | Developer · runs `stapel init` · at the root of a git repository without `.stapel/` · and gets `.stapel/stapel.toml`, `.stapel/tickets/` (with `.gitkeep`, so that the folder gets into git), `.stapel/allowlist.toml`, the line `/.stapel/index/` in `.gitignore`, `.claude/settings.json` and the git hook `pre-push` (AC-13); the command prints every created or appended file and exits with code 0. | `init::creates_layout_in_empty_repo`, `init::creates_layout_at_repo_root_from_subdir`, `init::appends_to_existing_gitignore` |
| AC-3 | `stapel-core` · reads the `stapel.toml` created by `init`, · without errors · and sees in it the sections `spec, design, proof, plan, review, summary`, the six roles from `docs/PLAN.md` §5 and the key rule `tickets.key` with the prefix given at `init`. | `core::config::default_config_roundtrips`, `core::config::default_config_has_roles_and_sections`, `core::config::config_rejects_missing_ticket_key` |
| AC-4 | Developer · runs `stapel init` again · in an already initialized repository · and no file changes: the contents and the modification time are the same; the command prints "already set up" and exits with code 0. | `init::second_run_changes_nothing` |
| AC-5 | Developer · runs `stapel init` · when `stapel.toml` already exists and was edited by hand · and the file stays as it is; missing parts of the layout and the hooks are appended. If `stapel.toml`, `.gitignore` or `.claude/settings.json` cannot be read (cannot be parsed, not UTF-8), `init` refuses with code 1, names the file and writes nothing. | `init::keeps_user_edited_config`, `init::second_run_with_other_prefix_keeps_key`, `init::refuses_unparsable_config`, `init::keeps_non_utf8_gitignore`, `hooks_install::refuses_broken_settings`, `init::refuses_non_utf8_config`, `hooks_install::refuses_non_utf8_settings` |
| AC-6 | `stapel init` · installs hooks into `.claude/settings.json` · when the file does not exist, when it exists with foreign keys and hooks, and on a repeated run · so that foreign keys and hooks are kept and the `stapel` entries are not duplicated; a hook line of the old form (`stapel hook pre-tool-use` without a `PATH` check) is replaced by the current one, and `init` prints "updated". | `hooks_install::hooks_into_missing_settings`, `hooks_install::hooks_merge_with_foreign_settings`, `hooks_install::hooks_not_duplicated`, `hooks_install::refuses_broken_settings`, `hooks_install::upgrades_old_hook_line` |
| AC-7 | The hook `stapel hook pre-tool-use` · receives a `Bash` call from Claude Code · with a command whose text shows `git push` in one of the listed forms (`git push`, `git -C dir push`, `cd x && git push`, `FOO=1 git push`, `git push` after `;` or `\|\|`, inside `if`, `for`, `{ }`, `$( )` and backticks, behind the wrappers `env`, `sudo`, `timeout`, `xargs`, `eval` and similar, as well as `git send-pack`, `git-push`, an alias through `-c alias.x=push` or the `GIT_CONFIG_*` variables, process substitution `<( )`, a line break with `\`, a word with `$` in the place of the subcommand, text fed to `sh` by a pipeline or a here-string, `find -exec`, `watch`, `git subtree push`, `git submodule foreach`, `git rebase --exec`, `git bisect run`, the value of `-c` (`core.pager` and similar), an argument with spaces for wrappers and launchers such as `watch`, `ssh`, `su`, `tmux`, `docker`; the words `--no-verify`, `send-pack`, `http-push` in any command, including a `python -c` string) · and answers with a refusal and a reason; a command that is too long (over 64 KiB), nested too deeply, or needs more work than the check budget allows is also rejected rather than passed. Forms that are visible only at run time are caught by the second layer (AC-13); the commands `git status`, `git log --grep push`, `echo "git push"` are passed. | `hook::denies_git_push_variants`, `hook::allows_non_push_commands`, `hook::denies_git_push_in_compound_forms`, `hook::allows_lookalikes_after_round_one`, `hook::denies_git_push_round_two`, `hook::allows_lookalikes_after_round_two`, `hook::pathological_input_is_denied_not_crashed`, `hook::denies_git_push_round_three`, `hook::allows_lookalikes_after_round_three`, `hook::wrapper_eval_chains_are_bounded` |
| AC-8 | The hook `stapel hook pre-tool-use` · receives a `Write`, `Edit`, `MultiEdit` or `NotebookEdit` call · for a path outside the list `guard.always_writable` from `stapel.toml` (by default `.stapel/` and `docs/`; an invalid or unreadable config narrows the list to `.stapel/`) · when no ticket has the build permitted · and answers with a refusal and the reason "build is not allowed"; paths from the list are passed. The repository is determined from the path itself with symbolic links resolved, not from the agent's current folder. Service files — everything in `.git/` and `.claude/`, the git hooks folder (including one from `core.hooksPath`), `.stapel/stapel.toml` and, in a ticket folder, `state.json`, `decisions.jsonl`, `findings.jsonl`, `runs.jsonl`, `tokens.jsonl` (names are compared case-insensitively) — are always forbidden to the write tools, even when the build is permitted. Symbolic links, including dangling ones, are resolved before the check. The user's global git and Claude Code configuration (`~/.gitconfig`, `$XDG_CONFIG_HOME/git/config`, `$GIT_CONFIG_GLOBAL`, `~/.claude/settings.json`, `~/.claude/settings.local.json`) is denied to the write tools in a stapel session. | `hook::denies_code_write_without_build`, `hook::allows_stapel_and_docs_writes`, `hook::always_writable_comes_from_config`, `hook::allows_writes_outside_repo`, `hook::denies_machine_file_writes_even_with_build`, `hook::finds_repo_from_target_and_project_dir`, `hook::resolves_symlinks_before_matching`, `hook::bad_always_writable_does_not_open_everything`, `core::config::config_rejects_bad_always_writable`, `hook::denies_settings_and_case_variants`, `hook::follows_dangling_symlink`, `hook::denies_git_and_claude_writes_even_with_build`, `hook::denies_global_config_writes` |
| AC-9 | The hook · receives a call that writes to code · when `.stapel/tickets/<key>/state.json` contains `"build": {"allowed": true}` · and passes it. | `hook::allows_code_write_when_build_allowed` |
| AC-10 | The hook · receives input it cannot parse, or is called outside a repository with a `stapel.toml` · and does not crash: unparsable input is rejected with a reason, outside a repository the call is passed. | `hook::rejects_garbage_input`, `hook::passes_outside_stapel_repo`, `hook::rejects_non_string_command` |
| AC-11 | Developer · runs `stapel init` · outside a git repository · and gets a refusal with code 1 and the message "not a git repository"; nothing is created. | `init::refuses_outside_git` |
| AC-13 | `stapel init` · installs the git hook `pre-push` into the hooks folder named by `git rev-parse --git-path hooks` · so that a push from an environment with the variable `CLAUDECODE` or `CLAUDE_CODE_ENTRYPOINT` (set by Claude Code in the agent's shell) is rejected with a reason, while a human's push from their own terminal passes; a foreign `pre-push` is not touched, `init` warns; a hooks folder outside the repository is not touched, `init` warns; its own `pre-push` without the execute bit is repaired; the `pre-tool-use` hook rejects commands whose text shows the unsetting of `CLAUDECODE` or work with git hooks (`unset`, `env -u`, `env -i`, `env -`, `exec -c`, `declare +x`, including behind `command` and `builtin`; `core.hooksPath`, `.git/hooks`, `hooks/pre-push` in words and in redirection targets). Assignments that move where git reads its config or hooks (`HOME=`, `XDG_CONFIG_HOME=`, `GIT_CONFIG_GLOBAL=`, `GIT_CONFIG_SYSTEM=`, `GIT_DIR=`, `GIT_COMMON_DIR=`, also after `export` or `env`) are denied too. `env` and `exec` options are read only before the program they run, so the program's own flags (`sed -i`, `grep -i`) are not mistaken for them. | `hooks_install::installs_pre_push_hook`, `hooks_install::keeps_foreign_pre_push`, `hook::denies_pre_push_bypass`, `hook::denies_pre_push_bypass_round_three`, `hooks_install::pre_push_blocks_entrypoint_env`, `hooks_install::skips_hooks_dir_outside_repo`, `hooks_install::repairs_pre_push_exec_bit`, `hook::denies_pre_push_bypass_round_four`, `hook::allows_lookalikes_after_round_four` |
| AC-12 | Developer · runs the first `stapel init` · with `--prefix ABC` or by answering a question in the terminal · and gets `tickets.key = "ABC-{n}"` in `stapel.toml`; the prefix is 2 to 8 uppercase Latin letters, otherwise a refusal with a reason; without a terminal and without `--prefix` — a refusal with code 1 and the hint `--prefix`, nothing is created; a repeated `init` does not ask for the prefix. | `init::prefix_from_flag`, `init::prefix_from_prompt`, `init::rejects_bad_prefix`, `init::refuses_without_prefix_noninteractive` |

### Questions

Questions with a recommendation. All five have been answered.

1. **Model identifier for the builder and the reviewers.** `docs/PLAN.md` §5 says `claude-sonnet-5`, while
   the current model of the family is `claude-sonnet-5-5`. Recommendation: write `claude-sonnet-5-5` in the
   starting `stapel.toml` and fix `PLAN.md` in the same commit. Answer (2026-10-02, human): fix it.
   `PLAN.md` was fixed by a commit of this ticket.
2. **How the hook learns that the build is permitted.** `state.json` appears only in STP-2. Recommendation:
   STP-1 fixes a minimal contract — the field `build.allowed` in the `state.json` of any ticket — and STP-2
   starts writing it. The alternative is to postpone the write ban until STP-2. Answer (2026-10-02, human):
   as recommended.
3. **What the hook does if the `stapel` binary is not in `PATH`.** (Clarified by review finding E-4: the
   line written to `settings.json` exits with code 2 without `stapel` in `PATH`, that is, it forbids the
   call; the `init` warning remained.) Claude Code treats a failure of the hook command (an exit code other
   than 2) as non-blocking, so the protection silently disappears. Recommendation: `init` checks `PATH` and
   warns; `settings.json` gets `stapel hook pre-tool-use` without an absolute path, so that the file stays
   portable between machines. Answer (2026-10-02, human): as recommended.
4. **Whether to install `init` into this repository at the end of STP-1.** (Extended by findings E-5 and
   E2-3: the ticket's closing commit sets `build.allowed = false` in `.stapel/tickets/STP-1/state.json`,
   otherwise after the merge writing to code here would stay open.) Then the write ban starts working for
   the orchestrator right away, and `state.json` is kept by hand in phase 0. Recommendation: yes, this is
   the first real run on someone else's repository; the orchestrator creates `state.json` for STP-2 by hand
   with `build.allowed = true` after the confirmations. Answer (2026-10-02, human): as recommended.
5. **Key prefix in someone else's repository.** Recommendation: `init --prefix <ABC>`, `STP` by default.
   Answer (2026-10-02, human): the key is stored in the config, and `init` asks for it if it is not set.
   Result: the key rule is the field `tickets.key` in `stapel.toml`; `init` takes the prefix from
   `--prefix`, otherwise asks in the terminal; without a terminal and without `--prefix` — a refusal with a
   hint. There is no default `STP`. Criterion AC-12.

### Out of scope

- Removing or renaming the hooks directory through `cd` or globs (`cd .git && rm -rf hooks`, `rm .git/hook*`): the first layer only sees the literal words listed in AC-13 (finding E4-3).
- The commands `new`, `ok`, `status`, and the creation and modification of `state.json` (STP-2).
- The decision log and the token journal as commands (STP-3); this ticket's journal is kept by hand.
- A write ban through `Bash` (`echo > file`, `sed -i`, `cargo fmt`): the hook looks only at the write tools.
  This is a known hole, closed by reviewing the diff; a separate decision in phase 1.
- False refusals on heredoc text (a document embedded in a shell command): its lines are parsed as
  commands. The refusal is superfluous but safe (finding F-8, not fixed). The workaround is to write such
  text to a file.
- The first layer reads only the command text: it does not parse variable values, the output of
  substitutions, scripts in files and other interpreters (`python -c`, `perl -e`, `node -e`). Such pushes
  are caught by the second layer (`pre-push`), unless the agent bypassed it in a way that is not in AC-13.
- A push from a script run in an environment without `CLAUDECODE` that is not visible in the command text:
  `sudo` with an environment reset, `ssh`, a container, `systemd-run`, and also `os.environ` inside an
  interpreter. The first layer sees only the command text, the second only the environment variables.
- A push from another copy of the repository without `stapel init` (a fresh `git clone`): git hooks are not
  cloned.
- False refusals on reads: any word with `.git/hooks`, `hooks/pre-push` or `core.hooksPath`, including
  `cat .git/hooks/pre-push`, `git config --get core.hooksPath` and the text of a commit message in a
  heredoc, is rejected. This is deliberate: telling a read from a write by text is unreliable. Read hooks
  with the Read tool, and pass the commit message through `git commit -F <file>`.
- A push alias set up in advance through `git config alias.x push` and called as a separate command, and
  `gh pr create`, which may push the branch itself: the first layer does not see them, the second
  (`pre-push`) catches them. The decision about `gh` is in STP-6, where `stapel mr open` appears.
- A build permit for a specific ticket and with a limited term: for now `build.allowed` of any ticket opens
  writing for everyone (finding E-5). STP-2 refines the contract together with the place where the state is
  stored.
- A "reviewer without write access to git" hook: phase 0 reviewers work on a `git archive` without `.git`,
  so the hook is not needed.
- Hooks for agents other than Claude Code.
- Removing the hooks (`stapel deinit`).

## Design

**Packages.** `stapel-core` is a library: the type `Config` (serde, `toml`), the starting `stapel.toml` as a
constant, the hook decision function `decide(input, cwd, project_dir) -> Decision`. `stapel-cli` is the
binary: `clap`, the subcommands `init` and `hook pre-tool-use`, input and output. The other five are a
`lib.rs` with a comment about their purpose.

**`init`.** The root is `git rev-parse --show-toplevel`. Each file is written only if it does not exist or
its contents must change; this gives AC-4 without separate logic. The key prefix is asked for only when
`stapel.toml` does not exist yet and stdin is a terminal (`std::io::IsTerminal`); the AC-12 test supplies the
answer through stdin with the variable `STAPEL_ASSUME_TTY=1`, which is needed only for tests and is described
in `--help` as internal. `settings.json` is read as a `serde_json::Value`, a `stapel` entry is recognised by
its command line and is added only if it is not there.

**The hook.** It reads Claude Code's JSON from stdin (`tool_name`, `tool_input`, `cwd`). A refusal is code 2
and the reason in stderr: this is the documented way to forbid, and both the agent and the human see it. A
`Bash` command is split by its own small lexer (quotes, backslash and line continuation, the separators
`; & | ( )` and the line feed) into simple commands; redirections (`2>&1`, `>log`) are discarded, scripts
from `$( )`, backticks and `<( )` are parsed separately, and in the place of the substitution the word `$`
remains: its value is known only at run time, and in the place of a git subcommand it means a refusal. The
work of the check is limited by a budget (about a million units, which is milliseconds): nested `eval`s
behind wrappers can multiply the work, and when the budget is exhausted the command is rejected. A command
longer than 64 KiB or nested deeper than 16 levels is also rejected, and a panic inside the check gives a
refusal. In each simple command, reserved words (`if`, `then`, `do`, `{`, `!` and similar) and variable
assignments are skipped. Behind a wrapper or launcher (`env`, `sudo`, `timeout`, `xargs`, `ssh`, `docker` and
twenty-eight more) push is searched for from any following word, and an argument with spaces is parsed as a
separate command, without knowing the flag syntax; `eval` parses its arguments as a script; the script after
`sh -c` or `bash -c` is checked by the same parsing. For `git`, global flags (`-C`, `-c`, `--git-dir` and
similar) are skipped, `-c alias.x=…` is checked for push, then the subcommand is compared with `push`,
`send-pack`, `http-push`; programs such as `git-push` are forbidden directly.

**Write.** The target path is walked by components: each symbolic link, including a dangling one, is resolved
through `read_link` (at most 40 hops), `.` and `..` are resolved in order. The repository is the nearest
ancestor of the target with `.stapel/stapel.toml`. For `Bash` the repository is looked up from the agent's
current folder and from `CLAUDE_PROJECT_DIR`.

**The hook line.** `settings.json` gets `command -v stapel >/dev/null 2>&1 || { echo …; exit 2; }; stapel
hook pre-tool-use` with a `timeout` of 10 seconds: Claude Code runs it through `sh -c`, and without `stapel`
in `PATH` it refuses. The divergence from klc is deliberate: its hooks pass on error (rule C-002), while
here the hook protects against push, so on error it refuses.

**The second layer — `pre-push`.** Text parsing is fundamentally incomplete: it does not see substitutions,
other interpreters and scripts in files. The git hook `pre-push` rejects a push if `CLAUDECODE` is in the
environment, however the push was launched. It can be bypassed by unsetting the variables, skipping the hook
(`--no-verify`), disabling or rewriting the hook, or launching the push in a way that does not call hooks
(`send-pack`). The first layer rejects such commands when they are visible in the text (AC-13 and the words
from AC-7); what is done at run time from a script is not visible — see "Out of scope".

**`init` in two stages.** First all files are read and checked, then everything is written; a refusal on any
file leaves no partial state. `shell-words` from the original design did not fit: it does not separate `&&`
without spaces (`cd x&&git push`).

**Options that were discarded.** A regular expression over the whole line — false positives on
`echo "git push"` and `git log --grep push`. An absolute path to the binary in `settings.json` — breaks the
portability of the tracked file.

**Risks.**

| Risk | Test |
|---|---|
| R-1 Command parsing will miss a variant of `git push` | `hook::denies_git_push_variants` (the table from AC-7, extended by findings) |
| R-2 `init` will corrupt an existing `settings.json` | `hooks_install::hooks_merge_with_foreign_settings` (comparing foreign keys before and after), `hooks_install::refuses_broken_settings` |
| R-5 Claude Code stops setting `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT`: they are not in the documentation, this is observed behaviour (noticed 2026-10-02) | `hooks_install::installs_pre_push_hook`, `hooks_install::pre_push_blocks_entrypoint_env` test the hook itself; the presence of the variables is checked by hand when Claude Code is updated |
| R-4 The hook silently does not work because `stapel` is not in `PATH` (question 3) | `hooks_install::warns_when_stapel_missing_from_path`, `hooks_install::no_warning_when_stapel_on_path`, `hooks_install::hook_command_fails_closed_without_stapel`, `hooks_install::hook_command_runs_stapel_when_present` |
| R-3 A repeated `init` will overwrite files with the same contents and change the modification time | `init::second_run_changes_nothing` |

## Proof

The tool fills in the result; in phase 0 the orchestrator does, from the output of `cargo test`.

| Criterion or risk | Test | Result |
|---|---|---|
| AC-1 | `cli::workspace_has_seven_members`, `cli::version_prints_package_version`, `cli::stub_packages_are_empty` | — |
| AC-2 | `init::creates_layout_in_empty_repo`, `init::creates_layout_at_repo_root_from_subdir`, `init::appends_to_existing_gitignore` | — |
| AC-3 | `core::config::default_config_roundtrips`, `core::config::default_config_has_roles_and_sections`, `core::config::config_rejects_missing_ticket_key` | — |
| AC-4, R-3 | `init::second_run_changes_nothing` | — |
| AC-5 | `init::keeps_user_edited_config`, `init::second_run_with_other_prefix_keeps_key`, `init::refuses_unparsable_config`, `init::keeps_non_utf8_gitignore`, `hooks_install::refuses_broken_settings`, `init::refuses_non_utf8_config`, `hooks_install::refuses_non_utf8_settings` | — |
| AC-6, R-2 | `hooks_install::hooks_into_missing_settings`, `hooks_install::hooks_merge_with_foreign_settings`, `hooks_install::hooks_not_duplicated`, `hooks_install::refuses_broken_settings`, `hooks_install::upgrades_old_hook_line` | — |
| R-4 | `hooks_install::warns_when_stapel_missing_from_path`, `hooks_install::no_warning_when_stapel_on_path`, `hooks_install::hook_command_fails_closed_without_stapel`, `hooks_install::hook_command_runs_stapel_when_present` | — |
| R-5 | `hooks_install::installs_pre_push_hook`, `hooks_install::pre_push_blocks_entrypoint_env` | — |
| AC-7, R-1 | `hook::denies_git_push_variants`, `hook::allows_non_push_commands`, `hook::denies_git_push_in_compound_forms`, `hook::allows_lookalikes_after_round_one`, `hook::denies_git_push_round_two`, `hook::allows_lookalikes_after_round_two`, `hook::pathological_input_is_denied_not_crashed`, `hook::denies_git_push_round_three`, `hook::allows_lookalikes_after_round_three`, `hook::wrapper_eval_chains_are_bounded` | — |
| AC-8 | `hook::denies_code_write_without_build`, `hook::allows_stapel_and_docs_writes`, `hook::always_writable_comes_from_config`, `hook::allows_writes_outside_repo`, `hook::denies_machine_file_writes_even_with_build`, `hook::finds_repo_from_target_and_project_dir`, `hook::resolves_symlinks_before_matching`, `hook::bad_always_writable_does_not_open_everything`, `core::config::config_rejects_bad_always_writable`, `hook::denies_settings_and_case_variants`, `hook::follows_dangling_symlink`, `hook::denies_git_and_claude_writes_even_with_build`, `hook::denies_global_config_writes` | — |
| AC-9 | `hook::allows_code_write_when_build_allowed` | — |
| AC-10 | `hook::rejects_garbage_input`, `hook::passes_outside_stapel_repo`, `hook::rejects_non_string_command` | — |
| AC-11 | `init::refuses_outside_git` | — |
| AC-13 | `hooks_install::installs_pre_push_hook`, `hooks_install::keeps_foreign_pre_push`, `hook::denies_pre_push_bypass`, `hook::denies_pre_push_bypass_round_three`, `hooks_install::pre_push_blocks_entrypoint_env`, `hooks_install::skips_hooks_dir_outside_repo`, `hooks_install::repairs_pre_push_exec_bit`, `hook::denies_pre_push_bypass_round_four`, `hook::allows_lookalikes_after_round_four` | — |
| AC-12 | `init::prefix_from_flag`, `init::prefix_from_prompt`, `init::rejects_bad_prefix`, `init::refuses_without_prefix_noninteractive` | — |

## Plan

Each step is a pair of commits: RED (tests only), then GREEN (code). The check command on all steps is
`cargo test --workspace`; at RED the named tests are expected to fail (or a compilation error only in them),
at GREEN all tests are green. RED may carry what the tests need in order to compile (manifests, an empty
`main`), and edits to the ticket tables; GREEN changes tests only when fixing a mistake in a test or changing
the format that the test checks, and says so in the commit message.

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 1 | Workspace, seven packages, `stapel --version` | AC-1 | there is no binary |
| 2 | `Config` and the starting `stapel.toml` in `stapel-core` | AC-3 | there is no type `Config` |
| 3 | `init`: layout, `.gitignore`, refusal outside git, key prefix | AC-2, AC-11, AC-12 | there is no subcommand |
| 4 | `init`: repeated run and manual edits | AC-4, AC-5 | files are overwritten |
| 5 | `init`: hooks in `.claude/settings.json` | AC-6 | there are no hooks |
| 6 | Hook: `git push` ban | AC-7, AC-10 | there is no `hook` subcommand |
| 7 | Hook: write ban before the build permit | AC-8, AC-9 | writing is passed |
| 8 | `init --prefix STP` on this repository | manual check, output into the journal | — |
| R1 | Review fixes, round 1 | new tests of AC-1, AC-2, AC-5, AC-7, AC-8, R-4 | see `findings.jsonl` |
| R2 | Review fixes, round 2, and the second layer `pre-push` | new tests of AC-5..AC-8, AC-13 | see `findings.jsonl` |
| R3 | Review fixes, round 3: work budget, writes to `.git/` and `.claude/`, environment clearing | new tests of AC-7, AC-8, AC-13, R-5 | see `findings.jsonl` |
| R4 | Review round 4 fixes: env/exec option scope, global config, non-string command | new tests AC-8, AC-10, AC-13 | see `findings.jsonl` |

Dependencies: `clap`, `serde`, `serde_json`, `toml`; for tests `assert_cmd`,
`tempfile`, `predicates`.

## Review

Generated. In phase 0 — three reviewers on a `git archive` of the final commit: a fresh code reviewer, an
external reviewer, a drift reviewer.

## Summary

Generated after the merge.
