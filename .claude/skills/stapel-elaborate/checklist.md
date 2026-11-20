<!-- stapel: 0.1.0 sha256:1342015035d160f339334680e8255d3d74539a999d914bffb0ad41e67210454c -->

# Checklist

Run each check over the draft. A pruned id is never reused. A finding names its check by the `EL` id.

## Draft

- EL-1: No bracketed guide of `template.md` is left unfilled.
- EL-2: Every assumed fact is a line `Assumption`.
- EL-3: Nothing in the draft contradicts a `Decision` line.
- EL-4: Outcome is one sentence.
- EL-5: Done when has three to six checks, and each check is testable or demonstrable.
- EL-6: Boundaries names at least one thing out.
- EL-7: An implication of the domain that the draft does not address is an `Open question`, never an invented answer.
- EL-8: Every sentence carries content that a later reader would act on.

## Requirements

- EL-9: Each requirement says what is possible and what shows it was met.
- EL-10: Each requirement says WHAT, not HOW. Implementation goes to the tickets.
- EL-11: Each constraint bends a design decision, and names what the person said about it.
- EL-12: Requirement ids are never reused or renumbered.
- EL-13: Every load-bearing claim of the person lands in a requirement, a constraint or a `Decision` line.

## Breakdown

- EL-14: Every requirement is covered by a ticket, or deferred with the person in a `Decision` line.
- EL-15: When tickets share a requirement id, each line says which part that ticket delivers.
- EL-16: Each ticket is one step toward Done when, and fits the limit of 12 Plan steps.
- EL-17: The done of each ticket is one runnable check.
- EL-18: Split, never shrink. "For now", "placeholder", "simplified" and "wired later" mean a second ticket.
- EL-19: A builder could write the criteria of the ticket from the draft and its line.
- EL-20: Tests go in every ticket.

## Dependencies

- EL-21: Every `after` is a real prerequisite, not a restated order.
- EL-22: No cycle in the `after` lines.
- EL-23: Tickets with no dependency path share no code, config, schema or setup. Otherwise add an `after` or merge them.
- EL-24: Shared setup belongs to the earliest ticket that needs it.
- EL-25: When one ticket relies on the output of another, both lines say so.
