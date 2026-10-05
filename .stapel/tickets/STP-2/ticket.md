# STP-2 — `new`, `ticket.md`, `state.json`, section hashes and confirmations, `status`, `close`

## Description

Source: `docs/PHASES.md`, phase 0, second ticket. No external tracker.

A person creates a ticket with `stapel new`, edits `ticket.md`, confirms sections with `stapel ok`, closes the
ticket with `stapel close`, and sees with `stapel status` whose decision is needed and which confirmations went
stale. The stage is never stored: it is computed from facts (PLAN.md principle 3). Closes phase-0 criteria 2
and 3. The permit to write code becomes computed from confirmations; the hand-kept `build.allowed` flag from
STP-1 is kept beside it until phase 1 removes it.

**How people use it.** Nobody types these commands. The agent runs `init`, `new` and `status`. A person's two
decisions, "confirm this section" and "close this ticket", reach `stapel` through one of two channels the agent
cannot fake: the Claude Code permission dialog (in the terminal and, if confirmed, in the Zed agent panel), or
a Zed task or key binding, which runs outside the agent's shell. `close` is the phase-0 stand-in for `ship`:
in phase 1 `ship` writes the same fact.

**Risk tags:** `data` (new formats `state.json`, `ticket.md` structure, grant files), `interface` (new commands,
their output), `guard` (the guard reads the permit, turns `ok` and `close` into a permission dialog, and must
stop agents from confirming). By CLAUDE.md all early checks are mandatory.

## Spec

### Guarantees

- **To the people on a ticket:** a confirmation records who confirmed which section, when, the exact content
  (by hash) they saw, and the dependency hashes at that moment. If that content or a recorded dependency
  changes, `status` says so and, for the section itself, shows the difference. Reverting the content makes the
  confirmation fresh again.
- **Only a person confirms or closes** (decided by the orchestrator while the human was away; confirmed by the human on 2026-10-05). In an agent's shell `stapel ok` and `stapel close` work only with a
  one-time grant that the PreToolUse guard issues after the person answers "yes" in the Claude Code permission
  dialog, in the permission modes listed in AC-15. This is verified for the Claude Code terminal in `auto`
  mode; the other listed modes and the Zed agent panel are checked in Plan step 11. Outside an agent's shell
  (a person's terminal, a Zed task) they need no grant.
- **To the write guard:** code writes are allowed only while some *open* ticket has fresh confirmations of
  every section in `build.requires` (default: spec, design, proof), computed at the moment of the call, or,
  during phase 0, while some ticket's `state.json` without a `closed` fact carries `build.allowed = true`.
  A closed ticket gives no permit; `close` also removes the hand-kept flag.
- `status` never writes anything.
- **Against:** honest mistakes; hand edits of `ticket.md`; editors that change line endings or Unicode form; a
  crash in the middle of a write; an agent that runs `ok` or `close` from its shell or from Bash in any form
  while `CLAUDECODE` or `CLAUDE_CODE_ENTRYPOINT` is present, without the person's "yes"; an agent that edits
  the section between the dialog and the confirmation; an agent that rewrites a grant file.
- **Not against:**
  - an agent that deliberately forges files through Bash: `state.json`, or a grant file with a token it chose
    (the write tools are denied on both; Bash writes are the declared STP-1 hole);
  - an agent that runs `ok` from an interpreter or script after removing `CLAUDECODE` and
    `CLAUDE_CODE_ENTRYPOINT` from its environment there (the same hole STP-1 declares for push); such a
    confirmation is recorded with `via: terminal` and shows in the git history;
  - an agent that answers the dialog by injecting terminal input with a tool not listed in AC-17;
  - a person who answers the dialog without reading it, or chooses "don't ask again" if Claude Code then
    stops asking (checked in Plan step 11; the guard denies when it finds such an allow rule, AC-16);
  - the Zed agent panel until Plan step 11 shows that it does not auto-approve an `ask`;
  - an agent that runs the `stapel` binary's hook entry point after removing `CLAUDE_CODE_EXECPATH` from
    its environment inside a script or interpreter: the guard denies `stapel hook` where the text shows
    it, and the hook issues no grant while that variable is present (code review F-1, F2-1, F2-2);
  - a `$` inside a single-quoted closing reason: it is denied as a run-time word (code review F2-4);
  - terminal input injection written into a file and run from there, or spelled otherwise than AC-17
    lists;
  - two different `stapel` builds of the same version: the grant is bound to the version, not to
    the build (code review D-3);
  - an agent reverting `ticket.md` to an earlier confirmed text: the person sees it only if their newer text
    was committed;
  - a hard line break written as two trailing spaces: trailing whitespace is not content (AC-8);
  - dependencies added to `stapel.toml` after a confirmation: a confirmation keeps those it was made with.

### Acceptance criteria

| № | Criterion | Test |
|---|---|---|
| AC-1 | A person or agent · runs `stapel new "<title>"` · in a repository with a valid `.stapel/stapel.toml` · and gets `.stapel/tickets/<KEY>/ticket.md` with an ATX level-2 heading (AC-9) for Description and for every section in `stapel.toml`, in that order, each followed by the line `TODO`, and `state.json` with `schema_version = 1`, the key, the title and an empty confirmation list; `<KEY>` is `tickets.key` with `{n}` = 1 + the largest number among existing ticket folders of that prefix (prefix compared without case, numbers without leading zeros); the command prints `created: <KEY>` and exits 0. The title must be non-empty after trimming and contain no control characters. | `new::creates_ticket_files`, `new::numbers_after_largest_existing`, `new::numbering_ignores_case_and_zero_padding`, `new::refuses_bad_title` |
| AC-2 | `stapel new "<title>" --tracker <url>` · stores the URL, which must start with `http://` or `https://` and contain no whitespace or control characters, in `state.json` and as the first line of Description, `Tracker: <url>`; without `--tracker` neither appears. Nothing is fetched. | `new::stores_tracker_link`, `new::refuses_bad_tracker_url` |
| AC-3 | `stapel new` · when the ticket folder for the computed key exists, or `stapel.toml` is missing or invalid (AC-13) · refuses with exit 1, names the reason and writes nothing. | `new::refuses_existing_key`, `new::refuses_without_config` |
| AC-4 | `status`, `ok` and `close` · take an optional ticket key, matched without case; without a key they use the only open ticket, and with zero or several open tickets list the open keys and exit 1. A ticket is **open** when its `state.json` is schema 1 and has no `closed` fact, **closed** when it has one, **legacy** when it has no `schema_version` (STP-1), and **unreadable** otherwise; legacy and unreadable tickets are never the implicit target and are reported by `status` when named. When the given or resolved key matches more than one folder (folders differing only in case), the command refuses with exit 1. | `status::fresh_ticket_waits_for_spec`, `status::key_matched_without_case`, `status::without_key_needs_one_open_ticket`, `status::legacy_ticket_is_not_open`, `status::refuses_ambiguous_case_variant_keys`, `ok::resolves_single_open_ticket` |
| AC-5 | `stapel ok [KEY] <section>` · appends to `state.json` a confirmation `{section, by, at, hash, normal_form, depends_on: {<id>: hash}, text}`: `by` is `git config user.name` read in the repository root, trimmed; `at` is UTC RFC 3339; `hash` is `sha256:<hex>` of the normal form; `normal_form` is `1`; `depends_on` holds the current hashes of the section's `depends_on` from `stapel.toml`; `text` is the normalized section text; `via` is `grant` or `terminal`. Only the latest confirmation of a section keeps `text`. It prints `confirmed: <KEY> <section> (<hash prefix>)` and exits 0. It refuses with exit 1 and a reason: for a section whose owner is `generated` or an unknown id (listing valid ids); on a closed ticket; when `user.name` is unset or blank; when the section or one of its dependencies is missing or duplicated; and when `CLAUDECODE` or `CLAUDE_CODE_ENTRYPOINT` is present in the environment (any value, empty included) without a valid grant (AC-16); that refusal (decided by the orchestrator while the human was away; confirmed by the human on 2026-10-05) names `env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT stapel ok <section>` for a Zed task started from a Claude Code shell. When `user.name` contains whitespace it warns on stderr that the name goes into a tracked file. | `ok::records_confirmation`, `ok::keeps_text_only_on_latest`, `ok::refuses_generated_and_unknown_sections`, `ok::refuses_on_closed_ticket`, `ok::refuses_without_identity`, `ok::refuses_blank_identity`, `ok::warns_on_name_with_space`, `ok::refuses_missing_dependency`, `ok::refuses_in_agent_shell_without_grant`, `ok::confirming_twice_keeps_history`, `ok::refuses_oversized_ticket` |
| AC-6 | `stapel status` · after a confirmation of spec and an edit of the spec section · shows `stale: spec — changed since confirmed by <by> at <at>` followed by a unified line diff between the confirmed and the current normalized text; after reverting the edit the confirmation is fresh again. | `status::edit_makes_confirmation_stale_with_diff`, `status::revert_makes_it_fresh_again` |
| AC-7 | The latest confirmation of a section (the last for that id in array order; `at` is informative) · is fresh when the section's current hash and the current hash of every id recorded in its `depends_on` equal the recorded ones; otherwise stale with one reason per cause: `changed since confirmed`, `depends on <id>, which changed`, `depends on <id>, which is no longer configured`, `depends on <id>, which is missing`, `depends on <id>, which is duplicated`, `section missing`, `section duplicated`, `confirmed under normal form <n>`. | `status::upstream_change_makes_dependents_stale`, `status::missing_or_duplicate_section_is_stale`, `status::removed_dependency_is_stale`, `status::older_normal_form_is_reported`, `status::dependency_missing_or_duplicated_is_named` |
| AC-8 | The normal form (version 1) of a section · strips a leading UTF-8 BOM, turns CRLF and lone CR into LF (parsing does the same, AC-9), applies Unicode NFC, strips trailing spaces and tabs on each line, drops blank lines at the start and end, and excludes the heading line; any other change, including whitespace inside a line, changes the hash. | `hash::ignores_non_content_differences`, `hash::detects_content_changes`, `hash::normal_form_is_idempotent`, `hash::any_inner_change_changes_hash` |
| AC-9 | `ticket.md` parsing · treats as a section heading only a line matching `^ {0,3}##[ \t]+(.+?)[ \t]*(#+[ \t]*)?$` outside fenced code blocks and outside HTML comment blocks (from a line starting, after up to three spaces, with `<!--` to the next line containing `-->`), and finds a section by the heading whose text equals the section `title` from `stapel.toml` (compared with ASCII case folding); lines end at LF, CRLF or a lone CR; the file is read only as a regular file of at most 4 MiB; a fence opens with three or more backticks or tildes and closes only with a fence of the same character at least as long; setext headings and indented code are not headings; a closing run of `#` is dropped from the title; a section ends at the next level-2 heading; order does not matter. A non-UTF-8 or missing `ticket.md` is reported by `status` and refused by `ok`. | `parse::finds_sections_by_title`, `parse::ignores_headings_in_code_blocks`, `parse::fence_variants`, `parse::setext_and_comments_are_not_headings`, `parse::reports_missing_and_duplicate_sections`, `parse::order_does_not_matter`, `parse::non_utf8_is_reported`, `status::missing_ticket_md_is_reported`, `parse::lone_cr_line_endings_parse` |
| AC-10 | `stapel status` · prints, one per line: `ticket: <KEY> — <title>`; `state: open`, `state: closed by <by> at <at>: <reason>` or `state: legacy (STP-1 format)`; `waiting for: <id> (owner: <owner>)` for the first section in `stapel.toml` order whose owner is not `generated` and that has no fresh confirmation, `waiting for: none`, or `waiting for: none (closed)`; one line per stale reason of a latest confirmation (AC-7), with the diff (AC-6) after a `changed since confirmed` line; `problem: <reason>` lines for an unreadable `state.json` or `ticket.md` (then no `waiting for` line); and the repository-wide permit, `build: allowed (by <KEY>)` for the first open ticket that grants it, else `build: allowed by hand (phase 0, <KEY>)`, else `build: not allowed`. It exits 0 when the ticket can be read, stale or not, and 1 only for an unreadable or invalid file, an invalid config or an ambiguous key. It writes no file. | `status::waiting_for_is_first_unconfirmed`, `status::build_line_names_the_granting_ticket`, `status::exit_codes`, `status::never_writes` |
| AC-11 | The PreToolUse write guard (STP-1 AC-8, AC-9) · allows code writes when some open ticket's `build.requires` confirmations are fresh, or when some `state.json` without a `closed` fact carries `build.allowed = true` (read from any JSON, with or without `schema_version`, also when the config is invalid); with an invalid or missing config it gives no computed permit. It reads `state.json` first and parses `ticket.md` only for an open ticket whose required sections all have a confirmation; it reads only regular files of at most 4 MiB, gives no permit from a ticket it cannot read or parse or that exceeds the bound, and stops at the first ticket that gives a permit. Its denial names, per open ticket, the required sections that are not confirmed or are stale, and that `stapel status` shows what is waiting; it does not tell the agent to run `ok` (decided by the orchestrator while the human was away; confirmed by the human on 2026-10-05). `state.json` is read only as a regular file of at most 4 MiB as well. | `hook::allows_code_write_with_fresh_confirmations`, `hook::denies_code_write_after_spec_edit`, `hook::closed_ticket_gives_no_permit`, `hook::legacy_build_flag_still_honoured`, `hook::closed_ticket_flag_not_honoured`, `hook::invalid_config_gives_no_computed_permit`, `hook::unreadable_ticket_gives_no_permit`, `hook::oversized_ticket_gives_no_permit`, `hook::non_regular_file_gives_no_permit`, `hook::permit_check_on_4mib_ticket_is_fast`, `hook::special_state_json_gives_no_permit_quickly`, `hook::denial_names_tickets_and_sections` |
| AC-12 | `state.json` · is written atomically: a temporary file `.state.json.<pid>.tmp` in the same folder, removed first if it exists and then opened exclusively (`create_new`), `fsync`, rename, `fsync` of the folder; a failure before the rename leaves the old file byte-identical; leftover temporary files are ignored when reading and removed by the next successful write. Fields it does not know are preserved. A file that is not valid JSON or does not parse into the schema-1 model, or has a `schema_version` other than 1, makes `ok` and `close` refuse with exit 1 and `status` report it; such files and legacy files are never rewritten by `ok`. | `state::failed_write_keeps_old_file`, `state::leftover_temp_is_ignored_and_removed`, `state::write_preserves_unknown_fields`, `state::refuses_corrupt_or_unknown_version`, `state::refuses_wrong_shape`, `state::legacy_file_is_reported`, `state::write_fails_cleanly_on_readonly_dir` |
| AC-13 | `stapel.toml` · is rejected by `new`, `ok`, `close` and `status` (exit 1 with the reason) and gives no computed permit when section ids or titles repeat (without case), a title is `Description`, `depends_on` names an unknown id or the section itself, `tickets.key` lacks `{n}`, or `build.requires` is empty or names an unknown id or a section whose owner is `generated`; when `build` is absent, `requires` defaults to spec, design, proof. | `core::config::rejects_bad_sections`, `core::config::build_requires_defaults`, `cli_config::bad_config_refused_by_every_command` |
| AC-14 | `stapel close [KEY] --reason "<text>"` · writes `closed: {by, at, reason}` to `state.json`, and removes a top-level `build` object if present, printing `removed: build.allowed (phase 0 hand permit)`; the reason must be non-empty; closing a closed ticket refuses with exit 1; the identity and agent-shell rules of `ok` apply. | `close::records_closed_fact`, `close::drops_hand_build_flag`, `close::refuses_twice_and_without_reason`, `close::refuses_in_agent_shell_without_grant` |
| AC-15 | The PreToolUse guard · recognises a Bash command that is exactly one simple command (assignments and redirections allowed and dropped) whose program's base name is `stapel`, or `cargo run [options] --`, and whose first word not starting with `-` after it is `ok` or `close` (without case). For it: when `permission_mode` in the hook input is not `default`, `acceptEdits`, `auto` or `plan`, it denies with the reason that confirmation needs the dialog; when the arguments of a `stapel ok|close` (or `cargo run ... -- ok|close`) anywhere in the command contain a word starting with `--grant` or a run-time word with `$`, it denies; it denies `stapel hook` run from Bash; otherwise it runs the checks `ok` or `close` would run (same repository root as the agent's folder, the ticket resolves and is open, the section exists once and is not `generated`, `user.name` is set, `close` has a non-empty reason) and denies with that reason when one fails, else answers `ask` (AC-16). Any other command, including compound or nested forms that run `stapel ok`, falls through to the STP-1 checks, and `ok` itself refuses there without a grant (AC-5). Text that only mentions the words is allowed. (decided by the orchestrator while the human was away; confirmed by the human on 2026-10-05) | `hook::asks_for_plain_stapel_ok`, `hook::asks_for_cargo_run_ok`, `hook::denies_ok_in_bypass_mode`, `hook::denies_command_with_own_grant`, `hook::denies_unresolvable_ok`, `hook::allows_mentions_of_stapel_ok`, `hook::allows_stapel_status_and_new_from_bash`, `hook::denies_running_the_hook_from_bash`, `hook::denies_stapel_ok_with_run_time_words` |
| AC-16 | For a command the guard asks about (AC-15) · when the hook process itself carries `CLAUDE_CODE_EXECPATH` (it was started from an agent's shell, not by Claude Code), it denies and issues nothing (decided by the orchestrator while the human was away; confirmed by the human on 2026-10-05); otherwise it creates a random 128-bit token (hex), removes expired grants, and writes a grant file whose name is the SHA-256 of `token`, key, action, section, section hash, the sorted dependency hashes and the `stapel` version, and whose content is only `expires_at` (30 minutes), in `git rev-parse --git-path stapel/grants` (inside `.git`, so git never tracks it and the write tools are already denied there). It answers `permissionDecision: "ask"` with a reason that names the ticket, the section, the hash prefix, the line count, and `same as HEAD`, `differs from HEAD (+a -b lines)` or `not in HEAD`, and says to answer Yes or No and not to choose "don't ask again"; and `updatedInput` rendered from the resolved values: `stapel` (the installed build that ran the checks) or the `cargo run ... --` words as given (the reason names it), then `ok <KEY> <section> --grant <token>` or `close <KEY> --reason=<reason> --grant <token>`, each word quoted for `sh`. For `close` the reason names the ticket and the closing reason. It denies instead when a Bash permission allow rule in `.claude/settings.json`, `.claude/settings.local.json` or the same files under `$HOME` matches the replaced command, using Claude Code's forms `Bash`, `Bash(*)`, `Bash(prefix:*)` and `*` patterns (managed settings are not read). `ok` and `close` with `--grant` recompute the file name from their arguments, the current hashes and their own version; the file must be a regular file, unexpired; it is deleted once found, whatever the outcome. A missing file refuses with the reason that the section, a dependency or the `stapel` version changed since the dialog, or the grant was used already; using a grant and issuing one both remove expired or unreadable grant files; an expired one refuses and says to ask the agent to run the command again. `--grant` has no other spelling. (decided by the orchestrator while the human was away; confirmed by the human on 2026-10-05), `hook::grant_needs_the_claude_code_hook_process`, `hook::allow_rule_prefix_needs_word_boundary` | `hook::ask_carries_grant_and_reason`, `hook::sweeps_expired_grants`, `hook::denies_when_allow_rule_exists`, `ok::accepts_valid_grant_once`, `ok::refuses_expired_or_mismatched_grant`, `ok::refuses_grant_after_section_edit`, `ok::ignores_rewritten_grant_file`, `close::accepts_valid_grant_once`, `close::refuses_ok_grant`, `flow::grant_round_trip`, `flow::grant_round_trip_refuses_after_edit`, `hook::allow_rules_are_matched_like_claude_code`, `hook::allow_rule_in_user_settings_is_found`, `hook::replaced_command_runs_installed_stapel`, `hook::close_reason_with_dash_round_trips`, `ok::consuming_sweeps_expired_grants` |
| AC-17 | The PreToolUse guard · denies, in a stapel repository, a Bash command whose text can inject terminal input: a program whose base name is `xdotool`, `ydotool` or `wtype`; `tmux` with an argument starting with `send` or one of `paste-buffer`, `pasteb`, `load-buffer`, `loadb`, `set-buffer`, `setb`; `screen` with `-X` and `stuff`; any word containing `TIOCSTI`, `0x5412` or `21522`; a word or redirection target that starts with `/dev/pts/` or equals `/dev/tty`. The reason says that only a person answers the dialog. (decided by the orchestrator while the human was away; confirmed by the human on 2026-10-05) | `hook::denies_terminal_input_injection`, `hook::allows_ordinary_tmux_use`, `hook::denies_injection_spellings` |

### Questions

All answered on 2026-10-02 by the human.

1. **Where state lives in phase 0.** In the working tree next to `ticket.md`, committed with the code; the
   separate branch is a phase-1 decision. Answer: as recommended.
2. **Permit computed or stored.** Computed (AC-11); the hand-kept flag keeps working in phase 0. Answer: as
   recommended.
3. **Which sections the build needs.** spec, design, proof, as data in `build.requires`. Answer: as
   recommended.
4. **What `by` records.** `git config user.name`; refuse when unset or blank; warn when it contains whitespace.
   Answer: as recommended; warn.
5. **Confirmed text in `state.json`.** On the latest confirmation of each section only. Answer: as recommended.
6. **Scope of the permit.** Any open ticket; tied to the ticket being built with `build` in phase 1; closing
   ends a ticket's permit. Answer: as recommended.
7. **Who may run `ok` and `close`.** Only a person. Answer: only a person.
8. **How a ticket is closed.** A `close` command that writes the `closed` fact; `ship` writes the same in
   phase 1. Answer: the command.
9. **Through which channels a person confirms.** Nobody types commands; a person works in the Claude Code
   terminal and in Zed. Answer: both. Design: the permission dialog (AC-15, AC-16) and Zed tasks.
10. **Spec review 3 decisions** (S3-1..S3-17): applied as the reviewer suggested; marked in AC-15, AC-16, AC-17. Answer (2026-10-05, human): confirmed, together with the code-review decisions marked in AC-15, AC-16, Design and Not against.

    **Author self-check (CLAUDE.md item 7), 2026-10-03.** Walked through the abuse rows against the code: an agent without a grant is refused by `ok`/`close` (compound and nested forms fall through to that refusal); a grant file written through Bash needs a token the agent chose, which is declared in Not against; rewriting a pending grant's content changes nothing (name binding); an edit between the dialog and the yes changes the name and is refused; `--grant` in any word, terminal input injection, bypass mode and allow rules are denied by the guard; `!stapel ok` typed by the person in Claude Code runs with `CLAUDECODE` set and is refused with the `env -u` hint. Nothing new found.

### Coverage taxonomy

Recorded after the build had started (code review D-13); the spec reviews covered these categories, but the
pass was not written down before.

| Category (klc `coverage-taxonomy.yml`) | Where the ticket answers it |
|---|---|
| functional-scope | Description, AC-1..AC-17, Out of scope; actors: person, agent, guard |
| domain-data-model | `state.json` (AC-12), confirmation record (AC-5), closed fact (AC-14), grant (AC-16); identity of a ticket by folder name (AC-4) |
| interaction-ux | `status` lines and exit codes (AC-10), refusal texts naming a way forward, the dialog reason (AC-16) |
| nfr | 4 MiB bounds, 2 s permit check, 200 ms parse (AC-9, AC-11, Test plan) |
| integration-external | External contract table: git, Claude Code hooks and permission modes, CommonMark |
| edge-failure | AC-7 reasons, AC-12 refusals, Abuse rows of the Test plan, Not against |
| constraints-tradeoffs | Questions 1–10, Decisions "Considered and rejected" |
| terminology | open / closed / legacy / unreadable (AC-4), fresh / stale (AC-7), grant, permit |
| completion-signals | Proof table, Plan step 11 |
| misc-placeholders | `TODO` lines that `new` writes; no open placeholders in this ticket |

**Exceptions to the process, recorded honestly.** Build steps 1–5 were built while spec review 3 ran and before
its revision was committed; steps 6–10 followed the revision (code review D-13). GREEN commits of steps 1–9
carry `cargo fmt` changes to test files without saying so (D-14); from round 1 on, `cargo fmt` runs before the
RED commit. Timestamps in this ticket's journals were written by hand until code review round 1 and are marked
`ts_source` (D-15).

### Out of scope

- Several people editing state at once: CAS push, holders, heartbeats (phase 1, with a shared state branch).
- Merging `state.json` across branches: a conflict makes the file unreadable, `ok` and `close` refuse,
  `status` reports it, the person resolves it by hand. Phase 1 decides.
- The decision log `decisions.jsonl` and `stapel tokens` (STP-3).
- `ok --at-own-risk`, `build`, `ask`, returning a section with a reason, reopening a closed ticket (phase 1).
- Generating Zed tasks and key bindings for `ok` and `close` (phase 3, with `stapel-lsp`); until then a person
  may add a Zed task that runs `stapel ok <section>` by hand.
- Fetching anything from the tracker URL; checking the URL for private terms.
- Adding or renaming sections in `stapel.toml` for existing tickets beyond AC-7 and AC-13.

## Design

### What klc does, and what we take

From a study of `../klc` (token journal entry `author-research`):

| klc | Take / do differently | Why |
|---|---|---|
| State is a stored phase string in `meta.json`; it drifted from reality (KLC-163, KLC-170) | Store only facts (confirmations, closed), compute the rest | The klc rethink itself proposes this |
| "Spec sealed after discovery ack" only in docs and prompts; KLC-154 amended criteria after review and nothing went stale | Hash at confirmation, compare on every read | The core of this ticket |
| Ack records have no "who", no hash, no reason; an ack 2 s after the request (KLC-154) | Record who, when, hash and text; only a person confirms, through a dialog the agent cannot answer | Principle 5 |
| `gate.py` takes the human's decision from `UserPromptSubmit` | Do differently: that hook also fires for agent messages (Claude Code issue #94675), so it is not a human channel | External contract |
| `write_meta` is a plain write; corrupt JSON raises an uncaught error | Temp + fsync + rename; refuse to overwrite an unreadable file | AC-12 |
| No `schema_version`; read-only commands wrote a migration back (KLC-062) | `schema_version` from day one; `status` never writes | AC-10, AC-12 |
| Keys supplied by hand, not normalised (`KLC-02`, `KLC-001`); tests only uppercase (KLC-129) | Generate keys; compare without case and zero padding | AC-1, AC-4 |
| No normalisation before hashing | Define and version the normal form | AC-8, AC-7 |
| Edits outside the tool swept silently into the next commit | Report "changed since confirmed" with a diff | AC-6 |
| Remedies pointed to commands that refuse (KLC-170) | Every refusal names the reason and a command that works | all refusals |
| CAS envelope, holders, heartbeats; fuzz gate with seven invariants (KLC-057) | Not now; invariants become property tests | Single user in phase 0 |

### External contract (facts)

| Claim | Source | Mark |
|---|---|---|
| `git config user.name` prints the name and exits 1 when unset; an empty value prints an empty line and exits 0 | `git help config`; spec review S-10 | verified |
| `rename(2)` replaces atomically on one filesystem; `File::sync_all` is `fsync`; `sync_all` on an opened directory flushes the rename on Linux | Rust std docs, POSIX | read |
| Claude Code blocks a PreToolUse call only on exit 2; another code, a crash or a timeout lets it through | Claude Code hooks docs; STP-1 reviews | read |
| A PreToolUse answer `permissionDecision: "ask"` shows the permission dialog, also in auto mode; with `updatedInput` the replaced command runs after "yes" | experiment 2026-10-02, Claude Code terminal, this repository | verified |
| After "no", the model gets only "the user doesn't want to proceed"; neither the replaced command nor its token is in the reply or in the session transcript file | same experiment; transcript records of the denied call checked | verified |
| `UserPromptSubmit` fires for agent and subagent messages and task notifications as well as for typed text; the payload has no source field | Claude Code issue #94675; docs | read |
| `!` shell mode in Claude Code skips PreToolUse hooks and has `CLAUDECODE` set | Claude Code docs | read |
| Hook processes have `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_PROJECT_DIR` | Claude Code hooks docs; STP-1 R-5 | read |
| The PreToolUse input has `permission_mode`; it was `auto` in this session | experiment log 2026-10-02 | verified |
| In `bypassPermissions` mode a hook `ask` is not shown and the call proceeds | Claude Code permission-modes docs; spec review S3-2 | read |
| A hook `ask` shows the dialog also in `default`, `acceptEdits` and `plan` modes | not yet tested (Plan step 11) | assumption |
| "Yes, and don't ask again" adds an allow rule that may stop later `ask` dialogs | spec review S3-10 | assumption |
| An agent can type into its own terminal with `tmux send-keys`, `screen -X stuff`, `xdotool` | spec review S3-1 | assumption |
| The Zed agent panel shows Claude Code's permission dialog for an `ask` answer and does not auto-approve it | not yet tested (Plan step 11) | assumption |
| The agent's Bash shell has `CLAUDE_CODE_EXECPATH` and no `CLAUDE_PROJECT_DIR`; a hook process has `CLAUDE_PROJECT_DIR` and no `CLAUDE_CODE_EXECPATH`; the hook's parent is `sh` started by `claude` | probe build, 2026-10-02, this session; `runs.jsonl` | verified (undocumented) |
| Codex CLI has comparable hooks and environment markers | a third-party blog | assumption |
| CommonMark: a paragraph followed by `---` is a setext heading; ATX allows up to three leading spaces and closing hashes; an HTML block starting `<!--` runs to `-->` | CommonMark 0.31; spec review S-8 | verified |
| NFC by `unicode-normalization`; unified diff by `similar`; SHA-256 by `sha2`; random tokens by `getrandom` | crate docs | read |

### Decisions

- **Modules in `stapel-core`** (as built): `ticket` (parse `ticket.md`), `hash` (normal form v1, `sha256:` hash), `state`
  (`State` with `#[serde(flatten)] extra`; atomic write with an injectable pre-rename step for tests; legacy
  detection), `stage` (pure: `freshness`, `waiting_for`, `build_permit`), `identity` (user name, agent-shell
  detection), `grant` (issue, verify, consume, expire), `tickets` (folders, numbering, key resolution
  shared by all commands and by the guard), `confirm` (the checks `ok` and the guard share), `dialog` (the
  `ask` answer), `time` (RFC 3339), and `guard`. The CLI adds `new`, `ok`, `status`, `close`.
- **The confirmation flow in Claude Code:** the person tells the agent to confirm; the agent runs
  `stapel ok spec`; the guard answers `ask` with the hash in the reason and the token in `updatedInput`; the
  person answers in the dialog; on "yes" `ok` runs with the grant and consumes it. In a person's terminal or a
  Zed task, `ok` runs without a grant.
- **Same build.** The guard runs the installed `stapel`; the replaced command runs `stapel` too, except
  in the `cargo run` form. The grant name includes the package version only, so it protects against a
  version change, not against two builds of one version (code review D-3) (decided by the orchestrator while the human was away; confirmed by the human on 2026-10-05); Plan step 10b
  installs the build before step 11.
- **Bounds.** The guard reads at most 4 MiB per file, only regular files (`symlink_metadata`), `state.json`
  before `ticket.md`; text is kept on the latest confirmation only.
- **Considered and rejected:** storing the stage (drifts, as in klc); `UserPromptSubmit` as the human channel
  (fires for agent messages); letting agents confirm with a mark; snapshot files (another machine file); hashing
  raw bytes.

### Risks

| Risk | Test |
|---|---|
| R-1 A non-content edit makes every confirmation stale | `hash::ignores_non_content_differences`, `hash::normal_form_is_idempotent` |
| R-2 A real edit is missed by the normal form | `hash::detects_content_changes`, `hash::any_inner_change_changes_hash` |
| R-3 A crash during `ok` breaks `state.json` | `state::failed_write_keeps_old_file` |
| R-4 `status` writes something (klc KLC-062) | `status::never_writes` |
| R-5 The guard grants a permit from a stale, closed, unreadable or oversized ticket | `hook::denies_code_write_after_spec_edit`, `hook::closed_ticket_gives_no_permit`, `hook::closed_ticket_flag_not_honoured`, `hook::unreadable_ticket_gives_no_permit`, `hook::oversized_ticket_gives_no_permit` |
| R-6 Parsing is fooled by headings in code blocks, setext or comments | `parse::ignores_headings_in_code_blocks`, `parse::fence_variants`, `parse::setext_and_comments_are_not_headings` |
| R-7 An agent confirms for itself | `ok::refuses_in_agent_shell_without_grant`, `close::refuses_in_agent_shell_without_grant`, `hook::denies_command_with_own_grant`, `ok::refuses_expired_or_mismatched_grant` |
| R-8 `ok` drops the hand-kept flag or a future field | `state::write_preserves_unknown_fields` |
| R-9 The parser panics or is slow on hostile input inside the guard | `parse::arbitrary_input_never_panics`, `hook::permit_check_on_4mib_ticket_is_fast` |
| R-10 A grant is reused or used for another section | `ok::accepts_valid_grant_once`, `ok::refuses_expired_or_mismatched_grant` |
| R-11 The dialog confirms text the person did not read, or is answered by injected input | `ok::refuses_grant_after_section_edit`, `hook::ask_carries_grant_and_reason`, `hook::denies_terminal_input_injection` |
| R-13 Claude Code starts setting `CLAUDE_CODE_EXECPATH` for hooks, or stops setting it for the Bash tool: the dialog then fails closed (no grant) or the second layer opens | `hook::grant_needs_the_claude_code_hook_process` checks the hook; the variables are checked by hand when Claude Code is updated |
| R-12 The dialog is skipped in a permission mode or by an allow rule | `hook::denies_ok_in_bypass_mode`, `hook::denies_when_allow_rule_exists` |

## Test plan

By category (CLAUDE.md, "Before the build", item 5). Every test named here is written in a RED commit.

**Harness.** The shared test helper removes `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT` and `CLAUDE_PROJECT_DIR`,
sets `GIT_CONFIG_GLOBAL=/dev/null` and a local `user.name` in each temp repository, so the tests behave the
same when run from inside Claude Code. Agent-shell tests set each variable explicitly, one test per variable,
with an empty value too. Identity tests unset the local name. The read-only folder test skips with a message
when running as uid 0.

| Category | Cases | Tests |
|---|---|---|
| Main path | new → status → ok spec → edit → stale with diff → revert → fresh; ok spec, design, proof → build allowed; close → build not allowed, hand flag removed; plain `stapel ok` from the agent → ask with grant → ok consumes the grant | `status::*`, `ok::records_confirmation`, `close::records_closed_fact`, `close::drops_hand_build_flag`, `hook::allows_code_write_with_fresh_confirmations`, `hook::closed_ticket_gives_no_permit`, `hook::ask_carries_grant_and_reason`, `ok::accepts_valid_grant_once` |
| Negative | unknown or generated section; closed ticket; unset or blank identity; missing or bad config for every command; existing key; missing or duplicate section or dependency; bad title or URL; close twice or without reason | `ok::refuses_*`, `new::refuses_*`, `close::refuses_twice_and_without_reason`, `core::config::rejects_bad_sections`, `cli_config::bad_config_refused_by_every_command`, `parse::reports_missing_and_duplicate_sections` |
| Abuse | agent runs `ok`/`close` in its shell without a grant; through Bash in compound or nested form; with a `--grant` of its own; with an expired grant or one for another section; reuses a grant; writes a grant file with a write tool; edits the spec and expects to keep writing | `ok::refuses_in_agent_shell_without_grant`, `close::refuses_in_agent_shell_without_grant`, `hook::denies_command_with_own_grant`, `ok::refuses_expired_or_mismatched_grant`, `ok::accepts_valid_grant_once`, `hook::denies_write_tools_on_grants`, `hook::denies_code_write_after_spec_edit` |
| Robustness | failure before rename; leftover temp; corrupt, wrong-shape, unknown-version or legacy state; 4 MiB bound and its time; FIFO in place of a file; arbitrary parser input (proptest: never panics, 1 MiB under 200 ms in a debug build); a 10 000-line diff | `state::*`, `hook::oversized_ticket_gives_no_permit`, `hook::permit_check_on_4mib_ticket_is_fast`, `hook::non_regular_file_gives_no_permit`, `parse::arbitrary_input_never_panics`, `hash::*` property tests |
| Environment | CRLF, lone CR, BOM, NFD; non-UTF-8 or missing `ticket.md`; read-only folder; keys in other case, zero padding, case-variant folders; `user.name` with a space; legacy STP-1 state beside new tickets | `hash::ignores_non_content_differences`, `parse::non_utf8_is_reported`, `status::missing_ticket_md_is_reported`, `state::write_fails_cleanly_on_readonly_dir`, `new::numbering_ignores_case_and_zero_padding`, `status::refuses_ambiguous_case_variant_keys`, `ok::warns_on_name_with_space`, `status::legacy_ticket_is_not_open` |
| Repeated runs | `status` twice changes nothing; `ok` twice adds a record, keeps text only on the latest | `status::never_writes`, `ok::confirming_twice_keeps_history`, `ok::keeps_text_only_on_latest` |
| Abuse (dialog) | grant file rewritten through Bash; section edited between dialog and yes; `ok` grant used for `close`; terminal input injection; bypass mode; allow rule for `stapel ok` | `ok::ignores_rewritten_grant_file`, `ok::refuses_grant_after_section_edit`, `close::refuses_ok_grant`, `hook::denies_terminal_input_injection`, `hook::denies_ok_in_bypass_mode`, `hook::denies_when_allow_rule_exists` |
| Integration | the real binary in a temp git repo with real `git config`; the guard's `updatedInput` run through `sh -c` with `CLAUDECODE=1` for `stapel ok` and `cargo run` forms; the dialog by hand in Claude Code in each permission mode, with "don't ask again", and in the Zed agent panel with "always allow" on and off (Plan step 11) | all CLI tests run the built binary, `flow::grant_round_trip`, `flow::grant_round_trip_refuses_after_edit` (the `cargo run` form is checked for its replaced command by `hook::asks_for_cargo_run_ok`; running it inside a test would start a nested build) |

## Proof

| Criterion or risk | Test | Result |
|---|---|---|
| AC-1 | `new::creates_ticket_files`, `new::numbers_after_largest_existing`, `new::numbering_ignores_case_and_zero_padding`, `new::refuses_bad_title` | — |
| AC-2 | `new::stores_tracker_link`, `new::refuses_bad_tracker_url` | — |
| AC-3 | `new::refuses_existing_key`, `new::refuses_without_config` | — |
| AC-4 | `status::fresh_ticket_waits_for_spec`, `status::key_matched_without_case`, `status::without_key_needs_one_open_ticket`, `status::legacy_ticket_is_not_open`, `status::refuses_ambiguous_case_variant_keys`, `ok::resolves_single_open_ticket` | — |
| AC-5, R-7 | `ok::records_confirmation`, `ok::keeps_text_only_on_latest`, `ok::refuses_generated_and_unknown_sections`, `ok::refuses_on_closed_ticket`, `ok::refuses_without_identity`, `ok::refuses_blank_identity`, `ok::warns_on_name_with_space`, `ok::refuses_missing_dependency`, `ok::refuses_in_agent_shell_without_grant`, `ok::confirming_twice_keeps_history`, `ok::refuses_oversized_ticket` | — |
| AC-6 | `status::edit_makes_confirmation_stale_with_diff`, `status::revert_makes_it_fresh_again` | — |
| AC-7 | `status::upstream_change_makes_dependents_stale`, `status::missing_or_duplicate_section_is_stale`, `status::removed_dependency_is_stale`, `status::older_normal_form_is_reported`, `status::dependency_missing_or_duplicated_is_named` | — |
| AC-8, R-1, R-2 | `hash::ignores_non_content_differences`, `hash::detects_content_changes`, `hash::normal_form_is_idempotent`, `hash::any_inner_change_changes_hash` | — |
| AC-9, R-6, R-9 | `parse::finds_sections_by_title`, `parse::ignores_headings_in_code_blocks`, `parse::fence_variants`, `parse::setext_and_comments_are_not_headings`, `parse::reports_missing_and_duplicate_sections`, `parse::order_does_not_matter`, `parse::non_utf8_is_reported`, `parse::arbitrary_input_never_panics`, `parse::large_ticket_parses_quickly`, `status::missing_ticket_md_is_reported`, `parse::lone_cr_line_endings_parse` | — |
| AC-10, R-4 | `status::waiting_for_is_first_unconfirmed`, `status::build_line_names_the_granting_ticket`, `status::exit_codes`, `status::never_writes` | — |
| AC-11, R-5 | `hook::allows_code_write_with_fresh_confirmations`, `hook::denies_code_write_after_spec_edit`, `hook::closed_ticket_gives_no_permit`, `hook::legacy_build_flag_still_honoured`, `hook::closed_ticket_flag_not_honoured`, `hook::invalid_config_gives_no_computed_permit`, `hook::unreadable_ticket_gives_no_permit`, `hook::oversized_ticket_gives_no_permit`, `hook::non_regular_file_gives_no_permit`, `hook::permit_check_on_4mib_ticket_is_fast`, `hook::special_state_json_gives_no_permit_quickly`, `hook::denial_names_tickets_and_sections` | — |
| AC-12, R-3, R-8 | `state::failed_write_keeps_old_file`, `state::leftover_temp_is_ignored_and_removed`, `state::write_preserves_unknown_fields`, `state::refuses_corrupt_or_unknown_version`, `state::refuses_wrong_shape`, `state::legacy_file_is_reported`, `state::write_fails_cleanly_on_readonly_dir` | — |
| AC-13 | `core::config::rejects_bad_sections`, `core::config::build_requires_defaults`, `cli_config::bad_config_refused_by_every_command` | — |
| AC-14 | `close::records_closed_fact`, `close::drops_hand_build_flag`, `close::refuses_twice_and_without_reason`, `close::refuses_in_agent_shell_without_grant` | — |
| AC-15, R-7, R-12 | `hook::asks_for_plain_stapel_ok`, `hook::asks_for_cargo_run_ok`, `hook::denies_ok_in_bypass_mode`, `hook::denies_command_with_own_grant`, `hook::denies_unresolvable_ok`, `hook::allows_mentions_of_stapel_ok`, `hook::allows_stapel_status_and_new_from_bash`, `hook::denies_running_the_hook_from_bash`, `hook::denies_stapel_ok_with_run_time_words` | — |
| AC-16, R-10, R-11 | `hook::ask_carries_grant_and_reason`, `hook::sweeps_expired_grants`, `hook::denies_when_allow_rule_exists`, `ok::accepts_valid_grant_once`, `ok::refuses_expired_or_mismatched_grant`, `ok::refuses_grant_after_section_edit`, `ok::ignores_rewritten_grant_file`, `close::accepts_valid_grant_once`, `close::refuses_ok_grant`, `flow::grant_round_trip`, `flow::grant_round_trip_refuses_after_edit`, `hook::allow_rules_are_matched_like_claude_code`, `hook::allow_rule_in_user_settings_is_found`, `hook::replaced_command_runs_installed_stapel`, `hook::close_reason_with_dash_round_trips`, `ok::consuming_sweeps_expired_grants`, `hook::grant_needs_the_claude_code_hook_process`, `hook::allow_rule_prefix_needs_word_boundary` | — |
| R-13 | `hook::grant_needs_the_claude_code_hook_process`, `hook::removing_claude_variables_is_a_bypass` | — |
| AC-17, R-11 | `hook::denies_terminal_input_injection`, `hook::allows_ordinary_tmux_use`, `hook::denies_injection_spellings` | — |

## Plan

Each step is a pair of commits, RED then GREEN. Check command: `cargo test --workspace`.

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 0 | The human creates STP-2's own `state.json` from their terminal with a one-line command the orchestrator gives (schema 1, key, title, no confirmations, phase-0 `build.allowed = true`), so the build can start; the orchestrator records it in `runs.jsonl` | — | — |
| 1 | `stapel.toml` validation and `build.requires` | AC-13 | function missing |
| 2 | Normal form and section hash | AC-8, R-1, R-2 | module missing |
| 3 | `ticket.md` parsing | AC-9, R-6, R-9 | module missing |
| 4 | `state.json`: model, atomic write, legacy, refusals | AC-12, R-3, R-8 | module missing |
| 5 | Key resolution and `stapel new` | AC-1, AC-2, AC-3, AC-4 (resolution) | subcommand missing |
| 6 | `stapel ok`, identity, agent-shell refusal | AC-5, R-7 | subcommand missing |
| 7 | `stapel status` | AC-4, AC-6, AC-7, AC-10 | subcommand missing |
| 8 | `stapel close` and the CLI check of AC-13 | AC-14, AC-13 | subcommand missing |
| 9 | Guard: computed permit, bounds | AC-11, R-5 | permit not computed |
| 10 | Guard: `ask` with grant; `ok`/`close` accept grants; terminal input injection | AC-15, AC-16, AC-17, R-10..R-12 | no ask |
| 10b | `cargo install --path crates/stapel-cli`; check that `stapel --version` is this version | manual | — |
| R1 | Code review round 1 fixes | new tests AC-5, AC-7, AC-9, AC-11, AC-15..AC-17 | see `findings.jsonl` |
| R2 | Code review round 2: grants only from a Claude Code hook process; allow-rule word boundary | new tests AC-16, R-13 | see `findings.jsonl` |
| 11 | In Claude Code the human tells the agent to confirm spec, design and proof of STP-2 and answers the dialogs; also: one dialog with "don't ask again" followed by a second `ok`, and the Zed agent panel with "always allow" on and off; results go into the External contract; the orchestrator records the `status` output in `runs.jsonl`; the same in the Zed agent panel if available | manual | — |

New dependencies: `sha2`, `unicode-normalization`, `similar`, `getrandom`; for tests `proptest`, `tempfile`. Already
present: `serde`, `serde_json`, `toml`, `clap`; for tests `assert_cmd`, `predicates`.

## Review

Generated. Spec review round 1: 20 findings; round 2: 12 findings; round 3: 17 findings; all applied to this text or recorded with
their fate in `findings.jsonl`. After the build: the three reviewers on a `git archive` copy. Each finding gets
`catchable_at`.

## Summary

Generated after merge.
