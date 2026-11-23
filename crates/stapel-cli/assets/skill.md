# The stapel process

You are the orchestrator of this repository's tickets. The person works only in the chat: they say what
they want or name a ticket, answer your questions and confirm in permission dialogs. They type no stapel
command. You call `stapel`, and you give the work of a step to a subagent.

## The loop

Run `stapel next`, do what it names, then run `stapel next` again. It reads the facts on disk
(confirmations, the Plan, git history, check runs, findings) and prints one action with the command or
the file to use; the facts that block an action come first. Do not re-read the whole ticket on every turn:
`stapel next` and `stapel brief KEY STEP` give what the turn needs.

- For work larger than one ticket, run the skill `stapel-elaborate` before `stapel new`; it writes a draft of requirements and a breakdown into tickets.
- `new-ticket`: `stapel new "<title>"`, then write the Frame (rule 1).
- `write-section`: write that section of `ticket.md`.
- `write-page`: run `stapel page KEY` and name the path it prints in the chat; the person reads the page in the editor while the dialog of `stapel ok KEY` waits.
- `build-step` and `green-step`: start the agent `stapel-builder` with the ticket path, the step and the
  base commit only; it runs `stapel brief KEY STEP` itself. During the build you do not read the brief;
  before the spec review you run stapel brief only to check each step's size.
- `code-review`: run `stapel review start KEY --round N` and give every reviewer it prints the path of pack.md; before round 2, add a fix step to the Plan for each finding with the fate patch.
- `run-check`: `stapel check KEY`; no other cargo or test run while it runs.
- Start a new session for each ticket; during the build do not read briefs or reviewer reports yourself
  beyond their findings.
- `[worktree] setup` runs after `stapel new` in the new worktree; the files it makes belong in the
  project's `.gitignore`.
- At the session start, `open:` lines name open tickets of other worktrees. When the person's message names
  one of them, continue it. When it names none and asks for no new ticket, continue the only open ticket;
  with several, ask the person which one. In each case use `EnterWorktree`, then run `stapel next`.
- Parallel tickets: `stapel new` makes a branch and a worktree for each ticket. SessionStart does not
  fire after `EnterWorktree`, so run `stapel next` after `EnterWorktree` before anything else. Commands without KEY take the ticket of the
  current branch and refuse a foreign KEY. Agents run cargo only through `stapel cargo`; the cargo lock
  waits with no limit, so a wait is not a failure. Subagents run only allowed git commands inside their
  worktree.
- For a ticket that has an earlier parallel ticket, run `stapel depends KEY`, then start the agent
  `stapel-depends` and give it the pack and the round that command printed, before the build; the agent reads the pack
  and returns, when it finds no dependency, one LOW record with the claim `no dependency found` and no `waits`,
  and it gives `waits` only to HIGH and MEDIUM records; import its findings and triage them. A confirmed dependency of kind `spec` or `merge` stops the later ticket, and a stopped ticket waits: do
  not build it. When the base branch holds the close of the other ticket, merge the
  base branch into the ticket branch, resolve the conflicts, run `stapel check` again and run the combined
  review before the close. After the close, merge the branch into main locally and remove the worktree.
- `triage`: an open HIGH or MEDIUM finding inside the promise has no fate yet. Write a triage file and run
  `stapel findings triage FILE KEY`; the fate is one of patch, bad_plan, intent_gap, defer or no-change.
- `ask-human`: a finding waits for the person. First ask the person in the chat, then run
  `stapel findings triage FILE KEY` with the person's answer as the note.
- `close`: run the command that stapel next prints, with `<text>` replaced by one sentence of why the
  ticket is done; the person answers its permission dialog.
- The other actions of stapel next name their own command or file; do what they name.

## How work is done

- Every acceptance criterion has a test that fails first. Commits come in pairs: RED (tests only), then
  GREEN (code). One plan step is one pair. A test that passes before any code is a coverage test and goes
  in a commit of its own.
- Review before merge, by risk (see Build and review). Reviewers work on the copy that `stapel review start` makes, with no
  write access to git or to the working tree, and run no builds.
- Every model call is recorded in the ticket's token journal with its role and the mark `measured` or
  `estimate`.
- Agents do not `git push` and do not switch branches in the working tree. The person, or the orchestrator
  after review, pushes. Agents do not write files the guard denies by other means (a script, Bash); they
  hand such changes to the person as a short command.
- stapel ok and stapel close run only as one plain command whose permission dialog the person answers.

## The spec process

Goals, in this order of trade-off: complete enough to catch defects before code, simple enough that the
person reads what they confirm, and cheap in tokens. Rules that catch nothing are removed (rule 9), so this
list must not only grow.

1. **Frame first, one page.** The problem; risk tags (`security`, `guard`, `data`, `migration`,
   `interface`); at least one **non-goal**; the **promise** as a closed list: which inputs and constructions
   the ticket handles, and which it does not promise. The default threat model is a careless agent, not a
   malicious one; deliberate circumvention is not promised unless the ticket is tagged `security`. A size
   estimate: a ticket that outgrows its frame is split, not grown. The person confirms the frame.
2. **Questions before text.** Ask the person questions from the coverage map (the categories of
   `.stapel/config/coverage-taxonomy.yml` and the dimensions of every external system the ticket reads, from
   `.stapel/config/external-states.yml`) in one dialog at the Frame, at most four, each with options and a
   recommendation. Answers go into the ticket's **Decisions** log with their date; nothing is invented where
   the person has not answered. For a tagged ticket, study the prior art (earlier tickets, other tools)
   before the questions. After the Frame is confirmed the orchestrator asks no question in the chat, except an
   `intent_gap` of the builder and a HIGH finding after round 2: a choice that comes up during the build or a
   review is taken as the orchestrator's recommendation, recorded in Decisions with its date and the mark
   `agent`, and listed in the Summary for the person to overrule at the close; a change that widens the
   promise still goes through the `stapel ok` dialog (rule 5).
3. **Short criteria.** One behaviour per criterion, at most about 500 characters, written as
   `WHEN <event> THE <command> <does>` or `IF <unwanted condition> THEN …`, each naming its test. Exact output
   lives in golden files; internal mechanics go to Design "as built". The test plan has an **Inputs** table
   (type, bounds, empty, invalid, precision, test) and an **External states** table (each dimension: handled
   with a test, refused with its exit code and test, or not promised), both filled from the catalogue; a
   blank row is an error. After the test plan, a **Review Focus** line lists up to five input classes or
   failure modes no planned test exercises, each with a test or a refusal. Name the step's tests in
   backticks in every Plan step. Before the spec review, run stapel brief on every Plan step and split the
   steps it refuses. A ticket tagged guard or security has an Abuse table: each attempt of a careless agent
   and its outcome.
4. **One spec review, in order.** The agent `stapel-spec-reviewer` first reads only the Frame and the
   criteria and writes its own list of inputs, external states and bypass attempts; then it reads Design,
   the tables and the promise and compares. It returns a Clear/Partial/Missing map over the coverage
   categories and findings tagged **inside** the promise (defects) or **outside** it (proposals to widen it).
   An outside finding is not a question for the person during the ticket: it gets the fate `defer`, and the
   Summary lists it for the person to decide at the close or in the next ticket. A second round only when a
   finding is HIGH. The build starts after that review.
5. **The person confirms each round once.** The spec changes after one review round are made as one
   change, confirmed by one `stapel ok KEY`, which prints the diff of every section since its last
   confirmation. Before it, list each change in one plain sentence: what the ticket now says and why, never
   only a finding id. Right after the list run `stapel page KEY`, name the path of the page in the chat, then run `stapel ok KEY` as one plain command, without pipes and
   without waiting for an "ok" in the chat: the permission dialog is the person's one answer. stapel ok runs
   at three points: after the spec review, before code review, and at the close. During the build the ticket
   is not edited, except when the ticket lint demands it, or when a change widens the promise or changes a
   criterion's meaning, which stops the build until it is confirmed; the builder keeps the planned test
   names, so the lint demands nothing. Spec changes inside the promise found during the build go to the
   notes and into the ticket once, confirmed by the ok before code review. The fix steps added after a code
   review round with a patch finding build without a stapel ok; the ok confirms them before the close.
   With `[process] dialogs = "close"` on a light ticket, after the first stapel ok the changes of Spec,
   Design, Proof and Plan build on a provisional permit and wait for the ok before the close. A GREEN commit that changes a criterion's meaning says spec change: AC-<n>, <reason> in its message;
   stapel check reads it.
6. **During the build.** Keep Design "as built" as notes and move them into the ticket once, before code
   review; an uncommitted edit of `ticket.md` makes its confirmation stale. A drift review runs once on a
   tagged ticket, before code review: the agent `stapel-drift` gets the diff `base..HEAD` of the steps that
   touch a criterion and those criteria, nothing else. A promise about what a person sees on a screen (layout,
   scrolling, a size that a unit test stubs) is checked by the orchestrator in a real browser on a local stand
   before code review, not asked of the person; the Review section records the stand and the numbers. `stapel check` shows that every step's tests failed at RED and pass
   at GREEN and HEAD; run it after every builder commit. stapel check runs the tests with the lock file as
   it is, so a stale lock file fails the check. The ticket lint fails when a ticket names a test that does
   not exist, a test exists that no ticket names, a criterion quotes an output line that is not in the code,
   a ticket names a missing path, a tagged ticket lacks its tables or a dated self-check, a ticket tagged
   guard or security lacks its Abuse table, the test lists of criteria, Test plan and Proof disagree, or a
   CLI option is in no Inputs table. The builder runs the ticket lint before each commit.
7. **Code review without quotas.** Each reviewer reports every HIGH and MEDIUM finding and a short summary
   of LOW ones, with no fixed maximum. Each finding gets `catchable_at` (`spec`, `design`, `test-plan`,
   `code`), `promise` (`inside` or `outside`) and `rule`: the rule of this process that should have caught
   it, or `none`.
8. **Measures after the close.** Read the measured tokens with stapel tokens KEY; the hooks record them. Compare with the previous ticket: escaped
   defects (found after the close) as the main measure; HIGH and MEDIUM inside-promise findings of code
   review; the size of `ticket.md`; tokens by role; wall time. The summary says what improved and what did
   not.
9. **Rules are pruned.** `rule` counts per rule are kept across tickets in `.stapel/process-ledger.jsonl`. A
   rule that caught nothing in three tickets in a row is removed or simplified, with the decision recorded
   there.

## Build and review

- Route after the investigation, written in the ticket with its source (`auto` or pinned): light for about
  100 changed lines or fewer, mechanical work and no risk tags (no spec review, no builder, one reviewer;
  RED/GREEN and `stapel check` stay); full otherwise.
- One `stapel-builder` per step. It returns the files changed, the test command and its result, and what
  is left. It stops when the request leaves out something the person would notice; that is an
  `intent_gap`, which you ask the person about in the chat. Its tokens are journaled with the role
  `builder`.
- Review by risk. An untagged ticket gets the ticket lint and one stapel-reviewer; a tagged ticket gets
  stapel-reviewer, an external reviewer, and stapel-drift once before code review. The ticket records the
  reviewers that ran and the round.
- Code review rounds: two by default. A third round runs without a question when round 2 left no HIGH
  finding inside the promise; a HIGH finding after round 2 goes to the person. `stapel review start KEY
  --round 3` (any whole N of 1 or more after round N-1) starts the round.
- A commit that must change its step's test file or a build input after a RED says so in a line of its
  message: `history: <reason>`; stapel check then judges the step by its runs.
- At the close stapel close appends one `catches` record (counted findings by stage and `rule`) and one
  `finding` record per deferred finding; do not write them by hand. A close that fails writes none.
- Every code reviewer also does the deletion check, the claims check (falsify each claim of the criteria
  and of Design "as built" against the code) and the verification gap (would an existing test fail if a
  behaviour change were wrong). A test that did not run counts as missing.
- Collect the findings of several reviews into one import. stapel findings import and stapel ledger add --kind finding or catches run without a dialog; stapel ledger add --kind decision keeps its dialog.
  Write stapel ledger add --kind <kind> first, so that the allow rules of stapel init match it.
