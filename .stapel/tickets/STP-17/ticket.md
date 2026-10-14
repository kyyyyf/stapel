# STP-17 — Fewer confirmation commands

## Description

Source: `docs/PHASES.md`, phase 0, STP-17; asked by the human during STP-16 (Summary of
`.stapel/tickets/STP-16/ticket.md`: eleven `stapel ok` runs for three confirmations).

### Frame

**Problem.** A ticket has four confirmable sections (`spec`, `design`, `proof`, `plan`). After any spec change
the human runs `stapel ok <section>` four times, and each run prints only `confirmed: KEY section (hash)`: the
person does not see what changed since the last confirmation. In STP-16 the human ran `stapel ok` eleven times
for three rounds of confirmation. The guard's permission dialog for an agent's `stapel ok` shows only a line
count and a comparison with `HEAD`, not a comparison with the last confirmation.

**Risk tags:** `interface` (a new form of `stapel ok` and its output), `guard` (the grant of the permission
dialog must still name every section it confirms; "only a person confirms" must not weaken).

**Non-goals.** Filtering sections by owner or role (a team with different owners uses the one-section form);
an interactive y/N question in the terminal; a diff in the text of the permission dialog; changes to `stapel
status`; reopening a closed ticket; concurrent writers of `state.json`.

**Promise (closed list).**

1. `stapel ok KEY`, where `KEY` is not a section id of `stapel.toml` and names a ticket folder, confirms every
   confirmable section of that ticket without a fresh confirmation (not confirmed, or stale), in the order of
   `stapel.toml`. It prints their list first, then for each section what changed since its last confirmation,
   then one `confirmed:` line per section. Each confirmation records today's time, the person, the hash and
   `depends_on`, as the one-section form does.
2. All or nothing: if one of these sections cannot be confirmed (the section or a dependency is missing or
   duplicated), the command refuses with the reason and records nothing; all confirmations go into
   `state.json` in one write. `decisions.jsonl` gets one record per section, in today's format.
3. When no section waits, the command prints that nothing waits, exits 0 and writes nothing.
4. `stapel ok [KEY] <section>` keeps its behaviour and prints the same "what changed" part for its section.
5. "What changed" for a section: a unified diff from the last confirmed text; `new, N lines` when the section
   was never confirmed or the stored text is absent; `unchanged` plus the stale reason when only a dependency
   or the normal form changed.
6. From an agent's shell, without a grant, both forms are refused as today. Through the guard, the permission
   dialog lists each section with its short hash and `+a -b lines since last confirmation`, followed by its
   stale reasons without the time when the diff is empty (Decision SR1-3); one grant names
   every section, its hash and its dependencies. If any of them changed after the dialog, the grant does not
   match and nothing is recorded. When no section waits, the guard denies and shows no dialog.

Not promised: a ticket key that equals a section id (the section wins, as today); deliberate circumvention of
the guard (no `security` tag); a terminal that cannot show a long diff; a file-system write error after
`state.json` is written (the `decisions.jsonl` records may then be partial, as today).

**Process side (no code).** CLAUDE.md item 5 gains one sentence: the spec changes after one review round are
made as one change, so one round needs one confirmation (`stapel ok KEY`). CLAUDE.md is outside the guard's
always-writable paths; the text goes to the human as a short command.

**Size.** Medium: eleven criteria (after spec review); about 200 changed source lines (`crates/stapel-cli/src/main.rs`,
`ok.rs`, `crates/stapel-core/src/confirm.rs`, `dialog.rs`) and about 900 test lines (CLI tests of both forms,
the guard's dialog and grant, golden output). Route: full (tags `interface`, `guard`).

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-14: Decision: the human asked for STP-17 during STP-16 (`.stapel/scratch/stp17/next-session.md`).
- 2026-10-14: Decision (question 1, answer "key only"): the form that confirms every section is `stapel ok KEY`;
  `stapel ok` with no argument stays a usage error. Rejected: no argument meaning the only open ticket
  (recommended); a `--all` flag.
- 2026-10-14: Decision (question 2, recommended): the form confirms sections without a fresh confirmation
  (not confirmed or stale), in `stapel.toml` order; fresh sections are left as they are; when none waits, a
  message and exit 0, no write. Rejected: every confirmable section again.
- 2026-10-14: Decision (question 3, recommended): the terminal form prints the list, then the change of each
  section since its last confirmation, then records, with no y/N question; a section never confirmed shows
  `new, N lines`, not its text. Rejected: a y/N question (needs a terminal).
- 2026-10-14: Decision (question 4, recommended): the permission dialog lists each section with its short
  hash and `+a -b lines since last confirmation`; one grant names all sections, their hashes and
  dependencies; a change in any of them voids the grant. The full diff stays in `stapel status`. Rejected: a
  diff of up to 40 lines in the dialog.
- 2026-10-14: Decision (question 5, recommended): a single argument is a section when it is a section id of
  `stapel.toml` (today's behaviour); otherwise a key when a ticket folder of that name exists; otherwise a
  refusal that lists the sections and the open tickets. Rejected: matching the `[tickets] key` pattern.
- 2026-10-14: Decision (question 6, recommended): all or nothing; one write of `state.json`. Rejected:
  confirming what can be confirmed and exiting 1.
- 2026-10-14: Decision (question 7, recommended): the one-section form prints the same change output.
- 2026-10-14: Decision (question 8, recommended): `decisions.jsonl` gets one record per section in today's
  format.
- 2026-10-14, spec review round 1 (SR1-1 to SR1-9, one HIGH; round 2 follows), fixed in the text: SR1-1
  (AC-10: the key-form grant used once), SR1-2 (AC-2: a changed section names its other stale reasons),
  SR1-4 (a section confirmed and now missing is waiting), SR1-5 (`confirm::target` shared by `ok` and the
  guard), SR1-6 (bypass mode, allow rule and expired grant cases for the key form), SR1-7 (heading marker
  `==`), SR1-8 (two more `KEY` refusals), SR1-9 (AC-9 split into AC-9 and AC-11; the replaced command names
  the folder key). SR1-3 (outside: stale reasons in the dialog) is decided below.
- 2026-10-14: Decision (SR1-3, outside the promise, recommended): the human widened promise item 6: a section
  with an empty diff shows its stale reasons in the dialog. Rejected: the line count only.
- 2026-10-14, spec review round 2 (SR2-1 to SR2-4, all LOW), fixed in the text: SR2-1 (legacy key and a key
  matching two folders in the Input row; `confirm::target` returns a key target for any matching folder),
  SR2-2 (plan step 1 calls `confirm::target`), SR2-3 (size: eleven criteria, about 200 source and 900 test
  lines), SR2-4 (AC-11 in a dialog permission mode). Every SR1 finding confirmed fixed.
- 2026-10-14, after step 1 (drift review D1-1 to D1-9, one MEDIUM), one spec change: quoted output lines use
  the `<KEY>` placeholder (the builder had put the literal lines into a doc comment to satisfy
  `ticket_drift`); AC-1 says the records follow the `state.json` write (D1-5); AC-2 names the bare
  `unchanged` and the `, ` separator (D1-6, D1-7); AC-5 names `none` (D1-2); coverage step 1c for D1-1, D1-2
  and D1-7. D1-3 goes to step 2; D1-4, D1-8 and D1-9 are notes.

## Spec

Terms:

- **Confirmable section**: a section of `stapel.toml` whose owner is not `generated`.
- **Waiting section**: a confirmable section of the ticket without a fresh confirmation: never confirmed, or
  its latest confirmation is stale (`stapel_core::stage::freshness` gives a reason).
- **Key form**: `stapel ok KEY`. **Section form**: `stapel ok [KEY] <section>`.
- **Change block** of a section: what the person sees about it before its `confirmed:` line (AC-2).

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the key form runs in a terminal and the ticket has waiting sections THE `stapel ok` command prints `to confirm: <KEY> <ids joined by comma and space, stapel.toml order>`, then the change block of each, then one `confirmed:` line each (today's format); it appends one confirmation per waiting section (by, at, hash, normal form, depends_on, text, via `terminal`) in one write of `state.json`, then appends one `decisions.jsonl` record per section in today's format; fresh sections get neither (spec change, D1-5). Output: `golden/ok_key_form.txt`. | `ok_all::key_form_confirms_every_waiting_section` |
| AC-2 | WHEN a change block is printed THE `stapel ok` command prints `== <id>: changed since the last confirmation` (followed by `; <other stale reasons>` when there are any) and a unified diff from the stored text when the section's hash changed; `== <id>: new, N lines` (`1 line` for one) when it was never confirmed or its latest confirmation has no stored text; `== <id>: unchanged; <stale reasons>` when only a dependency or the normal form made it stale, and `== <id>: unchanged` for a fresh section confirmed again by the section form (spec change, D1-6). Several reasons are joined by `, `. Output: `golden/ok_change_blocks.txt`. | `ok_all::change_block_for_each_kind`, `ok_all::change_block_with_two_reasons` |
| AC-3 | IF a waiting section, or a dependency of one, is missing or duplicated in `ticket.md` THEN the key form exits 1 with the one-section form's reason for that section, and `state.json` and `decisions.jsonl` stay byte for byte. | `ok_all::key_form_refuses_all_when_one_fails` |
| AC-4 | WHEN the key form runs and no section waits THE `stapel ok` command prints `nothing to confirm: <KEY>` (the folder key), exits 0 and changes no file. | `ok_all::key_form_with_nothing_waiting` |
| AC-5 | WHEN `stapel ok` gets one argument THE command takes it as a section when it is a section id of `stapel.toml` (any case, as today), else as a key when a ticket folder matches (any case), else exits 1 with `"X" is neither a section nor a ticket; sections: <ids>; open tickets: <keys>` (`none` when no ticket is open; spec change, D1-2). A closed or legacy ticket gives today's refusal of `prepare`. | `ok_all::one_argument_is_section_then_key`, `ok_all::section_id_wins_over_a_ticket_folder`, `ok_all::refusal_lists_only_open_tickets` |
| AC-6 | WHEN the section form confirms a section THE `stapel ok` command prints its change block before the `confirmed:` line; a fresh section is confirmed again, as today. | `ok_all::section_form_prints_change_block` |
| AC-7 | WHEN an agent runs the key form in a dialog permission mode THE guard answers `ask`: the reason names the ticket and, per waiting section, its id, short hash and `+a -b lines since last confirmation` (with `; <stale reasons>` when both counts are 0) or `new, N lines`; the replaced command is `<program> ok <folder key> --grant <token>`; one grant file is written. | `ok_all::dialog_lists_every_waiting_section` |
| AC-8 | IF after the key-form dialog a waiting section's hash, one of its dependencies, or the set of waiting sections changes THEN `stapel ok KEY --grant <token>` exits 1 with today's "the grant does not match" reason and records nothing. A grant of the section form does not confirm the key form, nor the reverse. | `ok_all::key_form_grant_names_every_section` |
| AC-9 | IF the key form runs in an agent's shell without a grant THEN it exits 1 with today's "only a person confirms" reason, naming `stapel ok KEY`. | `ok_all::key_form_agent_shell_refused` |
| AC-10 | WHEN `stapel ok KEY --grant <token>` runs with the token of the key-form dialog and nothing changed THE command confirms every section the dialog listed, as AC-1, with `via` `grant`, and removes the grant; the same token then exits 1 with "the grant does not match". | `ok_all::key_form_grant_confirms_once` |
| AC-11 | IF an agent runs the key form in a dialog permission mode and no section waits THEN the guard denies with `nothing to confirm: <KEY>` and writes no grant file. | `ok_all::guard_denies_when_nothing_waits` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-11; the existing tests of `ok.rs` and `hook.rs` keep the
  section form's behaviour.
- **Not promised:** the Frame's list.

## Design

### Facts (external contract and states)

- F-1: `stapel_core::confirm::prepare` resolves the ticket and one section, refuses closed, legacy and
  unreadable tickets, and computes the hash, normal-form text and `depends_on`. Both `ok` and the guard's
  dialog call it, so the dialog refuses what the command would refuse.
- F-2: only the latest confirmation of a section keeps `text` (`ok.rs` clears older ones); confirmations
  written before STP-2 AC-10 have none, which AC-2 shows as `new, N lines`.
- F-3: `stage::freshness` gives the stale reasons; `stage::diff` gives the unified diff from the stored text;
  `Reason::describe` of `Changed` includes the confirmation time, so AC-2 prints its own heading for it to keep
  golden files stable.
- F-4: `config.section` and `tickets::resolve` match ids and keys in any case.
- F-5: a grant file name is a hash of the token and `Facts` (key, action, detail, hash, dependencies, crate
  version); `consume` removes the file whatever the outcome.
- F-6: the guard reads `stapel ok` arguments as words of the parsed command (`only_positional`); words built at
  run time are denied before `dialog::ask`.

### Decisions (before the build)

- The key form uses grant `Facts` with action `ok-key`, detail the waiting ids joined by `,`, hash the hashes
  joined by `,` in the same order, dependencies `id:dep=hash` of every waiting section. The section form keeps
  action `ok`. So neither grant opens the other form (AC-8).
- One function in `stapel-core` (`confirm::prepare_waiting`) returns the waiting sections, each as a
  `Prepared`, or the first refusal; the CLI and the guard share it.
- One function in `stapel-core` (`confirm::target`) applies AC-5's one-argument rule and returns a section
  target or a key target, or AC-5's refusal; `ok.rs` uses it in step 1 and `dialog::prepare_ask` in step 2, so
  the rule has one copy (SR1-5). A key target is returned whenever any ticket folder matches ignoring case, so
  `tickets::resolve` gives its own refusal for several matching folders (SR2-1).
- The change block is built in `stapel-core` (no printing there) and printed by the CLI.

### Decisions (as built)

#### Step 1

- `confirm::target(root, config, arg)` returns `Target::Section(arg)` when `config.section` matches, generated
  sections included (so `prepare` refuses them as today); `Target::Key(arg)` when any ticket folder matches
  ignoring case; else AC-5's refusal with the confirmable ids and the open keys, or `none`.
- `confirm::prepare_waiting(root, config, key)` returns one `Prepared` per waiting section in `stapel.toml`
  order by calling `prepare` for each, so a refusal is `prepare`'s reason. A missing dependency is itself a
  waiting section, so the refusal names the dependency (D1-8).
- `confirm::change_block(config, &Prepared)` is called before the new confirmation is added; it reads
  `ticket.md` again for the other stale reasons and treats a read error as no reasons (D1-7, note).
- `ok::run_one` resolves the argument with `target`; in an agent shell without a grant an unresolved argument
  gives the section-form refusal, keeping today's order of checks. `refuse_agent_shell(form)` names the form
  typed: `stapel ok KEY` for the key form.
- The key form computes the identity once (the spaced `user.name` warning is printed once) before
  `prepare_waiting`, as the section form does, so a missing `user.name` refuses even when nothing waits
  (D1-4, note). It writes `state.json` once, clears older stored text per section, prints the `confirmed:`
  lines, then appends the `decisions.jsonl` records.
- Until step 2 the key form refuses a grant with "a grant for `stapel ok KEY` is not supported yet" (D1-3).

## Test plan

CLI tests in a new file `crates/stapel-cli/tests/ok_all.rs`, with `common::repo_with_ticket` (Spec, Design,
Proof set; Plan is `TODO`), `common::set_section`, `common::ask`, `common::hook_bash` and `common::token_of`.
Golden files under `crates/stapel-cli/tests/golden/`; hashes in them are stable because the fixture text is
fixed; no time is printed. Light tests: `cargo test -j 4 -p stapel-cli --test ok_all -- --test-threads=3`.

| Category | Cases | Tests |
|---|---|---|
| Main path | four sections waiting in mixed states; three records, one fresh section untouched; order of output and records | `ok_all::key_form_confirms_every_waiting_section` |
| Output | changed (diff), changed and a dependency changed (both in the heading), never confirmed, latest without stored text, dependency only, normal form only, one line | `ok_all::change_block_for_each_kind` |
| Negative | duplicated `## Design`; missing `## Plan` heading; missing dependency; a section confirmed before and now missing is waiting, so the key form refuses with `section plan (## Plan) is missing` | `ok_all::key_form_refuses_all_when_one_fails` |
| Empty | every section fresh: message, exit 0, files unchanged | `ok_all::key_form_with_nothing_waiting` |
| Input | `spec`, `SPEC`, `abc-1`, `ABC-1`, `review` (generated: today's refusal), `nope`, a closed ticket key, a legacy ticket key, a key matching two folders that differ only in case (Linux) | `ok_all::one_argument_is_section_then_key` |
| Regression | section form prints the change block; fresh section confirmed again | `ok_all::section_form_prints_change_block` |
| Coverage (step 1) | a ticket folder named `spec`: `stapel ok spec` and `stapel ok SPEC` take the section form (D1-1); the refusal lists open tickets only, and `none` (D1-2); a section with a changed dependency and an old normal form shows both reasons joined by `, ` (D1-7) | `ok_all::section_id_wins_over_a_ticket_folder`, `ok_all::refusal_lists_only_open_tickets`, `ok_all::change_block_with_two_reasons` |
| Guard | dialog reason and replaced command for `stapel ok ABC-1` and the `cargo run … --` form; a section stale only by a dependency shows `+0 -0 lines since last confirmation; depends on spec, which changed`; `bypassPermissions` mode denies; an allow rule `Bash(stapel ok:*)` denies | `ok_all::dialog_lists_every_waiting_section` |
| Guard | grant voided by an edit of a waiting section, of a dependency, by a new waiting section; section-form grant on the key form and the reverse; an expired key-form grant | `ok_all::key_form_grant_names_every_section` |
| Guard | agent shell without grant | `ok_all::key_form_agent_shell_refused` |
| Guard | the key-form grant used once (`via` `grant`, grant removed), then refused | `ok_all::key_form_grant_confirms_once` |
| Guard | nothing waiting gives a deny and no grant file | `ok_all::guard_denies_when_nothing_waits` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` (positional) | ticket folder name, any case | one ticket; several open tickets | absent: section form with the only open ticket, or today's usage error with no argument | no such folder, closed, legacy, unreadable `state.json`, several folders differing only in case: today's refusal | exact match ignoring ASCII case | `ok_all::one_argument_is_section_then_key` |
| `SECTION` (positional) | section id of `stapel.toml` | one confirmable section | absent with `KEY`: key form | generated or unknown: today's refusal | exact match ignoring ASCII case | `ok_all::one_argument_is_section_then_key`, `ok_all::section_form_prints_change_block` |
| `--grant` | 32 hex characters, set by the guard | one grant per dialog | absent: terminal path, or the agent refusal | wrong, expired, reused, other form: "does not match" or "expired" | exact | `ok_all::key_form_grant_names_every_section`, `ok_all::key_form_grant_confirms_once`, `ok_all::key_form_agent_shell_refused` |
| Stored `text` of the latest confirmation | normal-form string | absent to the section's size | absent: `new, N lines` | — | line diff | `ok_all::change_block_for_each_kind` |
| Set of waiting sections | ids in `stapel.toml` order | none to all four | none: AC-4 | a waiting section missing or duplicated: AC-3 | — | `ok_all::key_form_refuses_all_when_one_fails`, `ok_all::key_form_with_nothing_waiting` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | permission mode; whether the dialog is offered | as today: dialog modes ask, others deny | `ok_all::dialog_lists_every_waiting_section` |
| claude-code | environment of hook processes versus the agent's Bash shell | as today: an agent shell without grant is refused | `ok_all::key_form_agent_shell_refused` |
| claude-code | allow rules that skip the dialog | as today: a matching allow rule denies (the key form's command is checked) | `ok_all::dialog_lists_every_waiting_section` |
| claude-code | transcripts; settings levels other than allow rules | not touched | — |
| git | configuration: `user.name` | as today: blank refused, spaced warned once per command | `ok_all::key_form_confirms_every_waiting_section` |
| git | other dimensions | not touched: the key form reads no history | — |
| os | case-insensitive file systems | keys and ids match in any case (F-4) | `ok_all::one_argument_is_section_then_key` |
| os | symlinks, permissions, processes, time, WSL | not touched | — |
| cargo | every dimension | not touched | — |

**Review Focus:** a section stale only by a dependency (diff empty); a latest confirmation without stored
text; the set of waiting sections changing between the dialog and the run; a key equal to a section id (not
promised: the section wins); a spaced `user.name` warned once, not per section. Each has a case in the named
tests or a line in the promise.

**Author self-check:** 2026-10-14, before spec review: every promise item has a criterion and a test; the
Inputs and External states rows were checked against the code paths of `ok.rs`, `confirm.rs`, `dialog.rs`
and `tickets::resolve`, not only the catalogue; no open question is left.
2026-10-14, after spec review round 1: every inside finding applied; round 2 for SR1-1 (HIGH).
2026-10-14, after spec review round 2: SR2-1 to SR2-4 (all LOW) applied; no round 3.
2026-10-14, after step 1: design as built moved into the ticket; drift findings D1-1 to D1-9 applied as one
spec change (quoted lines use `<KEY>`, AC-1, AC-2 and AC-5 wording) and plan step 1c.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `ok_all::key_form_confirms_every_waiting_section` |
| AC-2 | `ok_all::change_block_for_each_kind`, `ok_all::change_block_with_two_reasons` |
| AC-3 | `ok_all::key_form_refuses_all_when_one_fails` |
| AC-4 | `ok_all::key_form_with_nothing_waiting` |
| AC-5 | `ok_all::one_argument_is_section_then_key`, `ok_all::section_id_wins_over_a_ticket_folder`, `ok_all::refusal_lists_only_open_tickets` |
| AC-6 | `ok_all::section_form_prints_change_block` |
| AC-7 | `ok_all::dialog_lists_every_waiting_section` |
| AC-8 | `ok_all::key_form_grant_names_every_section` |
| AC-9 | `ok_all::key_form_agent_shell_refused` |
| AC-10 | `ok_all::key_form_grant_confirms_once` |
| AC-11 | `ok_all::guard_denies_when_nothing_waits` |

## Plan

Route: full (source: `auto`; tags `interface`, `guard`).

| Step | What | Criteria | Must not change |
|---|---|---|---|
| 1 | `confirm::target`, `confirm::prepare_waiting` and the change block in `stapel-core`; the key form and the change block in `ok.rs` and `main.rs`, calling `confirm::target` | AC-1 to AC-6 | `grant.rs`, `dialog.rs`, the section form's records, existing tests |
| 1c | Coverage after the drift review of step 1: D1-1, D1-2, D1-7 (tests only, no code) | AC-2, AC-5 | `crates/*/src`, the six tests of step 1 |
| 2 | The key form in the guard's dialog and its grant facts; the agent-shell message | AC-7 to AC-11 | the section form's dialog reason and grant facts, `grant.rs` file naming |

## Review

TODO

## Summary

TODO
