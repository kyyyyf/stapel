# STP-2 — `new`, `ticket.md`, `state.json`, section hashes and confirmations, `status`, `close`

## Description

Source: `docs/PHASES.md`, phase 0, second ticket. No external tracker.

A person creates a ticket with `stapel new`, edits `ticket.md`, confirms sections with `stapel ok <section>`,
closes the ticket with `stapel close`, and sees with `stapel status` whose decision is needed and which
confirmations went stale. The stage is never stored: it is computed from facts (PLAN.md principle 3). Closes
phase-0 criteria 2 and 3 and replaces the hand-kept `build.allowed` flag from STP-1 with a permit computed
from confirmations.

**Risk tags:** `data` (new on-disk formats: `state.json`, the structure of `ticket.md`), `interface` (new
commands and their output), `guard` (the PreToolUse guard reads the permit and must stop agents from
confirming). By CLAUDE.md all early checks are mandatory.

## Spec

### Guarantees

- **To the people on a ticket:** a confirmation records who confirmed which section, when, and the exact
  content (by hash) they saw, with the dependencies that held at that moment. If that content or a recorded
  dependency changes, `status` says so and, for the section itself, shows the difference. Reverting the
  content makes the confirmation fresh again.
- **Only a person confirms or closes.** `stapel ok` and `stapel close` refuse in a shell that Claude Code
  gave to an agent, and the PreToolUse guard denies Bash commands that run them.
- **To the write guard:** code writes are allowed only while some *open* ticket has fresh confirmations of
  every section listed in `build.requires` (default: spec, design, proof), computed at the moment of the
  call; during phase 0 also while a hand-kept `build.allowed = true` is present in any ticket's
  `state.json` (STP-1). A closed ticket gives no permit.
- `status` never writes anything.
- **Against:** honest mistakes, hand edits of `ticket.md`, editors that change line endings or Unicode form,
  a crash in the middle of a write, an agent running `ok` or `close`.
- **Not against:**
  - a person or agent forging `state.json` through Bash (the write tools are denied on it, STP-1 AC-8;
    Bash writes are the declared STP-1 hole);
  - an agent reverting `ticket.md` to an earlier confirmed text through the write tools, which makes the
    confirmations fresh again: the human sees it in the diff of the ticket and in git history;
  - a hard line break written as two trailing spaces: trailing whitespace is not content (AC-8);
  - dependencies added to `stapel.toml` after a confirmation: a confirmation keeps the dependencies it was
    made with.

### Acceptance criteria

| № | Criterion | Test |
|---|---|---|
| AC-1 | A person · runs `stapel new "<title>"` · in a repository with a valid `.stapel/stapel.toml` · and gets `.stapel/tickets/<KEY>/ticket.md` with an ATX level-2 heading (AC-9) for Description and for every section in `stapel.toml`, in that order, each followed by the line `TODO`, and `state.json` with `schema_version = 1`, the key, the title and an empty confirmation list; `<KEY>` is `tickets.key` with `{n}` = 1 + the largest number among existing ticket folders of that prefix (prefix compared without case, numbers without leading zeros); the command prints `created: <KEY>` and exits 0. The title must be non-empty after trimming and contain no control characters. | `new::creates_ticket_files`, `new::numbers_after_largest_existing`, `new::numbering_ignores_case_and_zero_padding`, `new::refuses_bad_title` |
| AC-2 | A person · runs `stapel new "<title>" --tracker <url>` · and the URL, which must start with `http://` or `https://` and contain no whitespace or control characters, is stored in `state.json` and shown as the first line of Description, `Tracker: <url>`; without `--tracker` neither appears. Nothing is fetched. | `new::stores_tracker_link`, `new::refuses_bad_tracker_url` |
| AC-3 | `stapel new` · when the ticket folder for the computed key exists, or `stapel.toml` is missing or invalid (AC-13) · refuses with exit 1, names the reason and writes nothing. | `new::refuses_existing_key`, `new::refuses_without_config` |
| AC-4 | A person · runs `stapel status <KEY>` · and the key is matched without case; `stapel status` without a key uses the only open ticket, and with zero or several open tickets lists the open keys and exits 1. A ticket is open when its `state.json` has no `closed` fact; a ticket whose `state.json` cannot be read counts as open and is reported. Two folders whose keys differ only in case make `status` refuse with exit 1. | `status::fresh_ticket_waits_for_spec`, `status::key_matched_without_case`, `status::without_key_needs_one_open_ticket`, `status::refuses_ambiguous_case_variant_keys` |
| AC-5 | A person · runs `stapel ok <section>` · and `state.json` gets a confirmation `{section, by, at, hash, normal_form, depends_on: {<id>: hash}, text}`: `by` is `git config user.name` read in the repository root, trimmed; `at` is UTC RFC 3339; `hash` is `sha256:<hex>` of the normal form; `normal_form` is `1`; `depends_on` holds the current hashes of the section's `depends_on` from `stapel.toml`; `text` is the normalized section text. Only the latest confirmation of a section keeps `text`; older ones keep the rest. The command prints `confirmed: <section> (<hash prefix>)` and exits 0. It refuses with exit 1 and a reason: for a section whose owner is `generated` or an unknown id (listing valid ids); when `user.name` is unset or blank; when the section or one of its dependencies is missing or duplicated; when `CLAUDECODE` or `CLAUDE_CODE_ENTRYPOINT` is set. When `user.name` contains whitespace it warns on stderr that the name goes into a tracked file. | `ok::records_confirmation`, `ok::keeps_text_only_on_latest`, `ok::refuses_generated_and_unknown_sections`, `ok::refuses_without_identity`, `ok::refuses_blank_identity`, `ok::warns_on_name_with_space`, `ok::refuses_missing_dependency`, `ok::refuses_in_agent_shell`, `ok::confirming_twice_keeps_history` |
| AC-6 | `stapel status` · after `ok spec` and an edit of the spec section · shows `stale: spec — changed since confirmed by <by> at <at>` followed by a unified line diff between the confirmed and the current normalized text; after reverting the edit the confirmation is fresh again. | `status::edit_makes_confirmation_stale_with_diff`, `status::revert_makes_it_fresh_again` |
| AC-7 | The latest confirmation of a section (the last one for that id in array order; `at` is informative only) · is fresh when the section's current hash and the current hash of every id recorded in its `depends_on` equal the recorded ones; otherwise stale with one reason per cause: `changed since confirmed`, `depends on <id>, which changed`, `section missing`, `section duplicated`, `confirmed under normal form <n>`. | `status::upstream_change_makes_dependents_stale`, `status::missing_or_duplicate_section_is_stale`, `status::older_normal_form_is_reported` |
| AC-8 | The normal form (version 1) of a section · strips a leading UTF-8 BOM, turns CRLF and lone CR into LF, applies Unicode NFC, strips trailing spaces and tabs on each line, drops blank lines at the start and end, and excludes the heading line; any other change, including whitespace inside a line, changes the hash. | `hash::ignores_non_content_differences`, `hash::detects_content_changes`, `hash::normal_form_is_idempotent`, `hash::any_inner_change_changes_hash` |
| AC-9 | `ticket.md` parsing · treats as a section heading only a line matching `^ {0,3}##[ \t]+(.+?)[ \t]*(#+[ \t]*)?$` outside fenced code blocks, and finds a section by the heading whose text equals the section `title` from `stapel.toml` (compared without case); a fence opens with three or more backticks or tildes and closes only with a fence of the same character at least as long; setext headings, indented code and HTML comments are not headings; a section ends at the next level-2 heading; order does not matter. A non-UTF-8 or missing `ticket.md` is reported by `status` and refused by `ok`. | `parse::finds_sections_by_title`, `parse::ignores_headings_in_code_blocks`, `parse::fence_variants`, `parse::setext_and_comments_are_not_headings`, `parse::reports_missing_and_duplicate_sections`, `parse::order_does_not_matter`, `parse::non_utf8_is_reported`, `status::missing_ticket_md_is_reported` |
| AC-10 | `stapel status` · prints, one per line: `ticket: <KEY> — <title>`, `state: open` or `state: closed by <by> at <at>: <reason>`, `waiting for: <id> (owner: <owner>)` for the first section in `stapel.toml` order whose owner is not `generated` and that has no fresh confirmation, or `waiting for: none`; one line per stale latest confirmation (AC-7) with its diff (AC-6); and `build: allowed`, `build: allowed by hand (phase 0)` or `build: not allowed`. It exits 0 whenever the ticket can be read and 1 when a problem is reported. It writes no file. | `status::waiting_for_is_first_unconfirmed`, `status::build_allowed_when_required_fresh`, `status::exit_codes`, `status::never_writes` |
| AC-11 | The PreToolUse write guard (STP-1 AC-8, AC-9) · allows code writes when some open ticket's `build.requires` confirmations are fresh, or when any ticket's `state.json` carries `build.allowed = true` (read from any JSON, with or without `schema_version`); it reads only regular files of at most 4 MiB, gives no permit from a ticket it cannot read, parse or that exceeds the bound, and stops at the first ticket that gives a permit. | `hook::allows_code_write_with_fresh_confirmations`, `hook::denies_code_write_after_spec_edit`, `hook::closed_ticket_gives_no_permit`, `hook::legacy_build_flag_still_honoured`, `hook::unreadable_ticket_gives_no_permit`, `hook::oversized_ticket_gives_no_permit`, `hook::non_regular_file_gives_no_permit` |
| AC-12 | `state.json` · is written atomically: a temporary file `.state.json.<pid>.tmp` in the same folder, `fsync`, rename, `fsync` of the folder; a failure before the rename leaves the old file byte-identical; leftover temporary files are ignored when reading and removed by the next successful write. Fields it does not know are preserved. A file with corrupt JSON or a `schema_version` other than 1 makes `ok` and `close` refuse with exit 1 and `status` report it; a file without `schema_version` is reported as `legacy state (STP-1)`. Such files are never rewritten. | `state::failed_write_keeps_old_file`, `state::leftover_temp_is_ignored_and_removed`, `state::write_preserves_unknown_fields`, `state::refuses_corrupt_or_unknown_version`, `state::legacy_file_is_reported`, `state::write_fails_cleanly_on_readonly_dir` |
| AC-13 | `stapel.toml` · is rejected by `new`, `ok`, `close` and `status`, and gives no permit to the guard, when section ids or titles repeat (without case), a title is `Description`, `depends_on` names an unknown id or the section itself, `tickets.key` lacks `{n}`, or `build.requires` names an unknown id; when `build` is absent, `requires` defaults to spec, design, proof. | `core::config::rejects_bad_sections`, `core::config::build_requires_defaults` |
| AC-14 | A person · runs `stapel close <KEY> --reason "<text>"` · and `state.json` gets `closed: {by, at, reason}`; the reason must be non-empty; closing a closed ticket refuses with exit 1; the same identity and agent-shell refusals as `ok` apply. | `close::records_closed_fact`, `close::refuses_twice_and_without_reason`, `close::refuses_in_agent_shell` |
| AC-15 | The PreToolUse guard · denies a Bash command whose text runs `stapel ok` or `stapel close` in any form the push check recognises (wrappers, `sh -c`, `$( )`, `eval`), with the reason that only a person confirms. | `hook::denies_stapel_ok_from_bash`, `hook::allows_stapel_status_and_new_from_bash` |

### Questions

All answered on 2026-10-02 by the human.

1. **Where state lives in phase 0.** `state.json` stays in the working tree next to `ticket.md` and is
   committed with the code; the separate branch is a phase-1 decision. Answer: as recommended.
2. **Permit computed or stored.** Computed (AC-11); `ok` never writes a permit; the hand-kept `build.allowed`
   keeps working in phase 0 and is removed with `build` in phase 1. Answer: as recommended.
3. **Which sections the build needs.** spec, design, proof, as data in `build.requires`. Answer: as
   recommended (made configurable by spec review finding S-13).
4. **What `by` records.** `git config user.name` only, no e-mail; refuse when unset or blank; warn when it
   contains whitespace (spec review S-10). Answer: as recommended; warn.
5. **Confirmed text in `state.json`.** Stored on the latest confirmation of each section only (S-20).
   Answer: as recommended.
6. **Scope of the permit.** Still any open ticket; a permit tied to the ticket being built comes with
   `build` in phase 1. Closing ends a ticket's permit (S-2). Answer: as recommended.
7. **Who may run `ok` and `close`** (S-1). Only a person: both refuse in an agent shell, and the guard
   denies them in Bash (AC-5, AC-14, AC-15). Answer: only a person.
8. **How a ticket is closed** (S-2, S-3). A `stapel close` command writes the `closed` fact; open means
   no such fact. Answer: the command.

### Out of scope

- Several people editing state at once: CAS push, holders, heartbeats (klc has them; needed with a shared
  state branch in phase 1).
- Merging `state.json` across branches: a merge conflict makes the file unreadable, `ok` and `close` refuse
  and `status` reports it (AC-12); the person resolves the file by hand. Phase 1 decides the real answer.
- The decision log `decisions.jsonl` and `stapel tokens` (STP-3).
- `ok --at-own-risk`, `build`, `ask`, returning a section with a reason, reopening a closed ticket
  (phase 1).
- Fetching anything from the tracker URL; checking the URL for private terms (the person's responsibility).
- Adding or renaming sections in `stapel.toml` for existing tickets beyond what AC-7 and AC-13 say.

## Design

### What klc does, and what we take

From a study of `../klc` (token journal entry `author-research`):

| klc | Take / do differently | Why |
|---|---|---|
| State is a stored phase string in `meta.json`; it drifted from reality (KLC-163, KLC-170) | Do differently: store only facts (confirmations, closed), compute the rest | The klc rethink itself proposes this |
| "Spec sealed after discovery ack" exists only in docs and prompts; no hash of an approved artefact; KLC-154 amended criteria after review and nothing went stale | Do differently: hash at confirmation, compare on every read | The core of this ticket |
| Ack records have no "who", no content hash, no reason; KLC-154 shows an ack 2 s after the request | Record who, when, hash and text; only a person may confirm (AC-5, AC-15) | Principle 5 |
| `write_meta` is a plain write; corrupt JSON raises an uncaught error | Temp + fsync + rename; refuse to overwrite an unreadable file | AC-12 |
| No `schema_version`; read-only commands wrote a migration back (KLC-062) | `schema_version` from day one; `status` never writes | AC-10, AC-12 |
| Keys supplied by hand, not normalised: `KLC-02` and `KLC-001` coexist; tests only uppercase (KLC-129) | Generate keys, compare without case and zero padding | AC-1, AC-4 |
| No normalisation before hashing | Define and version the normal form | AC-8, AC-7 |
| Edits outside the tool swept silently into the next commit | Report "changed since confirmed" with a diff | AC-6 |
| Remedy messages pointed to commands that refuse (KLC-170) | Every refusal names the reason and a command that works | all refusals |
| CAS envelope, holders, heartbeats; a fuzz gate with seven invariants (KLC-057) | Not now; invariants become property tests | Single user in phase 0 |

### External contract (facts)

| Claim | Source | Mark |
|---|---|---|
| `git config user.name` prints the configured name and exits 1 when unset; an empty value prints an empty line and exits 0 | `git help config`; spec review S-10 | verified |
| `std::fs::rename` replaces the target atomically on the same filesystem (POSIX `rename(2)`); `File::sync_all` calls `fsync`; opening a directory and `sync_all` on it flushes the rename on Linux | Rust std docs, `rename(2)`, `fsync(2)` | read |
| Claude Code blocks a PreToolUse call only on exit code 2; any other code, a crash or a timeout lets the call through | Claude Code hooks docs; STP-1 reviews | read |
| Claude Code sets `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT` in the agent's shell | observed 2026-10-02, STP-1 R-5 | verified (undocumented) |
| CommonMark: a paragraph followed by `---` is a setext level-2 heading; ATX headings allow up to three leading spaces and closing hashes | CommonMark spec 0.31; spec review S-8 | verified |
| NFC by `unicode-normalization`; unified diff by `similar`; SHA-256 by `sha2` | crate docs | read |

### Decisions

- **Modules in `stapel-core`:** `ticket` (parse `ticket.md` into sections, AC-9), `hash` (normal form v1 and
  `sha256:` hash), `state` (`State` with `#[serde(flatten)] extra` for unknown fields; atomic write with an
  injectable pre-rename step for tests; legacy detection), `stage` (pure: `freshness`, `waiting_for`,
  `build_permit`), `identity` (user name, agent-shell detection). The CLI adds `new`, `ok`, `status`,
  `close`. The guard calls `stage::build_permit` and `guard` learns `stapel ok|close` as a denied program.
- **Bounds.** The guard reads at most 4 MiB per file and only regular files (`symlink_metadata`), so a FIFO
  or a huge file cannot stall it past the hook timeout. Text is kept on the latest confirmation only, so
  `state.json` grows by a hash per repeated confirmation.
- **Considered and rejected:** storing the stage (drifts, as in klc); snapshot files next to `state.json`
  (another machine file to protect); hashing raw bytes (any editor would make everything stale); letting
  agents confirm with a mark (more logic, same protection; the human chose "only a person").

### Risks

| Risk | Test |
|---|---|
| R-1 A non-content edit makes every confirmation stale | `hash::ignores_non_content_differences`, `hash::normal_form_is_idempotent` |
| R-2 A real edit is missed by the normal form | `hash::detects_content_changes`, `hash::any_inner_change_changes_hash` |
| R-3 A crash during `ok` breaks `state.json` | `state::failed_write_keeps_old_file` |
| R-4 `status` writes something (klc KLC-062) | `status::never_writes` |
| R-5 The guard grants a permit from a stale, closed, unreadable or oversized ticket | `hook::denies_code_write_after_spec_edit`, `hook::closed_ticket_gives_no_permit`, `hook::unreadable_ticket_gives_no_permit`, `hook::oversized_ticket_gives_no_permit` |
| R-6 Parsing is fooled by headings in code blocks, setext or comments | `parse::ignores_headings_in_code_blocks`, `parse::fence_variants`, `parse::setext_and_comments_are_not_headings` |
| R-7 An agent confirms for itself (spec review S-1) | `ok::refuses_in_agent_shell`, `close::refuses_in_agent_shell`, `hook::denies_stapel_ok_from_bash` |
| R-8 `ok` drops the hand-kept flag or a future field | `state::write_preserves_unknown_fields` |
| R-9 The parser panics or is slow on hostile input inside the guard | `parse::arbitrary_input_never_panics` |

## Test plan

By category (CLAUDE.md, "Before the build", item 5). Every test named here is written in a RED commit.

| Category | Cases | Tests |
|---|---|---|
| Main path | new → status → ok spec → edit → stale with diff → revert → fresh; ok spec, design, proof → build allowed; close → build not allowed | `status::*`, `ok::records_confirmation`, `close::records_closed_fact`, `hook::allows_code_write_with_fresh_confirmations`, `hook::closed_ticket_gives_no_permit` |
| Negative | unknown or generated section; unset or blank identity; missing config; bad config; existing key; missing or duplicate section or dependency; bad title or URL; close twice or without reason | `ok::refuses_*`, `new::refuses_*`, `close::refuses_twice_and_without_reason`, `core::config::rejects_bad_sections`, `parse::reports_missing_and_duplicate_sections` |
| Abuse | agent runs `ok` or `close` in its shell or through Bash in any wrapped form; agent edits the spec and expects to keep writing; state written by a write tool (already denied) | `ok::refuses_in_agent_shell`, `close::refuses_in_agent_shell`, `hook::denies_stapel_ok_from_bash`, `hook::denies_code_write_after_spec_edit`, STP-1 `hook::denies_machine_file_writes_even_with_build` |
| Robustness | failure injected before rename; leftover temp file; corrupt or unknown-version or legacy state; 4 MiB bound; FIFO in place of a file; arbitrary input to the parser (proptest, 1 MiB parses under 200 ms in a debug build, never panics); diff of a 10 000-line section | `state::*`, `hook::oversized_ticket_gives_no_permit`, `hook::non_regular_file_gives_no_permit`, `parse::arbitrary_input_never_panics`, `hash::*` property tests |
| Environment | CRLF, lone CR, BOM, NFD; non-UTF-8 `ticket.md`; missing `ticket.md`; read-only ticket folder; keys in other case, zero padding, case-variant folders; `user.name` with a space | `hash::ignores_non_content_differences`, `parse::non_utf8_is_reported`, `status::missing_ticket_md_is_reported`, `state::write_fails_cleanly_on_readonly_dir`, `new::numbering_ignores_case_and_zero_padding`, `status::refuses_ambiguous_case_variant_keys`, `ok::warns_on_name_with_space` |
| Repeated runs | `status` twice changes nothing; `ok` twice adds a record, keeps text only on the latest, stays fresh | `status::never_writes`, `ok::confirming_twice_keeps_history`, `ok::keeps_text_only_on_latest` |
| Integration | the real binary in a temp git repo with real `git config`; the guard reading real files; `CLAUDECODE` set and unset | all CLI tests run the built binary |

## Proof

| Criterion or risk | Test | Result |
|---|---|---|
| AC-1 | `new::creates_ticket_files`, `new::numbers_after_largest_existing`, `new::numbering_ignores_case_and_zero_padding`, `new::refuses_bad_title` | — |
| AC-2 | `new::stores_tracker_link`, `new::refuses_bad_tracker_url` | — |
| AC-3 | `new::refuses_existing_key`, `new::refuses_without_config` | — |
| AC-4 | `status::fresh_ticket_waits_for_spec`, `status::key_matched_without_case`, `status::without_key_needs_one_open_ticket`, `status::refuses_ambiguous_case_variant_keys` | — |
| AC-5, R-7 | `ok::records_confirmation`, `ok::keeps_text_only_on_latest`, `ok::refuses_generated_and_unknown_sections`, `ok::refuses_without_identity`, `ok::refuses_blank_identity`, `ok::warns_on_name_with_space`, `ok::refuses_missing_dependency`, `ok::refuses_in_agent_shell`, `ok::confirming_twice_keeps_history` | — |
| AC-6 | `status::edit_makes_confirmation_stale_with_diff`, `status::revert_makes_it_fresh_again` | — |
| AC-7 | `status::upstream_change_makes_dependents_stale`, `status::missing_or_duplicate_section_is_stale`, `status::older_normal_form_is_reported` | — |
| AC-8, R-1, R-2 | `hash::ignores_non_content_differences`, `hash::detects_content_changes`, `hash::normal_form_is_idempotent`, `hash::any_inner_change_changes_hash` | — |
| AC-9, R-6, R-9 | `parse::finds_sections_by_title`, `parse::ignores_headings_in_code_blocks`, `parse::fence_variants`, `parse::setext_and_comments_are_not_headings`, `parse::reports_missing_and_duplicate_sections`, `parse::order_does_not_matter`, `parse::non_utf8_is_reported`, `parse::arbitrary_input_never_panics`, `status::missing_ticket_md_is_reported` | — |
| AC-10, R-4 | `status::waiting_for_is_first_unconfirmed`, `status::build_allowed_when_required_fresh`, `status::exit_codes`, `status::never_writes` | — |
| AC-11, R-5 | `hook::allows_code_write_with_fresh_confirmations`, `hook::denies_code_write_after_spec_edit`, `hook::closed_ticket_gives_no_permit`, `hook::legacy_build_flag_still_honoured`, `hook::unreadable_ticket_gives_no_permit`, `hook::oversized_ticket_gives_no_permit`, `hook::non_regular_file_gives_no_permit` | — |
| AC-12, R-3, R-8 | `state::failed_write_keeps_old_file`, `state::leftover_temp_is_ignored_and_removed`, `state::write_preserves_unknown_fields`, `state::refuses_corrupt_or_unknown_version`, `state::legacy_file_is_reported`, `state::write_fails_cleanly_on_readonly_dir` | — |
| AC-13 | `core::config::rejects_bad_sections`, `core::config::build_requires_defaults` | — |
| AC-14 | `close::records_closed_fact`, `close::refuses_twice_and_without_reason`, `close::refuses_in_agent_shell` | — |
| AC-15, R-7 | `hook::denies_stapel_ok_from_bash`, `hook::allows_stapel_status_and_new_from_bash` | — |

## Plan

Each step is a pair of commits, RED then GREEN. Check command: `cargo test --workspace`.

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 0 | The orchestrator writes STP-2's own `state.json` by hand: schema 1, key, title, no confirmations, and the phase-0 `build.allowed = true`, so the build can start; recorded in `runs.jsonl` | — | — |
| 1 | `stapel.toml` validation and `build.requires` | AC-13 | function missing |
| 2 | Normal form and section hash | AC-8, R-1, R-2 | module missing |
| 3 | `ticket.md` parsing | AC-9, R-6, R-9 | module missing |
| 4 | `state.json`: model, atomic write, legacy, refusals | AC-12, R-3, R-8 | module missing |
| 5 | `stapel new` | AC-1, AC-2, AC-3 | subcommand missing |
| 6 | `stapel ok`, identity, agent-shell refusal | AC-5, R-7 | subcommand missing |
| 7 | `stapel status` | AC-4, AC-6, AC-7, AC-10 | subcommand missing |
| 8 | `stapel close` | AC-14 | subcommand missing |
| 9 | Guard: computed permit, bounds, denial of `stapel ok|close` in Bash | AC-11, AC-15, R-5 | permit not computed |
| 10 | The human runs `stapel ok spec`, `ok design`, `ok proof` and `stapel status STP-2` on this repository from their own terminal; the orchestrator records the output in `runs.jsonl` | manual | — |

Dependencies: `sha2`, `unicode-normalization`, `similar`, `proptest` (tests).

## Review

Generated. Spec review (round 0) by the external reviewer: 20 findings, all applied to this text; see
`findings.jsonl`. After the build: the three reviewers on a `git archive` copy. Each finding gets
`catchable_at`.

## Summary

Generated after merge.
