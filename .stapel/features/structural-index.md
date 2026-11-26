# The structural index and stapel q

## Description

Plan item 5.2 (`docs/PLAN.md`). When the work is done, `stapel` keeps a structural index of the tracked
files of a repository in `.stapel/index/`, outside git: the symbols, the import graph and the test files of
each module, taken from a tree-sitter parse. The command `stapel q` answers four queries over it: `symbol`,
`refs`, `tests` and `impact`. Each query first re-parses the files whose content changed, so the answer is
true for the working tree as it is. Rust and TypeScript are the first languages. This is the base of plan
item 5.3, where the process uses the blast radius of a change, and of 5.4 and 5.5.

## Outcome

An agent or the person can ask which places a change of a set of files touches and which tests cover them,
without reading the code by hand.

## Done when

- `stapel index` on this repository builds the index, and `git status` shows no change after it.
- `stapel q symbol`, `refs`, `tests` and `impact` answer on this repository, in text and with `--json`.
- A file edited outside stapel is seen by the next query without `stapel index`.
- The four queries answer on a TypeScript fixture project.
- The TypeScript adapter adds a grammar library, a query file and one adapter entry, and no other Rust code.
- The `stapel` binary does not grow with a language: each grammar is a library of its own, loaded only when needed.

## Boundaries

- Out: precise references through a SCIP indexer (plan item 5.5); the structural answers may be imprecise.
- Out: the blast radius in the process, the brief, the packs and the MCP tool (plan item 5.3).
- Out: the public API snapshot and the dependency dialog (plan item 5.4).
- Out: Go, Python and any language other than Rust and TypeScript.
- Out: grammar libraries that the person builds or brings, and query files that a project overrides.
- Out: a speed limit as a check; the speed is measured and written in each ticket.
- Must not change: the behaviour of the existing `stapel` commands; `stapel check` and `stapel next` do not read the index.

## Requirements

R1: The person or an agent can rebuild the whole index with one command; shown by `stapel index` writing `.stapel/index/` and printing the count of files and symbols per language.
R2: The index is never tracked by git; shown by a clean `git status` after `stapel index` in a fresh repository.
R3: Every query answers for the working tree as it is; shown by a test that changes, adds and removes a tracked file outside stapel, and the next query shows each change without `stapel index`.
R4: A change of the stapel version rebuilds the index without a command; shown by a test where an index written by another version is rebuilt by the next query.
R5: The person or an agent can find where a symbol is defined; shown by `stapel q symbol NAME` printing the path, the line and the kind of each definition.
R6: The person or an agent can find the places that name a symbol; shown by `stapel q refs NAME` printing the path and the line of each place.
R7: The person or an agent can find the test files of a module; shown by `stapel q tests PATH` printing the test files of the module at PATH.
R8: The person or an agent can find the blast radius of a set of files; shown by `stapel q impact PATH...` printing the files that depend on the set through the import graph, and their test files.
R9: A language is added as data: a grammar library, a query file and one adapter entry, with no other code; shown by the TypeScript ticket, whose diff outside its query file and adapter entry is only the grammar library.
R10: Rust and TypeScript files are indexed; shown by the four queries answering on this repository (Rust) and on a TypeScript fixture project.
R11: Every query has a text form and a `--json` form with the same data; shown by a test of each query that compares the two forms.
R12: A file in a language with no adapter, or a file with syntax errors, does not stop the index; shown by a test where such files are counted and skipped, and the other files are indexed.
R13: The `stapel` binary does not grow with each language, and a grammar is loaded only when the repository has files of its language; shown by a test where a query on a repository with no TypeScript files loads no TypeScript grammar, and by the binary holding no grammar code.
R14: `stapel` uses only the grammar library it was released with; shown by a test where a library with another content hash or an unfitting ABI version is refused with a message that names the file, and the files of that language are not indexed.
R15: A missing grammar library stops only its language; shown by a test where the query prints a message that names the missing library and how to install it, and the other languages are indexed.
R16: A grammar update re-indexes only the files of its language; shown by a test where a new grammar hash re-parses the files of that language and keeps the others.
R17: Each grammar update is checked before its release; shown by a test per language that parses a sample file and compares the symbols, imports and tests found with an expected list.

## Constraints

- Each grammar is a dynamic library of our own, a thin wrapper over the grammar crate of the tree-sitter
  project, built and released together with `stapel` and pinned by `Cargo.lock`. The person chose this
  because a grammar compiled into the binary makes `stapel` grow with each language, and a library that the
  person builds or brings, or the external tool `ast-grep`, lets the result change without our tests
  (chat, 2026-11-25).
- `stapel` is installed with `cargo install` (the CI workflow does so), and `cargo install` installs
  binaries only, not dynamic libraries; the delivery of the grammar libraries must work with this.
- The index trusts the working tree, not git events: a change made outside a ticket must be seen by the next
  query (plan item 5.2).
- Heavy test runs use at most `-j 4` and 4 test threads (CLAUDE.md); the C build of the grammars adds to
  each build, which bends the choice of how many grammars a test binary links.

## Decisions

- 2026-11-26: Decision: the person chose that ticket 2 (refs) takes six deferred findings of STP-44 (SR2-11, DR2-3/XR2-2, SR1-15, XR2-4, DR2-4, CR2-3); DR1-6, XR2-5 and XR2-6 stay deferred.
- 2026-11-25: Decision: the person chose the breakdown with an end-to-end Rust ticket first, then refs, then tests and impact, then the TypeScript adapter.
- 2026-11-25: Decision: the person first chose grammars compiled into the binary and built-in query files; replaced the same day by the next line.
- 2026-11-25: Decision: the person chose grammar libraries of our own, loaded when needed, built and released together with `stapel`; not `ast-grep`, not libraries the person builds.
- 2026-11-25: Decision: the person chose text output and a `--json` flag for `stapel q`.
- 2026-11-25: Decision: the person chose no speed limit in Done when; the speed is measured, not checked.
- 2026-11-25: Assumption: `refs` matches by name and by the import graph, not by type resolution; it may list a place that names another symbol of the same name.
- 2026-11-25: Assumption: the test files of a Rust module are the files under a crate's `tests/` that import the crate, and the module's own `#[cfg(test)]` block; the test files of a TypeScript module are the `*.test.ts` and `*.spec.ts` files that import it.
- 2026-11-25: Assumption: each worktree has its own `.stapel/index/`, and two queries at the same time in one worktree leave the index whole.
- 2026-11-25: Open question: whether a new file that is not yet tracked and not ignored is indexed, since a builder's new file is untracked until its commit.
- 2026-11-25: Open question: whether the Rust import graph resolves imports between the crates of a workspace, or only inside one crate.
- 2026-11-25: Decision: the person chose that `stapel` builds the grammar libraries from the pinned source on the machine, so they reach a machine where `stapel` comes from `cargo install`; not a release archive.
- 2026-11-25: Decision: the person chose that the query files are built into `stapel`, because the release of `stapel` pins the grammar version anyway; a query file on disk would change the result without our tests.

## Breakdown

- 1. The Rust index and `stapel q symbol`: covers R1, R2, R3, R4, R5, R11 (the `symbol` part), R12, R9 (the adapter entry form, with Rust as its one entry), R10 (the Rust part), R13, R14, R15, R16, R17 (the Rust part); after none; done when `stapel q symbol` finds a function of this repository that a test edits outside stapel, through the Rust grammar library.
- 1a. If ticket 1 outgrows 12 Plan steps, the grammar loader (R13, R14, R15, R16) is split into a ticket of its own before ticket 1, and ticket 1 comes after it.
- 2. The import graph and `stapel q refs`: covers R6, R11 (the `refs` part), R8 (the import graph that `impact` uses); after 1; done when `stapel q refs` lists the places of this repository that name a function of `stapel-core`. It also takes six findings deferred at the close of ticket 1 (STP-44): SR2-11 (the grammar folder keyed by the grammar version too), DR2-3 and XR2-2 (a query that does not compile against a library gives the refused line), SR1-15 (a clear message when the write into the common `.git/stapel/` is blocked), XR2-4 (`grammar build` refuses a bare repository), DR2-4 (the `index:` line counts removed files after a stamp rebuild), CR2-3 (the query compiled once per run). If they push the ticket over 12 Plan steps, they become ticket 2a of their own, after 1 and before 2.
- 3. `stapel q tests` and `stapel q impact`: covers R7, R8, R11 (the `tests` and `impact` parts); after 2; done when `stapel q impact` on a file of `stapel-core` lists the dependent files and their tests.
- 4. The TypeScript adapter: covers R9 (the proof), R10 (the TypeScript part), R17 (the TypeScript part); after 3; done when the four queries answer on a TypeScript fixture project with a diff of only the grammar library, the query file and the adapter entry.
