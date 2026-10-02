# STP-2 — `new`, `ticket.md`, `state.json`, section hashes and confirmations, `status`

## Description

Source: `docs/PHASES.md`, phase 0, second ticket. No external tracker.

A person creates a ticket with `stapel new`, edits `ticket.md`, confirms sections with `stapel ok <section>`
and sees with `stapel status` whose decision is needed and which confirmations went stale. The stage is
never stored: it is computed from facts (PLAN.md principle 3). Closes phase-0 criteria 2 and 3 and replaces
the hand-kept `build.allowed` flag from STP-1 with a permit computed from confirmations.

**Risk tags:** `data` (new on-disk formats `state.json` and `ticket.md` structure), `interface` (new commands
and their output), `guard` (the PreToolUse write guard reads the permit). By CLAUDE.md all early checks are
mandatory.

## Spec

### Guarantees

What this ticket promises, to whom, and what it does not.

- **To the people on a ticket:** a confirmation records who confirmed which section, when, and the exact
  content (by hash) they saw. If that content, or a section it depends on, changes afterwards, `status`
  says so and shows the difference. Reverting the content makes the confirmation fresh again.
- **To the write guard:** code writes are allowed only while some ticket has fresh confirmations of spec,
  design and proof (PLAN.md §3), computed at the moment of the call from `ticket.md` and `state.json`.
- **Against:** honest mistakes and edits outside the tool (hand edits of `ticket.md`, a text editor that
  changes line endings, a crash in the middle of a write). **Not against** a person or agent that edits
  `state.json` by hand to forge a confirmation: the write tools are already denied on it (STP-1, AC-8);
  Bash writes are the declared STP-1 hole.
- `status` never writes anything.

### Acceptance criteria

| № | Criterion | Test |
|---|---|---|
| AC-1 | A person · runs `stapel new "<title>"` · in a repository with `.stapel/stapel.toml` · and gets `.stapel/tickets/<KEY>/ticket.md` with a level-2 heading for Description and for every section in `stapel.toml`, in that order, each with a placeholder line, and `state.json` with `schema_version = 1`, the key and the title; `<KEY>` is `tickets.key` with `{n}` = 1 + the largest number among existing ticket folders of that prefix (keys compared without case, numbers without leading zeros); the command prints the key and exits 0. | `new::creates_ticket_files`, `new::numbers_after_largest_existing`, `new::numbering_ignores_case_and_zero_padding` |
| AC-2 | A person · runs `stapel new "<title>" --tracker <url>` · and the URL is stored in `state.json` and shown on the Description line `Tracker: <url>`; without `--tracker` neither appears. Nothing is fetched from the URL. | `new::stores_tracker_link` |
| AC-3 | `stapel new` · when the ticket folder for the computed key already exists (a race or a hand-made folder), or `stapel.toml` is missing or unreadable · refuses with exit code 1, names the reason and writes nothing. | `new::refuses_existing_key`, `new::refuses_without_config` |
| AC-4 | A person · runs `stapel status <KEY>` (or `stapel status` with exactly one open ticket) · on a fresh ticket · and sees `waiting for: spec (owner: product)`; exit 0. Ticket keys on the command line are matched without case. | `status::fresh_ticket_waits_for_spec`, `status::key_matched_without_case` |
| AC-5 | A person · runs `stapel ok spec` · and `state.json` gets a confirmation `{section, by, at, hash, depends_on: {<id>: hash}, text}`, where `by` is `git config user.name`, `at` is UTC RFC 3339, `hash` is the section hash and `text` the normalized section text; the command prints what was confirmed. `ok` works for sections whose owner is not `generated`; for `review`, `summary` or an unknown id it refuses with exit 1 and lists the valid ids. Without `user.name` it refuses with exit 1. | `ok::records_confirmation`, `ok::refuses_generated_and_unknown_sections`, `ok::refuses_without_identity` |
| AC-6 | `stapel status` · after `ok spec` and an edit of the spec section · shows `stale: spec — changed since confirmed by <by> at <at>` followed by a unified line diff between the confirmed and the current normalized text; after reverting the edit the confirmation is fresh again. | `status::edit_makes_confirmation_stale_with_diff`, `status::revert_makes_it_fresh_again` |
| AC-7 | `stapel status` · after `ok spec`, `ok design` and an edit of the spec · shows design as `stale: design — depends on spec, which changed`; a confirmation is stale when its own section hash or the hash of any `depends_on` section differs from the one recorded with it. | `status::upstream_change_makes_dependents_stale` |
| AC-8 | Section hashes · ignore differences that are not content: CRLF vs LF, a UTF-8 BOM, Unicode NFC vs NFD, trailing whitespace on a line, blank lines at the start and end of the section, and the text of the heading itself (a section is found by its id-to-title mapping, see AC-9); any other change, including a change of only whitespace inside a line, changes the hash. | `hash::ignores_non_content_differences`, `hash::detects_content_changes` |
| AC-9 | `ticket.md` parsing · finds a section by the level-2 heading whose text equals the section `title` from `stapel.toml` (compared without case and surrounding whitespace); a section ends at the next level-2 heading; fenced code blocks are not scanned for headings. A missing section, or a duplicate heading, is reported by `status` and refused by `ok` with exit 1. Sections in a different order still parse. | `parse::finds_sections_by_title`, `parse::ignores_headings_in_code_blocks`, `parse::reports_missing_and_duplicate_sections`, `parse::order_does_not_matter` |
| AC-10 | `status` · computes "waiting for" as the first section in `stapel.toml` order whose owner is not `generated` and that has no fresh confirmation, prints the owner, and lists every stale confirmation; when spec, design and proof are fresh it prints `build: allowed`. `status` writes no file (contents and modification times unchanged). | `status::waiting_for_is_first_unconfirmed`, `status::build_allowed_when_three_fresh`, `status::never_writes` |
| AC-11 | The write guard (STP-1 AC-8, AC-9) · allows code writes when some ticket's spec, design and proof confirmations are fresh, computed from `ticket.md` and `state.json` at the time of the call; the hand-kept `build.allowed` flag keeps working during phase 0 and is reported by `status` as `build: allowed by hand (phase 0)`. | `hook::allows_code_write_with_fresh_confirmations`, `hook::denies_code_write_after_spec_edit`, `hook::legacy_build_flag_still_honoured` |
| AC-12 | `state.json` · is written atomically (temporary file in the same folder, fsync, rename), and is never overwritten when it cannot be read: corrupt JSON, a `schema_version` other than 1, or a missing file make `ok` refuse with exit 1 and `status` report the problem; the file is left as it was. | `state::atomic_write_leaves_no_partial_file`, `state::refuses_corrupt_or_unknown_version` |

### Questions

Questions with a recommendation. Until answered, the build follows the recommendation.

1. **Where state lives in phase 0.** PLAN.md §4 makes a separate `stapel-state` branch the default for
   phase 1. Recommendation: in STP-2, `state.json` stays in the working tree next to `ticket.md` and is
   committed with the code; the branch is a phase-1 decision, and nothing in STP-2 depends on it. Answer: —
2. **Permit computed or stored.** Recommendation: computed (AC-11), as principle 3 says; `ok` never writes a
   permit. The hand-kept `build.allowed` keeps working until phase 1 so that STP-2 itself can be built
   before `ok` exists, and is removed in the ticket that adds `build` (phase 1). Answer: —
3. **Which sections need confirmation for the build.** PLAN.md §3 says spec, design and proof. Plan is
   owned by the engineer and is not in that list. Recommendation: keep spec, design, proof; `ok plan` works
   but is not needed for the permit. Answer: —
4. **What `by` records.** Recommendation: `git config user.name` only, no e-mail (tracked files must not
   carry private data); refusing when it is unset. Roles are not checked: one person may hold all roles.
   Answer: —
5. **Confirmed text in `state.json`.** The diff in AC-6 needs the old text. Recommendation: store the
   normalized text with the confirmation; it is what the person saw, and it avoids digging in git history.
   The cost is a larger `state.json`. Answer: —
6. **Scope of the permit (finding E-5 of STP-1).** Recommendation: still any ticket opens code writes; a
   permit tied to the ticket being built needs a notion of the current ticket, which comes with `build` in
   phase 1. Answer: —

### Out of scope

- Several people editing state at once: CAS push, holders, heartbeats (klc has them; stapel needs them only
  with a shared state branch, phase 1).
- The decision log `decisions.jsonl` and `stapel tokens` (STP-3). `ok` records the confirmation in
  `state.json` only.
- `ok --at-own-risk`, `build`, `ask`, returning a section with a reason (phase 1).
- Fetching anything from the tracker URL; tracker adapters.
- Detecting a hand-forged `state.json` (see Guarantees).
- Moving a ticket between tracks or adding sections to an existing ticket; a new section in `stapel.toml`
  shows as missing in old tickets.

## Design

### What klc does, and what we take

From a study of `../klc` (report kept in this ticket's token journal entry `author-research`):

| klc | Take / do differently | Why |
|---|---|---|
| State is a stored phase string in `meta.json`; it drifted from reality (KLC-163, KLC-170) | Do differently: store only facts (confirmations), compute "waiting for" and staleness | The klc rethink itself proposes this |
| "Spec sealed after discovery ack" exists only in docs and prompts; no hash of an approved artefact; KLC-154 amended criteria after review and nothing went stale | Do differently: hash at confirmation, compare on every read | The core of this ticket |
| Ack records have no "who", no content hash, no reason; KLC-154 shows an ack 2 s after the request | Take the lesson: record who, when, hash and the text seen | Decisions must be traceable (principle 5) |
| `write_meta` is a plain write, not atomic; corrupt JSON raises an uncaught error | Do differently: temp + fsync + rename; refuse to overwrite an unreadable file | AC-12 |
| No `schema_version`; legacy strings migrated on read, and read-only commands wrote the migration back (KLC-062) | Take `read_meta_ro` as an idea: `status` never writes; add `schema_version` from day one | AC-10, AC-12 |
| Keys supplied by hand, checked by a regex, not normalised: `KLC-02` and `KLC-001` coexist; tests used only uppercase (KLC-129) | Do differently: generate keys, compare without case and zero padding | AC-1, AC-4 |
| No normalisation before hashing anywhere | Do differently: define the normal form (AC-8) | Editors change line endings and Unicode forms |
| Edits outside the tool are swept silently into the next commit | Do differently: report "changed since confirmed" with a diff | AC-6 |
| CAS transaction envelope, holders, heartbeats; fuzz gate with seven invariants (KLC-057) | Not now; the invariants idea is taken for property tests | Single user in phase 0 |

### External contract (facts)

| Claim | Source | Mark |
|---|---|---|
| `git config user.name` prints the configured name, exit 1 when unset | `git help config` | read; `kyyyyf` here, verified |
| `std::fs::rename` replaces the target atomically when both are on the same filesystem (POSIX `rename(2)`) | Rust std docs, `rename(2)` | read |
| `File::sync_all` calls `fsync` | Rust std docs | read |
| Unicode NFC is provided by the `unicode-normalization` crate | crate docs | read |
| A unified diff can be produced by the `similar` crate | crate docs | read |

### Decisions

- **Model.** `stapel-core` gets `ticket` (parse `ticket.md` into sections by title), `hash` (normal form and
  SHA-256 hex of it), `state` (`State { schema_version, key, title, tracker, confirmations: Vec<Confirmation> }`,
  atomic read/write), and `stage` (pure functions: `freshness(confirmation, sections) -> Fresh | Stale(reason)`,
  `waiting_for`, `build_permit`). The CLI adds `new`, `ok`, `status`.
- **Only the latest confirmation of a section counts**; older ones stay in `state.json` as history.
- **Freshness** compares the section hash and each `depends_on` hash recorded at confirmation with the
  current ones; `depends_on` comes from `stapel.toml` at confirmation time.
- **Normal form:** strip a leading BOM; CRLF and CR to LF; NFC; strip trailing whitespace on each line; drop
  blank lines at the start and the end; the heading line is not part of the section text.
- **The guard** calls the same `build_permit` for every ticket folder; a ticket that cannot be parsed or
  read gives no permit (fail closed).
- **Considered and rejected:** storing the stage (drifts, as in klc); snapshot files next to `state.json`
  (another machine file for the guard to protect; the text in `state.json` is enough); hashing raw bytes
  (any editor would make everything stale).

### Risks

| Risk | Test |
|---|---|
| R-1 A non-content edit (line endings, Unicode form) makes every confirmation stale | `hash::ignores_non_content_differences`, property test `hash::normal_form_is_idempotent` |
| R-2 A real edit is missed by the normal form | `hash::detects_content_changes`, property test `hash::any_inner_change_changes_hash` |
| R-3 A crash during `ok` leaves a broken `state.json` and the ticket is unusable | `state::atomic_write_leaves_no_partial_file` |
| R-4 `status` writes something (klc KLC-062) | `status::never_writes` |
| R-5 The guard grants a permit from a stale or unreadable ticket | `hook::denies_code_write_after_spec_edit`, `hook::unreadable_ticket_gives_no_permit` |
| R-6 Section parsing is fooled by headings in code blocks or duplicate headings | `parse::ignores_headings_in_code_blocks`, `parse::reports_missing_and_duplicate_sections` |

## Test plan

By category (CLAUDE.md, "Before the build", item 5). Every test named here is written in a RED commit.

| Category | Cases | Tests |
|---|---|---|
| Main path | new → status → ok spec → edit → status stale with diff → revert → fresh; ok spec, design, proof → build allowed | `status::*`, `ok::records_confirmation`, `hook::allows_code_write_with_fresh_confirmations` |
| Negative | unknown or generated section; missing identity; missing config; existing key; missing or duplicate section | `ok::refuses_*`, `new::refuses_*`, `parse::reports_missing_and_duplicate_sections` |
| Abuse | agent edits the spec after confirmation and expects to keep writing code; `state.json` written by a write tool (already denied) | `hook::denies_code_write_after_spec_edit`, STP-1 `hook::denies_machine_file_writes_even_with_build` |
| Robustness | corrupt or partial `state.json`; unknown `schema_version`; a very large `ticket.md` (1 MiB); a section of 10 000 lines in the diff; property tests on the normal form | `state::refuses_corrupt_or_unknown_version`, `parse::large_ticket_parses_quickly`, `hash::normal_form_is_idempotent`, `hash::any_inner_change_changes_hash` |
| Environment | CRLF files, BOM, NFD text (as written by some editors), keys in other case and with zero padding | `hash::ignores_non_content_differences`, `new::numbering_ignores_case_and_zero_padding`, `status::key_matched_without_case` |
| Repeated runs | `status` twice changes nothing; `ok` twice on the same text adds a second record and stays fresh | `status::never_writes`, `ok::confirming_twice_keeps_history` |
| Integration | the real binary in a temp git repo; the guard reading real files | all CLI tests run the built binary |

## Proof

| Criterion or risk | Test | Result |
|---|---|---|
| AC-1 | `new::creates_ticket_files`, `new::numbers_after_largest_existing`, `new::numbering_ignores_case_and_zero_padding` | — |
| AC-2 | `new::stores_tracker_link` | — |
| AC-3 | `new::refuses_existing_key`, `new::refuses_without_config` | — |
| AC-4 | `status::fresh_ticket_waits_for_spec`, `status::key_matched_without_case` | — |
| AC-5 | `ok::records_confirmation`, `ok::refuses_generated_and_unknown_sections`, `ok::refuses_without_identity`, `ok::confirming_twice_keeps_history` | — |
| AC-6 | `status::edit_makes_confirmation_stale_with_diff`, `status::revert_makes_it_fresh_again` | — |
| AC-7 | `status::upstream_change_makes_dependents_stale` | — |
| AC-8, R-1, R-2 | `hash::ignores_non_content_differences`, `hash::detects_content_changes`, `hash::normal_form_is_idempotent`, `hash::any_inner_change_changes_hash` | — |
| AC-9, R-6 | `parse::finds_sections_by_title`, `parse::ignores_headings_in_code_blocks`, `parse::reports_missing_and_duplicate_sections`, `parse::order_does_not_matter`, `parse::large_ticket_parses_quickly` | — |
| AC-10, R-4 | `status::waiting_for_is_first_unconfirmed`, `status::build_allowed_when_three_fresh`, `status::never_writes` | — |
| AC-11, R-5 | `hook::allows_code_write_with_fresh_confirmations`, `hook::denies_code_write_after_spec_edit`, `hook::legacy_build_flag_still_honoured`, `hook::unreadable_ticket_gives_no_permit` | — |
| AC-12, R-3 | `state::atomic_write_leaves_no_partial_file`, `state::refuses_corrupt_or_unknown_version` | — |

## Plan

Each step is a pair of commits, RED then GREEN. Check command: `cargo test --workspace`.

| Step | What | Tests | Expected at RED |
|---|---|---|---|
| 1 | Normal form and section hash in `stapel-core` | AC-8, R-1, R-2 | module missing |
| 2 | `ticket.md` parsing by title | AC-9, R-6 | module missing |
| 3 | `state.json` model, atomic write, refusals | AC-12, R-3 | module missing |
| 4 | `stapel new` | AC-1, AC-2, AC-3 | subcommand missing |
| 5 | `stapel ok` | AC-5 | subcommand missing |
| 6 | `stapel status`: waiting for, stale, diff, never writes | AC-4, AC-6, AC-7, AC-10 | subcommand missing |
| 7 | Guard reads the computed permit | AC-11, R-5 | permit not computed |
| 8 | Run `new`, `ok`, `status` on this repository's STP-2 itself | manual, output in `runs.jsonl` | — |

Dependencies: `sha2`, `unicode-normalization`, `similar`, `proptest` (tests).

## Review

Generated. Before the build: spec review by the external reviewer on this file only. After the build: the
three reviewers on a `git archive` copy. Each finding gets `catchable_at`.

## Summary

Generated after merge.
