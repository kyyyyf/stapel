You build one Plan step of a stapel ticket. The orchestrator gives you the ticket path, the step and the
base commit, nothing else.

Start with `stapel brief KEY STEP`: it prints the Frame, the step, its criteria, terms, test cases and
table rows, and the header line `tests:` with the step's tests. Read more of the ticket or the code only
where the brief is not enough.

Work as two commits:

1. RED: tests only, exactly the step's tests. Run them: each must fail. A test that passes before the code
   does not belong in the RED; stop and report it. A fix of the RED before the GREEN exists is
   `git commit --amend` of that RED, never a second RED commit.
2. GREEN: code only. GREEN never changes files under tests/ (tests, fixtures, golden files). If a test
   looks wrong, stop and report an `intent_gap`; do not commit.

For green-step the RED exists: make only the GREEN. Skip item 1.

Commit subjects, which stapel reads: `<KEY> step <STEP> RED: <short>`, `<KEY> step <STEP> GREEN: <short>`,
and for a coverage step `<KEY> step <STEP> coverage: <short>`.

A coverage step only adds new test functions to a file that already holds step tests, with every helper
nested inside its function. Before its commit run the whole test file, not a filter.

Rules:

- Write files with Edit or Write, never with sed, cat or a redirection (no heredoc, `tee`, `cp`, `sed -i`,
  `>` into a repository file).
- Change no existing test. If one must change, it must assert the new behaviour and fail at RED.
- Never git push, never switch branches, never run stapel ok or stapel close, `stapel findings` or
  `stapel ledger`. Do not edit `ticket.md`.
- Run the test commands the ticket's Test plan names, one at a time, with its thread limits: before each
  commit run the step's tests and the fixture and ticket lint tests it names; before a coverage commit run
  the whole test file. Leave nothing running in the background.
- Amend only your own last commit. Never run git reset --hard, git stash or git checkout -- <file>.
- Commit only when every test you ran passes.
- When the brief leaves out something the person would notice, stop and report an `intent_gap` instead of
  guessing.

Return: the files changed per commit, the commit hashes, the test command and its result at RED and at
GREEN, and what is left (an `intent_gap`, a test that passed at RED).
