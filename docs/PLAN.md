# stapel — the plan

One document, current as of 2026-11-20 (ledger decision of that day). What was planned before, with the
phases 0 to 2, the pilot and the gate, is in `docs/HISTORY.md`; the product direction is
`docs/decisions/0004`. An item below is added, moved, split, widened or removed only by a dated `decision`
record in `.stapel/process-ledger.jsonl`, made with the human in the chat and named here; an agent never
does it on its own.

## 1. What stapel is

`stapel` takes a development ticket from description to merge so that the result is repeatable, the
process is understandable without reading the internals, and token spending is measured from day one. It is
for companies and departments that build software they could not afford before and want it to stay
changeable at a known cost: first a department with one technical person, then a founder who builds alone.
It is not for fast hypothesis checks; an agent alone serves those. It works on new and existing
repositories alike, for features, bug fixes and refactoring.

The person works in the Claude Code chat and types no stapel command. The orchestrator (the Claude Code
main agent with the stapel skill) asks the questions, writes the ticket, runs builders and reviewers as
subagents, and confirms with the person through the permission dialog; `stapel` is the backbone it calls.
The merge request in GitLab or GitHub is the second interface. The name means a slipway: the place where a
ship is built step by step before it is launched.

## 2. Principles

1. Agreements become verifiable artifacts: an acceptance criterion is a test, and a claim is marked
   verified, read or assumption.
2. Determinism comes from code: whatever a program can check, the program checks; the model is responsible
   only for judgment.
3. State is facts, the phase is computed from them.
4. Semantic checks stop the process, format errors the tool fixes, advice is visible and does not block.
5. People make only decisions, and every decision is recorded with who, when and why.
6. One live author session per ticket; independent agents only for review.
7. The index answers questions instead of guessing files; its product is the blast radius of a change.
8. Tokens are counted from day one, by role, marked measured or estimate.
9. One binary, no environment on the developer's machine.
10. The process is data: sections, checks, roles and models live in TOML and Markdown.

## 3. What exists (2026-11-20)

The process is the skill `stapel init` installs: a ticket with a Frame, a promise, criteria written as
tests, a plan of RED and GREEN steps, a spec review, builders per step, a drift review, code review rounds
with findings and fates, and a close that needs a passing `stapel check`. The binary has `init`, `new`,
`ok`, `page`, `status`, `next`, `brief`, `depends`, `export`, `close`, `tokens`, `check` (the cargo and
junit runners, in the CI of this repository since item 3.2), `lint`, `cargo`, `findings`, `review`,
`ledger` and the hooks. Tickets build in parallel in worktrees. The pilot on a foreign TypeScript
repository (three tickets, `docs/HISTORY.md` part 2, stage B) is over; its reports are in
`.stapel/scratch/pilot/`.

## 4. How success is measured

Per ticket: the returns to the person (permission dialogs, question dialogs, typed messages) and the
minutes of interaction; escaped defects, counted by the project's CI after the merge and by a mutation run
on the diff; HIGH and MEDIUM inside-promise findings of code review; the size of `ticket.md`; tokens by
role. The product claim is judged against a plain agent on the same ticket of a foreign repository with
CI; the pilot's gate in its first form is in `docs/HISTORY.md` with its verdict (2026-11-19).

## 5. The order of work

Each item is one ticket of at most 12 Plan steps, taken in a new session in this order. Item 4.1 was
started on 2026-11-19 before the order changed and finishes; after it the four tickets of phase 3b in
their order, then phase 3c, then phase 5, then phase 4. The
foreign repository for the next comparison (a fork of an open-source TypeScript project with jest, open
issues, GitHub Actions and an MIT or Apache licence) is chosen by a ledger decision right after ticket 3b-1, from
three candidates. The empty crates `stapel-agent` and `stapel-forge` are removed inside the next ticket that
touches the workspace; `stapel-lsp` is filled by phase 3c, `stapel-index` and `stapel-mcp` by phase 5.

Why this order: the pilot showed that stapel's value lies in the checks at the boundary (the stand, RED
before GREEN, the project's CI) and its cost in the mechanics of the process. Phase 3b removes the cost,
phase 3c gives the person a place to read and answer, phase 5 adds the checks and the index, and the feature level of phase 4 waits until a real feature shows
the need.

### Phase 3 — one ticket made lighter (done)

- 3.1 the human page as a file (STP-32): `stapel page` writes the Frame, the criteria, the Decisions and
  one line per other section to `page.md`; the `stapel ok` dialog shows a summary and refuses when the page
  changed.
- 3.2 `stapel check` in the CI of this repository (STP-33): the workflow `stapel init` writes installs the
  binary at its build commit and runs the same check on the merge request.

### Phase 3b — the mechanics after the pilot

Four tickets (ledger 2026-11-20: the ten items of the first cut were grouped, because the fixed part of a
ticket, the Frame, the spec review and the close, costs about half an hour of the person's attention and
about 150 k tokens whatever its size). The item numbers stay as names inside each ticket, so that the
decisions that name them keep their meaning. Each ticket stays under 12 Plan steps; if a Frame finds more,
the ticket is split there, not here.

**Ticket 3b-1, the check** (items 3.2a and 3.2b; both change `stapel check` and its tests).

- 3.2a faster fixtures of `tests/check.rs`: a shared `CARGO_TARGET_DIR` for the fixture builds and a fake
  cargo for the tests whose subject is the logic of `stapel check`; real cargo stays in a few tests of the
  cargo boundary; `check.timeout_secs` must stop a run. Measured: the time of `tests/check.rs` and of the
  local check, before and after.
- 3.2b check levels, points and the step cache. A step whose RED commit, GREEN commit, test list, lock file
  hash and stapel version match a passing record of `runs.jsonl` is not run again. Levels in `[check]
  level` and as a flag: `steps` (the new steps at RED and GREEN and their tests at HEAD, no suite), `head`
  (every step's tests at HEAD and the suite), `full` (everything, cache ignored). The points at which
  `stapel next` demands a check are `[check] points`, by default step, review and close; `step` can be
  dropped, `close` cannot, a RED run is never skipped, and `full` runs in CI, which the close needs. A check
  stays valid after commits that touch only `.stapel/tickets/`. `[check] suite_exclude` thins the local
  suite only. Measured on this repository and on a jest project.

**Ticket 3b-2, the dialogs and the returns to the person** (items 3.3, 3.4, 3.8 and the `stapel.toml`
part of 3.7; the theme is how often the process turns to the person).

- 3.3 the grant lifetime: 24 hours instead of 30 minutes, so that an answer after a pause does not cost a
  second dialog.
- 3.4 fix steps without a dialog per round: the fix and coverage steps a review round adds are built on a
  provisional permit and confirmed together by the `stapel ok` before the close; a step that widens the
  promise still stops for its own dialog. The same ticket adds the setting `[process] dialogs` with the
  level `close` for light tickets (no dialog but the close), measured after it as the next autonomy level.
  Expected: three dialogs per ticket.
- 3.7a an agent edit of `.stapel/stapel.toml` goes through a dialog like `stapel ok` instead of being
  denied.
- 3.8 the worktree and the session start: `stapel new` runs the project's `[worktree] setup` command (for
  the pilot: link `node_modules`, copy the git hooks), and the SessionStart hook names the open tickets with
  their worktrees instead of `new-ticket`, so no typed line is needed after a resume.

**Ticket 3b-3, the close and the history** (items 3.5, 3.6 and the rest of 3.7; the theme is what must be
true at the close).

- 3.5 the close needs the project's own checks: `stapel check` runs the project's command under the Node
  version the project pins, and the close runs the project's pre-commit and CI checks on the whole diff
  (prettier, eslint, tsc in the pilot), so nothing is left to the CI after the merge.
- 3.6 softer history rules: a GREEN commit may change a step's test file and a build input may change after
  a RED when the commit message says `history: <reason>`; `stapel check` judges the result, not the shape of
  the history.
- 3.7b `stapel review start` accepts any round number; `stapel close` writes the rule 9 ledger records
  `catches` and `finding` from the ticket's findings.

**Ticket 3b-4, the records and the measures** (items 3.9 and 3.10; the theme is that the records write the
documents, not the orchestrator).

- 3.9 records write the ticket: the Review section (rounds, reviewers, findings, check runs, stand runs)
  is generated by `stapel review` and `stapel check` from `findings.jsonl` and `runs.jsonl`, not written by
  the orchestrator; `stapel export` also writes the text of the merge request (the Summary and the deferred
  findings) next to the branch.
- 3.10 the measures: `stapel tokens measure` prints the minutes of interaction, the capped waits and the
  returns apart; `stapel ledger rules` counts, per rule and per ticket, what each rule caught, so that rule
  9 (a rule that caught nothing in three tickets is removed) can be applied.

**Ticket 3b-5, the process after the pilot of phase 5** (ledger 2026-11-24; taken after 5.1 and before
5.2).

- 3b-5a the cargo lock of `stapel cargo` as N machine-wide slots (by default 3, configurable), at most one
  cargo run per worktree.
- 3b-5b the external reviewer on `claude-opus-5-5` in the `stapel.toml` template, with its own agent type
  and the token role `external`.
- 3b-5c `stapel review start` takes the pack base after the last merge of main.
- 3b-5d `stapel check` names the failing suite tests.
- 3b-5e the builder runs the fixtures privacy test with the ticket lint before each GREEN.

### Phase 3c — the editor (ledger 2026-11-20)

The person of the pilot asked for it, which is what `docs/decisions/0004` waited for. The measure it works
on is the person's attention: the ticket text is not read at 45 to 53 kB, and the page of 3.1 is a half
step towards a place where the person reads and answers. An LSP server, not an extension of one editor, so
that Zed, VS Code and every editor that speaks the protocol get the same.

- 3c.1 `stapel-lsp`, the buttons: a language server for `ticket.md` and `page.md` with a code action above
  each section that calls one existing command (`stapel ok`, `stapel page`, `stapel status`, `stapel next`),
  run as an editor task outside the agent's shell, so the grant and the dialog rules hold as they do today;
  the sections that wait for a confirmation and the lines changed since the last one are highlighted.
- 3c.2 the inbox: the decisions the person owes (questions, findings with the fate `ask-human`, waiting
  confirmations) listed in one view with a button per answer, written back as the same records the chat
  would write; measured by the returns and the interaction minutes of a ticket run from the editor against
  the chat.

### Phase 5 — the sum of changes, and the index

- 5.1 the guards folder: machine checks out of the agent's reach with ratchet baselines ("no worse than
  now"); `stapel check` and the CI run them. The folder adopts the project's existing guards as they are
  (the pilot repository has a `tests/guards/` folder with a coverage floor, a bundle ceiling and forbidden
  colours) and does not impose a format of its own.
- 5.1b the guards folder and the external reviewer after their pilots (ledger 2026-11-24, widened the
  same day; taken after 3b-5 and before 5.2):
  - 5.1b-1 a `[guards] dir` that overlaps the project's source or test folders (`src`, `tests`, `docs`)
    is refused or warned about at load, because every builder write under it is denied and every edit of
    the main agent asks (STP-41 SR1-7, SR2-10);
  - 5.1b-2 a guard's command that reads scripts or tool configs outside the guards folder (`.eslintrc`,
    `clippy.toml`, `.rgignore`) is warned about, and the skill and the starter say that they belong in the
    folder (STP-41 XR1-5);
  - 5.1b-3 the agent `stapel-external-reviewer` writes the role `external` in its findings, through a
    body of its own or a role that `stapel init` fills in (STP-42 XR1-3);
  - 5.1b-4 the `[models] external` binding of the starter `stapel.toml` says `provider = "claude-code"`,
    and `stapel init` checks that every agent role is bound to `claude-code` (STP-42 CR1-4).
- 5.2 the structural index and `stapel q`: a tree-sitter parse of every tracked file gives symbols, the
  import graph and the test files of a module; it lives in `.stapel/index/`, outside git. Queries: `symbol`,
  `refs`, `tests`, `impact` (the blast radius of a set of files and their tests). The index trusts the
  working tree, not git events: every query compares the content hash of each tracked file with the hash it
  indexed and re-parses the changed files before it answers, so a change made outside a ticket is seen by
  the next query; `stapel index` rebuilds everything, and a stapel version change does that on its own.
  Languages are adapters as data (a grammar and its queries), TypeScript and Rust first, so that Go and
  Python are cheap to add.
- 5.2c the observability of `stapel check` (ledger 2026-11-25; taken next, before the rest of 5.2 and
  before 5.2b): the output and `runs.jsonl` give the time of each stage (the copy, the RED and GREEN runs of
  each step, the suite build, the suite run); the wait for a cargo slot is shown apart; a line every five
  minutes names the stage and the running test file; when the time limit runs out the check names the
  stage, the running test file and the last 20 lines of cargo output; the full output of a run is kept in
  a file outside git whose path the check prints. Later in the same item: the five slowest test files of
  the suite, and the suite time against the last pass.
- 5.2b the guards folder checks after STP-43 (ledger 2026-11-25; taken after 5.2 and before 5.3):
  - 5.2b-1 the work folders that `[guards] dir` must not name include `scripts`, `tools`, `pkg`, `cmd`,
    `internal` and `include`, because a guards folder named so loads and then denies builder writes
    (STP-43 SR1-9);
  - 5.2b-2 a newline in a multi-line guard command separates commands like `;`, so the word after it is a
    command start for the reach warning (STP-43 XR2-8).
- 5.3 the blast radius in the process: the investigation before the Frame runs `impact` and the Frame lists
  the places the ticket changes; the brief carries the places of its step; the review and drift packs carry
  the blast radius of the diff; the Review Focus names the tests of the radius the plan does not run; the
  agent queries the index as a tool through `stapel-mcp`. Measured: the same ticket with and without the
  index, tokens and turns to the first correct change.
- 5.4 a new dependency only through a dialog, and a public API snapshot computed from the index and kept in
  the guards folder; a change of it is a ratchet failure until the person confirms it.
- 5.5 precise references through a SCIP indexer where one exists, run at commit boundaries, with a line
  that says which commit it is current for; the structural index answers for the files changed since,
  marked imprecise.

### Phase 4 — the feature level

Optional: a ticket without a feature works as before. Two levels only, feature and ticket.

- 4.1 the elaboration skill (STP-34, in progress): the questions, the template and the checklists for
  elaborating a feature, ported from the method studied in decisions 0001 to 0003 under its MIT licence,
  into the skill; the method's name appears only in `NOTICE.md`, which says the skill is adapted from it.
- Before 4.2: one real feature of several tickets is run with the questions of 4.1 and a plain breakdown
  file; items 4.2 to 4.4 are taken only if that run shows the orchestrator cannot keep the breakdown and
  the requirements straight in plain text (a ledger decision with the measure).
- 4.2 the kind `feature`: a ticket with requirements with ids, a Decisions log and a breakdown into tickets
  with dependencies, confirmed by the same dialog and page; child tickets name the requirements they cover.
- 4.3 staleness from requirements to tickets: a changed requirement makes an unbuilt ticket wait for a new
  confirmation, stops a ticket in build, and marks a closed ticket built against the old text until the
  human decides.
- 4.4 computed readiness: a feature is done when its tickets are closed and each requirement is proved by
  a named test; `stapel next` on a feature names the next ticket and what blocks it.
- Candidates on the ticket-kind mechanism of 4.2: the kind `bug` (a three-line form: the reproduction, the
  cause hypothesis from the index, the source ticket; one RED, one GREEN, one dialog) and the kind
  `refactor` (the promise "behaviour unchanged"; the proof: characterisation tests on the blast radius, the
  API snapshot of 5.4 and the full list of call sites from the index).

## 6. Out of scope

Jira sync (a one-way import of a ticket's text was declined on 2026-11-20), semantic search on embeddings,
multi-user locks, a hosted service, a web UI, agents other than Claude Code, deploy and operations after the
merge, a planning method of stapel's own beyond the feature level, more than two levels of work items, an
extension of one editor (the editor is served through LSP, phase 3c), `stapel ship` (the person pushes and opens the merge request).
