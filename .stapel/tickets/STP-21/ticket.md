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
   the Test plan's cases table, Inputs and External states tables that name a test of those criteria (a
   cases row also by naming the step); the records of `findings.jsonl` whose id the step names (id,
   severity, claim, fate). It writes nothing.
2. It refuses with exit 1 when the step is not in the Plan or is in it twice, a criterion the step names
   has no row, a finding it names has a recorded prefix but no record, the step names no criterion and no
   finding, or the brief exceeds 1600 tokens estimated as bytes / 4 (it names each part's size).
3. `stapel next` names `stapel brief KEY STEP` for `build-step` and `green-step` when the Plan has the step;
   when that brief refuses, `next` names its reason as a fact and the action `write-section Plan`.

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
- 2026-10-20: Decision (size after SR1-1, recommended): the 1600 limit stays a refusal, though no RED/GREEN
  step of STP-20 fits it (its Frame alone is about 810 tokens): a Frame is about 300 tokens (decision 0003)
  and a step about three criteria. Orchestrator rule: before the spec review, run `stapel brief` on every
  step and split the steps that refuse, so a refusal never comes after the confirmation; the rule goes into
  the skill of item 1.3. Bytes / 4 underestimates by about a fifth (SR1-15); accepted. Rejected: leave the
  Frame out of the count (briefs up to about 2500 tokens); refuse only over 3000.
- 2026-10-20: Decision (cases, SR1-7, recommended): the brief also carries the rows of the Test plan's cases
  table that name a step test or the step, since they hold the cases a builder writes in RED.
- 2026-10-20: Decision (empty step, SR1-8, recommended): a step that names no criterion and no finding
  refuses with exit 1. Rejected: a brief of Frame and step only.
- 2026-10-20: Decision (`next` and a refusing brief, SR1-5, recommended): `next` assembles the brief with the
  same function, writes nothing, and on a refusal names the reason as a fact and `write-section Plan`.
  Rejected: always name the command and leave the refusal to the orchestrator (a loop).
- 2026-10-20: Decision (after spec review round 2, recommended): the fixtures of the `next` tests gain a
  criterion and steps that name it (SR2-1), so `next` blocks only real tickets whose step names nothing;
  the size of this ticket's own steps is checked with `.stapel/scratch/stp21/estimate.py` before the
  confirmation, as the command does not exist yet (SR2-6, step 1 about 1500 tokens). No third round: the
  HIGH finding is settled in the text and the human decides by the confirmation (decision 0002: two rounds).

## Spec

Terms:

- **Brief command**: `stapel brief KEY STEP`.
- **Step item**: the Plan line whose label (`next::plan_steps`) is `step <STEP>`, and the lines after it up
  to the next step line, a blank line or the end of the Plan; ids are read in it with every run of
  whitespace (line breaks too) taken as one space, and it is printed as written.
- **Named criterion**: `AC-<n>` in the step item, and every `AC-<i>` with n ≤ i ≤ m for `AC-<n> to AC-<m>`.
- **Criterion row**: a Spec line that starts with `| AC-<n> |`; its tests are the backticked
  `<module>::<name>` names of its last cell.
- **Finding prefix**: the part of a finding id before its last `-<digits>` (`D1` of `D1-2`, `SR1` of
  `SR1-1`).
- **Named finding**: a word `<prefix>-<digits>` of the step item (not `AC`) whose prefix is the finding
  prefix of a record in the ticket's `findings.jsonl`; ranges as for a named criterion, with one prefix.
- **Term**: each `**<name>**:` of the list between the Spec line `Terms:` and the criteria table, with the
  text up to the next such name; a text uses it when `<name>` occurs in it as whole words, ASCII case
  ignored, the last word also with `s` or `es` added.
- **Step test**: a test of a named criterion; the step tests are listed in Spec order, each once.
- **Names a test**: a cell holds a backticked name equal to the test; a test of the step is a step test.
- **Cases table**: the first table of the `## Test plan` section; **Inputs table** and **External states
  table**: the tables under `### Inputs` and `### External states` in it.
- **Estimated tokens**: the bytes of a text divided by 4, rounded up.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN the brief command runs on an open or closed ticket whose Plan has the step THE command prints the line `# Brief: <KEY> step <STEP>` (`<KEY>` the folder key, here and below), the header lines, then the parts of AC-3 to AC-9 in that order, each under its `## ` heading, leaving out a part with nothing in it; exit 0; it writes no file. Output: `golden/brief_text.txt`. | `brief::prints_the_brief`, `brief::writes_nothing`, `brief::closed_ticket`, `brief::runs_in_an_agent_shell`, `brief::lettered_step_and_folder_key` |
| AC-2 | WHEN the brief is printed THE header lines are `ticket: .stapel/tickets/<KEY>/ticket.md` and `base: <base of state.json>` (or `base: none`), then the line of AC-18. | `brief::prints_the_brief`, `brief::base_of_state_json` |
| AC-3 | WHEN the Description has a `### Frame` subsection THE part `## Frame` holds its lines up to the next heading, without each paragraph that starts with `**Prior art.**` or `**Size.**`, trimmed. | `brief::frame_without_prior_art_and_size`, `brief::frame_edges` |
| AC-4 | WHEN the brief is printed THE part `## Step` holds the step item as written in the Plan. | `brief::prints_the_brief`, `brief::step_item_ends_at_a_blank_line` |
| AC-5 | WHEN the step item has a named criterion THE part `## Criteria` holds the header row and separator of the Spec criteria table, then each criterion row of a named criterion in Spec order, once. | `brief::criteria_and_ranges` |
| AC-6 | WHEN a printed criterion row uses a term THE part `## Terms` holds that term and each term a printed term uses, until none is added, in Spec order, once. | `brief::terms_used_and_their_terms` |
| AC-7 | WHEN a row of the cases table names a test of the step, or its first cell holds `step <STEP>` as whole words, THE part `## Cases` holds the header row, separator and each such row. | `brief::cases_of_the_step` |
| AC-8 | WHEN the last cell of a row of the Inputs table or the External states table names a test of the step THE parts `## Inputs` and `## External states` hold the header row, separator and each such row of their table. | `brief::rows_of_the_step_tests` |
| AC-9 | WHEN the step item has a named finding THE part `## Findings` holds one line per named finding in step order, once: `- <id> (<severity>): <claim> Fate: <fate>`, from the last well-formed record with that id, `none` for a missing field. | `brief::findings_last_record` |
| AC-10 | IF `STEP` is not digits followed by at most one lowercase letter, or `KEY` names no ticket or several in case, THEN the command prints `stapel: <reason>` on stderr and exits 2; IF the ticket is legacy, or `ticket.md`, `state.json` or an existing `findings.jsonl` cannot be read as a whole, THEN it exits 1 with `stapel: <reason>`. | `brief::refuses_a_wrong_call`, `brief::refuses_an_unreadable_ticket` |
| AC-11 | IF the Plan has no step `step <STEP>`, or no `plan` section is configured, THEN the command prints `stapel: the Plan of <KEY> has no step <STEP>` and exits 1; IF it has the step twice THEN `stapel: the Plan of <KEY> names step <STEP> twice`, exit 1. | `brief::step_not_in_the_plan` |
| AC-12 | IF a named criterion has no criterion row THEN the command prints `stapel: step <STEP> names AC-<n>, which the Spec does not have` for the first such id and exits 1; IF a range has n > m THEN `stapel: step <STEP> names AC-<n> to AC-<m>, which is not a range`, exit 1. | `brief::unknown_criterion` |
| AC-13 | IF a named finding has no well-formed record THEN the command prints `stapel: step <STEP> names <id>, which findings.jsonl does not have` for the first such id and exits 1. | `brief::unknown_finding` |
| AC-14 | IF the step item has no named criterion and no named finding THEN the command prints `stapel: step <STEP> names no criterion and no finding` and exits 1. | `brief::step_names_nothing` |
| AC-15 | IF the brief has more than 1600 estimated tokens THEN stdout is empty, the command prints `stapel: the brief of step <STEP> has <n> tokens, more than 1600: <part> <n>, …` (header and every printed part, in print order) and exits 1. | `brief::too_large` |
| AC-16 | WHEN `stapel next` names `build-step` or `green-step` for a label that is a Plan step and its brief assembles THE target is the command `stapel brief <KEY> <STEP>` (text after ` — `; `command` in `--json`, `path` `null`); without a `plan` section it stays the ticket path. | `next::build_step_names_the_brief` |
| AC-17 | IF that brief refuses with exit 1 (AC-11 to AC-15) THEN `stapel next` adds the fact `brief of step <STEP>: <the refusal without stapel: >` and the action line is `next: write-section <title of the plan section> — <ticket path>`; exit 0. | `next::brief_refusal_blocks_the_step` |
| AC-18 | WHEN the step has a step test THE header ends with the line `tests: <each step test, joined by ", ">`; with none the line is left out. | `brief::prints_the_brief`, `brief::empty_parts_are_left_out` |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-18; the guard lets the brief command run in an agent's
  shell; `stapel next` keeps every other action and target for a step whose brief assembles; in `--json`,
  `path` is `null` for a `build-step` or `green-step` that names the brief; the facts of `next` (AC-1 of
  STP-20) gain the kind `brief of step <STEP>: …`, so a step of an open ticket that names no criterion and
  no finding now stops `next` (AC-14, AC-17).
- **Not promised:** the Frame's list; a typo in a finding prefix (`DX-1`); other range forms (`AC-3–AC-5`,
  `AC-3..AC-5`, `AC-3 to 5`); an exclusion in the step (`AC-1 without <test>`: every test of AC-1 is a step
  test); other inflections of a term (`criteria` for `criterion`, `is done` for `Done step`); a term or step
  line inside a code fence; a Frame paragraph that starts `**Prior art**` without the dot; ids in other
  sections than the step item.

## Test plan

CLI tests in a new file `crates/stapel-cli/tests/brief.rs`, with `common::repo_with_ticket`,
`common::set_section` and a fixture ticket written by the test. Golden file
`crates/stapel-cli/tests/golden/brief_text.txt`; it grows in the RED of each step that adds a part.
Every refusal test asserts its own stderr text, so it fails while the command does not exist (clap's
unknown subcommand also exits 2); every other test asserts exit 0 and a printed `# Brief:` line, so an
equality or an absence is never its only assertion. Light tests:
`cargo test -j 4 -p stapel-cli --test brief -- --test-threads=3`; for step 8 also `--test next`, and before
each GREEN `--test ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Main path | a fixture with Frame, Plan steps `1.` and `2.`, criteria, terms, all three tables and findings; `stapel brief ABC-1 2`: the golden file | `brief::prints_the_brief` |
| Read-only | exit 0, a `# Brief:` line, and `common::snapshot` of the repository before and after the run is equal | `brief::writes_nothing` |
| Closed | the same fixture after `stapel close`: exit 0 and the golden file | `brief::closed_ticket` |
| Agent shell | `CLAUDECODE=1` set and the guard hook asked about `stapel brief ABC-1 2`: allowed, and the run prints the brief | `brief::runs_in_an_agent_shell` |
| Lettered step | a Plan step `2c.` and `stapel brief abc-1 2c` on folder `ABC-1`: `# Brief: ABC-1 step 2c`, path with `ABC-1` (D1-7) | `brief::lettered_step_and_folder_key` |
| Base | `state.json` with a base: `base: <that sha>` (D1-2) | `brief::base_of_state_json` |
| Step end | a step item followed by a blank line and a paragraph: the paragraph is not printed (D1-1) | `brief::step_item_ends_at_a_blank_line` |
| Frame edges | a Frame of only Prior art and Size paragraphs: no part; a Frame with no heading after it (D1-7) | `brief::frame_edges` |
| Empty parts | a step that names one finding only: exit 0, `## Frame`, `## Step` and `## Findings` printed, no `tests:` line; no `## Criteria`, `## Terms`, `## Cases`, `## Inputs`, `## External states` | `brief::empty_parts_are_left_out` |
| Frame | `**Prior art.**` and `**Size.**` paragraphs of two lines each are dropped; `**Prior art**` without the dot and a paragraph that only mentions `**Size.**` later are kept; no `### Frame`: no part | `brief::frame_without_prior_art_and_size` |
| Criteria | `AC-1, AC-3 to AC-5`; `AC-3` named twice; Spec order kept when the step names `AC-5` before `AC-1`; `AC-10` is not `AC-1`; a range broken after `to` with an indented next line | `brief::criteria_and_ranges` |
| Terms | a term used in upper case; a term used with `s` and with `es` added; a term used only by another term; a name that is a prefix of a longer word (`Step` in `Stepping`) is not used; two terms on one line; a `- **Promised:**` line after the table is no term | `brief::terms_used_and_their_terms` |
| Cases | a row naming a step test with another test; a row `Coverage (step 2)`; a row `Coverage (step 2c)` for step 2 is not taken; a row naming `x::a_b` for the step test `x::a` is not taken | `brief::cases_of_the_step` |
| Tables | a row naming a step test with another test; a row naming only another step's test; a row with `—`; a Test plan without the tables | `brief::rows_of_the_step_tests` |
| Findings | two records of `D1-2`, the last counts; a record without `fate`; an unreadable line; `D1-5 to D1-6`; `UTF-8` and `STP-22` in the step are no findings | `brief::findings_last_record` |
| Wrong call | `STEP` `x`, `c2`, `2cc`, `2C`, `step 2`; `KEY` `NOPE-1`; two folders differing in case; a missing argument: exit 2, stderr text, empty stdout | `brief::refuses_a_wrong_call` |
| Unreadable | `state.json` not JSON; a legacy ticket; `ticket.md` not UTF-8; `findings.jsonl` a directory or not UTF-8: exit 1 | `brief::refuses_an_unreadable_ticket` |
| No step | `STEP` `3` with steps `1.` and `2.`; a `stapel.toml` without the `plan` section; a Plan that is `TODO`; two lines `2.` | `brief::step_not_in_the_plan` |
| Unknown criterion | `AC-9` with no row; `AC-5 to AC-3` with both rows present | `brief::unknown_criterion` |
| Unknown finding | `D1-9` with records `D1-1` and `D1-2`; the only `D1-9` line unreadable while `D1-1` exists | `brief::unknown_finding` |
| Names nothing | a step with no `AC-` and no recorded prefix (no `findings.jsonl`, the words `D1-1` and `UTF-8`) | `brief::step_names_nothing` |
| Size | a Frame of 6 000 bytes: exit 1, the part sizes, empty stdout; a brief of exactly 1600 tokens passes, 1601 refuses | `brief::too_large` |
| Next | `build-step` and `green-step` name `stapel brief ABC-1 <n>` in text and JSON; without a `plan` section the open RED keeps the ticket path; the golden files of `next` change only on those lines | `next::build_step_names_the_brief` |
| Next refusal | the first open step names `AC-9` with no row: the fact and `write-section Plan`; a step naming nothing: its fact; a too large brief: the fact with the sizes; exit 0; `next` writes nothing | `next::brief_refusal_blocks_the_step` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` (positional of `brief`) | ticket folder name, any case | one ticket | absent: exit 2 (clap) | no such folder, several in case: exit 2; legacy or unreadable: exit 1 | ASCII case ignored | `brief::refuses_a_wrong_call`, `brief::refuses_an_unreadable_ticket` |
| `STEP` (positional of `brief`) | digits followed by at most one lowercase ASCII letter | `1`; any number of digits | absent: exit 2 (clap) | `x`, `c2`, `2cc`, `2C`, `step 2`: exit 2 | exact label | `brief::refuses_a_wrong_call`, `brief::step_not_in_the_plan` |
| `ticket.md` parts | Markdown | — | a missing Frame, table or term list: no part | not UTF-8: exit 1 | title case of sections ignored (`ticket::find`) | `brief::empty_parts_are_left_out`, `brief::refuses_an_unreadable_ticket` |
| criterion ids in the step | `AC-<digits>`, `AC-<n> to AC-<m>` | `AC-1` | none: no part | no row, n > m: exit 1 | whole word: `AC-10` is not `AC-1` | `brief::criteria_and_ranges`, `brief::unknown_criterion` |
| finding ids in the step | `<prefix>-<digits>`, ranges with one prefix | — | none: no part; neither ids: exit 1 | a recorded prefix, no record: exit 1 | exact id | `brief::findings_last_record`, `brief::unknown_finding`, `brief::step_names_nothing` |
| `findings.jsonl` | JSON lines with a string `id` | — | no file: no finding prefix | a line that is not JSON or has no string `id`: skipped; a file not readable as a whole: exit 1 | last record wins | `brief::findings_last_record`, `brief::refuses_an_unreadable_ticket` |
| `state.json` `base` | string | — | absent: `base: none` | not JSON, legacy: exit 1 | as written | `brief::empty_parts_are_left_out`, `brief::refuses_an_unreadable_ticket` |
| brief size | estimated tokens | 1600 passes | — | 1601 or more: exit 1 | bytes / 4, rounded up | `brief::too_large` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | every dimension | not read by `brief`; `next` reads it as before | — |
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
2026-10-20, after spec review round 1 (external): 3 HIGH, 5 MEDIUM, 7 LOW; 12 inside, 3 outside, all
outside ones taken with the human. One change: the size limit stays with an orchestrator rule (SR1-1); the
step item is one text (SR1-2); terms match with `s`/`es`, other inflections not promised, and the Spec uses
its terms exactly (SR1-3); several terms per line, only the Terms list (SR1-4, SR1-11); `next` assembles
the brief and blocks on its refusal (SR1-5, AC-17); exclusions and other range forms not promised (SR1-6,
SR1-10); the part `## Cases` (SR1-7, AC-7); a step naming nothing refuses (SR1-8, AC-14); `STEP` wording,
equal test names, a label twice, the range text (SR1-9, SR1-12 to SR1-14); SR1-15 accepted in the size
decision. Round 2 runs, as SR1-1 to SR1-3 are HIGH.
2026-10-20, after spec review round 2 (external): SR1-1 to SR1-15 settled, SR1-3 partly (SR2-2); 1 HIGH,
3 MEDIUM, 5 LOW, all inside. One change: step 8's RED gives the `next` fixtures a criterion (SR2-1);
AC-7 and AC-8 use the term Names a test (SR2-2); every non-refusal test asserts exit 0 and a printed
brief (SR2-3); the `tests:` line moves to AC-18 in step 3 (SR2-4); the Step item is printed as written
(SR2-5); AC-17 uses the plan section's title and the Guarantees name the new fact (SR2-7); `<KEY>` is the
folder key (SR2-8); an unreadable `findings.jsonl` exits 1 (SR2-9); SR2-6 noted in Decisions.
2026-10-20, after the drift review of step 1: D1-1 to D1-7 (3 MEDIUM, 4 LOW). One spec change: plan step
1c with four coverage tests (D1-1, D1-2, D1-7) and the two refusal tests of AC-10 and AC-11 whose code
step 1 built early (D1-3); step 2 keeps only the check of `STEP`. No change for D1-4 (the Description
title is fixed, as in `next`), D1-5 (heading forms, Design as built), D1-6 (the guard half guards against
a future guard rule).
2026-10-21, before step 3: `brief::empty_parts_are_left_out` (AC-18) needs the Findings part, so it is
written in step 6, not step 3; step 6 also adds the `findings.jsonl` cases of the unreadable test, left out
of step 1c (findings were not read yet). Drift review of step 2: D2-1 to D2-4 (1 MEDIUM, 3 LOW), no change
(D2-1: `2c` is accepted in `brief::lettered_step_and_folder_key`).

## Design

### Facts

- F-1: `tickets::resolve` gives the ticket choice and its texts; `stapel next` maps them to exit 2 and a
  legacy or unreadable ticket to exit 1 (STP-20, `crates/stapel-cli/src/next.rs`).
- F-2: `next::plan_steps` and `step_label` give the labels of the Plan (`step 1c`), repeats kept;
  `ticket::parse` and `ticket::find` give `## ` sections, so `### Frame`, `### Inputs` and `### External
  states` are found inside the body of their section.
- F-3: the guard treats only `ok`, `close`, `findings` and `ledger` of `stapel` as person commands
  (`guard::check_stapel`); every other subcommand passes.
- F-4: `next::history_action` builds `build-step` and `green-step` with `Target::Path(ticket_path)`.

### Decisions (before the build)

- A pure function `stapel_core::brief::assemble` takes the ticket text, the `findings.jsonl` text, the key,
  the step and the base, and returns the parts or a refusal; `crates/stapel-cli/src/brief.rs` reads the
  files and prints. Most cases are tested through the binary on fixture tickets.
- The size check runs on the assembled text before anything is printed, so a refusal leaves stdout empty.
- `next` calls `assemble` only for the step it names, so its other actions read nothing new; it builds the
  brief command only when the label is a Plan step, so the no-`plan` case of AC-21 of STP-20 keeps its path.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `brief::prints_the_brief`, `brief::writes_nothing`, `brief::closed_ticket`, `brief::runs_in_an_agent_shell`, `brief::lettered_step_and_folder_key` |
| AC-2 | `brief::prints_the_brief`, `brief::base_of_state_json` |
| AC-3 | `brief::frame_without_prior_art_and_size`, `brief::frame_edges` |
| AC-4 | `brief::prints_the_brief`, `brief::step_item_ends_at_a_blank_line` |
| AC-5 | `brief::criteria_and_ranges` |
| AC-6 | `brief::terms_used_and_their_terms` |
| AC-7 | `brief::cases_of_the_step` |
| AC-8 | `brief::rows_of_the_step_tests` |
| AC-9 | `brief::findings_last_record` |
| AC-10 | `brief::refuses_a_wrong_call`, `brief::refuses_an_unreadable_ticket` |
| AC-11 | `brief::step_not_in_the_plan` |
| AC-12 | `brief::unknown_criterion` |
| AC-13 | `brief::unknown_finding` |
| AC-14 | `brief::step_names_nothing` |
| AC-15 | `brief::too_large` |
| AC-16 | `next::build_step_names_the_brief` |
| AC-17 | `next::brief_refusal_blocks_the_step` |
| AC-18 | `brief::prints_the_brief`, `brief::empty_parts_are_left_out` |

## Plan

Route: full (tag `interface`), auto.

1. The command prints the header without its `tests:` line, the Frame and the Step (AC-1 to AC-4). Must not change: `stapel next`,
   `stapel status`, `ticket::parse`, `tickets::resolve`.
1c. Coverage after the drift review of step 1 (D1-1, D1-2, D1-3, D1-7): four new `#[test]` functions
   (`brief::lettered_step_and_folder_key`, `brief::base_of_state_json`,
   `brief::step_item_ends_at_a_blank_line`, `brief::frame_edges`) and `brief::step_not_in_the_plan` and
   `brief::refuses_an_unreadable_ticket` of AC-10 and AC-11, whose code step 1 already built. Tests only,
   each helper nested inside its function.
2. The check of `STEP` and the ticket choice (AC-10, test `brief::refuses_a_wrong_call` only). Must not
   change: the brief of step 1 and the refusals tested in step 1c.
3. Criteria, the `tests:` line and the unknown criterion (AC-5, AC-12, AC-18; the golden file holds the
   `tests:` line). Must not change: the parts of step 1.
4. Terms and a step naming nothing (AC-6, AC-14). Must not change: the parts of steps 1 and 3.
5. Cases, Inputs and External states rows (AC-7, AC-8). Must not change: the parts of steps 1, 3 and 4.
6. Findings and the unknown finding (AC-9, AC-13), with `brief::empty_parts_are_left_out` of AC-18 (it
   needs the Findings part) and the `findings.jsonl` cases of `brief::refuses_an_unreadable_ticket`. Must not change: the format of `findings.jsonl` and
   `stapel findings`.
7. The size limit (AC-15). Must not change: the text of a brief within the limit.
8. `stapel next` names the brief and blocks on its refusal (AC-16, AC-17). The RED gives the shared
   fixtures of `tests/next.rs` (`written`, `ready` and the Plans `1. One.`) a criterion row `AC-1` and steps
   that name it, and changes the expected `build-step` and `green-step` targets. Must not change: every
   other action of `next`, its facts for a step whose brief assembles, and its exit codes.

## Review

TODO

## Summary

TODO
