# Review pack ABC-1, round 2

- ticket: ABC-1
- round: 2
- base: <base>
- head: <head>

## Paths

- copy: .stapel/scratch/review/ABC-1-r2/tree
- ticket: .stapel/scratch/review/ABC-1-r2/tree/.stapel/tickets/ABC-1/ticket.md
- diff: .stapel/scratch/review/ABC-1-r2/diff.patch

## Stat

```
 .stapel/tickets/ABC-1/state.json | 5 +++--
 code.txt                         | 1 +
 2 files changed, 4 insertions(+), 2 deletions(-)
```

## Spec

**Terms**

- **Widget**: a thing.

| № | Criterion | Test |
|---|---|---|
| AC-1 | first criterion | `t::a` |
| AC-2 | second criterion | `t::b` |

**Guarantees**

- A widget stays whole.

## Findings of round 1 with the fate patch

- R-1 (MEDIUM): first claim

## Checks

- Deletion check: for each removed line or file, say what it guaranteed and what replaces or retires it.
- Claims check: try to falsify each checkable claim of the criteria and of Design "as built" against the code.
- Verification gap: for each behaviour change, say whether an existing test would fail if the change were wrong.
