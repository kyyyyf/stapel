# STP-36 — The dialogs and the returns to the person

## Description

Source: `docs/PLAN.md`, phase 3b, ticket 3b-2 (items 3.3, 3.4, 3.7a, 3.8); ledger 2026-11-20 (the ten items
of the first cut grouped into four tickets).

### Frame

**Problem.** The process turns to the person more often than the decisions need. A grant lives 30 minutes,
so an answer after a pause costs a second dialog. Each code review round with a fix step costs one more
`stapel ok` before the fix is built, although the fix only follows a finding inside the promise. A light
ticket has the same dialogs as a full one. An agent may not edit `.stapel/stapel.toml` at all, so the
person types the change. `stapel new` gives a worktree without the project's setup (the pilot linked
`node_modules` by hand), and a session that starts on the base branch says `new-ticket` while open tickets
wait in their worktrees, so the person types which ticket to continue.

**Plan item:** 3b-2 (3.3, 3.4, 3.7a, 3.8).

**Risk tags:** `guard` (the build permit, the guard's answer for `.stapel/stapel.toml`), `interface` (the
keys `[process] dialogs` and `[worktree] setup`, the actions and lines of `stapel next`, the output of
`stapel new`).

**Non-goals.** Writes of `.stapel/stapel.toml` through Bash; dialogs in a permission mode that offers none;
other machine files (`process-ledger.jsonl`, `.claude/settings.json`); a time limit of the setup command;
removing a worktree whose setup failed; items 3.5 to 3.10; the removal of the empty crates.

**Promise (closed list).**

1. Grant lifetime: a grant stays valid 24 hours after its dialog appears.
2. Fix steps: when the newest code review round has a finding with the fate `patch`, and the only change
   since the last confirmations is new text after the end of the confirmed Plan, `stapel next` hands out
   the new steps for the build without a `stapel ok`; the Plan waits for the `stapel ok` before the Summary
   and the close. A change of any other section or of the confirmed Plan text stops the build until
   `stapel ok`, as today.
3. Dialog level: `[process] dialogs` is `all` (the default, as today) or `close`. With `close` on a ticket
   whose Plan said `Route: light` at its last confirmation, the first `stapel ok` stays; after it a change of Spec, Design, Proof or
   Plan keeps the build going on a provisional permit and waits for one `stapel ok` before the close, and a
   change of the Description stops the build until `stapel ok`. A full ticket keeps every dialog but those of
   item 2.
4. `.stapel/stapel.toml`: an Edit or Write of it by the main agent gets the Claude Code permission dialog
   instead of a denial, when the resulting file loads; it is denied as today in a mode without a dialog, for
   a subagent, and when the result does not load (with the load error).
5. The close: `stapel close` refuses while a section waits for its confirmation.
6. Worktree setup: `[worktree] setup` is a shell command that `stapel new` runs in the new worktree after
   it writes the ticket and prints the worktree, with no input and its output on stderr; its failure keeps
   the worktree and the ticket, names the failure and exits 1.
7. Open tickets at the session start: `stapel next` where no ticket of the current branch is open (the base
   branch or another branch without a ticket), and so the SessionStart hook, prints
   one line per open ticket of another worktree, with its key, title, worktree path and next action, before
   `new-ticket`.
8. The skill and the starter `stapel.toml` of `stapel init` name items 2, 3, 6 and 7.

Not promised: worktrees whose folder is gone (skipped); a setup command that never ends; files the setup
makes that the project does not ignore (they show as uncommitted, as any file).

**Prior art.** STP-2 (grants and the dialog), STP-13 and later (the build permit, `[build] requires`),
STP-28 (parallel tickets, `parallel::worktrees`), STP-32 (the page and the `stapel ok` summary).

**Size.** About 400 source lines (`grant.rs`, `next.rs` of core and cli, `stage.rs`, `guard.rs`,
`hook.rs`, `config.rs`, `new.rs`, the skill and the starter `stapel.toml`), about 500 test lines. 13 Plan
steps after the drift fixes (Decisions). Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-22: Decision (fix steps, the human, recommended): a step added to the Plan after a review round
  with a `patch` finding builds without a dialog when no other section changed since its confirmation.
  Rejected: an explicit `fix F-<n>` mark in the step; both conditions.
- 2026-11-22: Decision (`stapel.toml`, the human, recommended): the guard answers `ask` to an Edit or Write
  of `.stapel/stapel.toml`; no grant; Bash writes not promised. Rejected: a command `stapel config apply`
  with a grant.
- 2026-11-22: Decision (dialog level, the human, recommended): `[process] dialogs = "close"` keeps the
  Frame confirmation and folds the other confirmations of a light ticket into the close. Rejected: no dialog
  before the close at all; moving the setting to another ticket.
- 2026-11-22: Decision (session start and setup, the human, recommended): list every open ticket of
  another worktree before `new-ticket`; a failed setup keeps the worktree and exits 1. Rejected: removing
  the worktree; naming only the newest ticket.
- 2026-11-22: Decision (agent): the drift fixes D1-1 and D1-2 are Plan steps 12 and 13, so the Plan has
  13 steps, one past the limit of `docs/PLAN.md`; one combined step had a brief of 2031 tokens. Rejected:
  a follow-up ticket for two fixes of this ticket's own criteria.

## Spec

Terms:

- **Waiting section**: a section (owner not `generated`) whose latest confirmation is stale, or that has none.
- **Patch round**: the newest whole round of stage `code-review` in `findings.jsonl` (`review::finding_round`),
  when it has a finding whose current fate is `patch` and whose verdict is not `false`.
- **Appended Plan**: a current Plan whose normal form, split into lines, begins with every line of the
  normal form of the Plan text of its last confirmation, each line equal and in order, and has at least one
  line more; a confirmed line that was extended is a changed line.
- **Provisional section**: a waiting section that does not stop the build: under AC-2 the Plan, under AC-6
  Spec, Design, Proof and Plan. It waits for the `stapel ok` before `write-summary` and `close`.
- **Provisional permit**: the build permit of a ticket that has a waiting required section (`[build]
  requires`) and whose waiting required sections are all provisional; `stapel status` prints it as
  `build: provisional <KEY>`.
- **Dialog mode**: a permission mode of `dialog::DIALOG_MODES` (`default`, `acceptEdits`, `auto`, `plan`).

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the guard issues a grant for a dialog THE grant expires 86400 seconds after it is issued; a grant used before then is accepted and one used after is refused with `stapel: the grant expired; ask the agent to run the command again`, as today. | `ok::grant_lives_24_hours` |
| AC-2 | WHEN the ticket has a patch round, the Plan is its only waiting section and it is an appended Plan THE `stapel next` prints the action its history gives (such as `build-step`) instead of `write-page` or `confirm`, with the fact line `provisional: <ids joined by ", "> wait for stapel ok <KEY> before the close`, printed only while one waits; when the Plan is in `[build] requires` the guard allows code writes and `stapel status` prints `build: provisional <KEY>`. | `next_provisional::fix_steps_build_without_a_dialog` |
| AC-3 | IF in the state of AC-2 another section also waits, or a line of the confirmed Plan changed (the Plan is not an appended Plan), or the newest round has no patch finding (round 2 without one included), or the Plan was never confirmed THEN `stapel next` prints `write-page` or `confirm` as today. | `next_provisional::fix_steps_build_without_a_dialog` |
| AC-4 | WHEN a provisional section waits and the history gives `write-summary` or `close` THE `stapel next` prints `write-page` or `confirm` with the confirmation facts instead. | `next_provisional::close_waits_for_the_ok` |
| AC-5 | IF a section whose owner is not `generated` waits when `stapel close` runs THEN it exits 1 with `stapel: <ids joined by ", "> wait for stapel ok <KEY>` and writes nothing, and the guard refuses the close dialog with the same text before it issues a grant; WHEN no such section waits THE close is as today. | `close::refuses_waiting_sections` |
| AC-6 | WHEN `[process] dialogs` is `close`, the Plan text of the Plan's last confirmation says `Route: light`, every section was confirmed once and the Description is fresh THE waiting Spec, Design, Proof and Plan are provisional: the guard allows code writes, `stapel status` prints `build: provisional <KEY>`, and `stapel next` goes on by its history, with the fact line of AC-2, until AC-4. | `next_provisional::light_ticket_at_level_close` |
| AC-7 | IF the key is `close` and the Description waits, or the confirmed Plan text does not say `Route: light` (a current Plan changed to `light` included), or a section was never confirmed, or a section other than Spec, Design, Proof and Plan waits THEN the permit and `stapel next` are as with `all`; WHEN `[process] dialogs` is absent THE level is `all`; IF it is another word THEN loading `stapel.toml` fails with `process.dialogs must be all or close: <value>`. | `next_provisional::light_ticket_at_level_close`, `cli_config::process_dialogs` |
| AC-8 | WHEN the main agent's Edit, MultiEdit or Write of `.stapel/stapel.toml` comes (the path resolved as the guard resolves every path, so `./`, `..`, an absolute path and a symlink to it count, and compared without case) in a dialog mode and the file it would leave loads THE guard answers `ask` with the reason `an edit of .stapel/stapel.toml: the person confirms it in this dialog` and leaves the tool input unchanged. | `hook::stapel_toml_edit_asks` |
| AC-9 | IF that edit comes from a subagent (the refusals are judged in this order: subagent, mode, apply, load) (the hook input has `agent_id`) THEN the guard denies with `only the main agent edits .stapel/stapel.toml, through the person's dialog`; IF the mode is not a dialog mode THEN it denies with `an edit of .stapel/stapel.toml needs the permission dialog, which Claude Code does not show in permission mode "<mode>"`; IF an Edit's `old_string` is not found exactly once (or once with `replace_all`) THEN it denies with `the edit of .stapel/stapel.toml does not apply`; IF the result does not load THEN it denies with `.stapel/stapel.toml would not load: <error>`; `process-ledger.jsonl` and `.claude/settings.json` stay denied. | `hook::stapel_toml_edit_refusals` |
| AC-10 | WHEN `[worktree] setup` holds a command and `stapel new` makes a worktree THE `stapel new` prints its `worktree:` line, then runs the command through `sh -c` in the new worktree with an empty input and its output (stdout and stderr) on stderr, then prints `setup: ok` and exits 0; WHEN the key is absent THE output is as today; IF it is empty or only whitespace THEN loading `stapel.toml` fails with `worktree.setup is empty: name a shell command`. | `new_branch::runs_the_setup` |
| AC-11 | IF the setup command exits with a status other than 0 or cannot start THEN `stapel new` keeps the worktree, the branch and the ticket, prints `stapel: the setup command failed (<status>); the worktree is kept: <path>`, where `<status>` is `exit <n>`, `signal <n>` or `could not start: <error>`, and exits 1. | `new_branch::failed_setup_keeps_the_worktree` |
| AC-12 | WHEN `stapel next` or the SessionStart hook runs where no ticket of the current branch is open THE output has, before the `next:` line, one line `open: <KEY> — <title> — <worktree path> — next: <action>` per open ticket on the branch of another worktree, sorted by key; the current worktree, a closed ticket and a worktree whose folder is gone are left out; IF the ticket or the next action of another worktree cannot be read THEN its line is `open: <branch> — <worktree path> — next: unreadable` and the rest of the output is as without it; with none the output is as today. | `parallel::names_open_tickets_of_other_worktrees`, `hook_session::names_open_tickets` |
| AC-13 | WHEN `stapel init` writes the skill and the starter `stapel.toml` THE skill names the provisional sections of AC-2 and AC-6, `[process] dialogs`, `[worktree] setup` and the `open:` lines of AC-12, and the starter `stapel.toml` has the keys `dialogs` and `setup` as comments. | `init_process::names_dialogs_and_setup` |

### Guarantees

- **Promised:** item 1 by AC-1, item 2 by AC-2 to AC-4, item 3 by AC-6 and AC-7, item 4 by AC-8 and AC-9,
  item 5 by AC-5, item 6 by AC-10 and AC-11, item 7 by AC-12, item 8 by AC-13. Unchanged: the dialogs of a
  ticket without a patch round at level `all`; the `stapel ok` dialog and its grant other than the lifetime;
  the guard for every other path; `stapel new` without the key.
- **Not promised:** the Frame's list; a dialog for a Bash write of `.stapel/stapel.toml` (it stays
  unguarded, as today); an Edit whose file changed between the hook and the tool.

## Test plan

Integration tests in `crates/stapel-cli/tests/`: the new file `next_provisional.rs`, and `ok.rs`,
`close.rs`, `cli_config.rs`, `hook.rs`, `new_branch.rs`, `parallel.rs`, `hook_session.rs`,
`init_process.rs`. Light tests: `stapel cargo test -j 4 -p stapel-cli --test <file> -- --test-threads=4`;
before each GREEN also `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Grant lifetime | the `expires_at` of a grant the hook issues is the test's clock plus 86400 s, within 5 s; the grant is accepted at once | `ok::grant_lives_24_hours` |
| Fix steps | a patch round 1, a step appended to the Plan (also with the last confirmed line extended, which stops): `build-step` of the new step and the fact `provisional: plan wait for stapel ok STP-1 before the close`, absent once the Plan is confirmed; the same with the patch finding in round 2; the Plan in `[build] requires`: a hook Write of `src/x.rs` allowed and `build: provisional STP-1`. Stops: Spec changed too; Design changed; an old step's test name changed; the `Route:` line changed; a round 2 with no patch finding after a patch round 1; the finding's fate `defer` only; before any confirmation | `next_provisional::fix_steps_build_without_a_dialog` |
| Close waits | all steps built and the review written, the Plan waiting: `confirm` (or `write-page`) instead of `write-summary`; with the Summary written: instead of `close` | `next_provisional::close_waits_for_the_ok` |
| Close refuses | `stapel close` with the Plan waiting; with Spec waiting; the hook dialog of `stapel close` refuses the same before a grant file exists; every section fresh: closes as today | `close::refuses_waiting_sections` |
| Level close | a light ticket, `dialogs = "close"`, Spec and Plan changed after the first ok: a hook Write allowed, `build: provisional STP-1`, `build-step`, the fact naming `spec, plan`; every section fresh: no fact. Stops: the Description changed; a confirmed Plan with `Route: full`, also when the current Plan says `light`; no `Route:` line; a section never confirmed; a waiting custom section of `[build] requires`; level `all` | `next_provisional::light_ticket_at_level_close` |
| Level key | absent; `all`; `close`; `Close`; `none`; `""` | `cli_config::process_dialogs` |
| toml ask | Edit, MultiEdit (two edits, the second on the text of the first) and Write of `.stapel/stapel.toml` in modes `default`, `acceptEdits` and `auto`; the paths `.STAPEL/Stapel.toml`, `./.stapel/stapel.toml`, `crates/../.stapel/stapel.toml`, the absolute path and a symlink to it; the answer `ask` with the tool input unchanged | `hook::stapel_toml_edit_asks` |
| toml refusals | `agent_id` set; mode `bypassPermissions` and no mode; `old_string` absent and found twice; a result with `[process] dialogs = "x"`; a result that is not TOML; Write of `process-ledger.jsonl` and `.claude/settings.json` | `hook::stapel_toml_edit_refusals` |
| Setup | `touch setup-ran` runs in the worktree, `setup: ok`, exit 0; no key: the output of today; `""` and `"  "`: load error | `new_branch::runs_the_setup` |
| Setup fails | `exit 3` (`exit 3`); a command that is not found (`exit 127`); `kill -KILL $$` (`signal 9`); the worktree, branch and `ticket.md` exist after | `new_branch::failed_setup_keeps_the_worktree` |
| Open tickets | a ticket worktree with a corrupt `state.json` (`next: unreadable`, and the `next:` line still follows); two ticket worktrees, one closed, one whose folder was removed: one `open:` line with the next action of that worktree; run from main and from a worktree of a non-ticket branch; no open ticket: as today | `parallel::names_open_tickets_of_other_worktrees`, `hook_session::names_open_tickets` |
| Init | the skill and the starter `stapel.toml` that `stapel init` writes | `init_process::names_dialogs_and_setup` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `[process] dialogs` | string | `all`, `close` | absent: `all` | another word, other case, `""`: load error | exact | `cli_config::process_dialogs` |
| `[worktree] setup` | string, a shell command | one character / no limit | absent: no setup; `""` or whitespace: load error | a failing or missing command: AC-11 | run as given through `sh -c` | `new_branch::runs_the_setup`, `new_branch::failed_setup_keeps_the_worktree` |
| Hook input of Edit, MultiEdit, Write | JSON: `tool_name`, `tool_input` (`file_path`, `old_string`, `new_string`, `replace_all`, `edits`, `content`), `permission_mode`, `agent_id` | — | no `permission_mode`: refused as not a dialog mode | `old_string` not found once: refused | exact string match, as the tool does | `hook::stapel_toml_edit_asks`, `hook::stapel_toml_edit_refusals` |
| Grant files | JSON with `expires_at` | — | — | unreadable or expired: refused, as today | whole seconds | `ok::grant_lives_24_hours` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | permission mode | a dialog mode asks; another mode or none is denied | `hook::stapel_toml_edit_refusals` |
| claude-code | main agent or subagent (`agent_id` in the hook input) | a subagent is denied | `hook::stapel_toml_edit_refusals` |
| claude-code | allow rules that skip the dialog | Assumption: a hook `ask` shows the dialog whatever the allow rules of Edit say; not promised | — |
| claude-code | environment of hook processes, transcripts, tool success | not read by this ticket | — |
| git | linked worktrees, including ones whose folder is gone | an `open:` line per live worktree; a gone folder is skipped | `parallel::names_open_tickets_of_other_worktrees` |
| git | detached HEAD in another worktree | no ticket branch: no line | `parallel::names_open_tickets_of_other_worktrees` |
| git | paths with spaces in the worktree path | the setup runs with the worktree as its working folder, not in the command text | `new_branch::runs_the_setup` |
| git | history shape, object format, submodules, other dimensions | unchanged by this ticket | — |
| os | case-insensitive file systems | the path of `.stapel/stapel.toml` matches in any case | `hook::stapel_toml_edit_asks` |
| os | processes: a setup command that waits for input or never ends | not promised (Frame) | — |
| os | time: clock skew | the grant uses wall-clock seconds, as today; not promised | — |
| cargo | every dimension | not read by this ticket | — |

**Review Focus:** a setup start failure (`could not start:`) is not tested, since `sh` is always there; two sessions that issue grants for the same ticket within 24 hours (each grant is bound to
its facts; not tested); a MultiEdit whose second edit depends on the first (applied in order, as the tool
does; tested once); a patch round whose fix step also edits the Proof table (stops the build by AC-3; the
review judges whether that is too strict); the SessionStart hook in a repository with many worktrees (one
`stapel next` computation per worktree; not measured).

### Abuse table

| Attempt of a careless agent | Outcome | Test |
|---|---|---|
| edits the Spec while it builds a fix step, so that the criterion fits the code | the Spec waits; the build stops at `confirm` | `next_provisional::fix_steps_build_without_a_dialog` |
| rewrites an old Plan step or adds a feature step after a patch round | an old line changed stops the build; an appended step builds and waits for the ok before the close, which shows the Plan diff | `next_provisional::fix_steps_build_without_a_dialog` |
| runs `stapel close` while the Plan waits for its ok | refused, exit 1 | `close::refuses_waiting_sections` |
| a subagent edits `.stapel/stapel.toml` | denied | `hook::stapel_toml_edit_refusals` |
| writes a `stapel.toml` that does not load | denied with the load error | `hook::stapel_toml_edit_refusals` |
| writes the path as `.STAPEL/stapel.TOML`, `./.stapel/stapel.toml` or through a symlink | the same dialog | `hook::stapel_toml_edit_asks` |
| sets `[process] dialogs = "close"` for itself | the edit goes through the person's dialog | `hook::stapel_toml_edit_asks` |
| writes `Route: light` into the Plan of a full ticket at level `close` | the route comes from the confirmed Plan text; the build stops | `next_provisional::light_ticket_at_level_close` |
| appends to `.stapel/stapel.toml` through Bash | not guarded, as today (Non-goals) | — |

**Author self-check (item 7).** Done on 2026-11-22: each row of the Abuse table maps to a promise item or
to the Not promised list; the provisional permit never covers the Description, so the promise cannot widen
without a dialog.

## Design

### Facts

- F-1: `grant.rs:13` sets `LIFETIME = 30 * 60`; `issue` writes `expires_at = now + LIFETIME`, and
  `consume_with` refuses a grant past it.
- F-2: `next.rs::decide` (core, about line 430) collects a confirmation fact for every section whose owner
  is not `generated` and returns `write-page` or `confirm` before any history action while one exists.
- F-3: `Facts` of `next.rs` carries `review_round` and `patch_finding` (lines 127 and 129);
  `after_the_steps` (line 323) gives `code-review`, `write-summary` and `close`.
- F-4: the guard's `decide_write` (`guard.rs:195`) denies `.stapel/stapel.toml` through `is_machine_file`
  before any permit; the permit is `stage::build_permit` (`stage.rs:179`), which needs every section of
  `[build] requires` fresh; this repository's list leaves out the Plan.
- F-5: `guard::Decision::Ask` carries a replacement `command`; `hook.rs:30` writes it as
  `updatedInput.command`, which only fits Bash.
- F-6: `close.rs` checks the base merge, the check and the merge review, not the confirmations.
- F-7: `new.rs::create` makes the worktree with `add_worktree` and writes `ticket.md` and `state.json`
  there; nothing runs after it. `config.rs::Config` has no `[process]` or `[worktree]` table.
- F-8: the SessionStart hook (`hook.rs:53`) prints `next::text_at(root)`; on the base branch the tickets
  folder of that checkout holds no ticket that lives only on a ticket branch, so it prints `new-ticket`.
  `parallel::worktrees` lists the worktrees with `prunable`, and `parallel.rs` (`go_on_notices`) already
  walks them and skips closed tickets.

### Decisions (before the build)

- The patch round is a new fact of `next::Facts` computed from `review::finding_round` and the findings of
  that round (fate `patch`, verdict not `false`); the existing `patch_finding` (round 1 only) stays for
  `review::next_round`.
- One function `stage::provisional(config, sections, state, facts)` returns the provisional sections of
  AC-2 and AC-6; `next::decide`, `stage::build_permit`, `status` and `close` all read it, so the four
  cannot disagree. `Permit` gets a third case `Provisional(key)`.
- `next::decide` drops the confirmation facts of provisional sections and adds the `provisional:` fact;
  `after_the_steps` returns `confirm` or `write-page` in place of `write-summary` and `close` while a
  provisional section waits.
- `guard::Decision` gets a case `AskEdit(reason)`; the hook prints `permissionDecision: "ask"` with no
  `updatedInput`. The guard computes the resulting text (Write: `content`; Edit: one replacement or all
  with `replace_all`; MultiEdit: the `edits` in order) and loads it with `Config::parse_at`.
- `[worktree] setup` runs with `std::process::Command::new("sh").arg("-c")`, the worktree as its working
  folder, stdout and stderr to stapel's stderr.
- The `open:` lines come from `stapel next` itself, so the hook and the command print the same; the next
  action of another worktree is `next::text_at(worktree)`'s action word.
- Assumption (2026-11-22, agent): the hook input of a subagent's tool call has `agent_id`; the main
  agent's has none. The builder checks this against the Claude Code hook documentation and stops with an
  `intent_gap` if it is wrong.
- 2026-11-22, agent (SR1-7): the edit dialog uses the same dialog modes as `stapel ok`. In this repository
  the `stapel ok` dialog reached the person in mode `auto` in STP-32 to STP-35, so a hook `ask` is shown
  there. For `acceptEdits` the builder checks the Claude Code hook documentation (a hook `ask` prompts
  whatever the mode accepts) and stops with an `intent_gap` if it says otherwise.
- 2026-11-22, agent (SR1-5): a fixture of `close.rs` and `flow.rs` that closes with a section never
  confirmed after this ticket fails AC-5; if such fixtures exist, the builder of step 5 lists them in
  its report, and their change goes to main as a test-only commit (ledger 2026-11-20, test-refactor step).
- 2026-11-22, agent (SR1-9): the setup's files are the project's to ignore in `.gitignore`; the skill says
  so (AC-13); `stapel new` does not read them.
- 2026-11-22, agent (SR1-12): the stop cases of AC-3 and AC-7 live in the tests of AC-2 and AC-6, whose
  positive case fails at the RED, so no step is a coverage step.

### Decisions (as built)

- `grant::LIFETIME` is `24 * 60 * 60`; the test reads `expires_at` of the grant the hook issues.
- `config.rs::Process { dialogs }` (default `all`) and `config.rs::Worktree { setup }`; the empty setup
  fails to load with `worktree.setup is empty: name a shell command`.
- `review::patch_round` gives the patch round; `stage::{appended_plan, provisional}` and
  `Permit::Provisional`; `provisional` is `level_close || fix_steps`, and `level_close` reads
  `next::is_light` of the Plan's last confirmed text. `build_permit` prefers a `Computed` permit and falls
  back to `Provisional`. The `provisional:` line has no `blocked:` prefix and is printed on the history path.
- `history_action` and `after_the_steps` take the confirmation facts of a provisional permit;
  `after_the_steps` returns `confirm` or `write-page` after the head-check and merge-review gates, before
  `write-summary` and before `close`.
- `stage::close_refusal` is called by `close.rs` right after the status match and by the close branch of
  `dialog.rs` before the grant.
- `guard::Decision::AskEdit`, `edited_text` and `toml_edit_ask` run in `decide` before `decide_write`;
  the refusals come in the order subagent, mode, apply, load; the hook prints `ask` without `updatedInput`.
  The step 7 builder read the Claude Code hooks page: a subagent's PreToolUse input has `agent_id`, and
  `ask` prompts the user; nothing says `acceptEdits` skips it (the Assumption holds).
- `new.rs::run_setup` runs `sh -c` with a null stdin and both streams on stderr, after the `worktree:` line.
  A worktree path with spaces is not tested; the path goes through `current_dir`, not the command text.
- `next.rs::open_lines` (cli) runs in the `new-ticket` branch, text mode only; the `open:` lines follow a
  `can go on:` notice and precede `next: new-ticket`; the action is the last line of `text_at(worktree)`.
- The starter `stapel.toml` is built by `config.rs::starter_toml`, not by a file under `assets/`; the
  commented `[process]` and `[worktree]` keys are the constant `PROCESS_COMMENT` there.
- 2026-11-22, agent (SR1-5): AC-5 broke 35 existing tests in 14 files (7 in `close.rs` and `records.rs`, then 28 in 12 more) whose fixtures closed a ticket with
  sections never confirmed. Two test-only commits went to main, `0fc7a9a` and `1d3ef6e` (helpers
  `confirm_sections`, `confirm_these`, `confirmed_repo_with_ticket`); the human fast-forwarded main both
  times. `decisions::ok_and_close_append_a_decision` now expects one decision per `stapel ok` (five), the
  spec ok first and the close last. The branch was rebuilt twice so that each merge of main (`673c823`,
  `9dac874`) comes before the RED of a step that touches the same test file.
- 2026-11-22, agent: three REDs changed existing tests to the new behaviour: step 8
  `hook::denies_machine_file_writes_even_with_build` and `hook::denies_settings_and_case_variants` (a write
  of `.stapel/stapel.toml` with no permission mode now gets the mode refusal); step 10
  `parallel::can_go_on_notice` (the `open: ABC-2` line between the notice and `next: new-ticket`).
- 2026-11-22, agent: Plan step 8 was shortened, same meaning, because AC-9 grew its brief past 1600 tokens;
  confirmed by `stapel ok`.
- 2026-11-22, agent: the step 3 builder edited three source files with a Python script instead of the Edit
  tool while the build was allowed; no guard was bypassed; later builders used Edit and Write only.
- Pre-existing failures on this Mac with the default TMPDIR: `next::refuses_a_wrong_call`,
  `status::refuses_ambiguous_case_variant_keys`, `records::dialog_refuses_an_unreadable_config`,
  `records::import_refuses_bad_input_path` (case-insensitive and `/private/var` paths); every check ran
  with a case-sensitive TMPDIR.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `ok::grant_lives_24_hours` |
| AC-2 | `next_provisional::fix_steps_build_without_a_dialog` |
| AC-3 | `next_provisional::fix_steps_build_without_a_dialog` |
| AC-4 | `next_provisional::close_waits_for_the_ok` |
| AC-5 | `close::refuses_waiting_sections` |
| AC-6 | `next_provisional::light_ticket_at_level_close` |
| AC-7 | `next_provisional::light_ticket_at_level_close`, `cli_config::process_dialogs` |
| AC-8 | `hook::stapel_toml_edit_asks` |
| AC-9 | `hook::stapel_toml_edit_refusals` |
| AC-10 | `new_branch::runs_the_setup` |
| AC-11 | `new_branch::failed_setup_keeps_the_worktree` |
| AC-12 | `parallel::names_open_tickets_of_other_worktrees`, `hook_session::names_open_tickets` |
| AC-13 | `init_process::names_dialogs_and_setup` |

## Plan

Route: full (tags `guard`, `interface`), auto.

1. Grant lifetime (AC-1; test `ok::grant_lives_24_hours`, which fails at the RED because a grant expires
   after 30 minutes): the GREEN sets `LIFETIME` to 24 hours. Must not change: the grant's binding to its facts.
2. Dialog level key (the load part of AC-7; test `cli_config::process_dialogs`, which fails at the RED
   because `[process]` is unknown): the GREEN adds `config.rs::Process` with `dialogs`. Must not change:
   the other tables.
3. Fix steps (AC-2, AC-3; test `next_provisional::fix_steps_build_without_a_dialog`, red on `confirm`):
   `stage::provisional`, `Permit::Provisional`, used by `next`, the permit and `status`. Must not change:
   a ticket without a patch round.
4. The close waits (AC-4; test `next_provisional::close_waits_for_the_ok`, which fails at the RED because
   step 3 lets `write-summary` through): the GREEN makes `after_the_steps` return the confirmation first.
   Must not change: `code-review` and the check points.
5. Close refusal (AC-5; test `close::refuses_waiting_sections`, which fails at the RED because the close
   reads no confirmation): the GREEN adds the refusal to `close.rs` and to the close dialog. Must not
   change: the other close gates.
6. Level close (AC-6, AC-7; test `next_provisional::light_ticket_at_level_close`, which fails at the RED
   because only the Plan can be provisional; its stop cases hold before and after): the GREEN widens
   `stage::provisional` with the route of the confirmed Plan text. Must not change: level `all`.
7. stapel.toml dialog (AC-8; test `hook::stapel_toml_edit_asks`, which fails at the RED because the
   guard denies every edit of the file): the GREEN adds `Decision::AskEdit` and the result computation,
   and asks only for the main agent in a dialog mode with a result that loads; every other case keeps
   today's denial until step 8. Must not change: the other machine files.
8. Refusals (AC-9; test `hook::stapel_toml_edit_refusals`, red on the old texts): the four refusals.
   Must not change: AC-8.
9. Worktree setup (AC-10, AC-11; tests `new_branch::runs_the_setup`,
   `new_branch::failed_setup_keeps_the_worktree`, which fail at the RED because the key is unknown): the
   GREEN adds `[worktree] setup` and runs it in `new.rs`. Must not change: `stapel new` without the key.
10. Open tickets (AC-12; tests `parallel::names_open_tickets_of_other_worktrees`,
    `hook_session::names_open_tickets`, which fail at the RED because the base branch prints only
    `new-ticket`): the GREEN adds the `open:` lines to `stapel next`. Must not change: the output with no
    open ticket elsewhere.
11. Skill and starter (AC-13; test `init_process::names_dialogs_and_setup`, which fails at the RED because
    the skill and the starter do not name the keys): the GREEN edits `crates/stapel-cli/assets/` (the skill
    and the starter `stapel.toml`) and runs `stapel init` here, so that
    `init_process::this_repository_has_the_current_files` passes. Must not change: the other assets.
12. Fact line D1-1 (AC-2; test `next_provisional::fix_steps_build_without_a_dialog`, red on the old
    line): the line names the waiting sections, only while one waits. Must not change: the permit.
13. Four sections D1-2 (AC-7; test `next_provisional::light_ticket_at_level_close`, red on a waiting
    custom section): level `close` covers only Spec, Design, Proof and Plan. Must not change: AC-6.

## Review

TODO

## Summary

TODO
