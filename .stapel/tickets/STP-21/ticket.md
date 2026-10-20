# STP-21 — stapel brief: the builder brief of one step

## Description

Source: `docs/PHASES.md`, plan item 1.2 (`docs/decisions/0003-ticket-documents-from-bmad.md`, label STP-10);
the ledger decision of 2026-10-20 that changes the item.

### Frame

**Problem.** Today the orchestrator writes each builder brief by hand from the 50 KB ticket, and a builder
that gets only the ticket path reads all of it. `stapel brief KEY STEP` assembles the brief of one Plan step
from the ticket, so neither reads more than the step needs.

**Plan item:** 1.2.

**Risk tags:** `interface` (a new command whose output a builder reads; `stapel next` names it).

**Non-goals.** Fact ids `F-n` (decision 0003, item 5); the general working rules of a builder (item 1.3,
the agent definition); a test command; Design text; checking confirmations (`next` does); the ticket lint
(item 1.6); writing any file.

**Promise (closed list).**

1. `stapel brief KEY STEP` (`STEP` as `2` or `2c`) prints on stdout, in this order: a header (key, step
   label, base from `state.json`, ticket path, the test names of the step's criteria); the Frame without its
   `**Prior art.**` and `**Size.**` paragraphs; the step item of the Plan; the Spec criteria the step names
   (`AC-n`, `AC-n to AC-m`); the Spec terms those criteria use, with the terms those terms use; the rows of
   the Inputs and External states tables whose Test cell names a test of those criteria; the records of
   `findings.jsonl` whose id the step names (id, severity, claim, fate). It writes nothing.
2. It refuses with exit 1 when the step is not in the Plan, a criterion the step names has no row, a
   finding it names has a recorded prefix but no record, or the brief exceeds 1600 tokens estimated as
   bytes / 4 (it names each part's size).
3. `stapel next` names `stapel brief KEY STEP` for `build-step` and `green-step` when the Plan has the step.

Not promised: steps or tables written in another form; a finding id inside a criterion; a brief between
900 and 1300 tokens (a target, not checked); deliberate circumvention.

**Prior art.** The hand briefs of STP-5 to STP-20 (`.stapel/scratch/stp5-r1-brief.md`); `next::plan_steps`,
`ticket::find`, `tickets::resolve`; BMAD's build plan (900–1300 tokens, split at 1600).

**Size.** About 400 changed source lines (`crates/stapel-core`: a `brief` module; `crates/stapel-cli`: the
command, `main.rs`, the `next` target) and about 1 200 test lines. Past about 600 source lines the ticket is
split before the spec review. Route: full (tag `interface`), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-20: Decision (output, recommended): the brief goes to stdout and the command writes nothing; `next`
  names the command, the builder runs it, the orchestrator does not read the brief. Rejected: a file under
  the ticket with its path (two kinds of target in `next`). Recorded in the ledger as a change of item 1.2.
- 2026-10-20: Decision (size, recommended): over 1600 estimated tokens the command refuses with exit 1 and
  names the size of each part; the author splits the step or shortens the Frame. Rejected: print with a
  warning, which a careless agent skips.
- 2026-10-20: Decision (Frame, recommended): the Frame enters without the Prior art and Size paragraphs.
- 2026-10-20: Decision (contents, recommended): the brief also carries the Spec terms its criteria use, a
  header (key, step, base, ticket path, test names) that replaces the three things CLAUDE.md gives a builder,
  and the findings the step names, because coverage and review steps name no criterion. Not taken: Design
  decisions, the general working rules (item 1.3), a test command (the Test plan line is free text).
- 2026-10-20: Decision (key, recommended): `KEY` is required, as in item 1.2; a closed ticket gives its
  brief too, as the command writes nothing.
- 2026-10-20: Decision (unknown ids, recommended): a criterion id with no row, or a finding id whose prefix
  a recorded finding of the ticket has but whose record is missing (`D1-9`), refuses with exit 1; other
  ids (`UTF-8`, `STP-22`, `F-3`) are not findings. Rejected: refuse every unknown id except the key format,
  which refuses `UTF-8`; a word list in the code; ids only in brackets.
- 2026-10-20: Decision (terms, recommended): a term is used when its name occurs as whole words, ASCII case
  ignored. Rejected: exact case; every term in every brief (about 550 tokens in STP-20).
- 2026-10-20: Decision (findings, recommended): of several records with one id the last well-formed one
  counts; unreadable lines are skipped, as `status` skips them. Rejected: refuse a repeated id.
- 2026-10-20: Decision (form, recommended): Markdown, one `## ` heading per part; a part with nothing in it
  is left out. Rejected: `--json` (no reader needs it); a part with the word `none`.

## Spec

Terms:

- **Brief command**: `stapel brief KEY STEP`.
- **Step item**: the Plan line whose label (`next::plan_steps`) is `step <STEP>`, and the lines after it up
  to the next step line, a blank line or the end of the Plan.
- **Named criterion**: `AC-<n>` in the step item, and every `AC-<i>` with n ≤ i ≤ m for `AC-<n> to AC-<m>`.
- **Criterion row**: a Spec line that starts with `| AC-<n> |`; its tests are the backticked
  `<module>::<name>` names of its last cell.
- **Finding prefix**: the part of a finding id before its last `-<digits>` (`D1` of `D1-2`, `SR1` of
  `SR1-1`).
- **Named finding**: a word `<prefix>-<digits>` of the step item (not `AC`) whose prefix is the finding
  prefix of a record in the ticket's `findings.jsonl`; ranges as for criteria, with one prefix.
- **Term**: a Spec line `- **<name>**: …` with the indented lines under it; a text uses it when `<name>`
  occurs in it as whole words, ASCII case ignored.
- **Step tests**: the tests of the named criteria, in Spec order, each once.
- **Estimated tokens**: the bytes of a text divided by 4, rounded up.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the brief command runs on an open or closed ticket whose Plan has the step THE command prints the brief: the line `# Brief: <KEY> step <STEP>`, the header lines, then the parts of AC-3 to AC-8 in that order, each under its `## ` heading, leaving out a part with nothing in it; exit 0; it writes no file. Output: `golden/brief_text.txt`. | `brief::prints_the_brief`, `brief::writes_nothing`, `brief::closed_ticket`, `brief::runs_in_an_agent_shell` |
| AC-2 | WHEN the brief is printed THE header lines are `ticket: .stapel/tickets/<KEY>/ticket.md`, `base: <base of state.json>` (or `base: none`) and `tests: <step tests, joined by ", ">`, the last left out when there are none. | `brief::prints_the_brief`, `brief::empty_parts_are_left_out` |
| AC-3 | WHEN the Description has a `### Frame` subsection THE part `## Frame` holds its lines up to the next heading, without each paragraph that starts with `**Prior art.**` or `**Size.**`, trimmed. | `brief::frame_without_prior_art_and_size` |
| AC-4 | WHEN the brief is printed THE part `## Step` holds the step item as written. | `brief::prints_the_brief` |
| AC-5 | WHEN the step names criteria THE part `## Criteria` holds the header row and separator of the Spec criteria table, then the rows of the named criteria in Spec order, each once. | `brief::criteria_and_ranges` |
| AC-6 | WHEN the printed criteria use terms THE part `## Terms` holds those terms and the terms they use, again until none is added, in Spec order, each once. | `brief::terms_used_and_their_terms` |
| AC-7 | WHEN a row of the `### Inputs` or `### External states` table of the Test plan names a step test in its last cell THE parts `## Inputs` and `## External states` hold the header row, separator and each such row of their table. | `brief::rows_of_the_step_tests` |
| AC-8 | WHEN the step names findings THE part `## Findings` holds one line per finding in step order, each once: `- <id> (<severity>): <claim> Fate: <fate>`, from the last well-formed record with that id, `none` for a missing field. | `brief::findings_last_record` |
| AC-9 | IF `STEP` is not digits with at most one lowercase letter, or `KEY` names no ticket or several in case, THEN the command prints `stapel: <reason>` on stderr and exits 2; IF the ticket is legacy or its files cannot be read THEN it exits 1 with `stapel: <reason>`. | `brief::refuses_a_wrong_call`, `brief::refuses_an_unreadable_ticket` |
| AC-10 | IF the Plan has no step `step <STEP>`, or no `plan` section is configured, THEN the command prints `stapel: the Plan of <KEY> has no step <STEP>` and exits 1. | `brief::step_not_in_the_plan` |
| AC-11 | IF a named criterion has no criterion row, or a range has n > m, THEN the command prints `stapel: step <STEP> names AC-<n>, which the Spec does not have` (the first such id; the range as written for n > m) and exits 1. | `brief::unknown_criterion` |
| AC-12 | IF a named finding has no well-formed record THEN the command prints `stapel: step <STEP> names <id>, which findings.jsonl does not have` for the first such id and exits 1. | `brief::unknown_finding` |
| AC-13 | IF the brief has more than 1600 estimated tokens THEN stdout is empty, the command prints `stapel: the brief of step <STEP> has <n> tokens, more than 1600: <part> <n>, …` (header and every printed part, in print order) and exits 1. | `brief::too_large` |
| AC-14 | WHEN `stapel next` names `build-step` or `green-step` for a label that is a Plan step THE target is the command `stapel brief <KEY> <STEP>` (text after ` — `; `command` in `--json`, `path` `null`); without a `plan` section it stays the ticket path. | `next::build_step_names_the_brief` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-14; the guard lets the brief command run in an agent's
  shell; `stapel next` keeps every other action and target.
- **Not promised:** the Frame's list; a typo in a finding prefix (`DX-1`); a term or step line inside a code
  fence; a Frame paragraph that starts `**Prior art**` without the dot; an `AC-<n>` inside a finding's
  claim; ids in other sections than the step item.

## Test plan

CLI tests in a new file `crates/stapel-cli/tests/brief.rs`, with `common::repo_with_ticket`,
`common::set_section` and a fixture ticket written by the test. Golden file
`crates/stapel-cli/tests/golden/brief_text.txt`. Every refusal test asserts its own stderr text, so it fails
while the command does not exist (clap's unknown subcommand also exits 2). Light tests:
`cargo test -j 4 -p stapel-cli --test brief -- --test-threads=3`; for step 5 also `--test next`, and before
each GREEN `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Main path | a fixture with Frame, Plan steps `1.` and `2.`, criteria, terms, both tables and findings; `stapel brief ABC-1 2`: the golden file | `brief::prints_the_brief` |
| Read-only | `common::snapshot` of the repository before and after a run is equal | `brief::writes_nothing` |
| Closed | the same fixture after `stapel close`: the same stdout | `brief::closed_ticket` |
| Empty parts | a step with no criteria, terms, rows or findings: no `tests:` line, only `## Frame` and `## Step` | `brief::empty_parts_are_left_out` |
| Frame | `**Prior art.**` and `**Size.**` paragraphs of two lines each are dropped; `**Prior art**` without the dot and a paragraph that only mentions `**Size.**` later are kept; no `### Frame`: no part | `brief::frame_without_prior_art_and_size` |
| Criteria | `AC-1, AC-3 to AC-5`; `AC-3` named twice; Spec order kept when the step names `AC-5` before `AC-1`; `AC-10` is not `AC-1` | `brief::criteria_and_ranges` |
| Terms | a term used in upper case; a term used only by another term; a term whose name is a prefix of a word (`Step` in `Steps`) is not used; a term with an indented second line | `brief::terms_used_and_their_terms` |
| Tables | a row naming a step test with another test; a row naming only another step's test; a row with `—`; a Test plan without the tables | `brief::rows_of_the_step_tests` |
| Findings | two records of `D1-2`, the last counts; a record without `fate`; an unreadable line; `D1-5 to D1-6`; `UTF-8` and `STP-22` in the step are no findings | `brief::findings_last_record` |
| Wrong call | `STEP` `x`, `2cc`, `2C`, `step 2`; `KEY` `NOPE-1`; two folders differing in case; a missing argument: exit 2, stderr text, empty stdout | `brief::refuses_a_wrong_call` |
| Unreadable | `state.json` not JSON; a legacy ticket; `ticket.md` not UTF-8: exit 1 | `brief::refuses_an_unreadable_ticket` |
| No step | `STEP` `3` with steps `1.` and `2.`; a `stapel.toml` without the `plan` section; a Plan that is `TODO` | `brief::step_not_in_the_plan` |
| Unknown criterion | `AC-9` with no row; `AC-5 to AC-3` | `brief::unknown_criterion` |
| Unknown finding | `D1-9` with records `D1-1` and `D1-2`; the only `D1-9` line unreadable; no `findings.jsonl` and `D1-1` named: no finding, exit 0 | `brief::unknown_finding` |
| Size | a Frame of 6 000 bytes: exit 1, the part sizes, empty stdout; a brief of exactly 1600 tokens passes | `brief::too_large` |
| Agent shell | `CLAUDECODE=1` set and the guard hook asked about `stapel brief ABC-1 2`: allowed, and the run prints the brief | `brief::runs_in_an_agent_shell` |
| Next | `build-step` and `green-step` name `stapel brief ABC-1 <n>` in text and JSON; without a `plan` section the open RED keeps the ticket path; the golden files of `next` change only on those lines | `next::build_step_names_the_brief` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` (positional of `brief`) | ticket folder name, any case | one ticket | absent: exit 2 (clap) | no such folder, several in case: exit 2; legacy or unreadable: exit 1 | ASCII case ignored | `brief::refuses_a_wrong_call`, `brief::refuses_an_unreadable_ticket` |
| `STEP` (positional of `brief`) | digits and at most one lowercase ASCII letter | `1`; any number of digits | absent: exit 2 (clap) | `x`, `2cc`, `2C`, `step 2`: exit 2 | exact label | `brief::refuses_a_wrong_call`, `brief::step_not_in_the_plan` |
| `ticket.md` parts | Markdown | — | a missing Frame, table or term list: no part | not UTF-8: exit 1 | title case of sections ignored (`ticket::find`) | `brief::empty_parts_are_left_out`, `brief::refuses_an_unreadable_ticket` |
| criterion ids in the step | `AC-<digits>`, `AC-<n> to AC-<m>` | `AC-1` | none: no part | no row, n > m: exit 1 | whole word: `AC-10` is not `AC-1` | `brief::criteria_and_ranges`, `brief::unknown_criterion` |
| finding ids in the step | `<prefix>-<digits>`, ranges with one prefix | — | none: no part | a recorded prefix, no record: exit 1 | exact id | `brief::findings_last_record`, `brief::unknown_finding` |
| `findings.jsonl` | JSON lines with a string `id` | — | no file: no finding prefix | a line that is not JSON or has no string `id`: skipped | last record wins | `brief::findings_last_record` |
| `state.json` `base` | string | — | absent: `base: none` | not JSON, legacy: exit 1 | as written | `brief::empty_parts_are_left_out`, `brief::refuses_an_unreadable_ticket` |
| brief size | estimated tokens | 1600 passes | — | 1601 or more: exit 1 | bytes / 4, rounded up | `brief::too_large` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | every dimension | not read | — |
| cargo | every dimension | not run | — |
| claude-code | environment of hook processes versus the agent's Bash shell | the guard allows `stapel brief` and the command runs with `CLAUDECODE` set | `brief::runs_in_an_agent_shell` |
| claude-code | permission mode, settings, transcripts, tool results | not read | — |
| os | case-insensitive file systems; symlinks; permissions | as `stapel status` reads the ticket; an unreadable file exits 1 | `brief::refuses_an_unreadable_ticket` |
| os | processes, time, WSL | not touched | — |

**Review Focus:** a step line `1.` and `01.` (labels differ, as in `next`; not promised); a term named
inside backticks (`` `Step item` ``; whole-word match finds it; terms test); a Frame `### Frame` heading
written twice (the first counts; not tested); a closed stdout pipe (`out.rs` drops it, STP-16; not tested);
a criterion row of the Test plan table that starts with `| AC-` (only the Spec section is read; criteria
test).

**Author self-check:** 2026-10-20, before spec review: every promise item has a criterion and a test; each
input has its empty and invalid case; the Inputs and External states rows were checked against
`ticket.rs`, `tickets.rs`, `next.rs`, `guard.rs` and the catalogue; no test is expected to pass before its
code (every one runs `stapel brief`, or asserts the new target of `next`); no open question is left.

## Design

### Facts

- F-1: `tickets::resolve` gives the ticket choice and its texts; `stapel next` maps them to exit 2 and a
  legacy or unreadable ticket to exit 1 (STP-20, `crates/stapel-cli/src/next.rs`).
- F-2: `next::plan_steps` and `step_label` give the labels of the Plan (`step 1c`); `ticket::parse` and
  `ticket::find` give `## ` sections, so `### Frame`, `### Inputs` and `### External states` are found
  inside the body of their section.
- F-3: the guard treats only `ok`, `close`, `findings` and `ledger` of `stapel` as person commands
  (`guard::check_stapel`); every other subcommand passes.
- F-4: `next::history_action` builds `build-step` and `green-step` with `Target::Path(ticket_path)`.

### Decisions (before the build)

- A pure function `stapel_core::brief::assemble` takes the ticket text, the `findings.jsonl` text, the key,
  the step and the base, and returns the parts or a refusal; `crates/stapel-cli/src/brief.rs` reads the
  files and prints. Most cases are tested through the binary on fixture tickets.
- The size check runs on the assembled text before anything is printed, so a refusal leaves stdout empty.
- `next` builds the brief command only when the label is a Plan step, so the no-`plan` case of AC-21 of
  STP-20 keeps its path.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `brief::prints_the_brief`, `brief::writes_nothing`, `brief::closed_ticket`, `brief::runs_in_an_agent_shell` |
| AC-2 | `brief::prints_the_brief`, `brief::empty_parts_are_left_out` |
| AC-3 | `brief::frame_without_prior_art_and_size` |
| AC-4 | `brief::prints_the_brief` |
| AC-5 | `brief::criteria_and_ranges` |
| AC-6 | `brief::terms_used_and_their_terms` |
| AC-7 | `brief::rows_of_the_step_tests` |
| AC-8 | `brief::findings_last_record` |
| AC-9 | `brief::refuses_a_wrong_call`, `brief::refuses_an_unreadable_ticket` |
| AC-10 | `brief::step_not_in_the_plan` |
| AC-11 | `brief::unknown_criterion` |
| AC-12 | `brief::unknown_finding` |
| AC-13 | `brief::too_large` |
| AC-14 | `next::build_step_names_the_brief` |

## Plan

Route: full (tag `interface`), auto.

1. The command, the header, the Frame and the Step, its refusals (AC-1 to AC-4, AC-9, AC-10, and the agent
   shell row). Must not change: `stapel next`, `stapel status`, `ticket::parse`, `tickets::resolve`.
2. Criteria and terms with the unknown criterion (AC-5, AC-6, AC-11). Must not change: the parts of step 1.
3. Tables and findings with the unknown finding (AC-7, AC-8, AC-12). Must not change: the format of
   `findings.jsonl` and `stapel findings`.
4. The size limit (AC-13). Must not change: the brief text of a brief within the limit.
5. `stapel next` names the brief (AC-14). Must not change: every other action and target of `next`, and
   the exit codes of `next`.

## Review

TODO

## Summary

TODO
