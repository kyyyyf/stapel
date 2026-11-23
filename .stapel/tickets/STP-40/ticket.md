# STP-40 — The inbox of the person's decisions

## Description

Source: `docs/PLAN.md`, phase 3c, item 3c.2; ledger 2026-11-20 (the editor returns to the plan).

### Frame

**Problem.** The decisions a person owes are spread over the open tickets: findings that wait for the
person sit in each ticket's `findings.jsonl`, sections that wait for `stapel ok` in each `state.json`, and
the tickets live in several worktrees. The person learns of them only when an orchestrator asks in the chat,
and answers only there. STP-39 gives buttons inside one ticket, not a list of what waits across tickets.

**Plan item:** 3c.2.

**Risk tags:** `interface` (`stapel inbox`, `stapel answer`, the format of `inbox.md`, the new buttons),
`guard` (`stapel answer` writes the person's answer, which an agent may not write), `data` (it appends
triage records to `findings.jsonl`).

**Non-goals.** The measured run of a ticket from the editor against the chat (the plan's measure of 3c.2;
it is taken on the next ticket and recorded at its close); `Open question` lines of the Decisions logs;
questions of the Frame (they have no record); a change of the chat path (`stapel findings triage` and
`stapel ok` stay as they are); an answer through a permission dialog; inboxes of several repositories;
watching files on disk; notifications outside the editor.

**Promise (closed list).**

1. List: `stapel inbox` writes `.stapel/inbox.md` in the root it runs in, with one entry per owed decision
   of every open ticket of the repository, in this checkout and in every worktree of `git worktree list`:
   each finding that waits for the person (the rule of `stapel next` for `ask-human`) and each ticket with
   a section that waits for `stapel ok`. Each entry names the ticket, its worktree path and the item; an
   empty inbox says so. A ticket whose files cannot be read gets one line that says so.
2. Answer: `stapel answer KEY ID <fate> <note>` appends to the ticket's `findings.jsonl` the triage record
   `stapel findings triage` would append for that finding, fate and note, marked `by: person`. It refuses a
   finding that does not wait for the person, an unknown id, a fate outside the fate list and an empty
   note, and appends nothing then.
3. Guard: `stapel answer` refuses in an agent's shell, as `stapel ok` does, and the PreToolUse guard denies
   it for an agent; it has no `--grant` form.
4. Buttons: in an open `.stapel/inbox.md` the server of STP-39 offers per finding one button per fate a
   person gives (`patch`, `bad_plan`, `defer`, `no-change`), which takes the note from the entry's `note:`
   line in the buffer and runs `stapel answer` in that ticket's worktree; per waiting ticket a button
   `stapel page KEY` in that worktree; and one button `stapel inbox` that writes the file again. After a
   run that ran, the server writes the inbox again.
5. Setup: `stapel init` adds `/.stapel/inbox.md` to the ignore lines it writes.

Not promised: an answer between the writing of `inbox.md` and the click that another session already gave
(the command refuses it by item 2, the stale entry stays until the next write); clients without code
actions; worktrees outside `git worktree list`; deliberate
circumvention of the guard (no `security` tag).

**Prior art.** STP-18 and STP-23 (records, `stapel findings triage`, `is_persons`), STP-17 and STP-2 (the
agent-shell refusal of `stapel ok`), STP-36 (the `open:` lines of open tickets in worktrees), STP-39 (the
language server, buttons and their run rules).

**Size.** About 450 source lines (`stapel inbox`, `stapel answer`, the guard entry, the inbox buttons) and
about 600 test lines; 11 Plan steps. Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-23: Decision (the view, the human, recommended): `stapel inbox` writes `.stapel/inbox.md` for
  every open ticket of every worktree, and the language server gives a button per answer in that file.
  Rejected: the terminal only; diagnostics in `ticket.md` only.
- 2026-11-23: Decision (the items, the human, recommended): findings that wait for the person and tickets
  with sections that wait for `stapel ok`. Rejected: the `Open question` lines as well; findings only.
- 2026-11-23: Decision (the answer, the human, recommended): a new command `stapel answer KEY ID <fate>
  <note>` appends the triage record marked as the person's (`by: person`, the value a person's `stapel findings triage` already writes) and refuses in an agent's shell. Rejected: a
  triage file through `stapel findings triage` marked `by: agent`; no write-back.
- 2026-11-23: Decision (the order, the human, recommended): the Frame, the spec and its review now; the
  build waits until STP-39 is closed and merged into main, because `stapel-lsp` is only in its branch.
  `stapel depends` records the dependency. Rejected: a branch from STP-39; the command-line part first.

## Spec

Terms:

- **Inbox ticket**: an open ticket named by the branch of a live worktree of `git worktree list`, this
  checkout included, chosen as the `open:` lines of `stapel next` choose them (`parallel::worktrees`,
  `tickets::branch_ticket`; a prunable worktree or a missing folder is skipped).
- **Owed finding**: a finding of an inbox ticket's `findings.jsonl` for which `triage::State::is_persons`
  holds, the rule by which `stapel next` names `ask-human`.
- **Owed confirmation**: an inbox ticket whose `stapel next` action, computed in its worktree, is
  `write-page` or `confirm`.
- **Person's fate**: one of `patch`, `bad_plan`, `defer`, `no-change`.
- **Agent shell**: `identity::in_agent_shell` holds (`CLAUDECODE` or `CLAUDE_CODE_ENTRYPOINT` is set).
- **Inbox document**: a `file:` URI whose path is `<root>/.stapel/inbox.md` where
  `<root>/.stapel/stapel.toml` loads; `<root>` is the **inbox root**.
- **Entry**: the lines of an inbox document from a `### ` heading to the next heading; a **finding entry**
  has the heading `### <KEY> <ID>`, a **confirmation entry** `### <KEY> confirmation`. Lines are split as
  `ticket::scan` splits.
- **Button** and the run report of a button are those of STP-39 (its Terms and AC-4).

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel inbox` runs THE command writes `.stapel/inbox.md` in the root as the golden file `inbox.md` shows: tickets sorted by key, each with `## <KEY> — <title>` and `worktree: <path>`, then per owed finding in file order `### <KEY> <ID>`, the line `<stage> round <round>, <severity>, <promise>: <claim>` and an empty `note:` line; it prints `inbox: <n> decisions wait in .stapel/inbox.md` and exits 0. | `inbox::lists_owed_findings` |
| AC-2 | WHEN an inbox ticket owes a confirmation THE entry `### <KEY> confirmation` follows its findings with the lines `waiting: <section ids, comma separated>` and `page: <worktree>/.stapel/tickets/<KEY>/page.md`, and counts as one decision. | `inbox::lists_owed_confirmations` |
| AC-3 | IF no inbox ticket owes a decision THEN `inbox.md` holds the title line and `Nothing waits.`, and the command prints `inbox: nothing waits`. | `inbox::nothing_waits` |
| AC-4 | WHEN several live worktrees have open branch tickets THE inbox lists the owed decisions of each, this checkout's included; a closed ticket, a branch that names no ticket and a prunable worktree give nothing, and a ticket that owes nothing gives no heading. | `inbox::tickets_of_every_worktree` |
| AC-5 | IF the `state.json` or `findings.jsonl` of an inbox ticket cannot be read, or its `stapel next` fails THEN its heading is followed by `unreadable: <reason>`, it counts as one decision, and the other tickets are listed. | `inbox::unreadable_ticket` |
| AC-6 | WHEN `stapel answer KEY ID FATE NOTE` runs outside an agent shell on an owed finding of the checkout's ticket with a person's fate THE command appends to its `findings.jsonl` the line `{"kind":"triage","ticket":"<KEY>","id":"<ID>","fate":"<FATE>","note":"<NOTE>","imported_by":{"by":"person","user":"<user>"}}`, prints `answered: <ID> of <KEY>: <FATE>`, exits 0, and `stapel next` no longer names the finding. | `answer::appends_a_person_triage` |
| AC-7 | IF the id is no finding of the ticket, the finding is not owed, the fate is not a person's fate, the note is empty after trimming, the record line would exceed the journal's line limit, or KEY is not the checkout's branch ticket THEN `stapel answer` prints `stapel: <reason>` (texts in Design), exits 2 and appends nothing. | `answer::refusals` |
| AC-8 | IF `stapel answer` runs in an agent shell THEN it prints `stapel: only a person answers a finding: this shell belongs to an agent (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set)`, exits 1 and appends nothing; no option makes it run there. | `answer::refuses_in_an_agent_shell` |
| AC-9 | WHEN an agent's Bash command runs `stapel answer`, alone, in a compound command or as `stapel cargo run -- answer` THE PreToolUse guard denies it with `stapel answer records a person's answer; an agent asks the person in the chat and runs stapel findings triage`. | `hook::denies_stapel_answer` |
| AC-10 | WHEN `stapel init` writes its ignore lines THE list holds `/.stapel/inbox.md`. | `init::ignores_the_inbox` |
| AC-11 | WHEN a code action request without an `only` filter names an open inbox document THE server returns, inside a finding entry, the buttons `answer <ID>: <fate>` for the four person's fates in their order, inside a confirmation entry `stapel page <KEY>`, and then `stapel inbox` at any range; an inbox document gets no diagnostic. | `lsp::inbox_buttons` |
| AC-12 | WHEN the button `stapel.answer` runs with the inbox URI, KEY, ID and a person's fate THE server takes the note from the first `note:` line of the entry `### <KEY> <ID>` in the buffer (the text after `note:`, trimmed), runs `stapel answer KEY ID FATE NOTE` in the live worktree whose branch names KEY, reports the run as STP-39 AC-4 does, and then writes the inbox again in the inbox root. | `lsp::answers_from_the_inbox` |
| AC-13 | IF the buffer has no entry `### <KEY> <ID>`, its note is empty, no live worktree has a branch that names KEY, or the server runs in an agent shell THEN the server runs nothing, answers `null` and shows an Error message (texts in Design); IF the command name or the arguments are not those of AC-12 or AC-14 THEN it answers `InvalidParams` as STP-39 AC-5 does. | `lsp::inbox_refusals` |
| AC-14 | WHEN the button `stapel.page` runs from a confirmation entry, or `stapel.inbox` from an inbox document THE server runs `stapel page KEY` in the worktree of KEY, or `stapel inbox` in the inbox root, and after every run that ran writes the inbox again. | `lsp::inbox_written_again_after_a_run` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-1 to AC-5, item 2 by AC-6 and AC-7, item 3 by AC-8
  and AC-9, item 4 by AC-11 to AC-14, item 5 by AC-10. Unchanged: `stapel findings triage`, `stapel ok`,
  the buttons of `ticket.md` and `page.md`, every other command.
- **Not promised:** the Frame's list; an entry the editor shows after the file changed on disk and before
  the editor reloads it; two answers to one finding from two sessions (the second is refused by AC-7).

### Abuse table

| Attempt of a careless agent | Outcome | Test |
|---|---|---|
| runs `stapel answer` in its Bash tool to close a finding it does not want to ask about | the guard denies it | `hook::denies_stapel_answer` |
| runs it as `cd x && stapel answer ...` or `stapel cargo run -- answer ...` | the guard denies it | `hook::denies_stapel_answer` |
| runs the binary from a script the guard does not parse | the binary refuses in the agent shell | `answer::refuses_in_an_agent_shell` |
| starts `stapel lsp` from its shell and sends `stapel.answer` | agent shell: nothing runs, Error message | `lsp::inbox_refusals` |
| writes the triage line into `findings.jsonl` by hand with `by: person` | the guard denies the write of a machine file, as today (STP-18); unchanged here | — |
| edits the `worktree:` line of an entry to point the answer at another checkout | the server ignores the line and takes the worktree from `git worktree list` | `lsp::answers_from_the_inbox` |
| writes a note into the `note:` line and asks the person to press a fate | the person sees the note in the buffer before the click; the record holds it; not refused | — |
| answers `intent_gap` or `open` to keep a finding open for later | not a person's fate: refused | `answer::refusals` |
| unsets `CLAUDECODE` with `env -u` and runs the binary from a script | deliberate circumvention, not promised (no `security` tag) | — |

**Author self-check (item 7).** Done on 2026-11-23: each row maps to promise item 3 or 4 or to the Not
promised list; `stapel answer` writes nothing `stapel findings triage` in a person's terminal cannot.

## Test plan

Integration tests in `crates/stapel-cli/tests/`: the new files `inbox.rs` and `answer.rs`, and the
existing `hook.rs`, `init.rs` and `lsp.rs` (from STP-39). Fixture repositories come from `stapel init` and
`stapel new` in a temporary folder; worktrees from `stapel new` in a fixture with parallel tickets, as in
`parallel.rs`; findings from `stapel findings import`. The tests remove the agent variables, as
`common::stapel` does, and set them again only in the agent shell tests. Light tests: `stapel cargo test
-j 4 -p stapel-cli --test inbox --test answer -- --test-threads=4`; before each GREEN also `--test
ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Findings | one ticket with an `intent_gap`, an `outside` open finding, a round 2 HIGH code review finding and a round 1 MEDIUM one (not owed), a LOW one (not owed), a triaged one (not owed): three entries in file order, the golden file | `inbox::lists_owed_findings` |
| Confirmations | a ticket whose next action is `write-page`; one with `confirm`; one in its build (no entry); waiting ids in config order | `inbox::lists_owed_confirmations` |
| Empty | a fixture with no open ticket; a ticket that owes nothing; `inbox.md` written over an old one | `inbox::nothing_waits` |
| Worktrees | two worktrees with tickets that owe; this checkout on a ticket branch; a closed ticket; a branch `feature` with no ticket; a worktree folder deleted (prunable) | `inbox::tickets_of_every_worktree` |
| Unreadable | a `findings.jsonl` that is a folder; a `state.json` that does not parse; the other ticket still listed | `inbox::unreadable_ticket` |
| Answer | each person's fate on an owed finding; the line byte for byte; the user name; `stapel next` afterwards | `answer::appends_a_person_triage` |
| Refusals | unknown id; a triaged finding; a LOW finding; fate `open`, `intent_gap`, `PATCH`; a note of spaces; a note of 70 000 bytes; KEY of another ticket than the branch's; a closed ticket; `findings.jsonl` unchanged each time | `answer::refusals` |
| Agent shell | `CLAUDECODE=1`; `CLAUDE_CODE_ENTRYPOINT=cli`; `findings.jsonl` unchanged | `answer::refuses_in_an_agent_shell` |
| Guard | `stapel answer STP-1 F-1 patch x`; `cd a && stapel answer ...`; `STAPEL answer` in another case; `stapel cargo run -- answer ...`; `stapel findings triage f` still allowed | `hook::denies_stapel_answer` |
| Init | a fresh `stapel init`: `.gitignore` holds `/.stapel/inbox.md`; Local mode: `info/exclude` holds it | `init::ignores_the_inbox` |
| Inbox buttons | a range in a finding entry, in a confirmation entry, on the title line; the order; an `only` filter; no diagnostic after `didOpen` | `lsp::inbox_buttons` |
| Inbox answer | a note `ok, fix it`; the record in the worktree's `findings.jsonl`; the Info message; `inbox.md` written again without the entry; an entry whose `worktree:` line names another folder | `lsp::answers_from_the_inbox` |
| Inbox refusals | no entry for the id; `note:` empty; a branch of KEY with no live worktree; an agent shell; fate `open`; three arguments; a URI that is not an inbox document | `lsp::inbox_refusals` |
| Page and inbox | `stapel.page` from a confirmation entry writes the page in the worktree; `stapel.inbox` writes the file; the file is written again after a run that failed | `lsp::inbox_written_again_after_a_run` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `stapel inbox` arguments | none | none / none | the only form | any argument: clap refuses, exit 2 | — | `inbox::nothing_waits` |
| `stapel answer` KEY | string | the template with `1` / any number | clap refuses, exit 2 | another ticket than the branch's: exit 2 | as `tickets::resolve`: case ignored | `answer::refusals` |
| `stapel answer` ID | string | one character / any | clap refuses, exit 2 | no finding of that id: exit 2 | exact | `answer::refusals` |
| `stapel answer` FATE | string | — | clap refuses, exit 2 | not a person's fate: exit 2 | exact, case kept | `answer::refusals` |
| `stapel answer` NOTE | string | one non-space character / the journal line limit | empty or spaces: exit 2 | over the limit: exit 2 | kept as given | `answer::refusals` |
| `stapel.answer` arguments | JSON array | four strings | none: `InvalidParams` | not four strings, not an inbox URI, a fate not a person's: `InvalidParams` | exact | `lsp::inbox_refusals` |
| `note:` line in the buffer | text | one non-space character / one line | empty: Error message | — | trimmed | `lsp::inbox_refusals` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | worktrees: live, prunable, folder missing; branch naming no ticket | listed, skipped, skipped, skipped | `inbox::tickets_of_every_worktree` |
| git | no worktree for KEY at the run | Error message, nothing runs | `lsp::inbox_refusals` |
| git | the checkout's branch names another ticket than KEY | `stapel answer` refuses | `answer::refusals` |
| file system | `findings.jsonl` or `state.json` unreadable | `unreadable:` line | `inbox::unreadable_ticket` |
| file system | `inbox.md` exists, from an older run | written over | `inbox::nothing_waits` |
| file system | `inbox.md` cannot be written | exit 1 with the cause (not tested) | — |
| environment | `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT` | `stapel answer` and the inbox buttons refuse | `answer::refuses_in_an_agent_shell`, `lsp::inbox_refusals` |
| claude-code | the PreToolUse guard | denies `stapel answer` | `hook::denies_stapel_answer` |
| editor | the buffer of `inbox.md` differs from disk | the note comes from the buffer; the entry list from the buffer | `lsp::answers_from_the_inbox` |
| editor | reload of `inbox.md` after the server writes it | not promised | — |
| child process | `stapel answer` exits non-zero | the Error message of STP-39 AC-4; the inbox is written again | `lsp::inbox_written_again_after_a_run` |
| cargo | none read | — | — |
| os | symlinked worktree folders | not promised | — |

**Review Focus:** a finding whose `claim` holds a line break (the entry must stay one line; tested only by
the golden file's ordinary claims); two worktrees on branches of the same key in another case (`branch_ticket`
compares without case; not tested); an inbox written while another session triages (the stale entry stays
until the next write; not tested); an inbox root that is a worktree, not the main checkout (the worktree
list is the same; tested by `inbox::tickets_of_every_worktree` from the main checkout only); a note with
a `"` or a backslash (JSON escaping by `serde_json`; tested in `answer::appends_a_person_triage`).

## Design

### Facts

- F-1: `triage::State::is_persons` and `triage::state_of` give a finding's state; `next.rs` turns them into
  the `ask-human` action. `records::importer_json(false, user)` gives `{"by":"person","user":...}`.
- F-2: `stapel findings triage` appends through `triage::plan_triage` and `journal`; `journal::MAX_LINE`
  is 64 KiB.
- F-3: `next.rs::open_lines` walks `parallel::worktrees` and `tickets::branch_ticket` and computes each
  ticket's action with `text_at(worktree)`.
- F-4: `guard.rs::check_stapel` reads the action word of `stapel` and of `stapel cargo run --`.
- F-5: `init.rs` holds the ignore lines (`/.stapel/tickets/*/page.md` among them) and writes them to
  `.gitignore` or, in Local mode, to `info/exclude`.
- F-6: STP-39's `stapel-lsp` keeps the open buffers, classifies a URI, offers buttons, refuses in an agent
  shell and runs `current_exe()` with arguments in a root.

### Shape

- `stapel-core::inbox`: `Entry` (key, title, worktree, kind: finding with id and line, confirmation with
  ids, unreadable with reason) and `render(entries) -> String`; `parse(text) -> Vec<EntryRef>` that the
  server uses to find the entry at a line and its note.
- `stapel-cli`: `inbox.rs` collects the entries (the walk of `open_lines`, the findings by `is_persons`,
  the action by `text_at`) and writes the file; `answer.rs` checks and appends one triage line through the
  same journal call as `findings triage`; `main.rs` gains `Inbox` and `Answer`.
- `stapel-core::guard`: `answer` in `check_stapel` sets a new reason.
- `stapel-lsp`: `classify` also knows the inbox document; the commands `stapel.answer` and `stapel.inbox`
  join `COMMANDS`; `stapel.page` accepts an inbox URI plus a key; the worktree of a key comes from
  `parallel::worktrees` and `tickets::branch_ticket`, never from the buffer.

Refusal texts of `stapel answer` (AC-7): `<ID> is not a finding of <KEY>`; `<ID> of <KEY> does not wait
for a person`; `the fate must be one of patch, bad_plan, defer, no-change`; `the note is empty`; `the
record is longer than the journal line limit`; the refusal of `tickets::match_branch` for another key.
Error messages of the server (AC-13): `stapel lsp: <KEY> <ID> is not in this inbox; run stapel inbox`;
`stapel lsp: write the note after note: in the entry of <KEY> <ID> first`; `stapel lsp: no worktree has
the branch of <KEY>`; the agent shell message of STP-39 AC-6.

### Decisions (before the build)

- 2026-11-23, agent: the note comes from a `note:` line in the buffer, because LSP has no standard request
  for a line of text; the person writes it and then presses the fate.
- 2026-11-23, agent: a person's fate excludes `open` and `intent_gap`, which would leave the finding with
  the person.
- 2026-11-23, agent: an owed confirmation is a ticket whose next action is `write-page` or `confirm`, not
  every unconfirmed section, so a Frame still in writing owes nothing.
- 2026-11-23, agent: the server takes the worktree of a key from git, so an edited `worktree:` line
  points nowhere.

### As built

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `inbox::lists_owed_findings` |
| AC-2 | `inbox::lists_owed_confirmations` |
| AC-3 | `inbox::nothing_waits` |
| AC-4 | `inbox::tickets_of_every_worktree` |
| AC-5 | `inbox::unreadable_ticket` |
| AC-6 | `answer::appends_a_person_triage` |
| AC-7 | `answer::refusals` |
| AC-8 | `answer::refuses_in_an_agent_shell` |
| AC-9 | `hook::denies_stapel_answer` |
| AC-10 | `init::ignores_the_inbox` |
| AC-11 | `lsp::inbox_buttons` |
| AC-12 | `lsp::answers_from_the_inbox` |
| AC-13 | `lsp::inbox_refusals` |
| AC-14 | `lsp::inbox_written_again_after_a_run` |

Stand (item 4): Zed with the dev extension of STP-39 and `stapel` on `PATH`, opened on `.stapel/inbox.md`
of a fixture with one owed finding; the Review section records the buttons seen and one answer.

## Plan

Route: full (tags `interface`, `guard`, `data`), auto. Steps 8 to 11 wait for the close of STP-39 and its
merge into main.

1. Owed findings (AC-1; test `inbox::lists_owed_findings` with the golden file `inbox.md`): the GREEN adds
   `stapel-core::inbox` and the subcommand `inbox` for the checkout's branch ticket. Must not change:
   `stapel next`.
2. Confirmations and the empty inbox (AC-2, AC-3; tests `inbox::lists_owed_confirmations`,
   `inbox::nothing_waits`): the GREEN adds the confirmation entry and the empty form. Must not change: the
   finding entries.
3. Every worktree (AC-4, AC-5; tests `inbox::tickets_of_every_worktree`, `inbox::unreadable_ticket`): the
   GREEN walks the live worktrees and writes the `unreadable:` line. Must not change: the `open:` lines.
4. The answer (AC-6; test `answer::appends_a_person_triage`): the GREEN adds the subcommand `answer`.
   Must not change: `stapel findings triage`.
5. Answer refusals (AC-7; test `answer::refusals`, which fails at the RED because the command appends):
   the GREEN checks the finding, the fate, the note, the line limit and the branch before it writes. Must
   not change: a valid answer.
6. Agent shell and guard (AC-8, AC-9; tests `answer::refuses_in_an_agent_shell`,
   `hook::denies_stapel_answer`): the GREEN refuses in an agent shell and adds the guard reason. Must not
   change: the other guard reasons.
7. Ignore line (AC-10; test `init::ignores_the_inbox`): the GREEN adds `/.stapel/inbox.md` to the ignore
   lines and runs `stapel init` in this repository (message line `history: the installed .gitignore gains
   the inbox`). Must not change: the other installed files.
8. Inbox buttons (AC-11; test `lsp::inbox_buttons`): the GREEN classifies the inbox document and offers
   its buttons. Must not change: the buttons of ticket documents.
9. Answer from the inbox (AC-12; test `lsp::answers_from_the_inbox`): the GREEN runs `stapel answer` in
   the worktree of the key with the buffer's note. Must not change: the run report of STP-39.
10. Inbox refusals (AC-13; test `lsp::inbox_refusals`, which fails at the RED because the run goes ahead):
    the GREEN checks the entry, the note, the worktree and the agent shell before the run. Must not
    change: a valid answer.
11. Page, inbox and rewrite (AC-14; test `lsp::inbox_written_again_after_a_run`): the GREEN runs
    `stapel page` and `stapel inbox` from the inbox and writes the inbox again after a run. Must not
    change: the refresh of `ticket.md` diagnostics.

## Review

TODO

## Summary

TODO
