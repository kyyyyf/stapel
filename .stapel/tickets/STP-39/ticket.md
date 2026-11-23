# STP-39 — The language server and its buttons

## Description

Source: `docs/PLAN.md`, phase 3c, item 3c.1; ledger 2026-11-20 (the editor returns to the plan: an LSP
server, not an extension of one editor).

### Frame

**Problem.** The person confirms a ticket only through the chat and its permission dialog. In the pilot
the ticket text (45 to 53 kB) was not read; the page of 3.1 lies in the editor with no way to act on it
there, and the editor does not show which sections wait or which lines changed. `stapel-lsp` is empty.

**Plan item:** 3c.1.

**Risk tags:** `interface` (`stapel lsp`, the command names and arguments, the message texts, the Zed
extension), `guard` (a button runs `stapel ok`, which only a person may run).

**Non-goals.** The inbox (3c.2); extensions for other editors; publishing the Zed extension; hovers,
completion, semantic tokens, formatting; edits from a button; `stapel close` and record commands as
buttons; several repositories in one workspace; LSP over a socket; Windows paths.

**Promise (closed list).**

1. Server: `stapel lsp` speaks LSP over standard input and output (lifecycle, document sync, code
   actions, execute command, diagnostics) and acts only on `ticket.md` and `page.md` of
   `.stapel/tickets/<KEY>/`; another document gets no action and no diagnostic.
2. Buttons: a code action anywhere in an open `ticket.md` or `page.md` offers `stapel status KEY`,
   `stapel next` and `stapel page KEY`, and in `page.md` `stapel ok KEY` under item 4.
3. Running: a button runs the same `stapel` binary in the root of the document's path with the editor's
   environment; the first output line and the exit status go to a message, the whole output to the log.
   A command or argument outside items 2 and 4 is refused first; in an agent's shell nothing runs.
4. `ok` on a current page: offered only when `page.md` on disk is the page `stapel page KEY` would write
   now and the buffer equals the disk; it carries the page's hash, and its run is refused unless
   `page.md` still has that hash and is still current.
5. Highlighting: each waiting section's heading gets an Information diagnostic; each line added or
   changed since the section's last confirmation gets a Hint; computed from the buffer and `state.json`
   on open, change, save and after a run.
6. Zed: `editors/zed/` holds an extension that only starts `stapel lsp` from `PATH` for Markdown; a hand
   stand checks it.
7. Workspace: the empty crates `stapel-agent` and `stapel-forge` are removed (`docs/PLAN.md` §5).

Not promised: clients without code actions, non-UTF-8 buffers, encodings other than UTF-16; deleted lines;
a section renamed in an unsaved buffer; a run that does not end; disk changes made outside the editor
before the next event of the document; symlinks; another case on a case-insensitive file system; a
`ticket.md` change between the server's page check and the child's own read; a bound on the size of a
run's output.

**Prior art.** STP-2, STP-17 (`stapel ok`, the agent shell refusal), STP-32 (`page.md`, `page_hash`),
`stage.rs`; the crates `lsp-server`, `lsp-types` (synchronous) and `tower-lsp` (needs tokio);
`zed_extension_api`.

**Size.** About 600 source lines (`stapel-lsp`, the subcommand in `stapel-cli`, the Zed extension) and
about 700 test lines; 11 Plan steps. Route: full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-23: Decision (how a button runs a command, the human, recommended): the server runs `stapel`
  as a child process with the editor's environment and refuses in an agent's shell; the output goes to the
  editor's message and log. Rejected: a Zed task from `tasks.json` (Zed only).
- 2026-11-23: Decision (the `ok` button, the human, recommended): only in `page.md`, and only when the page
  is current and saved; one click runs `stapel ok KEY`. Rejected: `ok` at every waiting section with no
  page check.
- 2026-11-23: Decision (Zed, the human, recommended): a small extension in `editors/zed/` that only
  starts `stapel lsp`; its wasm build is outside `stapel check`; a hand-checked stand. Rejected: no
  extension, configuration examples for generic LSP clients only.
- 2026-11-23: Decision (highlighting, the human, recommended): diagnostics, Information on a waiting
  heading and Hint on a changed line. Rejected: headings only.
- 2026-11-23: Decision (agent): the server is the subcommand `stapel lsp` of the one binary (principle 9),
  built on `lsp-server` and `lsp-types` (synchronous, no async runtime).
- 2026-11-23: Assumption: STP-38 is open in parallel; `stapel depends` runs before the build (DP-1: no
  dependency found).
- 2026-11-23: Decision (agent, build): a `state.json` without `schema_version` counts as no state and is
  not logged, because the fixtures of other tests write `{}`; AC-13 says so.
- 2026-11-23: Decision (agent, review round 1, XR1-1): a code action request with a non-empty `only`
  filter gets no button, because a client that applies code actions on format or save would otherwise
  run `stapel ok` without a click. Rejected: giving the buttons a kind, which a client may still apply.
- 2026-11-23: Decision (agent, review round 1, XR1-3): the `ok` button follows the branch rule of
  `stapel ok` itself; a checkout whose branch names no ticket offers it. AC-8 and the Terms are narrowed.
- 2026-11-23: Decision (agent): the Plan goes past the 11 steps of the Size with the fix and coverage
  steps 12 to 16 of code review round 1; they close findings of this ticket's promise.

## Spec

Terms:

- **Ticket document**: a `file:` URI whose path is `<root>/.stapel/tickets/<KEY>/ticket.md` or
  `<root>/.stapel/tickets/<KEY>/page.md`, where `<root>/.stapel/stapel.toml` and
  `<root>/.stapel/tickets/<KEY>/state.json` exist, `stapel.toml` loads, and `tickets::key_number` accepts
  the folder name `<KEY>` for the template `tickets.key`. `<root>` is the **root**, `<KEY>` the **key**.
  The path is the URI decoded by `lsp-types` (percent-encoding removed).
- **Button**: a code action whose command is one of `stapel.status`, `stapel.next`, `stapel.page`,
  `stapel.ok`; its arguments are the document's URI, and for `stapel.ok` also the **offered hash**, the
  `page::page_hash` of `page.md` when the button was offered.
- **Lines** of a text: split as `ticket::scan` splits (`\r\n`, `\r` and `\n` end a line), positions in
  UTF-16 code units. A diagnostic **on a line** covers it from character 0 to its end.
- **Button**s are code actions without a kind; a request with a non-empty `only` filter gets none.
- **Agent shell**: the server's environment has `CLAUDECODE` or `CLAUDE_CODE_ENTRYPOINT` set.
- **Current page**: `page.md` on disk is byte for byte the text `stapel page KEY` would write now
  (`page::page_text` of `confirm::prepare_waiting`, which refuses a key that is another ticket than the
  one the root's branch names; a branch that names no ticket passes, as for `stapel ok`), there is at least one waiting section, and the open buffer of `page.md` equals the file
  on disk.
- **Output** of a run: its standard output followed by its standard error, read lossy as UTF-8; its
  **first line** is the first line of the output that is not whitespace only.
- **Waiting section**: a section of `stapel.toml` whose owner is not `generated` and that
  `stage::is_fresh` judges not fresh for the buffer text and the ticket's `state.json`.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel lsp` gets `initialize` THE server answers with full text sync with open, close and save, a code action provider and an execute command provider with the commands `stapel.status`, `stapel.next`, `stapel.page` and `stapel.ok`; after `shutdown` and `exit` it exits 0; IF `exit` comes without `shutdown`, or standard input ends THEN it exits 1; a request it does not handle gets `MethodNotFound`, a request after `shutdown` gets `InvalidRequest`. | `lsp::initialize_and_shutdown`, `lsp::exit_without_shutdown`, `lsp::capabilities_name_open_close_and_save` |
| AC-2 | WHEN a code action request names an open ticket document THE server returns the buttons titled `stapel status <KEY>`, `stapel next` and `stapel page <KEY>`, in this order, at any range of the document; IF the request carries a non-empty `only` filter THEN it returns no button, so a client that applies code actions on format or save never runs one. | `lsp::buttons_on_ticket_documents`, `lsp::no_buttons_under_a_kind_filter` |
| AC-3 | IF a document is not an open ticket document (another name, another folder, no `stapel.toml`, no `state.json`, a folder name that is not a key such as `-x`, a scheme other than `file`, a URI not opened) THEN the server returns no code action and publishes no diagnostic for it. | `lsp::nothing_for_other_documents` |
| AC-4 | WHEN `workspace/executeCommand` names a button with an open ticket document THE server runs its own executable with `status <KEY>`, `next`, `page <KEY>` or `ok <KEY>` in the root, waits for it, sends `window/logMessage` with the output, sends `window/showMessage` of type Info with `stapel <args>: <first line>` on exit 0 (`stapel <args>: done` when there is none), or of type Error with `stapel <args>: exit <n>: <first line>` (`stapel <args>: exit <n>` when there is none), `stapel <args>: killed by signal <n>` or `stapel <args>: could not run: <reason>`, and answers `null`. | `lsp::runs_a_button`, `lsp::a_failing_button` |
| AC-5 | IF the command is not a button name, or its arguments are not the URI of an open ticket document (for `stapel.ok` the URI of an open `page.md` plus one string, none more for the others) THEN the server answers the error `InvalidParams` with `stapel lsp: unknown command <name>` or `stapel lsp: the arguments must be the URI of an open ticket.md or page.md`, and runs nothing; this check comes before every other. | `lsp::refuses_unknown_commands`, `lsp::refusals_beyond_the_first_cases` |
| AC-6 | IF the server runs in an agent shell THEN `workspace/executeCommand` of a valid button runs nothing, answers `null` and shows an Error message `stapel lsp: this editor was started from an agent's shell (CLAUDECODE or CLAUDE_CODE_ENTRYPOINT is set); start it from your own shell to use the buttons`. | `lsp::refuses_in_an_agent_shell`, `lsp::refusals_beyond_the_first_cases` |
| AC-7 | WHEN the document of a code action request is `page.md` and it is a current page THE buttons also hold `stapel ok <KEY>`, last, with the offered hash; the run confirms every waiting section with `via: terminal`, as `stapel ok KEY` from a terminal does. | `lsp::ok_on_a_current_page` |
| AC-8 | IF `page.md` is not a current page (edited, older than the ticket text, unsaved buffer, no waiting section, a key that is another ticket than the branch's) or the document is `ticket.md` THEN no `ok` button is offered. | `lsp::no_ok_on_a_stale_page` |
| AC-9 | IF `stapel.ok` arrives when `page.md` is not a current page, or its hash on disk is not the offered hash (a page regenerated after the offer) THEN the server runs nothing, answers `null` and shows an Error message `stapel lsp: page.md is not current; run stapel page <KEY> and read it again`. | `lsp::ok_refused_after_the_page_changed`, `lsp::ok_refused_after_a_reloaded_page` |
| AC-10 | WHEN a `ticket.md` is opened, changed or saved THE server publishes for it one Information diagnostic `waiting for confirmation (owner: <owner>)`, source `stapel`, on the heading line of each waiting section (on each such line when the heading is duplicated, none when it is missing); WHEN it is closed, or its `state.json` disappears, turns into a state without `schema_version`, or its folder stops being a key, THE server publishes an empty list for it once and forgets its logged errors; a `page.md` gets no diagnostic. | `lsp::waiting_headings`, `lsp::save_refreshes_diagnostics`, `lsp::diagnostics_cleared_when_the_state_goes`, `lsp::folder_not_a_key_and_missing_config` |
| AC-11 | WHEN a section of `ticket.md` is waiting because its text hash changed and its latest confirmation has its text THE server publishes a Hint diagnostic `changed since the last confirmation`, source `stapel`, on each non-blank line of its body in the buffer that the line diff of `similar` marks inserted against the lines of the confirmed text, both sides compared line by line after NFC and trailing whitespace removal; a confirmation without text, a fresh section, or a body or text of more than 5000 lines, blank lines at its edges not counted, gives none (the last with one `window/logMessage` per open document and section, again only after the section was under the limit); the confirmed text is split into lines as `ticket::scan` splits, with no empty element after a final line end. | `lsp::changed_lines`, `lsp::crlf_confirmed_text`, `lsp::long_section_warns_once` |
| AC-12 | WHEN a button run ends THE server publishes the diagnostics of every open `ticket.md` again, read from disk state after the run. | `lsp::diagnostics_after_ok` |
| AC-13 | IF `stapel.toml` or `state.json` of an open `ticket.md` in a ticket folder cannot be read or parsed (a `stapel.toml` that is missing or does not load included; a `state.json` without `schema_version` counts as no state and is not logged) THEN the server publishes no diagnostic for it, sends one `window/logMessage` of type Error naming the file and the cause until the file reads again, and serves the next message. | `lsp::bad_state_is_logged`, `lsp::folder_not_a_key_and_missing_config` |
| AC-14 | WHEN the files of `editors/zed/` are read THE `extension.toml` declares the language server `stapel` for the language `Markdown`, `src/lib.rs` starts `worktree.which("stapel")` with the one argument `lsp` and returns `stapel not found on PATH` when there is none, and `cargo metadata --no-deps --manifest-path editors/zed/Cargo.toml` loads; the behaviour in Zed is the stand of Proof. | `lsp::the_zed_extension_starts_stapel_lsp` |
| AC-15 | WHEN `cargo metadata` lists the workspace THE members are `stapel-core`, `stapel-index`, `stapel-mcp`, `stapel-lsp` and `stapel-cli`; `stapel-agent` and `stapel-forge` are gone, and the stub check covers `stapel-index` and `stapel-mcp` only. | `cli::no_empty_crates`, `cli::workspace_has_seven_members`, `cli::stub_packages_are_empty` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-1 and AC-3, item 2 by AC-2, item 3 by AC-4 to
  AC-6, item 4 by AC-7 to AC-9, item 5 by AC-10 to AC-13, item 6 by AC-14 and the stand of the Review
  section, item 7 by AC-15. Unchanged: every other command of `stapel`, the guard and the hooks.
- **Not promised:** the Frame's list; a button run while another one runs (the server serves one message
  at a time); the order of messages between `showMessage` and `logMessage` in the client.

### Abuse table

| Attempt of a careless agent | Outcome | Test |
|---|---|---|
| starts `stapel lsp` from its shell and sends `workspace/executeCommand` `stapel.ok` on its standard input | agent shell: nothing runs, Error message | `lsp::refuses_in_an_agent_shell` |
| starts Zed from its shell, so the server inherits `CLAUDECODE` | the same refusal for every button | `lsp::refuses_in_an_agent_shell` |
| sends `stapel.close`, `stapel.ledger` or another name as a command, in an agent shell or not | `InvalidParams`, nothing runs | `lsp::refuses_unknown_commands`, `lsp::refuses_in_an_agent_shell` |
| passes a key, a path or extra words instead of the document URI, to run `ok` on another ticket or with other arguments | `InvalidParams`, nothing runs; the key comes only from the URI's folder | `lsp::refuses_unknown_commands` |
| makes a folder named `--version` with a `state.json` under `.stapel/tickets/` | not a ticket document: no button | `lsp::nothing_for_other_documents` |
| edits `page.md` so that it lies about the change, then asks the person to press `ok` | the page differs from the computed one: no `ok` button, a run is refused | `lsp::no_ok_on_a_stale_page`, `lsp::ok_refused_after_the_page_changed` |
| edits `ticket.md` after the person opened a current page | the computed page changes: the run is refused | `lsp::ok_refused_after_the_page_changed` |
| edits `ticket.md` and runs `stapel page` after the button was offered, so the new page is current again | the hash differs from the offered hash: the run is refused | `lsp::ok_refused_after_the_page_changed`, `lsp::ok_refused_after_a_reloaded_page` |
| removes `CLAUDECODE` with `env -u` before it starts the server | deliberate circumvention, not promised (no `security` tag); as for `stapel ok` today | — |

**Author self-check (item 7).** Done on 2026-11-23: each row of the Abuse table maps to promise item 3 or
4 or to the Not promised list; the server adds no way to confirm that a terminal does not already have.

## Test plan

Integration tests in `crates/stapel-cli/tests/`: the new file `lsp.rs` and the existing `cli.rs`. `lsp.rs`
starts the built `stapel lsp` with the environment of `common::stapel` (agent variables removed, set again
only by `refuses_in_an_agent_shell`), writes framed JSON-RPC messages to its standard input and reads
framed messages until the expected answer, with a time limit of 30 seconds per test. Fixture repositories
come from `stapel init` and `stapel new` in a temporary folder, as in `ok_page.rs`. Light tests: `stapel
cargo test -j 4 -p stapel-cli --test lsp -- --test-threads=4`; before each GREEN also `--test
ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Lifecycle | `initialize` capabilities and the four command names; `shutdown` then `exit`: exit 0; `exit` alone: exit 1; standard input closed after `initialize`: exit 1; a `textDocument/hover` request: `MethodNotFound`; a request after `shutdown`: `InvalidRequest` | `lsp::initialize_and_shutdown`, `lsp::exit_without_shutdown` |
| Buttons | `ticket.md` at line 0 and at the last line; `page.md`; titles and order; the argument is the URI | `lsp::buttons_on_ticket_documents` |
| Other documents | `README.md`; `ticket.md` outside `.stapel/tickets/`; a ticket folder without `state.json`; a root without `stapel.toml`; a folder `-x` with a `state.json`; a folder `stp-039` (a ticket document, key as named); a `stapel.toml` that does not load; an `untitled:` URI; a ticket document never opened; a root whose path has a space (`%20` in the URI) is a ticket document | `lsp::nothing_for_other_documents` |
| Running | `stapel.status` prints the `ticket:` line in the Info message and the whole output in the log; `stapel.next`; `stapel.page` writes `page.md`; a key whose `state.json` is malformed: `stapel.page` gives an Error message with `exit 1`; a `ticket.md` of a ticket that is not the branch's: `stapel.page` shows the child's refusal with `exit 2` | `lsp::runs_a_button`, `lsp::a_failing_button` |
| Refusals | `stapel.close`; `stapel.status` with no argument, two arguments, a number, a URI of `README.md`, the URI of another ticket's file plus a key, a ticket document closed before the run; `stapel.ok` without the hash; `stapel.ok` with the URI of an open `ticket.md` and the hash of a current `page.md` | `lsp::refuses_unknown_commands` |
| Agent shell | `CLAUDECODE=1`; `CLAUDE_CODE_ENTRYPOINT=cli`; each with `stapel.ok` and with `stapel.status`; `state.json` unchanged; `stapel.close` in an agent shell: `InvalidParams`, no message (`stapel.ok` on a really current page: `CLAUDECODE` only, in `lsp::refusals_beyond_the_first_cases`) | `lsp::refuses_in_an_agent_shell` |
| `ok` | a current page: the button is last with the hash of `page.md`; its run confirms every waiting section, `via` is `terminal` | `lsp::ok_on_a_current_page` |
| Stale page | `page.md` with one byte changed; `page.md` written before a later edit of the Spec; buffer changed but not saved; no `page.md`; no waiting section; a page of a ticket that is not the branch's; `ticket.md` | `lsp::no_ok_on_a_stale_page` |
| Stale run | the button offered, then the Spec changed, then the run; the button offered, then the Spec changed and `stapel page` run again, then the run; a run with a made-up hash on a current page; each refused, `state.json` unchanged | `lsp::ok_refused_after_the_page_changed` |
| Waiting | a new ticket: one diagnostic per configured section on its heading line; a confirmed Description: no diagnostic for it; a buffer change that edits the confirmed Description: its diagnostic comes back; the same buffer with CRLF line ends: the same lines; the range covers the whole heading line; a duplicated `## Spec` heading: one diagnostic on each; the `## Design` heading removed: none for Design; an open `page.md`: no diagnostic; close: empty list | `lsp::waiting_headings` |
| Changed lines | a confirmed section with one line changed, one line added and one removed: Hints on the changed and the added line only; a section with the line `- a` twice and a third `- a` added: one Hint, on the line `similar` marks; a fresh section with CRLF line ends: none; a changed section whose other lines gained trailing spaces and two leading blank lines: Hints only on the really changed line; a section confirmed without stored text: none; a body of 5001 lines: none and one log line | `lsp::changed_lines` |
| After a run | an open `ticket.md` with waiting sections, then `stapel.ok` from its `page.md`: the waiting diagnostics are gone | `lsp::diagnostics_after_ok` |
| Bad state | a malformed `state.json` logged once over two changes; a `stapel.toml` that does not load; after each, a code action request on a good document is served | `lsp::bad_state_is_logged` |
| Zed | `editors/zed/extension.toml` names the server `stapel` for `Markdown`; `src/lib.rs` uses `worktree.which("stapel")`, the argument `lsp` and the error text; `cargo metadata --no-deps` loads its `Cargo.toml` | `lsp::the_zed_extension_starts_stapel_lsp` |
| Workspace | `cargo metadata` members; the folders of the crates `stapel-agent` and `stapel-forge` absent; the stub check over `index` and `mcp` | `cli::no_empty_crates`, `cli::workspace_has_seven_members`, `cli::stub_packages_are_empty` |
| Review round 1 fixes | a code action request with `only: ["source.fixAll"]` on a current `page.md` and on `ticket.md`: no button; an open `ticket.md` with waiting diagnostics whose `state.json` is deleted, then rewritten as `{}`, then restored as a bad file twice: an empty list once each time, and the bad file logged again; `didClose` after `state.json` is gone: an empty list; a confirmed text with CRLF line ends and one changed line: one Hint on that line; a confirmed text of exactly 5000 lines with a final line end: Hints still computed | `lsp::no_buttons_under_a_kind_filter`, `lsp::diagnostics_cleared_when_the_state_goes`, `lsp::crlf_confirmed_text` |
| Review round 1 coverage | `didSave` after `stapel ok` from a terminal: the waiting diagnostics are gone; `stapel lsp extra`: exit 2; `stapel.ok` in an agent shell on a current page; `stapel.ok` with a number as the second argument and with three arguments; a bad argument in an agent shell: `InvalidParams`; the page regenerated after the offer and sent as `didChange`: refused by the hash | `lsp::save_refreshes_diagnostics`, `lsp::refusals_beyond_the_first_cases`, `lsp::ok_refused_after_a_reloaded_page` |
| Review round 2 | a section of 5001 lines changed twice: one Warning; shortened under the limit and lengthened again: a second Warning; the `initialize` answer has `openClose: true` and `save`; an open `ticket.md` whose key template in `stapel.toml` changes so its folder is no key: an empty list once; a root without `stapel.toml`: one Error log line naming it | `lsp::long_section_warns_once`, `lsp::capabilities_name_open_close_and_save`, `lsp::folder_not_a_key_and_missing_config` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `stapel lsp` arguments | none | none / none | the only form | any argument: clap refuses, exit 2 | — | `lsp::initialize_and_shutdown`, `lsp::refusals_beyond_the_first_cases` |
| LSP message | `Content-Length` framed JSON-RPC | one header / the client's size | end of input: exit 1 | bad framing or JSON: the `lsp-server` error, exit 1 (not tested) | — | `lsp::exit_without_shutdown` |
| Document URI | URI string | `file:///` / any | — | not `file:`, not a ticket document: no actions, no diagnostics | the path as decoded by `lsp-types` | `lsp::nothing_for_other_documents` |
| Command name | string | — / — | empty: unknown command | not a button name: `InvalidParams` | exact | `lsp::refuses_unknown_commands` |
| Command arguments | JSON array | one / two (`stapel.ok`) | none: `InvalidParams` | more, not a string, not an open ticket document, `stapel.ok` without the hash: `InvalidParams` | exact | `lsp::refuses_unknown_commands` |
| Buffer text | UTF-8 string | empty / the client's size; 5000 lines per section for Hints | empty: every section waits, no heading to mark (none published) | — | lines as in Terms | `lsp::waiting_headings`, `lsp::changed_lines` |
| Key folder name | string | the template with `1` / any number | — | a name `tickets::key_number` refuses: not a ticket document | as `key_number`: case and leading zeros ignored | `lsp::nothing_for_other_documents` |
| `page.md` on disk | file | absent / the page size | absent: no `ok` | any other byte: not current | byte for byte | `lsp::no_ok_on_a_stale_page` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| editor | capabilities: no code action support; incremental sync asked | the server offers full sync only; a client without code actions gets no buttons (not promised) | `lsp::initialize_and_shutdown` |
| editor | position encoding other than UTF-16 | not promised | — |
| editor | an unsaved buffer of `page.md` or `ticket.md` | diagnostics from the buffer; `ok` needs the buffer equal to the disk | `lsp::no_ok_on_a_stale_page`, `lsp::waiting_headings` |
| editor | `exit` without `shutdown`; standard input closed | exit 1 | `lsp::exit_without_shutdown` |
| environment | `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT` | refusal of every run | `lsp::refuses_in_an_agent_shell` |
| file system | `state.json` or `stapel.toml` malformed or unreadable | logged, no diagnostic, server goes on | `lsp::bad_state_is_logged` |
| file system | `page.md` absent | no `ok` | `lsp::no_ok_on_a_stale_page` |
| child process | exit 0; exit non-zero | Info or Error message | `lsp::runs_a_button`, `lsp::a_failing_button` |
| child process | output with non-UTF-8 bytes; no output (`done`, bare `exit N`) | bytes read lossy; the short forms of AC-4; not tested (Review Focus) | — |
| child process | killed by a signal; the executable cannot start | the `killed by signal` and `could not run` forms; not tested (Review Focus) | — |
| child process | a run that does not end | not promised; the server waits | — |
| git | the branch of the root, read in the server by `prepare_waiting` for the page check | a key that is not the branch's ticket: no `ok` button, a run refused | `lsp::no_ok_on_a_stale_page` |
| git | every other dimension | read by the child `stapel` command as today | — |
| os | percent-encoded paths in the URI | decoded, then classified | `lsp::nothing_for_other_documents` |
| os | symlinked folders or files; another case on a case-insensitive file system | not promised | — |
| Zed | `stapel` not on `PATH`; the extension's wasm build | the error text (AC-14); the build is checked on the stand, outside `stapel check`; its manifest loads | `lsp::the_zed_extension_starts_stapel_lsp` |
| claude-code | the hooks and the guard | unchanged; the server is not started through them | — |

**Review Focus:** a button run in a worktree whose branch is another ticket (`stapel next` takes the
branch's ticket; the review judges it); a `page.md` whose line endings the editor changed on save (not
current; tested by the one-byte case only); a heading inside a fenced block (`ticket::scan` rules; not
tested here); a child killed by a signal or an executable that cannot start (the two message forms of
AC-4; not tested, it needs a broken executable); a child command that prints more than the client shows
(the whole output goes to the log; not tested); a child output with non-UTF-8 bytes or with no output
(read lossy; the `done` and bare `exit N` forms of AC-4; not tested: every `stapel` button prints a line
and the server runs its own executable, so a test needs such a child).

## Design

### Facts

- F-1: `identity::in_agent_shell` reads `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT`; `ok.rs` refuses with
  it and records `via: terminal` for a run without a grant.
- F-2: `confirm::prepare_waiting(root, config, key)` gives the waiting sections in order; `page.rs` writes
  `page::page_text(config, &waiting)` to `page.md`; `prepare` refuses a key that is not the ticket of the
  root's branch (`tickets::match_branch`).
- F-3: `ticket::parse` splits at level-2 headings outside fences; `ticket::scan` gives each line with its
  heading; `stage::is_fresh` and `stage::latest` judge a section; `Confirmation.text` holds the body at
  the latest confirmation only (older ones are cleared by `ok.rs::add`).
- F-4: `crates/stapel-lsp` holds only a doc comment; the crates `stapel-agent` and `stapel-forge` held
  only a doc comment and are named in `cli::workspace_has_seven_members` (STP-1).
- F-5: the workspace members are `crates/*`; a crate under `editors/` is outside it.
- F-6: `cli::stub_packages_are_empty` asserts that `src/lib.rs` of agent, index, forge, mcp and lsp holds
  only comments.
- F-7: `repo::load` in `stapel-cli` reads `.stapel/stapel.toml` with `Config::parse_at`;
  `page.rs::marked_diff` uses `similar::TextDiff`.

### Shape

- `stapel-lsp` (library): `run() -> i32` on `lsp-server::Connection::stdio`, the document store (URI to
  buffer text), `classify(uri) -> Option<TicketDoc>` with the root, the key and the kind, the buttons, the
  diagnostics of a buffer, the changed lines (LCS over lines), the current page check, and the run of
  `std::env::current_exe()` with `Command::output()` in the root.
- `stapel-cli`: the subcommand `Lsp` in `main.rs` calls `stapel_lsp::run` and exits with its code;
  `stapel-lsp` depends on `stapel-core`, `lsp-server`, `lsp-types`, `serde_json` and `similar`; it reads
  `stapel.toml` with `Config::parse_at` as `repo::load` does, and `repo.rs` is unchanged.
- `editors/zed/`: `extension.toml`, `Cargo.toml` (its own `[workspace]`, crate type `cdylib`,
  `zed_extension_api`) and `src/lib.rs` with `language_server_command`; `README.md` says how to install it
  as a dev extension.

### Decisions (before the build)

- 2026-11-23, agent: the key and the root come only from the URI path, never from the arguments, so a
  button cannot name another ticket.
- 2026-11-23, agent: in an agent shell the buttons are still offered and their run is refused with a
  message, so that the person learns why nothing happens.
- 2026-11-23, agent: `cli::workspace_has_seven_members` keeps its name (STP-1 names it); its list becomes
  five members, and `cli::stub_packages_are_empty` covers index and mcp, in the GREEN of step 1 with a
  `history:` line; the new test `cli::no_empty_crates` is the RED. Step 2's GREEN drops lsp from the stub
  check with a `history:` line.
- 2026-11-23, agent (SR1-1): the `ok` button carries the offered hash as a second argument, so the run
  needs no state in the server and a page regenerated after the offer is refused.
- 2026-11-23, agent (SR1-4, SR2-3): the changed lines come from a `similar` line diff of their own; only
  the `+a -b` counts of the page use the same diff, so the page and the editor may mark other lines.

### As built

Notes of the build, dated 2026-11-23, agent.

- `stapel-lsp` uses `lsp-server` 0.7 and `lsp-types` 0.97. The text sync capability is an options object:
  open and close, full change, save.
- `locate(uri)` decides the path shape (root, key folder, kind); `classify(uri)` adds the `stapel.toml`,
  `state.json` and key checks. The diagnostics use `locate`, so that a `stapel.toml` that stops loading
  while the document is open is logged (AC-13).
- `button_args` checks the command and its arguments first (AC-5); then `agent_shell()` (AC-6); then, for
  `stapel.ok`, `offered_page_is_current` runs `current_page` again and compares the offered hash (AC-9).
  `run_button` runs `current_exe()` with standard input closed and reads the output lossy.
- `current_page` needs `prepare_waiting` to succeed, at least one waiting section, the file equal to
  `page_text` and the buffer equal to the file; its doc comment says the run confirms with
  `via: terminal`.
- A bad state is logged again only when its cause changes or the files read in between; a shown list
  is cleared on a bad state. A `state.json` without `schema_version` (the `{}` of older fixtures) is
  treated as no state: no diagnostic and no log line.
- `changed_lines` compares each line after `hash::normal_form`; above 5000 lines on either side, blank
  lines at the edges of each side not counted (agent decision of step 14), it logs one Warning. `similar` marks the first of several equal lines as inserted.
- After every run that ran, the diagnostics of every open document are published again before the
  answer.
- The Zed extension passes `worktree.shell_env()` as the environment; an editor started from an agent's
  shell therefore hands `CLAUDECODE` to the server, which refuses (AC-6). `editors/zed/target` and its
  lock file are in `.gitignore`.
- History lines: step 1's GREEN (the crate lists of `cli.rs`) and step 11's GREEN (the extension's own
  `Cargo.toml` is a build input). Step 2 needed none: step 1 had already taken lsp out of the stub check.
- Tests: the RED of step 10 changed the helper `waiting_lines` of step 9 to skip Hints, because an
  edited section now also gets a Hint.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `lsp::initialize_and_shutdown`, `lsp::exit_without_shutdown`, `lsp::capabilities_name_open_close_and_save` |
| AC-2 | `lsp::buttons_on_ticket_documents`, `lsp::no_buttons_under_a_kind_filter` |
| AC-3 | `lsp::nothing_for_other_documents` |
| AC-4 | `lsp::runs_a_button`, `lsp::a_failing_button` |
| AC-5 | `lsp::refuses_unknown_commands`, `lsp::refusals_beyond_the_first_cases` |
| AC-6 | `lsp::refuses_in_an_agent_shell`, `lsp::refusals_beyond_the_first_cases` |
| AC-7 | `lsp::ok_on_a_current_page` |
| AC-8 | `lsp::no_ok_on_a_stale_page` |
| AC-9 | `lsp::ok_refused_after_the_page_changed`, `lsp::ok_refused_after_a_reloaded_page` |
| AC-10 | `lsp::waiting_headings`, `lsp::save_refreshes_diagnostics`, `lsp::diagnostics_cleared_when_the_state_goes`, `lsp::folder_not_a_key_and_missing_config` |
| AC-11 | `lsp::changed_lines`, `lsp::crlf_confirmed_text`, `lsp::long_section_warns_once` |
| AC-12 | `lsp::diagnostics_after_ok` |
| AC-13 | `lsp::bad_state_is_logged`, `lsp::folder_not_a_key_and_missing_config` |
| AC-14 | `lsp::the_zed_extension_starts_stapel_lsp`, and the stand below |
| AC-15 | `cli::no_empty_crates`, `cli::workspace_has_seven_members`, `cli::stub_packages_are_empty` |

Stand (item 6): Zed with the dev extension of `editors/zed/` and `stapel` on `PATH`, opened on a ticket of
this repository; the Review section records the Zed version, the buttons seen, one `stapel status` run and
the waiting diagnostics.

## Plan

Route: full (tags `interface`, `guard`), auto.

No `ok` button exists before step 7; until then a valid `stapel.ok` run gets the refusal of AC-9, so
`stapel ok` never runs without the page check.

1. Empty crates (AC-15; test `cli::no_empty_crates`, which fails at the RED because the folders exist):
   the GREEN removes the crates `stapel-agent` and `stapel-forge`, updates `Cargo.lock`, the layout of
   `CLAUDE.md`, the list of `cli::workspace_has_seven_members` and of `cli::stub_packages_are_empty`
   (message line `history: the two empty crates are removed`). Must not change: the other crates.
2. Lifecycle (AC-1; tests `lsp::initialize_and_shutdown`, `lsp::exit_without_shutdown`): the RED adds
   `tests/lsp.rs` with its client helper; the GREEN adds the dependencies, `stapel_lsp::run`, the
   subcommand `lsp`; `cli::stub_packages_are_empty` already leaves lsp out after step 1. Must not change:
   the other subcommands.
3. Buttons (AC-2, AC-3; tests `lsp::buttons_on_ticket_documents`, `lsp::nothing_for_other_documents`):
   the GREEN keeps the open buffers, classifies the URI and answers code actions. Must not change: the
   lifecycle.
4. Running a button (AC-4; tests `lsp::runs_a_button`, `lsp::a_failing_button`): the GREEN runs the
   executable in the root and sends the log and the message. Must not change: the buttons.
5. Refused commands (AC-5; test `lsp::refuses_unknown_commands`, which fails at the RED because a bad
   command gets no `InvalidParams`): the GREEN checks the command name and the arguments before any other
   check. Must not change: a valid run.
6. Agent shell (AC-6; test `lsp::refuses_in_an_agent_shell`, which fails at the RED because the run goes
   ahead): the GREEN refuses every valid run in an agent shell. Must not change: a run outside it.
7. The `ok` button (AC-7, AC-8; tests `lsp::ok_on_a_current_page`, `lsp::no_ok_on_a_stale_page`): the
   GREEN computes the current page, offers the button with the offered hash only on it, and runs it. Must
   not change: the three other buttons.
8. Stale run (AC-9; test `lsp::ok_refused_after_the_page_changed`, which fails at the RED because the
   run of step 7 does not check the page again): the GREEN checks the current page and the offered hash
   at the run and refuses. Must not change: `ok` on a current page.
9. Waiting headings (AC-10, AC-13; tests `lsp::waiting_headings`, `lsp::bad_state_is_logged`, which fail
   at the RED because no diagnostic and no log line come): the GREEN publishes the Information
   diagnostics and logs a bad state once. Must not change: the buttons.
10. Changed lines and refresh (AC-11, AC-12; tests `lsp::changed_lines`, `lsp::diagnostics_after_ok`,
    which fail at the RED because no Hint comes and the diagnostics are not published again): the GREEN adds the Hints of the `similar` line diff and publishes again after a run. Must not change:
    the waiting headings.
11. Zed extension (AC-14; test `lsp::the_zed_extension_starts_stapel_lsp`): the GREEN adds
    `editors/zed/` with `extension.toml`, `Cargo.toml`, `src/lib.rs` and `README.md`. Must not change: the
    cargo workspace.
12. Kind filter (XR1-1, AC-2; test `lsp::no_buttons_under_a_kind_filter`, which fails at the RED because
    the buttons come back under `only`): the GREEN returns no button when `context.only` is non-empty.
    Must not change: a request without `only`.
13. Clearing diagnostics (XR1-2, CR1-1, DR1-4, AC-10; test `lsp::diagnostics_cleared_when_the_state_goes`,
    which fails at the RED because the old list stays): the GREEN publishes an empty list once when a
    shown `ticket.md` loses its state or key, forgets its logged errors, and empties it on `didClose`
    whatever the state. Must not change: the waiting headings of a readable ticket.
14. CRLF confirmed text (CR1-3, DR1-2, AC-11; test `lsp::crlf_confirmed_text`, which fails at the RED
    because the CRLF text gains empty lines): the GREEN splits the confirmed text as `ticket::scan` does
    and drops the empty element after a final line end. Must not change: LF texts.
15. Save and reload coverage (XR1-4, XR1-6, AC-9, AC-10; tests `lsp::save_refreshes_diagnostics`,
    `lsp::ok_refused_after_a_reloaded_page`): a coverage commit `STP-39 step 15 tests: ...`. If a test
    fails, it becomes a RED and the GREEN fixes the code.
16. Refusal coverage (XR1-5, AC-5, AC-6; test `lsp::refusals_beyond_the_first_cases`): a coverage
    commit `STP-39 step 16 tests: ...`. If the test fails, it becomes a RED and the GREEN fixes the code.
17. One long-section warning (CR2-1, XR2-5, DR2-2, AC-11; test `lsp::long_section_warns_once`, which
    fails at the RED because a second `didChange` of the same long section logs the Warning again): the
    GREEN logs the Warning of a section over 5000 lines once per open document and section, and again only
    after that section has been under the limit. Must not change: the Hints of shorter sections.
18. Capability coverage (XR2-2, AC-1; test `lsp::capabilities_name_open_close_and_save`): a coverage
    commit `STP-39 step 18 tests: ...` that checks `openClose` and `save` in the `initialize` answer. If
    the test fails, it becomes a RED and the GREEN fixes the code.
19. Branch coverage (XR2-6, AC-10, AC-13; test `lsp::folder_not_a_key_and_missing_config`): a coverage
    commit `STP-39 step 19 tests: ...` that checks the empty list once when the key template of
    `stapel.toml` changes so the folder is no key, and the Error log line for a root without
    `stapel.toml`. If the test fails, it becomes a RED and the GREEN fixes the code.

## Review

- Spec review: two rounds of `stapel-spec-reviewer` (SR1: 17 findings, two HIGH; SR2: 12 findings, no
  HIGH). Dependency review with STP-38: three rounds of `stapel-depends`, each `no dependency found`.
- Drift review before code review: `stapel-drift` on the steps that touch a criterion (DR-1 to DR-5).
- Code review round 1 at `ab290e2`: `stapel-reviewer` (role reviewer, CR1), an external
  `stapel-reviewer` (XR1, one HIGH: the `only` filter) and `stapel-drift` on the whole ticket (DR1); fix
  steps 12 to 16.
- main, holding the close of STP-38, was merged into the branch at `4df5d82` before round 2.
- Code review round 2 at `e107601`: the same three roles (CR2, XR2, DR2; no HIGH, two MEDIUM); fix steps
  17 to 19. No HIGH finding inside the promise is left, so no round 3 runs.
- Check runs (`stapel check STP-39`, a case-sensitive `TMPDIR` on macOS): head at `ab290e2` passed with
  1117 tests; at `e107601` after the merge with 1146; at `326869f` with 1149. Each builder commit was
  followed by a steps-level check.
- Stand (item 6): Zed with the dev extension of `editors/zed/` and the `stapel` binary of this branch on
  `PATH`; the person's report is recorded here at the close.

## Summary

The language server `stapel lsp` offers `stapel status`, `stapel next` and `stapel page` as code actions on
the documents of a ticket, and `stapel ok` only on a current `page.md` with its hash; every run is the
`stapel` binary itself, refused in an agent shell. Diagnostics show the waiting headings (Information) and
the changed lines (Hint). A small Zed extension in `editors/zed/` starts it. The empty crates
`stapel-agent` and `stapel-forge` are gone.

Agent decisions to overrule (all 2026-11-23):

- The key and the root come only from the URI path; a button cannot name another ticket.
- In an agent shell the buttons are offered and their run is refused with a message.
- The `ok` button carries the offered page hash; a page regenerated after the offer is refused.
- The changed lines come from their own `similar` line diff, so the page and the editor may mark other
  lines.
- A `state.json` without `schema_version` counts as no state and is not logged (AC-13).
- The 5000-line limit of AC-11 does not count blank lines at the edges of a side; its Warning comes once
  per document and section.
- The `done` and bare `exit N` forms of AC-4 and non-UTF-8 child output are not tested: every button
  prints a line, and the server runs its own executable.

Deferred for the person or a later ticket:

- SR1-11: watch the files on disk instead of reading them at each event.
- SR1-17: narrow the Zed matcher to ticket files instead of all Markdown.
- SR2-10: plan 3c.1 says "above each section"; the ticket gives per-ticket buttons anywhere in the file.
- DR-2 to DR-5, DR1-1, DR1-3, DR1-5, XR1-8: wording and coverage gaps of the drift and first review.
- CR2-2: the window between the hash check and the child `stapel ok`, which does not take the hash.
- CR2-3: a hung child blocks the server; no time limit on a run.
- XR2-4: parameters that do not deserialize get the serde text, not the AC-5 text.
- XR2-7: `stapel review start` should diff from the merge-base with main, not the old base.
- XR2-8, DR2-1: `didClose` sends an empty list for any path shaped like a `ticket.md`.
- XR2-9: the page check runs git on each code action request; cache it by file times.
- LOW leftovers: CR1-4 (serde text of `InvalidParams`), CR1-5 (a test name), XR1-9.

Measures: 19 Plan steps; three HIGH findings inside the promise, all before the code (SR1-1, SR1-2) or
fixed in round 1 (XR1-1); 23 MEDIUM inside; `ticket.md` 41 KB; wall time 8 h, attention 48 min.
