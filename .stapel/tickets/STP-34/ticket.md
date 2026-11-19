# STP-34 — The elaboration skill

## Description

Source: `docs/PHASES.md`, plan item 4.1; ledger decisions 2026-11-10 (the feature level; the texts are
ported from the method's repository) and 2026-11-17 (phases 3 to 5 start before the gate).

### Frame

**Problem.** stapel elaborates one ticket at a time: the Frame questions, the criteria and the spec
review. A piece of work larger than one ticket has no text that turns an intent into requirements with
ids and a breakdown into tickets, and phase 4 needs that text before the kind `feature` (4.2) can store
its result. The method studied in decisions 0001 to 0003 has these texts under the MIT licence; this
ticket ports them, adapted to stapel's rules, into a second skill that `stapel init` installs.

**Plan item:** 4.1.

**Risk tags:** `interface` (four new files that `stapel init` installs and a re-run updates; a skill
the orchestrator runs; a new marker form for files without frontmatter).

**Non-goals.** The kind `feature` and any stapel command that reads a feature file (4.2); requirement
hashes and staleness (4.3); computed readiness (4.4); the kind `bug`; personas, tracker stores, the
method's scripts or any Python; a stapel command that compares two snapshots of the method; making the
model follow the skill text; a change of the ticket process of the skill `stapel`, other than one line
that names the new skill.

**Promise (closed list).**

1. `stapel init` writes four files in the folder `.claude/skills/stapel-elaborate/`: `SKILL.md`,
   `template.md`, `checklist.md` and `NOTICE.md`. Each carries a stapel marker with the version and the
   hash; `SKILL.md` has it in its frontmatter as the skill `stapel` has. A re-run creates a missing file,
   updates an unchanged file of stapel, and keeps an edited, unmarked or newer file with a warning
   line, as for the other process files; a person's own file in that folder is kept so, and the missing
   files are still written. The local and the shared mode, and `stapel export`, treat the folder as they
   treat the folder of the skill `stapel`.
2. `SKILL.md` says when the skill applies (a piece of work the person describes as larger than one
   ticket), the questions it asks (elicit the person's intent, never author it; at most four per dialog,
   each with options and a recommendation), and its steps: write the draft from `template.md` to
   `.stapel/features/<slug>.md`, run `checklist.md` over it, fix or list what fails, and name the path
   in the chat. No stapel command reads that file in this ticket.
3. `template.md` has the sections Description, Outcome, Done when (three to six checks), Boundaries (at
   least one thing out), Requirements (lines `R<n>`, ids never reused or renumbered), Constraints,
   Decisions (typed lines: Decision, Assumption, Open question) and Breakdown (one line per ticket with
   the ids it covers and the tickets it comes after).
4. `checklist.md` lists the checks with stable ids (`EL-<n>`), so that a finding can name the check as
   its rule and rule 9 of the spec process can prune it; a pruned id is never reused.
5. `NOTICE.md` holds the method's copyright and permission notice word for word, the source repository,
   the commit `e435a22`, the source files each stapel file adapts, and a statement that the texts are
   adapted from the method. The method's name appears in no installed file other than `NOTICE.md`.
6. The skill `stapel` gets one line that names the skill `stapel-elaborate` for work larger than one
   ticket.
7. A first report of the method's snapshot (what is taken, what is not, and why) is a new document
   under `docs/method-reports/`, named for the commit.
8. This repository gets the four files from `stapel init`, and
   `init_process::this_repository_has_the_current_files` passes.

Not promised: a check by code that a feature draft follows the template or the checklist; a CRLF file;
deliberate circumvention.

**Prior art.** Decisions 0001 to 0003 (what was taken from the method before: the coverage map, the
silent-dimension rule, non-goals, review lenses, typed Decisions lines); the ledger decisions of
2026-11-04 and 2026-11-10 (why stapel ports texts and does not run the method); STP-22 (the marker and
the re-run rules of the process files); STP-33 (the first-line marker of a file without frontmatter);
the method's repository read on 2026-11-19: tag v6.12.4 (2026-10-14, maintenance branch of v6:
`bmad-prd`, `bmad-create-epics-and-stories`) and main at `e435a22` (2026-11-12, draft of v7: the PM
agent's PRD templates; the PO agent's `references/spec.md`, `slice.md` and `validate-checks.md`).

**Size.** About 130 changed source lines (`crates/stapel-cli/src/init.rs`: four more files, the marker of
a file without frontmatter, the skill folder in both modes; `crates/stapel-cli/src/export.rs`: the folder
as a process path), about 350 lines of skill text under `crates/stapel-cli/assets/`, about 350 test
lines, and the report. 9 Plan steps; split past 300 source lines. Route: full (tag `interface`), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-19: Decision (snapshot, the human, recommended): the method's main at commit `e435a22`
  (2026-11-12). Its requirement ids, `covers` lines, the rule for changed lines and the Set,
  Dependencies and Closure checks match plan items 4.2 to 4.4. The report for the tag v7.0.0 compares
  with this commit. Rejected: the tag v6.12.4 (a maintenance branch of v6, nearer to a PRD than to a
  breakdown); main as the base plus the PRD rubric of v6.12.4 (two sources); waiting for v7.0.0.
- 2026-11-19: Decision (form, the human, recommended): a separate skill `stapel-elaborate` with
  `SKILL.md`, `template.md`, `checklist.md` and `NOTICE.md`. Rejected: a section of the skill `stapel`
  (one file that grows with every ticket).
- 2026-11-19: Decision (output, the human, recommended): the skill writes a draft file
  `.stapel/features/<slug>.md` that no stapel command reads; 4.2 turns it into a ticket of kind
  `feature`. Rejected: text in the chat only; an ordinary ticket (inside the promise of 4.2).
- 2026-11-19: Decision (report, the human, recommended): this ticket writes the first report as a
  document under `docs/method-reports/`; later reports are written by hand. Rejected: a stapel command
  that compares two snapshots; no report in this ticket.
- 2026-11-19: Assumption (orchestrator): the statement in `NOTICE.md` says "adapted from", not
  "compatible with", because stapel reads none of the method's files; the method's TRADEMARK.md
  allows "inspired by" wording and a link to the repository.
- 2026-11-19: Assumption (orchestrator): `template.md`, `checklist.md` and `NOTICE.md` carry the marker
  as their first line, an HTML comment `<!-- stapel: <version> sha256:<hash> -->`, with the hash over
  the file without that line, as the CI workflow of STP-33 does with a YAML comment.
- 2026-11-19: Assumption (orchestrator): from the ledger of STP-33, a local `stapel check` on this
  machine takes about 30 minutes and needs a case-sensitive TMPDIR on a disk image; a fix that lies only
  in test files goes to main as a commit of its own, never into this ticket's range.

## Spec

Terms:

- **Elaboration files**: `.claude/skills/stapel-elaborate/SKILL.md`, `template.md`, `checklist.md` and
  `NOTICE.md` in the same folder.
- **Line marker**: the first line `<!-- stapel: <version> sha256:<hex> -->`, where `<hex>` is the SHA-256
  of the file's bytes after that line. `SKILL.md` carries the frontmatter marker of the skill `stapel`
  (STP-22) instead. A file is **stapel's** when its marker is valid and its hash matches.
- **The method's name**: the text `bmad` in any letter case.
- **Collapsed body**: the text after the frontmatter with every run of whitespace made one space, as
  `init_process::collapsed` makes it.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel init` runs and an elaboration file is missing THE command writes it and prints `created: <path>`; `SKILL.md` has the frontmatter lines `name: stapel-elaborate`, a quoted `description:` and the marker; the other three have a valid line marker. | `init_elaborate::creates_the_files` |
| AC-2 | WHEN `stapel init` runs again and an elaboration file is stapel's of a version not greater than the binary's THE command writes the new text and prints `updated: <path>` when the text differs, and prints nothing for that file when it is equal. | `init_elaborate::updates_an_unchanged_file` |
| AC-3 | IF an elaboration file is edited, has no valid marker, is a symlink or is not UTF-8 THEN `init` keeps its bytes and prints the kept warning of STP-22 for that path; IF it is stapel's of a newer version THEN `init` keeps it with the newer-version warning. | `init_elaborate::keeps_an_edited_file`, `init_elaborate::keeps_a_newer_file` |
| AC-4 | IF a directory stands at the path of an elaboration file THEN `init` exits 1 with `<path> is a directory; nothing was written`; IF a file or a symlink stands at the path of the folder `.claude/skills/stapel-elaborate` THEN `init` exits 1 with `<path> is not a directory; nothing was written`; in both cases it writes no file. | `init_elaborate::refuses_a_directory` |
| AC-5 | WHEN `stapel init` runs in the local mode THE checklist's line is `git add -f .claude/skills/stapel .claude/skills/stapel-elaborate .claude/agents/stapel-*.md .claude/settings.json`. | `init_elaborate::local_checklist_names_the_folder` |
| AC-6 | WHEN the skill is written THE collapsed body of `SKILL.md` holds each of: `larger than one ticket`; `elicit the person's intent, never author it`; `at most four questions in one dialog, each with options and a recommendation`; `.stapel/features/<slug>.md`; `without the first line of template.md`; `run checklist.md over the draft`; `name the path of the draft in the chat`; `.claude/skills/stapel-elaborate/`. | `init_elaborate::skill_text_holds_the_steps` |
| AC-7 | WHEN the skill is written THE first line of the body of `template.md` is `# [Title of the feature]`, and its section headings are, in order: Description, Outcome, Done when, Boundaries, Requirements, Constraints, Decisions, Breakdown; its Requirements example lines start with `R1` and `R2`, its Breakdown line names `covers` and `after`, and the template holds the lines `three to six checks`, `at least one thing out` and `never reused or renumbered`. | `init_elaborate::template_sections` |
| AC-8 | WHEN the skill is written THE check lines of `checklist.md` start with `- EL-<n>:`, the ids increase down the file and never repeat (a gap left by a pruned check is allowed), and each group heading (Draft, Requirements, Breakdown, Dependencies) has at least one check. | `init_elaborate::checklist_ids` |
| AC-9 | WHEN the skill is written THE body of `NOTICE.md` equals the golden file `elaborate_notice.md`: the statement that the texts are adapted from the method, the repository URL, the full commit id of `e435a22`, a table of each stapel file and its source files, and the method's `LICENSE` file at that commit word for word (the MIT copyright and permission notice and its trademark notice). | `init_elaborate::notice_text` |
| AC-10 | WHEN `stapel init` writes its files THE method's name appears in `NOTICE.md` and in no other file it writes, in the shared and the local mode. | `init_elaborate::method_name_only_in_notice` |
| AC-11 | WHEN the skill `stapel` is written THE body has one line that names the skill `stapel-elaborate` for work larger than one ticket. | `init_elaborate::stapel_skill_names_it` |
| AC-12 | WHEN the tests of this repository run THE four elaboration files exist in this repository, and each equals what `init` writes. | `init_elaborate::this_repository_has_the_files` |
| AC-13 | WHEN the tests of this repository run THE report `docs/method-reports/e435a22.md` has the headings Taken, Not taken and Ideas for later, and each list item under Taken and Not taken has a `Why:` part. | `init_elaborate::method_report` |
| AC-14 | WHEN `stapel export` writes the export branch THE paths under `.claude/skills/stapel-elaborate/` are removed from its commits and taken from the Onto tree, as the paths under `.claude/skills/stapel/` are. | `export::removes_the_elaboration_skill` |

### Guarantees

- **Promised:** the Frame's promise items 1 to 8: item 1 by AC-1 to AC-5 and AC-14, item 2 by AC-6, item
  3 by AC-7, item 4 by AC-8, item 5 by AC-9 and AC-10, item 6 by AC-11, item 7 by AC-13, item 8 by AC-12.
  Unchanged: every other file and line `init` writes, except the checklist line of AC-5 and the line of
  AC-11; the exit codes of `init` except AC-4; every other stapel command, and `stapel export` except
  AC-14.
- **Not promised:** the Frame's list; the quality of the ported texts beyond the checked lines (the spec
  review and the code review read them); a feature draft that does not follow the template; a skill
  folder that Claude Code does not load.

## Test plan

CLI tests in the new file `crates/stapel-cli/tests/init_elaborate.rs` with `common::git_repo`,
`common::stapel` and `common::read`; the marker checks reuse the rules of `init_process::check_marker`
(frontmatter) and of `init_workflow` (first line). Golden file
`crates/stapel-cli/tests/golden/elaborate_notice.md`. Light tests: `stapel cargo test -j 4 -p stapel-cli
--test init_elaborate -- --test-threads=4` (and `--test init_process`, `--test init`, `--test
init_local`, `--test init_workflow`, and `--test export` in step 3a); before each GREEN also `--test ticket_drift`. Every GREEN that changes an asset also
runs `stapel cargo run -p stapel-cli -- init` in the worktree, so the installed files of this repository
stay equal to the assets and `init_process` passes at every step.

| Category | Cases | Tests |
|---|---|---|
| Create | a fresh repository: four `created:` lines; frontmatter of `SKILL.md`; line markers valid | `init_elaborate::creates_the_files` |
| Re-run | a second run: no line for the four files; an older stapel version of each file: `updated:` | `init_elaborate::updates_an_unchanged_file` |
| Kept | a line added; the marker removed; a wrong hash; an upper-case or a short hash; a version `1.2`; no closing `-->`; the marker on the second line; a symlink to stapel's text; non-UTF-8 bytes: kept, warning, bytes unchanged | `init_elaborate::keeps_an_edited_file` |
| Newer | marker version `99.0.0` with a valid hash: kept, warning names `99.0.0` | `init_elaborate::keeps_a_newer_file` |
| Directory | a folder at each of the four paths in turn; a file and a symlink at the folder path: exit 1, the AC-4 text, no file written | `init_elaborate::refuses_a_directory` |
| Local | `init --local`: the checklist line | `init_elaborate::local_checklist_names_the_folder` |
| Skill text | each of the eight AC-6 phrases in the collapsed body | `init_elaborate::skill_text_holds_the_steps` |
| Template | the title line first; headings in order; `R1`, `R2`; `covers`, `after`; the three guidance lines | `init_elaborate::template_sections` |
| Checklist | ids increase and never repeat; four groups, each with a check | `init_elaborate::checklist_ids` |
| Notice | body equals the golden file | `init_elaborate::notice_text` |
| Name | every file `init` writes in the shared and the local mode, case-insensitive search: found in `NOTICE.md` (so the test fails at the RED on the first text), found nowhere else | `init_elaborate::method_name_only_in_notice` |
| Stapel skill | one line names `stapel-elaborate` | `init_elaborate::stapel_skill_names_it` |
| This repository | the four files against a fresh `init` | `init_elaborate::this_repository_has_the_files` |
| Report | the three headings; a `Why:` part in each item under Taken and Not taken | `init_elaborate::method_report` |
| Export | a ticket commit that adds `.claude/skills/stapel-elaborate/SKILL.md`; an Onto tree that has the folder: the export commits do not change it, the export tree has the Onto's entries; a person's `.claude/skills/stapel-foo/SKILL.md` stays in the export commits | `export::removes_the_elaboration_skill` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| An elaboration file | file | 0 bytes / any | missing: written | edited, no marker, wrong hash, symlink, non-UTF-8, newer: kept; directory: AC-4 | exact bytes | `init_elaborate::keeps_an_edited_file`, `init_elaborate::refuses_a_directory` |
| The folder `.claude/skills/stapel-elaborate` | folder | — | missing: created | a file or a symlink at its path: AC-4 | — | `init_elaborate::refuses_a_directory` |
| `[repository] history` / `--local` | `shared`, `local` | — | absent: shared | — | exact | `init_elaborate::local_checklist_names_the_folder` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| claude-code | a second skill folder under `.claude/skills/`; its frontmatter `name` and `description` | written as the skill `stapel` is; loading is Claude Code's | `init_elaborate::creates_the_files` |
| claude-code | permission mode, hook environment, transcripts, settings levels, tool call results | not read by the new code | — |
| git | the local mode: the folder must be added with `git add -f` | AC-5 | `init_elaborate::local_checklist_names_the_folder` |
| git | the export branch of the local mode, built from the ticket's commits onto the Onto tree | the folder is a process path (AC-14) | `export::removes_the_elaboration_skill` |
| git | history shape, object format, worktrees, submodules, paths, dirty tree, configuration | not read by the new code | — |
| os | a directory at a file path | AC-4 | `init_elaborate::refuses_a_directory` |
| os | a symlink at a file path | kept as not stapel's | `init_elaborate::keeps_an_edited_file` |
| os | case-insensitive file systems; permissions | not promised | — |

**Review Focus:** a source text of the method that still names its scripts, personas or tracker stores
(the code review reads the asset texts; no test); an HTML comment marker that a markdown viewer hides, so
a person edits the file without seeing the marker (kept as not stapel's on the next run; not tested);
CRLF line ends in an edited file (read as not stapel's; not tested); a person's own skill folder
`stapel-elaborate` (each own file kept with a warning, AC-3; the missing files written).

## Design

### Facts

- F-1: `init.rs::plan_create_text` writes a process file when nothing is at its path, updates it when the
  marker reader it gets finds a valid marker of a version not greater than the binary's, and keeps it
  with a warning otherwise; the process files pass `unchanged_version` (frontmatter marker), the CI
  workflow passes `unchanged_workflow_version` (first-line YAML comment).
- F-2: `init.rs::plan_process_files` refuses a directory at a process file's path before anything is
  written.
- F-3: `init.rs::LOCAL_CHECKLIST` names `.claude/skills/stapel` in its `git add -f` line.
- F-5: `export.rs` names the process paths in three places: `REMOVE`, `is_process_path` (prefix
  `.claude/skills/stapel/` with the slash) and `onto_entries`; none matches `.claude/skills/stapel-elaborate/`.
- F-4: the method's texts at `e435a22`: the PM agent's `assets/template-prd-light.md`; the PO agent's
  `assets/ticket-template.md`, `references/spec.md` (the eight rules of a spec, `R<n>` ids, typed lines),
  `references/slice.md` (epic into stories, tracer bullet) and `references/validate-checks.md` (Set,
  Dependencies, Closure); the copyright line is `Copyright (c) 2025 BMad Code, LLC`.

### Decisions (before the build)

- The four assets live in `crates/stapel-cli/assets/elaborate/`; `init.rs` adds them to
  `plan_process_files`, `SKILL.md` through `process_text` and the other three through a first-line
  marker function that shares the version and hash rules of the workflow marker.
- The asset texts are written in English by the rules of this repository (short sentences, one term per
  meaning) and adapted, not copied: no persona, no script, no tracker store, no template engine.
- The report is `docs/method-reports/e435a22.md`.
- 2026-11-19, agent: the draft is written without the marker line of `template.md` (SR1-3); a pruned
  checklist id leaves a gap and is never reused (SR1-4); a file or a symlink at the folder path is
  refused before anything is written, as STP-33 refuses one at `.github` (SR1-8).

### Decisions (as built)

- `init.rs::plan_elaboration_files` plans the four files through `plan_create_text`: `SKILL.md` with
  `process_text` and `unchanged_version`, the other three with a line marker and the new reader
  `line_marker_version` (a valid version, a lower-case 64-hex hash that matches the bytes after the first
  line, the closing ` -->`). `plan_process_files` calls it last; the four file paths join its directory
  refusal, and a file or a symlink at `.claude/skills/stapel-elaborate` refuses with
  `<path> is not a directory; nothing was written` before anything is written.
- The assets are `crates/stapel-cli/assets/elaborate/skill.md`, `template.md`, `checklist.md` and
  `notice.md`. The three files with a line marker start with one blank line after the marker;
  `init_elaborate::notice_text` drops that line before it compares with the golden file.
- `checklist.md` holds EL-1 to EL-25: Draft, Requirements, Breakdown and Dependencies.
- The line of the skill `stapel` (AC-11) is the first item of its loop list, before `new-ticket`.
- `export.rs`: `REMOVE`, `is_process_path` and the pathspecs of `onto_entries` name
  `.claude/skills/stapel-elaborate`; the copy of `is_process_path` in `tests/export.rs` does too.
- The builders could not read the method's source files (outside the worktree); the orchestrator gave
  the content to adapt in the step requests, and the NOTICE text word for word.
- 2026-11-19, agent: `init_local::foreign_repository_end_to_end` failed from the step 1 GREEN to the step
  3a GREEN, because `stapel export` did not yet remove the new folder; the step order was kept, since
  `stapel check` runs the suite at HEAD only and the export change in step 1 would make the step 3a RED
  pass. Rejected: the export change in step 1; step 3a before step 1.
- The ticket lint failed at the RED of step 7, because the ticket names `docs/method-reports/` before
  the GREEN writes the report; it passes at the GREEN.
- The local checks ran with `TMPDIR` on a case-sensitive disk image (ledger 2026-11-19 of STP-33).

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `init_elaborate::creates_the_files` |
| AC-2 | `init_elaborate::updates_an_unchanged_file` |
| AC-3 | `init_elaborate::keeps_an_edited_file`, `init_elaborate::keeps_a_newer_file` |
| AC-4 | `init_elaborate::refuses_a_directory` |
| AC-5 | `init_elaborate::local_checklist_names_the_folder` |
| AC-6 | `init_elaborate::skill_text_holds_the_steps` |
| AC-7 | `init_elaborate::template_sections` |
| AC-8 | `init_elaborate::checklist_ids` |
| AC-9 | `init_elaborate::notice_text` |
| AC-10 | `init_elaborate::method_name_only_in_notice` |
| AC-11 | `init_elaborate::stapel_skill_names_it` |
| AC-12 | `init_elaborate::this_repository_has_the_files` |
| AC-13 | `init_elaborate::method_report` |
| AC-14 | `export::removes_the_elaboration_skill` |

## Plan

Route: full (tag `interface`), auto.

1. Create the files (AC-1, AC-12; tests `init_elaborate::creates_the_files`,
   `init_elaborate::this_repository_has_the_files`, which fail at the RED because no file is written):
   the new file `tests/init_elaborate.rs` comes with this RED; the GREEN adds the four assets with short
   first texts, writes each with its marker only when nothing is at its path, silently on a later run and
   never rewriting (knowingly wrong until step 2), and runs `init` in the worktree. The RED also adds
   the folder to the `git add -f` list of `init_local::foreign_repository_real_pushes`, which fails there
   because the path does not exist yet. Must not change: the other process files.
2. Re-run and kept files (AC-2, AC-3; tests `init_elaborate::updates_an_unchanged_file`,
   `init_elaborate::keeps_an_edited_file`, `init_elaborate::keeps_a_newer_file`, which fail at the RED
   because step 1 never rewrites and prints no warning): the GREEN moves the files into
   `plan_process_files` with the first-line marker reader. Must not change: the process-file rules.
2a. Refusals (AC-4; test `init_elaborate::refuses_a_directory`, which fails at the RED because a file or
   a symlink at the folder path is not checked before the write): the GREEN checks the folder path with
   the other refusals. Must not change: step 2.
3. Local checklist (AC-5; test `init_elaborate::local_checklist_names_the_folder`, which fails at the
   RED because the line lacks the folder): the RED also changes the line in the golden file
   `init_local_checklist.txt`, so `init_local::prints_the_checklist` fails there too; the GREEN changes
   `LOCAL_CHECKLIST`. Must not change: the other checklist lines.
3a. Export (AC-14; test `export::removes_the_elaboration_skill`, which fails at the RED because the
   folder is no process path of `export.rs`): the RED extends the `is_process_path` copy of
   `tests/export.rs`; the GREEN adds the folder to `REMOVE`, `is_process_path` and `onto_entries`. Must not
   change: the other process paths; a person's other skill folders.
4. The skill texts (AC-6, AC-11; tests `init_elaborate::skill_text_holds_the_steps`,
   `init_elaborate::stapel_skill_names_it`): the GREEN writes the body of `SKILL.md` and the line of the
   skill `stapel`, and runs `init` in the worktree. Must not change: the other lines of the skill
   `stapel`.
5. The template and the checklist (AC-7, AC-8; tests `init_elaborate::template_sections`,
   `init_elaborate::checklist_ids`); the GREEN runs `init` in the worktree. Must not change: `SKILL.md`.
6. The notice and the name rule (AC-9, AC-10; tests `init_elaborate::notice_text`,
   `init_elaborate::method_name_only_in_notice`): the golden file comes with the RED; the GREEN runs
   `init` in the worktree. Must not change: the other three files.
7. The report (AC-13; test `init_elaborate::method_report`): the GREEN writes
   `docs/method-reports/e435a22.md`. Must not change: the installed files.

Fix steps after code review round 1 (findings in `findings.jsonl`):

8. The trademark notice (X1-2; AC-9; test `init_elaborate::notice_text`, which fails at the RED because
   the RED adds the six lines of the `TRADEMARK NOTICE` paragraph of the method's `LICENSE` to the golden
   file): the GREEN adds them to the asset `notice.md` and runs `init` in the worktree. Must not change:
   the other lines of `NOTICE.md`.
9. The skill text (R1-1, R1-2; AC-6; test `init_elaborate::skill_text_holds_the_steps`, which fails at
   the RED on the folder phrase): the GREEN says in `SKILL.md` where the two files are, that `None` is
   allowed only in Constraints, and that the slug comes from the title line of the draft; runs `init` in
   the worktree. Must not change: `template.md`, `checklist.md`.
9a. The template (X1-3, X1-4, X1-5; AC-7; test `init_elaborate::template_sections`, which fails at the
   RED on the title line): the GREEN starts `template.md` with the title line, makes the Breakdown example
   a plain list line, writes `the person` for `the source` and drops the idiom; corrects the report's
   introduction on the Closure check and the stale doc comment of `plan_process_files`; runs `init` in the
   worktree. Must not change: the checks of `checklist.md`.

## Review

TODO

## Summary

TODO
