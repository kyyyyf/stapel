# The test pyramid and the test suites

## Description

Plan item 5.2d with 5.2d-1 and 5.2d-2 (`docs/PLAN.md`, ledger 2026-11-26). When the work is done, every test
of a project that stapel runs in has a kind on two axes: a level (unit, component, library API, system through
the program, e2e with real tools) and a technique (example, golden, property-based). Rules kept as data per
language and runner give the kind, cargo and junit first; a mark in the test overrides them, and a project adds
its own rules. A report prints the count and the run time of each level and each technique. The pyramid is
over the levels. The
project keeps a target pyramid in its `stapel.toml`; a guard keeps the pyramid no worse than its baseline;
the ticket lint and the spec review read the kind of each test in a ticket's Test plan before code. Named
suites say which tests run while a ticket is built, at push, and at a release or on a schedule, and
`stapel init` writes the CI runs for them. The target is proposed at `stapel init`, flagged when it no
longer fits the project, and changed only with the person. It matters because the suite of this repository
is almost only system tests through the program with a fake cargo (about 1378 tests under `tests/`, 6 in
`src/`), has one property-based test and no e2e test, and no rule says which tests run at which point.
This repository is the first pilot; the feature serves every project stapel runs in.

## Outcome

The person and the agents of any project see which kinds of tests the project has and what they cost, and a
ticket cannot make the pyramid worse without a reason the person reads.

## Done when

- The report on this repository prints the count and the run time of each level and each technique, and lists the tests with no kind.
- The report on a TypeScript fixture with the junit runner and a project rule gives each test its kind.
- `stapel check` fails on a fixture whose pyramid got worse than its baseline, and the person can let the ticket go on in a dialog with a recorded reason when the pyramid is the only cause of the failure.
- The ticket lint fails on a Test plan test with no kind, and on a departure from the pyramid with no dated Decision.
- `stapel init` on a fixture proposes a target that the person confirms in a dialog, and writes GitHub Actions runs of the push, release and schedule suites.
- This repository has property-based tests of its parsers and e2e tests with real cargo, and its release suite runs them.

## Boundaries

- Out: CI systems other than GitHub Actions.
- Out: runners other than cargo and junit; a later runner adds rules as data in a ticket of its own.
- Out: test coverage by lines or branches; the pyramid counts tests, not covered code.
- Out: the blast radius of a change and which tests it selects (plan item 5.3).
- Out: a speed limit per level or technique as a check; run time is reported, not checked.
- Must not change: the RED and GREEN runs of the steps in `stapel check`; the suites choose the suite run only.
- Must not change: a project with no target pyramid and no suites in its `stapel.toml` works as before.

## Requirements

R1: Every test of a project gets a kind from rules: one level of unit, component, library API, system or e2e, and one technique of example, golden or property-based; shown by the report listing each test of this repository (cargo) and of a TypeScript fixture (junit) with its level and its technique, where a golden test through the program is a system test with the technique golden.
R2: The kind rules of a language and runner are data shipped with stapel, separate from the code of the runner; shown by the cargo and junit rule sets being data files, and by a test that changes a rule in the data and sees another kind with no code change.
R3: A project adds rules or overrides shipped rules in its `stapel.toml`, and names its components there (a crate, a package or a folder); shown by a fixture where a project rule gives the tests under `tests/guards/` their kind ahead of the shipped rule.
R4: A test that no rule gives a kind is counted as having no kind and is named; shown by a fixture with such a test, which the report counts and lists by name.
R5: The report prints the count and the run time of each level and of each technique; shown by the report on this repository (cargo) and on the junit fixture, where each kind has a count and a time, and a kind's time comes from a run of only that kind's tests where the runner gives no time per test.
R6: A project keeps its target pyramid in its `stapel.toml` as a range of shares per level with a tolerance; shown by a fixture with a valid target that the report compares with the current shares, and by a refusal with an exit code of a target with an unknown level or ranges that no set of shares can meet.
R7: A guard keeps the pyramid no worse than its baseline; shown by `stapel check` failing on a fixture where a level's share moved away from its target range by more than the tolerance against the baseline in the guards folder, and writing a tightened baseline when the pyramid got better.
R8: A distance from the target while the project has not reached it is a warning, not a failure; shown by `stapel check` passing with a warning on a fixture whose shares are outside the target and no worse than the baseline.
R9: A ticket's Test plan names the kind of each test, and a ticket whose planned tests move the pyramid of the project after the ticket away from the target needs a dated Decision with its reason; shown by the ticket lint failing on a Test plan row with no kind, on a kind no rule set knows, and on such a ticket with no such Decision.
R10: The spec review reads the kinds of the Test plan against the pyramid before code; shown by the spec review pack carrying the target, the current shares and the kinds of the ticket's planned tests, and by the spec reviewer's instructions naming the comparison.
R11: A project names suites that say which tests run while a ticket is built, at push, and at a release or on a schedule; shown by a fixture whose suites in `stapel.toml` select tests by kind, where `stapel check` runs the ticket suite as its suite run and a suite run prints which tests it ran.
R12: `stapel init` writes the GitHub Actions runs of the push, release and schedule suites; shown by golden workflow files of an init on a fixture, where the push run also runs the guard of R7.
R13: The release and schedule runs print the full report, e2e and property-based tests included, and the pyramid part of it blocks nothing; shown by a release suite run on a fixture far from its target and worse than its baseline, whose pyramid lines are warnings.
R14: `stapel init`, or the first use of the feature in a project with no target, proposes a target from the kind of project (program, library, service, UI) and the report of its current tests, and the person confirms it in a dialog; shown by a fixture of each kind of project getting its proposed target, written to `stapel.toml` only after the dialog.
R15: The release and schedule report flags a target that no longer fits; shown by fixtures where each of these gives a flag: a growing share of tests with no kind, a level of the target with no new test in the last N closed tickets, and departures of several tickets for the same reason.
R16: The target is reviewed like a rule of the process and changed only by a dated `decision` record made with the person; shown by `stapel next` naming the review on a flag, at a release or every N tickets, and by `stapel check` failing when the target in `stapel.toml` changed with no `decision` record that names it.
R17: This repository is the first pilot: every test has a kind, the target and the baseline are set, the parsers have property-based tests, and a few e2e tests run with real cargo one at a time under the cargo slots; shown by the report on this repository with no test without a kind and with property-based and e2e counts above zero, and by its release suite running the e2e tests.
R18: When the pyramid guard of R7 is the only cause of a failed `stapel check`, the person can let the ticket go on in a dialog, with a reason; shown by `stapel next` naming this choice, by a dialog that records a dated `decision` record with the reason, after which the check counts the pyramid failure as accepted, the accepted pyramid becomes the baseline, and the close passes, and by a fixture where a failing test besides the pyramid still fails the check with no choice.
R19: A mark in a test (a name prefix, a Rust attribute, a junit tag) gives its level or its technique ahead of the rules by place; shown by a cargo fixture and a junit fixture where a marked test in the same folder as unit tests gets the level component, and the report names a mark that no level or technique knows.

## Constraints

- Every feature of stapel works for any project it runs in, not only this Rust repository: kind rules, suites and CI runs are data per language and runner, and project choices live in the project's `stapel.toml`. The person said so on 2026-11-26 (ledger) when the item was added.
- Agents run cargo only through `stapel cargo`, at most `[cargo] slots` runs at once on the machine, and heavy runs with at most `-j 4` and 4 test threads (CLAUDE.md, STP-4: parallel nested builds crashed the machine). This bends R5, where each kind's time costs one cargo run of its own, and R17, where e2e tests with real cargo run one at a time.
- The suite of `stapel check` here already takes about 11 min with `jobs = 4` and `test_threads = 4` (STP-45), and the timeout is 9000 s; a per-kind timing run in every check would add to it, which bends where the time of R5 is measured.
- The CI run installs stapel with `cargo install` at a pinned revision (`.github/workflows/stapel-check.yml`); the workflows of R12 follow the same form.

## Decisions

- 2026-11-26: Decision: the person added plan item 5.2d, widened it the same day with 5.2d-1 and 5.2d-2, and asked for its requirements and tickets from `stapel-elaborate` after STP-45 (ledger).
- 2026-11-26: Decision: the person placed 5.2d after 5.2b and before 5.3 (ledger); 5.2b has no ticket yet, so the first ticket of this draft waits for 5.2b.
- 2026-11-27: Decision: the person chose a target as a range of shares per level with a tolerance; the guard fails when a level's share moves away from its target range by more than the tolerance against the baseline.
- 2026-11-27: Decision: the person chose kind rules shipped with stapel per language and runner, which a project adds to or overrides in its `stapel.toml`.
- 2026-11-27: Decision: the person chose that for cargo the report times each level and technique with a run of only its tests, under the cargo slots.
- 2026-11-27: Decision: the person chose GitHub Actions as the only CI system that `stapel init` writes runs for.
- 2026-11-27: Decision: the person added the kind component (one component tested alone, its neighbours replaced) to the kinds of R1.
- 2026-11-27: Decision: the person chose that a `stapel check` that fails only because of the pyramid guard gives the person a choice to go on, by the person's decision (R18); an agent cannot take this choice.
- 2026-11-27: Assumption: the guard of R7 and the lint of R9 compare counts and shares only; the run time of R5 is measured by the report at a release, on a schedule or on demand, not in every `stapel check`.
- 2026-11-27: Assumption: a failing test fails a release or schedule run; only the pyramid lines of the report block nothing (R13).
- 2026-11-27: Assumption: the techniques are example, golden and property-based; other techniques (fuzz, snapshot, metamorphic) are out of this feature.
- 2026-11-27: Assumption: the shares of the pyramid are taken over the tests with a kind; tests with no kind are counted apart (R4, R15).
- 2026-11-27: Decision: the person chose two axes for the kind of a test: a level (the pyramid) and a technique (the report); a golden test is a technique, not a level.
- 2026-11-27: Decision: the person chose that N of R15 and R16 and the tolerance of R6 are defaults in `stapel.toml` that a project changes; tickets 4 and 7 choose the default values.
- 2026-11-27: Decision: the person chose that a pyramid accepted in the dialog of R18 becomes the new baseline.
- 2026-11-27: Decision: the person chose how a level is found: rules by place and form by default, a mark in the test ahead of them where the place says nothing, and the project's own rules and component names in its `stapel.toml`; a test with neither gets no kind (R4).
- 2026-11-27: Decision: the person chose that a departure in R9 is judged on the pyramid of the project after the ticket, the same pyramid that the guard of R7 compares.

## Breakdown

- 1. Kinds and counts for cargo: covers R1 (cargo part), R2 (cargo rule set), R4, R5 (counts part); after none; its report and rule data are used by tickets 2 and 3; done when the report on this repository prints the count of each kind and names the tests with no kind.
- 2. Run time per level and technique: covers R5 (time part, cargo by a run per level and technique, junit by its times); after 1 (it adds the time column to the report of ticket 1); its times are used by the release report of ticket 6; done when the report on this repository prints a time for each level and each technique.
- 3. junit rules and project rules: covers R1 (junit part), R2 (junit rule set), R3, R19; after 1 (it extends the rule data and the `stapel.toml` keys of ticket 1) and 2 (both change the report's table); its kinds are used by the target of ticket 4; done when the report on a TypeScript junit fixture with a `tests/guards/` project rule and a marked component test gives each test its kind.
- 4. The target and the guard: covers R6, R7, R8, R18; after 3 (the target names the levels of all rule sets, and it adds keys to the same `stapel.toml` table); its target and guard are used by tickets 5 and 6; done when `stapel check` fails on a fixture worse than its baseline and passes after the person accepts the pyramid in the dialog of R18.
- 5. Kinds in the Test plan: covers R9, R10; after 4 (the lint and the spec review compare with the target of ticket 4); its departure Decisions are used by the flags of ticket 7; done when the ticket lint fails on a fixture ticket whose Test plan departs from the pyramid with no dated Decision.
- 6. Named suites and CI runs: covers R11, R12, R13; after 2 (the release report carries the times), 4 (the push run runs the guard) and 5 (both add keys to the `stapel.toml` config of ticket 4); its release report is used by the flags of ticket 7 and its release suite by ticket 8; done when an init on a fixture writes the golden GitHub Actions runs of the push, release and schedule suites.
- 7. The upkeep of the target: covers R14, R15, R16; after 5 (departures come from the Decisions of ticket 5) and 6 (the flags go into the release report of ticket 6); its proposed target is used by ticket 8; done when an init on a library fixture writes its proposed target to `stapel.toml` only after the dialog.
- 8. The pilot in this repository: covers R17; after 7 (the target of this repository is proposed and confirmed through ticket 7, and the e2e tests run in the release suite of ticket 6); done when the release suite of this repository prints a report with no test without a kind and with property-based and e2e counts above zero.
