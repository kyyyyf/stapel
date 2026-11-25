# STP-44 — The Rust index and stapel q symbol

## Description

Source: `docs/PLAN.md`, phase 5, item 5.2; the draft `.stapel/features/structural-index.md`, ticket 1 of
its Breakdown (R1 to R5, R9 to R17 in their Rust parts).

### Frame

**Problem.** An agent finds a definition by grepping code; `crates/stapel-index` is empty. The index must
answer for the working tree as it is, and `stapel` must not grow with each language.

**Plan item:** 5.2.

**Risk tags:** `interface` (three new commands, their lines and exit codes), `data` (the index and the
grammar folder).

**Non-goals.** `refs`, `tests`, `impact` and the import graph (next tickets); languages other than Rust;
items 5.3 and 5.5; a speed limit; removal of libraries of older versions; libraries the person brings;
query files on disk; a build during a query.

**Promise (closed list).**

1. Build: `stapel grammar build` builds the Rust grammar library of the version `stapel` pins, with cargo
   under the machine-wide lock, into `<git common folder>/stapel/grammars/<stapel version>/`, shared by
   every worktree, and records its content hash. A failed build exits non-zero and leaves no library.
2. Load: a query loads the library only when there is a Rust file, and refuses one whose hash, grammar
   version or ABI version does not fit, naming the file and `stapel grammar build`.
3. Missing: with no library, a query names `stapel grammar build`, skips the Rust files, exits non-zero.
4. Files: tracked files present in the tree and untracked files git does not ignore; a file with no
   adapter is counted and skipped; a file with syntax errors is indexed as far as it parses and counted.
5. Freshness: each query re-parses the files whose content hash changed and drops removed ones; a new
   stapel version or query file rebuilds all, a new library hash re-parses its language only.
6. `stapel index` rebuilds all and prints the counts of files, symbols, skipped and erroneous files.
7. `stapel q symbol NAME` prints `path:line kind name` per definition named exactly NAME, in path and line
   order, or the same as JSON with `--json`; eleven Rust kinds; none found: exit 1.
8. Store: `.stapel/index/` of each worktree, never tracked, whole after two queries at once.
9. Adapter: a language is one entry (extensions, library, built-in query file); `stapel` links no grammar.
10. Sample: a test parses a sample file through the library against an expected list.

Not promised: paths not UTF-8 or with a newline; symlinks and files over 1 MB (skipped); a build offline
with no cached source; a read-only common folder; Windows; definitions inside or made by macros; a
deliberate edit of the record.

**Prior art.** STP-42 (cargo slots); STP-18 (exit codes); the `tags` queries of tree-sitter grammars.

**Size.** About 900 source and 900 test lines; 16 Plan steps, over 12 by the person's choice. Route:
full, auto.

### Decisions

Typed lines (`docs/decisions/0003-ticket-documents-from-bmad.md`): Decision, Assumption, Open question.

- 2026-11-25: Decision (the human): grammars are libraries of our own, built by `stapel` from the source
  version it pins; query files are built into `stapel`. Rejected: grammars compiled into the binary;
  `ast-grep`; libraries the person builds; query files on disk.
- 2026-11-25: Decision (scope, the human): one ticket for the library, the index and `stapel q symbol`,
  though it is over 12 Plan steps. Rejected (recommended): the library first as a ticket of its own.
- 2026-11-25: Decision (build, the human, recommended): a command `stapel grammar build`; a query never
  builds. Rejected: a build during the first query; a build in `stapel init`.
- 2026-11-25: Decision (place, the human): one grammar folder per repository, in the common git folder, for
  main and every worktree. Rejected: one folder per machine (`~/.cache/stapel/grammars/`).
- 2026-11-25: Decision (files, the human, recommended): untracked files that git does not ignore are
  indexed. Rejected: tracked files only.
- 2026-11-25: Decision (output, the human, recommended): text lines and `--json`.
- 2026-11-25: Decision (syntax errors, agent): a file with syntax errors is indexed with what the parse
  finds and counted, not skipped as the draft's R12 says, because a file in an agent's edit often has an
  error and skipping it would hide all its symbols.
- 2026-11-25: Assumption: a cargo build of a grammar crate needs a C compiler, which `cargo install` of
  `stapel` already needs.

## Spec

Terms:

- **Common folder**: the folder that `git rev-parse --git-common-dir` prints for the current folder,
  made absolute; main and every linked worktree of a repository share it.
- **Grammar folder**: `<common folder>/stapel/grammars/<stapel version>/`.
- **Library**: the file `rust-<first 16 hex digits of its SHA-256>.dylib` (macOS) or `.so` (Linux) in the
  grammar folder; its **record** is `rust.json` next to it, with the grammar crate version, the library's
  file name and its SHA-256. A build writes a new library file, then replaces the record; it never
  replaces a library file in place.
- **Build folder**: `<common folder>/stapel/grammars/build-<process id>-<random hex>/`, made by one build
  and removed at its end.
- **Standalone manifest**: the `Cargo.toml` that a build writes for the wrapper: the package with explicit
  fields (no `.workspace = true`) and an empty `[workspace]` table, so cargo does not take the build
  folder for a part of an enclosing workspace.
- **Pinned version**: the version of the crate `tree-sitter-rust` in `crates/stapel-grammar-rust/grammar.lock`,
  which `stapel` carries built in.
- **Index stamp**: the stapel version and the SHA-256 of the built-in query file of each language.
- **File list**: the files that `git ls-files -z --cached --others --exclude-standard` prints when run at
  the top of the working tree (tracked files and untracked files that git does not ignore), with paths
  relative to that top, in any current folder of the tree, without the paths under `.stapel/index/`.
- **Indexed file**: a file of the list that exists, is a regular file (no symlink), has at most 1 MB
  (1,048,576 bytes), can be read, has an adapter (`.rs`), and whose language has a usable library.
- **Rust candidate**: a file of the list with the extension `.rs` that exists, is a regular file, has at
  most 1 MB and can be read.
- **Skipped**: every other file of the list, except tracked files that are absent from the working tree
  when the list is taken; a file that vanishes or cannot be read during the query is skipped.
- **Query**: `stapel index` or `stapel q symbol`.
- **Library line**: `stapel: no grammar library for rust: run stapel grammar build` (missing) or
  `stapel: grammar library <path> refused: <reason>; run stapel grammar build` (refused), with `<reason>`
  one of `content hash differs from its record`, `grammar version <v> is not <pinned>`,
  `cannot load: <error>`, `no symbol stapel_grammar_language`, `tree-sitter refused its ABI version`.
  The checks run in this order: record, library file, version, hash, load, symbol, ABI; the first that
  fails gives the line.
- **Path order**: the byte order of the UTF-8 paths.

| № | Criterion | Test |
|---|---|---|
| AC-1 | WHEN `stapel grammar build` runs in a git working tree THE it takes a slot of the machine-wide cargo lock, writes the wrapper crate of the pinned version with its standalone manifest and `grammar.lock` as its `Cargo.lock` into a new build folder, runs `cargo build --release --locked` there, puts the library into the grammar folder under its hash name through a temporary file and a rename (keeping a file of that name whose hash fits), writes the record through a temporary file and a rename, removes the build folder, and prints `built: rust <pinned> <library path>`, exit 0; the standalone manifest names the same dependencies as the member manifest of `crates/stapel-grammar-rust`, plus `tree-sitter`. | `grammar_build::builds_into_the_common_folder`, `grammar_build::standalone_manifest_matches_the_member` |
| AC-2 | IF cargo exits non-zero or leaves no library THEN `stapel grammar build` prints cargo's output and `stapel: grammar build failed for rust`, exits 1, the grammar folder holds the files it held before, and no build folder is left. | `grammar_build::a_failed_build_leaves_the_folder` |
| AC-3 | WHEN `stapel index` runs in a linked worktree THE it uses the library that `stapel grammar build` built in main, with no build of its own. | `index_query::worktrees_share_the_grammar` |
| AC-4 | IF the list holds a Rust candidate and the grammar folder has no readable record (none, not JSON, or without `version`, `file` or `sha256`), or a record whose library file is missing or cannot be read THEN the query prints the missing library line on standard error, skips the Rust files, drops the stored Rust definitions, prints its answer from the other files and exits 2, also when that answer is empty; the dropped definitions give no `index:` line. | `index_query::missing_library`, `index_query::no_index_line_after_a_missing_library`, `index_query::unreadable_record_is_missing` |
| AC-5 | IF the library's SHA-256 differs from its record, the record's version is not the pinned one, the file cannot be loaded, it has no symbol `stapel_grammar_language`, or tree-sitter refuses its ABI version THEN the query prints the refused library line with the reason of the first failing check (Terms) and handles the Rust files as AC-4, exit 2. | `index_query::refuses_a_changed_library`, `index_query::check_order_takes_the_first_fault` |
| AC-6 | WHEN the file list holds no Rust candidate THE the query loads no library and prints no library line, even when the library is missing. | `index_query::no_rust_files_load_nothing` |
| AC-7 | WHEN a query runs, from the top of the tree or from a subfolder THE it indexes the indexed files of the whole file list with paths relative to the top, names with spaces and non-ASCII UTF-8 letters as written; deleted tracked files and ignored files are not in it. | `index_query::file_list` |
| AC-8 | WHEN a Rust file has syntax errors THE the query indexes the definitions its parse finds and counts the file as one with syntax errors. | `index_query::syntax_errors` |
| AC-9 | WHEN a file of the list was changed, added or removed since the last query THE the next `stapel q symbol` re-parses the changed and added files, drops the removed ones, prints `index: <p> parsed, <r> removed` on standard error when either count is above 0 (`<r>` counts files gone from the list; dropped definitions of AC-4 are not counted), and answers for the new content; unchanged files are not parsed. | `index_query::sees_edits_outside_stapel` |
| AC-10 | WHEN the index stamp differs from that of the running `stapel` THE the next `stapel q symbol` parses every indexed file; WHEN the library's SHA-256 differs from the one the index used THE it parses every indexed file of that language; in both cases its `index:` line counts them. | `index_query::rebuilds_on_version_change`, `index_query::reparses_on_grammar_change` |
| AC-11 | WHEN `stapel index` runs THE it rebuilds the whole index and prints `rust: <f> files, <s> symbols` (files with syntax errors included; no such line when the Rust library is missing or refused), then `skipped: <n> files` and `syntax errors: <n> files`, and no `index:` line; exit 0, or 2 after a library line. | `index_query::index_prints_counts` |
| AC-12 | WHEN `stapel q symbol NAME` runs THE it prints one line `<path>:<line> <kind> <name>` per definition whose name equals NAME exactly, in path order (Terms) then line order, with the path relative to the top of the tree and the line (from 1) of the name. | `index_query::symbol_text`, `index_query::path_order_mixes_tracked_and_untracked` |
| AC-13 | WHEN the index holds Rust definitions THE their kinds are: `fn` for a `fn` outside an `impl` or `trait` body (in a module, in an `extern` block, or nested in a `fn` body); `method` for a `fn` in an `impl` or `trait` body, with or without a body; `struct`, `enum`, `union`, `trait`; `type` and `const`, also inside an `impl` or `trait` body; `static`, also in an `extern` block; `mod`, with or without a body; `macro` for `macro_rules!`; each definition gives one line, two `#[cfg]` variants two lines. | `grammar_load::all_kinds` |
| AC-14 | WHEN `stapel q symbol NAME --json` runs THE it prints one JSON array of objects `{"path","line","kind","name"}` with the data and order of AC-12, `[]` after a library line with nothing found. | `index_query::symbol_json` |
| AC-15 | IF no definition has the name and no library line was printed THEN `stapel q symbol` prints `stapel: no definition of <NAME>` on standard error and exits 1, with no standard output (`--json` too); after a library line it prints no such message and exits 2. | `index_query::symbol_not_found` |
| AC-16 | WHEN a query runs THE the index lives in `.stapel/index/` of the worktree and `git status --porcelain` is the same before and after it, with or without a `.gitignore` entry; two queries started at the same time both answer and leave an index that a third query reads. | `index_query::index_is_not_tracked`, `index_query::concurrent_queries` |
| AC-17 | WHEN the library built from the pinned version parses `crates/stapel-cli/tests/data/grammar/sample.rs` THE the definitions found equal `sample.expected`; the adapter table holds one entry (Rust, extension `rs`, library `rust`, the query file `crates/stapel-index/queries/rust.scm` built in); the manifests of `stapel-cli` and of the path crates it depends on (not as dev-dependencies) name `tree-sitter` and no `tree-sitter-<language>` crate; `tree-sitter-language`, the crate of the ABI type, is no grammar. | `grammar_load::sample_symbols`, `grammar_load::adapter_table_is_data`, `grammar_load::binary_links_no_grammar` |
| AC-18 | IF the current folder is not in a git working tree THEN `stapel grammar build`, `stapel index` and `stapel q symbol` print `stapel: not a git working tree` and exit 1. | `index_query::outside_git` |

### Guarantees

- **Promised:** the Frame's promise items: item 1 by AC-1 and AC-2; item 2 by AC-3, AC-5 and AC-6; item 3
  by AC-4; item 4 by AC-7 and AC-8; item 5 by AC-9 and AC-10; item 6 by AC-11; item 7 by AC-12 to AC-15;
  item 8 by AC-16; items 9 and 10 by AC-17. Unchanged: every existing command; `stapel check`, `stapel
  next` and the hook do not read the index.
- **Not promised:** the Frame's list; a definition whose name is a raw identifier (`r#type`) is listed
  under the name as written; two definitions on one line in no further order; a library of another stapel
  version (left in its folder); a change of the extraction code with no change of the stapel version or
  the query file; a NAME that starts with `-` unless given after `--`; the build folder of a killed build
  (left in place); the folder of another pinned version under one stapel version (SR2-11, deferred).

**Author self-check (item 7).** Done on 2026-11-25: each promise item maps to a criterion; the cargo
lock already gives slots across worktrees (STP-42), so AC-1 takes one; the index folder ignores itself,
so AC-16 holds where `stapel init` has not run; the tests run a fake `cargo` for AC-1 and AC-2, so no
test runs a nested cargo build (STP-4); a cargo experiment showed that a dev-dependency with crate types
`cdylib` and `rlib` leaves its `.dylib` in `target/debug/deps` (F-7).

## Test plan

Integration tests in `crates/stapel-cli/tests/`: new files `grammar_build.rs`, `grammar_load.rs` and
`index_query.rs`; data in `crates/stapel-cli/tests/data/grammar/`. The real library comes from the
dev-dependency `stapel-grammar-rust`, which cargo builds next to the test binaries (F-7); the tests put
it into a fixture's grammar folder through a fake `cargo` on `PATH` that `stapel grammar build` runs, so
the record is written by the code under test. Light tests: `stapel cargo test -j 4 -p stapel-cli --test
index_query -- --test-threads=4` (and `--test grammar_build`, `--test grammar_load`); before each GREEN
also `--test ticket_drift` and `--test fixtures`.

| Category | Cases | Tests |
|---|---|---|
| Build | fake cargo that writes a library and copies the manifest it finds: the library under its hash name and the record in `<common>/stapel/grammars/<version>/`, the printed line, the cargo arguments `build --release --locked`, the manifest with an empty `[workspace]` table and no `.workspace = true`, a slot of the lock held during the run, no build folder after it; a second build with the same bytes keeps the library file (same inode) | `grammar_build::builds_into_the_common_folder` |
| Failed build | fake cargo exit 1; exit 0 with no library: exit 1, the message, an earlier library and record unchanged, no other file in the grammar folder, no build folder | `grammar_build::a_failed_build_leaves_the_folder` |
| Lock agreement | the versions of `tree-sitter-rust` and `tree-sitter` in `grammar.lock` equal those in the workspace `Cargo.lock` | `grammar_build::wrapper_lock_matches_the_workspace` |
| Worktrees | build in main, `git worktree add`, `stapel index` in the worktree: its `rust:` line counts the worktree's files | `index_query::worktrees_share_the_grammar` |
| Missing | `stapel index` with no grammar folder, and with a record whose library file is gone: the line, exit 2, no `rust:` line, `skipped` counts the Rust files | `index_query::missing_library` |
| Check order | a record with another version and a library whose hash differs: the version reason | `index_query::check_order_takes_the_first_fault` |
| Unreadable record | a record `garbage`, a record without `sha256`, a library file of mode 000: the missing line, exit 2 | `index_query::unreadable_record_is_missing` |
| Dropped, no line | an index with Rust definitions, the record removed: `q symbol` prints no `index:` line | `index_query::no_index_line_after_a_missing_library` |
| Standalone manifest | the `[dependencies]` of `standalone.toml` equal those of the member `Cargo.toml` plus `tree-sitter`; crate type `cdylib`; an empty `[workspace]` | `grammar_build::standalone_manifest_matches_the_member` |
| Refused | one byte of the library changed; the record's version changed; a text file as the library with a fitting record (cannot load); a library of another crate with no `stapel_grammar_language` (the `libdep` kind of F-7): each reason, exit 2 | `index_query::refuses_a_changed_library` |
| No Rust | no grammar folder and a repository with only `README.md`; with a deleted tracked `a.rs`; with only a symlink `x.rs`: `stapel index` exit 0, no library line | `index_query::no_rust_files_load_nothing` |
| File list | tracked, untracked, ignored, deleted tracked, a symlink to a `.rs` file, a `.rs` file of 1,048,577 bytes and one of 1,048,576, a `.txt` file, a file with mode 000, a path with a space, `é.rs`; the same answer from a subfolder; a second `stapel index` gives the same `skipped` count | `index_query::file_list` |
| Syntax errors | a file with one broken `fn` and one valid `struct`: the `struct` found, `syntax errors: 1 files` | `index_query::syntax_errors` |
| Freshness | change a name in a file, add a file, remove a file, each outside stapel: the next `q symbol` shows it and the `index:` line counts it; an unchanged second query prints no `index:` line | `index_query::sees_edits_outside_stapel` |
| Version | an index whose stamp has another version; one whose query hash differs: the next `q symbol` prints `index: <n> parsed, 0 removed` with `<n>` the indexed files | `index_query::rebuilds_on_version_change` |
| Grammar change | the record pointing to a library with another hash: the next `q symbol` prints `index: <n> parsed, 0 removed` with `<n>` the Rust files | `index_query::reparses_on_grammar_change` |
| Counts | three `.rs` files with 2, 1 and 0 definitions and one `.txt` file: `rust: 3 files, 3 symbols`, `skipped: 1 files`, `syntax errors: 0 files`, no `index:` line | `index_query::index_prints_counts` |
| Symbol | two definitions of one name in two files and one in a nested folder, in `B.rs` and `a.rs` (byte order puts `B.rs` first), a name that is a prefix of another: the lines in order | `index_query::symbol_text` |
| Tracked and untracked | an untracked `a.rs` and a tracked `b.rs` that define one name: `a.rs` first | `index_query::path_order_mixes_tracked_and_untracked` |
| Kinds | each construct of AC-13 in the sample of the test, with two `#[cfg]` variants of one `fn` | `grammar_load::all_kinds` |
| JSON | the same fixture as Symbol: the array equals the text lines field by field; after a missing library: `[]`, exit 2 | `index_query::symbol_json` |
| Not found | a name with no definition, an empty name: the message, exit 1, no standard output; after an index with a Rust name, the record removed: no output, no message, exit 2; a build, then the name is found | `index_query::symbol_not_found` |
| Store | `git status --porcelain` before and after `stapel index` in a repository with no `.gitignore` entry | `index_query::index_is_not_tracked` |
| Concurrency | two queries started together, then a third, repeated 20 times | `index_query::concurrent_queries` |
| Sample | the definitions of `sample.rs` equal `sample.expected`; the adapter table; the manifests | `grammar_load::sample_symbols`, `grammar_load::adapter_table_is_data`, `grammar_load::binary_links_no_grammar` |
| Outside git | each of the three commands in a temporary folder outside git | `index_query::outside_git` |

### Inputs

| Input | Type | Smallest / largest | Empty | Invalid | Precision | Test |
|---|---|---|---|---|---|---|
| `NAME` of `stapel q symbol` | string | one character / any | no definition: exit 1 | a name starting with `-` is read as an option unless after `--` | exact, case-sensitive | `index_query::symbol_not_found` |
| `--json` of `stapel q symbol` | flag | — | absent: text | — | — | `index_query::symbol_json` |
| a file of the file list | bytes | 0 bytes / 1,048,576 bytes (larger skipped) | indexed with no definition | not UTF-8 or syntax errors: parsed as far as it goes; unreadable: skipped | exact bytes hashed | `index_query::file_list`, `index_query::syntax_errors` |
| the record `rust.json` | JSON | — | missing, unreadable, not JSON or without a field: the missing line | other version: the refused line | exact | `index_query::missing_library`, `index_query::unreadable_record_is_missing`, `index_query::refuses_a_changed_library` |
| the index in `.stapel/index/` | JSON | — | missing: built | unreadable or another stamp: rebuilt | exact | `index_query::rebuilds_on_version_change` |

### External states

| System | Dimension | Handling | Test |
|---|---|---|---|
| git | linked worktrees | share the grammar folder through the common folder | `index_query::worktrees_share_the_grammar` |
| git | dirty tree: staged, unstaged, untracked, ignored files | the working tree content decides; ignored files are left out | `index_query::file_list` |
| git | paths with spaces and non-ASCII UTF-8 | handled (`-z`, no quoting) | `index_query::file_list` |
| git | paths with newlines or non-UTF-8 bytes; submodules | not promised (a submodule is a folder and is skipped) | — |
| git | history shape, shallow clone, object format, detached HEAD | not read: only the file list and the working tree | — |
| git | configuration (`core.quotepath`, `GIT_*` environment) | `-z` ignores `core.quotepath`; `GIT_*` not promised | `index_query::file_list` |
| git | not a working tree | refused, exit 1 | `index_query::outside_git` |
| cargo | offline, network errors, no cached source | the build fails as AC-2 | `grammar_build::a_failed_build_leaves_the_folder` |
| cargo | an enclosing workspace `Cargo.toml` above the build folder | the standalone manifest's empty `[workspace]` table | `grammar_build::builds_into_the_common_folder`, and the stand of Design |
| cargo | `.cargo/config.toml` of the repository, `CARGO_*`, `RUSTFLAGS`, `build.target` | they apply to the wrapper build; a library at another path is "no library" (AC-2) | not promised beyond AC-2 |
| cargo | the machine-wide lock with all slots taken | the build waits with no limit | not promised beyond STP-42 |
| os | symlinks; files with no read permission | skipped | `index_query::file_list` |
| os | case-insensitive file systems | not promised: paths as git lists them | — |
| os | a common folder that is not writable | not promised | — |
| os | macOS and Linux library names; Windows | `.dylib` and `.so`; Windows not promised | `grammar_build::builds_into_the_common_folder` |

**Review Focus:** a real cargo build of the wrapper (no test runs one: the stand of Design); a library that a careless agent rebuilds by hand in the grammar folder (refused by its
hash, AC-5); the ABI reason of AC-5 (no test: a library of another ABI needs another grammar version; a
code review checks the branch); two `stapel grammar build` runs at the same time (each its own build
folder; the library through a temporary file and a rename; the record replaced whole; no test); a file that changes during a query
(the hash read before the parse; no test); a record with extra fields (read as long as its fields are
there; no test).

## Design

### Facts

- F-1: `crates/stapel-index/src/lib.rs` is empty; `stapel-cli` does not depend on `stapel-index`.
- F-2: `crates/stapel-cli/src/cargo.rs` gives `take(command)`, which waits for a slot of the machine-wide
  cargo lock in `~/.cache/stapel` and holds it until the returned guard drops (STP-42).
- F-3: `crates/stapel-cli/src/fail.rs` gives exit 2 to refusals and exit 1 to other failures (STP-18).
- F-4: `crates/stapel-cli/src/init.rs` writes `/.stapel/index/` into `.gitignore`.
- F-5: the ticket lint (`crates/stapel-cli/tests/ticket_drift.rs`) finds tests only in
  `crates/stapel-core/tests/` and `crates/stapel-cli/tests/`, so the tests of this ticket live in
  `crates/stapel-cli/tests/`.
- F-6: `tree-sitter-rust` exports `LANGUAGE`, a `LanguageFn`; the `tree-sitter` crate checks the ABI
  version of a language in `Parser::set_language` and returns an error when it does not fit.
- F-7: a cargo experiment on 2026-11-25: a crate with a path dev-dependency of crate types `cdylib` and
  `rlib` left `target/debug/deps/libdep.dylib` after `cargo test`.

### Shape

- New crate `crates/stapel-grammar-rust`: crate types `cdylib` and `rlib`, one exported function
  `stapel_grammar_language` that returns the language of `tree-sitter-rust`; for the standalone build its
  lock file `grammar.lock` and the text of the standalone manifest `standalone.toml`, both built into
  `stapel`.
- `stapel-cli` depends on `stapel-index` and has the dev-dependency `stapel-grammar-rust`; the tests find
  the library in the `deps` folder next to the test executable (F-7). At the RED of step 1 the tests do
  not compile, because the `stapel-index` API is missing.
- `stapel-index`: the adapter table (`Adapter { language, extensions, library, query }`, one entry for
  Rust, the query `crates/stapel-index/queries/rust.scm` built in with `include_str!`); `grammar` (the
  grammar folder, the record, the pinned version read from the built-in `grammar.lock`, the load with
  `libloading` of the file the record names, the hash, version, symbol and ABI checks); `index` (the file
  list, the store `.stapel/index/index.json` with the index stamp, the library hash per language and per
  file its SHA-256, language and definitions; the freshness pass; each write to a temporary file named
  with the process id and a random part, then a rename); `symbol` (the lookup). Depends on `tree-sitter`,
  `libloading`, `sha2`, `serde_json`.
- `stapel-cli`: the commands `grammar build`, `index` and `q symbol` in a new `index.rs`; `grammar
  build` takes the cargo lock through `cargo::take`, writes the wrapper files (built in with
  `include_str!`), `standalone.toml` as `Cargo.toml` and `grammar.lock` as `Cargo.lock` to the build folder, runs `cargo build --release
  --locked --target-dir <build folder>/target` there, puts the library under its hash name through a temporary file and a rename, writes the
  record through a temporary file and a rename, and removes the build folder on every exit path.
- The index folder holds its own `.gitignore` with `*`.
- The method kind: a `function_item` or `function_signature_item` whose parent is the `declaration_list`
  of an `impl_item` or a `trait_item`; every other `function_item` and `function_signature_item` is `fn`.

### Decisions (before the build)

- 2026-11-25, agent: the wrapper crate is a workspace member, so the tests get the real library from the
  dev-dependency (F-7); `grammar.lock` is checked against the workspace lock
  (`grammar_build::wrapper_lock_matches_the_workspace`).
- 2026-11-25, agent: tests never run a real nested cargo build (STP-4); a fake `cargo` on `PATH` copies
  the library that the test build made; `binary_links_no_grammar` reads manifests, not `cargo tree`.
- 2026-11-25, agent: a library line gives exit 2 over exit 1, so an agent tells "not found" from "could
  not look" (SR1-2).
- 2026-11-25, agent: the definitions of a language without a usable library are dropped, so no answer
  comes from a library that was not checked (SR1-8).
- 2026-11-25, agent: a library file carries its hash in its name and is never replaced in place, so a
  query loads the file it hashed while another build runs (SR1-9).
- 2026-11-25, agent: the 1 MB bound keeps generated files (lock files, bundles) out of the parse.
- 2026-11-25, agent (SR2-1): no test runs a real cargo build (STP-4); before code review the orchestrator
  runs `stapel grammar build` and `stapel q symbol` in this repository as a stand and records the result
  in Review.

### Decisions (as built)

- Step 1 (170fac5, cebe129): `stapel_index::adapters()`, `Library::open(&Path)` and `extract(&Adapter,
  &Library, &[u8]) -> Extract { definitions, has_errors }`; `tree-sitter` 0.25, `tree-sitter-language`
  0.1, `libloading` 0.8; `tree-sitter-rust = "=0.24.2"`. The GREEN has a `history:` line for
  `crates/stapel-cli/Cargo.toml`.
- Step 2 (f049eea, 26c8075): `extract` keeps one definition per start byte of its name; `method` wins
  over `fn`.
- Step 3 (81f85fa): 2026-11-25, agent: `binary_links_no_grammar` allows `tree-sitter-language`, the ABI
  type crate, which is no grammar (AC-17 says so since DR1-3).
- Step 4 (761c623, 308e7a3): `crates/stapel-cli/src/grammar.rs`; the record `rust.json` has `version`,
  `file` and `sha256`; the grammar folder is named by `CARGO_PKG_VERSION`; `standalone.toml` has the
  package `stapel-grammar-rust`, crate type `cdylib`, an empty `[workspace]` and `tree-sitter =
  "=0.25.10"`, so `grammar.lock` keeps that entry; `grammar.lock` was generated from the standalone
  manifest (tree-sitter-rust 0.24.2, tree-sitter 0.25.10); `grammar::pinned_version()` lives in the CLI
  crate. The tests remove `STAPEL_CARGO_LOCK_HELD`, which `stapel cargo` sets. The GREEN already handled a
  failed build.
- 2026-11-25, agent: steps 5, 7, 9 and 14 became coverage commits (5: 6077964, 7: ab341cc, 9: 38a94a6,
  14: d5883c1), and step 8 has a coverage commit (1ff37d1), because earlier GREENs already met their
  criteria.
- Step 6 (d4fac48, 812cc85): `crates/stapel-cli/src/index.rs` holds `stapel index`, the load, the file
  list, the counts and the store `.stapel/index/index.json` (per file its hash, definitions and an errors
  flag); `grammar.rs` makes `common_dir`, `hex`, `random_part` and `write_replacing` public.
- Step 8 (c25c878, a74d7d2): the library loads only for a Rust candidate.
- Step 10 (f6a72a3, 2f5967d): `index::build(top)` returns counts, the stored map and the library state;
  files of a language without a usable library are not stored.
- Step 11 (054a0d1, 2ee2196): `symbol(name, json)`. 2026-11-25, agent: with `--json` and nothing found it
  printed `[]` with exit 1 and its test expected that, against AC-15; found by the orchestrator, fixed in
  step 17.
- Step 12 (64628e1, ef4223b): `build(top, incremental)` reuses the entries whose content hash is
  unchanged; `stapel index` rebuilds all and prints no `index:` line.
- Step 13 (5cda6e3, 275b3b7): the index stores `stapel`, `queries.<language>` (query SHA-256) and
  `libraries.<language>` (library SHA-256, empty when none loaded); a new library hash clears the stored
  entries of that language.
- Step 15 (e7973cc, a24d472): `.stapel/index/.gitignore` with `*` is written when missing, through
  `grammar::write_replacing` (`.<name>.tmp-<pid>-<random>` and a rename), which every index write uses.
- Step 16 (1ea5f2b): coverage commit; 20 rounds of two queries at once on a tree with no index.
- 2026-11-25, agent (drift round 1): DR1-1 and DR1-5 become coverage step 18, DR1-2 coverage step 19 and
  the stand; DR1-4 (a path that is not UTF-8) is not promised; DR1-6 is deferred.
- e401202: `tests/cli.rs` of STP-1 counts six workspace members with `stapel-grammar-rust`, and
  `stub_packages_are_empty` checks only `stapel-mcp`, because this ticket fills `stapel-index` (XR2-7).
- 2026-11-25, the person (code review round 2): an unreadable or incomplete record and an unreadable library
  file give the missing line, as the code already did (XR2-1, DR2-2, CR2-2); DR2-3 and XR2-2 (a query that
  does not compile against a library) are deferred.

## Proof

| Criterion | Test |
|---|---|
| AC-1 | `grammar_build::builds_into_the_common_folder`, `grammar_build::standalone_manifest_matches_the_member` |
| AC-2 | `grammar_build::a_failed_build_leaves_the_folder` |
| AC-3 | `index_query::worktrees_share_the_grammar` |
| AC-4 | `index_query::missing_library`, `index_query::no_index_line_after_a_missing_library`, `index_query::unreadable_record_is_missing` |
| AC-5 | `index_query::refuses_a_changed_library`, `index_query::check_order_takes_the_first_fault` |
| AC-6 | `index_query::no_rust_files_load_nothing` |
| AC-7 | `index_query::file_list` |
| AC-8 | `index_query::syntax_errors` |
| AC-9 | `index_query::sees_edits_outside_stapel` |
| AC-10 | `index_query::rebuilds_on_version_change`, `index_query::reparses_on_grammar_change` |
| AC-11 | `index_query::index_prints_counts` |
| AC-12 | `index_query::symbol_text`, `index_query::path_order_mixes_tracked_and_untracked` |
| AC-13 | `grammar_load::all_kinds` |
| AC-14 | `index_query::symbol_json` |
| AC-15 | `index_query::symbol_not_found` |
| AC-16 | `index_query::index_is_not_tracked`, `index_query::concurrent_queries` |
| AC-17 | `grammar_load::sample_symbols`, `grammar_load::adapter_table_is_data`, `grammar_load::binary_links_no_grammar` |
| AC-18 | `index_query::outside_git` |

Not in a criterion: `grammar_build::wrapper_lock_matches_the_workspace` (the pin of Design).

## Plan

Route: full (tags `interface`, `data`), auto.

1. Sample (AC-17; tests `grammar_load::sample_symbols`, `grammar_load::adapter_table_is_data`): the RED
   adds the tests, `sample.rs` and `sample.expected`; the GREEN adds `stapel-grammar-rust`, the adapter
   table, `rust.scm` and the load of a library file in `stapel-index`.
2. Kinds (AC-13; test `grammar_load::all_kinds`): the RED adds the test; the GREEN completes `rust.scm`
   and the method kind.
3. No grammar linked (AC-17; test `grammar_load::binary_links_no_grammar`): a coverage commit only.
4. Build (AC-1; tests `grammar_build::builds_into_the_common_folder`,
   `grammar_build::wrapper_lock_matches_the_workspace`): the RED adds the tests and the fake cargo; the
   GREEN adds `stapel grammar build`, `grammar.lock` and the record.
5. Failed build (AC-2; test `grammar_build::a_failed_build_leaves_the_folder`): a coverage commit only,
   because the GREEN of step 4 already handles a failed build.
6. Index and counts (AC-3, AC-11; tests `index_query::index_prints_counts`,
   `index_query::worktrees_share_the_grammar`): the RED adds the tests; the GREEN adds `stapel index`, the
   store, and the load of the library the record names in the grammar folder of the common folder.
7. File list (AC-7; test `index_query::file_list`): a coverage commit only, because the GREEN of step 6
   already met AC-7.
8. Missing library (AC-4, AC-6; tests `index_query::missing_library`,
   `index_query::no_rust_files_load_nothing`): the RED adds `no_rust_files_load_nothing`; the GREEN loads the
   library only for a Rust candidate; `missing_library` is a coverage commit, as step 6 already printed the
   line.
9. Refused library (AC-5; test `index_query::refuses_a_changed_library`): a coverage commit only, because
   the GREEN of step 6 already ran the checks.
10. Symbol (AC-12, AC-15, AC-18; tests `index_query::symbol_text`, `index_query::symbol_not_found`,
    `index_query::outside_git`): the RED adds the tests; the GREEN adds `stapel q symbol`, the drop of
    the stored definitions of a language without a usable library, and the outside git refusal of the
    three commands.
11. JSON (AC-14; test `index_query::symbol_json`): the RED adds the test; the GREEN adds `--json`.
12. Freshness (AC-9; test `index_query::sees_edits_outside_stapel`): the RED adds the test; the GREEN
    adds the hash pass and the `index:` line.
13. Rebuilds (AC-10; tests `index_query::rebuilds_on_version_change`,
    `index_query::reparses_on_grammar_change`): the RED adds the tests; the GREEN compares the index stamp
    and the library hash.
14. Syntax errors (AC-8; test `index_query::syntax_errors`): a coverage commit only, because steps 1 and
    6 already counted files with errors.
15. Store (AC-16; test `index_query::index_is_not_tracked`): the RED adds the test; the GREEN adds the
    self-ignoring index folder and the write through a unique temporary file and a rename.
16. Concurrency (AC-16; test `index_query::concurrent_queries`): a coverage commit only.
17. JSON with nothing found (AC-15; test `index_query::symbol_json`): the RED changes the test, which
    expected `[]` with exit 1, to expect no standard output (a `history:` line); the GREEN prints the
    array only when a definition was found or a library line was printed.
18. Coverage of the check order and the dropped definitions (DR1-1, DR1-5; AC-4, AC-5; tests
    `index_query::check_order_takes_the_first_fault`, `index_query::no_index_line_after_a_missing_library`):
    a coverage commit only.
19. Coverage of the standalone manifest (DR1-2; AC-1; test
    `grammar_build::standalone_manifest_matches_the_member`): a coverage commit only.
20. Coverage of an unreadable record (XR2-1; AC-4; test `index_query::unreadable_record_is_missing`): a
    coverage commit only.
21. Coverage of the path order (XR2-3; AC-12; test `index_query::path_order_mixes_tracked_and_untracked`):
    a coverage commit only.

## Review

Stand, 2026-11-25, this repository at 1ea5f2b..fc2fc6a, debug build, macOS arm64:

- `stapel grammar build` printed `built: rust 0.24.2 <git common dir>/stapel/grammars/0.1.0/rust-0811a8132fba622f.dylib`
  in 9.9 s.
- `stapel index` printed `rust: 144 files, 3513 symbols`, `skipped: 336 files` and `syntax errors: 0 files`
  in 4.0 s.
- `stapel q symbol pinned_version` printed `crates/stapel-cli/src/grammar.rs:24 fn pinned_version` in 0.27 s.
- `stapel q symbol Library --json` gave the enum in `crates/stapel-cli/src/index.rs` and the struct in
  `crates/stapel-index/src/lib.rs`.
- `git status` stayed clean after the build and the index.

Reviewers: stapel-drift (round 1, before code review).

<!-- stapel:review:begin -->
Generated by stapel; edits between the markers are overwritten.

Findings:

| Stage | Round | Roles | HIGH | MEDIUM | LOW | False | Fates |
|---|---|---|---|---|---|---|---|
| spec-review | 1 | external | 1 | 10 | 4 | 0 | defer 1, no-change 1, patch 13 |
| spec-review | 2 | reviewer | 1 | 4 | 6 | 0 | defer 1, patch 10 |
| code-review | 1 | drift | 0 | 2 | 4 | 0 | defer 1, no-change 1, patch 4 |
| code-review | 2 | drift, external, reviewer | 0 | 1 | 14 | 0 | defer 7, no-change 3, patch 5 |

Check runs:

| Level | Result | Commit | At | Steps |
|---|---|---|---|---|
| head | pass | 27e6b10 | 2026-11-25T19:24:58Z | cached 11 |
<!-- stapel:review:end -->

## Summary

Done: `stapel grammar build` builds the pinned Rust grammar into a library under its hash name in one
grammar folder per repository, shared by all worktrees; `stapel index` builds the structural index of
the file list in `.stapel/index/`; `stapel q symbol NAME [--json]` answers from it, re-parses only
changed files, and refuses a missing or changed library with exit 2. The binary links no grammar.
On this repository: build 9.9 s, index of 144 Rust files 4.0 s, a query 0.27 s.

For the person to decide (agent decisions, overrule if wanted):

- Files with syntax errors are indexed as far as they parse, not skipped.
- `binary_links_no_grammar` allows `tree-sitter-language`, the ABI type crate.
- Steps 5, 7, 9 and 14 became coverage commits, because earlier GREENs met their criteria.
- Exit 2 (a library line) wins over exit 1 (nothing found).
- Definitions of a language without a usable library are dropped from the index, not kept stale.
- `tests/cli.rs` of STP-1 counts the new crate and no longer checks `stapel-index` as a stub.

Deferred, outside the promise (proposals to widen it):

- SR1-15: grammar build from a worktree writes into the main `.git/stapel/`, which a sandbox may block.
- SR2-11: the grammar folder is keyed by the stapel version only, so two pins at one version collide.
- DR1-6: the `--target-dir` of the build and the index folder's `.gitignore` have no criterion text.
- DR2-3, XR2-2: a built-in query that does not compile against a library skips every Rust file silently
  (the person deferred it).

Deferred, inside the promise, LOW:

- DR2-4: after a stamp rebuild, `index:` counts 0 removed even when files left the list.
- XR2-4: `grammar build` runs in a bare repository or inside `.git`, where `index` refuses.
- XR2-5: no test runs the locked standalone build; the stand shows it.
- CR2-3, XR2-6: the query is compiled per file; empty cargo output prints blank warning lines;
  a test uses `shasum`.

Process note: checks 3 and 4 hit the 5400 s limit; the suite inside `stapel check` takes about 60 min,
against 11 min with `-j 4 --test-threads=4`. The person agreed to a plan item on the observability of
`stapel check` after this close.
