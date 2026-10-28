# STP-24 — Review start and the review pack

## Description

Source: `docs/PHASES.md`, plan item 1.5, part 2 (split by the ledger decision of 2026-10-22);
`docs/decisions/0002-build-and-review-from-bmad.md`, items 1 to 3 and 7 (label STP-8).

### Frame

**Problem.** Before each code review round the orchestrator makes the `git archive` copy, the diff and the
reviewer prompts by hand and picks the reviewers from memory (STP-23: three reviewers, two rounds). A copy
of the wrong commit or a missing reviewer goes unnoticed.

**Plan item:** 1.5 (part 2).

**Risk tags:** `interface` (a new command, a changed `next` action).

**Non-goals.** Running the reviewers (the orchestrator starts the agents); the import and the triage (part
1, STP-23); the MR review map (item 1.7); a new agent file for the external reviewer (ledger 2026-10-22);
review of uncommitted changes; a third round.

**Promise (closed list).**

1. `stapel review start KEY --round N` refuses an unknown or closed ticket, a dirty tree, a round other
   than 1 or 2, and a round 2 before round 1 has code review findings.
2. It writes a copy of HEAD with the content of `git archive` to `.stapel/scratch/review/<KEY>-r<N>/tree/`; a re-run of the
   same round replaces the copy.
3. It writes `diff.patch` (the diff `base..HEAD`) and `pack.md` beside the copy: base and HEAD commits,
   the stat of the diff, the paths of the copy, the ticket and the diff, every criterion, the three review
   checks of decision 0002; round 2 also lists the round 1 findings with the fate `patch`.
4. It prints one line per reviewer by route and tags: light or untagged gives `stapel-reviewer`; tagged
   gives `stapel-reviewer`, `stapel-reviewer` with the model of the role `external`, and `stapel-drift` on
   the whole ticket.
5. `stapel next` gives `code-review` with the command `stapel review start KEY --round N`: round 1 first,
   round 2 when round 1 has a finding with the fate `patch` and round 2 has not run, else the ticket path
   as today.
6. The skill and the reviewer agent text name the pack.
7. `review start` removes the review folders of the ticket's other rounds and of closed tickets.

Not promised: a copy that stays read-only for a careless agent outside Claude Code tools; diffs of binary
files; a base commit that is no longer in the history.

**Size.** About 300 source lines and 700 test lines; split past 600. Route: full (tag), auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-10-22: Decision (external reviewer, the human, recommended; ledger): the agent `stapel-reviewer`
  with the model of the role `external`. Rejected: a fifth agent file.
- 2026-10-27: Decision (copy place, the human, recommended): `.stapel/scratch/review/<KEY>-r<N>/`, inside
  the repository and ignored by git, so a reviewer's reads need no dialog. Rejected: the system temporary
  folder.
- 2026-10-27: Decision (round 2 pack, the human, recommended): every criterion of the ticket and the round 1
  findings with the fate `patch`. Rejected: only the criteria the fixes touch.
- 2026-10-27: Decision (drift reviewer, the human, recommended): a tagged ticket gets `stapel-drift` on the
  whole ticket as its third code reviewer, as in STP-23. Rejected: drift only after GREEN steps.
- 2026-10-27: Decision (round number, the human, recommended): `next` takes the highest `round` of the
  `code-review` records in `findings.jsonl` plus one; `review start` writes nothing to `state.json`.
  Narrowed by the decisions "round 2 in next" and "round run" below.
  Rejected: a round field in `state.json`.
- 2026-10-27: Decision (copy method, the human, recommended): `git read-tree HEAD` into a temporary index
  and `git checkout-index --prefix`; the content of `git archive` with no new dependency and no external
  `tar`; the main index stays. Rejected: `git archive | tar`; the `tar` crate.
- 2026-10-27: Decision (diff file, the human, recommended): the diff goes to `diff.patch` beside `pack.md`;
  the pack holds its stat and path. Rejected: the diff inside `pack.md`.
- 2026-10-27: Decision (round 2 in next, the human, recommended): `next` names round 2 only when round 1
  has a finding with the fate `patch`; else, and after round 2, the ticket path. Rejected: always the next
  round until the Review section is written.
- 2026-10-27: Decision (round run, the orchestrator, spec review SR1-1): a round also counts as run when
  its review folder holds `pack.md`, so a clean round ends the naming of that round.
- 2026-10-27: Decision (old copies, the human, SR1-7): `review start` removes the review folders of the
  ticket's other rounds and of closed tickets, so copies do not pile up. Rejected: defer to `close`.
- 2026-10-28: Decision (ignore file, the human, recommended; B-4): `review start` writes one
  `.stapel/scratch/review/.gitignore` with `*`, because `stapel init` does not ignore `.stapel/scratch/` in
  other repositories. Rejected: a `.gitignore` in each round folder; an `init` change.

## Spec

Terms:

- **Review folder**: `.stapel/scratch/review/<KEY>-r<N>/`, with `<KEY>` as in `state.json`; ignored by git.
- **Tags**: the backtick spans equal to `security`, `guard`, `data`, `migration` or `interface` in the
  paragraph of the Frame that starts with `**Risk tags:**`, up to the next blank line or heading line.
- **Light**: the Plan's `Route:` line starts with `light` (`next::is_light`).
- **Code-review round** of a ticket: the highest of the whole rounds (STP-23) of the finding records with
  stage `code-review` in `findings.jsonl` and the rounds whose review folder holds `pack.md`; 0 when there
  is none.
- **Patch finding**: a finding record with stage `code-review` and whole round 1 whose current state
  (STP-23) has the fate `patch` and is not false.

| № | Criterion | Test |
|---|---|---|
| AC-1 | IF `stapel review start KEY --round N` names an unknown ticket, a closed ticket or a round other than 1 or 2, or the tree has uncommitted changes (as `next` counts them) THEN exit 2 with the text of the cases table and nothing written; a legacy (STP-1) ticket, a missing base or a base not on the first-parent history of HEAD exit 1; the first case in the table's order wins. | `review::refuses_tickets`, `review::refuses_a_dirty_tree`, `review::refuses_bad_rounds`, `review::refuses_a_bad_base`, `review::refusal_order`, `review::refusal_order_of_the_later_cases` |
| AC-2 | IF `--round 2` runs while the ticket has no finding record with stage `code-review` and whole round 1 THEN exit 2 with `stapel: round 2 needs the code review findings of round 1 in .stapel/tickets/<KEY>/findings.jsonl` and nothing written. | `review::round_two_needs_round_one` |
| AC-3 | WHEN `review start` runs THE review folder is removed if present and gets `tree/`, the files of HEAD as a checkout with `core.autocrlf=false` writes them (the `.gitattributes` of HEAD apply), with their modes and no `.git`, while the repository's index, working tree and refs stay unchanged; `.stapel/scratch/review/.gitignore` holds `*` so that the copies never make the tree dirty. | `review::copies_head`, `review::rerun_replaces_the_copy`, `review::copy_keeps_the_index`, `review::copy_ignores_autocrlf`, `review::copy_needs_no_tar` |
| AC-4 | WHEN `review start` runs THE review folder gets `diff.patch`, the output of `git -c core.quotepath=false diff --no-color --no-ext-diff --no-textconv --find-renames --src-prefix=a/ --dst-prefix=b/ base..HEAD`, and `pack.md` written last: the key, round, base and HEAD commits, the paths of the copy, the ticket in the copy and `diff.patch`, the stat of the same diff with `--stat=120 --stat-graph-width=20`, the criteria rows of the Spec and the three review checks of decision 0002, as the golden file of round 1. | `review::pack_of_round_one`, `review::diff_file` |
| AC-5 | WHEN `--round 2` runs THE pack also lists every patch finding of the working tree's `findings.jsonl` as `- <id> (<current severity>): <claim>` in file order, or `- none`, as the golden file of round 2. | `review::pack_of_round_two` |
| AC-6 | WHEN `review start` succeeds THE output is `copy: <folder>tree (HEAD <12 hex>)`, `pack: <folder>pack.md`, then one line `reviewer: <agent>, model <model> (role <role>)` per reviewer: for a light ticket or one without tags only `stapel-reviewer` (role `reviewer`); for a tagged one also `stapel-reviewer` (role `external`) and `stapel-drift` (role `drift`) followed by `, on the whole ticket`; a role absent from `[models]` prints `no model` for `model <model>`. | `review::reviewers_by_route_and_tags`, `review::reviewer_without_a_model` |
| AC-7 | WHEN `next` would give `code-review` and the code-review round is 0 THE target is the command `stapel review start <KEY> --round 1`; when it is 1 and a patch finding exists, `stapel review start <KEY> --round 2`; otherwise the ticket path, as before. | `review::next_names_round_one`, `review::next_names_round_two`, `review::next_after_the_rounds`, `review::next_after_a_clean_round` |
| AC-8 | WHEN `init` writes the skill and the reviewer agent THE skill holds a line that starts ``- `code-review`:`` and holds `stapel review start KEY --round N` and `pack.md`, and the reviewer body holds `Read pack.md of stapel review start first; read files only under the copy it names.` | `init_process::skill_text_names_the_review_pack` |
| AC-9 | WHEN `review start` runs THE folders directly under `.stapel/scratch/review/` named `<KEY>-r<n>`, with `<n>` a whole number, of the same ticket and another round or of a closed ticket are removed without following symlinks; other open tickets' folders and other names stay. | `review::removes_old_copies` |

Cases table of the refusals, in the order they are checked (exit 2 unless named):

| Case | Text after `stapel: ` |
|---|---|
| round not a number | the clap usage error, exit 2 |
| unknown key | `no ticket <KEY> in .stapel/tickets` (as `resolve`) |
| legacy, exit 1 | `ticket <KEY> is in the STP-1 format; it has no review pack` |
| closed | `ticket <KEY> is closed; it has no review round` |
| round 0, 3 or more | `--round must be 1 or 2; after round 2 the person decides` |
| dirty tree | `uncommitted changes: <first five paths>; commit them before a review round` |
| no base, exit 1 | `state.json of <KEY> has no base commit` |
| bad base, exit 1 | the text of `build_inputs::check_base` |
| round 2 without round 1 | the text of AC-2 |

### Guarantees

- **Promised:** the Frame's promise, by AC-1 to AC-9; outside `.stapel/scratch/review/` `review start`
  writes nothing; `pack.md` is written last, so a folder without it is incomplete; the other actions of `next` and their order are unchanged.
- **Not promised:** the Frame's list; the copy's own `findings.jsonl` is that of HEAD and may be older
  than the working tree's; the round 2 refusal reads legacy records like new ones and skips damaged
  lines, as `next` does (D1-5); submodules (a gitlink gives no files); symlinks in
  HEAD that point outside the copy; `.gitattributes` `export-ignore` (the copy is a checkout, not an
  archive); a Claude Code allow rule for `review start`; two `review start` runs at once.

## Test plan

CLI tests in the new file `crates/stapel-cli/tests/review.rs` with `common::git_repo`, `common::stapel`
and a ticket built as in `tests/next.rs` (Frame, Spec with two AC rows, Plan, a base commit and one later
commit). The golden files are `crates/stapel-cli/tests/golden/review_pack_r1.md` and
`review_pack_r2.md`; a test replaces the base and HEAD ids with `<base>` and `<head>` before it compares.
AC-8 adds one function to `tests/init_process.rs`. Light tests: `cargo test -j 4 -p stapel-cli --test
review -- --test-threads=3` (and `--test next`, `--test init_process`); before each GREEN also `--test
ticket_drift`.

| Category | Cases | Tests |
|---|---|---|
| Tickets | `ABC-9` unknown: exit 2, the `resolve` text; a closed ticket: exit 2, the closed text; a legacy `state.json`: exit 1, the legacy text; no review folder after each | `review::refuses_tickets` |
| Dirty tree | an untracked file, then a staged change, then an unstaged change to `ticket.md`: exit 2, the dirty text with the path; an ignored file and a change to `findings.jsonl` (a machine file): accepted | `review::refuses_a_dirty_tree` |
| Rounds | `--round 0`, `--round 3`: exit 2, the round text; `--round x`, `--round -1`, no `--round`: the clap usage error, exit 2; nothing written | `review::refuses_bad_rounds` |
| Refusal order | a closed ticket with `--round 3` and a dirty tree: the closed text; a legacy ticket with a dirty tree: exit 1, the legacy text; round 2 with no findings and a bad base: exit 1, the base text | `review::refusal_order` |
| Later order | an unknown key with `--round 3`: the unknown text; `--round 3` with a dirty tree: the round text; a dirty tree with no base: the dirty text; a dirty tree with a bad base: the dirty text (D1D-1) | `review::refusal_order_of_the_later_cases` |
| Base | `base` removed from `state.json`: exit 1, the no-base text; a base not on the first-parent history: exit 1, the `check_base` text | `review::refuses_a_bad_base` |
| Round 2 needs round 1 | no `findings.jsonl`; only `spec-review` and `step-drift` records; a `code-review` record of round 2 only: exit 2, the AC-2 text; one `code-review` record of round `"1"`: accepted | `review::round_two_needs_round_one` |
| Copy | HEAD with `a b.txt`, `dir/é.rs`, an executable script and an empty file: the same bytes and the executable bit under `tree/`; no `tree/.git`; a file deleted in the last commit is absent | `review::copies_head` |
| No autocrlf | `core.autocrlf=true` in the repository config and a file with LF endings: the copy has LF endings | `review::copy_ignores_autocrlf` |
| No tar | `PATH` holds only a folder with a link to `git`: the copy is made, exit 0 | `review::copy_needs_no_tar` |
| Re-run | a stray file put into `tree/` and a new commit, then the same round again: the stray file is gone, the new commit's files are there | `review::rerun_replaces_the_copy` |
| Index kept | in a repository whose `.gitignore` does not name `.stapel/scratch/`: the bytes of `.git/index`, `git status --porcelain` and `git rev-parse HEAD` before and after: equal; no `index` file left in the review folder; `.stapel/scratch/review/.gitignore` is `*` and a newline, and the round folder has no `.gitignore` of its own | `review::copy_keeps_the_index` |
| Pack, round 1 | an untagged ticket with AC-1 and AC-2: `pack.md` equals `review_pack_r1.md` | `review::pack_of_round_one` |
| Diff | with `diff.external`, `color.diff=always`, `diff.noprefix=true`, `diff.renames=false` and `core.quotepath=true` set in the repository config, and a renamed file `é.txt`: `diff.patch` equals the AC-4 command run by the test with those settings removed | `review::diff_file` |
| Pack, round 2 | round 1 findings R-1 (MEDIUM, triaged `patch`), R-2 (LOW, `defer`), R-3 (HIGH, `patch`, then verdict `false`), R-4 (`patch`, stage `step-drift`): the round 2 list holds only R-1 and equals `review_pack_r2.md`; with no patch finding: `- none` | `review::pack_of_round_two` |
| Reviewers | an untagged ticket: one line; tags `interface` on a wrapped second line of the paragraph: three lines; the word `guard` without backticks only: one line; `Route: light` with a tag: one line; a tags paragraph followed directly by a `**Non-goals.**` line holding `` `data` ``: one line | `review::reviewers_by_route_and_tags` |
| No model | `[models]` without `external`: `reviewer: stapel-reviewer, no model (role external)`, exit 0 | `review::reviewer_without_a_model` |
| Next, round 1 | after the build, check passing, Review unwritten, no findings: `next: code-review — stapel review start ABC-1 --round 1`; `--json` gives it as `command` | `review::next_names_round_one` |
| Next, round 2 | a `code-review` finding of round 1 triaged `patch`: `--round 2` | `review::next_names_round_two` |
| Next after the rounds | round 1 findings all `defer`; a round 2 finding; a round 1 `patch` finding with verdict `false`: the ticket path; the Review section written: `write-summary` | `review::next_after_the_rounds` |
| Next after a clean round | no findings and `pack.md` in `ABC-1-r1`: the ticket path; a round 1 `patch` finding and `pack.md` in `ABC-1-r2`: the ticket path | `review::next_after_a_clean_round` |
| Old copies | folders `ABC-1-r1`, `ABC-2-r1` (ABC-2 closed), `ABC-3-r1` (open), `ABC-2-rx`, `notes` and a symlink `ABC-2-r2` to a folder outside: `review start ABC-1 --round 2` leaves `ABC-1-r2`, `ABC-3-r1`, `ABC-2-rx`, `notes` and the symlink's target; the symlink itself is removed | `review::removes_old_copies` |
| Skill text | the sentences of AC-8 in the files written by `init` | `init_process::skill_text_names_the_review_pack` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `KEY` of `review start` | ticket key, required | — | absent: the clap usage error | unknown or closed: exit 2 | case-insensitive, as `resolve` | `review::refuses_tickets` |
| `--round <n>` of `review start` | whole number, required | 1 / 2 | absent: the clap usage error | 0, 3 and more: exit 2; not a number: clap | exact | `review::refuses_bad_rounds` |
| `**Risk tags:**` paragraph | Frame text | — | absent or no known span: untagged | an unknown span is ignored | exact, case counts | `review::reviewers_by_route_and_tags` |
| `[models]` roles `reviewer`, `external`, `drift` | `stapel.toml` table | — | absent role: `no model` | as `repo::open` | as written | `review::reviewer_without_a_model` |
| `round` of a finding | number or string | as STP-23 | — | not a whole round: not counted | a whole round | `review::round_two_needs_round_one`, `review::next_after_the_rounds` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | history shape | the base must be on the first-parent history of HEAD, else exit 1 | `review::refuses_a_bad_base` |
| git | shallow clone; object format SHA-256 | not promised | — |
| git | detached HEAD; linked worktrees | the copy is of the HEAD of the current worktree; not tested | — |
| git | submodules | not promised (Guarantees) | — |
| git | paths with spaces, non-ASCII names; renames | copied as they are; renames show in `diff.patch` as git prints them | `review::copies_head` |
| git | commit messages | not read | — |
| git | dirty tree | untracked, staged and unstaged refuse; ignored and machine files do not | `review::refuses_a_dirty_tree` |
| git | configuration, `GIT_*` environment | the diff flags of AC-4 override diff, colour, prefix, rename and quoting settings; `core.autocrlf` is false for the copy; `.gitattributes` filters and `eol` apply as on checkout; `GIT_INDEX_FILE` is set only for the copy's own index; other `GIT_*` variables not promised | `review::diff_file`, `review::copy_ignores_autocrlf`, `review::copy_keeps_the_index` |
| claude-code | permission mode; allow rules | no dialog is involved; an allow rule for `review start` is not promised | — |
| claude-code | environment, transcripts, tool results | not read | — |
| cargo | every dimension | not run | — |
| os | file permissions | the executable bit is kept; a write error exits 1 with the path | `review::copies_head` |
| os | symlinks; case-insensitive file systems; processes; time; WSL | not promised | — |

**Review Focus:** a Frame whose `**Risk tags:**` paragraph has no blank line after it (read up to the
next heading line; `review::reviewers_by_route_and_tags` covers a `**Non-goals.**` line, not a `###` heading); a failure in the middle of the copy (no `pack.md` is written; not tested); a removal of the review
folder that follows a symlink (`remove_dir_all` does not follow it; not tested); a diff of tens of
megabytes (written as git prints it; not tested).

**Author self-check:** 2026-10-27, before spec review: every promise item has a criterion and a test (item
1: AC-1, AC-2; item 2: AC-3; item 3: AC-4, AC-5; item 4: AC-6; item 5: AC-7; item 6: AC-8); each input has
its empty and invalid case; the rows were checked against `tickets::resolve`, `worktree::dirty_paths`,
`build_inputs::check_base`, `next::is_light`, `triage::state_of`, `brief.rs` and the catalogue; no open
question is left.
2026-10-27, after spec review round 1 (stapel-spec-reviewer): 10 findings, 0 HIGH, 3 MEDIUM; 9 inside, 1
outside, taken by the human as AC-9 (SR1-7). One change: a round with `pack.md` counts as run (SR1-1); the
copy pins `core.autocrlf` and the diff pins its flags (SR1-3, SR1-4); the refusal order (SR1-5); `pack.md`
last (SR1-6); the step 1 stub (SR1-8); the source of the round 2 list (SR1-9); the tags paragraph ends at a
heading line (SR1-10); a decision narrowed (SR1-2).

## Design

### Facts

- F-0: prior art: the reviewer prompts of STP-22 and STP-23 under `.stapel/scratch/`, the agent text
  `crates/stapel-cli/assets/agents/reviewer.md`; no code runs `git archive` or reads a tar, and
  `stapel-cli` has no tar dependency.
- F-1: `tickets::resolve` gives the key and status; `build_inputs::check_base` checks the base;
  `worktree::dirty_paths` lists dirty paths without ignored and machine files.
- F-2: `next::is_light` reads the route; risk tags are read only by `tests/ticket_drift.rs`, from the first
  line; the AC rows are read by private functions of `brief.rs` (`row_number`, `cells`).
- F-3: `triage::state_of` and `triage::whole_round` give the current state and round of a finding.
- F-4: `next::after_the_steps` gives `code-review` with the ticket path; five tests of `tests/next.rs`
  assert that path (`uncommitted_changes_before_the_check`, `run_check_after_the_steps`,
  `code_review_then_summary`, `review_and_summary_edges`, `index_untouched`).

### Decisions (before the build)

- The new module `crates/stapel-cli/src/review.rs` holds `start`; `crates/stapel-core/src/review.rs` holds the
  pure parts: the tags of a Frame, the reviewer list, the patch findings, the pack text and the round that
  `next` names.
- The copy: `git -c core.autocrlf=false read-tree HEAD` with `GIT_INDEX_FILE` set to `<folder>index`, then
  `git checkout-index -a --prefix=<folder>tree/` with the same variable, then the index file is removed.
- `diff.patch` and the stat: the commands of AC-4; `checkout-index` also runs with `-c core.autocrlf=false`.
- `review start` writes `.stapel/scratch/review/.gitignore` (`*`) before the copy (the human, 2026-10-28,
  B-4).
- Step 1 GREEN: a command that passes every refusal exits 0 and prints and writes nothing; steps 1b to 3
  replace this.
- `brief.rs` makes `row_number` and `cells` public for the criteria rows.
- `next::Facts` gets the code-review round and whether a patch finding exists, computed by the CLI `next`
  from the records it already reads.
- Old tests: in the RED of step 4, the five tests of F-4 change their expected `code-review` line to the
  command, so that they fail at the RED.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `review::refuses_tickets`, `review::refuses_a_dirty_tree`, `review::refuses_bad_rounds`, `review::refuses_a_bad_base`, `review::refusal_order`, `review::refusal_order_of_the_later_cases` |
| AC-2 | `review::round_two_needs_round_one` |
| AC-3 | `review::copies_head`, `review::rerun_replaces_the_copy`, `review::copy_keeps_the_index`, `review::copy_ignores_autocrlf`, `review::copy_needs_no_tar` |
| AC-4 | `review::pack_of_round_one`, `review::diff_file` |
| AC-5 | `review::pack_of_round_two` |
| AC-6 | `review::reviewers_by_route_and_tags`, `review::reviewer_without_a_model` |
| AC-7 | `review::next_names_round_one`, `review::next_names_round_two`, `review::next_after_the_rounds`, `review::next_after_a_clean_round` |
| AC-8 | `init_process::skill_text_names_the_review_pack` |
| AC-9 | `review::removes_old_copies` |

## Plan

Route: full (tag `interface`), auto.

1. `stapel review start KEY --round N` with its refusals (AC-1, AC-2; tests `review::refuses_tickets`,
   `review::refuses_a_dirty_tree`, `review::refuses_bad_rounds`, `review::refuses_a_bad_base`,
   `review::round_two_needs_round_one`, `review::refusal_order`); a success is the stub of Design. Must not
   change: the other commands.
1d. Fix after the drift review of step 1 (D1-1 to D1-4, D1-6; AC-1; tests `review::refuses_tickets`,
   `review::refuses_a_dirty_tree`, `review::refuses_bad_rounds`, `review::refuses_a_bad_base`,
   `review::refusal_order`): the RED makes these tests assert the exact text and exit code of the Spec's
   refusal cases table (under AC-9; not in the brief) in its order; seven dirty paths name five; a base off
   the first-parent history exits 1; `common::snapshot` unchanged. The GREEN changes
   `crates/stapel-cli/src/review.rs` only.
1e. Coverage after the drift review of step 1d (D1D-1; AC-1): new `#[test]` function
   `review::refusal_order_of_the_later_cases`. Tests only, each helper nested inside its function, no
   `cargo fmt`; a test that fails is an `intent_gap`.
1b. The copy of HEAD (AC-3; tests `review::copies_head`, `review::rerun_replaces_the_copy`,
   `review::copy_keeps_the_index`, `review::copy_ignores_autocrlf`). Must not change: the repository's
   index, working tree and refs; step 1.
1f. Fix after step 1b (B-2 to B-4; AC-3; tests `review::copy_needs_no_tar`,
   `review::copy_keeps_the_index`): the copy by a temporary index and `checkout-index` as Design says, not
   `git archive | tar`; one `.gitignore` in `.stapel/scratch/review/`; output only through `out.rs`
   (`closed_pipe::no_print_macros_outside_out`). The RED adds `review::copy_needs_no_tar` and changes
   `review::copy_keeps_the_index` so that both fail there. Must not change: steps 1 to 1e.
1c. Old copies removed (AC-9; test `review::removes_old_copies`). Must not change: steps 1 and 1b.
2. `pack.md` and `diff.patch` (AC-4, AC-5; tests `review::pack_of_round_one`, `review::diff_file`,
   `review::pack_of_round_two`); the golden files come with the RED. Must not change: steps 1 to 1c; the output of
   `brief`.
3. The reviewer lines (AC-6; tests `review::reviewers_by_route_and_tags`,
   `review::reviewer_without_a_model`). Must not change: steps 1 and 2.
4. The `code-review` target of `next` (AC-7; tests `review::next_names_round_one`,
   `review::next_names_round_two`, `review::next_after_the_rounds`, `review::next_after_a_clean_round`); the old tests as in Design. Must not
   change: the other actions of `next` and their order.
5. The skill and reviewer texts (AC-8; test `init_process::skill_text_names_the_review_pack`):
   `crates/stapel-cli/assets/skill.md` and `assets/agents/reviewer.md`; the GREEN runs the built
   `stapel init` here so that `init_process::this_repository_has_the_current_files` holds. Must not change:
   the other sentences that `init_process` tests.

## Review

TODO

## Summary

TODO
